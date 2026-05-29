# 时区安全日期分组 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将剪贴板时间戳改存 UTC、新增独立 `local_date` 列承载日期分组，并对存量数据做幂等全量迁移，使排序与分组在任何时区/夏令时下都正确。

**Architecture:** 后端三处协同——`service_runtime` 改 `now_iso` 输出 UTC 并新增 `today_local`；`repository` 在 schema 中加 `local_date` 列与对应索引、新增幂等 `migrate_schema`、为写入函数加 `local_date` 入参、把四处按日期 SQL 从 `substr(created_at,1,10)` 改为 `local_date`；`service::new` 在 `init_schema` 后对两连接调 `migrate_schema`，capture 路径传 `today_local()`。前端零改动（new Date 解析 UTC `Z` 串、`local_date` 与 `todayKey()` 同为本地民用日期）。

**Tech Stack:** Rust 2021, rusqlite (bundled), chrono, SQLite（`strftime` 做偏移→UTC 转换、`PRAGMA table_info` 做幂等判定）。

参考 spec：`docs/superpowers/specs/2026-05-29-timezone-safe-date-grouping-design.md`

---

## File Structure

- `src-tauri/src/clipboard/service_runtime.rs` — `now_iso` 改 UTC 整秒、新增 `today_local`
- `src-tauri/src/clipboard/repository.rs` — `init_schema` 加 `local_date` 列与新索引、新增 `migrate_schema`、`insert_text_item`/`upsert_text_item` 加 `local_date` 入参、四处查询改 `local_date`
- `src-tauri/src/clipboard/service.rs` — `new` 调 `migrate_schema`、capture 传 `today_local()`
- `src-tauri/src/clipboard/settings.rs` — 两处时间戳改 `now_iso()`
- `src-tauri/src/clipboard/repository_tests.rs` — 新增迁移/排序/分组测试 + 既有 `upsert_text_item` 调用补 `local_date`
- `src-tauri/src/clipboard/service_tests.rs` — 既有 `upsert_text_item` 调用补 `local_date`
- `docs/clipboard-toolbox-design.md` — §7.1 schema 补 `local_date`、§5.2/§11 同步
- `docs/2026-05-28-clipboard-toolbox-audit.md` — 标记 P0 #5 已修复

执行顺序：先做无依赖的 helper（Task 1），再做 repository 的 schema/迁移/签名（Task 2-5），然后 service 接线（Task 6），settings 统一（Task 7），最后文档（Task 8）。每个 Task 内部 TDD red→green→commit。

---

### Task 1: `service_runtime` 时间戳助手改 UTC + 新增 `today_local`

**Files:**
- Modify: `src-tauri/src/clipboard/service_runtime.rs`

- [ ] **Step 1: 阅读现状确认改动点**

打开 `src-tauri/src/clipboard/service_runtime.rs`，确认当前 `now_iso` 实现：

```rust
pub fn now_iso() -> String {
    Local::now().to_rfc3339()
}
```

确认文件顶部 `use chrono::...`（当前应已 `use chrono::Local;`）。

- [ ] **Step 2: 改 `now_iso` 为 UTC 整秒并新增 `today_local`**

把 `use chrono::Local;` 改为同时引入 `Utc`（若已是 `use chrono::{Local, ...};` 则把 `Utc` 加进花括号；否则新增 `use chrono::Utc;`）。替换 `now_iso`，并在其后新增 `today_local`：

```rust
pub fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn today_local() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}
```

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri; cargo check`
Expected: 通过（可能出现 `today_local` 未被使用的 warning，后续 Task 6 接线后消除；`now_iso` 调用处签名不变，不报错）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/clipboard/service_runtime.rs
git commit -m "refactor(clipboard): now_iso 改存 UTC 整秒并新增 today_local"
```

---

