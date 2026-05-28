# 连接复用与保留策略触发收敛设计

> 日期：2026-05-28
> 范围：`src-tauri/src/clipboard/` 模块（repository、settings、maintenance、service、service_runtime）
> 目标：消除 SQLite 每次调用 `Connection::open` 的开销，并把保留策略从"每次写入触发"改为"阈值计数触发 + 设置变更立即触发"

---

## 1. 背景

当前 `src-tauri/src/clipboard/repository.rs` 12 个公开函数（`init_database`、`upsert_text_item`、`list_date_groups`、`list_items_by_date`、`search_items`、`get_item_by_id`、`soft_delete_item`、`soft_delete_items_by_date`、`get_i64_setting`、`get_string_setting`、`set_setting`、`cleanup_items`）以及 `maintenance.rs` 的 `purge_deleted_items`、`vacuum_database`，全部走 `Connection::open(path)?` 模式：每次调用都新开一个 SQLite 连接、用完即丢。

此外 `service.rs::capture_current_clipboard` 在每次成功写入后立即调用 `apply_retention_policy`，对应 `repository::cleanup_items` 会执行一次按日期裁剪 + 按数量裁剪。后者的 `cleanup_by_count` 使用 `ORDER BY last_copied_at DESC, id DESC LIMIT -1 OFFSET ?`，需要对表做全量排序，开销随表大小线性增长。

**问题**：

- 每次操作 open/close 连接，pragma、文件锁、缓存预热都被反复支付
- 每次复制都触发保留策略全表扫描，浪费 99% 以上的工作（用户通常不会一次性突破 max_record_count）
- 监控线程 800ms 一次轮询，意味着稳定输入下每秒约 1 次连接生命周期 + 1 次清理排序

## 2. 设计原则

- **YAGNI**：不引入连接池（`r2d2_sqlite`），单进程桌面应用不需要并发取连接的能力
- **沿用现有 `Mutex<...>` 模式**：`ClipboardService` 已有 `Mutex<PathBuf>`、`Mutex<Option<String>>` 等，连接复用走同样的锁定模型
- **分离两个逻辑库**：settings 始终在 `default_database_path`，items 走 `active_database_path`（受 `storage_dir` 配置控制），分别持有连接
- **保留外部 API 不变**：Tauri 命令层与监控线程对外签名零变化
- **保留 settings 立即触发 retention**：用户调小保留阈值后期望立即生效

## 3. 整体架构

```
ClipboardService
├── default_database_path: PathBuf            (settings DB 路径，构造期确定)
├── database_path: Mutex<PathBuf>             (items DB 当前路径)
├── settings_conn: Mutex<Connection>          (新增：常驻 settings 连接)
├── items_conn: Mutex<Connection>             (新增：常驻 items 连接)
├── last_seen_hash: Mutex<Option<String>>
├── last_app_write: Mutex<Option<AppWriteGuard>>
├── monitor_enabled: Mutex<bool>
└── captures_since_cleanup: Mutex<u32>        (新增：触发保留策略的计数器)
```

两个 Mutex<Connection> 在 `storage_dir` 为空（默认值）时指向同一 SQLite 文件，但仍按"设置只走 settings_conn、记录只走 items_conn"切分访问；启用自定义目录时它们指向不同文件，分离锁让设置读取不阻塞记录写入。WAL 模式支持同一文件多连接并发，无一致性风险。

## 4. ClipboardService 状态与生命周期

### 4.1 构造

`ClipboardService::new(default_database_path: PathBuf)`：

1. `settings_conn = service_runtime::open_connection(&default_database_path)?`
2. `repository::init_schema(&settings_conn)?`
3. 通过 settings_conn 读取 `storage_dir` 设置，结合 `service_runtime::resolve_database_path` 解析 `active_path`
4. `items_conn = service_runtime::open_connection(&active_path)?`
5. `repository::init_schema(&items_conn)?`
6. 计数器初始 0；其他字段沿用现有初始化

### 4.2 销毁

`Drop` 自动关闭两个连接。无需手动 `close()`。

### 4.3 锁顺序约定

固定顺序：`database_path → settings_conn → items_conn → captures_since_cleanup`。其他互斥锁（`monitor_enabled`、`last_seen_hash`、`last_app_write`）与连接锁互不相关。所有路径遵循此顺序避免环形死锁。

## 5. 模块接口契约

### 5.1 `service_runtime.rs` 新增

```rust
pub fn open_connection(path: &Path) -> Result<Connection, ClipboardError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; \
         PRAGMA synchronous=NORMAL; \
         PRAGMA foreign_keys=ON;",
    )?;
    Ok(conn)
}
```

