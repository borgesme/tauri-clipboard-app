# 清空当日可撤销 + 物理清理二次确认 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让「清空当日」可在 6 秒内一键撤销，并为「清理已删除记录」（物理删）加原生二次确认。

**Architecture:** 后端 `soft_delete_items_by_date` 改用 `RETURNING id` 回传被软删的 id 列表，新增 `restore_items` 按 id 把 `deleted_at` 置回 `NULL`；命令层新增 `restore_clipboard_items`（不 emit 事件）。前端新增 `useUndoToast` hook（计时 + 可见性）与 `UndoToast` 浮层组件，`useClipboardWorkspace` 接线清空→toast→撤销闭环；purge 在按钮组件内先 `confirm` 再执行。

**Tech Stack:** Rust + rusqlite 0.32（`RETURNING` / `params_from_iter`）、Tauri 2、React 19 + TypeScript、Vitest 4（globals + jsdom）、`@tauri-apps/plugin-dialog`。

**基线说明：** 本计划从 commit `f8b8874`（spec 与旧 plan 已在库内、尚未应用任何代码改动）出发。spec 文件 `docs/superpowers/specs/2026-05-30-clear-undo-purge-confirm-design.md` **保持不变**，本次只重写本 plan 并按其落地。

**spec 内部矛盾的处理（重要）：** spec §4.5 与 §7 要求改 `SettingsAdvancedActions.tsx` 的 `MaintenanceAction`，但 §6 测试计划第 14 条写「`DesktopSettingsPanel.test.tsx` 补」。purge 确认逻辑放在 `MaintenanceAction` 内，对应测试随之放到**新建** `SettingsAdvancedActions.test.tsx`（测试跟随被测组件，而非 spec 笔误指向的 `DesktopSettingsPanel.test.tsx`）。**用户已明确选择「严格按 spec」改 `SettingsAdvancedActions.tsx`。** 但当前实际渲染的设置面板是 `DesktopSettingsPanel.tsx`（其「清理」按钮 `DesktopSettingsPanel.tsx:61-65` 直接调 `onPurgeDeletedItems`），而 `MaintenanceAction` 在代码库中**未被任何地方引用（dead code）**。因此 Task 11 完成后单测会绿、但 `pnpm tauri dev` 里点「清理」**不会**弹确认框——这是严格遵循 spec 的已知后果，已在 Task 12 的 GUI 验证里标注。

---

## 文件结构

**后端（`src-tauri/src/clipboard/`）**
- `repository.rs` — `soft_delete_items_by_date` 改 `RETURNING id` → `Vec<i64>`；新增 `restore_items`；补 `params_from_iter`
- `service.rs` — `clear_items_by_date` 透传 `Vec<i64>`；新增 `restore_items`
- `commands.rs` — `clear_clipboard_items_by_date` 回 `Vec<i64>`；新增 `restore_clipboard_items`
- `repository_tests.rs` / `service_tests.rs` — 新增用例
- `src-tauri/src/lib.rs` — 注册 `restore_clipboard_items`

**前端（`src/`）**
- `api/clipboard.ts` — `clearClipboardItemsByDate` → `Promise<number[]>`；新增 `restoreClipboardItems`
- `hooks/useUndoToast.ts`（新增）+ `hooks/useUndoToast.test.tsx`（新增）
- `components/clipboard/UndoToast.tsx`（新增）+ `UndoToast.test.tsx`（新增）
- `hooks/useClipboardWorkspace.ts` + `useClipboardWorkspace.test.tsx` — 接线 undo
- `components/clipboard/ClipStudioPage.tsx` — 渲染 `UndoToast`
- `App.css` — `.undo-toast` 样式
- `components/clipboard/SettingsAdvancedActions.tsx` + `SettingsAdvancedActions.test.tsx`（新增）— purge 二次确认

**文档**
- `docs/2026-05-28-clipboard-toolbox-audit.md` — 标记 #14 已修复

---

## 测试约定（务必遵守，旧 plan 在此踩坑）

- **后端 `repository_tests.rs`**：`temp_database_path(name: &str)` **带参**；函数从 `use super::repository::{...}` 引入，调用**不带** `repository::` 前缀；`open_connection` 从 `use super::service_runtime::open_connection` 引入；`upsert_text_item(...).unwrap()` 返回 **`ClipboardItem`**，取 id 用 `item.id`（不是 i64）。
- **后端 `service_tests.rs`**：`use super::repository;` 已在顶部，故 `repository::xxx` **有效**；`temp_database_path(name: &str)` **带参**；`ClipboardService::new(default_path)` 接 **owned `PathBuf`**（需复用就 `.clone()`）；开连接用 `super::service_runtime::open_connection(&path)`。
- **前端**：Vitest `globals: true`——测试文件**不要** `import { ... } from "vitest"`，直接用 `vi` / `describe` / `it` / `expect` / `beforeEach` / `afterEach`。涉及 DOM 的测试文件首行加 `// @vitest-environment jsdom`。
- **CSS**：复用既有 `var(--clip-*)` 变量（`--clip-surface` / `--clip-border-strong` / `--clip-fg` / `--clip-accent` / `--clip-muted` 等），**不要**臆造 `hsl(var(--background))` 之类。

**命令（仓库根目录，PowerShell）**
- 后端单测：`cargo test --manifest-path src-tauri/Cargo.toml <name> -- --nocapture`
- 后端全量 + 检查：`cargo test --manifest-path src-tauri/Cargo.toml` / `cargo check --manifest-path src-tauri/Cargo.toml`
- 前端单测（按文件名过滤）：`pnpm.cmd test <pattern>`
- 前端类型检查：`pnpm.cmd exec tsc --noEmit`
- 前端全量构建：`pnpm.cmd build`

