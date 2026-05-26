import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ClipboardChangeEvent,
  ClipboardDateGroup,
  ClipboardItem,
} from "@/types/clipboard";

export function listClipboardDates(): Promise<ClipboardDateGroup[]> {
  return invoke("list_clipboard_dates");
}

export function listClipboardItems(date: string): Promise<ClipboardItem[]> {
  return invoke("list_clipboard_items", { date });
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

export function onClipboardItemCreated(
  handler: (event: ClipboardChangeEvent) => void,
): Promise<UnlistenFn> {
  return listen<ClipboardChangeEvent>("clipboard:item-created", (event) => {
    handler(event.payload);
  });
}
