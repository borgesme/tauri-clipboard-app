import { useCallback, useEffect, useMemo, useState } from "react";

import {
  clearClipboardItemsByDate,
  copyClipboardItem,
  deleteClipboardItem,
  getClipboardMonitorStatus,
  getDesktopSettings,
  hideMainWindow,
  listClipboardDates,
  listClipboardItems,
  searchClipboardItems,
  setClipboardMonitorEnabled,
  updateDesktopSettings,
} from "@/api/clipboard";
import { useClipboardEvents } from "@/hooks/useClipboardEvents";
import { todayKey } from "@/lib/date";
import type { ClipboardDateGroup, ClipboardItem, DesktopSettings } from "@/types/clipboard";

export function useClipboardWorkspace() {
  const [dates, setDates] = useState<ClipboardDateGroup[]>([]);
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [selectedDate, setSelectedDate] = useState(todayKey());
  const [selectedItemId, setSelectedItemId] = useState<number | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [monitorEnabled, setMonitorEnabled] = useState(true);
  const [desktopSettings, setDesktopSettings] = useState<DesktopSettings | null>(null);
  const [message, setMessage] = useState("复制一段文本后，它会自动出现在这里。");
  const [errorMessage, setErrorMessage] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  const selectedItem = useSelectedItem(items, selectedItemId);
  const loadDates = useCallback(async () => setDates(await listClipboardDates()), []);
  const setLoadedItems = useLoadedItemsSetter(setItems, setSelectedItemId);
  const loadItems = useLoadItems(searchTerm, selectedDate, setLoadedItems);
  const refreshView = useRefreshView({ loadDates, loadItems, setIsBusy, setErrorMessage });

  useInitialStatus({ setMonitorEnabled, setDesktopSettings, setErrorMessage });
  useEffect(() => void refreshView(), [refreshView]);
  useClipboardEvents({ refreshView, setMessage, setErrorMessage, setMonitorEnabled });

  return {
    dates,
    items,
    selectedDate,
    selectedItem,
    searchTerm,
    monitorEnabled,
    desktopSettings,
    message,
    errorMessage,
    isBusy,
    setSearchTerm,
    setSelectedItemId,
    selectDate: createDateSelector(setSelectedDate, setSearchTerm, setMessage),
    toggleMonitor: createMonitorToggle({ monitorEnabled, setMonitorEnabled, setMessage }),
    clearDate: createClearDate({ selectedDate, refreshView, setMessage }),
    copyItem: createCopyItem(setMessage),
    deleteItem: createDeleteItem(refreshView, setMessage),
    updateSettings: createSettingsUpdater({ setDesktopSettings, setIsBusy, setErrorMessage, setMessage, refreshView }),
    hideWindow: createHideWindow(setErrorMessage),
  };
}

function useSelectedItem(items: ClipboardItem[], selectedId: number | null) {
  return useMemo(
    () => items.find((item) => item.id === selectedId) ?? items[0] ?? null,
    [items, selectedId],
  );
}

function useLoadedItemsSetter(
  setItems: (items: ClipboardItem[]) => void,
  setSelectedItemId: React.Dispatch<React.SetStateAction<number | null>>,
) {
  return useCallback((nextItems: ClipboardItem[]) => {
    setItems(nextItems);
    setSelectedItemId((currentId) => selectNextItemId(currentId, nextItems));
  }, [setItems, setSelectedItemId]);
}

function useLoadItems(
  searchTerm: string,
  selectedDate: string,
  setLoadedItems: (items: ClipboardItem[]) => void,
) {
  return useCallback(async () => {
    const keyword = searchTerm.trim();
    const nextItems = keyword
      ? await searchClipboardItems(keyword)
      : await listClipboardItems(selectedDate);
    setLoadedItems(nextItems);
  }, [searchTerm, selectedDate, setLoadedItems]);
}

interface RefreshViewOptions {
  loadDates: () => Promise<void>;
  loadItems: () => Promise<void>;
  setIsBusy: (value: boolean) => void;
  setErrorMessage: (value: string) => void;
}

