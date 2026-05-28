# 连接复用与保留策略触发收敛 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除 `repository.rs` 公开 API 每次 `Connection::open` 开销，并把保留策略从"每次写入即触发"改为"每 50 次写入触发 + 设置更新立即触发"。

**Architecture:** 三阶段重构。①新增 `service_runtime::open_connection` 集中处理 pragma；②机械迁移 `repository`/`settings`/`maintenance` 公开 API 的 `&Path → &Connection`（行为不变，纯接口改造）；③`ClipboardService` 持有 `settings_conn`、`items_conn` 双 `Mutex<Connection>` 与 `captures_since_cleanup` 计数器，capture 路径攒到 `RETENTION_TRIGGER_THRESHOLD=50` 才触发 `run_retention`。

**Tech Stack:** Rust 2021、`rusqlite = "0.32"` (bundled)、`std::sync::Mutex`、`chrono`、Tauri 2。

参考 spec：`docs/superpowers/specs/2026-05-28-connection-pool-retention-trigger-design.md`

---

## File Structure

- **Modify** `src-tauri/src/clipboard/service_runtime.rs` — 新增 `pub fn open_connection(path) -> Result<Connection>`，集中 pragma
- **Modify** `src-tauri/src/clipboard/repository.rs` — 12 个公开 API 签名 `&Path → &Connection`；`init_database` 改名 `init_schema`；私有 helper 同步
- **Modify** `src-tauri/src/clipboard/settings.rs` — `get_stored_settings`、`update_stored_settings`、`update_monitor_enabled`、`apply_retention_policy` 签名改 `&Connection`
- **Modify** `src-tauri/src/clipboard/maintenance.rs` — `purge_deleted_items`、`vacuum_database` 签名改 `&Connection`
- **Modify** `src-tauri/src/clipboard/service.rs` — 新增 `settings_conn`、`items_conn`、`captures_since_cleanup` 字段；新增 `RETENTION_TRIGGER_THRESHOLD` 常量与 `run_retention` 方法；构造与所有公开方法改为持锁取连接
- **Modify** `src-tauri/src/clipboard/repository_tests.rs` — 11 个用例模板替换
- **Modify** `src-tauri/src/clipboard/service_tests.rs` — 3 处直调 repository 改造 + 新增 2 个用例（阈值触发、计数重置）
- **Modify** `docs/clipboard-toolbox-design.md` — 同步双连接、阈值触发描述
- **Modify** `docs/2026-05-28-clipboard-toolbox-audit.md` — 标记 P1 #6 与 P1 #7 完成

不改动文件：`commands.rs`、`monitor.rs`、`storage_path_tests.rs`、`error.rs`、`models.rs`、`hash.rs`、`mod.rs`。

---

## Task 1: 新增 `open_connection` 助手

**Files:**
- Modify: `src-tauri/src/clipboard/service_runtime.rs`

将所有"开 SQLite 连接"收敛到单一入口，集中应用 WAL/synchronous/foreign_keys pragma。此 Task 只新增公开函数，不修改任何调用方，行为零变化。

- [ ] **Step 1: 在 `service_runtime.rs` 顶部追加 rusqlite import**

定位 `service_runtime.rs:1-8` 当前 import 区：

```rust
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use arboard::Clipboard;
use chrono::Local;

use super::error::ClipboardError;
use super::models::{CaptureOutcome, ClipboardSkipReason};
```

在 `use chrono::Local;` 后追加一行 `use rusqlite::Connection;`，结果如下：

```rust
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use arboard::Clipboard;
use chrono::Local;
use rusqlite::Connection;

use super::error::ClipboardError;
use super::models::{CaptureOutcome, ClipboardSkipReason};
```

- [ ] **Step 2: 在文件末尾追加 `open_connection` 函数**

文件当前末尾在 `resolve_database_path` 之后（`service_runtime.rs:63` 是 `}` 闭合 `resolve_database_path`）。在该 `}` 之后追加：

```rust

pub fn open_connection(path: &Path) -> Result<Connection, ClipboardError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "PRAGMA journal_mode=WAL; \
         PRAGMA synchronous=NORMAL; \
         PRAGMA foreign_keys=ON;",
    )?;
    Ok(connection)
}
```

- [ ] **Step 3: 编译通过**

Run: `cd src-tauri; cargo check`

Expected: 编译通过，无新警告。可能出现 `unused function: open_connection` 警告（因为还无调用方），属正常，下个 Task 会消除。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/clipboard/service_runtime.rs
git commit -m "$(cat <<'EOF'
feat(clipboard): 新增 open_connection 助手并启用 WAL pragma

集中 SQLite 连接构造逻辑：自动创建父目录、启用 WAL 与 synchronous=NORMAL、强制 foreign_keys=ON。本提交仅新增函数，不修改调用方，行为零变化。
EOF
)"
```

---

## Task 2: `repository.rs` / `settings.rs` / `maintenance.rs` 接口重构

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs`
- Modify: `src-tauri/src/clipboard/settings.rs`
- Modify: `src-tauri/src/clipboard/maintenance.rs`
- Modify: `src-tauri/src/clipboard/service.rs`
- Modify: `src-tauri/src/clipboard/repository_tests.rs`
- Modify: `src-tauri/src/clipboard/service_tests.rs`

机械改造：12 个 `repository::xxx(path, ...)` + 4 个 `settings::xxx(path, ...)` + 2 个 `maintenance::xxx(path, ...)` 全部改为接 `&Connection`，并同步所有调用方。`init_database` 改名 `init_schema` 反映"只跑 migration"语义。此 Task 单一原子提交，行为不变（service.rs 内部临时每次 `open_connection` 然后传给 repository，下个 Task 才换持锁连接）。

- [ ] **Step 1: 重写 `repository.rs`**

定位 `repository.rs` 全文。整体替换内容如下（覆盖 1-300 行）：

