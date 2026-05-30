# #14 设计：清空当日可撤销 + 物理清理二次确认

> 设计日期：2026-05-30
> 审查项：`docs/2026-05-28-clipboard-toolbox-audit.md` P2 #14（回收处于中间态）
> 范围：`src-tauri/src/clipboard/{repository.rs,service.rs,commands.rs}`、`src-tauri/src/lib.rs`、`src/api/clipboard.ts`、`src/hooks/useClipboardWorkspace.ts`、新增 `src/hooks/useUndoToast.ts` 与 `src/components/clipboard/UndoToast.tsx`、`src/components/clipboard/SettingsAdvancedActions.tsx`、`src/components/clipboard/ClipStudioPage.tsx`、`src/App.css` 及对应测试

## 1. 背景与问题

当前删除链路（探索结论）：

- `delete_clipboard_item` / `clear_clipboard_items_by_date` 都是**软删**（置 `deleted_at`），所有查询带 `deleted_at IS NULL` 过滤，删后对用户**立即彻底不可见**。
- retention 自动清理（`cleanup_items`，每捕获满 50 条或保存设置时触发）**也是软删**，与用户手动删的混在同一个 `deleted_at`。
- 只有用户在设置里点「清理已删除记录」触发 `purge_deleted_clipboard_items` 才 `DELETE`（物理删，不可逆）。
- **无任何恢复入口**：后端无 restore 函数，前端无回收站 UI。

「回收中间态」的体感问题有二：批量「清空当日」一旦手滑无法挽回；物理清理是不可逆操作却无任何确认。

## 2. 设计目标与非目标

### 目标

- **清空当日可撤销**：`clear_clipboard_items_by_date` 后弹独立浮层 toast，约 **6 秒**内可一键撤销（把这批刚软删的记录 `deleted_at` 置回 `NULL`）。
- **物理清理二次确认**：「清理已删除记录」点击后先弹原生确认对话框，确认才执行。

### 非目标

- **不做回收站 / 已删除项浏览**：用户诉求是「防批量误删」，非「长期可追溯」。不新增列出/管理已删除项的 UI 或查询。YAGNI。
- **单条删除不变**：`delete_clipboard_item` 维持即时软删，不加撤销、不加确认（用户明确表示单条删不在意）。
- **retention 不变**：自动清理仍走软删，不进入撤销范围（非用户主动操作）。
- **vacuum 行为不变**：purge 仍固定 `vacuum=true`。
- **FTS 不改**：restore 是 `UPDATE deleted_at`，不动 `content`/`preview`，`clipboard_fts_au` 触发器的 `WHEN` 守卫不重写 FTS；软删行本就留在 FTS、靠查询 JOIN 的 `deleted_at IS NULL` 排除，恢复后自然重新可见。

## 3. 后端设计（Rust）

### 3.1 repository 层（`repository.rs`）

**改造 `soft_delete_items_by_date`** —— 返回被软删的 id 列表（SQLite 3.46 支持 `RETURNING`，用 `query_map` 读回）：

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

**新增 `restore_items`** —— 按 id 精确恢复（动态占位符，`params_from_iter`）：

```rust
use rusqlite::params_from_iter; // 顶部 use 补充

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

`AND deleted_at IS NOT NULL` 保证只动确实处于软删态的行；某些 id 在撤销窗口内已被 purge 物理删则命中不到，不报错。`soft_delete_item`（单条）与 `cleanup_items`（retention）**不动**。

### 3.2 service 层（`service.rs`）

`clear_items_by_date` 返回类型 `usize` → `Vec<i64>`（透传）；新增 `restore_items`，与 clear 共用 `lock_items_conn()`：

```rust
pub fn clear_items_by_date(&self, date: &str) -> Result<Vec<i64>, ClipboardError> {
    let conn = self.lock_items_conn()?;
    repository::soft_delete_items_by_date(&conn, date, &now_iso())
}

pub fn restore_items(&self, ids: &[i64]) -> Result<usize, ClipboardError> {
    let conn = self.lock_items_conn()?;
    repository::restore_items(&conn, ids)
}
```

`delete_item` / `purge_deleted_items` 不变。

### 3.3 commands 层（`commands.rs`）

`clear_clipboard_items_by_date` 返回 `Vec<i64>`（仍 `emit_deleted`，让其余监听刷新）；新增 `restore_clipboard_items`：

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

`restore_clipboard_items` **不 emit 事件**：本应用为单主窗口，撤销发起方（前端）撤销成功后自行刷新即可，无需广播；避免发一个语义不符的 `item-deleted` 事件。`purge_deleted_clipboard_items` 命令本身不变（确认在前端）。

### 3.4 命令注册（`lib.rs`）

`use clipboard::commands::{...}` 补 `restore_clipboard_items`；`tauri::generate_handler![...]` 列表加 `restore_clipboard_items`。

### 3.5 capabilities

`capabilities/default.json` 只声明插件权限（`core:default` / `dialog:default` / `opener:default`），不含自定义命令白名单 → 新命令**无需改动**；`dialog:default` 已覆盖 purge 二次确认所需的 `confirm`。

## 4. 前端设计（React / TS）

### 4.1 API 层（`api/clipboard.ts`）

```ts
export function clearClipboardItemsByDate(date: string): Promise<number[]> {
  return invoke("clear_clipboard_items_by_date", { date });
}

