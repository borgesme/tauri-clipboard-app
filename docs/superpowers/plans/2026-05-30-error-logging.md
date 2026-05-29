# 错误日志接入 tauri-plugin-log 与采集失败前端上报 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用 `tauri-plugin-log` 统一后端日志（写终端 + 文件），把 7 处 `eprintln!` 改为分级 `log` 宏，并把剪贴板采集失败以边沿触发方式上报前端展示。

**Architecture:** 后端注册 `tauri-plugin-log` 插件（Stdout + LogDir 两个目标）。非循环路径的失败直接换 `log::warn!`。剪贴板监听轮询抽出纯函数 `monitor_signal` 做「正常↔失败」边沿检测，仅在跳变时 `log` + emit 新事件 `clipboard:monitor-error`。前端复用现有 `listen` 事件机制订阅该事件，用既有 `errorMessage`/`message` 通道展示与清除。

**Tech Stack:** Rust + Tauri 2、`tauri-plugin-log` 2、`log` 0.4；React + TypeScript、Vitest（jsdom）。

> **执行约定**：本计划对应的 spec（`docs/superpowers/specs/2026-05-30-error-logging-design.md`）与本 plan 已在执行前合并为单个规划 commit；以下每个 Task 的代码改动各自独立提交。

---

## 文件结构

**后端（`src-tauri/src/`）：**
- `Cargo.toml` — 加 `tauri-plugin-log`、`log` 依赖
- `lib.rs` — 注册日志插件
- `desktop.rs` — 2 处 `eprintln!` → `log::warn!`
- `clipboard/commands.rs` — 2 处 `eprintln!` → `log::warn!`
- `clipboard/monitor.rs` — `emit_skip_event` 的 `eprintln!` → `warn!`；新增 `MonitorSignal`/`monitor_signal`；重写轮询循环（含原 :25 / :33）
- `clipboard/models.rs` — 新增 `ClipboardMonitorErrorEvent`
- `clipboard/mod.rs` — 挂 `#[cfg(test)] mod monitor_tests;`
- `clipboard/monitor_tests.rs` — 新增，覆盖 `monitor_signal`

**前端（`src/`）：**
- `types/clipboard.ts` — 新增 `ClipboardMonitorErrorEvent`
- `api/clipboard.ts` — 新增 `onClipboardMonitorError`
- `hooks/useClipboardEvents.ts` — 订阅 `monitor-error`，接 `errorMessage`/`message`
- `hooks/useClipboardWorkspace.test.tsx` — 升级 `listen` mock + 2 个事件用例

**文档：**
- `docs/2026-05-28-clipboard-toolbox-audit.md` — 标记 P1 #9 已修复

---

## Task 1: 接入 tauri-plugin-log 并注册插件

**Files:**
- Modify: `src-tauri/Cargo.toml:20-31`（`[dependencies]`）
- Modify: `src-tauri/src/lib.rs:24-27`（插件链）

- [ ] **Step 1: 添加依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 段，把 `tauri-plugin-dialog = "2"` 之后插入两行：

```toml
tauri-plugin-dialog = "2"
tauri-plugin-log = "2"
log = "0.4"
```

- [ ] **Step 2: 注册日志插件**

在 `src-tauri/src/lib.rs`，把现有的：

```rust
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
```

改为在其后追加日志插件（保持其它插件不变）：

```rust
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: None,
                    }),
                ])
                .build(),
        )
```

- [ ] **Step 3: 验证编译**

Run: `cd src-tauri; cargo check`
Expected: 编译通过，无新警告（下载/编译 `tauri-plugin-log`、`log` 两个新 crate）。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs
git commit -m "feat(logging): 接入 tauri-plugin-log，日志写终端与文件"
```

---

## Task 2: 非循环路径的 eprintln! 改用 log 宏

> 处理 5 处：`desktop.rs` 两处、`commands.rs` 两处、`monitor.rs` 的 `emit_skip_event`。`monitor.rs` 轮询循环里的 :25 / :33 留到 Task 3 随循环重写一并处理，避免重复改动。

**Files:**
- Modify: `src-tauri/src/desktop.rs:55,198`
- Modify: `src-tauri/src/clipboard/commands.rs:173,179`
- Modify: `src-tauri/src/clipboard/monitor.rs:58`

- [ ] **Step 1: desktop.rs 两处**

`src-tauri/src/desktop.rs:55`：

```rust
                if let Err(error) = close_window.hide() {
                    log::warn!("failed to hide window: {error}");
                }
