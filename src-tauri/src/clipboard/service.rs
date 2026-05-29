use std::path::PathBuf;
use std::sync::Mutex;

use arboard::Clipboard;
use rusqlite::Connection;

use super::error::ClipboardError;
use super::hash::content_hash;
use super::maintenance;
use super::models::{
    CaptureOutcome, ClipboardDateGroup, ClipboardItem, ClipboardMonitorStatus, ClipboardSkipReason,
    DesktopSettings, DesktopSettingsUpdate,
};
use super::repository;
use super::service_runtime::{
    self, now_iso, read_clipboard_text, resolve_database_path, skip_outcome, today_local,
    AppWriteGuard,
};
use super::settings;

pub const RETENTION_TRIGGER_THRESHOLD: u32 = 50;

pub struct ClipboardService {
    default_database_path: PathBuf,
    database_path: Mutex<PathBuf>,
    settings_conn: Mutex<Connection>,
    items_conn: Mutex<Connection>,
    last_seen_hash: Mutex<Option<String>>,
    last_app_write: Mutex<Option<AppWriteGuard>>,
    monitor_enabled: Mutex<bool>,
    captures_since_cleanup: Mutex<u32>,
}

impl ClipboardService {
    pub fn new(default_database_path: PathBuf) -> Result<Self, ClipboardError> {
        let settings_conn = service_runtime::open_connection(&default_database_path)?;
        repository::init_schema(&settings_conn)?;
        let stored = settings::get_stored_settings(&settings_conn)?;
        let database_path = resolve_database_path(&default_database_path, &stored.storage_dir);
        let items_conn = service_runtime::open_connection(&database_path)?;
        repository::init_schema(&items_conn)?;
        Ok(Self {
            default_database_path,
            database_path: Mutex::new(database_path),
            settings_conn: Mutex::new(settings_conn),
            items_conn: Mutex::new(items_conn),
            last_seen_hash: Mutex::new(None),
            last_app_write: Mutex::new(None),
            monitor_enabled: Mutex::new(stored.monitor_enabled),
            captures_since_cleanup: Mutex::new(0),
        })
    }

    pub fn capture_current_clipboard(&self) -> Result<CaptureOutcome, ClipboardError> {
        if !self.is_monitor_enabled()? {
            return Ok(skip_outcome(ClipboardSkipReason::MonitorDisabled, 0, 0));
        }
        let content = read_clipboard_text()?;
        if content.is_empty() {
            return Ok(skip_outcome(ClipboardSkipReason::Empty, 0, 0));
        }
        let stored_settings = {
            let conn = self.lock_settings_conn()?;
            settings::get_stored_settings(&conn)?
        };
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
        let item = {
            let conn = self.lock_items_conn()?;
            repository::upsert_text_item(&conn, &content, &hash, &now_iso(), &today_local())?
        };
        self.remember_seen_hash(hash)?;
        let should_clean = {
            let mut count = self
                .captures_since_cleanup
                .lock()
                .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
            *count += 1;
            if *count >= RETENTION_TRIGGER_THRESHOLD {
                *count = 0;
                true
            } else {
                false
            }
        };
        if should_clean {
            self.run_retention()?;
        }
        Ok(CaptureOutcome::Item(item))
    }

    pub fn list_date_groups(&self) -> Result<Vec<ClipboardDateGroup>, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::list_date_groups(&conn)
    }

    pub fn list_items_by_date(&self, date: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::list_items_by_date(&conn, date)
    }

    pub fn search_items(&self, keyword: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::search_items(&conn, keyword)
    }

    pub fn get_item(&self, id: i64) -> Result<ClipboardItem, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::get_item_by_id(&conn, id)
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
        let conn = self.lock_items_conn()?;
        repository::soft_delete_item(&conn, id, &now_iso())
    }

    pub fn clear_items_by_date(&self, date: &str) -> Result<usize, ClipboardError> {
        let conn = self.lock_items_conn()?;
        repository::soft_delete_items_by_date(&conn, date, &now_iso())
    }

    pub fn purge_deleted_items(&self, vacuum: bool) -> Result<usize, ClipboardError> {
        let conn = self.lock_items_conn()?;
        let removed = maintenance::purge_deleted_items(&conn)?;
        if vacuum {
            maintenance::vacuum_database(&conn)?;
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
        {
            let conn = self.lock_settings_conn()?;
            settings::update_monitor_enabled(&conn, enabled)?;
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

    pub fn desktop_settings(
        &self,
        autostart_enabled: bool,
    ) -> Result<DesktopSettings, ClipboardError> {
        let stored = {
            let conn = self.lock_settings_conn()?;
            settings::get_stored_settings(&conn)?
        };
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
        let new_database_path = resolve_database_path(&self.default_database_path, &storage_dir);

        let stored = {
            let conn = self.lock_settings_conn()?;
            settings::update_stored_settings(
                &conn,
                update.monitor_enabled,
                update.retention_days,
                update.max_record_count,
                update.max_text_length,
                update.ignore_password_like_text,
                &update.custom_secret_patterns,
                &storage_dir,
            )?
        };

        let needs_swap = {
            let path_guard = self.lock_database_path()?;
            new_database_path != *path_guard
        };
        if needs_swap {
            let new_conn = service_runtime::open_connection(&new_database_path)?;
            repository::init_schema(&new_conn)?;
            let mut path_guard = self.lock_database_path()?;
            let mut items_guard = self.lock_items_conn()?;
            *items_guard = new_conn;
            *path_guard = new_database_path;
        }

        self.set_monitor_enabled_state(stored.monitor_enabled)?;
        self.run_retention()?;
        {
            let mut count = self
                .captures_since_cleanup
                .lock()
                .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
            *count = 0;
        }

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

    fn run_retention(&self) -> Result<(), ClipboardError> {
        let settings_conn = self.lock_settings_conn()?;
        let items_conn = self.lock_items_conn()?;
        settings::apply_retention_policy(&items_conn, &settings_conn)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn tick_capture_count_for_test(&self) -> Result<(), ClipboardError> {
        let should_clean = {
            let mut count = self
                .captures_since_cleanup
                .lock()
                .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
            *count += 1;
            if *count >= RETENTION_TRIGGER_THRESHOLD {
                *count = 0;
                true
            } else {
                false
            }
        };
        if should_clean {
            self.run_retention()?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn captures_count_for_test(&self) -> Result<u32, ClipboardError> {
        let count = self
            .captures_since_cleanup
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
        Ok(*count)
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

    fn lock_database_path(&self) -> Result<std::sync::MutexGuard<'_, PathBuf>, ClipboardError> {
        self.database_path
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))
    }

    fn lock_settings_conn(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Connection>, ClipboardError> {
        self.settings_conn
            .lock()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))
    }

    fn lock_items_conn(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Connection>, ClipboardError> {
        self.items_conn
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
