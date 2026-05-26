# Clipboard MVP Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `docs/clipboard-toolbox-design.md` 中的 Milestone 1：基础闭环，让应用能自动捕获文本剪贴板、持久化到本地 SQLite、按日期展示，并支持查看详情、复制回剪贴板、删除记录。

**Architecture:** Rust/Tauri 后端负责系统剪贴板读写、SQLite 存储、去重、防回流与事件广播；React 前端负责日期侧栏、记录列表、详情面板和用户操作，并接入 TailwindCSS + shadcn/ui 作为界面基础。MVP 采用轮询监听，不实现搜索、监听开关、清空日期、系统托盘和设置页。

**Tech Stack:** Tauri 2、Rust 2021、React 19、TypeScript、Vite、TailwindCSS v4、shadcn/ui、`arboard`、`rusqlite`、`sha2`、`chrono`。

---

## Scope Boundary

本计划只覆盖 Milestone 1：基础闭环。

**包含：**
- 初始化本地 SQLite 数据库。
- 轮询读取系统文本剪贴板。
- 基于 `content_hash` 去重，重复内容更新 `last_copied_at` 和 `copy_count`。
- 防止应用自身“复制回剪贴板”造成监听回流。
- 按日期查询记录分组和记录列表。
- 查看完整内容、复制回剪贴板、删除单条记录。
- React 三栏界面：日期侧栏、记录列表、详情面板。

**不包含：**
- 搜索、暂停/恢复监听 UI、清空当前日期。
- 系统托盘、窗口隐藏策略、开机启动。
- 保留天数、最大记录数、敏感内容过滤设置。
- 图片、文件、富文本、标签、收藏、云同步。

## File Structure

- Modify `src-tauri/Cargo.toml`：增加剪贴板、SQLite、hash、时间依赖。
- Modify `src-tauri/src/lib.rs`：注册后端状态、命令和启动监听。
- Create `src-tauri/src/clipboard/mod.rs`：剪贴板后端模块入口。
- Create `src-tauri/src/clipboard/error.rs`：可序列化后端错误。
- Create `src-tauri/src/clipboard/models.rs`：后端 DTO。
- Create `src-tauri/src/clipboard/hash.rs`：文本标准化、hash、preview。
- Create `src-tauri/src/clipboard/repository.rs`：SQLite schema、查询、写入、软删除。
- Create `src-tauri/src/clipboard/service.rs`：业务逻辑、去重、防回流、剪贴板读写。
- Create `src-tauri/src/clipboard/monitor.rs`：轮询监听和事件广播。
- Create `src-tauri/src/clipboard/commands.rs`：Tauri command handlers。
- Create `src/types/clipboard.ts`：前端 DTO 类型。
- Create `src/api/clipboard.ts`：前端 Tauri API 封装。
- Modify `src/App.tsx`：替换模板页为剪贴板主界面。
- Create `src/index.css`：TailwindCSS v4 与 shadcn/ui 主题变量入口。
- Create `components.json`：shadcn/ui CLI 配置。
- Create `src/lib/utils.ts`：shadcn/ui `cn` 工具。
- Create `src/components/ui/button.tsx`：shadcn/ui Button。
- Create `src/components/ui/card.tsx`：shadcn/ui Card。
- Create `src/components/ui/badge.tsx`：shadcn/ui Badge。
- Modify `src/App.css`：不再承载主要布局，仅保留兼容或清空。

## Task 1: Backend Dependency Baseline

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Verify: `src-tauri/Cargo.lock`

- [ ] **Step 1: Add dependencies**

