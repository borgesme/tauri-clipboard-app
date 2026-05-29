# 前端测试基础设施 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 引入 Vitest + jsdom + @testing-library/react 测试基础设施，覆盖前端纯逻辑（clipStudioHelpers / date / skipMessage）与 `useClipboardWorkspace` 的对外行为，并把测试纳入 build 门禁。

**Architecture:** 复用 `vite.config.ts`（已配 `@/` alias 与 React 插件）加 `test` 字段；纯逻辑默认 node 环境，hook 测试用 per-file `// @vitest-environment jsdom` 注释 + mock `@tauri-apps/api`；唯一生产代码改动是给 `skipMessage` 加 `export`。

**Tech Stack:** Vitest, jsdom, @testing-library/react (≥16, React 19), TypeScript 5.8, Vite 7。

参考 spec：`docs/superpowers/specs/2026-05-29-frontend-test-infrastructure-design.md`

---

## File Structure

- `package.json` — 加 devDependencies（vitest / jsdom / @testing-library/react / @testing-library/dom）、加 `test` / `test:watch` 脚本、改 `build`
- `vite.config.ts` — 加 `/// <reference types="vitest/config" />` 与 `test` 字段
- `tsconfig.json` — `compilerOptions.types` 补 `["vitest/globals"]`，使 `tsc` 认全局 API
- `src/hooks/useClipboardEvents.ts` — `skipMessage` 加 `export`（唯一生产改动）
- 新增 `src/lib/date.test.ts` — `todayKey` 纯逻辑（node）
- 新增 `src/components/clipboard/clipStudioHelpers.test.ts` — 分类/过滤/工具箱/计数纯逻辑（node）
- 新增 `src/hooks/useClipboardEvents.test.ts` — `skipMessage` 纯逻辑（node）
- 新增 `src/hooks/useClipboardWorkspace.test.tsx` — hook 行为（jsdom + renderHook + Tauri mock）
- `docs/2026-05-28-clipboard-toolbox-audit.md` — 标记 P1 #8 已修复

执行顺序：先装环境并让一个最小测试跑通（Task 1），再逐文件加测试（Task 2-5），最后纳入 build 门禁 + 文档（Task 6）。

### 关键类型（来自 `src/types/clipboard.ts`，测试 fixture 必须匹配）

```ts
interface ClipboardItem {
  id: number; contentType: "text"; content: string; preview: string;
  contentHash: string; createdAt: string; lastCopiedAt: string; copyCount: number;
}
interface ClipboardDateGroup { date: string; count: number; }
type ClipboardSkipReason = "empty" | "monitorDisabled" | "tooLong" | "secretLike" | "duplicate" | "appWriteBack";
interface ClipboardSkippedEvent { reason: ClipboardSkipReason; contentLength: number; maxTextLength: number; }
interface ClipboardMonitorStatus { enabled: boolean; }
interface DesktopSettings {
  autostartEnabled: boolean; monitorEnabled: boolean; retentionDays: number;
  maxRecordCount: number; maxTextLength: number; ignorePasswordLikeText: boolean;
  customSecretPatterns: string; storageDir: string;
}
```

> 注意 `tsconfig.json` 开了 `noUnusedLocals` / `noUnusedParameters`。测试文件不可留未用变量/参数，否则 `tsc`（在 build 链里）会报错。

---

### Task 1: 安装 Vitest 环境并跑通最小测试

**Files:**
- Modify: `package.json`（devDependencies + 脚本）
- Modify: `vite.config.ts`（test 字段）
- Modify: `tsconfig.json`（types）
- Create: `src/lib/date.test.ts`（作为最小冒烟测试）

- [ ] **Step 1: 安装依赖**

在仓库根目录运行：
```bash
pnpm.cmd add -D vitest jsdom @testing-library/react @testing-library/dom
```
Expected: 四个包写入 `package.json` 的 devDependencies，pnpm 解析出与 React 19 兼容的版本（@testing-library/react 应为 16.x）。

