use std::path::{Path, PathBuf};
use std::sync::Mutex;

use arboard::Clipboard;

use super::error::ClipboardError;
use super::hash::content_hash;
use super::maintenance;
use super::models::{
    CaptureOutcome, ClipboardDateGroup, ClipboardItem, ClipboardMonitorStatus, ClipboardSkipReason,
    DesktopSettings, DesktopSettingsUpdate,
};
use super::repository;
use super::service_runtime::{
    now_iso, read_clipboard_text, resolve_database_path, skip_outcome, AppWriteGuard,
};
use super::settings;

pub struct ClipboardService {
    default_database_path: PathBuf,
    database_path: Mutex<PathBuf>,
    last_seen_hash: Mutex<Option<String>>,
    last_app_write: Mutex<Option<AppWriteGuard>>,
    monitor_enabled: Mutex<bool>,
}

impl ClipboardService {
    pub fn new(default_database_path: PathBuf) -> Result<Self, ClipboardError> {
        repository::init_database(&default_database_path)?;
        let stored = settings::get_stored_settings(&default_database_path)?;
        let database_path = resolve_database_path(&default_database_path, &stored.storage_dir);
        repository::init_database(&database_path)?;
        Ok(Self {
            default_database_path,
            database_path: Mutex::new(database_path),
            last_seen_hash: Mutex::new(None),
            last_app_write: Mutex::new(None),
            monitor_enabled: Mutex::new(stored.monitor_enabled),
        })
    }

    pub fn capture_current_clipboard(&self) -> Result<CaptureOutcome, ClipboardError> {
        if !self.is_monitor_enabled()? {
            return Ok(skip_outcome(ClipboardSkipReason::MonitorDisabled, 0, 0));
        }
        let database_path = self.active_database_path()?;
        let content = read_clipboard_text()?;
        if content.is_empty() {
            return Ok(skip_outcome(ClipboardSkipReason::Empty, 0, 0));
        }
        let stored_settings = settings::get_stored_settings(&self.default_database_path)?;
        if let Some(reason) = settings::content_skip_reason(&content, &stored_settings) {
            return Ok(skip_outcome(
                reason,
                content.chars().count() as i64,
                stored_settings.max_text_length,
            ));
        }
        let hash = content_hash(&content);
        if let Some(reason) = self.skip_hash_reason(&hash)? {
            return Ok(skip_outcome(reason, content.len() as i64, 0));
        }
        let item = repository::upsert_text_item(&database_path, &content, &hash, &now_iso())?;
        self.remember_seen_hash(hash)?;
        self.apply_retention_policy(&database_path)?;
        Ok(CaptureOutcome::Item(item))
    }

    pub fn list_date_groups(&self) -> Result<Vec<ClipboardDateGroup>, ClipboardError> {
        repository::list_date_groups(&self.active_database_path()?)
    }

    pub fn list_items_by_date(&self, date: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        repository::list_items_by_date(&self.active_database_path()?, date)
    }

    pub fn search_items(&self, keyword: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        repository::search_items(&self.active_database_path()?, keyword)
    }

    pub fn get_item(&self, id: i64) -> Result<ClipboardItem, ClipboardError> {
        repository::get_item_by_id(&self.active_database_path()?, id)
    }

    pub fn copy_item(&self, id: i64) -> Result<(), ClipboardError> {
        let item = self.get_item(id)?;
        let mut clipboard = Clipboard::new()?;
        clipboard.set_text(item.content)?;
        let mut guard = self.lock_app_write()?;
        *guard = Some(AppWriteGuard::new(item.content_hash));
        Ok(())
    }

    pub fn delete_item(&self, id: i64) -> Result<(), ClipboardError> {
        repository::soft_delete_item(&self.active_database_path()?, id, &now_iso())
    }

    pub fn clear_items_by_date(&self, date: &str) -> Result<usize, ClipboardError> {
        repository::soft_delete_items_by_date(&self.active_database_path()?, date, &now_iso())
    }

    pub fn purge_deleted_items(&self, vacuum: bool) -> Result<usize, ClipboardError> {
        let database_path = self.active_database_path()?;
        let removed = maintenance::purge_deleted_items(&database_path)?;
        if vacuum {
            maintenance::vacuum_database(&database_path)?;
        }
        Ok(removed)
    }

    pub fn set_monitor_enabled(
        &self,
        enabled: bool,
    ) -> Result<ClipboardMonitorStatus, ClipboardError> {
        if enabled {
            self.seed_current_clipboard_hash()?;
        }
        settings::update_monitor_enabled(&self.default_database_path, enabled)?;
        let mut guard = self.lock_monitor_enabled()?;
        *guard = enabled;
        Ok(ClipboardMonitorStatus { enabled })
    }

