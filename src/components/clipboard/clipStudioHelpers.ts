import type { ClipboardDateGroup, ClipboardItem } from "@/types/clipboard";

export type ClipFilter = "all" | "text" | "link" | "code" | "secret" | "frequent";
export type ClipKind = "text" | "link" | "code" | "secret";
export type PanelTab = "calendar" | "toolbox" | "settings";
export type ToolboxAction = "trim" | "upper" | "lower" | "markdown";

export function filterClipboardItems(items: ClipboardItem[], filter: ClipFilter) {
  if (filter === "all") {
    return items;
  }
  if (filter === "frequent") {
    return items.filter((item) => item.copyCount > 1);
  }
  return items.filter((item) => getClipKind(item) === filter);
}

export function getClipKind(item: ClipboardItem): ClipKind {
  const content = item.content.trim();
  if (isSecretLike(content)) {
    return "secret";
  }
  if (/^https?:\/\//i.test(content)) {
    return "link";
  }
  if (isCodeLike(content)) {
    return "code";
  }
  return "text";
}

export function getClipKindLabel(kind: ClipKind) {
  const labels: Record<ClipKind, string> = {
    text: "文本",
    link: "链接",
    code: "代码",
    secret: "敏感",
  };
  return labels[kind];
}

export function getClipIcon(kind: ClipKind) {
  const icons: Record<ClipKind, string> = {
    text: "文",
    link: "链",
    code: "码",
    secret: "密",
  };
  return icons[kind];
}

export function formatTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

export function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

export function countRecords(dates: ClipboardDateGroup[]) {
  return dates.reduce((total, group) => total + group.count, 0);
}

export function countTodayRecords(dates: ClipboardDateGroup[], today: string) {
  return dates.find((group) => group.date === today)?.count ?? 0;
}

export function createToolboxResult(action: ToolboxAction, value: string) {
  if (action === "trim") {
    return value.replace(/[ \t]+/g, " ").replace(/\n{3,}/g, "\n\n").trim();
  }
  if (action === "upper") {
    return value.toUpperCase();
  }
  if (action === "lower") {
    return value.toLowerCase();
  }
  return toMarkdownLink(value);
}

function isSecretLike(content: string) {
  return /\b(eyJ[a-z0-9_-]+\.[a-z0-9_-]+\.[a-z0-9_-]+|api[_-]?key|token|secret)\b/i.test(content);
}

function isCodeLike(content: string) {
  return /\n\s*(const|let|fn|function|class|import|export)\b/.test(content) || /[{};]\s*$/.test(content);
}

function toMarkdownLink(value: string) {
  const trimmed = value.trim();
  if (/^https?:\/\//i.test(trimmed)) {
    return `[链接标题](${trimmed})`;
  }
  return `[${trimmed || "链接标题"}](https://example.com)`;
}