- [ ] **Step 2: 在 vite.config.ts 加 test 字段**

文件 `vite.config.ts` 当前首行是 `import { fileURLToPath, URL } from "node:url";`。在文件最顶部（第 1 行之前）插入：
```ts
/// <reference types="vitest/config" />
```
然后在 `defineConfig(async () => ({ ... }))` 返回的对象里，于 `plugins: [react(), tailwindcss()],` 之后加入 `test` 字段：
```ts
  test: {
    environment: "node",
    globals: true,
    include: ["src/**/*.test.{ts,tsx}"],
  },
```
（`resolve.alias` 已存在，Vitest 自动继承，勿重复添加。）

- [ ] **Step 3: tsconfig 补 vitest/globals 类型**

文件 `tsconfig.json` 的 `compilerOptions` 当前没有 `types` 键。在 `compilerOptions` 内（如 `"skipLibCheck": true,` 之后）加：
```json
    "types": ["vitest/globals"],
```
这样 `tsc` 能识别全局 `describe` / `it` / `expect` / `vi`。

- [ ] **Step 4: 加 package.json 脚本（先不改 build）**

把 `scripts` 改为（暂不动 `build`，Task 6 再加门禁，避免中途半成品测试拖垮 build）：
```json
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "test": "vitest run",
    "test:watch": "vitest",
    "preview": "vite preview",
    "tauri": "tauri",
    "tauri:dev": "tauri dev"
  },
```

- [ ] **Step 5: 写最小冒烟测试**

创建 `src/lib/date.test.ts`：
```ts
import { todayKey } from "@/lib/date";

describe("todayKey", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("formats the current local date as YYYY-MM-DD with zero padding", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 0, 5, 9, 30, 0));
    expect(todayKey()).toBe("2026-01-05");
  });

  it("keeps two-digit month and day without extra padding", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 10, 23, 18, 0, 0));
    expect(todayKey()).toBe("2026-11-23");
  });
});
```

- [ ] **Step 6: 运行测试，确认通过**

Run: `pnpm.cmd test`
Expected: 1 个测试文件、2 个用例 PASS。若 `@/` 无法解析，确认 Step 2 的 `/// <reference>` 与现有 alias；若全局 `describe` 报类型错，确认 Step 3。

- [ ] **Step 7: 确认 tsc 不报错**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 无错误（验证 `vitest/globals` 类型与测试文件类型均通过）。

- [ ] **Step 8: Commit**

```bash
git add package.json pnpm-lock.yaml vite.config.ts tsconfig.json src/lib/date.test.ts
git commit -m "test(frontend): 引入 Vitest 环境并覆盖 todayKey"
```

---

### Task 2: clipStudioHelpers 纯逻辑测试

**Files:**
- Create: `src/components/clipboard/clipStudioHelpers.test.ts`

被测模块导出：`filterClipboardItems`、`getClipKind`、`getClipKindLabel`、`getClipIcon`、`createToolboxResult`、`countRecords`、`countTodayRecords`，类型 `ClipFilter` / `ClipKind` / `ToolboxAction`。

- [ ] **Step 1: 写测试文件**

