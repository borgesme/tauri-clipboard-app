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
