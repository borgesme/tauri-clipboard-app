# 错误日志接入 tauri-plugin-log 与采集失败前端上报设计

> 设计日期：2026-05-30
> 审查项：`docs/2026-05-28-clipboard-toolbox-audit.md` P1 #9
> 范围：`src-tauri/src/`（日志后端 + 采集失败 emit）、`src/`（前端事件上报）

## 1. 背景与问题

后端共 7 处用 `eprintln!` 吞错误：

- `desktop.rs:55`（隐藏窗口失败）、`desktop.rs:198`（桌面动作失败）
- `commands.rs:173`（emit 删除事件失败）、`commands.rs:179`（emit 监听状态事件失败）
- `monitor.rs:25`（emit 剪贴板事件失败）、`monitor.rs:58`（emit 跳过事件失败）、`monitor.rs:33`（采集循环出错）

两个真实缺陷：

1. **打包后 stderr 不可见** —— `eprintln!` 只进终端，打包应用没有终端，用户机器上的失败无任何留痕，排查全凭复现。
2. **采集失败静默** —— `monitor.rs:33` 的失败发生在 800ms 轮询里且完全静默。若持续失败，剪贴板历史静默停更，用户无感知。

## 2. 设计目标与非目标

### 目标

- 引入统一日志后端 `tauri-plugin-log`，日志同时写终端（dev）与文件（打包可查）。
- 7 处 `eprintln!` 全部改为分级 `log` 宏。
- 采集失败（`monitor.rs:33`）以**边沿触发**方式 emit 到前端，用户可见提示；恢复时清除。

### 非目标

- **不接 Webview/devtools 目标**：该目标需前端 `@tauri-apps/plugin-log` + capability 权限，本次不做，留作后续。
- 不改命令层签名；不动其余 6 处的前端可见性（它们仅记日志）。
- 不引入 `tracing` 的 span/结构化字段；`tauri-plugin-log` 基于 `log` 门面即可满足。

## 3. 依赖与日志初始化

`src-tauri/Cargo.toml` 新增：

```toml
tauri-plugin-log = "2"
log = "0.4"
```

`src-tauri/src/lib.rs` 在 `Builder` 链注册（setup 内的日志即可用）：

```rust
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

- **Stdout**：dev 终端实时可见。
- **LogDir**：写入系统应用日志目录（Windows 为 `%LOCALAPPDATA%\<bundleId>\logs`），打包后可查。

## 4. 七处 `eprintln!` → `log` 宏

| 位置 | 现文案 | 新级别 | 说明 |
|----|----|----|----|
| desktop.rs:55 | failed to hide window | `warn!` | UI 操作失败，非致命 |
| desktop.rs:198 | desktop action failed | `warn!` | 菜单动作失败，非致命 |
| commands.rs:173 | failed to emit clipboard deleted event | `warn!` | 事件投递失败 |
| commands.rs:179 | failed to emit clipboard monitor status event | `warn!` | 事件投递失败 |
| monitor.rs:25 | failed to emit clipboard event | `warn!` | 事件投递失败 |
| monitor.rs:58 | failed to emit clipboard skipped event | `warn!` | 事件投递失败 |
| monitor.rs:33 | clipboard monitor error | `error!` | 核心采集降级，另走 §5 上报 |

文案保留英文（与现状一致，便于 grep）。其余 4 处 emit 失败只能记日志：emit 通道本身已坏，再 emit 错误事件也发不出去。

## 5. 采集失败边沿上报（monitor.rs）

### 5.1 纯函数（可测）

```rust
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
```

### 5.2 poll loop 接线

循环外 `let mut last_poll_healthy = true;`。每轮：

1. 调 `capture_current_clipboard()`，成功分支照旧 emit（`item-created`/`item-updated`/`item-skipped`）。
2. 计算 `capture_err: Option<String>`（仅 `Err` 分支为 `Some(error.to_string())`）。
3. `match monitor_signal(last_poll_healthy, capture_err.as_deref())`：
   - `Failing(msg)` → `log::error!("clipboard monitor error: {msg}")` + emit `clipboard:monitor-error { failing: true, message: Some(msg) }`
   - `Recovered` → `log::info!("clipboard monitor recovered")` + emit `clipboard:monitor-error { failing: false, message: None }`
   - `Quiet` → 无操作
4. `last_poll_healthy = capture_err.is_none();`

持续失败、持续正常都落 `Quiet`，不刷屏。

### 5.3 事件载荷（models.rs）

```rust
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardMonitorErrorEvent {
    pub failing: bool,
    pub message: Option<String>,
}
```

## 6. 前端（复用现有事件机制）

`src/types/clipboard.ts`：

```ts
export interface ClipboardMonitorErrorEvent {
  failing: boolean;
  message: string | null;
}
```

`src/api/clipboard.ts`：

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

`src/hooks/useClipboardEvents.ts`：`registerEvents` 新增

```ts
disposers.push(await onClipboardMonitorError((event) => {
  if (disposed) return;
  if (event.failing) {
    setErrorMessage(event.message ?? "剪贴板监听出现错误。");
  } else {
    setErrorMessage("");
    setMessage("剪贴板监听已恢复。");
  }
}));
```

> `failing=true` 用 `setErrorMessage` 展示（与现有错误条同一通道）；`failing=false` 清除错误并给恢复提示。`registerEvents` 已持有 `disposed` 闭包变量与 `setMessage`/`setErrorMessage`，签名无需扩展。

## 7. 测试计划（TDD，先红后绿）

后端 `monitor_tests.rs`（新增，`mod.rs` 挂 `#[cfg(test)] mod monitor_tests;`）：