```rust
use rusqlite::{params, Connection, OptionalExtension, Row};

use super::error::ClipboardError;
use super::hash::preview;
use super::models::{ClipboardDateGroup, ClipboardItem};

const CONTENT_TYPE_TEXT: &str = "text";

pub fn init_schema(connection: &Connection) -> Result<(), ClipboardError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS clipboard_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content_type TEXT NOT NULL,
            content TEXT NOT NULL,
            preview TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_copied_at TEXT NOT NULL,
            copy_count INTEGER NOT NULL DEFAULT 1,
            deleted_at TEXT
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_clipboard_items_hash_active
            ON clipboard_items(content_hash)
            WHERE deleted_at IS NULL;
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_created_at_active
            ON clipboard_items(created_at)
            WHERE deleted_at IS NULL;
        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );",
    )?;
    Ok(())
}

pub fn upsert_text_item(
    connection: &Connection,
    content: &str,
    content_hash: &str,
    now: &str,
) -> Result<ClipboardItem, ClipboardError> {
    if let Some(id) = find_active_id_by_hash(connection, content_hash)? {
        update_existing_item(connection, id, now)?;
        return get_item_by_id_with_connection(connection, id);
    }
    insert_text_item(connection, content, content_hash, now)?;
    get_item_by_id_with_connection(connection, connection.last_insert_rowid())
}

pub fn list_date_groups(
    connection: &Connection,
) -> Result<Vec<ClipboardDateGroup>, ClipboardError> {
    let mut statement = connection.prepare(
        "SELECT substr(created_at, 1, 10) AS date, COUNT(*) AS count
         FROM clipboard_items
         WHERE deleted_at IS NULL
         GROUP BY date
         ORDER BY date DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ClipboardDateGroup {
            date: row.get(0)?,
            count: row.get(1)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ClipboardError::from)
}

pub fn list_items_by_date(
    connection: &Connection,
    date: &str,
) -> Result<Vec<ClipboardItem>, ClipboardError> {
    let mut statement = connection.prepare(
        "SELECT id, content_type, content, preview, content_hash, created_at, last_copied_at, copy_count
         FROM clipboard_items
         WHERE deleted_at IS NULL AND substr(created_at, 1, 10) = ?1
         ORDER BY last_copied_at DESC, id DESC",
    )?;
    let rows = statement.query_map([date], map_item)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ClipboardError::from)
}

pub fn search_items(
    connection: &Connection,
    keyword: &str,
) -> Result<Vec<ClipboardItem>, ClipboardError> {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = format!("%{trimmed}%");
    let mut statement = connection.prepare(
        "SELECT id, content_type, content, preview, content_hash, created_at, last_copied_at, copy_count
         FROM clipboard_items
         WHERE deleted_at IS NULL AND (content LIKE ?1 OR preview LIKE ?1)
         ORDER BY last_copied_at DESC, id DESC",
    )?;
    let rows = statement.query_map([pattern], map_item)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ClipboardError::from)
}

pub fn get_item_by_id(
    connection: &Connection,
    id: i64,
) -> Result<ClipboardItem, ClipboardError> {
    get_item_by_id_with_connection(connection, id)
}

pub fn soft_delete_item(
    connection: &Connection,
    id: i64,
    now: &str,
) -> Result<(), ClipboardError> {
    let changed = connection.execute(
        "UPDATE clipboard_items SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    if changed == 0 {
        return Err(ClipboardError::NotFound(id));
    }
    Ok(())
}

pub fn soft_delete_items_by_date(
    connection: &Connection,
    date: &str,
    now: &str,
) -> Result<usize, ClipboardError> {
    let changed = connection.execute(
        "UPDATE clipboard_items
         SET deleted_at = ?1
         WHERE substr(created_at, 1, 10) = ?2 AND deleted_at IS NULL",
        params![now, date],
    )?;
    Ok(changed)
}

pub fn get_i64_setting(
    connection: &Connection,
    key: &str,
    default: i64,
) -> Result<i64, ClipboardError> {
    let value = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value
        .and_then(|text: String| text.parse().ok())
        .unwrap_or(default))
}

pub fn get_string_setting(
    connection: &Connection,
    key: &str,
    default: &str,
) -> Result<String, ClipboardError> {
    let value = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.unwrap_or_else(|| default.to_string()))
}

pub fn set_setting(
    connection: &Connection,
    key: &str,
    value: &str,
    now: &str,
) -> Result<(), ClipboardError> {
    connection.execute(
        "INSERT INTO app_settings (key, value, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now],
    )?;
    Ok(())
}

pub fn cleanup_items(
    connection: &Connection,
    cutoff_date: &str,
    max_record_count: i64,
    now: &str,
) -> Result<usize, ClipboardError> {
    let by_date = cleanup_by_date(connection, cutoff_date, now)?;
    let by_count = cleanup_by_count(connection, max_record_count, now)?;
    Ok(by_date + by_count)
}

fn update_existing_item(connection: &Connection, id: i64, now: &str) -> Result<(), ClipboardError> {
    connection.execute(
        "UPDATE clipboard_items
         SET last_copied_at = ?1, copy_count = copy_count + 1
         WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    Ok(())
}

fn insert_text_item(
    connection: &Connection,
    content: &str,
    content_hash: &str,
    now: &str,
) -> Result<(), ClipboardError> {
    connection.execute(
        "INSERT INTO clipboard_items
         (content_type, content, preview, content_hash, created_at, last_copied_at, copy_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        params![
            CONTENT_TYPE_TEXT,
            content,
            preview(content),
            content_hash,
            now,
            now
        ],
    )?;
    Ok(())
}

fn cleanup_by_date(
    connection: &Connection,
    cutoff_date: &str,
    now: &str,
) -> Result<usize, ClipboardError> {
    connection
        .execute(
            "UPDATE clipboard_items
             SET deleted_at = ?1
             WHERE deleted_at IS NULL AND substr(created_at, 1, 10) < ?2",
            params![now, cutoff_date],
        )
        .map_err(ClipboardError::from)
}

fn cleanup_by_count(
    connection: &Connection,
    max_record_count: i64,
    now: &str,
) -> Result<usize, ClipboardError> {
    connection
        .execute(
            "UPDATE clipboard_items
             SET deleted_at = ?1
             WHERE id IN (
                SELECT id FROM clipboard_items
                WHERE deleted_at IS NULL
                ORDER BY last_copied_at DESC, id DESC
                LIMIT -1 OFFSET ?2
             )",
            params![now, max_record_count],
        )
        .map_err(ClipboardError::from)
}

fn find_active_id_by_hash(
    connection: &Connection,
    content_hash: &str,
) -> Result<Option<i64>, ClipboardError> {
    connection
        .query_row(
            "SELECT id FROM clipboard_items WHERE content_hash = ?1 AND deleted_at IS NULL",
            [content_hash],
            |row| row.get(0),
        )
        .optional()
        .map_err(ClipboardError::from)
}

fn get_item_by_id_with_connection(
    connection: &Connection,
    id: i64,
) -> Result<ClipboardItem, ClipboardError> {
    let item = connection
        .query_row(
            "SELECT id, content_type, content, preview, content_hash, created_at, last_copied_at, copy_count
             FROM clipboard_items
             WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_item,
        )
        .optional()?;
    item.ok_or(ClipboardError::NotFound(id))
}

fn map_item(row: &Row<'_>) -> rusqlite::Result<ClipboardItem> {
    Ok(ClipboardItem {
        id: row.get(0)?,
        content_type: row.get(1)?,
        content: row.get(2)?,
        preview: row.get(3)?,
        content_hash: row.get(4)?,
        created_at: row.get(5)?,
        last_copied_at: row.get(6)?,
        copy_count: row.get(7)?,
    })
}
```

注意要点：(a) 删除了顶部 `use std::path::Path;`；(b) `init_database → init_schema`；(c) 12 个公开函数首参 `path: &Path → connection: &Connection` 并删除函数体里的 `let connection = Connection::open(path)?;`；(d) `cleanup_items` 内部调 helper 时去掉 `&` 前缀（因 connection 已是引用）。

- [ ] **Step 2: 重写 `maintenance.rs`**

定位 `maintenance.rs` 全文（1-21 行）。整体替换：

```rust
use rusqlite::Connection;

use super::error::ClipboardError;

pub fn purge_deleted_items(connection: &Connection) -> Result<usize, ClipboardError> {
    connection
        .execute(
            "DELETE FROM clipboard_items WHERE deleted_at IS NOT NULL",
            [],
        )
        .map_err(ClipboardError::from)
}

pub fn vacuum_database(connection: &Connection) -> Result<(), ClipboardError> {
    connection.execute_batch("VACUUM")?;
    Ok(())
}
```

- [ ] **Step 3: 修改 `settings.rs` 公开函数签名**

定位 `settings.rs:20-54`（`get_stored_settings`）。整体替换该函数为：

```rust
pub fn get_stored_settings(connection: &Connection) -> Result<StoredSettings, ClipboardError> {
    Ok(StoredSettings {
        monitor_enabled: repository::get_i64_setting(
            connection,
            "monitor_enabled",
            bool_to_setting(DEFAULT_MONITOR_ENABLED),
        )? == 1,
        retention_days: repository::get_i64_setting(
            connection,
            "retention_days",
            DEFAULT_RETENTION_DAYS,
        )?,
        max_record_count: repository::get_i64_setting(
            connection,
            "max_record_count",
            DEFAULT_MAX_RECORD_COUNT,
        )?,
        max_text_length: repository::get_i64_setting(
            connection,
            "max_text_length",
            DEFAULT_MAX_TEXT_LENGTH,
        )?,
        ignore_password_like_text: repository::get_i64_setting(
            connection,
            "ignore_password_like_text",
            bool_to_setting(DEFAULT_IGNORE_PASSWORD_LIKE_TEXT),
        )? == 1,
        custom_secret_patterns: repository::get_string_setting(
            connection,
            "custom_secret_patterns",
            DEFAULT_CUSTOM_SECRET_PATTERNS,
        )?,
        storage_dir: repository::get_string_setting(connection, "storage_dir", DEFAULT_STORAGE_DIR)?,
    })
}
```

定位 `settings.rs:56-115`（`update_stored_settings`）。整体替换为：

```rust
pub fn update_stored_settings(
    connection: &Connection,
    monitor_enabled: bool,
    retention_days: i64,
    max_record_count: i64,
    max_text_length: i64,
    ignore_password_like_text: bool,
    custom_secret_patterns: &str,
    storage_dir: &str,
) -> Result<StoredSettings, ClipboardError> {
    validate_custom_secret_patterns(custom_secret_patterns)?;
    let settings = StoredSettings {
        monitor_enabled,
        retention_days: sanitize_setting_value(retention_days),
        max_record_count: sanitize_setting_value(max_record_count),
        max_text_length: sanitize_setting_value(max_text_length),
        ignore_password_like_text,
        custom_secret_patterns: custom_secret_patterns.trim().to_string(),
        storage_dir: storage_dir.trim().to_string(),
    };
    let now = Local::now().to_rfc3339();
    repository::set_setting(
        connection,
        "monitor_enabled",
        &bool_to_setting(settings.monitor_enabled).to_string(),
        &now,
    )?;
    repository::set_setting(
        connection,
        "retention_days",
        &settings.retention_days.to_string(),
        &now,
    )?;
    repository::set_setting(
        connection,
        "max_record_count",
        &settings.max_record_count.to_string(),
        &now,
    )?;
    repository::set_setting(
        connection,
        "max_text_length",
        &settings.max_text_length.to_string(),
        &now,
    )?;
    repository::set_setting(
        connection,
        "ignore_password_like_text",
        &bool_to_setting(settings.ignore_password_like_text).to_string(),
        &now,
    )?;
    repository::set_setting(
        connection,
        "custom_secret_patterns",
        &settings.custom_secret_patterns,
        &now,
    )?;
    repository::set_setting(connection, "storage_dir", &settings.storage_dir, &now)?;
    Ok(settings)
}
```

