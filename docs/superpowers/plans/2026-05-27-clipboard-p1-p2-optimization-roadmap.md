# Clipboard P1/P2 Optimization Roadmap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 按 P1/P2 优先级补齐剪贴板工具箱的设置可靠性、可解释性与数据维护能力。

**Architecture:** P1 聚焦“不误记、不丢设置、路径可用、用户知道发生了什么”，优先修改设置持久化、存储目录选择/校验与跳过原因事件。P2 聚焦桌面菜单语义、数据维护与高级隐私规则，在不破坏现有数据模型的前提下增加物理清理和自定义敏感规则。

**Tech Stack:** Tauri 2、Rust 2021、SQLite、React 19、TypeScript、TailwindCSS v4、shadcn-style Button/Card/Badge/Switch。

---

## Priority Overview

### P1：可靠性与可解释性

- 持久化监听状态，避免用户关闭监听后重启又自动开启。
- 增加目录选择器与路径可写校验，降低自定义存储目录配置错误。
- 增加剪贴板内容跳过原因事件，让超长/敏感内容被跳过时有反馈。

### P2：桌面语义与维护能力

- 修正“大小”菜单语义，改为明确的“恢复默认大小”，或实现真正的 resize dragging。
- 增加物理删除/压缩清理入口，减少软删除数据长期膨胀。
- 增加敏感规则自定义正则，为不同用户的 token/password 规则留出口。

---

## Task 1: P1 Persist Monitor State

**Files:**
- Modify: `src-tauri/src/clipboard/models.rs`
- Modify: `src-tauri/src/clipboard/settings.rs`
- Modify: `src-tauri/src/clipboard/service.rs`
- Modify: `src-tauri/src/clipboard/commands.rs`
- Modify: `src-tauri/src/clipboard/service_tests.rs`
- Modify: `src/types/clipboard.ts`
- Modify: `src/components/clipboard/DesktopSettingsPanel.tsx`
- Modify: `docs/clipboard-toolbox-design.md`

- [x] **Step 1: Extend stored settings with monitor state**

  In `src-tauri/src/clipboard/models.rs`, add `monitor_enabled: bool` to `DesktopSettings`, `DesktopSettingsUpdate`, and `StoredSettings`.

- [x] **Step 2: Add default and persistence keys**

  In `src-tauri/src/clipboard/settings.rs`, add `DEFAULT_MONITOR_ENABLED: bool = true`, read key `monitor_enabled`, and persist it through `update_stored_settings`.

- [x] **Step 3: Initialize service from persisted value**

  In `src-tauri/src/clipboard/service.rs`, initialize `monitor_enabled: Mutex<bool>` from `settings::get_stored_settings(&default_database_path)?.monitor_enabled`.

- [x] **Step 4: Persist monitor toggle command**

  Update `ClipboardService::set_monitor_enabled` to write `monitor_enabled` into default settings DB when the user toggles listening.

- [x] **Step 5: Keep settings panel and runtime status in sync**

  Add `monitorEnabled` to `DesktopSettings` in `src/types/clipboard.ts`; when settings load or monitor event fires, keep `desktopSettings.monitorEnabled` and `monitorEnabled` aligned.

- [x] **Step 6: Add regression tests**

  Add tests in `src-tauri/src/clipboard/service_tests.rs`:
  - default monitor state is `true` on first startup;
  - after `set_monitor_enabled(false)`, a new `ClipboardService` created with the same default DB reports `false`.

- [x] **Step 7: Verify P1 task 1**

  Run:
  - `cd src-tauri; cargo test clipboard::service_tests`
  - `cd src-tauri; cargo check`
  - `pnpm.cmd build`

---

## Task 2: P1 Directory Picker and Storage Path Validation