---

## Task 1: `soft_delete_items_by_date` 返回 `Vec<i64>`

> 改返回类型会同时影响 `service.rs::clear_items_by_date`（透传）和既有测试 `soft_deletes_all_items_by_date` 的断言。三处必须同一 commit 改完，否则 crate 编不过。`commands.rs` 用 `?` 丢弃返回值，本任务**无需**改它。

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs:237-249`
- Modify: `src-tauri/src/clipboard/service.rs:137-140`
- Test: `src-tauri/src/clipboard/repository_tests.rs`（改既有断言 + 新增 1 例）

- [ ] **Step 1: 新增失败测试 `clear_returns_soft_deleted_ids`**

在 `src-tauri/src/clipboard/repository_tests.rs` 末尾追加：

```rust
#[test]
fn clear_returns_soft_deleted_ids() {
    let path = temp_database_path("clear-returns-ids");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let one = upsert_text_item(&conn, "one", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    let two = upsert_text_item(&conn, "two", "hash-2", "2026-05-26T11:00:00+08:00", "2026-05-26").unwrap();
    let already = upsert_text_item(&conn, "gone", "hash-3", "2026-05-26T09:00:00+08:00", "2026-05-26").unwrap();
    soft_delete_item(&conn, already.id, "2026-05-26T09:30:00+08:00").unwrap();

    let mut ids = soft_delete_items_by_date(&conn, "2026-05-26", "2026-05-26T12:00:00+08:00").unwrap();
    ids.sort();
    let mut expected = vec![one.id, two.id];
    expected.sort();

    assert_eq!(expected, ids);
    assert!(!ids.contains(&already.id));
}
```

- [ ] **Step 2: 运行确认编译失败（红）**

Run: `cargo test --manifest-path src-tauri/Cargo.toml clear_returns_soft_deleted_ids`
Expected: 编译失败——`soft_delete_items_by_date` 现返回 `usize`，`.sort()` / `.contains` 在整数上不存在。

- [ ] **Step 3: 改 `soft_delete_items_by_date` 返回 `Vec<i64>`**

把 `src-tauri/src/clipboard/repository.rs:237-249` 整个函数替换为：

```rust
pub fn soft_delete_items_by_date(
    connection: &Connection,
    date: &str,
    now: &str,
) -> Result<Vec<i64>, ClipboardError> {
    let mut statement = connection.prepare(
        "UPDATE clipboard_items
         SET deleted_at = ?1
         WHERE local_date = ?2 AND deleted_at IS NULL
         RETURNING id",
    )?;
    let ids = statement
        .query_map(params![now, date], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}
```

- [ ] **Step 4: 透传 service 层 `clear_items_by_date` 的返回类型**

把 `src-tauri/src/clipboard/service.rs:137-140` 替换为（仅返回类型 `usize` → `Vec<i64>`）：

```rust
    pub fn clear_items_by_date(&self, date: &str) -> Result<Vec<i64>, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::soft_delete_items_by_date(&conn, date, &now_iso())
    }
```

- [ ] **Step 5: 修既有测试 `soft_deletes_all_items_by_date` 的断言**

`src-tauri/src/clipboard/repository_tests.rs:113` 把 `assert_eq!(2, changed);` 改为：

```rust
    assert_eq!(2, changed.len());
```

- [ ] **Step 6: 运行验证通过（绿）**

Run: `cargo test --manifest-path src-tauri/Cargo.toml soft_delete -- --nocapture` 再跑 `cargo test --manifest-path src-tauri/Cargo.toml clear_returns_soft_deleted_ids`
Expected: `clear_returns_soft_deleted_ids`、`soft_deletes_all_items_by_date` 均 PASS，crate 编译通过。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/service.rs src-tauri/src/clipboard/repository_tests.rs
git commit -m "feat(clipboard): soft_delete_items_by_date 返回被删 id 列表"
```

---

## Task 2: 新增 `restore_items`（repository 层）

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs:1`（补 `params_from_iter`）
- Modify: `src-tauri/src/clipboard/repository.rs`（新增 `restore_items`）
- Test: `src-tauri/src/clipboard/repository_tests.rs:3-7`（import 补 `restore_items`）+ 新增 4 例

- [ ] **Step 1: 新增 4 个失败测试 + 补 import**

先把 `src-tauri/src/clipboard/repository_tests.rs:3-7` 的 import 改为（加 `restore_items`）：

```rust
use super::repository::{
    cleanup_items, get_i64_setting, get_item_by_id, get_string_setting, init_schema,
    list_date_groups, list_items_by_date, migrate_schema, restore_items, search_items, set_setting,
    soft_delete_item, soft_delete_items_by_date, upsert_text_item,
};
```

再在文件末尾追加：

```rust
#[test]
fn restore_items_clears_deleted_at() {
    let path = temp_database_path("restore-clears");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    upsert_text_item(&conn, "alpha note", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    upsert_text_item(&conn, "beta note", "hash-2", "2026-05-26T11:00:00+08:00", "2026-05-26").unwrap();
    let ids = soft_delete_items_by_date(&conn, "2026-05-26", "2026-05-26T12:00:00+08:00").unwrap();
    assert!(list_items_by_date(&conn, "2026-05-26").unwrap().is_empty());

    let restored = restore_items(&conn, &ids).unwrap();

    assert_eq!(2, restored);
    assert_eq!(2, list_items_by_date(&conn, "2026-05-26").unwrap().len());
    assert_eq!(1, search_items(&conn, "alpha").unwrap().len());
    assert_eq!(1, search_items(&conn, "beta").unwrap().len());
}

#[test]
fn restore_items_empty_returns_zero() {
    let path = temp_database_path("restore-empty");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let restored = restore_items(&conn, &[]).unwrap();

    assert_eq!(0, restored);
}

#[test]
fn restore_items_ignores_already_active() {
    let path = temp_database_path("restore-active");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let item = upsert_text_item(&conn, "active", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();

    let restored = restore_items(&conn, &[item.id]).unwrap();

    assert_eq!(0, restored);
    assert_eq!(1, list_items_by_date(&conn, "2026-05-26").unwrap().len());
}

#[test]
fn restore_items_does_not_touch_other_deleted() {
    let path = temp_database_path("restore-isolated");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    // A 批：手动清空当日（软删）
    upsert_text_item(&conn, "batch-a", "hash-a", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    let batch_a = soft_delete_items_by_date(&conn, "2026-05-26", "2026-05-26T12:00:00+08:00").unwrap();
    // B 批：另一天单条软删（模拟 retention / 单删）
    let b = upsert_text_item(&conn, "batch-b", "hash-b", "2026-05-27T10:00:00+08:00", "2026-05-27").unwrap();
    soft_delete_item(&conn, b.id, "2026-05-27T12:00:00+08:00").unwrap();

    let restored = restore_items(&conn, &batch_a).unwrap();

    assert_eq!(1, restored);
    assert_eq!(1, list_items_by_date(&conn, "2026-05-26").unwrap().len());
    assert!(list_items_by_date(&conn, "2026-05-27").unwrap().is_empty());
}
```

- [ ] **Step 2: 运行确认失败（红）**

Run: `cargo test --manifest-path src-tauri/Cargo.toml restore_items`
Expected: 编译失败——`restore_items` 尚未定义（import 解析不到）。

- [ ] **Step 3: 补 `params_from_iter` 导入**

`src-tauri/src/clipboard/repository.rs:1` 改为：

```rust
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
```

- [ ] **Step 4: 实现 `restore_items`**

在 `src-tauri/src/clipboard/repository.rs` 的 `soft_delete_items_by_date` 函数之后插入：

```rust
pub fn restore_items(connection: &Connection, ids: &[i64]) -> Result<usize, ClipboardError> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE clipboard_items SET deleted_at = NULL
         WHERE id IN ({placeholders}) AND deleted_at IS NOT NULL"
    );
    let changed = connection.execute(&sql, params_from_iter(ids.iter()))?;
    Ok(changed)
}
```

- [ ] **Step 5: 运行验证通过（绿）**

Run: `cargo test --manifest-path src-tauri/Cargo.toml restore_items -- --nocapture`
Expected: `restore_items_clears_deleted_at`、`restore_items_empty_returns_zero`、`restore_items_ignores_already_active`、`restore_items_does_not_touch_other_deleted` 全 PASS。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/repository_tests.rs
git commit -m "feat(clipboard): 新增 restore_items 按 id 恢复软删记录"
```

---

## Task 3: service 层新增 `restore_items` + service 测试

**Files:**
- Modify: `src-tauri/src/clipboard/service.rs`（在 `clear_items_by_date` 之后新增 `restore_items`）
- Test: `src-tauri/src/clipboard/service_tests.rs`（新增 1 例）

- [ ] **Step 1: 新增失败测试**

在 `src-tauri/src/clipboard/service_tests.rs` 末尾追加：

```rust
#[test]
fn clear_returns_ids_and_restore_brings_them_back() {
    let default_path = temp_database_path("clear-restore");
    let service = ClipboardService::new(default_path.clone()).unwrap();
    {
        let conn = super::service_runtime::open_connection(&default_path).unwrap();
        repository::upsert_text_item(&conn, "alpha", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
        repository::upsert_text_item(&conn, "beta", "hash-2", "2026-05-26T11:00:00+08:00", "2026-05-26").unwrap();
    }

    let ids = service.clear_items_by_date("2026-05-26").unwrap();
    assert_eq!(2, ids.len());
    assert!(service.list_items_by_date("2026-05-26").unwrap().is_empty());

    let restored = service.restore_items(&ids).unwrap();
    assert_eq!(2, restored);
    assert_eq!(2, service.list_items_by_date("2026-05-26").unwrap().len());
}
```

- [ ] **Step 2: 运行确认失败（红）**

Run: `cargo test --manifest-path src-tauri/Cargo.toml clear_returns_ids_and_restore_brings_them_back`
Expected: 编译失败——`service.restore_items` 尚未定义。

- [ ] **Step 3: 实现 service `restore_items`**

在 `src-tauri/src/clipboard/service.rs` 的 `clear_items_by_date`（约 137-140 行）之后插入：

```rust
    pub fn restore_items(&self, ids: &[i64]) -> Result<usize, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::restore_items(&conn, ids)
    }
```

- [ ] **Step 4: 运行验证通过（绿）**

Run: `cargo test --manifest-path src-tauri/Cargo.toml clear_returns_ids_and_restore_brings_them_back -- --nocapture`
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/clipboard/service.rs src-tauri/src/clipboard/service_tests.rs
git commit -m "feat(clipboard): service 层透传 clear ids 并新增 restore_items"
```

---

## Task 4: commands 层 + lib 注册

**Files:**
- Modify: `src-tauri/src/clipboard/commands.rs:68-77`（`clear_clipboard_items_by_date` 回 `Vec<i64>`）+ 新增 `restore_clipboard_items`
- Modify: `src-tauri/src/lib.rs:3-9`（import）、`src-tauri/src/lib.rs:53-69`（generate_handler）

> commands 层无单测；正确性由 `cargo check` + 后续前端集成保证。`restore_clipboard_items` 按 spec §3.3 **不 emit 事件**（单主窗口，发起方自行刷新）。

- [ ] **Step 1: 改 `clear_clipboard_items_by_date` 返回 ids，并新增 `restore_clipboard_items`**

把 `src-tauri/src/clipboard/commands.rs:68-77` 替换为：

```rust
#[tauri::command]
pub fn clear_clipboard_items_by_date(
    app_handle: AppHandle,
    date: String,
    state: State<'_, ClipboardState>,
) -> Result<Vec<i64>, ClipboardError> {
    let ids = state.0.clear_items_by_date(&date)?;
    emit_deleted(&app_handle, None, Some(date));
    Ok(ids)
}

#[tauri::command]
pub fn restore_clipboard_items(
    ids: Vec<i64>,
    state: State<'_, ClipboardState>,
) -> Result<usize, ClipboardError> {
    state.0.restore_items(&ids)
}
```

- [ ] **Step 2: lib.rs 导入 `restore_clipboard_items`**

把 `src-tauri/src/lib.rs:3-9` 的 use 块替换为（在 `purge_deleted_clipboard_items,` 后加 `restore_clipboard_items,`）：

```rust
use clipboard::commands::{
    clear_clipboard_items_by_date, copy_clipboard_item, delete_clipboard_item, get_clipboard_item,
    get_clipboard_monitor_status, get_desktop_settings, hide_main_window, list_clipboard_dates,
    list_clipboard_items, purge_deleted_clipboard_items, restore_clipboard_items,
    search_clipboard_items, set_clipboard_monitor_enabled, show_main_window, update_desktop_settings,
    validate_storage_dir, ClipboardState,
};
```

- [ ] **Step 3: lib.rs 注册到 generate_handler**

`src-tauri/src/lib.rs:61` 在 `purge_deleted_clipboard_items,` 之后插入一行，使该段成为：

```rust
            purge_deleted_clipboard_items,
            restore_clipboard_items,
```

- [ ] **Step 4: 运行验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 通过、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/clipboard/commands.rs src-tauri/src/lib.rs
git commit -m "feat(clipboard): 命令层暴露 clear ids 与 restore_clipboard_items"
```

---

## Task 5: API 层

**Files:**
- Modify: `src/api/clipboard.ts:40-42`（返回类型）+ 新增 `restoreClipboardItems`

- [ ] **Step 1: 改 `clearClipboardItemsByDate` 返回类型并新增 `restoreClipboardItems`**

把 `src/api/clipboard.ts:40-42` 替换为：

```ts
export function clearClipboardItemsByDate(date: string): Promise<number[]> {
  return invoke("clear_clipboard_items_by_date", { date });
}

export function restoreClipboardItems(ids: number[]): Promise<number> {
  return invoke("restore_clipboard_items", { ids });
}
```

- [ ] **Step 2: 运行类型检查**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 通过。（既有 `createClearDate` 用 `await clearClipboardItemsByDate(...)` 但不消费返回值，类型变化不报错。）

- [ ] **Step 3: Commit**

```bash
git add src/api/clipboard.ts
git commit -m "feat(clipboard): API 层 clear 返回 number[] 并新增 restoreClipboardItems"
```

---

## Task 6: `useUndoToast` hook

**Files:**
- Create: `src/hooks/useUndoToast.ts`
- Test: `src/hooks/useUndoToast.test.tsx`

- [ ] **Step 1: 写失败测试**

创建 `src/hooks/useUndoToast.test.tsx`：

```tsx
// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";

import { useUndoToast } from "@/hooks/useUndoToast";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useUndoToast", () => {
  it("exposes pending state after show", () => {
    const { result } = renderHook(() => useUndoToast(6000));

    act(() => result.current.show({ ids: [1, 2], date: "2026-05-29", count: 2 }));

    expect(result.current.pending).toEqual({ ids: [1, 2], date: "2026-05-29", count: 2 });
  });

  it("auto-dismisses pending after durationMs", () => {
    const { result } = renderHook(() => useUndoToast(6000));

    act(() => result.current.show({ ids: [1], date: "2026-05-29", count: 1 }));
    act(() => vi.advanceTimersByTime(6000));

    expect(result.current.pending).toBeNull();
  });

  it("replaces pending and resets the timer on a second show", () => {
    const { result } = renderHook(() => useUndoToast(6000));

    act(() => result.current.show({ ids: [1], date: "2026-05-29", count: 1 }));
    act(() => vi.advanceTimersByTime(4000));
    act(() => result.current.show({ ids: [2, 3], date: "2026-05-28", count: 2 }));

    // 再过 4s（旧批次本应在此前消失）：计时被重置，pending 仍是新批次
    act(() => vi.advanceTimersByTime(4000));
    expect(result.current.pending).toEqual({ ids: [2, 3], date: "2026-05-28", count: 2 });

    // 新批次满 6s 后才归 null
    act(() => vi.advanceTimersByTime(2000));
    expect(result.current.pending).toBeNull();
  });

  it("clears pending immediately and stops the timer", () => {
    const { result } = renderHook(() => useUndoToast(6000));

    act(() => result.current.show({ ids: [1], date: "2026-05-29", count: 1 }));
    act(() => result.current.clear());

    expect(result.current.pending).toBeNull();
    act(() => vi.advanceTimersByTime(6000));
    expect(result.current.pending).toBeNull();
  });
});
```

- [ ] **Step 2: 运行确认失败（红）**

Run: `pnpm.cmd test useUndoToast`
Expected: FAIL——`@/hooks/useUndoToast` 模块不存在。

- [ ] **Step 3: 实现 hook**

创建 `src/hooks/useUndoToast.ts`（`timerRef` 用显式 `undefined` 初值，兼容各版本 `@types/react`，行为与 spec §4.2 一致）：

```ts
import { useCallback, useEffect, useRef, useState } from "react";

export interface UndoState {
  ids: number[];
  date: string;
  count: number;
}

export function useUndoToast(durationMs = 6000) {
  const [pending, setPending] = useState<UndoState | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const show = useCallback(
    (next: UndoState) => {
      clearTimeout(timerRef.current);
      setPending(next);
      timerRef.current = setTimeout(() => setPending(null), durationMs);
    },
    [durationMs],
  );

  const clear = useCallback(() => {
    clearTimeout(timerRef.current);
    setPending(null);
  }, []);

  useEffect(() => () => clearTimeout(timerRef.current), []);

  return { pending, show, clear };
}
```

- [ ] **Step 4: 运行验证通过（绿）**

Run: `pnpm.cmd test useUndoToast`
Expected: 4 个用例全 PASS。

- [ ] **Step 5: Commit**

```bash
git add src/hooks/useUndoToast.ts src/hooks/useUndoToast.test.tsx
git commit -m "feat(clipboard): 新增 useUndoToast hook（计时 + 可见性）"
```

---

## Task 7: `UndoToast` 组件

**Files:**
- Create: `src/components/clipboard/UndoToast.tsx`
- Test: `src/components/clipboard/UndoToast.test.tsx`

- [ ] **Step 1: 写失败测试**

创建 `src/components/clipboard/UndoToast.test.tsx`：

```tsx
// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";

import { UndoToast } from "@/components/clipboard/UndoToast";

describe("UndoToast", () => {
  it("renders nothing when pending is null", () => {
    const { container } = render(
      <UndoToast pending={null} onUndo={() => {}} onDismiss={() => {}} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders the count and fires onUndo / onDismiss", () => {
    const onUndo = vi.fn();
    const onDismiss = vi.fn();
    render(
      <UndoToast
        pending={{ ids: [1, 2, 3], date: "2026-05-29", count: 3 }}
        onUndo={onUndo}
        onDismiss={onDismiss}
      />,
    );

    expect(screen.getByText(/已清空 3 条记录/)).toBeTruthy();

    fireEvent.click(screen.getByText("撤销"));
    expect(onUndo).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByLabelText("关闭"));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 2: 运行确认失败（红）**

Run: `pnpm.cmd test UndoToast`
Expected: FAIL——`@/components/clipboard/UndoToast` 不存在。

- [ ] **Step 3: 实现组件**

创建 `src/components/clipboard/UndoToast.tsx`：

```tsx
import type { UndoState } from "@/hooks/useUndoToast";

export function UndoToast({
  pending,
  onUndo,
  onDismiss,
}: {
  pending: UndoState | null;
  onUndo: () => void;
  onDismiss: () => void;
}) {
  if (!pending) {
    return null;
  }
  return (
    <div className="undo-toast" role="status">
      <span>已清空 {pending.count} 条记录</span>
      <button type="button" onClick={onUndo}>
        撤销
      </button>
      <button type="button" aria-label="关闭" onClick={onDismiss}>
        ×
      </button>
    </div>
  );
}
```

- [ ] **Step 4: 运行验证通过（绿）**

Run: `pnpm.cmd test UndoToast`
Expected: 2 个用例全 PASS。

- [ ] **Step 5: Commit**

```bash
git add src/components/clipboard/UndoToast.tsx src/components/clipboard/UndoToast.test.tsx
git commit -m "feat(clipboard): 新增 UndoToast 浮层组件"
```

---

## Task 8: `useClipboardWorkspace` 接线 undo

**Files:**
- Modify: `src/hooks/useClipboardWorkspace.ts`（import、实例化、`ClearDateOptions`、`createClearDate`、return、新增 `createUndoClear`）
- Test: `src/hooks/useClipboardWorkspace.test.tsx`（改 `setupInvoke` + 新增 undo describe）

- [ ] **Step 1: 写失败测试 + 调整 `setupInvoke`**

先改 `src/hooks/useClipboardWorkspace.test.tsx:90-95`，把 `clear_clipboard_items_by_date` 从合并分支里拆出来返回空数组、并补 `restore_clipboard_items`，使该段成为：

```tsx
      case "clear_clipboard_items_by_date":
        return Promise.resolve([]);
      case "restore_clipboard_items":
        return Promise.resolve(0);
      case "delete_clipboard_item":
      case "copy_clipboard_item":
        return Promise.resolve();
      case "purge_deleted_clipboard_items":
        return Promise.resolve(0);
```

再在文件末尾追加：

```tsx
describe("useClipboardWorkspace undo", () => {
  it("shows undoState after clearing a non-empty date", async () => {
    setupInvoke({ clear_clipboard_items_by_date: () => [11, 22] });
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));

    await act(async () => {
      await result.current.clearDate();
    });

    expect(result.current.undoState).toEqual({
      ids: [11, 22],
      date: result.current.selectedDate,
      count: 2,
    });
  });

  it("keeps undoState null when the cleared date has no items", async () => {
    setupInvoke({ clear_clipboard_items_by_date: () => [] });
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));

    await act(async () => {
      await result.current.clearDate();
    });

    expect(result.current.undoState).toBeNull();
  });

  it("restores items and clears undoState on undoClear", async () => {
    setupInvoke({ clear_clipboard_items_by_date: () => [11, 22] });
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));

    await act(async () => {
      await result.current.clearDate();
    });
    await act(async () => {
      await result.current.undoClear();
    });

    expect(countCalls("restore_clipboard_items")).toBe(1);
    const call = invoke.mock.calls.find(([name]) => name === "restore_clipboard_items");
    expect(call?.[1]).toEqual({ ids: [11, 22] });
    expect(result.current.undoState).toBeNull();
  });
});
```

- [ ] **Step 2: 运行确认失败（红）**

Run: `pnpm.cmd test useClipboardWorkspace`
Expected: 新增 3 例 FAIL——`undoState` / `undoClear` 尚未暴露（`undefined`）。

- [ ] **Step 3: 接线 hook —— import + 实例化**

`src/hooks/useClipboardWorkspace.ts:3-16` 的 import 块，在 `purgeDeletedClipboardItems,` 之后补 `restoreClipboardItems,`：

```ts
import {
  clearClipboardItemsByDate,
  copyClipboardItem,
  deleteClipboardItem,
  getClipboardMonitorStatus,
  getDesktopSettings,
  hideMainWindow,
  listClipboardDates,
  listClipboardItems,
  purgeDeletedClipboardItems,
  restoreClipboardItems,
  searchClipboardItems,
  setClipboardMonitorEnabled,
  updateDesktopSettings,
} from "@/api/clipboard";
```

在 `src/hooks/useClipboardWorkspace.ts:17`（`useClipboardEvents` 导入）之后新增一行：

```ts
import { useUndoToast, type UndoState } from "@/hooks/useUndoToast";
```

在 hook 体内 `src/hooks/useClipboardWorkspace.ts:36`（`const refreshView = useRefreshView(...)`）之后新增：

```ts
  const undo = useUndoToast();
