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