```

`src-tauri/src/desktop.rs:198`：

```rust
    if let Err(error) = result {
        log::warn!("desktop action failed: {error}");
    }
```

- [ ] **Step 2: commands.rs 两处**

`src-tauri/src/clipboard/commands.rs:173`：

```rust
    if let Err(error) = app_handle.emit("clipboard:item-deleted", event) {
        log::warn!("failed to emit clipboard deleted event: {error}");
    }
```

`src-tauri/src/clipboard/commands.rs:179`：

```rust
    if let Err(error) = app_handle.emit("clipboard:monitor-status-changed", status) {
        log::warn!("failed to emit clipboard monitor status event: {error}");
    }
```

- [ ] **Step 3: monitor.rs 的 emit_skip_event**

`src-tauri/src/clipboard/monitor.rs:58`：

```rust
    if let Err(error) = app_handle.emit("clipboard:item-skipped", event) {
        log::warn!("failed to emit clipboard skipped event: {error}");
    }
```

- [ ] **Step 4: 验证编译**

Run: `cd src-tauri; cargo check`
Expected: 编译通过，无新警告。

- [ ] **Step 5: 确认这 5 处已无 eprintln!**

Run: `git grep -n "eprintln!" src-tauri/src/desktop.rs src-tauri/src/clipboard/commands.rs`
Expected: 无输出（这两个文件已无 `eprintln!`）。`monitor.rs` 仍剩 :25 / :33 两处，Task 3 处理。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/desktop.rs src-tauri/src/clipboard/commands.rs src-tauri/src/clipboard/monitor.rs
git commit -m "refactor(clipboard): eprintln! 失败日志改用 log 宏分级输出"
```

---

## Task 3: 采集失败边沿触发 + 上报 monitor-error 事件

**Files:**
- Modify: `src-tauri/src/clipboard/models.rs`（追加事件结构体）
- Create: `src-tauri/src/clipboard/monitor_tests.rs`
- Modify: `src-tauri/src/clipboard/mod.rs:17`（挂测试模块）
- Modify: `src-tauri/src/clipboard/monitor.rs`（新增纯函数 + 重写循环）

- [ ] **Step 1: 新增事件模型**

在 `src-tauri/src/clipboard/models.rs` 末尾追加：

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardMonitorErrorEvent {
    pub failing: bool,
    pub message: Option<String>,
}
```

- [ ] **Step 2: 写 monitor_signal 的失败测试**

创建 `src-tauri/src/clipboard/monitor_tests.rs`：

```rust
use super::monitor::{monitor_signal, MonitorSignal};

#[test]
fn signals_failing_on_healthy_to_error_edge() {
    assert_eq!(
        MonitorSignal::Failing("boom".to_string()),
        monitor_signal(true, Some("boom"))
    );
}

#[test]
fn signals_recovered_on_error_to_healthy_edge() {
    assert_eq!(MonitorSignal::Recovered, monitor_signal(false, None));
}

#[test]
fn stays_quiet_while_continuously_failing() {
    assert_eq!(MonitorSignal::Quiet, monitor_signal(false, Some("boom")));
}

#[test]
fn stays_quiet_while_continuously_healthy() {
    assert_eq!(MonitorSignal::Quiet, monitor_signal(true, None));
}
```

在 `src-tauri/src/clipboard/mod.rs:17`（`storage_path_tests` 行之后）追加：

```rust
#[cfg(test)]
mod monitor_tests;
```

- [ ] **Step 3: 运行测试，确认失败**

Run: `cd src-tauri; cargo test --lib clipboard::monitor_tests`
Expected: 编译失败 —— `monitor_signal` / `MonitorSignal` 未定义（`cannot find function/ type`）。

- [ ] **Step 4: 实现 monitor_signal 与重写循环**

把 `src-tauri/src/clipboard/monitor.rs` 顶部的 imports 与函数替换为以下完整内容（保留 `emit_skip_event` 不变，Task 2 已改其日志）：

```rust
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use super::models::{
    CaptureOutcome, ClipboardChangeEvent, ClipboardMonitorErrorEvent, ClipboardSkipReason,
    ClipboardSkippedEvent,
};
use super::service::ClipboardService;

const POLL_INTERVAL: Duration = Duration::from_millis(800);

