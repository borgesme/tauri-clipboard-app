# Settings Top Menu and Storage Directory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将设置入口移动到 Tauri 原生预定义菜单，设置面板使用 Switch 管理监听/开机启动，并支持用户配置本地 SQLite 存储目录。

**Architecture:** 后端创建 Tauri 原生菜单栏，包含“系统”和“设置”两组菜单；“系统”菜单包含还原、移动、大小、最小化、最大化、关闭，“设置”菜单触发前端打开设置浮层。后端把 `storage_dir` 作为稳定设置保存到默认数据库，同时将当前活动数据库路径切换为 `{storage_dir}/clipboard.sqlite`，不自动迁移旧数据。

**Tech Stack:** Tauri 2、Rust 2021、SQLite、React 19、TypeScript、TailwindCSS v4、shadcn-style Button/Card/Badge/Switch。

---

## Scope Boundary

包含：
- Tauri 原生菜单：系统、设置两组菜单。
- 系统菜单：还原、移动、大小、最小化、最大化、关闭。
- 设置菜单触发的浮层：监听 Switch、开机启动 Switch、隐藏到托盘、保留天数、最大记录数、自定义本地存储目录。
- 后端设置：`storage_dir`，空字符串表示默认应用数据目录。
- 存储目录语义：用户填写目录，应用使用该目录下的 `clipboard.sqlite`。
- 切换目录后初始化新数据库，并后续读写新数据库。

不包含：
- 自动迁移旧数据库。
- 文件夹选择对话框插件。
- 多配置文件或多数据库合并。

## Task 1: Backend Storage Directory Setting

- [x] Extend `DesktopSettings` and `DesktopSettingsUpdate` with `storage_dir`.
- [x] Keep `default_database_path` and mutable active `database_path` in `ClipboardService`.
- [x] Read `storage_dir` from default database on startup and initialize active database.
- [x] Persist `storage_dir` to default database when settings change.
- [x] Switch active database to `{storage_dir}/clipboard.sqlite` after update.
- [x] Add repository/service tests for default and custom storage directory behavior where possible.

## Task 2: Native Menu and Settings Panel

- [x] Add `Switch` UI component.
- [x] Add Tauri native menu with System and Settings groups.
- [x] Move settings panel out of sidebar and open it from native Settings menu event.
- [x] Remove monitor button from `DateSidebar`.
- [x] Add monitor Switch and storage directory input to settings page.
- [x] Update API/types for `storageDir`.

## Task 3: Verification

- [x] `cd src-tauri; cargo test clipboard`
- [x] `cd src-tauri; cargo check`
- [x] `pnpm.cmd build`
- [x] `git diff --check`

## Self-Review

- Setting entry is the native Settings menu only.
- Monitor switch is in settings only.
- Storage path uses directory semantics and does not claim migration.
