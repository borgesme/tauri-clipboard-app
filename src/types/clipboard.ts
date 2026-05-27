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

export type ClipboardSkipReason =
  | "empty"
  | "monitorDisabled"
  | "tooLong"
  | "secretLike"
  | "duplicate"
  | "appWriteBack";

export interface ClipboardSkippedEvent {
  reason: ClipboardSkipReason;
  contentLength: number;
  maxTextLength: number;
}

export interface ClipboardMonitorStatus {
  enabled: boolean;
}

export interface DesktopSettings {
  autostartEnabled: boolean;
  monitorEnabled: boolean;
  retentionDays: number;
  maxRecordCount: number;
  maxTextLength: number;
  ignorePasswordLikeText: boolean;
  customSecretPatterns: string;
  storageDir: string;
}

export type DesktopSettingsUpdate = DesktopSettings;
