# 时区安全的日期分组设计

> 设计日期：2026-05-29
> 审查项：`docs/2026-05-28-clipboard-toolbox-audit.md` P0 #5
> 范围：`src-tauri/src/clipboard/` 后端；前端零改动

## 1. 背景与问题

当前剪贴板记录的时间戳由 `service_runtime::now_iso()` 生成：

```rust
pub fn now_iso() -> String {
    Local::now().to_rfc3339()  // 例：2026-05-29T08:53:00+08:00
}
```

`created_at` / `last_copied_at` 存的是「带本地时区偏移的 RFC3339 字符串」。日期分组、按日期查询、按日期清理、按日期软删除全部用 `substr(created_at, 1, 10)` 截前 10 位拿日期。

存在两个真实缺陷：

1. **排序不可靠**：`list_items_by_date` / `search_items` 用 `ORDER BY last_copied_at DESC` 对带偏移的 RFC3339 字符串做字典序比较。跨时区或夏令时切换写入的记录，其偏移不同，字典序不等于真实时刻序。例如 `2026-05-29T08:00:00+08:00`（=00:00 UTC）字典序大于 `2026-05-29T07:00:00+00:00`（=07:00 UTC），但前者实际更早。

2. **分组语义绑定写入时区**：`substr` 取的是写入时刻所在时区的本地日期。该值随记录冻结，但前端 `todayKey()` 用浏览器**当前**本地日期比较。用户换时区或夏令时切换后，「今天」高亮与旧记录的冻结日期可能错位；备份到异时区机器后时间线出现不一致。

## 2. 设计目标与非目标

### 目标

- 时间戳以 UTC 存储，保证字典序即时间序，排序在任何时区/夏令时下正确。
- 分组依据显式化为独立的 `local_date` 列（写入时刻的本地民用日期 `YYYY-MM-DD`），语义稳定（「我哪天复制的」不随后续时区变化而改变）。
- 存量数据全量迁移：回填 `local_date`，旧时间戳转 UTC。
- 迁移幂等，可安全多次执行（每次启动都会调用）。

### 非目标

- 不引入用户可配置时区。本地民用日期由系统时区决定。
- 不改动命令层签名、`ClipboardItem` 序列化字段、前端代码。
- 不处理图片/文件剪贴板（仍属后续路线图）。

## 3. 数据模型变更

`clipboard_items` 新增列：

```sql
ALTER TABLE clipboard_items ADD COLUMN local_date TEXT;
```

- `local_date`：写入时刻的本地民用日期，格式 `YYYY-MM-DD`。所有按日期的分组、查询、清理、软删除均依据此列。
- `created_at` / `last_copied_at`：改存 UTC，格式 `YYYY-MM-DDTHH:MM:SSZ`（无小数秒，末尾 `Z`）。
- `deleted_at`：随之转为同一 UTC 格式（仅为格式一致；不参与分组/排序，仅做 `IS NULL` 判定）。

索引调整：

```sql
CREATE INDEX IF NOT EXISTS idx_clipboard_items_local_date_active
  ON clipboard_items(local_date)
  WHERE deleted_at IS NULL;
```

删除旧索引 `idx_clipboard_items_created_at_active`（`created_at` 不再用于过滤，仅在按日期子集内由 `last_copied_at` 排序）。

## 4. 迁移设计

新增 `repository::migrate_schema(connection: &Connection) -> Result<(), ClipboardError>`，在 `init_schema` 之后由 `ClipboardService::new` 对两个连接分别调用一次。

### 4.1 幂等判定

通过 `PRAGMA table_info(clipboard_items)` 检查 `local_date` 列是否存在。存在即视为已迁移，直接返回。

> 说明：`settings` 连接对应的库可能没有 `clipboard_items` 表（仅存 settings）。`init_schema` 已对所有连接建 `clipboard_items` 表（`CREATE TABLE IF NOT EXISTS`），故 `table_info` 总能查到该表；空表迁移也安全（UPDATE 影响 0 行）。

### 4.2 迁移步骤（列不存在时）

1. `ALTER TABLE clipboard_items ADD COLUMN local_date TEXT;`
2. 回填与时间戳转换（单条 UPDATE）：

```sql
UPDATE clipboard_items
SET local_date = substr(created_at, 1, 10),
    created_at = strftime('%Y-%m-%dT%H:%M:%SZ', created_at),
    last_copied_at = strftime('%Y-%m-%dT%H:%M:%SZ', last_copied_at),
    deleted_at = CASE
        WHEN deleted_at IS NOT NULL
        THEN strftime('%Y-%m-%dT%H:%M:%SZ', deleted_at)
        ELSE NULL
    END;
```

3. 建新索引 `idx_clipboard_items_local_date_active`。
4. 删除旧索引：`DROP INDEX IF EXISTS idx_clipboard_items_created_at_active;`

> `local_date = substr(created_at, 1, 10)` 用的是**转换前**的本地日期，保留既有分组归属。SQLite UPDATE 的 SET 表达式基于行的原始值求值，故同一语句内 `substr(created_at,...)` 取到的是旧值，正确。
>
> `strftime('%Y-%m-%dT%H:%M:%SZ', created_at)`：SQLite 将带 `+HH:MM` 偏移的 ISO8601 字符串解释为需转 UTC，输出 UTC。例 `2026-05-29T08:53:00+08:00` → `2026-05-29T00:53:00Z`。
>
> **小数秒**：`chrono::to_rfc3339()` 会带纳秒小数（如 `2026-05-29T08:53:00.123456789+08:00`），故存量 `created_at` 多含小数秒。SQLite 的时间函数把小数秒当实数解析，`strftime('%...SZ', ...)` 输出无小数秒的整秒 UTC，正好与新 `now_iso()` 的格式（§5，亦无小数秒）统一。迁移测试需覆盖含小数秒的旧值。