定位 `settings.rs:117-125`（`update_monitor_enabled`）。整体替换为：

```rust
pub fn update_monitor_enabled(
    connection: &Connection,
    enabled: bool,
) -> Result<(), ClipboardError> {
    let now = Local::now().to_rfc3339();
    repository::set_setting(
        connection,
        "monitor_enabled",
        &bool_to_setting(enabled).to_string(),
        &now,
    )
}
```

定位 `settings.rs:143-149`（`apply_retention_policy`）。整体替换为：

```rust
pub fn apply_retention_policy(
    items_connection: &Connection,
    settings_connection: &Connection,
) -> Result<usize, ClipboardError> {
    let settings = get_stored_settings(settings_connection)?;
    let now = Local::now().to_rfc3339();
    let cutoff = Local::now() - Duration::days(settings.retention_days);
    let cutoff_date = cutoff.format("%Y-%m-%d").to_string();
    repository::cleanup_items(
        items_connection,
        &cutoff_date,
        settings.max_record_count,
        &now,
    )
}
```

定位 `settings.rs:1-9` 顶部 import 区。替换为：

```rust
use std::path::PathBuf;
use std::sync::OnceLock;

use chrono::{Duration, Local};
use regex::Regex;
use rusqlite::Connection;

use super::error::ClipboardError;
use super::models::{ClipboardSkipReason, StoredSettings};
use super::repository;
```

（删除了 `use std::path::{Path, PathBuf}` 中的 `Path`，因 `validate_storage_dir` 仍用到 `PathBuf`；新增 `use rusqlite::Connection;`）

- [ ] **Step 4: 修改 `service.rs` 调用点**

定位 `service.rs:28-40`（`ClipboardService::new`）。整体替换为（保留行为，仅改用 `open_connection`，连接用完即丢——下个 Task 才持久化）：

```rust
    pub fn new(default_database_path: PathBuf) -> Result<Self, ClipboardError> {
        let settings_conn = service_runtime::open_connection(&default_database_path)?;
        repository::init_schema(&settings_conn)?;
        let stored = settings::get_stored_settings(&settings_conn)?;
        let database_path = resolve_database_path(&default_database_path, &stored.storage_dir);
        let items_conn = service_runtime::open_connection(&database_path)?;
        repository::init_schema(&items_conn)?;
        Ok(Self {
            default_database_path,
            database_path: Mutex::new(database_path),
            last_seen_hash: Mutex::new(None),
            last_app_write: Mutex::new(None),
            monitor_enabled: Mutex::new(stored.monitor_enabled),
        })
    }
```

定位 `service.rs:14-17` import 区。替换为：

```rust
use super::repository;
use super::service_runtime::{
    self, now_iso, read_clipboard_text, resolve_database_path, skip_outcome, AppWriteGuard,
};
use super::settings;
```

（新增 `self` 别名让 `service_runtime::open_connection` 可访问）

定位 `service.rs:42-67`（`capture_current_clipboard`）。整体替换为：

```rust
    pub fn capture_current_clipboard(&self) -> Result<CaptureOutcome, ClipboardError> {
        if !self.is_monitor_enabled()? {
            return Ok(skip_outcome(ClipboardSkipReason::MonitorDisabled, 0, 0));
        }
        let database_path = self.active_database_path()?;
        let content = read_clipboard_text()?;
        if content.is_empty() {
            return Ok(skip_outcome(ClipboardSkipReason::Empty, 0, 0));
        }
        let settings_conn = service_runtime::open_connection(&self.default_database_path)?;
        let stored_settings = settings::get_stored_settings(&settings_conn)?;
        if let Some(reason) = settings::content_skip_reason(&content, &stored_settings) {
            return Ok(skip_outcome(
                reason,
                content.chars().count() as i64,
                stored_settings.max_text_length,
            ));
        }
        let hash = content_hash(&content);
        if let Some(reason) = self.skip_hash_reason(&hash)? {
            return Ok(skip_outcome(reason, content.len() as i64, 0));
        }
        let items_conn = service_runtime::open_connection(&database_path)?;
        let item = repository::upsert_text_item(&items_conn, &content, &hash, &now_iso())?;
        self.remember_seen_hash(hash)?;
        self.apply_retention_policy(&database_path)?;
        Ok(CaptureOutcome::Item(item))
    }
```

定位 `service.rs:69-83`（`list_date_groups` / `list_items_by_date` / `search_items` / `get_item`）。整体替换为：

```rust
    pub fn list_date_groups(&self) -> Result<Vec<ClipboardDateGroup>, ClipboardError> {
        let conn = service_runtime::open_connection(&self.active_database_path()?)?;
        repository::list_date_groups(&conn)
    }

    pub fn list_items_by_date(&self, date: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        let conn = service_runtime::open_connection(&self.active_database_path()?)?;
        repository::list_items_by_date(&conn, date)
    }

    pub fn search_items(&self, keyword: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        let conn = service_runtime::open_connection(&self.active_database_path()?)?;
        repository::search_items(&conn, keyword)
    }

    pub fn get_item(&self, id: i64) -> Result<ClipboardItem, ClipboardError> {
        let conn = service_runtime::open_connection(&self.active_database_path()?)?;
        repository::get_item_by_id(&conn, id)
    }
```

定位 `service.rs:94-100`（`delete_item` 与 `clear_items_by_date`）。整体替换为：

```rust
    pub fn delete_item(&self, id: i64) -> Result<(), ClipboardError> {
        let conn = service_runtime::open_connection(&self.active_database_path()?)?;
        repository::soft_delete_item(&conn, id, &now_iso())
    }

    pub fn clear_items_by_date(&self, date: &str) -> Result<usize, ClipboardError> {
        let conn = service_runtime::open_connection(&self.active_database_path()?)?;
        repository::soft_delete_items_by_date(&conn, date, &now_iso())
    }
```

定位 `service.rs:102-109`（`purge_deleted_items`）。整体替换为：

```rust
    pub fn purge_deleted_items(&self, vacuum: bool) -> Result<usize, ClipboardError> {
        let database_path = self.active_database_path()?;
        let conn = service_runtime::open_connection(&database_path)?;
        let removed = maintenance::purge_deleted_items(&conn)?;
        if vacuum {
            maintenance::vacuum_database(&conn)?;
        }
        Ok(removed)
    }
```

定位 `service.rs:111-122`（`set_monitor_enabled`）。整体替换为：

```rust
    pub fn set_monitor_enabled(
        &self,
        enabled: bool,
    ) -> Result<ClipboardMonitorStatus, ClipboardError> {
        if enabled {
            self.seed_current_clipboard_hash()?;
        }
        let conn = service_runtime::open_connection(&self.default_database_path)?;
        settings::update_monitor_enabled(&conn, enabled)?;
        let mut guard = self.lock_monitor_enabled()?;
        *guard = enabled;
        Ok(ClipboardMonitorStatus { enabled })
    }
```

定位 `service.rs:130-145`（`desktop_settings`）。整体替换为：

```rust
    pub fn desktop_settings(
        &self,
        autostart_enabled: bool,
    ) -> Result<DesktopSettings, ClipboardError> {
        let conn = service_runtime::open_connection(&self.default_database_path)?;
        let stored = settings::get_stored_settings(&conn)?;
        Ok(DesktopSettings {
            autostart_enabled,
            monitor_enabled: self.is_monitor_enabled()?,
            retention_days: stored.retention_days,
            max_record_count: stored.max_record_count,
            max_text_length: stored.max_text_length,
            ignore_password_like_text: stored.ignore_password_like_text,
            custom_secret_patterns: stored.custom_secret_patterns,
            storage_dir: stored.storage_dir,
        })
    }
```

定位 `service.rs:147-179`（`update_desktop_settings`）。整体替换为：