创建 `src/components/clipboard/clipStudioHelpers.test.ts`：
```ts
import {
  countRecords,
  countTodayRecords,
  createToolboxResult,
  filterClipboardItems,
  getClipIcon,
  getClipKind,
  getClipKindLabel,
} from "@/components/clipboard/clipStudioHelpers";
import type { ClipboardDateGroup, ClipboardItem } from "@/types/clipboard";

function makeItem(overrides: Partial<ClipboardItem> = {}): ClipboardItem {
  return {
    id: 1,
    contentType: "text",
    content: "hello",
    preview: "hello",
    contentHash: "h",
    createdAt: "2026-05-29T00:00:00Z",
    lastCopiedAt: "2026-05-29T00:00:00Z",
    copyCount: 1,
    ...overrides,
  };
}

describe("getClipKind", () => {
  it("classifies JWT-like content as secret", () => {
    expect(getClipKind(makeItem({ content: "eyJhbGc.eyJzdWI.sIgnAture" }))).toBe("secret");
  });
  it("classifies api_key / token / secret keywords as secret", () => {
    expect(getClipKind(makeItem({ content: "my api_key here" }))).toBe("secret");
    expect(getClipKind(makeItem({ content: "auth token value" }))).toBe("secret");
    expect(getClipKind(makeItem({ content: "the secret sauce" }))).toBe("secret");
  });
  it("classifies http(s) URLs as link", () => {
    expect(getClipKind(makeItem({ content: "https://example.com" }))).toBe("link");
    expect(getClipKind(makeItem({ content: "http://foo.bar/baz" }))).toBe("link");
  });
  it("classifies code-like content as code", () => {
    expect(getClipKind(makeItem({ content: "x\n  const a = 1" }))).toBe("code");
    expect(getClipKind(makeItem({ content: "let y = 2;" }))).toBe("code");
  });
  it("falls back to text", () => {
    expect(getClipKind(makeItem({ content: "just a sentence" }))).toBe("text");
  });
  it("prioritizes secret over link when content matches both", () => {
    expect(getClipKind(makeItem({ content: "https://x.com/?token=abc" }))).toBe("secret");
  });
});

describe("filterClipboardItems", () => {
  const items = [
    makeItem({ id: 1, content: "https://example.com", copyCount: 1 }),
    makeItem({ id: 2, content: "plain text", copyCount: 3 }),
    makeItem({ id: 3, content: "let z = 1;", copyCount: 1 }),
  ];
  it("returns all items unchanged for 'all'", () => {
    expect(filterClipboardItems(items, "all")).toHaveLength(3);
  });
  it("returns only items with copyCount > 1 for 'frequent'", () => {
    const result = filterClipboardItems(items, "frequent");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe(2);
  });
  it("filters by kind for 'link'", () => {
    const result = filterClipboardItems(items, "link");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe(1);
  });
});

describe("createToolboxResult", () => {
  it("trims and collapses whitespace for 'trim'", () => {
    expect(createToolboxResult("trim", "  a   b\n\n\n\nc  ")).toBe("a b\n\nc");
  });
  it("uppercases for 'upper'", () => {
    expect(createToolboxResult("upper", "abc")).toBe("ABC");
  });
  it("lowercases for 'lower'", () => {
    expect(createToolboxResult("lower", "ABC")).toBe("abc");
  });
  it("wraps a URL as a markdown link", () => {
    expect(createToolboxResult("markdown", "https://example.com")).toBe("[链接标题](https://example.com)");
  });
  it("wraps plain text as a markdown link with placeholder url", () => {
    expect(createToolboxResult("markdown", "hello")).toBe("[hello](https://example.com)");
  });
  it("uses placeholder title for empty text", () => {
    expect(createToolboxResult("markdown", "")).toBe("[链接标题](https://example.com)");
  });
});

describe("count helpers", () => {
  const groups: ClipboardDateGroup[] = [
    { date: "2026-05-29", count: 3 },
    { date: "2026-05-28", count: 5 },
  ];
  it("sums all counts", () => {
    expect(countRecords(groups)).toBe(8);
  });
  it("returns today's count when present", () => {
    expect(countTodayRecords(groups, "2026-05-29")).toBe(3);
  });
  it("returns 0 when today is absent", () => {
    expect(countTodayRecords(groups, "2026-01-01")).toBe(0);
  });
});

describe("label and icon maps", () => {
  it("maps each kind to a label", () => {
    expect(getClipKindLabel("text")).toBe("文本");
    expect(getClipKindLabel("link")).toBe("链接");
    expect(getClipKindLabel("code")).toBe("代码");
    expect(getClipKindLabel("secret")).toBe("敏感");
  });
  it("maps each kind to an icon", () => {
    expect(getClipIcon("text")).toBe("文");
    expect(getClipIcon("secret")).toBe("密");
  });
});
```

- [ ] **Step 2: 运行测试，确认通过**