```

- [ ] **Step 4: 改 return 对象**

把 `src/hooks/useClipboardWorkspace.ts:68`（`clearDate: createClearDate({ selectedDate, refreshView, setMessage }),`）替换为：

```ts
    clearDate: createClearDate({ selectedDate, refreshView, setMessage, undoShow: undo.show }),
    undoState: undo.pending,
    undoClear: createUndoClear({
      pending: undo.pending,
      dismissUndo: undo.clear,
      setSearchTerm,
      setSelectedDate,
      setLoadedItems,
      loadDates,
      setMessage,
    }),
    dismissUndo: undo.clear,
```

- [ ] **Step 5: 改 `ClearDateOptions` 与 `createClearDate`，新增 `createUndoClear`**

把 `src/hooks/useClipboardWorkspace.ts:197-209` 替换为：

```ts
interface ClearDateOptions {
  selectedDate: string;
  refreshView: () => Promise<void>;
  setMessage: (value: string) => void;
  undoShow: (next: UndoState) => void;
}

function createClearDate({ selectedDate, refreshView, setMessage, undoShow }: ClearDateOptions) {
  return async () => {
    const ids = await clearClipboardItemsByDate(selectedDate);
    setMessage(`已清空 ${selectedDate} 的剪贴板记录。`);
    await refreshView();
    if (ids.length > 0) {
      undoShow({ ids, date: selectedDate, count: ids.length });
    }
  };
}

