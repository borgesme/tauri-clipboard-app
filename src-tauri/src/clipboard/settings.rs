use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::{Duration, Local};
use regex::Regex;

use super::error::ClipboardError;
use super::models::{ClipboardSkipReason, StoredSettings};
use super::repository;

pub const DEFAULT_RETENTION_DAYS: i64 = 30;
pub const DEFAULT_MAX_RECORD_COUNT: i64 = 1000;
pub const DEFAULT_MAX_TEXT_LENGTH: i64 = 20_000;
pub const DEFAULT_STORAGE_DIR: &str = "";
pub const DEFAULT_CUSTOM_SECRET_PATTERNS: &str = "";
const MIN_SETTING_VALUE: i64 = 1;
const DEFAULT_MONITOR_ENABLED: bool = true;
const DEFAULT_IGNORE_PASSWORD_LIKE_TEXT: bool = false;

pub fn get_stored_settings(path: &Path) -> Result<StoredSettings, ClipboardError> {
    Ok(StoredSettings {
        monitor_enabled: repository::get_i64_setting(
            path,
            "monitor_enabled",
            bool_to_setting(DEFAULT_MONITOR_ENABLED),
        )? == 1,
        retention_days: repository::get_i64_setting(
            path,
            "retention_days",
            DEFAULT_RETENTION_DAYS,
        )?,
        max_record_count: repository::get_i64_setting(
            path,
            "max_record_count",
            DEFAULT_MAX_RECORD_COUNT,
        )?,
        max_text_length: repository::get_i64_setting(
            path,
            "max_text_length",
            DEFAULT_MAX_TEXT_LENGTH,
        )?,
        ignore_password_like_text: repository::get_i64_setting(
            path,
            "ignore_password_like_text",
            bool_to_setting(DEFAULT_IGNORE_PASSWORD_LIKE_TEXT),
        )? == 1,
        custom_secret_patterns: repository::get_string_setting(
            path,
            "custom_secret_patterns",
            DEFAULT_CUSTOM_SECRET_PATTERNS,
        )?,
        storage_dir: repository::get_string_setting(path, "storage_dir", DEFAULT_STORAGE_DIR)?,
    })
}

pub fn update_stored_settings(
    path: &Path,
    monitor_enabled: bool,
    retention_days: i64,
    max_record_count: i64,
    max_text_length: i64,
    ignore_password_like_text: bool,
    custom_secret_patterns: &str,
    storage_dir: &str,
) -> Result<StoredSettings, ClipboardError> {
    validate_custom_secret_patterns(custom_secret_patterns)?;
    let settings = StoredSettings {
        monitor_enabled,
        retention_days: sanitize_setting_value(retention_days),
        max_record_count: sanitize_setting_value(max_record_count),
        max_text_length: sanitize_setting_value(max_text_length),
        ignore_password_like_text,
        custom_secret_patterns: custom_secret_patterns.trim().to_string(),
        storage_dir: storage_dir.trim().to_string(),
    };
    let now = Local::now().to_rfc3339();
    repository::set_setting(
        path,
        "monitor_enabled",
        &bool_to_setting(settings.monitor_enabled).to_string(),
        &now,
    )?;
    repository::set_setting(
        path,
        "retention_days",
        &settings.retention_days.to_string(),
        &now,
    )?;
    repository::set_setting(
        path,
        "max_record_count",
        &settings.max_record_count.to_string(),
        &now,
    )?;
    repository::set_setting(
        path,
        "max_text_length",
        &settings.max_text_length.to_string(),
        &now,
    )?;
    repository::set_setting(
        path,
        "ignore_password_like_text",
        &bool_to_setting(settings.ignore_password_like_text).to_string(),
        &now,
    )?;
    repository::set_setting(
        path,
        "custom_secret_patterns",
        &settings.custom_secret_patterns,
        &now,
    )?;
    repository::set_setting(path, "storage_dir", &settings.storage_dir, &now)?;
    Ok(settings)
}

pub fn update_monitor_enabled(path: &Path, enabled: bool) -> Result<(), ClipboardError> {
    let now = Local::now().to_rfc3339();
    repository::set_setting(
        path,
        "monitor_enabled",
        &bool_to_setting(enabled).to_string(),
        &now,
    )
}

pub fn validate_storage_dir(storage_dir: &str) -> Result<(), ClipboardError> {
    let storage_dir = storage_dir.trim();
    if storage_dir.is_empty() {
        return Ok(());
    }
    let directory = PathBuf::from(storage_dir);
    if directory.exists() && !directory.is_dir() {
        return Err(ClipboardError::Io("存储路径不是文件夹".to_string()));
    }
    std::fs::create_dir_all(&directory)?;
    let probe_path = directory.join(".clipboard-write-test.tmp");
    std::fs::write(&probe_path, b"ok")?;
    std::fs::remove_file(probe_path)?;
    Ok(())
}