function useRefreshView({ loadDates, loadItems, setIsBusy, setErrorMessage }: RefreshViewOptions) {
  return useCallback(async () => {
    setIsBusy(true);
    setErrorMessage("");
    try {
      await Promise.all([loadDates(), loadItems()]);
    } catch (error) {
      setErrorMessage(String(error));
    } finally {
      setIsBusy(false);
    }
  }, [loadDates, loadItems, setIsBusy, setErrorMessage]);
}

interface InitialStatusOptions {
  setMonitorEnabled: (value: boolean) => void;
  setDesktopSettings: (value: DesktopSettings) => void;
  setErrorMessage: (value: string) => void;
}

function useInitialStatus({
  setMonitorEnabled,
  setDesktopSettings,
  setErrorMessage,
}: InitialStatusOptions) {
  useEffect(() => {
    void Promise.all([getClipboardMonitorStatus(), getDesktopSettings()])
      .then(([monitorStatus, desktopSettings]) => {
        setMonitorEnabled(monitorStatus.enabled);
        setDesktopSettings(desktopSettings);
      })
      .catch((error: unknown) => setErrorMessage(String(error)));
  }, [setMonitorEnabled, setDesktopSettings, setErrorMessage]);
}

function selectNextItemId(currentId: number | null, items: ClipboardItem[]) {
  if (currentId && items.some((item) => item.id === currentId)) {
    return currentId;
  }
  return items[0]?.id ?? null;
}

function createDateSelector(
  setSelectedDate: (value: string) => void,
  setSearchTerm: (value: string) => void,
  setMessage: (value: string) => void,
) {
  return (date: string) => {
    setSelectedDate(date);
    setSearchTerm("");
    setMessage(`正在查看 ${date} 的剪贴板记录。`);
  };
}

interface MonitorToggleOptions {
  monitorEnabled: boolean;
  setMonitorEnabled: (value: boolean) => void;
  setMessage: (value: string) => void;
}

function createMonitorToggle({ monitorEnabled, setMonitorEnabled, setMessage }: MonitorToggleOptions) {
  return async () => {
    const status = await setClipboardMonitorEnabled(!monitorEnabled);
    setMonitorEnabled(status.enabled);
    setMessage(status.enabled ? "已恢复剪贴板监听。" : "已暂停剪贴板监听。");
  };
}

interface ClearDateOptions {
  selectedDate: string;
  refreshView: () => Promise<void>;
  setMessage: (value: string) => void;
}

function createClearDate({ selectedDate, refreshView, setMessage }: ClearDateOptions) {
  return async () => {
    await clearClipboardItemsByDate(selectedDate);
    setMessage(`已清空 ${selectedDate} 的剪贴板记录。`);
    await refreshView();
  };
}

function createCopyItem(setMessage: (value: string) => void) {
  return async (item: ClipboardItem) => {
    await copyClipboardItem(item.id);
    setMessage("已复制回系统剪贴板。");
  };
}

function createDeleteItem(refreshView: () => Promise<void>, setMessage: (value: string) => void) {
  return async (item: ClipboardItem) => {
    await deleteClipboardItem(item.id);
    setMessage("已删除该条记录。");
    await refreshView();
  };
}

interface SettingsUpdaterOptions {
  setDesktopSettings: (value: DesktopSettings) => void;
  setIsBusy: (value: boolean) => void;
  setErrorMessage: (value: string) => void;
  setMessage: (value: string) => void;
  refreshView: () => Promise<void>;
}

function createSettingsUpdater(options: SettingsUpdaterOptions) {
  return async (settings: DesktopSettings) => {
    options.setIsBusy(true);
    options.setErrorMessage("");
    try {
      options.setDesktopSettings(await updateDesktopSettings(settings));
      options.setMessage("桌面设置已保存，并已应用保留策略。");
      await options.refreshView();
    } catch (error) {
      options.setErrorMessage(String(error));
    } finally {
      options.setIsBusy(false);
    }
  };
}

function createHideWindow(setErrorMessage: (value: string) => void) {
  return async () => {
    try {
      await hideMainWindow();
    } catch (error) {
      setErrorMessage(String(error));
    }
  };
}
