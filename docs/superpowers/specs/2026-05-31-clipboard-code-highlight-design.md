# 设计：复制条目代码高亮（剪贴板详情 + 工具箱送入区）

> 设计日期：2026-05-31
> 需求：当剪贴板条目内容是代码时，在「剪贴板详情」与「工具箱送入区」展示语法高亮
> 范围：新增 `src/lib/highlight.ts`、`src/components/clipboard/CodeBlock.tsx` 及测试；改 `src/components/clipboard/{clipStudioHelpers.ts,ClipStudioDetailDialog.tsx,ClipStudioPanel.tsx}`、`src/main.tsx`、`src/App.css`、`package.json`

## 1. 背景与问题

当前代码类内容均以纯文本展示，无任何语法着色：

- **剪贴板详情**（`ClipStudioDetailDialog.tsx`）用 `<div className="detail-content">{item.content}</div>` 平铺，代码与普通文本观感一致，难以快速辨识结构。
- **工具箱送入区**（`ClipStudioPanel.tsx` 的 `ToolboxPanel`）用可编辑 `<textarea className="toolbox-input">` 承载送入的原文，同样无高亮。

`clipStudioHelpers.ts` 的 `getClipKind()` 已能把条目粗分为 `text | link | code | secret`（`isCodeLike()` 启发式正则判断），但这个分类目前只用于列表图标/标签，未驱动任何展示差异。

## 2. 设计目标与非目标

### 目标

- **仅对代码类条目高亮**：当 `getClipKind() === "code"` 时，在剪贴板详情与工具箱送入区渲染语法高亮；其余类型（text/link/secret）保持现有纯文本展示。
- **自动语言识别**：剪贴板内容语言不可控，用 highlight.js `highlightAuto` 自动检测（覆盖 common 子集约 35 种常用语言）。
- **工具箱编辑/预览切换**：送入区在内容为代码时提供「编辑 / 预览」切换——编辑态沿用 `textarea`，预览态显示只读高亮视图。

### 非目标

- **不引入需要改 CSP 的引擎**：选 highlight.js（纯 JS），不选 Shiki（需 `wasm-unsafe-eval`）。现有 CSP `script-src 'self'` / `style-src 'self' 'unsafe-inline'` 不动。
- **不改 `isCodeLike` 判定逻辑**：沿用现有粗判（受其限制，单行/无明显特征的代码不会被识别为 code，因而不高亮）。
- **不做手动语言选择 UI**：仅自动检测。
- **不高亮工具箱转换结果区**（`tool-result`）：用户选定的目标是「送入的原始内容区」。
- **不动 `DetailPanel.tsx`**：探测确认其为死代码（src 内无引用），与本次无关，按「不做无关重构」原则跳过。

## 3. 核心模块设计（新增 `src/lib/highlight.ts`）

封装 highlight.js，对外暴露一个纯函数，吞掉所有异常并在边界情况回退：

```ts
import hljs from "highlight.js/lib/common";

export interface HighlightResult {
  /** 高亮后的安全 HTML（已转义源码 + token span）；null 表示未高亮，调用方应回退纯文本 */
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

- `highlight.js/lib/common` 子集已注册常用语言，体积远小于完整包，且为纯 JS（无 eval / wasm），契合现有 CSP。
- `highlightAuto` 输出的 `value` 已对源码做 HTML 转义，token 仅是 `<span class="hljs-...">` 包裹，无 XSS 风险。
- **回退三类**：空内容、超长（> `MAX_LENGTH`）、`highlightAuto` 抛错 → 均返回 `{ html: null, language: null }`，由组件走纯文本分支。

## 4. 展示组件（新增 `src/components/clipboard/CodeBlock.tsx`）

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

- `html !== null` → 用高亮 HTML；`null`（回退）→ React 文本节点渲染 `content`（自动转义，双重保险）。
- `.code-block` 控制容器（圆角、滚动、最大高度），`.hljs` 由主题 CSS 提供前景/背景配色。

## 5. 类型判断小重构（`clipStudioHelpers.ts`）

工具箱要对裸字符串 `toolboxText` 判断是否代码，而现有 `getClipKind` 只接收 `ClipboardItem`。抽出按内容判断的函数，原函数复用之（对外行为不变）：

```ts
export function getClipKind(item: ClipboardItem): ClipKind {
  return getClipKindFromContent(item.content);
}