**Files:**
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/clipboard/error.rs`
- Modify: `src-tauri/src/clipboard/service.rs`
- Modify: `src-tauri/src/clipboard/commands.rs`
- Modify: `src/api/clipboard.ts`
- Modify: `src/components/clipboard/DesktopSettingsPanel.tsx`
- Create: `src-tauri/src/clipboard/storage_path_tests.rs`
- Modify: `src-tauri/src/clipboard/mod.rs`
- Modify: `docs/clipboard-toolbox-design.md`

- [x] **Step 1: Add dialog plugin dependencies**

  Add dependencies:
  - frontend: `@tauri-apps/plugin-dialog`
  - backend: `tauri-plugin-dialog = "2"`

  Register `.plugin(tauri_plugin_dialog::init())` in `src-tauri/src/lib.rs`.

- [x] **Step 2: Add path validation command**

  Add Tauri command `validate_storage_dir(storage_dir: String) -> Result<(), ClipboardError>`.

  Validation rules:
  - blank string is valid and means default app data directory;
  - non-blank path must be a directory or creatable;
  - app must be able to create and remove a small temp file in that directory;
  - validation must not create `clipboard.sqlite`.

- [x] **Step 3: Validate before switching active database**

  In `ClipboardService::update_desktop_settings`, validate `storage_dir` before `repository::init_database(&database_path)` and before persisting settings.

- [x] **Step 4: Add directory picker UI**

  In `DesktopSettingsPanel`, import `open` from `@tauri-apps/plugin-dialog` and add “选择目录” next to the storage path input.

  Expected behavior:
  - choosing a directory fills the draft path;
  - “保存路径” calls `validate_storage_dir` first;
  - validation error is shown inline near the storage path field.

- [x] **Step 5: Add storage path tests**

  Add tests in `src-tauri/src/clipboard/storage_path_tests.rs`:
  - blank path passes;
  - temp directory passes;
  - path whose parent is a file fails;
  - successful validation does not create `clipboard.sqlite`.

- [x] **Step 6: Verify P1 task 2**

  Run:
  - `cd src-tauri; cargo test clipboard::storage_path_tests`
  - `cd src-tauri; cargo check`
  - `pnpm.cmd build`

---

## Task 3: P1 Skipped Clipboard Feedback

**Files:**
- Modify: `src-tauri/src/clipboard/models.rs`
- Modify: `src-tauri/src/clipboard/service.rs`
- Modify: `src-tauri/src/clipboard/monitor.rs`
- Modify: `src-tauri/src/clipboard/settings.rs`
- Modify: `src-tauri/src/clipboard/service_tests.rs`
- Modify: `src/types/clipboard.ts`
- Modify: `src/api/clipboard.ts`
- Modify: `src/hooks/useClipboardEvents.ts`
- Modify: `src/hooks/useClipboardWorkspace.ts`
- Modify: `docs/clipboard-toolbox-design.md`

- [x] **Step 1: Replace boolean skip with typed reason**

  Introduce `ClipboardSkipReason` values:
  - `empty`
  - `monitorDisabled`
  - `tooLong`
  - `secretLike`
  - `duplicate`
  - `appWriteBack`

  Keep `empty`, `duplicate`, and `appWriteBack` silent in UI; surface `tooLong` and `secretLike`.

- [x] **Step 2: Add skip event model**

  Add `ClipboardSkippedEvent { reason, content_length, max_text_length }` to `src-tauri/src/clipboard/models.rs` with camelCase serialization.

- [x] **Step 3: Emit skip event from monitor**

  Change capture result from `Result<Option<ClipboardItem>, ClipboardError>` to a small enum such as `CaptureOutcome::CreatedOrUpdated(item) | Skipped(reason)` so `monitor.rs` can emit `clipboard:item-skipped` for surfaced reasons.

- [x] **Step 4: Show user-facing feedback**

  In `useClipboardEvents`, listen to `clipboard:item-skipped`. In `useClipboardWorkspace`, set message:
  - too long: `该剪贴板内容超过单条文本上限，已跳过。`
  - secret-like: `疑似敏感内容已按设置跳过。`

- [x] **Step 5: Add tests**

  Add service tests for `tooLong` and `secretLike` outcomes without requiring actual system clipboard access by extracting pure decision logic into `settings.rs` or a small service helper.

- [x] **Step 6: Verify P1 task 3**

  Run:
  - `cd src-tauri; cargo test clipboard`
  - `cd src-tauri; cargo check`
  - `pnpm.cmd build`

---

## Task 4: P2 Native Menu Semantics

**Files:**
- Modify: `src-tauri/src/desktop.rs`
- Modify: `docs/clipboard-toolbox-design.md`

- [x] **Step 1: Rename ambiguous menu label**

  Change system menu label `大小` to `恢复默认大小` if keeping the current fixed-size behavior.

- [x] **Step 2: Keep dimensions named constants**

  Keep `DEFAULT_WINDOW_WIDTH` and `DEFAULT_WINDOW_HEIGHT` as constants and document their meaning in `desktop.rs` only if needed by maintainers.

- [x] **Step 3: Optional true resize dragging path**

  If true Windows-style resizing is required, replace fixed-size behavior with `window.start_resize_dragging(ResizeDirection::SouthEast)` and verify platform support manually on Windows. Current implementation keeps fixed default-size restore behavior.

- [x] **Step 4: Verify P2 task 4**

  Run:
  - `cd src-tauri; cargo check`
  - `pnpm.cmd build`

---

## Task 5: P2 Physical Cleanup for Soft Deletes

**Files:**
- Modify: `src-tauri/src/clipboard/repository.rs`
- Add: `src-tauri/src/clipboard/maintenance.rs`
- Modify: `src-tauri/src/clipboard/service.rs`
- Modify: `src-tauri/src/clipboard/commands.rs`
- Modify: `src-tauri/src/clipboard/repository_tests.rs`
- Modify: `src/api/clipboard.ts`
- Modify: `src/components/clipboard/DesktopSettingsPanel.tsx`
- Add: `src/components/clipboard/SettingsAdvancedActions.tsx`
- Modify: `docs/clipboard-toolbox-design.md`

- [x] **Step 1: Add repository cleanup function**

  Add `purge_deleted_items(path: &Path) -> Result<usize, ClipboardError>` that physically deletes rows where `deleted_at IS NOT NULL`.

- [x] **Step 2: Add optional SQLite vacuum**

  Add `vacuum_database(path: &Path) -> Result<(), ClipboardError>` and call it only from explicit user action, not automatically after each delete.

- [x] **Step 3: Add command and API wrapper**

  Add command `purge_deleted_clipboard_items(vacuum: bool): usize` and frontend API wrapper with the same semantics.

- [x] **Step 4: Add settings panel action**

  Add “清理已删除记录” button in `DesktopSettingsPanel`. Show result message with deleted row count.

- [x] **Step 5: Add repository tests**

  Add tests:
  - soft-deleted records are physically removed;
  - active records remain;
  - returned count matches removed rows.

- [x] **Step 6: Verify P2 task 5**

  Run:
  - `cd src-tauri; cargo test clipboard::repository_tests`
  - `cd src-tauri; cargo check`
  - `pnpm.cmd build`

---

## Task 6: P2 Custom Sensitive Regex Rules

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/clipboard/models.rs`
- Modify: `src-tauri/src/clipboard/settings.rs`
- Modify: `src-tauri/src/clipboard/service_tests.rs`
- Modify: `src/types/clipboard.ts`
- Modify: `src/components/clipboard/DesktopSettingsPanel.tsx`
- Modify: `docs/clipboard-toolbox-design.md`