所有打开连接都走这一入口，集中处理目录创建与 pragma。`foreign_keys=ON` 保持与原 `init_database` 行为一致（SQLite 默认 OFF）。

### 5.2 `repository.rs` 签名调整

| 现签名 | 新签名 |
|---|---|
| `init_database(path: &Path)` | `init_schema(conn: &Connection)` |
| `upsert_text_item(path, content, hash, ts)` | `upsert_text_item(conn, content, hash, ts)` |
| `list_date_groups(path)` | `list_date_groups(conn)` |
| `list_items_by_date(path, date)` | `list_items_by_date(conn, date)` |
| `search_items(path, query)` | `search_items(conn, query)` |
| `get_item_by_id(path, id)` | `get_item_by_id(conn, id)` |
| `soft_delete_item(path, id, deleted_at)` | `soft_delete_item(conn, id, deleted_at)` |
| `soft_delete_items_by_date(path, date, deleted_at)` | `soft_delete_items_by_date(conn, date, deleted_at)` |
| `get_i64_setting(path, key, default)` | `get_i64_setting(conn, key, default)` |
| `get_string_setting(path, key, default)` | `get_string_setting(conn, key, default)` |
| `set_setting(path, key, value, updated_at)` | `set_setting(conn, key, value, updated_at)` |
| `cleanup_items(path, cutoff, max_count, now)` | `cleanup_items(conn, cutoff, max_count, now)` |

私有 helper `cleanup_by_date`、`cleanup_by_count` 同步改 `&Connection`。事务起点从 `let mut conn = Connection::open(path)?; let tx = conn.transaction()?;` 改为 `let tx = conn.unchecked_transaction()?;`（因为 `&Connection` 无法 `transaction()`，但 `unchecked_transaction` 提供同样语义，调用方持锁保证独占性）。

`init_database` 改名 `init_schema` 反映"只跑 migration、不打开数据库"的新职责。

### 5.3 `settings.rs` 签名调整

| 现签名 | 新签名 |
|---|---|
| `get_stored_settings(path)` | `get_stored_settings(conn)` |
| `update_stored_settings(path, monitor, retention, max_count, max_text, ignore_pwd, custom, storage)` | `update_stored_settings(conn, /* 同 7 字段 */)` |
| `update_monitor_enabled(path, enabled)` | `update_monitor_enabled(conn, enabled)` |
| `apply_retention_policy(path, settings_path)` | `apply_retention_policy(items_conn, settings_conn)` |
| `validate_storage_dir(storage_dir)` | 不变（不碰 DB） |
| `content_skip_reason(content, settings)` | 不变 |
| `validate_custom_secret_patterns(patterns)` | 不变 |

`apply_retention_policy` 改为接两个连接：先 `get_stored_settings(settings_conn)` 取保留天数与最大记录数，再 `repository::cleanup_items(items_conn, &cutoff_date, max_record_count, &now)`。函数本身不再持有任何 path。

### 5.4 `maintenance.rs` 签名调整

| 现签名 | 新签名 |
|---|---|
| `purge_deleted_items(path)` | `purge_deleted_items(conn)` |
| `vacuum_database(path)` | `vacuum_database(conn)` |

注意：SQLite `VACUUM` 要求无活跃事务。调用方持有 `items_conn` 锁时其他读写路径在等锁，无冲突。

## 6. 触发计数与保留策略

### 6.1 常量

```rust
pub(super) const RETENTION_TRIGGER_THRESHOLD: u32 = 50;
```

放在 `service.rs` 顶部。`pub(super)` 允许 `service_tests` 引用以构造测试场景。

### 6.2 `capture_current_clipboard` 新流程

伪代码（保留既有跳过判断、AppWriteGuard 抑制、last_seen_hash 比较）：

