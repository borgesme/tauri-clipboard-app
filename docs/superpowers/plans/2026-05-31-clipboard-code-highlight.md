# 复制条目代码高亮 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 当剪贴板条目内容为代码时，在「剪贴板详情」与「工具箱送入区」展示 highlight.js 语法高亮。

**Architecture:** 新增纯函数 `highlightCode`（封装 highlight.js `highlightAuto`，含空/超长/异常回退）与展示组件 `CodeBlock`；详情对话框在 `kind === "code"` 时改用 `CodeBlock`；工具箱送入区在内容为代码时提供「编辑/预览」切换，预览态用 `CodeBlock`。仅 `getClipKind() === "code"` 触发高亮。

**Tech Stack:** React 19 + TypeScript + Vite + Tailwind 4 + Tauri 2；highlight.js（common 子集）；vitest + @testing-library/react（默认 `node` 环境，组件测试需 `// @vitest-environment jsdom`，`globals: true`）。

**测试命令约定（Windows PowerShell）：** 用 `pnpm.cmd`。跑单个文件：`pnpm.cmd test <文件路径>`（透传给 `vitest run`）。

**Spec：** `docs/superpowers/specs/2026-05-31-clipboard-code-highlight-design.md`

---

### Task 1: 引入 highlight.js 依赖

**Files:**
- Modify: `package.json`（dependencies 增加 `highlight.js`）

- [ ] **Step 1: 安装依赖**

Run: `pnpm.cmd add highlight.js`
Expected: `package.json` 的 `dependencies` 出现 `"highlight.js": "^11..."`，`pnpm-lock.yaml` 更新。

- [ ] **Step 2: 验证类型可用（无需额外 @types）**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 无新增类型错误（highlight.js v11 自带类型声明）。

- [ ] **Step 3: 提交**

```bash
git add package.json pnpm-lock.yaml
git commit -m "chore(clipboard): 引入 highlight.js 依赖"
```

---

### Task 2: `highlightCode` 核心模块（TDD）

**Files:**
- Create: `src/lib/highlight.ts`
- Test: `src/lib/highlight.test.ts`

- [ ] **Step 1: 写失败测试**

`src/lib/highlight.test.ts`（纯函数，沿用默认 node 环境，不加 jsdom 注释）：

```ts
import { highlightCode } from "@/lib/highlight";

describe("highlightCode", () => {
  it("highlights real code and reports a language", () => {
    const result = highlightCode('const greeting = "hello";\nfunction add(a, b) { return a + b; }');
    expect(result.language).not.toBeNull();
    expect(result.html).toContain("hljs-");
  });

  it("highlights json content", () => {
    const result = highlightCode('{\n  "name": "clip",\n  "count": 3\n}');
    expect(result.language).not.toBeNull();
    expect(result.html).toContain("hljs-");
  });

  it("returns nulls for empty content", () => {
    expect(highlightCode("")).toEqual({ html: null, language: null });
  });

  it("skips highlighting for content over the length cap", () => {
    const result = highlightCode("x".repeat(20001));
    expect(result.html).toBeNull();
    expect(result.language).toBeNull();
  });

  it("escapes html so source angle brackets cannot inject markup", () => {
    const result = highlightCode('const x = "<script>alert(1)</script>";');
    expect(result.html).not.toContain("<script>");
  });
});
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `pnpm.cmd test src/lib/highlight.test.ts`
Expected: FAIL —— 无法解析 `@/lib/highlight` / `highlightCode is not a function`。

- [ ] **Step 3: 写实现**

`src/lib/highlight.ts`：

```ts
import hljs from "highlight.js/lib/common";

export interface HighlightResult {
  /** 高亮后的安全 HTML（源码已转义 + token span）；null 表示未高亮，调用方应回退纯文本 */
  html: string | null;
  /** 检测到的语言；null 表示未识别或回退 */
  language: string | null;
}

const MAX_LENGTH = 20000;