export function restoreClipboardItems(ids: number[]): Promise<number> {
  return invoke("restore_clipboard_items", { ids });
}
```

### 4.2 `useUndoToast` hook（新增 `src/hooks/useUndoToast.ts`）

只负责「当前待撤销数据 + 计时 + 可见性」，撤销/关闭的业务动作由调用方处理：

```ts
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

`show` 替换旧的待撤销项（再次清空时旧 toast 直接消失，旧批次保持软删、不自动恢复）。

### 4.3 `useClipboardWorkspace` 接线

- 实例化 `const undo = useUndoToast();`，把 `undo.pending` 与下面两个动作经返回值暴露给页面。
- `clearDate` 改造：清空拿到 ids → 刷新 → 非空则 `undo.show(...)`：

```ts
function createClearDate({ selectedDate, refreshView, setMessage, undoShow }) {
  return async () => {
    const ids = await clearClipboardItemsByDate(selectedDate);
    setMessage(`已清空 ${selectedDate} 的剪贴板记录。`);
    await refreshView();
    if (ids.length > 0) {
      undoShow({ ids, date: selectedDate, count: ids.length });
    }
  };
}
```

- 新增 `undoClear` 动作（toast 撤销按钮调用）。撤销后**切回该批日期并按该日期刷新**；为规避 `refreshView` 闭包绑定旧 `selectedDate` 的时序问题，直接按 `pending.date` 重载，不依赖闭包：

```ts
// pending = undo.pending（非空时）
await restoreClipboardItems(pending.ids);
undo.clear();
setSearchTerm("");
setSelectedDate(pending.date);
setLoadedItems(await listClipboardItems(pending.date));
await loadDates();
setMessage(`已恢复 ${pending.count} 条记录。`);
```

- 暴露：`undoState: undo.pending`、`undoClear`、`dismissUndo: undo.clear`。

### 4.4 `UndoToast` 组件（新增 `src/components/clipboard/UndoToast.tsx`）

```tsx
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

在 `ClipStudioPage` 顶层渲染 `<UndoToast pending={workspace.undoState} onUndo={workspace.undoClear} onDismiss={workspace.dismissUndo} />`。`.undo-toast` 样式加入 `App.css`：右下角 `position: fixed`、与现有配色一致、`z-index` 高于主面板。

### 4.5 purge 二次确认（`SettingsAdvancedActions.tsx`）

`MaintenanceAction` 点击改为先确认（`@tauri-apps/plugin-dialog`，`dialog:default` 已授权）：

```tsx
import { confirm } from "@tauri-apps/plugin-dialog";