```rust
// 1) 写入 items（短临界区）
{
    let conn = self.items_conn.lock().expect("items connection poisoned");
    repository::upsert_text_item(&conn, &content, &hash, &now)?;
}

// 2) 计数 + 阈值决策（独立临界区，不嵌套连接锁）
let should_clean = {
    let mut count = self.captures_since_cleanup.lock().unwrap();
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

### 6.3 `run_retention` 私有方法

```rust
fn run_retention(&self) -> Result<usize, ClipboardError> {
    let settings_conn = self.settings_conn.lock().unwrap();
    let items_conn = self.items_conn.lock().unwrap();
    settings::apply_retention_policy(&items_conn, &settings_conn)
}
```

所有保留触发统一走此方法。锁顺序固定 settings → items，与全局约定一致。

### 6.4 `update_desktop_settings` 立即触发

设置更新后无条件调用 `run_retention` + 重置计数器：

```rust
self.run_retention()?;
*self.captures_since_cleanup.lock().unwrap() = 0;
```

理由：用户调小 retention_days 或 max_record_count 后期望立即生效；同时已经做过一次清理，没必要让计数器仍指向旧周期。

## 7. 存储路径切换

`update_desktop_settings` 完整流程：

```rust
pub fn update_desktop_settings(
    &self,
    update: DesktopSettingsUpdate,
    autostart_supported: bool,
) -> Result<DesktopSettings, ClipboardError> {
    settings::validate_custom_secret_patterns(&update.custom_secret_patterns)?;
    settings::validate_storage_dir(&update.storage_dir)?;

    // 1) 写设置（持 settings_conn 短临界区）
    let stored = {
        let conn = self.settings_conn.lock().unwrap();
        settings::update_stored_settings(
            &conn,
            update.monitor_enabled,
            update.retention_days,
            update.max_record_count,
            update.max_text_length,
            update.ignore_password_like_text,
            &update.custom_secret_patterns,
            &update.storage_dir,
        )?
    };

    // 2) storage_dir 变化时重建 items_conn
    let new_active = service_runtime::resolve_database_path(
        &self.default_database_path,
        &stored.storage_dir,
    );
    {
        let mut path = self.database_path.lock().unwrap();
        if new_active != *path {
            let new_conn = service_runtime::open_connection(&new_active)?;
            repository::init_schema(&new_conn)?;
            let mut items = self.items_conn.lock().unwrap();
            *items = new_conn;
            *path = new_active;
        }
    }

    // 3) 同步 monitor_enabled 内存态
    *self.monitor_enabled.lock().unwrap() = stored.monitor_enabled;

    // 4) 立即触发 retention + 重置计数
    self.run_retention()?;
    *self.captures_since_cleanup.lock().unwrap() = 0;

    self.desktop_settings(autostart_supported)
}
```

旧 items 连接随 `*items = new_conn` 赋值被 `Drop`，无需手动 close。`database_path → items_conn` 是此函数局部的锁顺序，其他路径不会反向同时持有这两把锁。

## 8. 并发安全与锁分析

- **监控线程**（`monitor.rs`）：每 800ms 一次 `service.capture_current_clipboard()`。临界区只覆盖 items_conn 单次 upsert + 可能的 retention。
- **Tauri 命令线程池**：所有读取命令（`list_*`、`search_items`、`get_item_by_id`）只 lock items_conn 短时间；设置命令 lock settings_conn 短时间；retention 同时持两把锁但 capture 路径已经先释放再申请，无嵌套。
- **死锁分析**：所有持双连接锁的路径（`run_retention`、`update_desktop_settings`）都按 settings_conn → items_conn 顺序申请。capture 路径申请单锁后释放再申请双锁，不存在反向链。
- **性能影响**：与原来"每次新连接"相比，单次访问省去 open + pragma + close 三步。写并发被序列化但桌面单用户场景无意义并发。读并发在 WAL 模式下原本可以并行，现在被 Mutex 序列化——可接受，因为读取命令延迟以毫秒计。

## 9. 测试改造

### 9.1 `repository_tests.rs`（11 个用例）

模板替换。`init_database(&path)` → `let conn = service_runtime::open_connection(&path).unwrap(); init_schema(&conn).unwrap();`，后续所有 repository 调用首参从 `&path` 换为 `&conn`。文件顶部 import 增加 `use super::service_runtime;`，`init_database` 改 `init_schema`。

### 9.2 `service_tests.rs`

3 处直调 repository 改造（`switch-default` 用例两处 `upsert_text_item`、`startup-default` 用例的 `init_database` + `upsert_text_item`），同样模板。`settings::update_stored_settings(&default_path, ...)` 调用改为先开临时 settings 连接：

```rust
let settings_conn = service_runtime::open_connection(&default_path).unwrap();
repository::init_schema(&settings_conn).unwrap();
settings::update_stored_settings(&settings_conn, false, 15, 50, 1024, true, "", &custom_dir_text).unwrap();
```

### 9.3 新增测试（净增 2 个）

```rust
#[test]
fn retention_triggers_only_after_threshold() {
    use super::service::RETENTION_TRIGGER_THRESHOLD;

    let path = temp_database_path("threshold");
    let service = ClipboardService::new(path).unwrap();

    // 设置极小的 max_record_count
    service
        .update_desktop_settings(
            DesktopSettingsUpdate {
                max_record_count: 3,
                ..desktop_update(String::new())
            },
            false,
        )
        .unwrap();

    // 写入 THRESHOLD - 1 条不重复记录，不应触发清理
    for i in 0..(RETENTION_TRIGGER_THRESHOLD - 1) {
        let conn = /* lock items_conn via test helper or call internal */;
        // 通过测试辅助直接 upsert，绕过 capture_current_clipboard 的剪贴板依赖
    }
    // 断言：当前条数 > max_record_count（说明 retention 未触发）

    // 再写一条触达阈值，触发清理后条数应回落到 max_record_count
}