将 `src-tauri/Cargo.toml` 的 `[dependencies]` 调整为：

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
arboard = "3"
chrono = { version = "0.4", features = ["serde"] }
rusqlite = { version = "0.32", features = ["bundled"] }
sha2 = "0.10"
```

- [ ] **Step 2: Resolve dependency lockfile**

Run:

```powershell
pnpm tauri info
```

Expected:
- 命令可完成依赖解析。
- `src-tauri/Cargo.lock` 出现 `arboard`、`chrono`、`rusqlite`、`sha2`。

- [ ] **Step 3: Dependency review**

Run:

```powershell
git diff -- src-tauri/Cargo.toml src-tauri/Cargo.lock
```

Expected:
- 只包含剪贴板 MVP 所需依赖变化。
- 不改 Tauri 根配置、不改前端依赖。

## Task 2: Backend Types, Errors, and Hash Helpers

**Files:**
- Create: `src-tauri/src/clipboard/mod.rs`
- Create: `src-tauri/src/clipboard/error.rs`
- Create: `src-tauri/src/clipboard/models.rs`
- Create: `src-tauri/src/clipboard/hash.rs`

- [ ] **Step 1: Create module entry**

Create `src-tauri/src/clipboard/mod.rs`:

```rust
pub mod commands;
pub mod error;
pub mod hash;
pub mod models;
pub mod monitor;
pub mod repository;
pub mod service;
```

- [ ] **Step 2: Define serializable command error**

Create `src-tauri/src/clipboard/error.rs` with:

```rust
#[derive(Debug)]
pub enum ClipboardError {
    Clipboard(String),
    Database(String),
    Io(String),
    NotFound(i64),
    Runtime(String),
}
```

Implementation requirements:
- Implement `std::fmt::Display` with clear user-readable messages.
- Implement `std::error::Error`.
- Implement `serde::Serialize` by serializing `self.to_string()`.
- Implement `From<rusqlite::Error>`, `From<std::io::Error>`, `From<arboard::Error>`.

- [ ] **Step 3: Define backend DTOs**

Create `src-tauri/src/clipboard/models.rs`:

```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub content: String,
    pub preview: String,
    pub content_hash: String,
    pub created_at: String,
    pub last_copied_at: String,
    pub copy_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardDateGroup {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardChangeEvent {
    pub item: ClipboardItem,
}
```

- [ ] **Step 4: Implement hash helpers**

Create `src-tauri/src/clipboard/hash.rs` with these public functions:

```rust
pub fn normalize_text(content: &str) -> String;
pub fn content_hash(content: &str) -> String;
pub fn preview(content: &str) -> String;
```

Implementation requirements:
- `normalize_text` only normalizes line endings: `\r\n` and `\r` become `\n`.
- Do not trim leading/trailing whitespace.
- `content_hash` uses SHA-256 over normalized text and returns lowercase hex.
- `preview` takes first 100 Unicode scalar values and appends `…` only when truncated.

- [ ] **Step 5: Add helper tests**

Add tests in `hash.rs`:
- `normalizes_line_endings_without_trimming`
- `hashes_equivalent_line_endings_equally`
- `truncates_long_preview`

Run:

```powershell
cd src-tauri
cargo test clipboard::hash
```

Expected: all helper tests pass.

## Task 3: SQLite Repository

**Files:**
- Create: `src-tauri/src/clipboard/repository.rs`

- [ ] **Step 1: Define schema**

Implement `init_database(path: &Path) -> Result<(), ClipboardError>`.

Schema:

```sql
CREATE TABLE IF NOT EXISTS clipboard_items (
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
```

- [ ] **Step 2: Implement repository functions**

Create these functions in `repository.rs`:

```rust
pub fn init_database(path: &Path) -> Result<(), ClipboardError>;
pub fn upsert_text_item(path: &Path, content: &str, content_hash: &str, now: &str) -> Result<ClipboardItem, ClipboardError>;
pub fn list_date_groups(path: &Path) -> Result<Vec<ClipboardDateGroup>, ClipboardError>;
pub fn list_items_by_date(path: &Path, date: &str) -> Result<Vec<ClipboardItem>, ClipboardError>;
pub fn get_item_by_id(path: &Path, id: i64) -> Result<ClipboardItem, ClipboardError>;
pub fn soft_delete_item(path: &Path, id: i64, now: &str) -> Result<(), ClipboardError>;
```

Behavior requirements:
- `upsert_text_item` inserts a new text row when no active row has the same hash.
- If active row exists, update `last_copied_at` and increment `copy_count`.
- `list_date_groups` groups by `substr(created_at, 1, 10)` and sorts newest first.
- `list_items_by_date` filters active rows by date and sorts by `last_copied_at DESC, id DESC`.
- `soft_delete_item` sets `deleted_at`; no physical delete in Milestone 1.
- `get_item_by_id` returns `ClipboardError::NotFound(id)` for deleted or missing rows.

- [ ] **Step 3: Add repository tests**

Add tests in `repository.rs`:
- `inserts_and_lists_items_by_date`
- `deduplicates_active_hashes`
- `soft_deleted_items_are_hidden`

Run:

```powershell
cd src-tauri
cargo test clipboard::repository
```

Expected:
- New item can be inserted and listed under `2026-05-26`.
- Same hash updates one row to `copy_count = 2`.
- Soft-deleted item no longer appears in detail or list queries.

## Task 4: Clipboard Service and Monitor

**Files:**
- Create: `src-tauri/src/clipboard/service.rs`
- Create: `src-tauri/src/clipboard/monitor.rs`

- [ ] **Step 1: Implement service state**

Create `ClipboardService` in `service.rs`:

```rust
pub struct ClipboardService {
    database_path: PathBuf,
    last_seen_hash: Mutex<Option<String>>,
    last_app_write: Mutex<Option<AppWriteGuard>>,
}
```

`AppWriteGuard` fields:
- `hash: String`
- `written_at: Instant`

- [ ] **Step 2: Implement service methods**

Add these methods:

```rust
impl ClipboardService {
    pub fn new(database_path: PathBuf) -> Result<Self, ClipboardError>;
    pub fn capture_current_clipboard(&self) -> Result<Option<ClipboardItem>, ClipboardError>;
    pub fn list_date_groups(&self) -> Result<Vec<ClipboardDateGroup>, ClipboardError>;
    pub fn list_items_by_date(&self, date: &str) -> Result<Vec<ClipboardItem>, ClipboardError>;
    pub fn get_item(&self, id: i64) -> Result<ClipboardItem, ClipboardError>;
    pub fn copy_item(&self, id: i64) -> Result<(), ClipboardError>;
    pub fn delete_item(&self, id: i64) -> Result<(), ClipboardError>;
}
```

Behavior requirements:
- `new` calls `repository::init_database`.
- `capture_current_clipboard` reads text with `arboard::Clipboard::get_text`.
- Empty text or `ContentNotAvailable` returns `Ok(None)`.
- Same as `last_seen_hash` returns `Ok(None)`.
- Same as `last_app_write.hash` within 2 seconds returns `Ok(None)`.
- Valid new content calls `repository::upsert_text_item` with `Local::now().to_rfc3339()`.
- `copy_item` loads item content, writes it to system clipboard, and records `last_app_write`.
- `delete_item` uses soft delete.

- [ ] **Step 3: Implement polling monitor**

Create `start_clipboard_monitor(app_handle: AppHandle, service: Arc<ClipboardService>)` in `monitor.rs`.

Behavior requirements:
- Spawn one background thread.
- Poll every 800ms.
- On `Ok(Some(item))`, emit `clipboard:item-created` with `ClipboardChangeEvent { item }`.
- On `Ok(None)`, do nothing.
- On error, log with `eprintln!` and keep polling.

- [ ] **Step 4: Backend syntax check**

Run:

```powershell
cd src-tauri
cargo check
```

Expected:
- If `lib.rs` is not wired yet, unresolved-module errors are acceptable at this point.
- No syntax errors should remain inside `service.rs` or `monitor.rs`.

## Task 5: Tauri Commands and Startup Wiring

**Files:**
- Create: `src-tauri/src/clipboard/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Implement Tauri commands**

Create `commands.rs` with state wrapper:

```rust
pub struct ClipboardState(pub Arc<ClipboardService>);
```

Commands:

```rust
#[tauri::command]
pub fn list_clipboard_dates(state: State<'_, ClipboardState>) -> Result<Vec<ClipboardDateGroup>, ClipboardError>;

#[tauri::command]
pub fn list_clipboard_items(date: String, state: State<'_, ClipboardState>) -> Result<Vec<ClipboardItem>, ClipboardError>;

#[tauri::command]
pub fn get_clipboard_item(id: i64, state: State<'_, ClipboardState>) -> Result<ClipboardItem, ClipboardError>;

#[tauri::command]
pub fn copy_clipboard_item(id: i64, state: State<'_, ClipboardState>) -> Result<(), ClipboardError>;

#[tauri::command]
pub fn delete_clipboard_item(id: i64, state: State<'_, ClipboardState>) -> Result<(), ClipboardError>;
```

Each command should delegate directly to `ClipboardService`.

- [ ] **Step 2: Replace template backend wiring**

Modify `src-tauri/src/lib.rs`:
- Remove the template `greet` command.
- Add `mod clipboard;`.
- In `.setup`, compute database path with `app.path().app_data_dir()?.join("clipboard.sqlite")`.
- Create `Arc<ClipboardService>`.
- Call `start_clipboard_monitor(app.handle().clone(), Arc::clone(&service))`.
- Register `ClipboardState(service)` with `app.manage(...)`.
- Register all five clipboard commands in `tauri::generate_handler![...]`.

- [ ] **Step 3: Verify backend**

Run:

```powershell
cd src-tauri
cargo test clipboard
cargo check
```

Expected:
- Clipboard unit tests pass.
- Tauri backend checks successfully.
- No `greet` command remains registered.

## Task 6: TailwindCSS and shadcn/ui Setup

**Files:**
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `tsconfig.json`
- Modify: `tsconfig.node.json`
- Modify: `vite.config.ts`
- Modify: `src/main.tsx`
- Create: `src/index.css`
- Create: `components.json`
- Create: `src/lib/utils.ts`
- Create: `src/components/ui/button.tsx`
- Create: `src/components/ui/card.tsx`
- Create: `src/components/ui/badge.tsx`

- [ ] **Step 1: Install UI dependencies**

Run:

```powershell
pnpm add tailwindcss @tailwindcss/vite class-variance-authority clsx tailwind-merge lucide-react
``` 

Expected: `package.json` and `pnpm-lock.yaml` include TailwindCSS v4, shadcn utility dependencies, and Lucide icons.

- [ ] **Step 2: Configure TypeScript path alias**

Update `tsconfig.json` and `tsconfig.node.json` with `baseUrl: "."` and path alias `"@/*": ["./src/*"]`.

- [ ] **Step 3: Configure Vite Tailwind plugin**

Update `vite.config.ts` to import `@tailwindcss/vite` and include `tailwindcss()` in `plugins` after `react()` before Tauri server options.

- [ ] **Step 4: Create Tailwind theme entry**

Create `src/index.css` with `@import "tailwindcss";`, shadcn-compatible CSS variables, base body styling, and dark-mode variables.

- [ ] **Step 5: Import global CSS**

Update `src/main.tsx` to import `./index.css` before `App`.

- [ ] **Step 6: Add shadcn config and base components**

Create `components.json`, `src/lib/utils.ts`, and local shadcn-style `Button`, `Card`, and `Badge` components using `class-variance-authority`, `clsx`, and `tailwind-merge`.

- [ ] **Step 7: Verify UI setup**

Run:

```powershell
pnpm build
``` 

Expected: Tailwind/shadcn setup type-checks. If `App.tsx` is still template UI, build should still pass.

## Task 7: Frontend Types and API Layer

**Files:**
- Create: `src/types/clipboard.ts`
- Create: `src/api/clipboard.ts`

- [ ] **Step 1: Create frontend types**

Create `src/types/clipboard.ts`:

```ts
export interface ClipboardItem {
  id: number;
  contentType: "text";
  content: string;
  preview: string;
  contentHash: string;
  createdAt: string;
  lastCopiedAt: string;
  copyCount: number;
}

export interface ClipboardDateGroup {
  date: string;
  count: number;
}

export interface ClipboardChangeEvent {
  item: ClipboardItem;
}
```

- [ ] **Step 2: Create API wrapper**

Create `src/api/clipboard.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ClipboardChangeEvent, ClipboardDateGroup, ClipboardItem } from "../types/clipboard";

export function listClipboardDates(): Promise<ClipboardDateGroup[]>;
export function listClipboardItems(date: string): Promise<ClipboardItem[]>;
export function getClipboardItem(id: number): Promise<ClipboardItem>;
export function copyClipboardItem(id: number): Promise<void>;
export function deleteClipboardItem(id: number): Promise<void>;
export function onClipboardItemCreated(handler: (event: ClipboardChangeEvent) => void): Promise<UnlistenFn>;
```

Implementation requirements:
- Use command names exactly as backend commands: `list_clipboard_dates`, `list_clipboard_items`, `get_clipboard_item`, `copy_clipboard_item`, `delete_clipboard_item`.
- `onClipboardItemCreated` listens to `clipboard:item-created` and passes `event.payload` to the handler.

- [ ] **Step 3: Frontend type check**

Run after Task 7 or together with Task 7:

```powershell
pnpm build
```

Expected: TypeScript accepts the API wrappers and imported types.

## Task 8: React shadcn MVP Workspace

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Replace template screen**

Replace the Tauri template UI with a three-panel clipboard workspace:
- Left: app title and date list.
- Middle: records for selected date.
- Right: full content, metadata, copy/delete actions.

State requirements:

```ts
const [dates, setDates] = useState<ClipboardDateGroup[]>([]);
const [items, setItems] = useState<ClipboardItem[]>([]);
const [selectedDate, setSelectedDate] = useState(todayKey());
const [selectedItemId, setSelectedItemId] = useState<number | null>(null);
const [message, setMessage] = useState("复制一段文本后，它会自动出现在这里。");
```

Required helpers:
- `todayKey()` returns local date key `YYYY-MM-DD`.
- `loadDates()` calls `listClipboardDates()`.
- `loadItems(date)` calls `listClipboardItems(date)` and selects the first item.
- `selectedItem` derives from `items` and `selectedItemId`.

- [ ] **Step 2: Wire event refresh**

Add an effect that calls `onClipboardItemCreated`.

Behavior requirements:
- Always refresh date groups when event arrives.
- Refresh list if `event.item.createdAt.startsWith(selectedDate)`.
- Show message `已捕获新的剪贴板文本。`.
- Dispose listener on unmount.

- [ ] **Step 3: Implement user actions**

Implement handlers:
- `handleDateClick(date)` sets selected date and loads its list.
- `handleCopy(item)` calls `copyClipboardItem(item.id)` and sets success message.
- `handleDelete(item)` calls `deleteClipboardItem(item.id)`, removes the item locally, refreshes date groups, and selects the next item.

- [ ] **Step 4: Remove template imports**

Ensure `src/App.tsx` no longer imports:
- `reactLogo`
- `@tauri-apps/api/core` directly
- template `greet` state or command

Run:

```powershell
pnpm build
```

Expected: TypeScript build succeeds after styling is also present.

## Task 9: Minimal CSS Cleanup

**Files:**
- Modify: `src/App.css`

- [ ] **Step 1: Replace template styles**

Replace all existing Tauri template CSS.

Required selectors:
- `.clipboard-shell`
- `.date-sidebar`
- `.brand-block`
- `.eyebrow`
- `.date-list`
- `.date-card`
- `.date-card.active`
- `.item-list-panel`
- `.detail-panel`
- `.panel-header`
- `.status-pill`
- `.item-list`
- `.item-card`
- `.item-card.active`
- `.item-preview`
- `.item-meta`
- `.empty-state`
- `.detail-content`
- `.metadata-grid`
- `.action-row`
- `.danger`
- `.message-line`

Layout requirements:
- Root shell uses CSS grid with three columns: date sidebar, item list, detail panel.
- App fills viewport height.
- List and detail content areas scroll independently.
- Detail `pre` uses `white-space: pre-wrap` and `word-break: break-word`.
- Minimum window width can remain desktop-oriented; no mobile layout required for MVP.

- [ ] **Step 2: Verify frontend build**

Run:

```powershell
pnpm build
```

Expected: TypeScript and Vite build succeed.

## Task 10: Integration Verification

**Files:**
- Verify: `src-tauri/src/lib.rs`
- Verify: `src-tauri/src/clipboard/*`
- Verify: `src/App.tsx`
- Verify: `src/App.css`

- [ ] **Step 1: Backend verification**

Run:

```powershell
cd src-tauri
cargo test clipboard
cargo check
```

Expected:
- Hash and repository tests pass.
- Backend compiles.

- [ ] **Step 2: Frontend verification**

Run from repo root:

```powershell
pnpm build
```

Expected:
- TypeScript compiles.
- Vite production build succeeds.

- [ ] **Step 3: Desktop smoke test**

Run from repo root:

```powershell
pnpm tauri:dev
```

Manual acceptance criteria:
1. App opens with clipboard toolbox UI instead of the Tauri template.
2. Copying a new text snippet outside the app creates a record under today.
3. Copying the same text again updates one record's copy count instead of adding duplicates.
4. Restarting the app keeps existing records visible.
5. Clicking `复制回剪贴板` writes content to the system clipboard.
6. The app's own copy-back action does not create a duplicate record within two seconds.
7. Clicking `删除记录` removes the row from the list and date count.

## Implementation Order

Recommended order:
1. Task 1: dependencies.
2. Task 2: DTO/error/hash helpers.
3. Task 3: repository and tests.
4. Task 4: service and monitor.
5. Task 5: commands and startup wiring.
6. Task 6: frontend types/API.
7. Task 7: React workspace.
8. Task 8: styling.
9. Task 9: full verification.

Do not start Milestone 2 features until all Task 9 acceptance criteria pass.

## Risk Notes

- Clipboard polling is intentionally simple for MVP; if CPU or battery usage becomes visible, optimize after Milestone 1.
- SQLite writes happen from the monitor thread and command handlers; open short-lived connections per repository call to avoid sharing a connection across threads.
- `created_at` and `last_copied_at` use local RFC3339 strings; date grouping relies on the first 10 characters.
- App data directory is the correct production storage location; tests should use OS temp directories.
- Soft delete preserves data on disk for MVP; physical purge can be added later if product requires it.

## Self-Review

- Spec coverage: `docs/clipboard-toolbox-design.md` Milestone 1 maps to dependencies, data model, database, monitor, commands, UI, and verification tasks.
- Placeholder scan: no `TBD`, no deferred implementation inside Milestone 1, and every task has concrete files, functions, commands, and expected results.
- Type consistency: Rust DTOs use Serde `camelCase`, matching frontend TypeScript fields and API wrappers.
- Scope control: Milestone 2 and 3 features are explicitly excluded from this plan.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-26-clipboard-mvp-foundation.md`.

Execution options:

1. **Inline Execution** - Use `superpowers:executing-plans` in this session and implement task-by-task with checkpoints.
2. **Manual Execution** - Use this document as the implementation checklist and run the verification commands at the end of each task.