```rust
    pub fn update_desktop_settings(
        &self,
        update: DesktopSettingsUpdate,
        autostart_enabled: bool,
    ) -> Result<DesktopSettings, ClipboardError> {
        let storage_dir = update.storage_dir.trim().to_string();
        settings::validate_storage_dir(&storage_dir)?;
        let database_path = resolve_database_path(&self.default_database_path, &storage_dir);
        let items_conn = service_runtime::open_connection(&database_path)?;
        repository::init_schema(&items_conn)?;
        let settings_conn = service_runtime::open_connection(&self.default_database_path)?;
        let stored = settings::update_stored_settings(
            &settings_conn,
            update.monitor_enabled,
            update.retention_days,
            update.max_record_count,
            update.max_text_length,
            update.ignore_password_like_text,
            &update.custom_secret_patterns,
            &storage_dir,
        )?;
        self.set_active_database_path(database_path.clone())?;
        self.set_monitor_enabled_state(stored.monitor_enabled)?;
        self.apply_retention_policy(&database_path)?;
        Ok(DesktopSettings {
            autostart_enabled,
            monitor_enabled: stored.monitor_enabled,
            retention_days: stored.retention_days,
            max_record_count: stored.max_record_count,
            max_text_length: stored.max_text_length,
            ignore_password_like_text: stored.ignore_password_like_text,
            custom_secret_patterns: stored.custom_secret_patterns,
            storage_dir: stored.storage_dir,
        })
    }
```

定位 `service.rs:181-184`（`apply_retention_policy` 私有方法）。整体替换为：

```rust
    fn apply_retention_policy(&self, database_path: &Path) -> Result<(), ClipboardError> {
        let items_conn = service_runtime::open_connection(database_path)?;
        let settings_conn = service_runtime::open_connection(&self.default_database_path)?;
        settings::apply_retention_policy(&items_conn, &settings_conn)?;
        Ok(())
    }
```

- [ ] **Step 5: 重写 `repository_tests.rs`**

定位 `repository_tests.rs` 全文（1-174 行）。整体替换为：

```rust
use std::time::{SystemTime, UNIX_EPOCH};

use super::repository::{
    cleanup_items, get_i64_setting, get_item_by_id, get_string_setting, init_schema,
    list_date_groups, list_items_by_date, search_items, set_setting, soft_delete_item,
    soft_delete_items_by_date, upsert_text_item,
};
use super::service_runtime::open_connection;

fn temp_database_path(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("clipboard-{name}-{unique}.sqlite"))
}

#[test]
fn inserts_and_lists_items_by_date() {
    let path = temp_database_path("insert-list");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let item = upsert_text_item(&conn, "hello", "hash-1", "2026-05-26T10:00:00+08:00").unwrap();
    let groups = list_date_groups(&conn).unwrap();
    let items = list_items_by_date(&conn, "2026-05-26").unwrap();

    assert_eq!("hello", item.content);
    assert_eq!("2026-05-26", groups[0].date);
    assert_eq!(1, groups[0].count);
    assert_eq!(item.id, items[0].id);
}

#[test]
fn deduplicates_active_hashes() {
    let path = temp_database_path("dedupe");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let first = upsert_text_item(&conn, "hello", "hash-1", "2026-05-26T10:00:00+08:00").unwrap();
    let second = upsert_text_item(&conn, "hello", "hash-1", "2026-05-26T10:01:00+08:00").unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(2, second.copy_count);
    assert_eq!("2026-05-26T10:01:00+08:00", second.last_copied_at);
}

#[test]
fn soft_deleted_items_are_hidden() {
    let path = temp_database_path("delete");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let item = upsert_text_item(&conn, "secret", "hash-2", "2026-05-26T10:00:00+08:00").unwrap();
    soft_delete_item(&conn, item.id, "2026-05-26T10:02:00+08:00").unwrap();

    assert!(get_item_by_id(&conn, item.id).is_err());
    assert!(list_items_by_date(&conn, "2026-05-26").unwrap().is_empty());
}

#[test]
fn searches_active_content_across_dates() {
    let path = temp_database_path("search");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    upsert_text_item(&conn, "alpha code", "hash-1", "2026-05-25T10:00:00+08:00").unwrap();
    upsert_text_item(&conn, "beta note", "hash-2", "2026-05-26T10:00:00+08:00").unwrap();
    upsert_text_item(&conn, "alpha docs", "hash-3", "2026-05-27T10:00:00+08:00").unwrap();

    let results = search_items(&conn, "alpha").unwrap();

    assert_eq!(2, results.len());
    assert_eq!("alpha docs", results[0].content);
    assert_eq!("alpha code", results[1].content);
}

#[test]
fn search_ignores_deleted_items_and_blank_keywords() {
    let path = temp_database_path("search-hidden");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let item = upsert_text_item(
        &conn,
        "temporary token",
        "hash-1",
        "2026-05-26T10:00:00+08:00",
    )
    .unwrap();
    soft_delete_item(&conn, item.id, "2026-05-26T10:02:00+08:00").unwrap();

    assert!(search_items(&conn, "temporary").unwrap().is_empty());
    assert!(search_items(&conn, "   ").unwrap().is_empty());
}

#[test]
fn soft_deletes_all_items_by_date() {
    let path = temp_database_path("clear-date");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    upsert_text_item(&conn, "today one", "hash-1", "2026-05-26T10:00:00+08:00").unwrap();
    upsert_text_item(&conn, "today two", "hash-2", "2026-05-26T11:00:00+08:00").unwrap();
    upsert_text_item(&conn, "other day", "hash-3", "2026-05-27T10:00:00+08:00").unwrap();

    let changed =
        soft_delete_items_by_date(&conn, "2026-05-26", "2026-05-26T12:00:00+08:00").unwrap();

    assert_eq!(2, changed);
    assert!(list_items_by_date(&conn, "2026-05-26").unwrap().is_empty());
    assert_eq!(1, list_items_by_date(&conn, "2026-05-27").unwrap().len());
}

#[test]
fn settings_default_and_update_roundtrip() {
    let path = temp_database_path("settings");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    assert_eq!(30, get_i64_setting(&conn, "retention_days", 30).unwrap());
    assert_eq!("", get_string_setting(&conn, "storage_dir", "").unwrap());
    set_setting(&conn, "retention_days", "7", "2026-05-26T10:00:00+08:00").unwrap();
    set_setting(&conn, "storage_dir", "D:/clip", "2026-05-26T10:00:00+08:00").unwrap();

    assert_eq!(7, get_i64_setting(&conn, "retention_days", 30).unwrap());
    assert_eq!(
        "D:/clip",
        get_string_setting(&conn, "storage_dir", "").unwrap()
    );
}

#[test]
fn cleanup_items_removes_old_dates() {
    let path = temp_database_path("cleanup-date");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    upsert_text_item(&conn, "old", "hash-1", "2026-05-01T10:00:00+08:00").unwrap();
    upsert_text_item(&conn, "new", "hash-2", "2026-05-26T10:00:00+08:00").unwrap();

    let changed = cleanup_items(&conn, "2026-05-10", 100, "2026-05-27T10:00:00+08:00").unwrap();

    assert_eq!(1, changed);
    assert!(search_items(&conn, "old").unwrap().is_empty());
    assert_eq!(1, search_items(&conn, "new").unwrap().len());
}

#[test]
fn cleanup_items_respects_max_record_count() {
    let path = temp_database_path("cleanup-count");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    upsert_text_item(&conn, "first", "hash-1", "2026-05-26T10:00:00+08:00").unwrap();
    upsert_text_item(&conn, "second", "hash-2", "2026-05-26T11:00:00+08:00").unwrap();
    upsert_text_item(&conn, "third", "hash-3", "2026-05-26T12:00:00+08:00").unwrap();

    let changed = cleanup_items(&conn, "2026-05-01", 2, "2026-05-27T10:00:00+08:00").unwrap();
    let items = list_items_by_date(&conn, "2026-05-26").unwrap();

    assert_eq!(1, changed);
    assert_eq!(2, items.len());
    assert_eq!("third", items[0].content);
    assert_eq!("second", items[1].content);
}

#[test]
fn purge_deleted_items_removes_only_soft_deleted_rows() {
    let path = temp_database_path("purge-deleted");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let active = upsert_text_item(&conn, "active", "hash-1", "2026-05-26T10:00:00+08:00").unwrap();
    let deleted =
        upsert_text_item(&conn, "deleted", "hash-2", "2026-05-26T11:00:00+08:00").unwrap();
    soft_delete_item(&conn, deleted.id, "2026-05-26T12:00:00+08:00").unwrap();

    let removed = super::maintenance::purge_deleted_items(&conn).unwrap();
    let items = list_items_by_date(&conn, "2026-05-26").unwrap();

    assert_eq!(1, removed);
    assert_eq!(1, items.len());
    assert_eq!(active.id, items[0].id);
}
```

- [ ] **Step 6: 修改 `service_tests.rs` 直调点**

