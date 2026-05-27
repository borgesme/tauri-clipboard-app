# Balanced Keyboard Page Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 参考 `docs/clipboard-main-app-balanced-keyboard.html`，把现有剪贴板页面重构为 Clip Studio 三栏工作台，并保留真实剪贴板数据、设置与复制/删除能力。

**Architecture:** `App.tsx` 只负责加载 workspace 与桌面设置事件，页面主体下沉到 focused clipboard components。列表、侧栏、右侧面板、详情弹窗与工具箱分文件实现，避免单文件膨胀。筛选、键盘导航、工具箱转换在前端本地处理，不改后端 schema。

**Tech Stack:** React 19、TypeScript、Tailwind CSS 4、现有 Tauri clipboard API、lucide-react。

---

### Task 1: 页面骨架

**Files:**
- Modify: `src/App.tsx`
- Create: `src/components/clipboard/ClipStudioPage.tsx`

- [x] 将 `App.tsx` 简化为 `useClipboardWorkspace()`、settings open event 与 `<ClipStudioPage />`。
- [x] 在 `ClipStudioPage.tsx` 建立 app shell 状态：active panel、active filter、detail modal、toolbox text/result。
- [x] 保留 `workspace.copyItem()`、`workspace.deleteItem()`、`workspace.updateSettings()` 等真实操作入口。

### Task 2: 三栏视觉结构

**Files:**
- Create: `src/components/clipboard/ClipStudioLayout.tsx`
- Create: `src/components/clipboard/ClipStudioList.tsx`
- Modify: `src/App.css`
- Modify: `src/index.css`

- [x] 复刻原型的窗口容器、顶部栏、左侧品牌导航、中间搜索/筛选/快捷键提示、底部状态栏。
- [x] 用现有 `ClipboardItem` 渲染中间列表，保持选中、复制、删除与详情入口。
- [x] 调整全局背景与最小尺寸，使用暖色 paper 风格变量。

### Task 3: 右侧面板

**Files:**
- Create: `src/components/clipboard/ClipStudioPanel.tsx`
- Reuse: `src/components/clipboard/DesktopSettingsPanel.tsx`

- [x] 实现日期看板：显示今日数量、总记录数、高频复用数量和日期按钮。
- [x] 实现文本工具箱：选中项送入、清理空格、大小写转换、Markdown 链接生成、复制结果。
- [x] 在设置 tab 中复用现有 `DesktopSettingsPanel`，不改后端设置结构。

### Task 4: 键盘优先交互

**Files:**
- Modify: `src/components/clipboard/ClipStudioPage.tsx`

- [x] `/` 聚焦搜索，`ArrowUp/ArrowDown` 在当前可见列表中移动选中。
- [x] `Enter` 复制当前选中项，`Space` 打开详情，`T` 送入工具箱。
- [x] `Escape` 关闭弹窗或清空搜索/筛选并返回历史视图。

### Task 5: 验证

**Files:**
- Build: `package.json`

- [x] 运行 `pnpm.cmd build` 验证 TypeScript 与 Vite 构建。
- [x] 运行 `git diff --check` 检查空白与补丁格式。
- [x] 汇总变更、说明未新增测试的原因：当前前端没有测试框架，且本次不引入依赖/根配置变更。

## Verification

- [x] `pnpm.cmd build` 通过：TypeScript 与 Vite 生产构建完成。
- [x] `git diff --check` 通过：未发现空白错误。
- [x] 最终命令因沙箱内 esbuild `spawn EPERM` 重跑到沙箱外后通过。
