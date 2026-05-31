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
