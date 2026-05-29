# 前端测试基础设施设计

> 设计日期：2026-05-29
> 审查项：`docs/2026-05-28-clipboard-toolbox-audit.md` P1 #8
> 范围：`src/` 前端；新增 Vitest 测试基础设施 + 关键逻辑/hook 测试；后端零改动

## 1. 背景与问题

`src/` 下没有任何 `*.test.ts(x)`，未安装 Vitest。核心逻辑全无自动化覆盖：

- `clipStudioHelpers.ts`：内容分类（`getClipKind`）、过滤（`filterClipboardItems`）、工具箱转换（`createToolboxResult`）、计数（`countRecords` / `countTodayRecords`）等纯函数。
- `lib/date.ts`：`todayKey()` 本地日期键（与后端 `local_date` 分组比较）。
- `useClipboardEvents.ts` 的 `skipMessage`：跳过原因 → 用户文案映射。
- `useClipboardWorkspace.ts`：工作区状态机（日期选择、监听开关、清空、删除、设置保存的状态流转与 message/error 反馈）。

这些逻辑回归风险靠手动验证，缺乏护栏。本设计搭建 Vitest 基础设施并覆盖上述逻辑与 hook 行为。

## 2. 目标与非目标

### 目标

- 引入 Vitest + jsdom + @testing-library/react 测试基础设施，与现有 Vite 7 / React 19 / `@vitejs/plugin-react` 集成。
- 覆盖纯逻辑（node 环境）与 `useClipboardWorkspace` 的对外行为（jsdom + renderHook，mock Tauri `invoke`/`listen`）。
- `pnpm test` 一键运行；`build` 脚本纳入测试门禁（`tsc && vitest run && vite build`）。

### 非目标

- 不做组件渲染测试（Radix/lucide 渲染测试维护成本高、收益递减）。
- 不引入 CI 配置（本项目无 `.github/workflows`，内部项目，后续可接入）。
- 不加 git hook。
- 不重构生产代码结构。唯一生产代码改动：`skipMessage` 加 `export`（见 §5）。

## 3. 测试环境与工具

- **Runner**：Vitest，复用 `vite.config.ts`（已含 `@vitejs/plugin-react` 与 `@/` → `./src` 的 `resolve.alias`，Vitest 自动继承，无需重复配置 alias）。
- **环境分层**：Vitest 默认 `environment: "node"`（在 `vite.config.ts` 的 `test` 字段设定）。需要 DOM 的 hook 测试在文件顶部用 per-file 注释 `// @vitest-environment jsdom` 覆盖。
- **类型**：`vite.config.ts` 顶部加 `/// <reference types="vitest/config" />`，使 `defineConfig` 接受 `test` 字段。
- **Tauri mock**：hook 测试中 `vi.mock("@tauri-apps/api/core", ...)` 提供受控 `invoke`，`vi.mock("@tauri-apps/api/event", ...)` 提供受控 `listen`（返回一个 no-op unlisten）。

### 3.1 新增 devDependencies

- `vitest`
- `jsdom`
- `@testing-library/react`（React 19 需 ≥16）
- `@testing-library/dom`（RTL 的 peer dependency）

> 实现时以 pnpm 解析到的兼容版本为准；React 19 必须配 RTL 16+。

### 3.2 vite.config.ts 的 test 字段

在 `defineConfig(async () => ({ ... }))` 返回对象中加入：

```ts
test: {
  environment: "node",
  globals: true,
  include: ["src/**/*.test.{ts,tsx}"],
},
```

`globals: true` 使 `describe`/`it`/`expect`/`vi` 无需逐文件 import；配合 `vitest/globals` 类型（在 tsconfig 的 `types` 或通过 `/// <reference types="vitest/globals" />` 提供，实现时确认 `tsconfig` 是否需补 `vitest/globals`，否则 `tsc` 会报未定义全局）。

## 4. 测试覆盖清单

### 4.1 `src/lib/date.test.ts`（node）

- `todayKey()` 返回 `YYYY-MM-DD` 格式，月/日补零。用 `vi.useFakeTimers()` + `vi.setSystemTime(new Date(2026, 0, 5))` 断言 `"2026-01-05"`；`afterEach` 还原 `vi.useRealTimers()`。
- 双位月份/日不补零场景：`setSystemTime(new Date(2026, 10, 23))` → `"2026-11-23"`。

### 4.2 `src/components/clipboard/clipStudioHelpers.test.ts`（node）

- `getClipKind`：
  - secret：含 `eyJ...JWT` 三段、`api_key` / `api-key`、`token`、`secret` 关键字 → `"secret"`。
  - link：`https://...` / `http://...` → `"link"`。
  - code：含换行后 `const`/`let`/`fn`/`function`/`class`/`import`/`export`、或行尾 `{` `}` `;` → `"code"`。
  - 其余 → `"text"`。
  - 优先级：secret 先于 link 先于 code（构造一条既像 link 又含 token 的内容，断言归 secret）。
