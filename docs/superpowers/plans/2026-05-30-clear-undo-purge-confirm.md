# #14 清空当日可撤销 + 物理清理二次确认 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 清空当日后弹 6 秒撤销 toast；物理清理前弹确认对话框。

**Architecture:** 后端 `soft_delete_items_by_date` 改用 `RETURNING id` 返回被删 id 列表；新增 `restore_items` 按 id 恢复；前端新增 `useUndoToast` hook + `UndoToast` 组件；purge 按钮加 `confirm` 确认。

**Tech Stack:** Rust (rusqlite 0.32 RETURNING + params_from_iter)、React hooks (useState/useRef/useEffect)、Tauri 2 dialog plugin、Vitest (fake timers)

---

### Task 1: 后端 repository 层 - soft_delete_items_by_date 返回 ids

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs:1` (顶部 use)
- Modify: `src-tauri/src/clipboard/repository.rs:237-249` (soft_delete_items_by_date)
- Modify: `src-tauri/src/clipboard/repository_tests.rs:100-116` (修复断言)

- [ ] **Step 1: 写失败测试 - clear_returns_soft_deleted_ids**

在 `src-tauri/src/clipboard/repository_tests.rs` 末尾（`purge_deleted_items_removes_only_soft_deleted_rows` 后）加：

```rust
#[test]
fn clear_returns_soft_deleted_ids() {
    let path = temp_database_path();
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let id1 = upsert_text_item(&conn, "day1-a", "hash1", "2026-05-30T10:00:00", "2026-05-30").unwrap();
    let id2 = upsert_text_item(&conn, "day1-b", "hash2", "2026-05-30T11:00:00", "2026-05-30").unwrap();
    let _id3 = upsert_text_item(&conn, "day2-a", "hash3", "2026-05-31T10:00:00", "2026-05-31").unwrap();
    soft_delete_item(&conn, id1, "2026-05-30T12:00:00").unwrap();

    let ids = repository::soft_delete_items_by_date(&conn, "2026-05-30", "2026-05-30T13:00:00").unwrap();

    assert_eq!(vec![id2], ids);
}
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cd src-tauri
cargo test clear_returns_soft_deleted_ids -- --nocapture
```

预期：编译错误 `mismatched types: expected Vec<i64>, found usize`

- [ ] **Step 3: 修改 repository.rs 顶部 use**

在 `src-tauri/src/clipboard/repository.rs:1` 的 `use rusqlite::{params, Connection, OptionalExtension, Row};` 改为：

```rust
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
```

- [ ] **Step 4: 改造 soft_delete_items_by_date 返回 Vec<i64>**

将 `src-tauri/src/clipboard/repository.rs:237-249` 的 `soft_delete_items_by_date` 函数替换为：

```rust
pub fn soft_delete_items_by_date(
    connection: &Connection,
    date: &str,
    now: &str,
) -> Result<Vec<i64>, ClipboardError> {
    let mut stmt = connection.prepare(
        "UPDATE clipboard_items
         SET deleted_at = ?1
         WHERE local_date = ?2 AND deleted_at IS NULL
         RETURNING id",
    )?;
    let ids = stmt
        .query_map(params![now, date], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}
```

- [ ] **Step 5: 修复 repository_tests.rs 中旧测试的断言**

将 `src-tauri/src/clipboard/repository_tests.rs:100-116` 的 `soft_deletes_all_items_by_date` 测试中的：

```rust
let changed = repository::soft_delete_items_by_date(&conn, "2026-05-30", "2026-05-30T12:00:00").unwrap();
assert_eq!(2, changed);
```

改为：

```rust
let ids = repository::soft_delete_items_by_date(&conn, "2026-05-30", "2026-05-30T12:00:00").unwrap();
assert_eq!(2, ids.len());
```

- [ ] **Step 6: 运行测试验证通过**

```bash
cd src-tauri
cargo test clipboard::repository_tests -- --nocapture
```

预期：所有 repository_tests 通过（含新增的 clear_returns_soft_deleted_ids）

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/repository_tests.rs
git commit -m "feat(clipboard): soft_delete_items_by_date 返回被删 id 列表"
```

---

### Task 2: 后端 repository 层 - restore_items

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs` (soft_delete_items_by_date 后新增 restore_items)
- Modify: `src-tauri/src/clipboard/repository_tests.rs` (新增 4 个测试)

- [ ] **Step 1: 写失败测试 - restore_items_clears_deleted_at**

在 `src-tauri/src/clipboard/repository_tests.rs` 末尾加：

```rust
#[test]
fn restore_items_clears_deleted_at() {
    let path = temp_database_path();
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let id1 = upsert_text_item(&conn, "text1", "hash1", "2026-05-30T10:00:00", "2026-05-30").unwrap();
    let id2 = upsert_text_item(&conn, "text2", "hash2", "2026-05-30T11:00:00", "2026-05-30").unwrap();
    soft_delete_item(&conn, id1, "2026-05-30T12:00:00").unwrap();
    soft_delete_item(&conn, id2, "2026-05-30T12:00:00").unwrap();

    let changed = repository::restore_items(&conn, &[id1, id2]).unwrap();

    assert_eq!(2, changed);
    let mut stmt = conn.prepare("SELECT id FROM clipboard_items WHERE deleted_at IS NULL ORDER BY id").unwrap();
    let restored: Vec<i64> = stmt.query_map([], |row| row.get(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(vec![id1, id2], restored);
}
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cd src-tauri
cargo test restore_items_clears_deleted_at -- --nocapture
```

预期：编译错误 `cannot find function restore_items`

- [ ] **Step 3: 实现 restore_items**

在 `src-tauri/src/clipboard/repository.rs` 的 `soft_delete_items_by_date` 函数后新增：

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

- [ ] **Step 4: 运行测试验证通过**

```bash
cd src-tauri
cargo test restore_items_clears_deleted_at -- --nocapture
```

预期：PASS

- [ ] **Step 5: 写测试 - restore_items_empty_returns_zero**

在 `src-tauri/src/clipboard/repository_tests.rs` 末尾加：

```rust
#[test]
fn restore_items_empty_returns_zero() {
    let path = temp_database_path();
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let changed = repository::restore_items(&conn, &[]).unwrap();

    assert_eq!(0, changed);
}
```

- [ ] **Step 6: 运行测试验证通过**

```bash
cd src-tauri
cargo test restore_items_empty_returns_zero -- --nocapture
```

预期：PASS

- [ ] **Step 7: 写测试 - restore_items_ignores_already_active**

在 `src-tauri/src/clipboard/repository_tests.rs` 末尾加：

```rust
#[test]
fn restore_items_ignores_already_active() {
    let path = temp_database_path();
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let id1 = upsert_text_item(&conn, "active", "hash1", "2026-05-30T10:00:00", "2026-05-30").unwrap();

    let changed = repository::restore_items(&conn, &[id1]).unwrap();

    assert_eq!(0, changed);
}
```

- [ ] **Step 8: 运行测试验证通过**

```bash
cd src-tauri
cargo test restore_items_ignores_already_active -- --nocapture
```

预期：PASS

- [ ] **Step 9: 写测试 - restore_items_does_not_touch_other_deleted**

在 `src-tauri/src/clipboard/repository_tests.rs` 末尾加：

```rust
#[test]
fn restore_items_does_not_touch_other_deleted() {
    let path = temp_database_path();
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let id1 = upsert_text_item(&conn, "batch-a", "hash1", "2026-05-30T10:00:00", "2026-05-30").unwrap();
    let id2 = upsert_text_item(&conn, "retention", "hash2", "2026-05-30T11:00:00", "2026-05-30").unwrap();
    soft_delete_item(&conn, id1, "2026-05-30T12:00:00").unwrap();
    soft_delete_item(&conn, id2, "2026-05-30T12:00:00").unwrap();

    repository::restore_items(&conn, &[id1]).unwrap();

    let mut stmt = conn.prepare("SELECT deleted_at FROM clipboard_items WHERE id = ?1").unwrap();
    let id2_deleted: Option<String> = stmt.query_row([id2], |row| row.get(0)).unwrap();
    assert!(id2_deleted.is_some());
}
```

- [ ] **Step 10: 运行测试验证通过**

```bash
cd src-tauri
cargo test restore_items_does_not_touch_other_deleted -- --nocapture
```

预期：PASS

- [ ] **Step 11: 运行所有 repository 测试**

```bash
cd src-tauri
cargo test clipboard::repository_tests -- --nocapture
```

预期：所有测试通过

- [ ] **Step 12: 提交**

```bash
git add src-tauri/src/clipboard/repository.rs src-tauri/src/clipboard/repository_tests.rs
git commit -m "feat(clipboard): 新增 restore_items 按 id 恢复软删记录"
```

---

### Task 3: 后端 service 层

**Files:**
- Modify: `src-tauri/src/clipboard/service.rs:137-140` (clear_items_by_date 改返回 Vec<i64>)
- Modify: `src-tauri/src/clipboard/service.rs` (clear_items_by_date 后新增 restore_items)
- Modify: `src-tauri/src/clipboard/service_tests.rs` (新增 2 个测试)

- [ ] **Step 1: 写失败测试 - clear_items_by_date_returns_ids**

在 `src-tauri/src/clipboard/service_tests.rs` 末尾加：

```rust
#[test]
fn clear_items_by_date_returns_ids() {
    let path = temp_database_path();
    let service = ClipboardService::new(&path).unwrap();
    let conn = open_connection(&path).unwrap();

    let id1 = repository::upsert_text_item(&conn, "day1-a", "hash1", "2026-05-30T10:00:00", "2026-05-30").unwrap();
    let id2 = repository::upsert_text_item(&conn, "day1-b", "hash2", "2026-05-30T11:00:00", "2026-05-30").unwrap();

    let ids = service.clear_items_by_date("2026-05-30").unwrap();

    assert_eq!(2, ids.len());
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cd src-tauri
cargo test clear_items_by_date_returns_ids -- --nocapture
```

预期：编译错误 `mismatched types: expected Vec<i64>, found ()`

- [ ] **Step 3: 修改 service.rs clear_items_by_date**

将 `src-tauri/src/clipboard/service.rs:137-140` 的 `clear_items_by_date` 函数替换为：

```rust
pub fn clear_items_by_date(&self, date: &str) -> Result<Vec<i64>, ClipboardError> {
    let conn = self.lock_items_conn()?;
    repository::soft_delete_items_by_date(&conn, date, &now_iso())
}
```

- [ ] **Step 4: 运行测试验证通过**

```bash
cd src-tauri
cargo test clear_items_by_date_returns_ids -- --nocapture
```

预期：PASS

- [ ] **Step 5: 写失败测试 - restore_items_passthrough**

在 `src-tauri/src/clipboard/service_tests.rs` 末尾加：

```rust
#[test]
fn restore_items_passthrough() {
    let path = temp_database_path();
    let service = ClipboardService::new(&path).unwrap();
    let conn = open_connection(&path).unwrap();

    let id1 = repository::upsert_text_item(&conn, "text1", "hash1", "2026-05-30T10:00:00", "2026-05-30").unwrap();
    repository::soft_delete_item(&conn, id1, "2026-05-30T12:00:00").unwrap();

    let changed = service.restore_items(&[id1]).unwrap();

    assert_eq!(1, changed);
    let results = service.search_items("text1").unwrap();
    assert_eq!(1, results.len());
}
```

- [ ] **Step 6: 运行测试验证失败**

```bash
cd src-tauri
cargo test restore_items_passthrough -- --nocapture
```

预期：编译错误 `no method named restore_items`

- [ ] **Step 7: 实现 service.rs restore_items**

在 `src-tauri/src/clipboard/service.rs` 的 `clear_items_by_date` 函数后新增：

```rust
pub fn restore_items(&self, ids: &[i64]) -> Result<usize, ClipboardError> {
    let conn = self.lock_items_conn()?;
    repository::restore_items(&conn, ids)
}
```

- [ ] **Step 8: 运行测试验证通过**

```bash
cd src-tauri
cargo test restore_items_passthrough -- --nocapture
```

预期：PASS

- [ ] **Step 9: 运行所有 service 测试**

```bash
cd src-tauri
cargo test clipboard::service_tests -- --nocapture
```

预期：所有测试通过

- [ ] **Step 10: 提交**

```bash
git add src-tauri/src/clipboard/service.rs src-tauri/src/clipboard/service_tests.rs
git commit -m "feat(clipboard): service 层透传 clear 返回 ids 与 restore"
```

---

### Task 4: 后端 commands 层与注册

**Files:**
- Modify: `src-tauri/src/clipboard/commands.rs:68-77` (clear_clipboard_items_by_date 改返回 Vec<i64>)
- Modify: `src-tauri/src/clipboard/commands.rs` (clear 后新增 restore_clipboard_items)
- Modify: `src-tauri/src/lib.rs:3-9` (use 补 restore_clipboard_items)
- Modify: `src-tauri/src/lib.rs:53-69` (generate_handler 补 restore_clipboard_items)

- [ ] **Step 1: 修改 commands.rs clear_clipboard_items_by_date**

将 `src-tauri/src/clipboard/commands.rs:68-77` 的 `clear_clipboard_items_by_date` 函数替换为：

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
```

- [ ] **Step 2: 新增 commands.rs restore_clipboard_items**

在 `src-tauri/src/clipboard/commands.rs` 的 `clear_clipboard_items_by_date` 函数后新增：

```rust
#[tauri::command]
pub fn restore_clipboard_items(
    ids: Vec<i64>,
    state: State<'_, ClipboardState>,
) -> Result<usize, ClipboardError> {
    state.0.restore_items(&ids)
}
```

- [ ] **Step 3: lib.rs 补充 use**

将 `src-tauri/src/lib.rs:3-9` 的 `use clipboard::commands::{...}` 中补充 `restore_clipboard_items`（按字母序插入）。

- [ ] **Step 4: lib.rs 补充 generate_handler**

将 `src-tauri/src/lib.rs:53-69` 的 `generate_handler![...]` 列表中补充 `restore_clipboard_items`（按字母序插入）。

- [ ] **Step 5: cargo check**

```bash
cd src-tauri
cargo check
```

预期：无错误无警告

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/clipboard/commands.rs src-tauri/src/lib.rs
git commit -m "feat(clipboard): 注册 restore_clipboard_items 命令"
```

---

### Task 5: 前端 API 层

**Files:**
- Modify: `src/api/clipboard.ts` (clearClipboardItemsByDate 改返回 number[]；新增 restoreClipboardItems)

- [ ] **Step 1: 修改 clearClipboardItemsByDate**

将 `src/api/clipboard.ts` 中的 `clearClipboardItemsByDate` 函数改为：

```typescript
export function clearClipboardItemsByDate(date: string): Promise<number[]> {
  return invoke("clear_clipboard_items_by_date", { date });
}
```

- [ ] **Step 2: 新增 restoreClipboardItems**

在 `src/api/clipboard.ts` 的 `clearClipboardItemsByDate` 后新增：

```typescript
export function restoreClipboardItems(ids: number[]): Promise<number> {
  return invoke("restore_clipboard_items", { ids });
}
```

- [ ] **Step 3: tsc 检查**

```bash
pnpm exec tsc --noEmit
```

预期：无错误

- [ ] **Step 4: 提交**

```bash
git add src/api/clipboard.ts
git commit -m "feat(clipboard): API 层 clear 返回 ids + restore"
```

---

### Task 6: useUndoToast hook

**Files:**
- Create: `src/hooks/useUndoToast.ts`
- Create: `src/hooks/useUndoToast.test.tsx`

- [ ] **Step 1: 写失败测试 - show 后 pending 含数据**

创建 `src/hooks/useUndoToast.test.tsx`：

```typescript
// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

import { useUndoToast } from "./useUndoToast";

describe("useUndoToast", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("show 后 pending 含 ids/date/count", () => {
    const { result } = renderHook(() => useUndoToast());

    act(() => {
      result.current.show({ ids: [1, 2], date: "2026-05-30", count: 2 });
    });

    expect(result.current.pending).toEqual({ ids: [1, 2], date: "2026-05-30", count: 2 });
  });
});
```

- [ ] **Step 2: 运行测试验证失败**

```bash
pnpm test useUndoToast
```

预期：`Cannot find module './useUndoToast'`

- [ ] **Step 3: 实现 useUndoToast.ts**

创建 `src/hooks/useUndoToast.ts`：

```typescript
import { useCallback, useEffect, useRef, useState } from "react";

export interface UndoState {
  ids: number[];
  date: string;
  count: number;
}

export function useUndoToast(durationMs = 6000) {
  const [pending, setPending] = useState<UndoState | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  const show = useCallback((next: UndoState) => {
    clearTimeout(timerRef.current);
    setPending(next);
    timerRef.current = setTimeout(() => setPending(null), durationMs);
  }, [durationMs]);

  const clear = useCallback(() => {
    clearTimeout(timerRef.current);
    setPending(null);
  }, []);

  useEffect(() => () => clearTimeout(timerRef.current), []);

  return { pending, show, clear };
}
```

- [ ] **Step 4: 运行测试验证通过**

```bash
pnpm test useUndoToast
```

预期：1 passed

- [ ] **Step 5: 写测试 - durationMs 后 pending 自动归 null**

在 `src/hooks/useUndoToast.test.tsx` 的 describe 块内加：

```typescript
  it("durationMs 后 pending 自动归 null", () => {
    const { result } = renderHook(() => useUndoToast(3000));

    act(() => {
      result.current.show({ ids: [1], date: "2026-05-30", count: 1 });
    });

    expect(result.current.pending).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(3000);
    });

    expect(result.current.pending).toBeNull();
  });
```

- [ ] **Step 6: 运行测试验证通过**

```bash
pnpm test useUndoToast
```

预期：2 passed

- [ ] **Step 7: 写测试 - 再次 show 替换旧 pending 并重置计时**

在 `src/hooks/useUndoToast.test.tsx` 的 describe 块内加：

```typescript
  it("再次 show 替换旧 pending 并重置计时", () => {
    const { result } = renderHook(() => useUndoToast(3000));

    act(() => {
      result.current.show({ ids: [1], date: "2026-05-30", count: 1 });
    });

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    act(() => {
      result.current.show({ ids: [2, 3], date: "2026-05-31", count: 2 });
    });

    expect(result.current.pending).toEqual({ ids: [2, 3], date: "2026-05-31", count: 2 });

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(result.current.pending).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(result.current.pending).toBeNull();
  });
```

- [ ] **Step 8: 运行测试验证通过**

```bash
pnpm test useUndoToast
```

预期：3 passed

- [ ] **Step 9: 写测试 - clear 立即清空并停止计时**

在 `src/hooks/useUndoToast.test.tsx` 的 describe 块内加：

```typescript
  it("clear 立即清空并停止计时", () => {
    const { result } = renderHook(() => useUndoToast(3000));

    act(() => {
      result.current.show({ ids: [1], date: "2026-05-30", count: 1 });
    });

    act(() => {
      result.current.clear();
    });

    expect(result.current.pending).toBeNull();

    act(() => {
      vi.advanceTimersByTime(3000);
    });

    expect(result.current.pending).toBeNull();
  });
```

- [ ] **Step 10: 运行测试验证通过**

```bash
pnpm test useUndoToast
```

预期：4 passed

- [ ] **Step 11: 提交**

```bash
git add src/hooks/useUndoToast.ts src/hooks/useUndoToast.test.tsx
git commit -m "feat(clipboard): 新增 useUndoToast hook"
```

---

### Task 7: UndoToast 组件

**Files:**
- Create: `src/components/clipboard/UndoToast.tsx`
- Create: `src/components/clipboard/UndoToast.test.tsx`

- [ ] **Step 1: 写失败测试 - pending 为 null 不渲染**

创建 `src/components/clipboard/UndoToast.test.tsx`：

```typescript
// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

import { UndoToast } from "./UndoToast";

describe("UndoToast", () => {
  it("pending 为 null 不渲染", () => {
    const { container } = render(
      <UndoToast pending={null} onUndo={vi.fn()} onDismiss={vi.fn()} />
    );

    expect(container.firstChild).toBeNull();
  });
});
```

- [ ] **Step 2: 运行测试验证失败**

```bash
pnpm test UndoToast
```

预期：`Cannot find module './UndoToast'`

- [ ] **Step 3: 实现 UndoToast.tsx**

创建 `src/components/clipboard/UndoToast.tsx`：

```typescript
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
  if (!pending) return null;
  return (
    <div className="undo-toast" role="status">
      <span>已清空 {pending.count} 条记录</span>
      <button type="button" onClick={onUndo}>撤销</button>
      <button type="button" aria-label="关闭" onClick={onDismiss}>×</button>
    </div>
  );
}
```

- [ ] **Step 4: 运行测试验证通过**

```bash
pnpm test UndoToast
```

预期：1 passed

- [ ] **Step 5: 写测试 - 非空渲染条数文案**

在 `src/components/clipboard/UndoToast.test.tsx` 的 describe 块内加：

```typescript
  it("非空渲染条数文案", () => {
    render(
      <UndoToast
        pending={{ ids: [1, 2, 3], date: "2026-05-30", count: 3 }}
        onUndo={vi.fn()}
        onDismiss={vi.fn()}
      />
    );

    expect(screen.getByRole("status")).toHaveTextContent("已清空 3 条记录");
  });