export function highlightCode(content: string): HighlightResult {
  if (!content || content.length > MAX_LENGTH) {
    return { html: null, language: null };
  }
  try {
    const result = hljs.highlightAuto(content);
    return { html: result.value, language: result.language ?? null };
  } catch {
    return { html: null, language: null };
  }
}
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `pnpm.cmd test src/lib/highlight.test.ts`
Expected: PASS（5 个用例全过）。

- [ ] **Step 5: 提交**

```bash
git add src/lib/highlight.ts src/lib/highlight.test.ts
git commit -m "feat(clipboard): 新增 highlightCode 代码高亮核心模块"
```

---

### Task 3: `CodeBlock` 展示组件 + 主题样式（TDD）

**Files:**
- Create: `src/components/clipboard/CodeBlock.tsx`
- Test: `src/components/clipboard/CodeBlock.test.tsx`
- Modify: `src/main.tsx`（引入 highlight.js 主题 CSS）
- Modify: `src/App.css`（`.code-block` 容器样式）

- [ ] **Step 1: 写失败测试**

`src/components/clipboard/CodeBlock.test.tsx`（需 jsdom）：

```tsx
// @vitest-environment jsdom
import { render } from "@testing-library/react";

import { CodeBlock } from "@/components/clipboard/CodeBlock";

describe("CodeBlock", () => {
  it("renders a highlighted code element for code content", () => {
    const { container } = render(<CodeBlock content={'const x = 1;\nfunction f() { return x; }'} />);
    const code = container.querySelector("code.hljs");
    expect(code).not.toBeNull();
    expect(code?.querySelector("span.hljs-keyword")).not.toBeNull();
  });

  it("falls back to plain text for over-cap content without injecting html", () => {
    const long = "x".repeat(20001);
    const { container } = render(<CodeBlock content={long} />);
    expect(container.querySelector("code")?.textContent).toBe(long);
  });

  it("does not create a real script node for malicious content", () => {
    const { container } = render(<CodeBlock content={'<script>alert(1)</script>'} />);
    expect(container.querySelector("script")).toBeNull();
  });
});
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `pnpm.cmd test src/components/clipboard/CodeBlock.test.tsx`
Expected: FAIL —— 无法解析 `@/components/clipboard/CodeBlock`。

- [ ] **Step 3: 写组件**

`src/components/clipboard/CodeBlock.tsx`：

```tsx
import { useMemo } from "react";

import { highlightCode } from "@/lib/highlight";
import { cn } from "@/lib/utils";

interface CodeBlockProps {
  content: string;
  className?: string;
}

export function CodeBlock({ content, className }: CodeBlockProps) {
  const { html, language } = useMemo(() => highlightCode(content), [content]);
  return (
    <pre className={cn("code-block", className)} data-language={language ?? undefined}>
      {html !== null ? (
        <code className="hljs" dangerouslySetInnerHTML={{ __html: html }} />
      ) : (
        <code className="hljs">{content}</code>
      )}
    </pre>
  );
}
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `pnpm.cmd test src/components/clipboard/CodeBlock.test.tsx`
Expected: PASS（3 个用例全过）。

> 备注：第一个用例断言 `span.hljs-keyword` 存在——`const`/`function` 是 highlightAuto 稳定识别的关键字。若某版本 token class 命名变化导致失败，放宽为 `expect(code?.innerHTML).toContain("hljs-")`。

- [ ] **Step 5: 引入主题 CSS 与容器样式**

在 `src/main.tsx` 顶部，紧跟 `import "./index.css";` 之后加：

```tsx
import "highlight.js/styles/github.css";
```

在 `src/App.css` 末尾追加：

```css
.code-block{margin:0;max-height:340px;overflow:auto;border:1px solid var(--clip-border-strong);border-radius:14px;background:#fff;}
.code-block code.hljs{display:block;padding:14px;background:transparent;font-family:ui-monospace,SFMono-Regular,"Consolas","Liberation Mono",monospace;font-size:13px;line-height:1.55;white-space:pre;}
```

- [ ] **Step 6: 类型与构建自检**

Run: `pnpm.cmd exec tsc --noEmit`
Expected: 无错误（CSS import 副作用不影响 tsc）。