Run: `pnpm.cmd test src/components/clipboard/clipStudioHelpers.test.ts`
Expected: 全部用例 PASS。若 trim 用例失败，对照 `createToolboxResult` 的实现（`replace(/[ \t]+/g, " ").replace(/\n{3,}/g, "\n\n").trim()`）核对断言期望值，调整断言以匹配真实实现（不要改实现）。

- [ ] **Step 3: 确认 tsc 通过**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 无错误。

- [ ] **Step 4: Commit**

```bash
git add src/components/clipboard/clipStudioHelpers.test.ts
git commit -m "test(frontend): 覆盖 clipStudioHelpers 分类/过滤/工具箱/计数"
```

---

### Task 3: skipMessage 测试（含最小 export 改动）

**Files:**
- Modify: `src/hooks/useClipboardEvents.ts`（`skipMessage` 加 `export`）
- Create: `src/hooks/useClipboardEvents.test.ts`

- [ ] **Step 1: 给 skipMessage 加 export**

文件 `src/hooks/useClipboardEvents.ts`，函数当前定义为：
```ts
function skipMessage(event: ClipboardSkippedEvent) {
```
改为：
```ts
export function skipMessage(event: ClipboardSkippedEvent) {
```
不改任何逻辑、不移动文件。`useClipboardEvents` 内部对 `skipMessage` 的调用无需改动。

- [ ] **Step 2: 写测试文件**

创建 `src/hooks/useClipboardEvents.test.ts`：
```ts
import { skipMessage } from "@/hooks/useClipboardEvents";
import type { ClipboardSkippedEvent } from "@/types/clipboard";

function makeEvent(overrides: Partial<ClipboardSkippedEvent> = {}): ClipboardSkippedEvent {
  return { reason: "tooLong", contentLength: 10, maxTextLength: 5000, ...overrides };
}

describe("skipMessage", () => {
  it("includes the max length for tooLong", () => {
    const message = skipMessage(makeEvent({ reason: "tooLong", maxTextLength: 5000 }));
    expect(message).toContain("5000");
  });
  it("returns the secret-like message", () => {
    expect(skipMessage(makeEvent({ reason: "secretLike" }))).toBe("疑似敏感内容已按设置跳过。");
  });
  it("returns the default message for other reasons", () => {
    expect(skipMessage(makeEvent({ reason: "duplicate" }))).toBe("该剪贴板内容已跳过。");
  });
});
```

- [ ] **Step 3: 运行测试，确认通过**

Run: `pnpm.cmd test src/hooks/useClipboardEvents.test.ts`
Expected: 3 个用例 PASS。

- [ ] **Step 4: 确认 tsc 通过**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 无错误。

- [ ] **Step 5: Commit**

```bash
git add src/hooks/useClipboardEvents.ts src/hooks/useClipboardEvents.test.ts
git commit -m "test(frontend): 覆盖 skipMessage 跳过原因文案"
```

---

### Task 4: useClipboardWorkspace 初始加载与 selectDate（建立 hook 测试骨架）

**Files:**
- Create: `src/hooks/useClipboardWorkspace.test.tsx`

这是 hook 测试的第一个文件，建立 mock 骨架。后续 Task 5 在同文件追加用例。

- [ ] **Step 1: 写测试文件（mock 骨架 + 初始加载 + selectDate）**