```

- [ ] **Step 6: 运行测试验证通过**

```bash
pnpm test UndoToast
```

预期：2 passed

- [ ] **Step 7: 写测试 - 点撤销触发 onUndo**

在 `src/components/clipboard/UndoToast.test.tsx` 的 describe 块内加：

```typescript
  it("点撤销触发 onUndo", () => {
    const onUndo = vi.fn();
    render(
      <UndoToast
        pending={{ ids: [1], date: "2026-05-30", count: 1 }}
        onUndo={onUndo}
        onDismiss={vi.fn()}
      />
    );

    screen.getByRole("button", { name: "撤销" }).click();

    expect(onUndo).toHaveBeenCalledTimes(1);
  });
```

- [ ] **Step 8: 运行测试验证通过**

```bash
pnpm test UndoToast
```

预期：3 passed

- [ ] **Step 9: 写测试 - 点关闭触发 onDismiss**

在 `src/components/clipboard/UndoToast.test.tsx` 的 describe 块内加：

```typescript
  it("点关闭触发 onDismiss", () => {
    const onDismiss = vi.fn();
    render(
      <UndoToast
        pending={{ ids: [1], date: "2026-05-30", count: 1 }}
        onUndo={vi.fn()}
        onDismiss={onDismiss}
      />
    );

    screen.getByRole("button", { name: "关闭" }).click();

    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
```

- [ ] **Step 10: 运行测试验证通过**

```bash
pnpm test UndoToast
```

预期：4 passed

- [ ] **Step 11: 提交**

```bash
git add src/components/clipboard/UndoToast.tsx src/components/clipboard/UndoToast.test.tsx
git commit -m "feat(clipboard): 新增 UndoToast 组件"
```

---

### Task 8: useClipboardWorkspace 接线 undo

**Files:**
- Modify: `src/hooks/useClipboardWorkspace.ts` (实例化 useUndoToast；改造 clearDate；新增 undoClear；暴露 undoState/undoClear/dismissUndo)
- Modify: `src/hooks/useClipboardWorkspace.test.tsx` (修复 clear mock；新增 2 个测试)

- [ ] **Step 1: 写失败测试 - clearDate 命中非空 ids 后产生 undoState**

在 `src/hooks/useClipboardWorkspace.test.tsx` 末尾加：

```typescript
describe("撤销清空", () => {
  it("clearDate 命中非空 ids 后产生 undoState", async () => {
    setupInvoke({
      clear_clipboard_items_by_date: () => Promise.resolve([10, 20]),
    });
    const { result } = renderHook(() => useClipboardWorkspace());

    await act(async () => {
      await result.current.selectDate("2026-05-30");
    });

    await act(async () => {
      await result.current.clearDate();
    });

    expect(result.current.undoState).toEqual({
      ids: [10, 20],
      date: "2026-05-30",
      count: 2,
    });
  });

  it("clearDate ids 为空则 undoState 仍为 null", async () => {
    setupInvoke({
      clear_clipboard_items_by_date: () => Promise.resolve([]),
    });
    const { result } = renderHook(() => useClipboardWorkspace());

    await act(async () => {
      await result.current.selectDate("2026-05-30");
    });

    await act(async () => {
      await result.current.clearDate();
    });

    expect(result.current.undoState).toBeNull();
  });
});
```

- [ ] **Step 2: 运行测试验证失败**

```bash
pnpm test useClipboardWorkspace
```

预期：`Property 'undoState' does not exist`

- [ ] **Step 3: 修复 useClipboardWorkspace.test.tsx 的 clear mock**

将 `src/hooks/useClipboardWorkspace.test.tsx` 中 `setupInvoke` 函数内的：

```typescript
case "clear_clipboard_items_by_date":
  return Promise.resolve();
```

改为：

```typescript
case "clear_clipboard_items_by_date":
  return overrides.clear_clipboard_items_by_date
    ? overrides.clear_clipboard_items_by_date(args)
    : Promise.resolve([]);
```

- [ ] **Step 4: useClipboardWorkspace.ts 实例化 useUndoToast**

在 `src/hooks/useClipboardWorkspace.ts` 顶部 import 补充：

```typescript
import { useUndoToast } from "./useUndoToast";
```

在 `useClipboardWorkspace` 函数内、`const [message, setMessage] = useState("");` 后加：

```typescript
const undo = useUndoToast();
```

- [ ] **Step 5: useClipboardWorkspace.ts 改造 clearDate**

将 `src/hooks/useClipboardWorkspace.ts` 中 `createClearDate` 函数内的实现改为：

```typescript
return async () => {
  const ids = await clearClipboardItemsByDate(selectedDate);
  setMessage(`已清空 ${selectedDate} 的剪贴板记录。`);
  await refreshView();
  if (ids.length > 0) {
    undoShow({ ids, date: selectedDate, count: ids.length });
  }
};
```

其中 `undoShow` 从 `undo.show` 解构。

- [ ] **Step 6: useClipboardWorkspace.ts 新增 undoClear**

在 `src/hooks/useClipboardWorkspace.ts` 的 `createClearDate` 后新增 `createUndoClear` 函数：

```typescript
function createUndoClear({
  pending,
  undoClear,
  setSearchTerm,
  setSelectedDate,
  setLoadedItems,
  loadDates,
  setMessage,
}: {
  pending: ReturnType<typeof useUndoToast>["pending"];
  undoClear: ReturnType<typeof useUndoToast>["clear"];
  setSearchTerm: (term: string) => void;
  setSelectedDate: (date: string) => void;
  setLoadedItems: (items: ClipboardItem[]) => void;
  loadDates: () => Promise<void>;
  setMessage: (msg: string) => void;
}) {
  return async () => {
    if (!pending) return;
    await restoreClipboardItems(pending.ids);
    undoClear();
    setSearchTerm("");
    setSelectedDate(pending.date);
    setLoadedItems(await listClipboardItems(pending.date));
    await loadDates();
    setMessage(`已恢复 ${pending.count} 条记录。`);
  };
}
```

并在 `restoreClipboardItems` 的 import 中补充（从 `@/api/clipboard` 导入）。

- [ ] **Step 7: useClipboardWorkspace.ts 暴露 undoState/undoClear/dismissUndo**

在 `useClipboardWorkspace` 返回对象中补充：

```typescript
undoState: undo.pending,
undoClear: createUndoClear({
  pending: undo.pending,
  undoClear: undo.clear,
  setSearchTerm,
  setSelectedDate,
  setLoadedItems,
  loadDates,
  setMessage,
}),
dismissUndo: undo.clear,
```

- [ ] **Step 8: 运行测试验证通过**

```bash
pnpm test useClipboardWorkspace
```

预期：所有测试通过（含新增的 2 个撤销测试）

- [ ] **Step 9: 写测试 - undoClear 调 restoreClipboardItems 并清 undoState**

在 `src/hooks/useClipboardWorkspace.test.tsx` 的 "撤销清空" describe 块内加：

```typescript
  it("undoClear 调 restoreClipboardItems 并清 undoState", async () => {
    const restoreMock = vi.fn().mockResolvedValue(2);
    setupInvoke({
      clear_clipboard_items_by_date: () => Promise.resolve([10, 20]),
      restore_clipboard_items: restoreMock,
    });
    const { result } = renderHook(() => useClipboardWorkspace());

    await act(async () => {
      await result.current.selectDate("2026-05-30");
    });

    await act(async () => {
      await result.current.clearDate();
    });

    expect(result.current.undoState).not.toBeNull();

    await act(async () => {
      await result.current.undoClear();
    });

    expect(restoreMock).toHaveBeenCalledWith({ ids: [10, 20] });
    expect(result.current.undoState).toBeNull();
  });
```

- [ ] **Step 10: 修复 setupInvoke 支持 restore_clipboard_items**

在 `src/hooks/useClipboardWorkspace.test.tsx` 的 `setupInvoke` 函数 switch 中加：

```typescript
case "restore_clipboard_items":
  return overrides.restore_clipboard_items
    ? overrides.restore_clipboard_items(args)
    : Promise.resolve(0);
```

- [ ] **Step 11: 运行测试验证通过**

```bash
pnpm test useClipboardWorkspace
```

预期：所有测试通过（含 3 个撤销测试）

- [ ] **Step 12: 提交**

```bash
git add src/hooks/useClipboardWorkspace.ts src/hooks/useClipboardWorkspace.test.tsx
git commit -m "feat(clipboard): useClipboardWorkspace 接线撤销清空"
```

---

### Task 9: ClipStudioPage 渲染 UndoToast

**Files:**
- Modify: `src/components/clipboard/ClipStudioPage.tsx` (import UndoToast；在 return 顶层渲染)

- [ ] **Step 1: 修改 ClipStudioPage.tsx import**

在 `src/components/clipboard/ClipStudioPage.tsx` 顶部 import 补充：

```typescript
import { UndoToast } from "./UndoToast";
```

- [ ] **Step 2: 修改 ClipStudioPage.tsx 渲染**

在 `src/components/clipboard/ClipStudioPage.tsx` 的 `return` 语句内、`<div className="clip-studio">` 之前加：

```tsx
<UndoToast
  pending={workspace.undoState}
  onUndo={workspace.undoClear}
  onDismiss={workspace.dismissUndo}
/>
```

- [ ] **Step 3: tsc 检查**

```bash
pnpm exec tsc --noEmit
```

预期：无错误

- [ ] **Step 4: 提交**

```bash
git add src/components/clipboard/ClipStudioPage.tsx
git commit -m "feat(clipboard): ClipStudioPage 渲染 UndoToast"
```

---

### Task 10: App.css 样式

**Files:**
- Modify: `src/App.css` (末尾新增 .undo-toast 样式)

- [ ] **Step 1: 新增 .undo-toast 样式**

在 `src/App.css` 末尾加：

```css
.undo-toast {
  position: fixed;
  right: 24px;
  bottom: 24px;
  z-index: 1000;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  background: hsl(var(--background));
  border: 1px solid hsl(var(--border));
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  font-size: 14px;
}

.undo-toast button {
  padding: 4px 12px;
  border: 1px solid hsl(var(--border));
  border-radius: 4px;
  background: hsl(var(--background));
  color: hsl(var(--foreground));
  cursor: pointer;
  font-size: 13px;
}

.undo-toast button:hover {
  background: hsl(var(--accent));
}

.undo-toast button[aria-label="关闭"] {
  padding: 4px 8px;
  font-size: 16px;
  font-weight: bold;
}
```

- [ ] **Step 2: 提交**

```bash
git add src/App.css
git commit -m "feat(clipboard): 新增 UndoToast 样式"
```

---

### Task 11: purge 二次确认

**Files:**
- Modify: `src/components/clipboard/DesktopSettingsPanel.tsx:2` (import confirm)
- Modify: `src/components/clipboard/DesktopSettingsPanel.tsx:61-65` (purge 按钮加 confirm)
- Modify: `src/components/clipboard/DesktopSettingsPanel.test.tsx:7` (mock confirm)
- Modify: `src/components/clipboard/DesktopSettingsPanel.test.tsx` (新增 2 个测试)

- [ ] **Step 1: 写失败测试 - confirm 返回 true 调 onPurgeDeletedItems**

在 `src/components/clipboard/DesktopSettingsPanel.test.tsx` 末尾加：

```typescript
describe("purge 二次确认", () => {
  it("confirm 返回 true 调 onPurgeDeletedItems", async () => {
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(confirm).mockResolvedValue(true);

    const onPurgeDeletedItems = vi.fn();
    render(
      <DesktopSettingsPanel
        settings={SETTINGS}
        isBusy={false}
        onSettingsChange={vi.fn()}
        onPurgeDeletedItems={onPurgeDeletedItems}
        onHideWindow={vi.fn()}
      />
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "清理" }));
    });

    expect(confirm).toHaveBeenCalledWith(
      "将物理删除所有已移入回收状态的记录，此操作不可恢复。是否继续？",
      { title: "清理已删除记录", kind: "warning" }
    );
    expect(onPurgeDeletedItems).toHaveBeenCalledTimes(1);
  });

  it("confirm 返回 false 不调 onPurgeDeletedItems", async () => {
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(confirm).mockResolvedValue(false);

    const onPurgeDeletedItems = vi.fn();
    render(
      <DesktopSettingsPanel
        settings={SETTINGS}
        isBusy={false}
        onSettingsChange={vi.fn()}
        onPurgeDeletedItems={onPurgeDeletedItems}
        onHideWindow={vi.fn()}
      />
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "清理" }));
    });

    expect(onPurgeDeletedItems).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: 运行测试验证失败**