- [ ] **Step 7: 提交**

```bash
git add src/components/clipboard/CodeBlock.tsx src/components/clipboard/CodeBlock.test.tsx src/main.tsx src/App.css
git commit -m "feat(clipboard): 新增 CodeBlock 高亮展示组件与主题样式"
```

---

### Task 4: 抽出 `getClipKindFromContent`（TDD）

**Files:**
- Modify: `src/components/clipboard/clipStudioHelpers.ts`
- Test: `src/components/clipboard/clipStudioHelpers.test.ts`（补充）

- [ ] **Step 1: 写失败测试（在文件现有 `describe("getClipKind", ...)` 之后追加）**

```ts
describe("getClipKindFromContent", () => {
  it("classifies content the same way getClipKind classifies an item", () => {
    const samples = [
      "eyJhbGci.eyJzdWIi.SflKxwRJ",
      "https://example.com",
      "title\nconst x = 1",
      "doThing();",
      "just a plain note",
    ];
    for (const content of samples) {
      expect(getClipKindFromContent(content)).toBe(getClipKind(makeItem({ content })));
    }
  });

  it("normalizes surrounding whitespace before classifying", () => {
    expect(getClipKindFromContent("   https://example.com   ")).toBe("link");
    expect(getClipKindFromContent("\n\n  just a plain note  \n")).toBe("text");
  });
});
```

并在文件顶部的 import 列表中加入 `getClipKindFromContent`：

```ts
import {
  countRecords,
  countTodayRecords,
  createToolboxResult,
  filterClipboardItems,
  getClipIcon,
  getClipKind,
  getClipKindFromContent,
  getClipKindLabel,
} from "@/components/clipboard/clipStudioHelpers";
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `pnpm.cmd test src/components/clipboard/clipStudioHelpers.test.ts`
Expected: FAIL —— `getClipKindFromContent` 未导出。

- [ ] **Step 3: 重构实现**

编辑 `src/components/clipboard/clipStudioHelpers.ts`，把现有 `getClipKind` 函数体替换为：

```ts
export function getClipKind(item: ClipboardItem): ClipKind {
  return getClipKindFromContent(item.content);
}

export function getClipKindFromContent(content: string): ClipKind {
  const trimmed = content.trim();
  if (isSecretLike(trimmed)) {
    return "secret";
  }
  if (/^https?:\/\//i.test(trimmed)) {
    return "link";
  }
  if (isCodeLike(trimmed)) {
    return "code";
  }
  return "text";
}
```

（原 `getClipKind` 对 `isSecretLike`/链接正则传入未 trim 的 `content`，重构统一改为 `trimmed`——既有测试用例内容均无前后空白，分类结果不变；新增的空白归一化是预期改进。）

- [ ] **Step 4: 运行测试，确认通过（含既有 `getClipKind` 用例不回归）**

Run: `pnpm.cmd test src/components/clipboard/clipStudioHelpers.test.ts`
Expected: PASS（既有 `getClipKind`/`filterClipboardItems` 等用例 + 新增 `getClipKindFromContent` 用例全过）。

- [ ] **Step 5: 提交**

```bash
git add src/components/clipboard/clipStudioHelpers.ts src/components/clipboard/clipStudioHelpers.test.ts
git commit -m "refactor(clipboard): 抽出 getClipKindFromContent 支持裸字符串判类"
```

---

### Task 5: 剪贴板详情接入高亮（TDD）

**Files:**
- Modify: `src/components/clipboard/ClipStudioDetailDialog.tsx`
- Test: `src/components/clipboard/ClipStudioDetailDialog.test.tsx`（新增）

- [ ] **Step 1: 写失败测试**

`src/components/clipboard/ClipStudioDetailDialog.test.tsx`（需 jsdom）：

```tsx
// @vitest-environment jsdom
import { render } from "@testing-library/react";

import { ClipStudioDetailDialog } from "@/components/clipboard/ClipStudioDetailDialog";
import type { ClipboardItem } from "@/types/clipboard";