创建 `src/hooks/useClipboardWorkspace.test.tsx`：
```tsx
// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";

import { useClipboardWorkspace } from "@/hooks/useClipboardWorkspace";
import type {
  ClipboardDateGroup,
  ClipboardItem,
  ClipboardMonitorStatus,
  DesktopSettings,
} from "@/types/clipboard";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

function makeItem(overrides: Partial<ClipboardItem> = {}): ClipboardItem {
  return {
    id: 1,
    contentType: "text",
    content: "hello",
    preview: "hello",
    contentHash: "h",
    createdAt: "2026-05-29T00:00:00Z",
    lastCopiedAt: "2026-05-29T00:00:00Z",
    copyCount: 1,
    ...overrides,
  };
}

function makeSettings(overrides: Partial<DesktopSettings> = {}): DesktopSettings {
  return {
    autostartEnabled: false,
    monitorEnabled: true,
    retentionDays: 30,
    maxRecordCount: 1000,
    maxTextLength: 5000,
    ignorePasswordLikeText: false,
    customSecretPatterns: "",
    storageDir: "",
    ...overrides,
  };
}

const dates: ClipboardDateGroup[] = [{ date: "2026-05-29", count: 2 }];
const items: ClipboardItem[] = [makeItem({ id: 1 }), makeItem({ id: 2, content: "world" })];
const monitorStatus: ClipboardMonitorStatus = { enabled: true };

function defaultInvoke(command: string): unknown {
  switch (command) {
    case "list_clipboard_dates":
      return Promise.resolve(dates);
    case "list_clipboard_items":
      return Promise.resolve(items);
    case "search_clipboard_items":
      return Promise.resolve(items);
    case "get_clipboard_monitor_status":
      return Promise.resolve(monitorStatus);
    case "get_desktop_settings":
      return Promise.resolve(makeSettings());
    default:
      return Promise.resolve(undefined);
  }
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => defaultInvoke(command));
});

describe("useClipboardWorkspace initial load", () => {
  it("loads dates, items, and selects the first item", async () => {
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toHaveLength(2));
    expect(result.current.dates).toEqual(dates);
    expect(result.current.selectedItem?.id).toBe(1);
  });
});

describe("useClipboardWorkspace selectDate", () => {
  it("updates selectedDate, clears search, sets message", async () => {
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toHaveLength(2));

    act(() => result.current.selectDate("2026-05-20"));

    expect(result.current.selectedDate).toBe("2026-05-20");
    expect(result.current.searchTerm).toBe("");
    expect(result.current.message).toContain("2026-05-20");
  });
});
```

- [ ] **Step 2: 运行测试，确认通过**

Run: `pnpm.cmd test src/hooks/useClipboardWorkspace.test.tsx`
Expected: 2 个用例 PASS。若报 `document is not defined`，确认文件首行的 `// @vitest-environment jsdom` 注释存在且在最顶部。若 mock 未生效（invoke 抛错），确认 `vi.mock` 路径字符串与源码 import（`@tauri-apps/api/core` / `@tauri-apps/api/event`）完全一致。

- [ ] **Step 3: 确认 tsc 通过**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 无错误。

- [ ] **Step 4: Commit**

```bash
git add src/hooks/useClipboardWorkspace.test.tsx
git commit -m "test(frontend): useClipboardWorkspace 初始加载与 selectDate"
```

---

### Task 5: useClipboardWorkspace 的 action 行为（toggle/clear/delete/updateSettings）

**Files:**
- Modify: `src/hooks/useClipboardWorkspace.test.tsx`（追加 describe 块）

- [ ] **Step 1: 追加 action 行为用例**

