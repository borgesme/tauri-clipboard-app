# Clipboard Milestone 3 Desktop Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `docs/clipboard-toolbox-design.md` 的 Milestone 3：系统托盘、窗口隐藏/显示策略、开机启动配置、保留天数和最大记录数配置。

**Architecture:** Rust/Tauri 后端新增桌面增强模块和设置持久化能力，系统托盘负责后台工具入口，窗口关闭默认隐藏到托盘；SQLite `app_settings` 表保存用户设置，剪贴板写入和设置更新时执行保留策略清理。React 前端在现有 shadcn/Tailwind 界面中加入桌面设置区，调用后端命令管理 autostart、隐藏窗口和保留策略。

**Tech Stack:** Tauri 2 tray-icon、tauri-plugin-autostart、Rust 2021、SQLite、React 19、TypeScript、TailwindCSS v4、shadcn/ui。

---

## Scope Boundary

包含：
- 系统托盘图标与菜单：显示窗口、隐藏窗口、退出。
- 左键点击托盘显示并聚焦主窗口。
- 点击窗口关闭按钮时隐藏窗口，不退出应用。
- 后端命令：显示窗口、隐藏窗口、获取/更新桌面设置。
- 开机启动配置：通过 Tauri autostart 插件启用/禁用，并在设置中展示。
- `app_settings` 表：保存 `retention_days`、`max_record_count`、`autostart_enabled`。
- 保留策略：按保留天数和最大记录数软删除旧记录。
- 前端设置 UI：开机启动、隐藏窗口、保留天数、最大记录数。

不包含：
- 系统托盘动态菜单文本同步。
- 全局快捷键。
- 敏感内容过滤和导入导出。
- 多窗口和跨设备能力。

## Task 1: Desktop Dependencies and Tray Wiring

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/desktop.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] Enable Tauri `tray-icon` feature.
- [ ] Add `tauri-plugin-autostart = "2"`.
- [ ] Create tray menu with `show`, `hide`, `quit`.
- [ ] On left tray click, show and focus main window.
- [ ] On window close requested, prevent close and hide main window.

## Task 2: Settings Persistence and Retention Policy

**Files:**
- Modify: `src-tauri/src/clipboard/models.rs`
- Modify: `src-tauri/src/clipboard/repository.rs`
- Modify: `src-tauri/src/clipboard/repository_tests.rs`
- Modify: `src-tauri/src/clipboard/service.rs`

- [ ] Add `DesktopSettings` DTO.
- [ ] Create `app_settings` table in database initialization.
- [ ] Add get/update setting functions.
- [ ] Add retention cleanup by `retention_days` and `max_record_count`.
- [ ] Apply cleanup after capture and settings update.
- [ ] Add repository tests for settings defaults, settings update, and cleanup.

## Task 3: Desktop Commands and Frontend API

**Files:**
- Modify: `src-tauri/src/clipboard/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/types/clipboard.ts`
- Modify: `src/api/clipboard.ts`

- [ ] Add commands for `get_desktop_settings`, `update_desktop_settings`, `hide_main_window`, `show_main_window`.
- [ ] Wire autostart enable/disable/is_enabled through backend commands.
- [ ] Add frontend types and API wrappers.

## Task 4: Settings UI

**Files:**
- Create: `src/components/clipboard/DesktopSettingsPanel.tsx`
- Modify: `src/App.tsx`
- Modify: `src/hooks/useClipboardWorkspace.ts`

- [ ] Load desktop settings on startup.
- [ ] Add settings card to sidebar area.
- [ ] Add autostart toggle, hide window button, retention days input, max records input.
- [ ] Refresh view after settings update applies retention cleanup.

## Task 5: Verification

**Commands:**
- `cd src-tauri; cargo test clipboard`
- `cd src-tauri; cargo check`
- `pnpm.cmd build`
- `git diff --check`

**Manual smoke:**
- Close button hides window instead of exiting.
- Tray left-click or tray menu shows window again.
- Tray hide menu hides window.
- Tray quit exits application.
- Autostart toggle reflects OS state and can be changed.
- Retention/max settings save and old records are cleaned.

## Self-Review

- Spec coverage: 覆盖 Milestone 3 四项：托盘、隐藏/显示、开机启动、保留天数与最大记录数。
- Placeholder scan: 无 TBD/TODO 占位。
- Scope control: 不引入全局快捷键、导入导出、敏感过滤等后续能力。
