# Clipboard Milestone 2 Experience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `docs/clipboard-toolbox-design.md` 的 Milestone 2：搜索、监听开关、清空当前日期、状态反馈和重复记录计数体验完善。

**Architecture:** 后端在现有 SQLite repository 和 `ClipboardService` 上扩展搜索、按日期批量软删除、监听状态管理与事件广播；前端在现有 TailwindCSS + shadcn/ui 三栏界面中加入搜索栏、监听开关、清空日期按钮、加载/错误/空状态和更明显的 copy count 展示。

**Tech Stack:** Tauri 2、Rust 2021、React 19、TypeScript、Vite、TailwindCSS v4、shadcn/ui、本地 SQLite。

---

## Scope Boundary

包含：
- `search_clipboard_items(keyword: string): ClipboardItem[]`
- `clear_clipboard_items_by_date(date: string): void`
- `set_clipboard_monitor_enabled(enabled: boolean): void`
- `get_clipboard_monitor_status(): { enabled: boolean }`
- `clipboard:item-updated`、`clipboard:item-deleted`、`clipboard:monitor-status-changed` 事件。
- 前端跨日期搜索结果展示，不受日期侧栏限制。
- 前端监听开关、清空当前日期、加载状态、错误提示、空状态、重复计数展示。

不包含：
- 系统托盘、开机启动、保留天数、最大记录数、敏感内容过滤。
- 图片/文件/富文本支持。
- 持久化监听开关到 settings 表；Milestone 2 仅做运行时状态。

## Task 1: Backend Search and Clear Repository

**Files:**
- Modify: `src-tauri/src/clipboard/models.rs`
- Modify: `src-tauri/src/clipboard/repository.rs`

- [ ] Add `ClipboardMonitorStatus { enabled: bool }` DTO.
- [ ] Add `search_items(path, keyword)` with `%keyword%` search over `content` and `preview`, sorted by `last_copied_at DESC, id DESC`.
- [ ] Add `soft_delete_items_by_date(path, date, now)` using `substr(created_at, 1, 10) = ?` and `deleted_at IS NULL`.
- [ ] Add repository tests for search and clear-by-date.

## Task 2: Backend Service, Commands, Events

**Files:**
- Modify: `src-tauri/src/clipboard/service.rs`
- Modify: `src-tauri/src/clipboard/commands.rs`
- Modify: `src-tauri/src/clipboard/monitor.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] Add runtime monitor state to `ClipboardService` with `Mutex<bool>`.
- [ ] Make `capture_current_clipboard` return `Ok(None)` when monitor is disabled.
- [ ] Add service methods: `search_items`, `clear_items_by_date`, `set_monitor_enabled`, `monitor_status`.
- [ ] Emit `clipboard:item-created` when `copy_count == 1`; emit `clipboard:item-updated` otherwise.
- [ ] Add Tauri commands for search, clear date, set monitor status, get monitor status.
- [ ] Register new commands in `src-tauri/src/lib.rs`.

## Task 3: Frontend API and Types

**Files:**
- Modify: `src/types/clipboard.ts`
- Modify: `src/api/clipboard.ts`

- [ ] Add `ClipboardMonitorStatus` type.
- [ ] Add API wrappers for new commands.
- [ ] Add listeners for updated, deleted, and monitor-status-changed events.

## Task 4: Frontend Milestone 2 UI

**Files:**
- Modify: `src/App.tsx`

- [ ] Add search input and cross-date search result mode.
- [ ] Add monitor on/off button.
- [ ] Add clear current date button with disabled state when empty/searching.
- [ ] Improve loading, error, and empty states.
- [ ] Make duplicate copy count visible with badges.
- [ ] Refresh data correctly after created, updated, deleted, clear, and monitor status events.

## Task 5: Verification

**Commands:**
- `cd src-tauri; cargo test clipboard`
- `cd src-tauri; cargo check`
- `pnpm.cmd build`

**Manual smoke:**
- 搜索能跨日期返回记录。
- 监听关闭后复制文本不会新增记录；开启后恢复新增。
- 清空当前日期后列表和日期计数同步更新。
- 重复复制同一内容只更新计数并刷新 UI。

## Self-Review

- Spec coverage: 覆盖 Milestone 2 的搜索、监听开关、清空日期、状态反馈和重复计数展示。
- Placeholder scan: 无 TBD/TODO 占位。
- Scope control: 桌面增强和设置持久化留给 Milestone 3。