    pub fn monitor_status(&self) -> Result<ClipboardMonitorStatus, ClipboardError> {
        Ok(ClipboardMonitorStatus {
            enabled: self.is_monitor_enabled()?,
        })
    }

    pub fn desktop_settings(
        &self,
        autostart_enabled: bool,
    ) -> Result<DesktopSettings, ClipboardError> {
        let stored = settings::get_stored_settings(&self.default_database_path)?;
        Ok(DesktopSettings {
            autostart_enabled,
            monitor_enabled: self.is_monitor_enabled()?,
            retention_days: stored.retention_days,
            max_record_count: stored.max_record_count,
            max_text_length: stored.max_text_length,
            ignore_password_like_text: stored.ignore_password_like_text,
            custom_secret_patterns: stored.custom_secret_patterns,
            storage_dir: stored.storage_dir,
        })
    }

    pub fn update_desktop_settings(
        &self,
        update: DesktopSettingsUpdate,
        autostart_enabled: bool,
    ) -> Result<DesktopSettings, ClipboardError> {
        let storage_dir = update.storage_dir.trim().to_string();
        settings::validate_storage_dir(&storage_dir)?;
        let database_path = resolve_database_path(&self.default_database_path, &storage_dir);
        repository::init_database(&database_path)?;
        let stored = settings::update_stored_settings(
            &self.default_database_path,
            update.monitor_enabled,
            update.retention_days,
            update.max_record_count,
            update.max_text_length,
            update.ignore_password_like_text,
            &update.custom_secret_patterns,
            &storage_dir,
        )?;
        self.set_active_database_path(database_path.clone())?;
        self.set_monitor_enabled_state(stored.monitor_enabled)?;
        self.apply_retention_policy(&database_path)?;
        Ok(DesktopSettings {
            autostart_enabled,
            monitor_enabled: stored.monitor_enabled,
            retention_days: stored.retention_days,
            max_record_count: stored.max_record_count,
            max_text_length: stored.max_text_length,
            ignore_password_like_text: stored.ignore_password_like_text,
            custom_secret_patterns: stored.custom_secret_patterns,
            storage_dir: stored.storage_dir,
        })
    }

    fn apply_retention_policy(&self, database_path: &Path) -> Result<(), ClipboardError> {
        settings::apply_retention_policy(database_path, &self.default_database_path)?;
        Ok(())
    }

    fn seed_current_clipboard_hash(&self) -> Result<(), ClipboardError> {
        let content = read_clipboard_text()?;
        if !content.is_empty() {
            self.remember_seen_hash(content_hash(&content))?;
        }
        Ok(())
    }

    fn skip_hash_reason(&self, hash: &str) -> Result<Option<ClipboardSkipReason>, ClipboardError> {
        if self.is_last_seen_hash(hash)? {
            return Ok(Some(ClipboardSkipReason::Duplicate));
        }
        if self.is_recent_app_write(hash)? {
            self.remember_seen_hash(hash.to_string())?;
            return Ok(Some(ClipboardSkipReason::AppWriteBack));
        }
        Ok(None)
    }

    fn is_last_seen_hash(&self, hash: &str) -> Result<bool, ClipboardError> {
        let guard = self.lock_last_seen()?;
        Ok(guard.as_deref() == Some(hash))
    }

    fn is_recent_app_write(&self, hash: &str) -> Result<bool, ClipboardError> {
        let guard = self.lock_app_write()?;
        Ok(guard.as_ref().is_some_and(|write| write.is_recent(hash)))
    }

    fn is_monitor_enabled(&self) -> Result<bool, ClipboardError> {
        Ok(*self.lock_monitor_enabled()?)
    }

    fn remember_seen_hash(&self, hash: String) -> Result<(), ClipboardError> {
        let mut guard = self.lock_last_seen()?;
        *guard = Some(hash);
        Ok(())
    }

    fn set_monitor_enabled_state(&self, enabled: bool) -> Result<(), ClipboardError> {
        let mut guard = self.lock_monitor_enabled()?;
        *guard = enabled;
        Ok(())
    }

    fn active_database_path(&self) -> Result<PathBuf, ClipboardError> {
        Ok(self.lock_database_path()?.clone())
    }

    fn set_active_database_path(&self, database_path: PathBuf) -> Result<(), ClipboardError> {
        let mut guard = self.lock_database_path()?;
        *guard = database_path;
        Ok(())
    }

    fn lock_database_path(&self) -> Result<std::sync::MutexGuard<'_, PathBuf>, ClipboardError> {
        self.database_path
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))
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