#[test]
fn retention_counter_resets_after_settings_update() {
    // 类似上面，写入 THRESHOLD/2 条后调 update_desktop_settings，
    // 再写 THRESHOLD/2 + 1 条，第二批不应触发（因为计数已重置）
}
```

实施细节：因为 `capture_current_clipboard` 依赖真实剪贴板，新增测试通过 `ClipboardService` 内部测试钩子或直接持锁 upsert + 手动调 `run_retention` 模拟。具体方式由实施 Plan 决定（可能需要在 `service.rs` 暴露 `pub(super) fn captures_count_for_test() -> u32` 或类似 hook）。

### 9.4 不动文件

- `storage_path_tests.rs`：只测 `validate_storage_dir`
- `settings.rs::tests`：只测 `is_password_like_text`（来自 P0 #4 修复，14 个用例）

## 10. 改动清单

| 路径 | 类别 | 主要变更 |
|---|---|---|
| `src-tauri/src/clipboard/service_runtime.rs` | 修改 | 新增 `open_connection` 含 WAL/synchronous/foreign_keys pragma |
| `src-tauri/src/clipboard/repository.rs` | 修改 | 12 个公开函数签名 `&Path → &Connection`，`init_database → init_schema`，私有 helper 同步 |
| `src-tauri/src/clipboard/settings.rs` | 修改 | 4 个公开函数签名改 `&Connection`，`apply_retention_policy` 改为接两个连接 |
| `src-tauri/src/clipboard/maintenance.rs` | 修改 | 2 个公开函数签名改 `&Connection` |
| `src-tauri/src/clipboard/service.rs` | 修改 | 新增 `settings_conn`、`items_conn`、`captures_since_cleanup` 字段；新增 `run_retention` 与 `RETENTION_TRIGGER_THRESHOLD` 常量；构造、capture、settings 更新、search、list、soft_delete、maintenance 等方法全部改为持锁取连接 |
| `src-tauri/src/clipboard/repository_tests.rs` | 修改 | 11 个用例模板替换 + import 调整 |
| `src-tauri/src/clipboard/service_tests.rs` | 修改 | 3 处直调改造 + 新增 2 个用例验证阈值与计数重置 |
| `src-tauri/src/clipboard/commands.rs` | 不动 | 命令层仅调 `ClipboardService` 方法，签名不变 |
| `src-tauri/src/clipboard/monitor.rs` | 不动 | 仅调 `service.capture_current_clipboard()` |
| `src-tauri/src/clipboard/storage_path_tests.rs` | 不动 | 仅测 path 校验 |
| `docs/clipboard-toolbox-design.md` | 修改 | 同步双连接、WAL、阈值触发描述 |
| `docs/2026-05-28-clipboard-toolbox-audit.md` | 修改 | P1 #6 与 P1 #7 标记为已完成 |

## 11. 非目标

- 不引入 `r2d2` / 连接池库
- 不调整监控线程轮询节奏（仍 800ms）
- 不改 Tauri 命令签名或前端 IPC 接口
- 不调整 SQLite schema、不新增索引（与 audit 中其他 P1 项独立）
- 不改 `is_password_like_text` 或敏感识别逻辑
- 不暴露 `RETENTION_TRIGGER_THRESHOLD` 为用户设置项

## 12. 验证

- `cd src-tauri; cargo test clipboard` — 全量后端测试通过（新增 2 个用例 + 既有用例全绿）
- `cd src-tauri; cargo check` — 编译通过且无新警告
- `pnpm.cmd build` — 前端构建通过
- `git diff --check` — 无空白错误
- 手动：启动应用、连续复制 60 次不重复文本 → 第 50 次触发清理（max_record_count 默认 1000 时无可见现象，可临时调小 max_record_count 至 30 验证可见效果）
- 手动：在自定义 storage_dir 设置切换 → 新目录生成 `clipboard.sqlite`，旧 items 文件不再写入；切换回默认目录后旧 items 仍可读
