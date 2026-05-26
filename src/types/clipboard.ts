export interface ClipboardItem {
  id: number;
  contentType: "text";
  content: string;
  preview: string;
  contentHash: string;
  createdAt: string;
  lastCopiedAt: string;
  copyCount: number;
}

export interface ClipboardDateGroup {
  date: string;
  count: number;
}

export interface ClipboardChangeEvent {
  item: ClipboardItem;
}
