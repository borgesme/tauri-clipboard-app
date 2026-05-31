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
