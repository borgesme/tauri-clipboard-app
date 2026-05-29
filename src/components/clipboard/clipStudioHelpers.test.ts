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
    contentHash: "hash",
    createdAt: "2026-05-29T00:00:00Z",
    lastCopiedAt: "2026-05-29T00:00:00Z",
    copyCount: 1,
    ...overrides,
  };
}

describe("getClipKind", () => {
  it("classifies JWT-like content as secret", () => {
    expect(getClipKind(makeItem({ content: "eyJhbGci.eyJzdWIi.SflKxwRJ" }))).toBe("secret");
  });

  it("classifies api_key / token / secret keywords as secret", () => {
    expect(getClipKind(makeItem({ content: "my api_key here" }))).toBe("secret");
    expect(getClipKind(makeItem({ content: "bearer token value" }))).toBe("secret");
    expect(getClipKind(makeItem({ content: "the secret sauce" }))).toBe("secret");
  });

  it("classifies http(s) urls as link", () => {
    expect(getClipKind(makeItem({ content: "https://example.com" }))).toBe("link");
    expect(getClipKind(makeItem({ content: "http://example.com" }))).toBe("link");
  });

  it("classifies code-like content as code", () => {
    expect(getClipKind(makeItem({ content: "title\nconst x = 1" }))).toBe("code");
    expect(getClipKind(makeItem({ content: "doThing();" }))).toBe("code");
    expect(getClipKind(makeItem({ content: "value }" }))).toBe("code");
  });

  it("falls back to text", () => {
    expect(getClipKind(makeItem({ content: "just a plain note" }))).toBe("text");
  });

  it("prefers secret over link when both match", () => {
    expect(getClipKind(makeItem({ content: "https://example.com?token=abc" }))).toBe("secret");
  });
});

describe("filterClipboardItems", () => {
  const items = [
    makeItem({ id: 1, content: "plain text", copyCount: 1 }),
    makeItem({ id: 2, content: "https://example.com", copyCount: 3 }),
    makeItem({ id: 3, content: "the secret value", copyCount: 1 }),
  ];

  it("returns all items unchanged for 'all'", () => {
    expect(filterClipboardItems(items, "all")).toEqual(items);
  });

  it("keeps only copyCount > 1 for 'frequent'", () => {
    expect(filterClipboardItems(items, "frequent").map((i) => i.id)).toEqual([2]);
  });

  it("filters by kind", () => {
    expect(filterClipboardItems(items, "link").map((i) => i.id)).toEqual([2]);
    expect(filterClipboardItems(items, "secret").map((i) => i.id)).toEqual([3]);
  });
});

describe("createToolboxResult", () => {
  it("trim collapses spaces and blank lines and trims ends", () => {
    expect(createToolboxResult("trim", "  a   b  ")).toBe("a b");
    expect(createToolboxResult("trim", "a\n\n\n\nb")).toBe("a\n\nb");
  });

  it("upper / lower convert case", () => {
    expect(createToolboxResult("upper", "aBc")).toBe("ABC");
    expect(createToolboxResult("lower", "aBc")).toBe("abc");
  });

  it("markdown wraps urls, text, and blanks", () => {
    expect(createToolboxResult("markdown", "https://example.com")).toBe(
      "[链接标题](https://example.com)",
    );
    expect(createToolboxResult("markdown", "hello")).toBe("[hello](https://example.com)");
    expect(createToolboxResult("markdown", "   ")).toBe("[链接标题](https://example.com)");
  });
});

describe("counting helpers", () => {
  const dates: ClipboardDateGroup[] = [
    { date: "2026-05-28", count: 4 },
    { date: "2026-05-29", count: 7 },
  ];

  it("countRecords sums all group counts", () => {
    expect(countRecords(dates)).toBe(11);
  });

  it("countTodayRecords returns matching count or 0", () => {
    expect(countTodayRecords(dates, "2026-05-29")).toBe(7);
    expect(countTodayRecords(dates, "2026-01-01")).toBe(0);
  });
});

describe("label / icon maps", () => {
  it("maps each kind to a label", () => {
    expect(getClipKindLabel("text")).toBe("文本");
    expect(getClipKindLabel("link")).toBe("链接");
    expect(getClipKindLabel("code")).toBe("代码");
    expect(getClipKindLabel("secret")).toBe("敏感");
  });

  it("maps each kind to an icon", () => {
    expect(getClipIcon("text")).toBe("文");
    expect(getClipIcon("link")).toBe("链");
    expect(getClipIcon("code")).toBe("码");
    expect(getClipIcon("secret")).toBe("密");
  });
});
