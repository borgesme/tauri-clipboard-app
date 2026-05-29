import { skipMessage } from "@/hooks/useClipboardEvents";
import type { ClipboardSkippedEvent } from "@/types/clipboard";

function makeEvent(overrides: Partial<ClipboardSkippedEvent>): ClipboardSkippedEvent {
  return {
    reason: "tooLong",
    contentLength: 100,
    maxTextLength: 5000,
    ...overrides,
  };
}

describe("skipMessage", () => {
  it("includes the max text length for tooLong", () => {
    expect(skipMessage(makeEvent({ reason: "tooLong", maxTextLength: 5000 }))).toBe(
      "该剪贴板内容超过单条文本上限（5000 字），已跳过。",
    );
  });

  it("returns the secret-skip message for secretLike", () => {
    expect(skipMessage(makeEvent({ reason: "secretLike" }))).toBe("疑似敏感内容已按设置跳过。");
  });

  it("falls back to the generic skip message for other reasons", () => {
    expect(skipMessage(makeEvent({ reason: "duplicate" }))).toBe("该剪贴板内容已跳过。");
  });
});