### Task 2: `init_schema` 加 `local_date` 列与新索引、删旧索引

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs`（`init_schema` 内的建表/建索引语句）

- [ ] **Step 1: 写失败测试 —— 新建库含 local_date 列且有新索引**

在 `src-tauri/src/clipboard/repository_tests.rs` 末尾追加（`use` 已含 `super::*` / `rusqlite::Connection`，沿用文件现有 in-memory 连接构造方式；下方 `open_in_memory()` 替换为该文件既有的建连辅助，如直接 `Connection::open_in_memory().unwrap()` 后 `init_schema`）：

```rust
#[test]
fn init_schema_creates_local_date_column_and_index() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();

    let has_local_date: bool = conn
        .prepare("PRAGMA table_info(clipboard_items)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(Result::ok)
        .any(|name| name == "local_date");
    assert!(has_local_date, "local_date column should exist");

    let has_index: bool = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name=?1")
        .unwrap()
        .query_map(["idx_clipboard_items_local_date_active"], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(Result::ok)
        .next()
        .is_some();
    assert!(has_index, "local_date active index should exist");
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri; cargo test clipboard::repository_tests::init_schema_creates_local_date_column_and_index`
Expected: FAIL（无 `local_date` 列）。

- [ ] **Step 3: 在 `init_schema` 建表语句加列、调整索引**

在 `init_schema` 的 `CREATE TABLE IF NOT EXISTS clipboard_items (...)` 中，于 `deleted_at TEXT` 之后加一列 `local_date TEXT`（新建库列即存在，迁移对存量库补列）。把原 `idx_clipboard_items_created_at_active` 索引创建语句替换为：

```rust
connection.execute(
    "CREATE INDEX IF NOT EXISTS idx_clipboard_items_local_date_active
        ON clipboard_items(local_date)
        WHERE deleted_at IS NULL",
    [],
)?;
connection.execute(
    "DROP INDEX IF EXISTS idx_clipboard_items_created_at_active",
    [],
)?;
```

> 保留 `DROP INDEX IF EXISTS` 以便对存量库（旧索引存在）也清掉。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri; cargo test clipboard::repository_tests::init_schema_creates_local_date_column_and_index`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/repository_tests.rs
git commit -m "feat(clipboard): init_schema 新增 local_date 列与活动索引"
```

---

### Task 3: 新增幂等 `migrate_schema`（回填 local_date + 时间戳转 UTC）

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs`（新增 `migrate_schema`）
- Test: `src-tauri/src/clipboard/repository_tests.rs`

- [ ] **Step 1: 写失败测试 —— 迁移回填 + 转 UTC + 幂等**

在 `repository_tests.rs` 追加。构造一条旧格式行（`created_at` 带 `+08:00` 偏移且含纳秒小数、无 `local_date`），跑迁移后断言：

```rust
#[test]
fn migrate_schema_backfills_local_date_and_converts_to_utc() {
    let conn = Connection::open_in_memory().unwrap();
    // 建一张不含 local_date 的旧表，模拟存量库
    conn.execute(
        "CREATE TABLE clipboard_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            preview TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_copied_at TEXT NOT NULL,
            copy_count INTEGER NOT NULL DEFAULT 1,
            deleted_at TEXT
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO clipboard_items
            (content, content_hash, preview, created_at, last_copied_at, copy_count, deleted_at)
         VALUES ('hi', 'h', 'hi',
            '2026-05-29T08:53:00.123456789+08:00',
            '2026-05-29T09:00:00.000000000+08:00', 1, NULL)",
        [],
    )
    .unwrap();

    migrate_schema(&conn).unwrap();

    let (local_date, created_at, last_copied_at): (String, String, String) = conn
        .query_row(
            "SELECT local_date, created_at, last_copied_at FROM clipboard_items WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!("2026-05-29", local_date, "local_date 取转换前本地日期");
    assert_eq!("2026-05-29T00:53:00Z", created_at, "created_at 转 UTC 整秒");
    assert_eq!("2026-05-29T01:00:00Z", last_copied_at, "last_copied_at 转 UTC 整秒");
}

#[test]
fn migrate_schema_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap(); // 已含 local_date 列
    // 连续两次不应报错、不应改变行为
    migrate_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let has_local_date: bool = conn
        .prepare("PRAGMA table_info(clipboard_items)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(Result::ok)
        .any(|name| name == "local_date");
    assert!(has_local_date);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri; cargo test clipboard::repository_tests::migrate_schema`
Expected: FAIL（`migrate_schema` 未定义，编译错误）。

- [ ] **Step 3: 实现 `migrate_schema`**

在 `repository.rs` 新增（紧邻 `init_schema`）：

```rust
pub fn migrate_schema(connection: &Connection) -> Result<(), ClipboardError> {
    let already_migrated = {
        let mut stmt = connection.prepare("PRAGMA table_info(clipboard_items)")?;
        let names = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut found = false;
        for name in names {
            if name? == "local_date" {
                found = true;
                break;
            }
        }
        found
    };
    if already_migrated {
        return Ok(());
    }

    connection.execute(
        "ALTER TABLE clipboard_items ADD COLUMN local_date TEXT",
        [],
    )?;
    connection.execute(
        "UPDATE clipboard_items
         SET local_date = substr(created_at, 1, 10),
             created_at = strftime('%Y-%m-%dT%H:%M:%SZ', created_at),
             last_copied_at = strftime('%Y-%m-%dT%H:%M:%SZ', last_copied_at),
             deleted_at = CASE
                 WHEN deleted_at IS NOT NULL
                 THEN strftime('%Y-%m-%dT%H:%M:%SZ', deleted_at)
                 ELSE NULL
             END",
        [],
    )?;
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_clipboard_items_local_date_active
            ON clipboard_items(local_date)
            WHERE deleted_at IS NULL",
        [],
    )?;
    connection.execute(
        "DROP INDEX IF EXISTS idx_clipboard_items_created_at_active",
        [],
    )?;
    Ok(())
}
```

> `substr(created_at,1,10)` 在同一 UPDATE 内取的是行原始值（SQLite SET 表达式基于原始行求值），故先拿旧本地日期，再覆盖 `created_at`，正确。`PRAGMA table_info` 第 2 列（索引 1）是列名。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri; cargo test clipboard::repository_tests::migrate_schema`
Expected: PASS（两个测试均过）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/repository_tests.rs
git commit -m "feat(clipboard): 新增幂等 migrate_schema 回填 local_date 并转 UTC"
```

---

### Task 4: 写入函数加 `local_date` 入参

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs`（`insert_text_item`、`upsert_text_item` 签名 + INSERT）
- Modify: `src-tauri/src/clipboard/repository_tests.rs`（既有调用补参 + 新增分组测试）
- Modify: `src-tauri/src/clipboard/service_tests.rs`（既有调用补参）

- [ ] **Step 1: 改 `upsert_text_item` / `insert_text_item` 签名与 INSERT**

`upsert_text_item` 改为：

```rust
pub fn upsert_text_item(
    connection: &Connection,
    content: &str,
    content_hash: &str,
    now: &str,
    local_date: &str,
) -> Result<ClipboardItem, ClipboardError> {
    // 重复命中分支保持不变（仅更新 last_copied_at / copy_count，不传 local_date）
    // 新插入分支转调 insert_text_item(connection, content, content_hash, now, local_date)
}
```

`insert_text_item` 同样增加 `local_date: &str` 参数，并在 `INSERT INTO clipboard_items (... , created_at, last_copied_at, copy_count, local_date) VALUES (..., ?, ?, 1, ?)` 中写入 `local_date`（按该文件现有参数绑定风格补一个绑定位）。`update_existing_item` 不变（保留首次捕获的 `local_date`）。

> 具体 SQL 文本以文件现状为准：在现有 INSERT 的列清单和 VALUES 占位符各加一项 `local_date` / 对应 `?`，并把 `local_date` 加入参数元组末尾。

- [ ] **Step 2: 更新 `repository_tests.rs` 既有调用 + 新增分组测试**

把该文件所有 `upsert_text_item(&conn, content, hash, now)` 调用补第 5 参 `local_date`。对断言特定日期分组的测试，显式传该日期，例如断言 `groups[0].date == "2026-05-26"` 的插入处传 `"2026-05-26"`。新增一个分组测试明确基于 `local_date`：

```rust
#[test]
fn list_date_groups_uses_local_date_column() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    upsert_text_item(&conn, "a", "ha", "2026-05-26T10:00:00Z", "2026-05-26").unwrap();
    upsert_text_item(&conn, "b", "hb", "2026-05-27T10:00:00Z", "2026-05-27").unwrap();

    let groups = list_date_groups(&conn).unwrap();
    assert_eq!("2026-05-27", groups[0].date);
    assert_eq!("2026-05-26", groups[1].date);
}
```

- [ ] **Step 3: 更新 `service_tests.rs` 既有调用**

把 `service_tests.rs` 中所有 `repository::upsert_text_item(...)`（约 5 处）补 `local_date` 参数，传与该测试时间戳一致的本地日期串（如时间戳 `"2026-05-26T..."` 则传 `"2026-05-26"`）。

- [ ] **Step 4: 运行测试确认通过（含新增分组测试）**

Run: `cd src-tauri; cargo test clipboard`
Expected: PASS（此刻 capture 调用点尚未改，会编译失败——见下）。若 `service.rs` capture 调用点报参数数量不符，是预期的，Task 5/6 会修；为保持本 Task 可编译，**在 Step 1 完成后立即同步改 service.rs 的 capture 调用点**（见 Task 6 Step 注），或先在此处仅运行 `cargo test --no-run` 确认测试文件本身签名正确，把 capture 调用点修正合并到本 Task 提交。

> 实操建议：本 Task 与 Task 6 的 capture 调用点绑定在一起改可一次编译通过。若严格分 Task，请在本 Task Step 1 后立即把 `service.rs` capture 调用临时补 `&today_local()`（Task 1 已提供该函数），使整树可编译，再继续。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/repository_tests.rs src-tauri/src/clipboard/service_tests.rs src-tauri/src/clipboard/service.rs
git commit -m "feat(clipboard): 写入路径加 local_date 入参并更新测试调用"
```

---

### Task 5: 四处按日期查询改用 `local_date` + 排序回归测试

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs`（`list_date_groups`、`list_items_by_date`、`soft_delete_items_by_date`、`cleanup_by_date`）
- Modify: `src-tauri/src/clipboard/repository_tests.rs`（排序回归测试）

- [ ] **Step 1: 写失败/回归测试 —— 排序按 UTC 真实时刻倒序**

```rust
#[test]
fn list_items_by_date_orders_by_real_utc_time() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    // 两条同 local_date、last_copied_at 为 UTC：07:00Z 实际晚于 00:53Z
    upsert_text_item(&conn, "early", "he", "2026-05-29T00:53:00Z", "2026-05-29").unwrap();
    upsert_text_item(&conn, "late", "hl", "2026-05-29T07:00:00Z", "2026-05-29").unwrap();

    let items = list_items_by_date(&conn, "2026-05-29").unwrap();
    assert_eq!("late", items[0].content, "更晚的 UTC 时刻排在前");
    assert_eq!("early", items[1].content);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri; cargo test clipboard::repository_tests::list_items_by_date_orders_by_real_utc_time`
Expected: FAIL（`list_items_by_date` 仍按 `substr(created_at,1,10)` 过滤；新行 `local_date` 已写入但查询用旧表达式取不到，返回空 → 断言越界 panic）。

- [ ] **Step 3: 四处查询把 `substr(created_at, 1, 10)` 改为 `local_date`**

- `list_date_groups`：`SELECT local_date AS date, COUNT(*) AS count FROM clipboard_items WHERE deleted_at IS NULL GROUP BY local_date ORDER BY local_date DESC`
- `list_items_by_date`：`WHERE deleted_at IS NULL AND local_date = ?1 ORDER BY last_copied_at DESC, id DESC`
- `soft_delete_items_by_date`：`WHERE local_date = ?2 AND deleted_at IS NULL`
- `cleanup_by_date`：`WHERE deleted_at IS NULL AND local_date < ?2`

> `ORDER BY last_copied_at DESC` 在 UTC `Z` 格式下字典序即真实时刻序，无需改排序表达式。占位符编号沿用各函数原有绑定，仅替换列引用。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri; cargo test clipboard`
Expected: PASS（排序回归 + 既有用例全过）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/repository_tests.rs
git commit -m "feat(clipboard): 按日期查询改用 local_date 并补排序回归测试"
```

---

### Task 6: `service` 接线 —— `new` 调 `migrate_schema`、capture 传 `today_local()`

**Files:**
- Modify: `src-tauri/src/clipboard/service.rs`

> 注：若 Task 4 已将 capture 调用点临时补参以保持编译，本 Task 收口为「确认 capture 传 `today_local()` 而非占位值」并加 `migrate_schema` 调用。

- [ ] **Step 1: `new` 在 `init_schema` 后对两连接调 `migrate_schema`**

在 `ClipboardService::new` 中，对 settings 连接与 items 连接各自 `init_schema` 之后紧跟 `repository::migrate_schema(&conn)?;`（两个连接都调，存量库才会被迁移；空 settings 库迁移安全，UPDATE 影响 0 行）。

- [ ] **Step 2: capture 调用点传 `today_local()`**

把 `repository::upsert_text_item(&conn, &content, &hash, &service_runtime::now_iso())?` 改为：

```rust
repository::upsert_text_item(
    &conn,
    &content,
    &hash,
    &service_runtime::now_iso(),
    &service_runtime::today_local(),
)?
```

- [ ] **Step 3: 全量测试 + check**

Run: `cd src-tauri; cargo test clipboard; cargo check`
Expected: 测试 PASS；`cargo check` 无新 warning（`today_local` 此刻已被使用）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/clipboard/service.rs
git commit -m "feat(clipboard): service.new 调 migrate_schema、capture 传 today_local"
```

---

### Task 7: `settings.rs` 时间戳统一为 `now_iso()`

**Files:**
- Modify: `src-tauri/src/clipboard/settings.rs`

- [ ] **Step 1: 两处 `Local::now().to_rfc3339()` 改 `now_iso()`**

`update_stored_settings` 与 `apply_retention_policy` 中各一处 `Local::now().to_rfc3339()` 改为 `service_runtime::now_iso()`（保持模块引用风格；若文件已 `use super::service_runtime;` 直接用，否则补引用）。

`apply_retention_policy` 的 cutoff 计算保持本地日期：

```rust
let cutoff = Local::now() - Duration::days(settings.retention_days);
let cutoff_date = cutoff.format("%Y-%m-%d").to_string();
```

`cutoff_date` 与 `local_date` 同为本地民用日期，`cleanup_by_date` 的 `local_date < cutoff_date` 语义一致。若 `Local` / `Duration` 在改动后变为未使用，按编译器提示清理 import；cutoff 仍用 `Local`，故 `Local` 应保留。

- [ ] **Step 2: 全量测试 + check**

Run: `cd src-tauri; cargo test clipboard; cargo check`
Expected: 测试 PASS；无新 warning。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/clipboard/settings.rs
git commit -m "refactor(clipboard): settings 时间戳统一为 now_iso (UTC)"
```

---

### Task 8: 文档同步 + 标记审查项

**Files:**
- Modify: `docs/clipboard-toolbox-design.md`
- Modify: `docs/2026-05-28-clipboard-toolbox-audit.md`

- [ ] **Step 1: 更新设计文档 schema 说明**

在 `docs/clipboard-toolbox-design.md` §7.1 的 `clipboard_items` schema 描述中补 `local_date TEXT` 列及其用途（按日期分组依据，写入时刻本地民用日期）；§5.2 / §11（若有时间戳/索引描述）同步说明 `created_at`/`last_copied_at` 改存 UTC、索引由 `created_at` 改为 `local_date`。

- [ ] **Step 2: 标记 P0 #5 已修复**

在 `docs/2026-05-28-clipboard-toolbox-audit.md`：
- 第 52 行标题 `### 5. 日期分组依赖时区敏感字段` 末尾加 ` ✅ 2026-05-29 已修复`
- 审查清单表格 P0 #5 行（约第 182 行 `| P0 | 5 | 日期分组时区敏感 | 数据正确性 |`）补已修复标记，与 #1-#4 的标记风格一致

- [ ] **Step 3: Commit**

```bash
git add docs/clipboard-toolbox-design.md docs/2026-05-28-clipboard-toolbox-audit.md
git commit -m "docs(clipboard): 同步 local_date schema 说明并标记 P0 #5 修复"
```

---

## 验证（全部 Task 完成后）

- `cd src-tauri; cargo test clipboard` 全量通过（含迁移回填、迁移幂等、分组基于 local_date、排序回归）
- `cd src-tauri; cargo check` 无新 warning
- `pnpm.cmd build` 前端构建通过（验证零改动约束）
- 手动：保留一份旧格式 DB 启动应用，确认旧记录仍按原日期分组、排序正确、重复启动不重复迁移
