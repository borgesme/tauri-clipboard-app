use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use chrono::Local;

use super::error::ClipboardError;
use super::hash::content_hash;
use super::models::{ClipboardDateGroup, ClipboardItem, ClipboardMonitorStatus};
use super::repository;

const APP_WRITE_IGNORE_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
struct AppWriteGuard {
    hash: String,
    written_at: Instant,
}

pub struct ClipboardService {
    database_path: PathBuf,
    last_seen_hash: Mutex<Option<String>>,
    last_app_write: Mutex<Option<AppWriteGuard>>,
    monitor_enabled: Mutex<bool>,
}

impl ClipboardService {
    pub fn new(database_path: PathBuf) -> Result<Self, ClipboardError> {
        repository::init_database(&database_path)?;
        Ok(Self {
            database_path,
            last_seen_hash: Mutex::new(None),
            last_app_write: Mutex::new(None),
            monitor_enabled: Mutex::new(true),
        })
    }

    pub fn capture_current_clipboard(&self) -> Result<Option<ClipboardItem>, ClipboardError> {
        if !self.is_monitor_enabled()? {
            return Ok(None);
        }

        let content = read_clipboard_text()?;
        if content.is_empty() {
            return Ok(None);
        }

        let hash = content_hash(&content);
        if self.should_skip_hash(&hash)? {
            return Ok(None);
        }

        let item = repository::upsert_text_item(&self.database_path, &content, &hash, &now_iso())?;
        self.remember_seen_hash(hash)?;
        Ok(Some(item))
    }

    pub fn list_date_groups(&self) -> Result<Vec<ClipboardDateGroup>, ClipboardError> {
        repository::list_date_groups(&self.database_path)
    }

    pub fn list_items_by_date(&self, date: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        repository::list_items_by_date(&self.database_path, date)
    }

    pub fn search_items(&self, keyword: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        repository::search_items(&self.database_path, keyword)
    }

    pub fn get_item(&self, id: i64) -> Result<ClipboardItem, ClipboardError> {
        repository::get_item_by_id(&self.database_path, id)
    }

    pub fn copy_item(&self, id: i64) -> Result<(), ClipboardError> {
        let item = self.get_item(id)?;
        let mut clipboard = Clipboard::new()?;
        clipboard.set_text(item.content)?;

        let mut guard = self.lock_app_write()?;
        *guard = Some(AppWriteGuard {
            hash: item.content_hash,
            written_at: Instant::now(),
        });
        Ok(())
    }

    pub fn delete_item(&self, id: i64) -> Result<(), ClipboardError> {
        repository::soft_delete_item(&self.database_path, id, &now_iso())
    }

    pub fn clear_items_by_date(&self, date: &str) -> Result<usize, ClipboardError> {
        repository::soft_delete_items_by_date(&self.database_path, date, &now_iso())
    }

    pub fn set_monitor_enabled(&self, enabled: bool) -> Result<ClipboardMonitorStatus, ClipboardError> {
        if enabled {
            self.seed_current_clipboard_hash()?;
        }
        let mut guard = self.lock_monitor_enabled()?;
        *guard = enabled;
        Ok(ClipboardMonitorStatus { enabled })
    }

    pub fn monitor_status(&self) -> Result<ClipboardMonitorStatus, ClipboardError> {
        Ok(ClipboardMonitorStatus {
            enabled: self.is_monitor_enabled()?,
        })
    }

    fn seed_current_clipboard_hash(&self) -> Result<(), ClipboardError> {
        let content = read_clipboard_text()?;
        if !content.is_empty() {
            self.remember_seen_hash(content_hash(&content))?;
        }
        Ok(())
    }

    fn should_skip_hash(&self, hash: &str) -> Result<bool, ClipboardError> {
        if self.is_last_seen_hash(hash)? {
            return Ok(true);
        }

        if self.is_recent_app_write(hash)? {
            self.remember_seen_hash(hash.to_string())?;
            return Ok(true);
        }

        Ok(false)
    }

    fn is_last_seen_hash(&self, hash: &str) -> Result<bool, ClipboardError> {
        let guard = self.lock_last_seen()?;
        Ok(guard.as_deref() == Some(hash))
    }

    fn is_recent_app_write(&self, hash: &str) -> Result<bool, ClipboardError> {
        let guard = self.lock_app_write()?;
        Ok(guard
            .as_ref()
            .is_some_and(|write| write.hash == hash && write.written_at.elapsed() < APP_WRITE_IGNORE_WINDOW))
    }

    fn is_monitor_enabled(&self) -> Result<bool, ClipboardError> {
        Ok(*self.lock_monitor_enabled()?)
    }

    fn remember_seen_hash(&self, hash: String) -> Result<(), ClipboardError> {
        let mut guard = self.lock_last_seen()?;
        *guard = Some(hash);
        Ok(())
    }

    fn lock_last_seen(&self) -> Result<std::sync::MutexGuard<'_, Option<String>>, ClipboardError> {
        self.last_seen_hash
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))
    }

    fn lock_app_write(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<AppWriteGuard>>, ClipboardError> {
        self.last_app_write
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))
    }

    fn lock_monitor_enabled(&self) -> Result<std::sync::MutexGuard<'_, bool>, ClipboardError> {
        self.monitor_enabled
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))
    }
}

fn read_clipboard_text() -> Result<String, ClipboardError> {
    let mut clipboard = Clipboard::new()?;
    match clipboard.get_text() {
        Ok(text) => Ok(text),
        Err(arboard::Error::ContentNotAvailable) => Ok(String::new()),
        Err(error) => Err(ClipboardError::Clipboard(error.to_string())),
    }
}

fn now_iso() -> String {
    Local::now().to_rfc3339()
}
