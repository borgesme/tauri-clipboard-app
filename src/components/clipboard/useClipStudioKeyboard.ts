import { useEffect } from "react";

import type { ClipboardItem } from "@/types/clipboard";

interface KeyboardShortcutOptions {
  visibleItems: ClipboardItem[];
  selectedItem: ClipboardItem | null;
  detailItem: ClipboardItem | null;
  searchInputRef: React.MutableRefObject<HTMLInputElement | null>;
  onCopy: (item: ClipboardItem) => void;
  onOpenDetail: (item: ClipboardItem) => void;
  onReset: () => void;
  onSelectItem: (id: number) => void;
  onSendToToolbox: (item: ClipboardItem) => void;
}

export function useKeyboardShortcuts(options: KeyboardShortcutOptions) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => handleShortcut(event, options);
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [options]);
}

function handleShortcut(event: KeyboardEvent, options: KeyboardShortcutOptions) {
  const isTyping = isTypingTarget(event.target);
  if (handleGlobalShortcut(event, isTyping, options)) {
    return;
  }
  if (isTyping) {
    return;
  }
  handleListShortcut(event, options);
}

function handleGlobalShortcut(event: KeyboardEvent, isTyping: boolean, options: KeyboardShortcutOptions) {
  if (event.key === "Escape") {
    event.preventDefault();
    options.onReset();
    return true;
  }
  if (event.key === "/" && !isTyping) {
    event.preventDefault();
    options.searchInputRef.current?.focus();
    return true;
  }
  return false;
}

function handleListShortcut(event: KeyboardEvent, options: KeyboardShortcutOptions) {
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    moveSelection(event.key === "ArrowDown" ? 1 : -1, options);
    return;
  }
  if (event.key === "Enter" && options.selectedItem) {
    options.onCopy(options.selectedItem);
    return;
  }
  if (event.key === " " && options.selectedItem) {
    event.preventDefault();
    options.onOpenDetail(options.selectedItem);
    return;
  }
  if (event.key.toLowerCase() === "t" && options.selectedItem && !options.detailItem) {
    options.onSendToToolbox(options.selectedItem);
  }
}

function moveSelection(offset: number, options: KeyboardShortcutOptions) {
  const items = options.visibleItems;
  if (items.length === 0) {
    return;
  }
  const currentIndex = Math.max(0, items.findIndex((item) => item.id === options.selectedItem?.id));
  const nextIndex = Math.min(items.length - 1, Math.max(0, currentIndex + offset));
  options.onSelectItem(items[nextIndex].id);
}

function isTypingTarget(target: EventTarget | null) {
  const element = target as HTMLElement | null;
  return element?.tagName === "INPUT" || element?.tagName === "TEXTAREA";
}