```bash
pnpm test DesktopSettingsPanel
```

预期：`confirm is not a function` 或类似错误

- [ ] **Step 3: 修复 DesktopSettingsPanel.test.tsx mock**

将 `src/components/clipboard/DesktopSettingsPanel.test.tsx:7` 的：

```typescript
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
```

改为：

```typescript
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), confirm: vi.fn() }));
```

- [ ] **Step 4: 修改 DesktopSettingsPanel.tsx import**

将 `src/components/clipboard/DesktopSettingsPanel.tsx:2` 的：

```typescript
import { open } from "@tauri-apps/plugin-dialog";
```

改为：

```typescript
import { confirm, open } from "@tauri-apps/plugin-dialog";
```

- [ ] **Step 5: 修改 DesktopSettingsPanel.tsx purge 按钮**

将 `src/components/clipboard/DesktopSettingsPanel.tsx:61-65` 的 ActionRow 内的 Button 的 `onClick={props.onPurgeDeletedItems}` 改为：

```typescript
onClick={async () => {
  const ok = await confirm(
    "将物理删除所有已移入回收状态的记录，此操作不可恢复。是否继续？",
    { title: "清理已删除记录", kind: "warning" },
  );
  if (ok) props.onPurgeDeletedItems();
}}
```

- [ ] **Step 6: 运行测试验证通过**