定位 `service_tests.rs:79-100`（`update_settings_switches_to_custom_storage_database` 函数体内的 3 处 repository 直调）。整体替换 `repository::upsert_text_item(&default_path, ...)` 与 `repository::upsert_text_item(&custom_path, ...)`：

将原代码：
```rust
    repository::upsert_text_item(
        &default_path,
        "default",
        "hash-1",
        "2026-05-26T10:00:00+08:00",
    )
    .unwrap();
```

替换为：
```rust
    {
        let conn = super::service_runtime::open_connection(&default_path).unwrap();
        repository::init_schema(&conn).unwrap();
        repository::upsert_text_item(&conn, "default", "hash-1", "2026-05-26T10:00:00+08:00")
            .unwrap();
    }
```

将原代码：
```rust
    repository::upsert_text_item(
        &custom_path,
        "custom",
        "hash-2",
        "2026-05-26T11:00:00+08:00",
    )
    .unwrap();
```

替换为：
```rust
    {
        let conn = super::service_runtime::open_connection(&custom_path).unwrap();
        repository::init_schema(&conn).unwrap();
        repository::upsert_text_item(&conn, "custom", "hash-2", "2026-05-26T11:00:00+08:00")
            .unwrap();
    }
```

定位 `service_tests.rs:104-138`（`startup_uses_storage_dir_from_default_database`）。把 `repository::init_database(&default_path).unwrap();` 与 `settings::update_stored_settings(&default_path, ...)` 调用块替换为：

```rust
    {
        let conn = super::service_runtime::open_connection(&default_path).unwrap();
        repository::init_schema(&conn).unwrap();
        settings::update_stored_settings(
            &conn,
            false,
            15,
            50,
            1024,
            true,
            "",
            &custom_dir_text,
        )
        .unwrap();
    }
```

把 `repository::upsert_text_item(&custom_path, ...)` 替换为：

```rust
    {
        let conn = super::service_runtime::open_connection(&custom_path).unwrap();
        repository::init_schema(&conn).unwrap();
        repository::upsert_text_item(&conn, "custom", "hash-1", "2026-05-26T11:00:00+08:00")
            .unwrap();
    }
```

- [ ] **Step 7: 编译通过**

Run: `cd src-tauri; cargo check`

Expected: 编译通过。如果有 `unused import` 警告，按提示清理。

- [ ] **Step 8: 全部 clipboard 测试通过**

Run: `cd src-tauri; cargo test clipboard`

Expected: 所有已有测试全部通过。

- [ ] **Step 9: 提交**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/maintenance.rs src-tauri/src/clipboard/settings.rs src-tauri/src/clipboard/service.rs src-tauri/src/clipboard/repository_tests.rs src-tauri/src/clipboard/service_tests.rs
git commit -m "$(cat <<'EOF'
refactor(clipboard): repository/settings/maintenance API 迁移到 &Connection

repository.rs 12 个公开函数 + maintenance.rs 2 个 + settings.rs 4 个全部从 &Path 改为 &Connection；init_database 改名 init_schema。所有调用方（service.rs、repository_tests.rs、service_tests.rs）同步迁移。本提交为机械接口重构，行为零变化——service.rs 内部临时改用 open_connection 临时连接，下个提交才换为持久化连接。
EOF
)"
```

---

## Task 3: ClipboardService 持有双连接

**Files:**
- Modify: `src-tauri/src/clipboard/service.rs`

把 service.rs 内部"每次 `open_connection` 临时连接"换成"`Mutex<Connection>` 持久化"。新增 `settings_conn` 与 `items_conn` 两个字段；构造时各开一次，运行期持锁取。storage_dir 切换时重建 `items_conn`。此 Task 仍不改保留策略触发逻辑（下个 Task 才动）。

- [ ] **Step 1: 修改 `ClipboardService` 字段定义**

定位 `service.rs:19-25`（结构体定义）。整体替换为：

```rust
pub struct ClipboardService {
    default_database_path: PathBuf,
    database_path: Mutex<PathBuf>,
    settings_conn: Mutex<Connection>,
    items_conn: Mutex<Connection>,
    last_seen_hash: Mutex<Option<String>>,
    last_app_write: Mutex<Option<AppWriteGuard>>,
    monitor_enabled: Mutex<bool>,
}
```

定位 `service.rs:1-17` import 区。在 `use std::sync::Mutex;` 之后追加 `use rusqlite::Connection;`，最终为：

```rust
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use arboard::Clipboard;
use rusqlite::Connection;

use super::error::ClipboardError;
use super::hash::content_hash;
use super::maintenance;
use super::models::{
    CaptureOutcome, ClipboardDateGroup, ClipboardItem, ClipboardMonitorStatus, ClipboardSkipReason,
    DesktopSettings, DesktopSettingsUpdate,
};
use super::repository;
use super::service_runtime::{
    self, now_iso, read_clipboard_text, resolve_database_path, skip_outcome, AppWriteGuard,
};
use super::settings;
```

- [ ] **Step 2: 修改 `new` 构造函数**

定位 `service.rs::new`（Task 2 Step 4 已改）。整体替换为：

```rust
    pub fn new(default_database_path: PathBuf) -> Result<Self, ClipboardError> {
        let settings_conn = service_runtime::open_connection(&default_database_path)?;
        repository::init_schema(&settings_conn)?;
        let stored = settings::get_stored_settings(&settings_conn)?;
        let database_path = resolve_database_path(&default_database_path, &stored.storage_dir);
        let items_conn = service_runtime::open_connection(&database_path)?;
        repository::init_schema(&items_conn)?;
        Ok(Self {
            default_database_path,
            database_path: Mutex::new(database_path),
            settings_conn: Mutex::new(settings_conn),
            items_conn: Mutex::new(items_conn),
            last_seen_hash: Mutex::new(None),
            last_app_write: Mutex::new(None),
            monitor_enabled: Mutex::new(stored.monitor_enabled),
        })
    }
```

- [ ] **Step 3: 添加连接锁助手**

在 `service.rs` 文件末尾的 `impl ClipboardService { ... }` 块内，靠近其他 `lock_xxx` helper 处（`service.rs:261-265` 附近，`lock_monitor_enabled` 后）追加：

```rust
    fn lock_settings_conn(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Connection>, ClipboardError> {
        self.settings_conn
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))
    }

    fn lock_items_conn(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Connection>, ClipboardError> {
        self.items_conn
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))
    }
```

- [ ] **Step 4: 改造 `capture_current_clipboard` 使用持锁连接**

定位 `capture_current_clipboard`（Task 2 Step 4 已改）。整体替换为：

```rust
    pub fn capture_current_clipboard(&self) -> Result<CaptureOutcome, ClipboardError> {
        if !self.is_monitor_enabled()? {
            return Ok(skip_outcome(ClipboardSkipReason::MonitorDisabled, 0, 0));
        }
        let content = read_clipboard_text()?;
        if content.is_empty() {
            return Ok(skip_outcome(ClipboardSkipReason::Empty, 0, 0));
        }
        let stored_settings = {
            let conn = self.lock_settings_conn()?;
            settings::get_stored_settings(&conn)?
        };
        if let Some(reason) = settings::content_skip_reason(&content, &stored_settings) {
            return Ok(skip_outcome(
                reason,
                content.chars().count() as i64,
                stored_settings.max_text_length,
            ));
        }
        let hash = content_hash(&content);
        if let Some(reason) = self.skip_hash_reason(&hash)? {
            return Ok(skip_outcome(reason, content.len() as i64, 0));
        }
        let item = {
            let conn = self.lock_items_conn()?;
            repository::upsert_text_item(&conn, &content, &hash, &now_iso())?
        };
        self.remember_seen_hash(hash)?;
        self.run_retention()?;
        Ok(CaptureOutcome::Item(item))
    }
```

注意：`run_retention()` 在下一 step 创建；本 step 暂时不保留 `apply_retention_policy` 私有方法的旧实现（下 step 替换）。

- [ ] **Step 5: 改造 `apply_retention_policy` 为 `run_retention`**

定位 `service.rs::apply_retention_policy`（Task 2 Step 4 已改的版本）。整体替换为：

```rust
    fn run_retention(&self) -> Result<(), ClipboardError> {
        let settings_conn = self.lock_settings_conn()?;
        let items_conn = self.lock_items_conn()?;
        settings::apply_retention_policy(&items_conn, &settings_conn)?;
        Ok(())
    }
