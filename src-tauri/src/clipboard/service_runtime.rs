use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use arboard::Clipboard;
use chrono::{Local, Utc};
use rusqlite::Connection;

use super::error::ClipboardError;
use super::models::{CaptureOutcome, ClipboardSkipReason};

const APP_WRITE_IGNORE_WINDOW: Duration = Duration::from_secs(2);
const DATABASE_FILE_NAME: &str = "clipboard.sqlite";

#[derive(Debug, Clone)]
pub struct AppWriteGuard {
    hash: String,
    written_at: Instant,
}

impl AppWriteGuard {
    pub fn new(hash: String) -> Self {
        Self {
            hash,
            written_at: Instant::now(),
        }
    }

    pub fn is_recent(&self, hash: &str) -> bool {
        self.hash == hash && self.written_at.elapsed() < APP_WRITE_IGNORE_WINDOW
    }
}

pub fn read_clipboard_text() -> Result<String, ClipboardError> {
    let mut clipboard = Clipboard::new()?;
    match clipboard.get_text() {
        Ok(text) => Ok(text),
        Err(arboard::Error::ContentNotAvailable) => Ok(String::new()),
        Err(error) => Err(ClipboardError::Clipboard(error.to_string())),
    }
}

pub fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn today_local() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn skip_outcome(
    reason: ClipboardSkipReason,
    content_length: i64,
    max_text_length: i64,
) -> CaptureOutcome {
    CaptureOutcome::Skipped {
        reason,
        content_length,
        max_text_length,
    }
}

pub fn resolve_database_path(default_database_path: &Path, storage_dir: &str) -> PathBuf {
    let storage_dir = storage_dir.trim();
    if storage_dir.is_empty() {
        return default_database_path.to_path_buf();
    }
    PathBuf::from(storage_dir).join(DATABASE_FILE_NAME)
}

pub fn open_connection(path: &Path) -> Result<Connection, ClipboardError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "PRAGMA journal_mode=WAL; \
         PRAGMA synchronous=NORMAL; \
         PRAGMA foreign_keys=ON;",
    )?;
    Ok(connection)
}