#[derive(Debug, PartialEq)]
pub(crate) enum MonitorSignal {
    Quiet,
    Failing(String),
    Recovered,
}

pub(crate) fn monitor_signal(last_healthy: bool, capture_err: Option<&str>) -> MonitorSignal {
    match (last_healthy, capture_err) {
        (true, Some(msg)) => MonitorSignal::Failing(msg.to_string()),
        (false, None) => MonitorSignal::Recovered,
        _ => MonitorSignal::Quiet,
    }
}

pub fn start_clipboard_monitor(app_handle: AppHandle, service: Arc<ClipboardService>) {
    thread::spawn(move || {
        let mut last_poll_healthy = true;
        loop {
            let capture_err = match service.capture_current_clipboard() {
                Ok(CaptureOutcome::Item(item)) => {
                    let event_name = if item.copy_count > 1 {
                        "clipboard:item-updated"
                    } else {
                        "clipboard:item-created"
                    };
                    let event = ClipboardChangeEvent { item };
                    if let Err(error) = app_handle.emit(event_name, event) {
                        log::warn!("failed to emit clipboard event: {error}");
                    }
                    None
                }
                Ok(CaptureOutcome::Skipped {
                    reason,
                    content_length,
                    max_text_length,
                }) => {
                    emit_skip_event(&app_handle, reason, content_length, max_text_length);
                    None
                }
                Err(error) => Some(error.to_string()),
            };

            match monitor_signal(last_poll_healthy, capture_err.as_deref()) {
                MonitorSignal::Failing(message) => {
                    log::error!("clipboard monitor error: {message}");
                    emit_monitor_error(&app_handle, true, Some(message));
                }
                MonitorSignal::Recovered => {
                    log::info!("clipboard monitor recovered");
                    emit_monitor_error(&app_handle, false, None);
                }
                MonitorSignal::Quiet => {}
            }

            last_poll_healthy = capture_err.is_none();
            thread::sleep(POLL_INTERVAL);
        }
    });
}

fn emit_monitor_error(app_handle: &AppHandle, failing: bool, message: Option<String>) {
    let event = ClipboardMonitorErrorEvent { failing, message };
    if let Err(error) = app_handle.emit("clipboard:monitor-error", event) {
        log::warn!("failed to emit clipboard monitor error event: {error}");
    }
}
```

> `emit_skip_event` 函数体保持现状（位于本文件下方，Task 2 已把其内部 `eprintln!` 改为 `log::warn!`）。

- [ ] **Step 5: 运行测试，确认通过**

Run: `cd src-tauri; cargo test --lib clipboard`
Expected: 全部通过（含 4 个 `monitor_tests` 新用例与既有 `repository_tests`/`service_tests`/`storage_path_tests`）。

- [ ] **Step 6: 验证无新警告**

Run: `cd src-tauri; cargo check`
Expected: 通过，无 `dead_code` 等新警告（`monitor_signal` 已被循环调用）。`git grep -n "eprintln!" src-tauri/src` 应无输出。

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/clipboard/models.rs src-tauri/src/clipboard/monitor.rs src-tauri/src/clipboard/monitor_tests.rs src-tauri/src/clipboard/mod.rs
git commit -m "feat(clipboard): 采集失败边沿触发上报前端 monitor-error 事件"
```

---

## Task 4: 前端订阅 monitor-error 展示/清除监听错误

**Files:**
- Modify: `src/types/clipboard.ts`（追加接口）
- Modify: `src/api/clipboard.ts`（追加监听封装）
- Modify: `src/hooks/useClipboardEvents.ts`（订阅事件）
- Modify: `src/hooks/useClipboardWorkspace.test.tsx`（升级 mock + 2 用例）

- [ ] **Step 1: 升级前端测试的 listen mock 并写失败用例**

在 `src/hooks/useClipboardWorkspace.test.tsx` 顶部，把现有的：

```ts
vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));
```

替换为可捕获并触发事件回调的版本：

```ts
const { eventListeners } = vi.hoisted(() => ({
  eventListeners: new Map<string, (event: { payload: unknown }) => void>(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: (event: { payload: unknown }) => void) => {
    eventListeners.set(event, handler);
    return Promise.resolve(() => eventListeners.delete(event));
  },
}));

function emitEvent(event: string, payload: unknown) {
  eventListeners.get(event)?.({ payload });
}
```