1. 正常→失败：`monitor_signal(true, Some("x"))` == `Failing("x")`
2. 失败→正常：`monitor_signal(false, None)` == `Recovered`
3. 持续失败：`monitor_signal(false, Some("x"))` == `Quiet`
4. 持续正常：`monitor_signal(true, None)` == `Quiet`

前端 `useClipboardWorkspace.test.tsx`（扩展现有 renderHook 用例）：

5. 收到 `failing:true` 事件 → `errorMessage` == message
6. 收到 `failing:false` 事件 → `errorMessage` 清空、`message` == 恢复文案

> 前端用例沿用现有 mock：测试已对 `@tauri-apps/api/event` 的 `listen` 打桩并手动触发事件回调。

## 8. 改动文件清单

- `src-tauri/Cargo.toml`：加 `tauri-plugin-log`、`log` 依赖
- `src-tauri/src/lib.rs`：注册日志插件
- `src-tauri/src/desktop.rs`：2 处 `eprintln!` → `warn!`
- `src-tauri/src/clipboard/commands.rs`：2 处 `eprintln!` → `warn!`
- `src-tauri/src/clipboard/monitor.rs`：2 处 emit 失败 → `warn!`、采集失败改边沿上报、新增 `monitor_signal`/`MonitorSignal`
- `src-tauri/src/clipboard/models.rs`：新增 `ClipboardMonitorErrorEvent`
- `src-tauri/src/clipboard/mod.rs`：挂 `monitor_tests`
- `src-tauri/src/clipboard/monitor_tests.rs`：新增
- `src/types/clipboard.ts`：新增 `ClipboardMonitorErrorEvent`
- `src/api/clipboard.ts`：新增 `onClipboardMonitorError`
- `src/hooks/useClipboardEvents.ts`：注册 `monitor-error` 处理
- `src/hooks/useClipboardWorkspace.test.tsx`：新增 2 用例
- `docs/2026-05-28-clipboard-toolbox-audit.md`：标记 P1 #9 已修复

## 9. 验证

- `cd src-tauri; cargo test clipboard` 全通过（含 `monitor_signal` 新用例）
- `cd src-tauri; cargo check` 无新警告
- `pnpm.cmd test` 前端用例通过
- `pnpm.cmd build` 构建通过
- 手动：dev 下确认日志进终端；查看应用 `logs` 目录有日志文件落盘