export function getClipKindFromContent(content: string): ClipKind {
  const trimmed = content.trim();
  if (isSecretLike(trimmed)) return "secret";
  if (/^https?:\/\//i.test(trimmed)) return "link";
  if (isCodeLike(trimmed)) return "code";
  return "text";
}
```

> 注：现有 `getClipKind` 对 `isSecretLike` / 链接正则传入的是未 trim 的 `content`、仅对 `isCodeLike` 传 `trim()` 后的值。重构统一改为对 `trimmed` 判断；需在测试中确认这一归一化不改变既有分类结果（见 §8）。

## 6. 剪贴板详情接入（`ClipStudioDetailDialog.tsx`）

已有 `const kind = getClipKind(item);`。仅替换内容展示分支：

```tsx
{kind === "code" ? (
  <CodeBlock content={item.content} />
) : (
  <div className="detail-content">{item.content}</div>
)}
```

code 分支用 `CodeBlock` 自有的 `.code-block` 样式（§8），**不复用** `.detail-content`——后者的白底与 `white-space: pre-wrap` 会和代码块的主题背景、横向滚动冲突。其余结构（meta、操作按钮）不变。

## 7. 工具箱接入（`ClipStudioPanel.tsx` 的 `ToolboxPanel`）

新增组件内局部 UI 状态，默认编辑态（保持现有行为）：

```tsx
const [mode, setMode] = useState<"edit" | "preview">("edit");
const isCode = getClipKindFromContent(props.toolboxText) === "code";
```

- **切换按钮**：仅当 `isCode` 时渲染「编辑 / 预览」切换（非代码无需预览）。
- **内容区**：`mode === "preview" && isCode` → `<CodeBlock content={props.toolboxText} />`（只读高亮）；否则现有 `<textarea className="toolbox-input">`。当用户在编辑态把内容改成非代码后，`isCode` 变 `false`，即使 `mode` 仍是 `preview` 也回落到 textarea。
- **处理按钮 / 复制结果不变**：始终对 `props.toolboxText` 操作，与显示态无关。

## 8. 样式（`src/main.tsx` + `src/App.css`）

- 在 `src/main.tsx` 顶部（紧邻现有全局 CSS 引入）`import "highlight.js/styles/github.css";`（亮色主题，配现有白底）。Vite 打包为 `<style>`（dev）/ 外链 CSS（build），CSP `style-src 'self' 'unsafe-inline'` 均已覆盖。
- 在 `App.css` 末尾补容器样式：`.code-block` 设等宽字体（`ui-monospace, SFMono-Regular, "Consolas", monospace`）、`padding`、圆角、横向滚动、`max-height` 及滚动；与现有 `.detail-content` / `.toolbox-input` 视觉对齐（边框、圆角半径一致）。github 主题的 `.hljs` 背景若与应用色调冲突，在此微调 `background` / `color`。

## 9. 数据流与边缘情况

**主流程**

1. 详情：打开对话框 → `getClipKind(item)` → `code` 则 `CodeBlock(item.content)` → `highlightCode` → `highlightAuto` → HTML 注入。
2. 工具箱：送入后 `toolboxText` 更新 → `getClipKindFromContent` 判定 → `code` 则出现切换按钮 → 点「预览」→ `CodeBlock(toolboxText)`。

**边缘情况**

- **空 / 超长（> 20000 字符）/ 高亮抛错**：`highlightCode` 返回 `html: null` → 组件渲染纯文本（转义），不阻塞、不报错。
- **含 HTML 特殊字符**（如 `<script>`）：`highlightAuto` 已转义；回退路径走 React 文本节点。两条路径均无注入风险（§10 测试覆盖）。
- **工具箱预览态下内容被改为非代码**：`isCode` 变 `false`，内容区回落 textarea（条件含 `&& isCode`）。
- **单行/无明显特征的代码**：受现有 `isCodeLike` 粗判限制不判为 code，不高亮（已知局限，非目标）。
- **`mode` 状态生命周期**：`ToolboxPanel` 为函数组件，切到其它面板再回来时 `mode` 重置为 `edit`，可接受。

## 10. 测试计划（vitest，遵循现有 `*.test.ts(x)` 模式）

**`src/lib/highlight.test.ts`（新增）**

1. JS 片段 → `language` 非 null、`html` 含 `hljs-` token span。
2. JSON 片段 → 正常返回 `html`、`language`。
3. 空字符串 → `{ html: null, language: null }`。
4. 超长内容（length > 20000）→ `html` 为 null。
5. 含 `<script>` 等特殊字符 → `html` 中无未转义的 `<script`（确认转义）。

**`src/components/clipboard/CodeBlock.test.tsx`（新增）**

6. code 内容 → 渲染 `code.hljs`，含 token span。
7. 超长内容 → 走回退分支，渲染纯文本且 `textContent` 等于原文。
8. 含 `<script>alert(1)</script>` → DOM 中不存在真实 `<script>` 节点（XSS 防护）。

**`src/components/clipboard/clipStudioHelpers.test.ts`（补充）**

9. `getClipKindFromContent` 对 code / text / link / secret 各分类正确。
10. `getClipKind(item)` 与 `getClipKindFromContent(item.content)` 结果一致；并校验重构后对「前后空白」的归一化未改变既有用例分类。

**`src/components/clipboard/ClipStudioPanel.test.tsx`（新增，轻量）**

11. `toolboxText` 为代码 → 渲染「编辑/预览」切换；为普通文本 → 不渲染切换。
12. 切到预览态 → 渲染 `CodeBlock`（`code.hljs`）而非 textarea。

## 11. 改动文件清单

- `package.json`：新增依赖 `highlight.js`（`^11`）。
- `src/lib/highlight.ts`（新增）+ `src/lib/highlight.test.ts`（新增）。
- `src/components/clipboard/CodeBlock.tsx`（新增）+ `CodeBlock.test.tsx`（新增）。
- `src/components/clipboard/clipStudioHelpers.ts`：抽出并导出 `getClipKindFromContent`，`getClipKind` 复用；`clipStudioHelpers.test.ts` 补用例。
- `src/components/clipboard/ClipStudioDetailDialog.tsx`：code 分支用 `CodeBlock`。
- `src/components/clipboard/ClipStudioPanel.tsx`：`ToolboxPanel` 加 edit/preview 切换；新增 `ClipStudioPanel.test.tsx`。
- `src/main.tsx`：引入 highlight.js 主题 CSS。
- `src/App.css`：`.code-block` / `.hljs` 容器样式微调。

## 12. 验证

- `pnpm.cmd test`：前端用例通过（highlight / CodeBlock / helpers / ToolboxPanel）。
- `pnpm.cmd build`：`tsc` + `vitest run` + `vite build` 通过（确认无类型错误、打包含 highlight.js）。
- `pnpm tauri dev`（用户肉眼确认；启动前清理占用 1420 端口的残留进程）：
  - 选中代码类条目打开详情 → 内容语法高亮、配色协调；
  - 普通文本/链接条目详情 → 仍纯文本；
  - 送入代码到工具箱 → 出现「编辑/预览」切换，预览态高亮、编辑态可改，处理按钮与复制结果正常；
  - 送入普通文本 → 无切换按钮。
