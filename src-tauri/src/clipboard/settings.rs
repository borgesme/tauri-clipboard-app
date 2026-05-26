use std::path::Path;

use chrono::{Duration, Local};

use super::error::ClipboardError;
use super::models::StoredSettings;
use super::repository;

pub const DEFAULT_RETENTION_DAYS: i64 = 30;
pub const DEFAULT_MAX_RECORD_COUNT: i64 = 1000;
const MIN_SETTING_VALUE: i64 = 1;

pub fn get_stored_settings(path: &Path) -> Result<StoredSettings, ClipboardError> {
    Ok(StoredSettings {
        retention_days: repository::get_i64_setting(path, "retention_days", DEFAULT_RETENTION_DAYS)?,
        max_record_count: repository::get_i64_setting(path, "max_record_count", DEFAULT_MAX_RECORD_COUNT)?,
    })
}

pub fn update_stored_settings(
    path: &Path,
    retention_days: i64,
    max_record_count: i64,
) -> Result<StoredSettings, ClipboardError> {
    let settings = StoredSettings {
        retention_days: sanitize_setting_value(retention_days),
        max_record_count: sanitize_setting_value(max_record_count),
    };
    let now = Local::now().to_rfc3339();
    repository::set_setting(path, "retention_days", &settings.retention_days.to_string(), &now)?;
    repository::set_setting(path, "max_record_count", &settings.max_record_count.to_string(), &now)?;
    Ok(settings)
}

pub fn apply_retention_policy(path: &Path) -> Result<usize, ClipboardError> {
    let settings = get_stored_settings(path)?;
    let now = Local::now().to_rfc3339();
    let cutoff = Local::now() - Duration::days(settings.retention_days);
    let cutoff_date = cutoff.format("%Y-%m-%d").to_string();
    repository::cleanup_items(path, &cutoff_date, settings.max_record_count, &now)
}

fn sanitize_setting_value(value: i64) -> i64 {
    value.max(MIN_SETTING_VALUE)
}
