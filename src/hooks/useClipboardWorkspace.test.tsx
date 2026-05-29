// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";

import { useClipboardWorkspace } from "@/hooks/useClipboardWorkspace";
import type {
  ClipboardDateGroup,
  ClipboardItem,
  DesktopSettings,
} from "@/types/clipboard";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

const DATES: ClipboardDateGroup[] = [
  { date: "2026-05-29", count: 2 },
  { date: "2026-05-28", count: 1 },
];

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

const ITEMS: ClipboardItem[] = [
  makeItem({ id: 1, content: "first" }),
  makeItem({ id: 2, content: "second" }),
];

const SETTINGS: DesktopSettings = {
  autostartEnabled: false,
  monitorEnabled: true,
  retentionDays: 30,
  maxRecordCount: 1000,
  maxTextLength: 5000,
  ignorePasswordLikeText: true,
  customSecretPatterns: "",
  storageDir: "",
};

type InvokeOverrides = Partial<Record<string, (args: unknown) => unknown>>;

function setupInvoke(overrides: InvokeOverrides = {}) {
  invoke.mockImplementation((command: string, args: unknown) => {
    const override = overrides[command];
    if (override) {
      return Promise.resolve(override(args));
    }
    switch (command) {
      case "list_clipboard_dates":
        return Promise.resolve(DATES);
      case "list_clipboard_items":
        return Promise.resolve(ITEMS);
      case "search_clipboard_items":
        return Promise.resolve(ITEMS);
      case "get_clipboard_monitor_status":
        return Promise.resolve({ enabled: true });
      case "get_desktop_settings":
        return Promise.resolve(SETTINGS);
      case "set_clipboard_monitor_enabled":
        return Promise.resolve({ enabled: true });
      case "update_desktop_settings":
        return Promise.resolve(SETTINGS);
      case "clear_clipboard_items_by_date":
      case "delete_clipboard_item":
      case "copy_clipboard_item":
        return Promise.resolve();
      case "purge_deleted_clipboard_items":
        return Promise.resolve(0);
      default:
        return Promise.resolve();
    }
  });
}

function countCalls(command: string) {
  return invoke.mock.calls.filter(([name]) => name === command).length;
}

beforeEach(() => {
  invoke.mockReset();
});

describe("useClipboardWorkspace initial load", () => {
  it("loads dates, items, and selects the first item", async () => {
    setupInvoke();
    const { result } = renderHook(() => useClipboardWorkspace());

    await waitFor(() => {
      expect(result.current.dates).toEqual(DATES);
      expect(result.current.items).toEqual(ITEMS);
      expect(result.current.selectedItem?.id).toBe(1);
    });
  });
});

describe("useClipboardWorkspace selectDate", () => {
  it("updates selectedDate, clears search, and sets a message", async () => {
    setupInvoke();
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));

    act(() => result.current.selectDate("2026-05-20"));

    expect(result.current.selectedDate).toBe("2026-05-20");
    expect(result.current.searchTerm).toBe("");
    expect(result.current.message).toContain("2026-05-20");
  });
});

describe("useClipboardWorkspace toggleMonitor", () => {
  it("disables monitoring and reports the paused message", async () => {
    setupInvoke({ set_clipboard_monitor_enabled: () => ({ enabled: false }) });
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));

    await act(async () => {
      await result.current.toggleMonitor();
    });

    expect(result.current.monitorEnabled).toBe(false);
    expect(result.current.message).toBe("已暂停剪贴板监听。");
  });
});

describe("useClipboardWorkspace clearDate", () => {
  it("clears the date and refreshes the view", async () => {
    setupInvoke();
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));
    const datesCallsBefore = countCalls("list_clipboard_dates");

    await act(async () => {
      await result.current.clearDate();
    });

    expect(countCalls("clear_clipboard_items_by_date")).toBe(1);
    expect(result.current.message).toContain("已清空");
    expect(countCalls("list_clipboard_dates")).toBeGreaterThan(datesCallsBefore);
  });
});

describe("useClipboardWorkspace deleteItem", () => {
  it("deletes the item and reports the message", async () => {
    setupInvoke();
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));

    await act(async () => {
      await result.current.deleteItem(ITEMS[0]);
    });

    expect(countCalls("delete_clipboard_item")).toBe(1);
    expect(result.current.message).toBe("已删除该条记录。");
  });
});

describe("useClipboardWorkspace updateSettings", () => {
  it("saves settings, updates state, and clears busy on success", async () => {
    const saved: DesktopSettings = { ...SETTINGS, retentionDays: 7 };
    setupInvoke({ update_desktop_settings: () => saved });
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));

    await act(async () => {
      await result.current.updateSettings({ ...SETTINGS, retentionDays: 7 });
    });

    expect(result.current.desktopSettings).toEqual(saved);
    expect(result.current.message).toBe("桌面设置已保存，并已应用保留策略。");
    expect(result.current.isBusy).toBe(false);
  });

  it("sets an error message and clears busy on failure", async () => {
    setupInvoke({
      update_desktop_settings: () => {
        throw new Error("boom");
      },
    });
    const { result } = renderHook(() => useClipboardWorkspace());
    await waitFor(() => expect(result.current.items).toEqual(ITEMS));

    await act(async () => {
      await result.current.updateSettings({ ...SETTINGS, retentionDays: 7 });
    });

    expect(result.current.errorMessage).toContain("boom");
    expect(result.current.isBusy).toBe(false);
  });
});