在现有的 `beforeEach` 内补一行清理（与 `invoke.mockReset()` 并列）：

```ts
beforeEach(() => {
  invoke.mockReset();
  eventListeners.clear();
});
```

在文件末尾追加两个用例：

```ts
describe("useClipboardWorkspace monitor errors", () => {
  it("surfaces the monitor failure message as an error", async () => {
    setupInvoke();
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));
    await waitFor(() =>
      expect(eventListeners.has("clipboard:monitor-error")).toBe(true),
    );

    act(() => emitEvent("clipboard:monitor-error", { failing: true, message: "boom" }));

    expect(result.current.errorMessage).toBe("boom");
  });

  it("clears the error and reports recovery when monitoring resumes", async () => {
    setupInvoke();
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));
    await waitFor(() =>
      expect(eventListeners.has("clipboard:monitor-error")).toBe(true),
    );

    act(() => emitEvent("clipboard:monitor-error", { failing: true, message: null }));
    expect(result.current.errorMessage).toBe("剪贴板监听出现错误。");

    act(() => emitEvent("clipboard:monitor-error", { failing: false, message: null }));
    expect(result.current.errorMessage).toBe("");
    expect(result.current.message).toBe("剪贴板监听已恢复。");
  });
});
```

- [ ] **Step 2: 运行前端测试，确认失败**

Run: `pnpm.cmd test -- useClipboardWorkspace`
Expected: 新增两个用例失败 —— `onClipboardMonitorError` 尚未实现，`clipboard:monitor-error` 监听从未注册，`eventListeners.has(...)` 永远为 false（`waitFor` 超时）。

- [ ] **Step 3: 新增前端类型**

在 `src/types/clipboard.ts` 的 `ClipboardSkippedEvent` 之后追加：

```ts
export interface ClipboardMonitorErrorEvent {
  failing: boolean;
  message: string | null;
}
```

- [ ] **Step 4: 新增监听封装**

在 `src/api/clipboard.ts` 的类型 import 中加入 `ClipboardMonitorErrorEvent`：

```ts
import type {
  ClipboardChangeEvent,
  ClipboardDateGroup,
  ClipboardDeletedEvent,
  ClipboardItem,
  ClipboardMonitorErrorEvent,
  ClipboardMonitorStatus,
  ClipboardSkippedEvent,
  DesktopSettings,
  DesktopSettingsUpdate,
} from "@/types/clipboard";
```

在文件末尾（`onClipboardMonitorStatusChanged` 之后）追加：

```ts
export function onClipboardMonitorError(
  handler: (event: ClipboardMonitorErrorEvent) => void,
): Promise<UnlistenFn> {
  return listen<ClipboardMonitorErrorEvent>(
    "clipboard:monitor-error",
    (event) => handler(event.payload),
  );
}
```

- [ ] **Step 5: 在 useClipboardEvents 订阅事件**

修改 `src/hooks/useClipboardEvents.ts`。

更新 import（加入 `onClipboardMonitorError` 与类型 `ClipboardMonitorErrorEvent`）：

```ts
import {
  onClipboardItemCreated,
  onClipboardItemDeleted,
  onClipboardItemUpdated,
  onClipboardItemSkipped,
  onClipboardMonitorError,
  onClipboardMonitorStatusChanged,
} from "@/api/clipboard";
import type {
  ClipboardMonitorErrorEvent,
  ClipboardSkippedEvent,
  DesktopSettings,
} from "@/types/clipboard";
```

在 `useEffect` 内、`remoteChange` 定义之后，新增 `monitorError` 回调（复用 `disposed` 闭包，与 `remoteChange` 同一模式），并把它传入 `registerEvents`：

```ts
    const remoteChange = (message: string) => {
      if (disposed) {
        return;
      }
      setMessage(message);
      void refreshView();
    };
    const monitorError = (event: ClipboardMonitorErrorEvent) => {
      if (disposed) {
        return;
      }
      if (event.failing) {
        setErrorMessage(event.message ?? "剪贴板监听出现错误。");
      } else {
        setErrorMessage("");
        setMessage("剪贴板监听已恢复。");
      }
    };
    void registerEvents(disposers, remoteChange, monitorError, setMonitorEnabled, setDesktopSettings)
      .catch((error: unknown) => setErrorMessage(String(error)));
```

更新 `registerEvents` 签名与函数体（新增 `monitorError` 形参并注册监听）：