### 4.3 settings 表

settings 的 `updated_at` 不迁移历史值（不参与任何比较/展示），新写入随 §5 改为 UTC 即可。

## 5. 时间戳助手变更（service_runtime.rs）

```rust
use chrono::Utc;

pub fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn today_local() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}
```

`settings.rs` 中两处 `Local::now().to_rfc3339()`（`update_stored_settings`、`apply_retention_policy` 的 `now`）改用 `service_runtime::now_iso()`，统一格式。

## 6. 写入路径变更

`repository::insert_text_item` 与 `upsert_text_item` 增加 `local_date: &str` 参数：

```rust
pub fn upsert_text_item(
    connection: &Connection,
    content: &str,
    content_hash: &str,
    now: &str,
    local_date: &str,
) -> Result<ClipboardItem, ClipboardError>
```

`INSERT` 写入 `local_date` 列。重复命中走 `update_existing_item`（仅更新 `last_copied_at` / `copy_count`，不改 `local_date`，保持首次捕获日期）。

`ClipboardService::capture_current_clipboard` 调用处传入 `now_iso()` 与 `today_local()`。

## 7. 查询/清理路径变更（repository.rs）

全部 `substr(created_at, 1, 10)` → `local_date`：

- `list_date_groups`：`SELECT local_date AS date, COUNT(*) AS count ... GROUP BY local_date ORDER BY local_date DESC`
- `list_items_by_date`：`WHERE deleted_at IS NULL AND local_date = ?1 ORDER BY last_copied_at DESC, id DESC`
- `soft_delete_items_by_date`：`WHERE local_date = ?2 AND deleted_at IS NULL`
- `cleanup_by_date`：`WHERE deleted_at IS NULL AND local_date < ?2`

`list_items_by_date` / `search_items` 的 `ORDER BY last_copied_at DESC` 在 UTC 格式下即正确时间序，无需改写排序表达式。

## 8. 保留 cutoff（settings.rs）

`apply_retention_policy` 中 cutoff 仍按本地日期计算：

```rust
let cutoff = Local::now() - Duration::days(settings.retention_days);
let cutoff_date = cutoff.format("%Y-%m-%d").to_string();
```

`cutoff_date` 与 `local_date` 同为本地民用日期，`cleanup_by_date` 的 `local_date < cutoff_date` 比较语义一致。`now` 改用 `now_iso()`（UTC）。

## 9. 前端

零改动，作为设计约束验证：

- `ClipboardItem.createdAt` 变为 UTC `Z` 串。`new Date(createdAt)`（ItemListPanel / DetailPanel / ClipStudioDetailDialog）按 UTC 解析后经 `Intl` 转浏览器本地显示，结果正确。
- `ClipboardDateGroup.date` 现来自 `local_date`，仍是 `YYYY-MM-DD`。
- `todayKey()`（浏览器当前本地日期）与 `local_date` 同为本地民用日期，`group.date === today` 比较照旧。

## 10. 测试计划

后端新增/更新（`repository_tests.rs` / `service_tests.rs`）：

1. **迁移回填**：构造旧格式行（`created_at` 带 `+08:00` 偏移**且含纳秒小数**、无 `local_date`），跑 `migrate_schema`，断言 `local_date` = 原本地日期、`created_at` 形如 `YYYY-MM-DDTHH:MM:SSZ`（无小数秒、末尾 `Z`）且时刻已转 UTC。
2. **迁移幂等**：连续两次 `migrate_schema` 不报错、不重复改值。
3. **分组基于 local_date**：插入跨日期记录，`list_date_groups` 按 `local_date` 正确分组。
4. **排序回归**：插入两条 `last_copied_at` 为 UTC、跨偏移来源的记录，断言 `list_items_by_date` 返回顺序为真实时刻倒序（旧实现会错）。
5. **既有用例签名更新**：所有 `upsert_text_item` / `insert_text_item` 调用补 `local_date` 入参。

## 11. 改动文件清单

- `src-tauri/src/clipboard/service_runtime.rs`：`now_iso` 改 UTC、新增 `today_local`
- `src-tauri/src/clipboard/repository.rs`：`init_schema` 加 local_date 列与新索引、新增 `migrate_schema`、insert/upsert 加参、四处查询改 `local_date`
- `src-tauri/src/clipboard/service.rs`：`new` 调 `migrate_schema`、capture 传 `today_local()`
- `src-tauri/src/clipboard/settings.rs`：两处时间戳改 `now_iso()`
- `src-tauri/src/clipboard/repository_tests.rs` / `service_tests.rs`：新增测试 + 更新调用签名
- `docs/clipboard-toolbox-design.md`：§7.1 schema 补 `local_date`、§5.2 / §11 同步说明
- `docs/2026-05-28-clipboard-toolbox-audit.md`：标记 P0 #5 已修复

## 12. 验证

- `cd src-tauri; cargo test clipboard` 全量通过（含新增迁移/排序用例）
- `cd src-tauri; cargo check` 无新警告
- `pnpm.cmd build` 前端构建通过
- 手动：保留一份旧格式 DB 启动应用，确认旧记录仍按原日期分组、排序正确、不重复迁移