interface UndoClearOptions {
  pending: UndoState | null;
  dismissUndo: () => void;
  setSearchTerm: (value: string) => void;
  setSelectedDate: (value: string) => void;
  setLoadedItems: (items: ClipboardItem[]) => void;
  loadDates: () => Promise<void>;
  setMessage: (value: string) => void;
}

function createUndoClear(options: UndoClearOptions) {
  return async () => {
    const pending = options.pending;
    if (!pending) {
      return;
    }
    await restoreClipboardItems(pending.ids);
    options.dismissUndo();
    options.setSearchTerm("");
    options.setSelectedDate(pending.date);
    options.setLoadedItems(await listClipboardItems(pending.date));
    await options.loadDates();
    options.setMessage(`已恢复 ${pending.count} 条记录。`);
  };
}
```

> 说明：`createUndoClear` 每次渲染随 return 重建，闭包内 `options.pending` 恒为最新；撤销按 `pending.date` 直接重载列表，规避 `refreshView` 闭包绑定旧 `selectedDate` 的时序问题（见 spec §4.3）。`setSelectedDate` / `setSearchTerm` / `setLoadedItems` / `loadDates` / `setMessage` 均已在 hook 体作用域内（分别为 `useState` 的 setter 与 `useCallback`）。

- [ ] **Step 6: 运行验证通过（绿）**

Run: `pnpm.cmd test useClipboardWorkspace`
Expected: 全部 PASS（含既有 clearDate 用例：默认 `setupInvoke` 返回 `[]` → 不弹 undo、message 仍含「已清空」）。

- [ ] **Step 7: 类型检查 + Commit**

Run: `pnpm.cmd exec tsc --noEmit`（Expected: 通过）

```bash
git add src/hooks/useClipboardWorkspace.ts src/hooks/useClipboardWorkspace.test.tsx
git commit -m "feat(clipboard): workspace 接线清空撤销闭环"
```

---

## Task 9: `ClipStudioPage` 渲染 `UndoToast`

**Files:**
- Modify: `src/components/clipboard/ClipStudioPage.tsx:1-16`（import）、`src/components/clipboard/ClipStudioPage.tsx:32-38`（return 包 Fragment）

> 该页面 return 的是 `<ClipStudioLayout>...</ClipStudioLayout>`（**无** `<div className="clip-studio">`，旧 plan 在此臆造了不存在的节点）。用 Fragment 包裹，把 toast 放在 layout 同级、顶层渲染。无独立单测；正确性由 tsc + Task 12 GUI 验证保证。

- [ ] **Step 1: 加 import**

在 `src/components/clipboard/ClipStudioPage.tsx:6`（`import { ClipStudioPanel } ...` 之后）新增：

```tsx
import { UndoToast } from "@/components/clipboard/UndoToast";
```

- [ ] **Step 2: return 包 Fragment 并挂 UndoToast**

把 `src/components/clipboard/ClipStudioPage.tsx:32-38` 替换为：

```tsx
  return (
    <>
      <ClipStudioLayout {...createLayoutProps(workspace, state)}>
        <ClipStudioList {...createListProps(workspace, state)} />
        <ClipStudioPanel {...createPanelProps(workspace, state)} />
        <ClipStudioDetailDialog {...createDialogProps(workspace, state)} />
      </ClipStudioLayout>
      <UndoToast
        pending={workspace.undoState}
        onUndo={() => void workspace.undoClear()}
        onDismiss={workspace.dismissUndo}
      />
    </>
  );