- `filterClipboardItems`：`"all"` 原样返回；`"frequent"` 仅保留 `copyCount > 1`；按 kind（`"link"` 等）过滤只留该类。
- `createToolboxResult`：
  - `"trim"`：多空格折叠为单空格、3+ 连续换行折叠为 2、首尾 trim。
  - `"upper"` / `"lower"`：大小写转换。
  - `"markdown"`：http 链接 → `[链接标题](url)`；普通文本 → `[文本](https://example.com)`；空文本 → `[链接标题](https://example.com)`。
- `countRecords`：多组 count 求和。
- `countTodayRecords`：命中 today 返回其 count；未命中返回 0。
- `getClipKindLabel` / `getClipIcon`：四种 kind 的映射值。

### 4.3 `src/hooks/useClipboardEvents.test.ts`（node）

测试 `skipMessage`（§5 将其 export）：
- `reason: "tooLong"`，`maxTextLength: 5000` → 文案含 `5000`。
- `reason: "secretLike"` → 敏感跳过文案。
- 未知 reason → 默认跳过文案。

### 4.4 `src/hooks/useClipboardWorkspace.test.tsx`（jsdom + renderHook）

文件顶部 `// @vitest-environment jsdom`。

mock 设置：
- `vi.mock("@tauri-apps/api/core")`：`invoke` 为 `vi.fn`，按命令名返回受控数据（`list_clipboard_dates` → 固定 groups；`list_clipboard_items` → 固定 items；`get_clipboard_monitor_status`/`get_desktop_settings` → 固定状态；`set_clipboard_monitor_enabled`/`update_desktop_settings`/`clear_clipboard_items_by_date`/`delete_clipboard_item`/`copy_clipboard_item`/`purge_deleted_clipboard_items` → 受控返回）。
- `vi.mock("@tauri-apps/api/event")`：`listen` 返回 `Promise.resolve(() => {})`。
- 每个用例 `beforeEach` 重置 mock。

用例（均用 `renderHook` + `waitFor`/`act`）：
1. **初始加载**：等待后 `dates`/`items` 为 mock 数据，`selectedItem` 为首项（或选中项）。
2. **selectDate**：`act(() => result.current.selectDate("2026-05-20"))` → `selectedDate === "2026-05-20"`、`searchTerm === ""`、`message` 含该日期。
3. **toggleMonitor**：mock `set_clipboard_monitor_enabled` 返回 `{ enabled: false }` → `await act` 后 `monitorEnabled === false`、message 为暂停文案。
4. **clearDate**：`await act(() => result.current.clearDate())` → `invoke` 以 `clear_clipboard_items_by_date` 被调用、message 含已清空、刷新后再次拉取（`list_clipboard_dates` 调用次数增加）。
5. **deleteItem**：`await act(() => result.current.deleteItem(item))` → `delete_clipboard_item` 被调用、message 为删除文案。
6. **updateSettings 成功**：mock `update_desktop_settings` 返回保存后的 settings → `desktopSettings` 更新、message 为保存文案；`isBusy` 最终回 false。
7. **updateSettings 失败**：mock `update_desktop_settings` reject → `errorMessage` 被设置为错误串、`isBusy` 回 false。

> `selectNextItemId`（保留当前选中 / 列表变更回落首项）与 `useSelectedItem` 通过初始加载与刷新用例间接覆盖，不单独抽出。

## 5. 生产代码改动（最小）

`src/hooks/useClipboardEvents.ts`：`function skipMessage(...)` 改为 `export function skipMessage(...)`。仅加 `export` 关键字，不改逻辑、不移动文件。`useClipboardEvents` 内部调用不变。

## 6. 构建集成

`package.json` scripts：

```json
"test": "vitest run",
"test:watch": "vitest",
"build": "tsc && vitest run && vite build"
```

`build` 链路加入 `vitest run`：测试失败则生产构建失败。

## 7. 改动文件清单

- `package.json`：加 `test` / `test:watch` 脚本、改 `build`、加 4 个 devDependencies
- `vite.config.ts`：加 `/// <reference types="vitest/config" />` 与 `test` 字段
- `tsconfig.json`（按需）：`types` 补 `vitest/globals`，使 `tsc` 认全局 `describe`/`it`/`expect`/`vi`
- `src/hooks/useClipboardEvents.ts`：`skipMessage` 加 `export`
- 新增：`src/lib/date.test.ts`
- 新增：`src/components/clipboard/clipStudioHelpers.test.ts`
- 新增：`src/hooks/useClipboardEvents.test.ts`
- 新增：`src/hooks/useClipboardWorkspace.test.tsx`
- `docs/2026-05-28-clipboard-toolbox-audit.md`：标记 P1 #8 已修复

## 8. 验证

- `pnpm.cmd test`：全部用例通过（预计 30+）。
- `pnpm.cmd build`（`tsc && vitest run && vite build`）：类型检查 + 测试 + 生产构建整链通过。
- 不引 CI、不加 git hook。