// onClick:
async () => {
  const ok = await confirm(
    "将物理删除所有已移入回收状态的记录，此操作不可恢复。是否继续？",
    { title: "清理已删除记录", kind: "warning" },
  );
  if (ok) onPurgeDeletedItems();
}
```

确认逻辑放在按钮组件内（最局部），`useClipboardWorkspace.purgeDeletedItems` 保持纯逻辑、便于测试。

## 5. 数据流与边缘情况

**主流程**

1. 点「清空当日」→ 后端 `RETURNING` 返回被软删 ids → 前端刷新（列表清空）→ ids 非空则 toast 出现 + 6s 计时。
2. 6s 内点「撤销」→ `restore_clipboard_items(ids)` 置 `deleted_at=NULL` → 切回该日期重载 → 记录恢复可见。
3. 6s 内未操作 → toast 自动消失 → 数据保持软删（后续 retention/purge 处理）。
4. 「清理已删除记录」→ confirm → 确认则 `purge`（`DELETE` + `VACUUM`，FTS `ad` 触发器移除）；取消则什么都不做。

**边缘情况**

- **当天无可删项**（ids 为空）：不弹 toast（无可撤销内容），仅保留「已清空」message。
- **撤销窗口内再次清空**（同一天或别的天）：`undo.show` 替换旧 pending，旧 toast 消失，旧批次保持软删、不自动恢复。
- **撤销窗口内切换日期/搜索**：toast 持有的是具体 ids，与当前视图无关，仍可撤销；撤销后切回 `pending.date` 展示恢复结果。
- **撤销时部分 id 已被 purge 物理删**：`restore_items` 的 `WHERE` 命中不到，返回行数 < ids 长度，不报错，能恢复多少恢复多少。
- **组件卸载**：`useUndoToast` 的 `useEffect` 清理 timer，避免泄漏。

## 6. 测试计划

### 后端 `repository_tests.rs`

1. `clear_returns_soft_deleted_ids`：插入某天多条 + 一条已软删，清空该天，返回 ids 恰为「原本未删的那些」。
2. `restore_items_clears_deleted_at`：软删若干 → `restore_items(ids)` → 这些行 `deleted_at` 为 NULL、返回行数正确、可被查询/搜索重新命中。
3. `restore_items_empty_returns_zero`：空切片返回 0、不执行 SQL。
4. `restore_items_ignores_already_active`：对未软删的 id 调用，不报错、不影响、返回 0。
5. `restore_items_does_not_touch_other_deleted`：恢复 A 批，retention 软删的 B 批 `deleted_at` 不受影响。

### 后端 `service_tests.rs`

6. `clear_items_by_date` 返回 ids；`restore_items` 透传恢复行数（沿用现有 service 测试搭建方式）。

### 前端

- 新增 `src/hooks/useUndoToast.test.tsx`（fake timers）：
  7. `show` 后 `pending` 含 ids/date/count；
  8. `durationMs` 后 `pending` 自动归 `null`；
  9. 再次 `show` 替换旧 pending 并重置计时；
  10. `clear` 立即清空并停止计时。
- `src/components/clipboard/UndoToast.test.tsx`：
  11. `pending` 为 null 不渲染；非空渲染条数文案；点「撤销」触发 `onUndo`；点关闭触发 `onDismiss`。
- `useClipboardWorkspace.test.tsx` 补：
  12. `clearDate` 命中非空 ids 后产生 `undoState`；ids 为空则 `undoState` 仍为 null；
  13. `undoClear` 调 `restoreClipboardItems(ids)` 并清 `undoState`。
- `DesktopSettingsPanel.test.tsx` 补（已 mock `@tauri-apps/plugin-dialog`）：
  14. confirm 返回 `true` → 调 `onPurgeDeletedItems`；返回 `false` → 不调用。

## 7. 改动文件清单

- `src-tauri/src/clipboard/repository.rs`：`soft_delete_items_by_date` 改 `RETURNING id` 返回 `Vec<i64>`；新增 `restore_items`；补 `use rusqlite::params_from_iter`
- `src-tauri/src/clipboard/service.rs`：`clear_items_by_date` 改返回 `Vec<i64>`；新增 `restore_items`
- `src-tauri/src/clipboard/commands.rs`：`clear_clipboard_items_by_date` 改返回 `Vec<i64>`；新增 `restore_clipboard_items`
- `src-tauri/src/lib.rs`：导入 + 注册 `restore_clipboard_items`
- `src-tauri/src/clipboard/repository_tests.rs` / `service_tests.rs`：新增用例
- `src/api/clipboard.ts`：`clearClipboardItemsByDate` 返回 `number[]`；新增 `restoreClipboardItems`
- `src/hooks/useUndoToast.ts`（新增）+ `src/hooks/useUndoToast.test.tsx`（新增）
- `src/hooks/useClipboardWorkspace.ts`：接线 undo、`clearDate` 改造、新增 `undoClear`；`useClipboardWorkspace.test.tsx` 补用例
- `src/components/clipboard/UndoToast.tsx`（新增）+ `UndoToast.test.tsx`（新增）
- `src/components/clipboard/ClipStudioPage.tsx`：渲染 `UndoToast`
- `src/components/clipboard/SettingsAdvancedActions.tsx`：purge 二次确认；`DesktopSettingsPanel.test.tsx` 补用例
- `src/App.css`：`.undo-toast` 样式
- `docs/2026-05-28-clipboard-toolbox-audit.md`：标记 #14 已处理（撤销 + 确认方向）

## 8. 验证

- `cd src-tauri; cargo test clipboard`：全通过（含新增 restore/clear 用例）
- `cd src-tauri; cargo check`：无新警告
- `pnpm.cmd test`：前端用例通过（含 useUndoToast / UndoToast / 设置面板 confirm）
- `pnpm.cmd build`：`tsc` + `vitest run` + `vite build` 通过
- `pnpm tauri dev`（用户肉眼确认）：清空当日出现撤销 toast、6s 自动消失、点撤销记录恢复并切回该日期；「清理已删除记录」弹确认、取消不删、确认才删