```

注意：方法名从 `apply_retention_policy` 改为 `run_retention`，参数从 `database_path: &Path` 删除（直接用 self.lock_*）。所有调用方需要相应改 `self.apply_retention_policy(&path)?` 为 `self.run_retention()?`。`capture_current_clipboard` 在 Step 4 已改；`update_desktop_settings` 在 Step 9 会改。

- [ ] **Step 6: 改造读路径方法持锁**

定位 `service.rs` 中 `list_date_groups` / `list_items_by_date` / `search_items` / `get_item`（Task 2 Step 4 已改）。整体替换为：

```rust
    pub fn list_date_groups(&self) -> Result<Vec<ClipboardDateGroup>, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::list_date_groups(&conn)
    }

    pub fn list_items_by_date(&self, date: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::list_items_by_date(&conn, date)
    }

    pub fn search_items(&self, keyword: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::search_items(&conn, keyword)
    }

    pub fn get_item(&self, id: i64) -> Result<ClipboardItem, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::get_item_by_id(&conn, id)
    }
```

- [ ] **Step 7: 改造 `delete_item` / `clear_items_by_date` / `purge_deleted_items` 持锁**

定位上述三个方法（Task 2 Step 4 已改的版本）。整体替换为：

```rust
    pub fn delete_item(&self, id: i64) -> Result<(), ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::soft_delete_item(&conn, id, &now_iso())
    }

    pub fn clear_items_by_date(&self, date: &str) -> Result<usize, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::soft_delete_items_by_date(&conn, date, &now_iso())
    }

    pub fn purge_deleted_items(&self, vacuum: bool) -> Result<usize, ClipboardError> {
        let conn = self.lock_items_conn()?;
        let removed = maintenance::purge_deleted_items(&conn)?;
        if vacuum {
            maintenance::vacuum_database(&conn)?;
        }
        Ok(removed)
    }
```

- [ ] **Step 8: 改造 `set_monitor_enabled` 与 `desktop_settings` 持锁**

定位 `set_monitor_enabled`（Task 2 Step 4 已改）。整体替换为：

```rust
    pub fn set_monitor_enabled(
        &self,
        enabled: bool,
    ) -> Result<ClipboardMonitorStatus, ClipboardError> {
        if enabled {
            self.seed_current_clipboard_hash()?;
        }
        {
            let conn = self.lock_settings_conn()?;
            settings::update_monitor_enabled(&conn, enabled)?;
        }
        let mut guard = self.lock_monitor_enabled()?;
        *guard = enabled;
        Ok(ClipboardMonitorStatus { enabled })
    }
```

定位 `desktop_settings`（Task 2 Step 4 已改）。整体替换为：

```rust
    pub fn desktop_settings(
        &self,
        autostart_enabled: bool,
    ) -> Result<DesktopSettings, ClipboardError> {
        let stored = {
            let conn = self.lock_settings_conn()?;
            settings::get_stored_settings(&conn)?
        };
        Ok(DesktopSettings {
            autostart_enabled,
            monitor_enabled: self.is_monitor_enabled()?,
            retention_days: stored.retention_days,
            max_record_count: stored.max_record_count,
            max_text_length: stored.max_text_length,
            ignore_password_like_text: stored.ignore_password_like_text,
            custom_secret_patterns: stored.custom_secret_patterns,
            storage_dir: stored.storage_dir,
        })
    }
```

- [ ] **Step 9: 改造 `update_desktop_settings` 持锁 + 切换 items_conn**

定位 `update_desktop_settings`（Task 2 Step 4 已改的版本）。整体替换为：

```rust
    pub fn update_desktop_settings(
        &self,
        update: DesktopSettingsUpdate,
        autostart_enabled: bool,
    ) -> Result<DesktopSettings, ClipboardError> {
        let storage_dir = update.storage_dir.trim().to_string();
        settings::validate_storage_dir(&storage_dir)?;
        let new_database_path = resolve_database_path(&self.default_database_path, &storage_dir);

        let stored = {
            let conn = self.lock_settings_conn()?;
            settings::update_stored_settings(
                &conn,
                update.monitor_enabled,
                update.retention_days,
                update.max_record_count,
                update.max_text_length,
                update.ignore_password_like_text,
                &update.custom_secret_patterns,
                &storage_dir,
            )?
        };

        {
            let mut path_guard = self.lock_database_path()?;
            if new_database_path != *path_guard {
                let new_conn = service_runtime::open_connection(&new_database_path)?;
                repository::init_schema(&new_conn)?;
                let mut items_guard = self.lock_items_conn()?;
                *items_guard = new_conn;
                *path_guard = new_database_path;
            }
        }

        self.set_monitor_enabled_state(stored.monitor_enabled)?;
        self.run_retention()?;

        Ok(DesktopSettings {
            autostart_enabled,
            monitor_enabled: stored.monitor_enabled,
            retention_days: stored.retention_days,
            max_record_count: stored.max_record_count,
            max_text_length: stored.max_text_length,
            ignore_password_like_text: stored.ignore_password_like_text,
            custom_secret_patterns: stored.custom_secret_patterns,
            storage_dir: stored.storage_dir,
        })
    }
```

注意：去掉了对 `set_active_database_path` 与旧 `apply_retention_policy` 的调用，直接在锁内联完成路径与连接替换，调用 `run_retention()` 处理保留策略。

- [ ] **Step 10: 清理未使用的 `set_active_database_path` 与 `Path` import**

`set_active_database_path` 现已无调用方。定位 `service.rs:235-239`：

```rust
    fn set_active_database_path(&self, database_path: PathBuf) -> Result<(), ClipboardError> {
        let mut guard = self.lock_database_path()?;
        *guard = database_path;
        Ok(())
    }
```

整体删除（含上下空行）。

`active_database_path` 仍被外部代码（无）调用。检查是否还有引用：

Run: `cd src-tauri; cargo check`

Expected: 编译通过。若 `active_database_path` 出现 `unused method` 警告，定位 `service.rs:231-233`：

```rust
    fn active_database_path(&self) -> Result<PathBuf, ClipboardError> {
        Ok(self.lock_database_path()?.clone())
    }
```

整体删除。

若顶部 `use std::path::{Path, PathBuf};` 中 `Path` 不再被使用（因为旧 `apply_retention_policy(&self, database_path: &Path)` 已替换为 `run_retention(&self)`），简化为：

```rust
use std::path::PathBuf;
```

- [ ] **Step 11: 全部 clipboard 测试通过**

Run: `cd src-tauri; cargo test clipboard`

Expected: 所有已有测试全部通过。本 Task 没有引入新行为，所以已有测试覆盖即可。

- [ ] **Step 12: 提交**

```bash
git add src-tauri/src/clipboard/service.rs
git commit -m "$(cat <<'EOF'
refactor(clipboard): ClipboardService 持有 settings_conn 与 items_conn