```

- [ ] **Step 3: 类型检查**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 通过（`workspace.undoState` / `undoClear` / `dismissUndo` 已由 Task 8 暴露）。

- [ ] **Step 4: Commit**

```bash
git add src/components/clipboard/ClipStudioPage.tsx
git commit -m "feat(clipboard): ClipStudioPage 顶层渲染 UndoToast"
```

---

## Task 10: `.undo-toast` 样式

**Files:**
- Modify: `src/App.css`（文件末尾追加）

> 复用既有 `var(--clip-*)` 变量；右下角固定、`z-index: 60` 高于 `.detail-backdrop`（30）与 `.clip-panel`（25）。旧 plan 用了项目里不存在的 `hsl(var(--background))` 等变量，已纠正。

- [ ] **Step 1: 追加样式**

在 `src/App.css` 末尾追加：

```css
.undo-toast{position:fixed;right:24px;bottom:24px;z-index:60;display:flex;align-items:center;gap:12px;border:1px solid var(--clip-border-strong);border-radius:14px;background:var(--clip-surface);color:var(--clip-fg);box-shadow:0 12px 32px rgba(20,20,19,0.22);padding:12px 16px;font-size:13px;}
.undo-toast span{color:var(--clip-fg);}
.undo-toast button{border:0;background:none;cursor:pointer;font-size:13px;color:var(--clip-accent);padding:2px 4px;}
.undo-toast button[aria-label="关闭"]{color:var(--clip-muted);font-size:16px;line-height:1;}
```

- [ ] **Step 2: 构建确认无破坏**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 通过（CSS 不影响类型；样式肉眼验证留到 Task 12）。

- [ ] **Step 3: Commit**

```bash
git add src/App.css
git commit -m "style(clipboard): 新增 undo-toast 浮层样式"
```

---

## Task 11: purge 二次确认（`SettingsAdvancedActions.tsx`，严格按 spec）

**Files:**
- Modify: `src/components/clipboard/SettingsAdvancedActions.tsx`（`MaintenanceAction` 加 confirm）
- Test: `src/components/clipboard/SettingsAdvancedActions.test.tsx`（新建）

> **dead-code 提醒（已与用户确认严格按 spec）：** `MaintenanceAction` 未被任何处引用，实际渲染的是 `DesktopSettingsPanel.tsx:61-65` 的「清理」按钮。本任务令单测通过，但 `pnpm tauri dev` 实际点「清理」**不会**弹确认框——这是严格遵循 spec §4.5 的已知后果，Task 12 GUI 验证里据此说明。

- [ ] **Step 1: 写失败测试**

创建 `src/components/clipboard/SettingsAdvancedActions.test.tsx`：

```tsx
// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { MaintenanceAction } from "@/components/clipboard/SettingsAdvancedActions";