function makeItem(overrides: Partial<ClipboardItem> = {}): ClipboardItem {
  return {
    id: 1,
    contentType: "text",
    content: "hello",
    preview: "hello",
    contentHash: "hash",
    createdAt: "2026-05-31T00:00:00Z",
    lastCopiedAt: "2026-05-31T00:00:00Z",
    copyCount: 1,
    ...overrides,
  };
}

const noop = () => {};

describe("ClipStudioDetailDialog", () => {
  it("renders a highlighted code block for code content", () => {
    const item = makeItem({ content: "title\nconst x = 1;\nfunction f() { return x; }" });
    const { container } = render(
      <ClipStudioDetailDialog item={item} onClose={noop} onCopy={noop} onDelete={noop} onSendToToolbox={noop} />,
    );
    expect(container.querySelector("code.hljs")).not.toBeNull();
    expect(container.querySelector(".detail-content")).toBeNull();
  });

  it("renders plain text for non-code content", () => {
    const item = makeItem({ content: "just a plain note" });
    const { container } = render(
      <ClipStudioDetailDialog item={item} onClose={noop} onCopy={noop} onDelete={noop} onSendToToolbox={noop} />,
    );
    expect(container.querySelector(".detail-content")).not.toBeNull();
    expect(container.querySelector("code.hljs")).toBeNull();
  });
});
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `pnpm.cmd test src/components/clipboard/ClipStudioDetailDialog.test.tsx`
Expected: FAIL —— code 内容仍渲染 `.detail-content`，断言 `code.hljs` 存在失败。

- [ ] **Step 3: 改组件**

在 `src/components/clipboard/ClipStudioDetailDialog.tsx` 顶部 import 区加入：

```tsx
import { CodeBlock } from "@/components/clipboard/CodeBlock";
```

把 `<div className="detail-body">` 内的这一行：

```tsx
          <div className="detail-content">{item.content}</div>
```

替换为：

```tsx
          {kind === "code" ? (
            <CodeBlock content={item.content} />
          ) : (
            <div className="detail-content">{item.content}</div>
          )}
```

（`const kind = getClipKind(item);` 已存在于组件中，直接复用。）

- [ ] **Step 4: 运行测试，确认通过**

Run: `pnpm.cmd test src/components/clipboard/ClipStudioDetailDialog.test.tsx`
Expected: PASS（2 个用例全过）。

- [ ] **Step 5: 提交**

```bash
git add src/components/clipboard/ClipStudioDetailDialog.tsx src/components/clipboard/ClipStudioDetailDialog.test.tsx
git commit -m "feat(clipboard): 剪贴板详情对代码内容启用高亮"
```

---

### Task 6: 工具箱送入区编辑/预览切换 + 高亮（TDD）

**Files:**
- Modify: `src/components/clipboard/ClipStudioPanel.tsx`（`ToolboxPanel`）
- Modify: `src/App.css`（切换按钮样式）
- Test: `src/components/clipboard/ClipStudioPanel.test.tsx`（新增）

- [ ] **Step 1: 写失败测试**

`src/components/clipboard/ClipStudioPanel.test.tsx`（需 jsdom；mock 掉日历/设置子面板以切断 tauri import 链——toolbox tab 下它们本不渲染）：