- [x] **Step 1: Add regex dependency**

  Add Rust dependency `regex = "1"`.

- [x] **Step 2: Add stored setting**

  Add `custom_secret_patterns: String` to settings. Store newline-separated regex patterns in `app_settings`.

- [x] **Step 3: Extend filter logic**

  In `settings::content_skip_reason`, if `ignore_password_like_text` is enabled, evaluate built-in heuristics first, then user regex patterns. Invalid regex patterns fail settings update and are surfaced as an error.

- [x] **Step 4: Add UI editor**

  Add a textarea in settings panel with helper text: “每行一个 Rust regex；仅在敏感内容过滤开启时生效。”

- [x] **Step 5: Add validation and tests**

  Add tests:
  - valid custom regex skips matching content;
  - non-matching content is kept;
  - invalid regex fails settings update or is rejected by frontend validation.

- [x] **Step 6: Verify P2 task 6**

  Run:
  - `cd src-tauri; cargo test clipboard::service_tests`
  - `cd src-tauri; cargo check`
  - `pnpm.cmd build`

---

## Final Verification

- [x] Run `cd src-tauri; cargo test clipboard` and confirm all clipboard tests pass.
- [x] Run `cd src-tauri; cargo check` and confirm Rust compiles.
- [x] Run `pnpm.cmd build` and confirm TypeScript/Vite build succeeds.
- [x] Run `git diff --check` and confirm no whitespace errors.
- [x] Update `docs/clipboard-toolbox-design.md` section 17 after each completed task.

## Suggested Commit Order

1. `feat(clipboard): 持久化监听状态`
2. `feat(clipboard): 增加存储目录选择与校验`
3. `feat(clipboard): 增加跳过原因反馈`
4. `fix(desktop): 明确窗口大小菜单语义`
5. `feat(clipboard): 增加已删除记录清理`
6. `feat(clipboard): 支持自定义敏感规则`