新增两个 Mutex<Connection> 字段并在 new 中构造，所有方法改为 lock_settings_conn / lock_items_conn 获取连接，移除每次 open_connection 临时连接的开销。update_desktop_settings 在 storage_dir 变化时原地重建 items_conn 并替换 database_path。retention 触发逻辑保持调用时机不变（每次 capture 与 settings 更新），下个提交才引入计数阈值。
EOF
)"
```

---

## Task 4: 引入计数阈值触发保留策略

**Files:**
- Modify: `src-tauri/src/clipboard/service.rs`
- Modify: `src-tauri/src/clipboard/service_tests.rs`

`capture_current_clipboard` 改为累计计数到阈值才触发 `run_retention`，TDD。`update_desktop_settings` 立即触发的行为保留，但需补充计数重置（在下个 Task）。

- [ ] **Step 1: 添加阈值触发测试（红）**

在 `service_tests.rs` 文件末尾追加：

```rust
#[test]
fn retention_runs_only_after_threshold_captures() {
    use super::service::RETENTION_TRIGGER_THRESHOLD;

    let default_path = temp_database_path("threshold");
    let service = ClipboardService::new(default_path.clone()).unwrap();

    // 设置：保留所有日期 + max_record_count = 5
    service
        .update_desktop_settings(
            DesktopSettingsUpdate {
                autostart_enabled: false,
                monitor_enabled: true,
                retention_days: 30,
                max_record_count: 5,
                max_text_length: 20_000,
                ignore_password_like_text: false,
                custom_secret_patterns: String::new(),
                storage_dir: String::new(),
            },
            false,
        )
        .unwrap();

    // 通过测试钩子直接写入 items 表，绕过剪贴板依赖
    for i in 0..(RETENTION_TRIGGER_THRESHOLD - 1) {
        let conn = super::service_runtime::open_connection(&default_path).unwrap();
        repository::upsert_text_item(
            &conn,
            &format!("item-{i}"),
            &format!("hash-{i}"),
            "2026-05-26T10:00:00+08:00",
        )
        .unwrap();
        // 模拟一次 capture 完成：调内部钩子推进计数
        service.tick_capture_count_for_test().unwrap();
    }

    // 此时已写入 THRESHOLD-1 条；max_record_count=5 但应未触发 retention，
    // 因此条目数等于 THRESHOLD-1（远超 5）
    let groups = service.list_date_groups().unwrap();
    let total: i64 = groups.iter().map(|g| g.count).sum();
    assert_eq!(
        (RETENTION_TRIGGER_THRESHOLD - 1) as i64,
        total,
        "retention 不应在阈值之前触发"
    );

    // 写第 THRESHOLD 条，应触发 retention，条目数回落到 5
    let conn = super::service_runtime::open_connection(&default_path).unwrap();
    repository::upsert_text_item(
        &conn,
        "item-final",
        "hash-final",
        "2026-05-26T10:00:00+08:00",
    )
    .unwrap();
    service.tick_capture_count_for_test().unwrap();

    let groups_after = service.list_date_groups().unwrap();
    let total_after: i64 = groups_after.iter().map(|g| g.count).sum();
    assert_eq!(5, total_after, "retention 触发后应裁剪到 max_record_count");
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd src-tauri; cargo test clipboard::service_tests::retention_runs_only_after_threshold_captures`

Expected: 编译错误，缺少 `RETENTION_TRIGGER_THRESHOLD` 与 `tick_capture_count_for_test`。这是红阶段。

- [ ] **Step 3: 在 `service.rs` 顶部添加常量**

定位 `service.rs:17`（紧接 `use super::settings;` 行之后）。追加空行 + 常量：

```rust

pub const RETENTION_TRIGGER_THRESHOLD: u32 = 50;
```

- [ ] **Step 4: 在 `ClipboardService` 字段中添加计数器**

定位 Task 3 Step 1 改过的结构体定义。在 `monitor_enabled` 字段后追加 `captures_since_cleanup: Mutex<u32>,`，结果：

```rust
pub struct ClipboardService {
    default_database_path: PathBuf,
    database_path: Mutex<PathBuf>,
    settings_conn: Mutex<Connection>,
    items_conn: Mutex<Connection>,
    last_seen_hash: Mutex<Option<String>>,
    last_app_write: Mutex<Option<AppWriteGuard>>,
    monitor_enabled: Mutex<bool>,
    captures_since_cleanup: Mutex<u32>,
}
```

定位 Task 3 Step 2 改过的 `new` 函数体内 `Ok(Self { ... })` 块。在 `monitor_enabled: Mutex::new(stored.monitor_enabled),` 后追加 `captures_since_cleanup: Mutex::new(0),`，结果：

```rust
        Ok(Self {
            default_database_path,
            database_path: Mutex::new(database_path),
            settings_conn: Mutex::new(settings_conn),
            items_conn: Mutex::new(items_conn),
            last_seen_hash: Mutex::new(None),
            last_app_write: Mutex::new(None),
            monitor_enabled: Mutex::new(stored.monitor_enabled),
            captures_since_cleanup: Mutex::new(0),
        })
```

- [ ] **Step 5: 改造 `capture_current_clipboard` 引入计数判定**

定位 Task 3 Step 4 改过的 `capture_current_clipboard`。把末尾 `self.run_retention()?;` 一行替换为：

```rust
        let should_clean = {
            let mut count = self
                .captures_since_cleanup
                .lock()
                .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
            *count += 1;
            if *count >= RETENTION_TRIGGER_THRESHOLD {
                *count = 0;
                true
            } else {
                false
            }
        };
        if should_clean {
            self.run_retention()?;
        }
```

完整方法体此时应为：

```rust
    pub fn capture_current_clipboard(&self) -> Result<CaptureOutcome, ClipboardError> {
        if !self.is_monitor_enabled()? {
            return Ok(skip_outcome(ClipboardSkipReason::MonitorDisabled, 0, 0));
        }
        let content = read_clipboard_text()?;
        if content.is_empty() {
            return Ok(skip_outcome(ClipboardSkipReason::Empty, 0, 0));
        }
        let stored_settings = {
            let conn = self.lock_settings_conn()?;
            settings::get_stored_settings(&conn)?
        };
        if let Some(reason) = settings::content_skip_reason(&content, &stored_settings) {
            return Ok(skip_outcome(
                reason,
                content.chars().count() as i64,
                stored_settings.max_text_length,
            ));
        }
        let hash = content_hash(&content);
        if let Some(reason) = self.skip_hash_reason(&hash)? {
            return Ok(skip_outcome(reason, content.len() as i64, 0));
        }
        let item = {
            let conn = self.lock_items_conn()?;
            repository::upsert_text_item(&conn, &content, &hash, &now_iso())?
        };
        self.remember_seen_hash(hash)?;
        let should_clean = {
            let mut count = self
                .captures_since_cleanup
                .lock()
                .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
            *count += 1;
            if *count >= RETENTION_TRIGGER_THRESHOLD {
                *count = 0;
                true
            } else {
                false
            }
        };
        if should_clean {
            self.run_retention()?;
        }
        Ok(CaptureOutcome::Item(item))
    }
```

- [ ] **Step 6: 添加测试钩子 `tick_capture_count_for_test`**

在 `impl ClipboardService { ... }` 块内（紧接 `run_retention` 之后或 Task 3 Step 3 加的 `lock_items_conn` 之后），追加：

```rust
    #[cfg(test)]
    pub fn tick_capture_count_for_test(&self) -> Result<(), ClipboardError> {
        let should_clean = {
            let mut count = self
                .captures_since_cleanup
                .lock()
                .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
            *count += 1;
            if *count >= RETENTION_TRIGGER_THRESHOLD {
                *count = 0;
                true
            } else {
                false
            }
        };
        if should_clean {
            self.run_retention()?;
        }
        Ok(())
    }
```

- [ ] **Step 7: 运行测试验证绿**

Run: `cd src-tauri; cargo test clipboard::service_tests::retention_runs_only_after_threshold_captures`

Expected: PASS。

- [ ] **Step 8: 跑全量 clipboard 测试确认无回归**

Run: `cd src-tauri; cargo test clipboard`

Expected: 所有用例通过。

- [ ] **Step 9: 提交**

```bash
git add src-tauri/src/clipboard/service.rs src-tauri/src/clipboard/service_tests.rs
git commit -m "$(cat <<'EOF'
feat(clipboard): 保留策略改为每 50 次捕获触发

ClipboardService 新增 captures_since_cleanup 计数器与 RETENTION_TRIGGER_THRESHOLD=50 常量。capture_current_clipboard 累计成功写入次数到阈值才调 run_retention，避免每次写入都做全表排序。新增 retention_runs_only_after_threshold_captures 测试 + tick_capture_count_for_test 钩子验证行为。
EOF
)"
```

---

## Task 5: 设置更新后立即触发并重置计数

**Files:**
- Modify: `src-tauri/src/clipboard/service.rs`
- Modify: `src-tauri/src/clipboard/service_tests.rs`

`update_desktop_settings` 已在 Task 3 调用 `run_retention()`。本 Task 在此基础上重置 `captures_since_cleanup`，并通过 TDD 验证。

- [ ] **Step 1: 添加计数重置测试（红）**

在 `service_tests.rs` 文件末尾追加：

```rust
#[test]
fn retention_counter_resets_after_settings_update() {
    use super::service::RETENTION_TRIGGER_THRESHOLD;

    let default_path = temp_database_path("counter-reset");
    let service = ClipboardService::new(default_path).unwrap();

    // 推进计数到 THRESHOLD - 1，刚好不触发
    for _ in 0..(RETENTION_TRIGGER_THRESHOLD - 1) {
        service.tick_capture_count_for_test().unwrap();
    }

    // 更新设置：应重置计数
    service
        .update_desktop_settings(
            DesktopSettingsUpdate {
                autostart_enabled: false,
                monitor_enabled: true,
                retention_days: 30,
                max_record_count: 1000,
                max_text_length: 20_000,
                ignore_password_like_text: false,
                custom_secret_patterns: String::new(),
                storage_dir: String::new(),
            },
            false,
        )
        .unwrap();

    // 再推进 THRESHOLD - 1 次，应仍不触发；若计数未重置则会触发
    for _ in 0..(RETENTION_TRIGGER_THRESHOLD - 1) {
        service.tick_capture_count_for_test().unwrap();
    }

    let count = service.captures_count_for_test().unwrap();
    assert_eq!(
        (RETENTION_TRIGGER_THRESHOLD - 1) as u32,
        count,
        "更新设置后计数应归零，再推进 N-1 次不应触发"
    );
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd src-tauri; cargo test clipboard::service_tests::retention_counter_resets_after_settings_update`

Expected: 编译错误，缺少 `captures_count_for_test`。

- [ ] **Step 3: 在 `service.rs` 添加计数器读取钩子**

定位 Task 4 Step 6 加的 `tick_capture_count_for_test`。紧接其后追加：

```rust
    #[cfg(test)]
    pub fn captures_count_for_test(&self) -> Result<u32, ClipboardError> {
        let count = self
            .captures_since_cleanup
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
        Ok(*count)
    }
```

- [ ] **Step 4: 在 `update_desktop_settings` 末尾添加计数重置**

定位 Task 3 Step 9 改过的 `update_desktop_settings`。在 `self.run_retention()?;` 之后追加重置代码：

```rust
        self.run_retention()?;
        {
            let mut count = self
                .captures_since_cleanup
                .lock()
                .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
            *count = 0;
        }
```

完整 `update_desktop_settings` 此时应为：

```rust
    pub fn update_desktop_settings(
        &self,
        update: DesktopSettingsUpdate,
        autostart_enabled: bool,
    ) -> Result<DesktopSettings, ClipboardError> {
        let storage_dir = update.storage_dir.trim().to_string();
        settings::validate_storage_dir(&storage_dir)?;
        let new_database_path = resolve_database_path(&self.default_database_path, &storage_dir);

        let stored = {
            let conn = self.lock_settings_conn()?;
            settings::update_stored_settings(
                &conn,
                update.monitor_enabled,
                update.retention_days,
                update.max_record_count,
                update.max_text_length,
                update.ignore_password_like_text,
                &update.custom_secret_patterns,
                &storage_dir,
            )?
        };

        {
            let mut path_guard = self.lock_database_path()?;
            if new_database_path != *path_guard {
                let new_conn = service_runtime::open_connection(&new_database_path)?;
                repository::init_schema(&new_conn)?;
                let mut items_guard = self.lock_items_conn()?;
                *items_guard = new_conn;
                *path_guard = new_database_path;
            }
        }

        self.set_monitor_enabled_state(stored.monitor_enabled)?;
        self.run_retention()?;
        {
            let mut count = self
                .captures_since_cleanup
                .lock()
                .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
            *count = 0;
        }

        Ok(DesktopSettings {
            autostart_enabled,
            monitor_enabled: stored.monitor_enabled,
            retention_days: stored.retention_days,
            max_record_count: stored.max_record_count,
            max_text_length: stored.max_text_length,
            ignore_password_like_text: stored.ignore_password_like_text,
            custom_secret_patterns: stored.custom_secret_patterns,
            storage_dir: stored.storage_dir,
        })
    }
```

- [ ] **Step 5: 运行测试验证绿**

Run: `cd src-tauri; cargo test clipboard::service_tests::retention_counter_resets_after_settings_update`

Expected: PASS。

- [ ] **Step 6: 跑全量 clipboard 测试确认无回归**

Run: `cd src-tauri; cargo test clipboard`

Expected: 所有用例通过。

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/clipboard/service.rs src-tauri/src/clipboard/service_tests.rs
git commit -m "$(cat <<'EOF'
feat(clipboard): 设置更新后重置 retention 计数

update_desktop_settings 在调用 run_retention 之后将 captures_since_cleanup 归零，避免设置变更后短时间内重复触发清理。新增 retention_counter_resets_after_settings_update 测试 + captures_count_for_test 钩子覆盖该路径。
EOF
)"
```

---

## Task 6: 文档同步

**Files:**
- Modify: `docs/clipboard-toolbox-design.md`
- Modify: `docs/2026-05-28-clipboard-toolbox-audit.md`

- [ ] **Step 1: 更新设计文档第 5.2 节后端职责**

打开 `docs/clipboard-toolbox-design.md`，定位第 120-128 行 `### 5.2 后端职责`。在最后一行 `- 管理设置项...` 后追加：

```markdown
- 通过 `ClipboardService` 持有 `settings` 与 `items` 两个长生命周期 SQLite 连接（WAL 模式 + synchronous=NORMAL），避免每次操作重复打开连接；记录写入累计 50 次或设置更新时触发一次保留策略清理。
```

- [ ] **Step 2: 标记审查文档中的 P1 第 6、7 项**

打开 `docs/2026-05-28-clipboard-toolbox-audit.md`。

定位第 64 行 `### 6. 数据库连接每次都新开`。整体替换为：

```markdown
### 6. 数据库连接每次都新开 ✅ 2026-05-28 已修复
```

定位第 72 行 `### 7. 保留策略每次写入都执行`。整体替换为：

```markdown
### 7. 保留策略每次写入都执行 ✅ 2026-05-28 已修复
```

- [ ] **Step 3: 提交文档同步**

```bash
git add docs/clipboard-toolbox-design.md docs/2026-05-28-clipboard-toolbox-audit.md
git commit -m "$(cat <<'EOF'
docs(clipboard): 同步连接复用与保留策略触发收敛说明
EOF
)"
```

---

## Final Verification

- [ ] `cd src-tauri; cargo test clipboard` — 后端全量测试通过（含新增 2 个 retention 用例）
- [ ] `cd src-tauri; cargo check` — 编译通过且无新警告
- [ ] `pnpm.cmd build` — 前端构建通过
- [ ] `git diff --check` — 无空白错误
- [ ] `git log --oneline -8` — 应看到 6 个新 commit：
  - `feat(clipboard): 新增 open_connection 助手并启用 WAL pragma`
  - `refactor(clipboard): repository/settings/maintenance API 迁移到 &Connection`
  - `refactor(clipboard): ClipboardService 持有 settings_conn 与 items_conn`
  - `feat(clipboard): 保留策略改为每 50 次捕获触发`
  - `feat(clipboard): 设置更新后重置 retention 计数`
  - `docs(clipboard): 同步连接复用与保留策略触发收敛说明`
- [ ] `git status` — 工作树清洁

## Self-Review

- spec §1 背景 → 现状已分析，无需 Task 实现 ✓
- spec §2 设计原则 → 全局贯彻，不引入 r2d2、保留 settings 立即触发 ✓
- spec §3 整体架构 → Task 3 实现双连接 ✓
- spec §4.1 构造 → Task 3 Step 2 ✓
- spec §4.2 销毁 → Drop 自动，无 Task 实现 ✓
- spec §4.3 锁顺序 → Task 3 Step 9（update_desktop_settings）与 Task 4 Step 5（capture）遵循；run_retention 内部 settings → items ✓
- spec §5.1 open_connection → Task 1 ✓
- spec §5.2 repository API → Task 2 Step 1 ✓
- spec §5.3 settings API → Task 2 Step 3 ✓
- spec §5.4 maintenance API → Task 2 Step 2 ✓
- spec §6.1 阈值常量 → Task 4 Step 3 ✓
- spec §6.2 capture 流程 → Task 4 Step 5 ✓
- spec §6.3 run_retention → Task 3 Step 5 ✓
- spec §6.4 设置更新立即触发 → Task 3 Step 9（保留 run_retention 调用）+ Task 5 Step 4（计数重置）✓
- spec §7 storage_dir 切换 → Task 3 Step 9 ✓
- spec §8 并发安全 → 锁顺序设计已在 Task 3、4、5 落实 ✓
- spec §9.1 repository_tests → Task 2 Step 5 ✓
- spec §9.2 service_tests 直调点 → Task 2 Step 6 ✓
- spec §9.3 新增 2 个用例 → Task 4 Step 1 + Task 5 Step 1 ✓
- spec §10 改动清单 → 全部覆盖 ✓
- spec §11 非目标 → 无 Task 触碰 ✓
- spec §12 验证 → Final Verification 覆盖 ✓

类型一致性：
- `RETENTION_TRIGGER_THRESHOLD: u32 = 50` 在 Task 4 Step 3 定义，Task 4 Step 1、Task 4 Step 5、Task 4 Step 6、Task 5 Step 1 全部以 `u32` 形式引用 ✓
- `captures_since_cleanup: Mutex<u32>` 在 Task 4 Step 4 定义，所有访问点均通过 `.lock()` 取 `u32` ✓
- `settings_conn` / `items_conn` 在 Task 3 Step 1 定义为 `Mutex<Connection>`，Task 3 Step 3 的 `lock_settings_conn` / `lock_items_conn` 返回 `MutexGuard<'_, Connection>` ✓
- `init_schema(conn: &Connection)` 在 Task 2 Step 1 定义，Task 2 Step 5、Task 2 Step 6、Task 3 Step 2、Task 3 Step 9 全部以 `&conn` 调用 ✓
- `open_connection(path: &Path) -> Result<Connection, ClipboardError>` 在 Task 1 Step 2 定义，所有调用点 path 类型一致 ✓

无 TBD / TODO 占位 ✓
