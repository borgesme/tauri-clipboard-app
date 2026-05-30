// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";

import { useUndoToast } from "@/hooks/useUndoToast";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useUndoToast", () => {
  it("exposes pending state after show", () => {
    const { result } = renderHook(() => useUndoToast(6000));

    act(() => result.current.show({ ids: [1, 2], date: "2026-05-29", count: 2 }));

    expect(result.current.pending).toEqual({ ids: [1, 2], date: "2026-05-29", count: 2 });
  });

  it("auto-dismisses pending after durationMs", () => {
    const { result } = renderHook(() => useUndoToast(6000));

    act(() => result.current.show({ ids: [1], date: "2026-05-29", count: 1 }));
    act(() => vi.advanceTimersByTime(6000));

    expect(result.current.pending).toBeNull();
  });

  it("replaces pending and resets the timer on a second show", () => {
    const { result } = renderHook(() => useUndoToast(6000));

    act(() => result.current.show({ ids: [1], date: "2026-05-29", count: 1 }));
    act(() => vi.advanceTimersByTime(4000));
    act(() => result.current.show({ ids: [2, 3], date: "2026-05-28", count: 2 }));

    // 再过 4s（旧批次本应在此前消失）：计时被重置，pending 仍是新批次
    act(() => vi.advanceTimersByTime(4000));
    expect(result.current.pending).toEqual({ ids: [2, 3], date: "2026-05-28", count: 2 });

    // 新批次满 6s 后才归 null
    act(() => vi.advanceTimersByTime(2000));
    expect(result.current.pending).toBeNull();
  });

  it("clears pending immediately and stops the timer", () => {
    const { result } = renderHook(() => useUndoToast(6000));

    act(() => result.current.show({ ids: [1], date: "2026-05-29", count: 1 }));
    act(() => result.current.clear());

    expect(result.current.pending).toBeNull();
    act(() => vi.advanceTimersByTime(6000));
    expect(result.current.pending).toBeNull();
  });
});