```tsx
// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";

vi.mock("@/components/clipboard/DesktopSettingsPanel", () => ({
  DesktopSettingsPanel: () => null,
}));
vi.mock("@/components/clipboard/ClipStudioCalendarPanel", () => ({
  ClipStudioCalendarPanel: () => null,
}));

import { ClipStudioPanel } from "@/components/clipboard/ClipStudioPanel";
import type { ClipStudioPanelProps } from "@/components/clipboard/ClipStudioPanel";

function makeProps(overrides: Partial<ClipStudioPanelProps> = {}): ClipStudioPanelProps {
  return {
    activeTab: "toolbox",
    dates: [],
    selectedDate: "2026-05-31",
    today: "2026-05-31",
    frequentCount: 0,
    selectedItem: null,
    toolboxText: "",
    toolboxResult: "",
    desktopSettings: null,
    isBusy: false,
    drawerOpen: true,
    onTabChange: vi.fn(),
    onCloseDrawer: vi.fn(),
    onDateSelect: vi.fn(),
    onToolboxTextChange: vi.fn(),
    onToolboxResultChange: vi.fn(),
    onSendSelectedToToolbox: vi.fn(),
    onCopyToolboxResult: vi.fn(),
    onSettingsChange: vi.fn(),
    onPurgeDeletedItems: vi.fn(),
    onHideWindow: vi.fn(),
    ...overrides,
  };
}

describe("ToolboxPanel highlight", () => {
  it("shows edit/preview switch only when toolbox text is code", () => {
    const { rerender, container } = render(
      <ClipStudioPanel {...makeProps({ toolboxText: "just a plain note" })} />,
    );
    expect(screen.queryByText("预览")).toBeNull();
    expect(container.querySelector("textarea.toolbox-input")).not.toBeNull();

    rerender(<ClipStudioPanel {...makeProps({ toolboxText: "const x = 1;\nfunction f() { return x; }" })} />);
    expect(screen.getByText("预览")).toBeTruthy();
    expect(screen.getByText("编辑")).toBeTruthy();
  });

  it("renders a highlighted code block in preview mode", () => {
    const { container } = render(
      <ClipStudioPanel {...makeProps({ toolboxText: "const x = 1;\nfunction f() { return x; }" })} />,
    );
    expect(container.querySelector("textarea.toolbox-input")).not.toBeNull();

    fireEvent.click(screen.getByText("预览"));
    expect(container.querySelector("code.hljs")).not.toBeNull();
    expect(container.querySelector("textarea.toolbox-input")).toBeNull();
  });
});
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `pnpm.cmd test src/components/clipboard/ClipStudioPanel.test.tsx`
Expected: FAIL —— 当前无「预览」按钮，且 `ClipStudioPanelProps` 未导出（导致 import 报错或断言失败）。

- [ ] **Step 3: 改组件**

(a) 调整 `src/components/clipboard/ClipStudioPanel.tsx` 顶部 import（**只做下列三处新增/修改，其余现有 import 原样保留**——尤其 `import { Copy, Send, Settings, WandSparkles, X } from "lucide-react";`、`Button`、`cn`、类型 import 不要删）：

- 在文件最上方新增：`import { useState } from "react";`
- 在现有组件 import 中新增：`import { CodeBlock } from "@/components/clipboard/CodeBlock";`
- 在现有 `clipStudioHelpers` 的具名 import 中加入 `getClipKindFromContent`，即改为：

```tsx
import {
  type PanelTab,
  type ToolboxAction,
  createToolboxResult,
  getClipKindFromContent,
} from "@/components/clipboard/clipStudioHelpers";
```

(b) 把 `interface ClipStudioPanelProps {` 改为导出（供测试构造 props）：

```tsx
export interface ClipStudioPanelProps {
```

(c) 用以下整体替换现有 `ToolboxPanel` 函数：

```tsx
function ToolboxPanel(props: ClipStudioPanelProps) {
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const isCode = getClipKindFromContent(props.toolboxText) === "code";
  const showPreview = mode === "preview" && isCode;
  return (
    <section className="clip-panel-view">
      <InfoCard icon={<WandSparkles className="size-4" />} title="文本处理工具箱">
        按 <Kbd>T</Kbd> 可把当前选中的剪贴板内容送入这里，减少鼠标操作。
      </InfoCard>
      <div className="panel-card">
        <div className="toolbox-head">
          <Button className="clip-primary-button" size="sm" onClick={props.onSendSelectedToToolbox} disabled={!props.selectedItem}>
            <Send className="size-4" />
            送入工具箱
          </Button>
          {isCode ? (
            <div className="toolbox-mode-switch">
              <button
                type="button"
                className={cn("toolbox-mode-button", mode === "edit" && "active")}
                onClick={() => setMode("edit")}
              >
                编辑
              </button>
              <button
                type="button"
                className={cn("toolbox-mode-button", mode === "preview" && "active")}
                onClick={() => setMode("preview")}
              >
                预览
              </button>
            </div>
          ) : null}
        </div>
        {showPreview ? (
          <CodeBlock content={props.toolboxText} className="toolbox-preview" />
        ) : (
          <textarea
            className="toolbox-input"
            value={props.toolboxText}
            onChange={(event) => props.onToolboxTextChange(event.currentTarget.value)}
          />
        )}
        <div className="tool-grid">
          {toolActions.map((action) => (
            <button
              className="tool-button"
              key={action.value}
              type="button"
              onClick={() => props.onToolboxResultChange(createToolboxResult(action.value, props.toolboxText))}
            >
              {action.label}
            </button>
          ))}
        </div>
        <div className="tool-result">{props.toolboxResult || "转换结果会显示在这里。"}</div>
        <Button className="clip-primary-button mt-3" onClick={props.onCopyToolboxResult} disabled={!props.toolboxResult.trim()}>
          <Copy className="size-4" />
          复制结果
        </Button>
      </div>
    </section>
  );
}
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `pnpm.cmd test src/components/clipboard/ClipStudioPanel.test.tsx`
Expected: PASS（2 个用例全过）。

- [ ] **Step 5: 加切换按钮样式**

先**修改** `src/App.css` 中现有的 `.toolbox-head` 规则（约第 119 行，原为 `display:flex;justify-content:flex-end;margin-bottom:10px;`），改为左对齐并允许子项靠右推：

```css
.toolbox-head{display:flex;align-items:center;gap:8px;margin-bottom:10px;}
```

再在 `src/App.css` 末尾**追加**切换按钮与预览样式：

```css
.toolbox-mode-switch{display:inline-flex;gap:4px;margin-left:auto;}
.toolbox-mode-button{border:1px solid var(--clip-border-strong);border-radius:10px;padding:4px 12px;font-size:13px;background:#fff;color:var(--clip-muted);cursor:pointer;}
.toolbox-mode-button.active{background:var(--clip-focus);color:#fff;border-color:transparent;}
.toolbox-preview{margin-top:0;}
```

（送入按钮自然靠左，`.toolbox-mode-switch` 的 `margin-left:auto` 把切换按钮推到右侧；`--clip-focus` 变量已用于 `.toolbox-input:focus`，无需新增。）

- [ ] **Step 6: 提交**

```bash
git add src/components/clipboard/ClipStudioPanel.tsx src/components/clipboard/ClipStudioPanel.test.tsx src/App.css
git commit -m "feat(clipboard): 工具箱送入区加编辑/预览切换与代码高亮"
```

---

### Task 7: 全量验证与 GUI 肉眼确认

**Files:** 无（仅验证）

- [ ] **Step 1: 全量单测**

Run: `pnpm.cmd test`
Expected: 全部测试通过（含新增 highlight / CodeBlock / helpers / ClipStudioDetailDialog / ClipStudioPanel）。

- [ ] **Step 2: 类型 + 构建**

Run: `pnpm.cmd build`
Expected: `tsc` + `vitest run` + `vite build` 全通过；产物含 highlight.js。

- [ ] **Step 3: GUI 肉眼确认（用户执行）**

启动前先清理占用 1420 端口的残留 `tauri dev` 进程，再运行：`pnpm tauri dev`

请逐项确认：
- 选中代码类条目打开剪贴板详情 → 内容呈现语法高亮、配色与白底协调；
- 普通文本/链接条目详情 → 仍为纯文本（`.detail-content`）；
- 按 T 送入一段代码到工具箱 → 出现「编辑/预览」切换：预览态高亮、编辑态可改文本，处理按钮与「复制结果」正常工作；
- 送入普通文本到工具箱 → 不出现切换按钮，仍为可编辑文本框。

> CSP 提示：`script-src 'self'`、`style-src 'self' 'unsafe-inline'` 足以加载 highlight.js（纯 JS）与主题 CSS，无需改 `tauri.conf.json`。若控制台报 CSP 拦截，先核对是否误引入了需 `wasm-unsafe-eval` 的依赖。