const confirmMock = vi.fn();

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: (...args: unknown[]) => confirmMock(...args),
}));

beforeEach(() => {
  confirmMock.mockReset();
});

describe("MaintenanceAction purge confirm", () => {
  it("calls onPurgeDeletedItems when confirm resolves true", async () => {
    confirmMock.mockResolvedValue(true);
    const onPurge = vi.fn();
    render(<MaintenanceAction isBusy={false} onPurgeDeletedItems={onPurge} />);

    fireEvent.click(screen.getByText("清理已删除记录"));

    await waitFor(() => expect(onPurge).toHaveBeenCalledTimes(1));
  });

  it("does not call onPurgeDeletedItems when confirm resolves false", async () => {
    confirmMock.mockResolvedValue(false);
    const onPurge = vi.fn();
    render(<MaintenanceAction isBusy={false} onPurgeDeletedItems={onPurge} />);

    fireEvent.click(screen.getByText("清理已删除记录"));

    await waitFor(() => expect(confirmMock).toHaveBeenCalledTimes(1));
    expect(onPurge).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: 运行确认失败（红）**

Run: `pnpm.cmd test SettingsAdvancedActions`
Expected: FAIL——现有实现直接 `onClick={onPurgeDeletedItems}`，confirm=false 用例里 `onPurge` 仍被调用。

- [ ] **Step 3: 给 `MaintenanceAction` 加 confirm**

把 `src/components/clipboard/SettingsAdvancedActions.tsx:1-21` 替换为：

```tsx
import { confirm } from "@tauri-apps/plugin-dialog";

import { Button } from "@/components/ui/button";

export function MaintenanceAction({
  isBusy,
  onPurgeDeletedItems,
}: {
  isBusy: boolean;
  onPurgeDeletedItems: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-xl border bg-background/60 p-3 md:col-span-2">
      <div>
        <div className="text-sm font-medium">数据维护</div>
        <div className="text-xs text-muted-foreground">物理删除已移入回收状态的记录并压缩数据库</div>
      </div>
      <Button
        disabled={isBusy}
        size="sm"
        variant="outline"
        onClick={() => void confirmPurge(onPurgeDeletedItems)}
      >
        清理已删除记录
      </Button>
    </div>
  );
}

async function confirmPurge(onPurgeDeletedItems: () => void) {
  const ok = await confirm(
    "将物理删除所有已移入回收状态的记录，此操作不可恢复。是否继续？",
    { title: "清理已删除记录", kind: "warning" },
  );
  if (ok) {
    onPurgeDeletedItems();
  }
}
```

> `CustomSecretPatternsSetting`（原 23 行起）保持不变。

- [ ] **Step 4: 运行验证通过（绿）**

Run: `pnpm.cmd test SettingsAdvancedActions`
Expected: 2 个用例全 PASS。

- [ ] **Step 5: 类型检查 + Commit**

Run: `pnpm.cmd exec tsc --noEmit`（Expected: 通过）

```bash
git add src/components/clipboard/SettingsAdvancedActions.tsx src/components/clipboard/SettingsAdvancedActions.test.tsx
git commit -m "feat(clipboard): purge 前增加原生二次确认（MaintenanceAction）"
```

---

## Task 12: 全量验证 + 审计标记

**Files:**
- Modify: `docs/2026-05-28-clipboard-toolbox-audit.md:124`、`docs/2026-05-28-clipboard-toolbox-audit.md:193`

- [ ] **Step 1: 后端全量测试 + 检查**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: 全 PASS、无新警告。

- [ ] **Step 2: 前端全量构建**

Run: `pnpm.cmd build`
Expected: `tsc` + `vitest run` + `vite build` 全通过。

- [ ] **Step 3: 标记审计 #14（章节标题）**

把 `docs/2026-05-28-clipboard-toolbox-audit.md:124` 的 `### 14. 回收处于中间态` 替换为：

```markdown
### 14. 回收处于中间态 ✅ 2026-05-30 已修复
```

并在该节正文（`docs/2026-05-28-clipboard-toolbox-audit.md:126` 那段「软删除后没有...」）之后新增一行：

```markdown

**修复：** 采用「撤销窗口 + 物理清理二次确认」方向（非回收站，YAGNI）：清空当日后弹 6 秒撤销 toast 可一键恢复该批；「清理已删除记录」改为先弹原生 confirm。详见 `docs/superpowers/specs/2026-05-30-clear-undo-purge-confirm-design.md`。
```

- [ ] **Step 4: 标记审计 #14（清单表）**

把 `docs/2026-05-28-clipboard-toolbox-audit.md:193` 的：

```markdown
| P2 | 14 | 回收无 UI | 体验 |
```

替换为：

```markdown
| P2 | 14 | 回收无 UI ✅ | 体验 |
```

- [ ] **Step 5: GUI 验证（需用户肉眼确认）**

> 提示用户在干净环境启动（按 memory：`tauri dev` 残留进程会占用端口 1420，启动前清理）。验证点：
> 1. 清空当日 → 右下角出现「已清空 N 条记录」toast；6 秒后自动消失。
> 2. 6 秒内点「撤销」→ 记录恢复、视图切回该日期、message 显示「已恢复 N 条记录。」。
> 3. 当天无可删项时清空 → **不**弹 toast、仅有「已清空」message。
> 4. **已知差异（严格按 spec 的后果）：** 设置面板里点「清理」**不会**弹确认框——实际渲染的是 `DesktopSettingsPanel` 的按钮，而 confirm 只加在了未被引用的 `MaintenanceAction`。若需真正生效，须把 `DesktopSettingsPanel` 的清理按钮接到同款 confirm（超出本 spec 范围，留作后续）。

- [ ] **Step 6: Commit**

```bash
git add docs/2026-05-28-clipboard-toolbox-audit.md
git commit -m "docs(clipboard): 标记审计 #14 已修复（撤销 + 二次确认）"
```

---

## 自检对照

- **spec §3.1**（repository）→ Task 1、2 ✓
- **spec §3.2**（service）→ Task 1（clear 透传）、Task 3（restore）✓
- **spec §3.3 / §3.4**（commands / lib）→ Task 4 ✓
- **spec §3.5**（capabilities 无需改）→ 无对应任务，正确 ✓
- **spec §4.1**（API）→ Task 5 ✓
- **spec §4.2**（useUndoToast）→ Task 6 ✓
- **spec §4.3**（workspace 接线）→ Task 8 ✓
- **spec §4.4**（UndoToast + ClipStudioPage + App.css）→ Task 7、9、10 ✓
- **spec §4.5**（purge 确认）→ Task 11（严格按 spec 改 `SettingsAdvancedActions.tsx`，dead-code 后果已标注）✓
- **spec §6 测试 1-5**（repo）→ Task 1、2 ✓；**6**（service）→ Task 3 ✓；**7-10**（useUndoToast）→ Task 6 ✓；**11**（UndoToast）→ Task 7 ✓；**12-13**（workspace）→ Task 8 ✓；**14**（purge confirm）→ Task 11（测试落在 `SettingsAdvancedActions.test.tsx`，非 spec 笔误的 `DesktopSettingsPanel.test.tsx`）✓
- **spec §7 文件清单** → 全覆盖（含 audit 标记，Task 12）✓
- **spec §8 验证** → Task 12 ✓
- **类型一致性**：`UndoState { ids:number[]; date:string; count:number }` 在 hook/组件/workspace 三处一致；`restoreClipboardItems(ids:number[]):Promise<number>` 与命令 `restore_clipboard_items(ids:Vec<i64>)->usize` 对应；`clearClipboardItemsByDate():Promise<number[]>` 与命令 `->Vec<i64>` 对应 ✓
