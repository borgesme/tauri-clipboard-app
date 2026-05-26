use serde::Serialize;

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
