use std::path::Path;

use chrono::{Duration, Local};

use super::error::ClipboardError;
use super::models::StoredSettings;
use super::repository;

pub const DEFAULT_RETENTION_DAYS: i64 = 30;
pub const DEFAULT_MAX_RECORD_COUNT: i64 = 1000;
pub const DEFAULT_MAX_TEXT_LENGTH: i64 = 20_000;
pub const DEFAULT_STORAGE_DIR: &str = "";
const MIN_SETTING_VALUE: i64 = 1;
const DEFAULT_IGNORE_PASSWORD_LIKE_TEXT: bool = false;

pub fn get_stored_settings(path: &Path) -> Result<StoredSettings, ClipboardError> {
    Ok(StoredSettings {
        retention_days: repository::get_i64_setting(path, "retention_days", DEFAULT_RETENTION_DAYS)?,
        max_record_count: repository::get_i64_setting(path, "max_record_count", DEFAULT_MAX_RECORD_COUNT)?,
        max_text_length: repository::get_i64_setting(path, "max_text_length", DEFAULT_MAX_TEXT_LENGTH)?,
        ignore_password_like_text: repository::get_i64_setting(
            path,
            "ignore_password_like_text",
            bool_to_setting(DEFAULT_IGNORE_PASSWORD_LIKE_TEXT),
        )? == 1,
        storage_dir: repository::get_string_setting(path, "storage_dir", DEFAULT_STORAGE_DIR)?,
    })
}

pub fn update_stored_settings(
    path: &Path,
    retention_days: i64,
    max_record_count: i64,
    max_text_length: i64,
    ignore_password_like_text: bool,
    storage_dir: &str,
) -> Result<StoredSettings, ClipboardError> {
    let settings = StoredSettings {
        retention_days: sanitize_setting_value(retention_days),
        max_record_count: sanitize_setting_value(max_record_count),
        max_text_length: sanitize_setting_value(max_text_length),
        ignore_password_like_text,
        storage_dir: storage_dir.trim().to_string(),
    };
    let now = Local::now().to_rfc3339();
    repository::set_setting(path, "retention_days", &settings.retention_days.to_string(), &now)?;
    repository::set_setting(path, "max_record_count", &settings.max_record_count.to_string(), &now)?;
    repository::set_setting(path, "max_text_length", &settings.max_text_length.to_string(), &now)?;
    repository::set_setting(
        path,
        "ignore_password_like_text",
        &bool_to_setting(settings.ignore_password_like_text).to_string(),
        &now,
    )?;
    repository::set_setting(path, "storage_dir", &settings.storage_dir, &now)?;
    Ok(settings)
}

pub fn apply_retention_policy(path: &Path, settings_path: &Path) -> Result<usize, ClipboardError> {
    let settings = get_stored_settings(settings_path)?;
    let now = Local::now().to_rfc3339();
    let cutoff = Local::now() - Duration::days(settings.retention_days);
    let cutoff_date = cutoff.format("%Y-%m-%d").to_string();
    repository::cleanup_items(path, &cutoff_date, settings.max_record_count, &now)
}

pub fn should_ignore_content(content: &str, settings: &StoredSettings) -> bool {
    content.chars().count() > settings.max_text_length as usize
        || (settings.ignore_password_like_text && is_password_like_text(content))
}

fn is_password_like_text(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.len() < 16 || trimmed.contains(char::is_whitespace) {
        return false;
    }
    is_jwt_like(trimmed) || looks_like_secret_token(trimmed)
}

fn is_jwt_like(value: &str) -> bool {
    value.split('.').count() == 3 && value.starts_with("eyJ")
}

fn looks_like_secret_token(value: &str) -> bool {
    let allowed = value
        .chars()
        .filter(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '-' | '='))
        .count();
    let letters = value.chars().filter(|char| char.is_ascii_alphabetic()).count();
    let digits = value.chars().filter(|char| char.is_ascii_digit()).count();
    allowed == value.len() && letters >= 8 && digits >= 4
}

fn sanitize_setting_value(value: i64) -> i64 {
    value.max(MIN_SETTING_VALUE)
}

fn bool_to_setting(value: bool) -> i64 {
    if value { 1 } else { 0 }
}