```bash
pnpm test DesktopSettingsPanel
```

预期：所有测试通过（含新增的 2 个 purge 确认测试）

- [ ] **Step 7: 提交**

```bash
git add src/components/clipboard/DesktopSettingsPanel.tsx src/components/clipboard/DesktopSettingsPanel.test.tsx
git commit -m "feat(clipboard): purge 二次确认"
```

---

### Task 12: 验证与收尾

**Files:**
- Modify: `docs/2026-05-28-clipboard-toolbox-audit.md:193` (#14 标记已处理)

- [ ] **Step 1: 运行所有后端测试**

```bash
cd src-tauri
cargo test clipboard -- --nocapture
```

预期：所有测试通过

- [ ] **Step 2: cargo check**

```bash
cd src-tauri
cargo check
```

预期：无错误无警告

- [ ] **Step 3: 运行所有前端测试**

```bash
pnpm test
```

预期：所有测试通过

- [ ] **Step 4: tsc 检查**

```bash
pnpm exec tsc --noEmit
```

预期：无错误

- [ ] **Step 5: 前端构建**

```bash
pnpm build
```

预期：`tsc` + `vitest run` + `vite build` 全通过

- [ ] **Step 6: 标记审计 #14 已处理**

将 `docs/2026-05-28-clipboard-toolbox-audit.md:193` 的：

```markdown
| P2 | 14 | 回收无 UI | 体验 |
```

改为：

```markdown
| P2 | 14 | 回收无 UI ✅ | 体验 |
```

并在 `### 14. 回收处于中间态` 标题后加 `✅ 2026-05-30 已实现`。

- [ ] **Step 7: 提交审计标记**

```bash
git add docs/2026-05-28-clipboard-toolbox-audit.md
git commit -m "docs(clipboard): 标记审计 #14 已实现"
```

- [ ] **Step 8: GUI 验证（需用户肉眼确认）**

启动开发环境：

```bash
pnpm tauri dev
```

**验证清单（用户确认）：**
1. 清空当日 → 右下角出现撤销 toast，显示「已清空 N 条记录」+ 撤销按钮 + 关闭按钮
2. 6 秒内不操作 → toast 自动消失
3. 6 秒内点「撤销」→ 记录恢复，切回该日期，toast 消失
4. 点关闭按钮 → toast 立即消失
5. 再次清空当日 → 旧 toast 消失，新 toast 出现
6. 设置面板点「清理」→ 弹原生确认对话框，标题「清理已删除记录」，内容「将物理删除...是否继续？」
7. 确认对话框点「取消」→ 不执行清理
8. 确认对话框点「确认」→ 执行清理

预期：所有验证项通过

---

## 执行完成

所有任务完成后，#14「清空当日可撤销 + 物理清理二次确认」功能已实现并验证通过。

