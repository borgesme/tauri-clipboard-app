import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ClipboardChangeEvent,
  ClipboardDateGroup,
  ClipboardDeletedEvent,
  ClipboardItem,
  ClipboardMonitorErrorEvent,
  ClipboardMonitorStatus,
  ClipboardSkippedEvent,
  DesktopSettings,
  DesktopSettingsUpdate,
} from "@/types/clipboard";

export function listClipboardDates(): Promise<ClipboardDateGroup[]> {
  return invoke("list_clipboard_dates");
}

export function listClipboardItems(date: string): Promise<ClipboardItem[]> {
  return invoke("list_clipboard_items", { date });
}

export function searchClipboardItems(keyword: string): Promise<ClipboardItem[]> {
  return invoke("search_clipboard_items", { keyword });
}

export function getClipboardItem(id: number): Promise<ClipboardItem> {
  return invoke("get_clipboard_item", { id });
}

export function copyClipboardItem(id: number): Promise<void> {
  return invoke("copy_clipboard_item", { id });
}

export function deleteClipboardItem(id: number): Promise<void> {
  return invoke("delete_clipboard_item", { id });
}

export function clearClipboardItemsByDate(date: string): Promise<number[]> {
  return invoke("clear_clipboard_items_by_date", { date });
}

export function restoreClipboardItems(ids: number[]): Promise<number> {
  return invoke("restore_clipboard_items", { ids });
}

export function purgeDeletedClipboardItems(vacuum = false): Promise<number> {
  return invoke("purge_deleted_clipboard_items", { vacuum });
}

export function setClipboardMonitorEnabled(enabled: boolean): Promise<ClipboardMonitorStatus> {
  return invoke("set_clipboard_monitor_enabled", { enabled });
}

export function getClipboardMonitorStatus(): Promise<ClipboardMonitorStatus> {
  return invoke("get_clipboard_monitor_status");
}

export function getDesktopSettings(): Promise<DesktopSettings> {
  return invoke("get_desktop_settings");
}

export function updateDesktopSettings(settings: DesktopSettingsUpdate): Promise<DesktopSettings> {
  return invoke("update_desktop_settings", { settings });
}

export function validateStorageDir(storageDir: string): Promise<void> {
  return invoke("validate_storage_dir", { storageDir });
}

export function hideMainWindow(): Promise<void> {
  return invoke("hide_main_window");
}

export function showMainWindow(): Promise<void> {
  return invoke("show_main_window");
}

export function onOpenSettings(handler: () => void): Promise<UnlistenFn> {
  return listen("app:open-settings", () => handler());
}

export function onClipboardItemCreated(
  handler: (event: ClipboardChangeEvent) => void,
): Promise<UnlistenFn> {
  return listen<ClipboardChangeEvent>("clipboard:item-created", (event) => handler(event.payload));
}

export function onClipboardItemUpdated(
  handler: (event: ClipboardChangeEvent) => void,
): Promise<UnlistenFn> {
  return listen<ClipboardChangeEvent>("clipboard:item-updated", (event) => handler(event.payload));
}

export function onClipboardItemDeleted(
  handler: (event: ClipboardDeletedEvent) => void,
): Promise<UnlistenFn> {
  return listen<ClipboardDeletedEvent>("clipboard:item-deleted", (event) => handler(event.payload));
}

export function onClipboardItemSkipped(
  handler: (event: ClipboardSkippedEvent) => void,
): Promise<UnlistenFn> {
  return listen<ClipboardSkippedEvent>("clipboard:item-skipped", (event) => handler(event.payload));
}

export function onClipboardMonitorStatusChanged(
  handler: (status: ClipboardMonitorStatus) => void,
): Promise<UnlistenFn> {
  return listen<ClipboardMonitorStatus>("clipboard:monitor-status-changed", (event) => handler(event.payload));
}

export function onClipboardMonitorError(
  handler: (event: ClipboardMonitorErrorEvent) => void,
): Promise<UnlistenFn> {
  return listen<ClipboardMonitorErrorEvent>("clipboard:monitor-error", (event) => handler(event.payload));
}