```ts
async function registerEvents(
  disposers: Array<() => void>,
  remoteChange: (message: string) => void,
  monitorError: (event: ClipboardMonitorErrorEvent) => void,
  setMonitorEnabled: (value: boolean) => void,
  setDesktopSettings: React.Dispatch<React.SetStateAction<DesktopSettings | null>>,
) {
  disposers.push(await onClipboardItemCreated(() => remoteChange("已捕获新的剪贴板文本。")));
  disposers.push(await onClipboardItemUpdated(() => remoteChange("重复内容已更新计数。")));
  disposers.push(await onClipboardItemDeleted(() => remoteChange("记录已删除。")));
  disposers.push(await onClipboardItemSkipped((event) => remoteChange(skipMessage(event))));
  disposers.push(await onClipboardMonitorError(monitorError));
  disposers.push(await onClipboardMonitorStatusChanged((status) => {
    setMonitorEnabled(status.enabled);
    setDesktopSettings((settings) => settings ? { ...settings, monitorEnabled: status.enabled } : settings);
  }));
}
```

- [ ] **Step 6: 运行前端测试，确认通过**

Run: `pnpm.cmd test -- useClipboardWorkspace`
Expected: 全部通过（含新增两个 monitor-error 用例）。

- [ ] **Step 7: 类型检查与全量前端测试**

Run: `pnpm.cmd test`
Expected: 全部用例通过，无 TypeScript 报错（`tsc` 在 `build` 脚本里，此处 vitest 用 esbuild，类型问题在 Task 5 的 build 中兜底）。

- [ ] **Step 8: 提交**

```bash
git add src/types/clipboard.ts src/api/clipboard.ts src/hooks/useClipboardEvents.ts src/hooks/useClipboardWorkspace.test.tsx
git commit -m "feat(clipboard): 前端订阅 monitor-error 展示/清除监听错误"
```

---

## Task 5: 标记审计项并全量验证

**Files:**
- Modify: `docs/2026-05-28-clipboard-toolbox-audit.md:84-88,186`

- [ ] **Step 1: 标记 P1 #9 已修复**

在 `docs/2026-05-28-clipboard-toolbox-audit.md`，把 §9 标题行：

```markdown
### 9. 错误统一吞进 `eprintln!`
```

改为：

```markdown
### 9. 错误统一吞进 `eprintln!` ✅ 2026-05-30 已修复
```

并把审查清单表格中 P1 #9 行：

```markdown
| P1 | 9 | 错误吞进 `eprintln!` | 可观测性 |
```

改为：

```markdown
| P1 | 9 | 错误吞进 `eprintln!` ✅ | 可观测性 |
```

- [ ] **Step 2: 后端全量测试**

Run: `cd src-tauri; cargo test`
Expected: 全部通过。

- [ ] **Step 3: 前端全量构建（含 tsc 类型检查）**

Run: `pnpm.cmd build`
Expected: `tsc` 无类型错误、`vitest run` 全通过、`vite build` 成功产出。

- [ ] **Step 4: 手动冒烟（日志落盘）**

Run: `pnpm.cmd tauri dev`
确认：
- 终端能看到 `tauri-plugin-log` 输出的启动日志。
- 应用日志目录（Windows：`%LOCALAPPDATA%\com.<...>\logs` 或 `%LOCALAPPDATA%\<productName>\logs`）下生成日志文件。
- 应用窗口正常打开、剪贴板采集与历史展示如常（无回归）。

> 若需快速验证采集失败上报，可临时不验证（属正向路径难以人为触发），以单测覆盖的边沿逻辑为准。

- [ ] **Step 5: 提交**

```bash
git add docs/2026-05-28-clipboard-toolbox-audit.md
git commit -m "docs(clipboard): 标记审计 P1 #9 错误日志已修复"
```

---

## 验证清单（完成后逐项确认）

- [ ] `git grep -n "eprintln!" src-tauri/src` 无输出（7 处全部替换）
- [ ] `cd src-tauri; cargo test` 全通过（含 4 个 `monitor_signal` 用例）
- [ ] `cd src-tauri; cargo check` 无新警告
- [ ] `pnpm.cmd build` 通过（tsc + vitest + vite build）
- [ ] 前端新增 2 个 monitor-error 用例通过
- [ ] dev 下日志进终端且日志文件落盘
- [ ] 审计文档 P1 #9 已标记
