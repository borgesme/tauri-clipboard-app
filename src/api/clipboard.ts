import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ClipboardChangeEvent,
  ClipboardDateGroup,
  ClipboardDeletedEvent,
  ClipboardItem,
  ClipboardMonitorStatus,
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

export function clearClipboardItemsByDate(date: string): Promise<void> {
  return invoke("clear_clipboard_items_by_date", { date });
}

export function setClipboardMonitorEnabled(enabled: boolean): Promise<ClipboardMonitorStatus> {
  return invoke("set_clipboard_monitor_enabled", { enabled });
}

export function getClipboardMonitorStatus(): Promise<ClipboardMonitorStatus> {
  return invoke("get_clipboard_monitor_status");
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

export function onClipboardMonitorStatusChanged(
  handler: (status: ClipboardMonitorStatus) => void,
): Promise<UnlistenFn> {
  return listen<ClipboardMonitorStatus>("clipboard:monitor-status-changed", (event) => handler(event.payload));
}
