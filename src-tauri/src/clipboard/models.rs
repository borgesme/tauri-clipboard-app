use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub content: String,
    pub preview: String,
    pub content_hash: String,
    pub created_at: String,
    pub last_copied_at: String,
    pub copy_count: i64,
}

#[derive(Debug, Clone)]
pub enum CaptureOutcome {
    Item(ClipboardItem),
    Skipped {
        reason: ClipboardSkipReason,
        content_length: i64,
        max_text_length: i64,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardDateGroup {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardChangeEvent {
    pub item: ClipboardItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardSkipReason {
    Empty,
    MonitorDisabled,
    TooLong,
    SecretLike,
    Duplicate,
    AppWriteBack,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSkippedEvent {
    pub reason: ClipboardSkipReason,
    pub content_length: i64,
    pub max_text_length: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardDeletedEvent {
    pub id: Option<i64>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardMonitorStatus {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettings {
    pub autostart_enabled: bool,
    pub monitor_enabled: bool,
    pub retention_days: i64,
    pub max_record_count: i64,
    pub max_text_length: i64,
    pub ignore_password_like_text: bool,
    pub custom_secret_patterns: String,
    pub storage_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettingsUpdate {
    pub autostart_enabled: bool,
    pub monitor_enabled: bool,
    pub retention_days: i64,
    pub max_record_count: i64,
    pub max_text_length: i64,
    pub ignore_password_like_text: bool,
    pub custom_secret_patterns: String,
    pub storage_dir: String,
}

#[derive(Debug, Clone)]
pub struct StoredSettings {
    pub monitor_enabled: bool,
    pub retention_days: i64,
    pub max_record_count: i64,
    pub max_text_length: i64,
    pub ignore_password_like_text: bool,
    pub custom_secret_patterns: String,
    pub storage_dir: String,
}