在 `src/hooks/useClipboardWorkspace.test.tsx` 末尾追加：
```tsx
describe("useClipboardWorkspace toggleMonitor", () => {
  it("flips monitor state and sets pause message", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "set_clipboard_monitor_enabled") {
        return Promise.resolve({ enabled: false });
      }
      return defaultInvoke(command);
    });
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toHaveLength(2));

    await act(async () => {
      await result.current.toggleMonitor();
    });

    expect(result.current.monitorEnabled).toBe(false);
    expect(result.current.message).toContain("暂停");
  });
});

describe("useClipboardWorkspace clearDate", () => {
  it("invokes clear command and sets cleared message", async () => {
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toHaveLength(2));

    await act(async () => {
      await result.current.clearDate();
    });

    expect(invokeMock).toHaveBeenCalledWith("clear_clipboard_items_by_date", expect.anything());
    expect(result.current.message).toContain("已清空");
  });
});

describe("useClipboardWorkspace deleteItem", () => {
  it("invokes delete command and sets deleted message", async () => {
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toHaveLength(2));

    await act(async () => {
      await result.current.deleteItem(makeItem({ id: 2 }));
    });

    expect(invokeMock).toHaveBeenCalledWith("delete_clipboard_item", expect.anything());
    expect(result.current.message).toContain("已删除");
  });
});

describe("useClipboardWorkspace updateSettings", () => {
  it("saves settings and sets success message on success", async () => {
    const saved = makeSettings({ retentionDays: 7 });
    invokeMock.mockImplementation((command: string) => {
      if (command === "update_desktop_settings") {
        return Promise.resolve(saved);
      }
      return defaultInvoke(command);
    });
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toHaveLength(2));

    await act(async () => {
      await result.current.updateSettings(makeSettings({ retentionDays: 7 }));
    });

    expect(result.current.desktopSettings?.retentionDays).toBe(7);
    expect(result.current.message).toContain("已保存");
    expect(result.current.isBusy).toBe(false);
  });

  it("sets errorMessage when update fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "update_desktop_settings") {
        return Promise.reject(new Error("boom"));
      }
      return defaultInvoke(command);
    });
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toHaveLength(2));

    await act(async () => {
      await result.current.updateSettings(makeSettings());
    });

    expect(result.current.errorMessage).toContain("boom");
    expect(result.current.isBusy).toBe(false);
  });
});
```

- [ ] **Step 2: 运行测试，确认通过**

Run: `pnpm.cmd test src/hooks/useClipboardWorkspace.test.tsx`
Expected: 初始 2 个 + 新增 5 个 = 7 个用例全 PASS。若 toggle 用例的 message 断言失败，对照 `createMonitorToggle` 的实际文案（启用→"已恢复剪贴板监听。"，禁用→"已暂停剪贴板监听。"）调整断言关键字。

- [ ] **Step 3: 确认 tsc 通过**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 无错误。

- [ ] **Step 4: Commit**

```bash
git add src/hooks/useClipboardWorkspace.test.tsx
git commit -m "test(frontend): useClipboardWorkspace 监听/清空/删除/设置行为"
```

---

### Task 6: build 门禁纳入测试 + 标记审查项

**Files:**
- Modify: `package.json`（`build` 脚本）
- Modify: `docs/2026-05-28-clipboard-toolbox-audit.md`（标记 P1 #8）

- [ ] **Step 1: 把测试纳入 build**

`package.json` 的 `build` 脚本由 `"tsc && vite build"` 改为：
```json
    "build": "tsc && vitest run && vite build",
```

- [ ] **Step 2: 全量运行测试**

Run: `pnpm.cmd test`
Expected: 4 个测试文件、全部用例 PASS（date 2 + helpers ~17 + skipMessage 3 + workspace 7 ≈ 29+）。

- [ ] **Step 3: 跑完整 build 链**

Run: `pnpm.cmd build`
Expected: `tsc` 无错误 → `vitest run` 全绿 → `vite build` 产物生成，整链成功退出。

- [ ] **Step 4: 标记审查项 P1 #8**

文件 `docs/2026-05-28-clipboard-toolbox-audit.md`：
- 找到 `### 8. 前端零单元测试` 标题，行尾加 ` ✅ 2026-05-29 已修复`（与 P0 #1-#5 的标记风格一致）。
- 审查清单表格行 `| P1 | 8 | 前端零测试 | 可维护性 |` 保持不变（既有 P0 已修复项也只在节标题加标记，表格无状态列，保持一致）。

- [ ] **Step 5: Commit**

```bash
git add package.json docs/2026-05-28-clipboard-toolbox-audit.md
git commit -m "test(frontend): 测试纳入 build 门禁并标记 P1 #8 修复"
```

---

## 验证（全部 Task 完成后）

- `pnpm.cmd test`：4 个测试文件全绿（~29+ 用例）。
- `pnpm.cmd build`（`tsc && vitest run && vite build`）：类型 + 测试 + 生产构建整链通过。
- 后端零改动（不跑 cargo）。
- 不引 CI、不加 git hook。