pub fn apply_retention_policy(path: &Path, settings_path: &Path) -> Result<usize, ClipboardError> {
    let settings = get_stored_settings(settings_path)?;
    let now = Local::now().to_rfc3339();
    let cutoff = Local::now() - Duration::days(settings.retention_days);
    let cutoff_date = cutoff.format("%Y-%m-%d").to_string();
    repository::cleanup_items(path, &cutoff_date, settings.max_record_count, &now)
}

pub fn content_skip_reason(
    content: &str,
    settings: &StoredSettings,
) -> Option<ClipboardSkipReason> {
    if content.chars().count() > settings.max_text_length as usize {
        return Some(ClipboardSkipReason::TooLong);
    }
    if !settings.ignore_password_like_text {
        return None;
    }
    if is_password_like_text(content)
        || matches_custom_secret_patterns(content, &settings.custom_secret_patterns)
    {
        return Some(ClipboardSkipReason::SecretLike);
    }
    None
}

pub fn validate_custom_secret_patterns(patterns: &str) -> Result<(), ClipboardError> {
    for pattern in parse_custom_secret_patterns(patterns) {
        Regex::new(pattern)
            .map_err(|error| ClipboardError::Runtime(format!("自定义敏感规则无效：{error}")))?;
    }
    Ok(())
}

fn matches_custom_secret_patterns(content: &str, patterns: &str) -> bool {
    parse_custom_secret_patterns(patterns)
        .filter_map(|pattern| Regex::new(pattern).ok())
        .any(|regex| regex.is_match(content))
}

fn parse_custom_secret_patterns(patterns: &str) -> impl Iterator<Item = &str> {
    patterns
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
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
    if value.len() < 20 {
        return false;
    }
    let allowed = value
        .chars()
        .filter(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '-' | '='))
        .count();
    if allowed != value.len() {
        return false;
    }
    let uppercase = value
        .chars()
        .filter(|char| char.is_ascii_uppercase())
        .count();
    let lowercase = value
        .chars()
        .filter(|char| char.is_ascii_lowercase())
        .count();
    let digits = value.chars().filter(|char| char.is_ascii_digit()).count();
    if uppercase < 2 || lowercase < 2 || digits < 2 {
        return false;
    }
    if is_pure_hex(value) {
        return false;
    }
    if is_uuid_format(value) {
        return false;
    }
    true
}

fn is_pure_hex(value: &str) -> bool {
    value.chars().all(|char| char.is_ascii_hexdigit())
}

fn is_uuid_format(value: &str) -> bool {
    uuid_regex().is_match(value)
}

fn uuid_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
            .expect("UUID regex must compile")
    })
}

fn sanitize_setting_value(value: i64) -> i64 {
    value.max(MIN_SETTING_VALUE)
}

fn bool_to_setting(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::is_password_like_text;

    #[test]
    fn rejects_git_commit_hash() {
        assert!(!is_password_like_text("73778f4abc123def4567890abcdef1234567890ab"));
    }

    #[test]
    fn rejects_sha256_hex() {
        assert!(!is_password_like_text(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ));
    }

    #[test]
    fn rejects_uuid() {
        assert!(!is_password_like_text("aBcDeF01-2345-6789-AbCd-eF0123456789"));
    }

    #[test]
    fn rejects_pure_uppercase_hex() {
        assert!(!is_password_like_text("ABCDEF0123456789ABCDEF0123456789"));
    }

    #[test]
    fn rejects_short_string() {
        assert!(!is_password_like_text("abcDEF12"));
    }

    #[test]
    fn rejects_whitespace_content() {
        assert!(!is_password_like_text("hello world 12345 ABC"));
    }

    #[test]
    fn accepts_github_pat_form() {
        assert!(is_password_like_text(
            "ghp_1234abCDEFghIJKL5678mnopQRstUVwxyz9012"
        ));
    }

    #[test]
    fn accepts_openai_style_key() {
        assert!(is_password_like_text("sk_test_4eC39HqLyjWDarjtT1zdp7dc"));
    }

    #[test]
    fn accepts_jwt_via_jwt_path() {
        assert!(is_password_like_text(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        ));
    }

    #[test]
    fn accepts_boundary_twenty_chars_mixed() {
        assert!(is_password_like_text("Aa1Bb2Cc3Dd4Ee5Ff6Gg"));
    }

    #[test]
    fn rejects_nineteen_chars_mixed() {
        assert!(!is_password_like_text("Aa1Bb2Cc3Dd4Ee5Ff6G"));
    }

    #[test]
    fn rejects_twenty_chars_without_mix() {
        assert!(!is_password_like_text("aaaaaaaaaaaaaaaaaaaa"));
    }

    #[test]
    fn rejects_aws_access_key_known_miss() {
        assert!(!is_password_like_text("AKIAIOSFODNN7EXAMPLE"));
    }
}
