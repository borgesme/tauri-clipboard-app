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

export interface ClipboardDeletedEvent {
  id?: number | null;
  date?: string | null;
}

export interface ClipboardMonitorStatus {
  enabled: boolean;
}

export interface DesktopSettings {
  autostartEnabled: boolean;
  retentionDays: number;
  maxRecordCount: number;
  storageDir: string;
}

export type DesktopSettingsUpdate = DesktopSettings;
