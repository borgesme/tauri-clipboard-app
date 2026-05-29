use std::time::{SystemTime, UNIX_EPOCH};

use super::models::{ClipboardSkipReason, DesktopSettingsUpdate};
use super::repository;
use super::service::ClipboardService;
use super::settings;

fn unique_name(name: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("clipboard-service-{name}-{unique}")
}

fn temp_database_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{}.sqlite", unique_name(name)))
}

fn temp_storage_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(unique_name(name))
}

fn desktop_update(storage_dir: String) -> DesktopSettingsUpdate {
    DesktopSettingsUpdate {
        autostart_enabled: false,
        monitor_enabled: true,
        retention_days: 30,
        max_record_count: 1000,
        max_text_length: 20_000,
        ignore_password_like_text: false,
        custom_secret_patterns: String::new(),
        storage_dir,
    }
}

fn stored_settings() -> super::models::StoredSettings {
    super::models::StoredSettings {
        monitor_enabled: true,
        retention_days: 30,
        max_record_count: 1000,
        max_text_length: 20_000,
        ignore_password_like_text: false,
        custom_secret_patterns: String::new(),
        storage_dir: String::new(),
    }
}

#[test]
fn monitor_state_defaults_to_enabled() {
    let default_path = temp_database_path("monitor-default");
    let service = ClipboardService::new(default_path).unwrap();
    let settings = service.desktop_settings(false).unwrap();

    assert!(settings.monitor_enabled);
    assert!(service.monitor_status().unwrap().enabled);
}

#[test]
fn monitor_state_persists_across_service_restart() {
    let default_path = temp_database_path("monitor-persist");
    let service = ClipboardService::new(default_path.clone()).unwrap();
    service.set_monitor_enabled(false).unwrap();

    let restarted_service = ClipboardService::new(default_path).unwrap();
    let settings = restarted_service.desktop_settings(false).unwrap();

    assert!(!settings.monitor_enabled);
    assert!(!restarted_service.monitor_status().unwrap().enabled);
}

#[test]
fn update_settings_switches_to_custom_storage_database() {
    let default_path = temp_database_path("switch-default");
    let custom_dir = temp_storage_dir("switch-custom");
    let custom_dir_text = custom_dir.to_string_lossy().into_owned();
    let custom_path = custom_dir.join("clipboard.sqlite");
    let service = ClipboardService::new(default_path.clone()).unwrap();

    {
        let conn = super::service_runtime::open_connection(&default_path).unwrap();
        repository::init_schema(&conn).unwrap();
        repository::upsert_text_item(&conn, "default", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26")
            .unwrap();
    }
    let settings = service
        .update_desktop_settings(desktop_update(custom_dir_text.clone()), false)
        .unwrap();
    {
        let conn = super::service_runtime::open_connection(&custom_path).unwrap();
        repository::init_schema(&conn).unwrap();
        repository::upsert_text_item(&conn, "custom", "hash-2", "2026-05-26T11:00:00+08:00", "2026-05-26")
            .unwrap();
    }

    assert_eq!(custom_dir_text, settings.storage_dir);
    assert!(service.search_items("default").unwrap().is_empty());
    assert_eq!(1, service.search_items("custom").unwrap().len());
}

#[test]
fn startup_uses_storage_dir_from_default_database() {
    let default_path = temp_database_path("startup-default");
    let custom_dir = temp_storage_dir("startup-custom");
    let custom_dir_text = custom_dir.to_string_lossy().into_owned();
    let custom_path = custom_dir.join("clipboard.sqlite");
    {
        let conn = super::service_runtime::open_connection(&default_path).unwrap();
        repository::init_schema(&conn).unwrap();
        settings::update_stored_settings(
            &conn,
            false,
            15,
            50,
            1024,
            true,
            "",
            &custom_dir_text,
        )
        .unwrap();
    }

    let service = ClipboardService::new(default_path).unwrap();
    {
        let conn = super::service_runtime::open_connection(&custom_path).unwrap();
        repository::init_schema(&conn).unwrap();
        repository::upsert_text_item(&conn, "custom", "hash-1", "2026-05-26T11:00:00+08:00", "2026-05-26")
            .unwrap();
    }
    let settings = service.desktop_settings(false).unwrap();

    assert_eq!(15, settings.retention_days);
    assert_eq!(50, settings.max_record_count);
    assert_eq!(1024, settings.max_text_length);
    assert!(settings.ignore_password_like_text);
    assert_eq!(custom_dir_text, settings.storage_dir);
    assert_eq!(1, service.search_items("custom").unwrap().len());
}

#[test]
fn content_filters_respect_length_and_secret_settings() {
    let settings = super::models::StoredSettings {
        max_text_length: 5,
        ignore_password_like_text: true,
        ..stored_settings()
    };

    assert!(super::settings::content_skip_reason("123456", &settings).is_some());
    assert!(super::settings::content_skip_reason("abcDEF1234567890", &settings).is_some());
    assert!(super::settings::content_skip_reason("hello", &settings).is_none());
}

#[test]
fn content_skip_reason_distinguishes_too_long_and_secret_like() {
    let settings = super::models::StoredSettings {
        max_text_length: 5,
        ignore_password_like_text: true,
        ..stored_settings()
    };

    assert_eq!(
        Some(ClipboardSkipReason::TooLong),
        super::settings::content_skip_reason("123456", &settings),
    );
    assert_eq!(
        Some(ClipboardSkipReason::SecretLike),
        super::settings::content_skip_reason(
            "abcDEFGH1234567890XY",
            &StoredSettingsForSecret::from(&settings)
        ),
    );
    assert_eq!(
        None,
        super::settings::content_skip_reason("hello", &settings)
    );
}

struct StoredSettingsForSecret;

impl StoredSettingsForSecret {
    fn from(settings: &super::models::StoredSettings) -> super::models::StoredSettings {
        super::models::StoredSettings {
            max_text_length: 20_000,
            ..settings.clone()
        }
    }
}

#[test]
fn custom_secret_pattern_skips_matching_content() {
    let settings = super::models::StoredSettings {
        ignore_password_like_text: true,
        custom_secret_patterns: "corp_[0-9]{4}".to_string(),
        ..stored_settings()
    };

    assert_eq!(
        Some(ClipboardSkipReason::SecretLike),
        super::settings::content_skip_reason("token=corp_1234", &settings),
    );
}

#[test]
fn custom_secret_pattern_keeps_non_matching_content() {
    let settings = super::models::StoredSettings {
        ignore_password_like_text: true,
        custom_secret_patterns: "corp_[0-9]{4}".to_string(),
        ..stored_settings()
    };

    assert_eq!(
        None,
        super::settings::content_skip_reason("plain project note", &settings),
    );
}

#[test]
fn custom_secret_pattern_requires_secret_filter_enabled() {
    let settings = super::models::StoredSettings {
        ignore_password_like_text: false,
        custom_secret_patterns: "corp_[0-9]{4}".to_string(),
        ..stored_settings()
    };

    assert_eq!(
        None,
        super::settings::content_skip_reason("token=corp_1234", &settings),
    );
}

#[test]
fn invalid_custom_secret_pattern_is_rejected() {
    let path = temp_database_path("invalid-pattern");
    let conn = super::service_runtime::open_connection(&path).unwrap();
    repository::init_schema(&conn).unwrap();

    let result = settings::update_stored_settings(&conn, true, 30, 1000, 20_000, false, "[", "");

    assert!(result.is_err());
}

#[test]
fn retention_runs_only_after_threshold_captures() {
    use super::service::RETENTION_TRIGGER_THRESHOLD;

    let default_path = temp_database_path("threshold");
    let service = ClipboardService::new(default_path.clone()).unwrap();

    // 设置：保留所有日期 + max_record_count = 5
    service
        .update_desktop_settings(
            DesktopSettingsUpdate {
                autostart_enabled: false,
                monitor_enabled: true,
                retention_days: 30,
                max_record_count: 5,
                max_text_length: 20_000,
                ignore_password_like_text: false,
                custom_secret_patterns: String::new(),
                storage_dir: String::new(),
            },
            false,
        )
        .unwrap();

    // 通过测试钩子直接写入 items 表，绕过剪贴板依赖
    for i in 0..(RETENTION_TRIGGER_THRESHOLD - 1) {
        let conn = super::service_runtime::open_connection(&default_path).unwrap();
        repository::upsert_text_item(
            &conn,
            &format!("item-{i}"),
            &format!("hash-{i}"),
            "2026-05-26T10:00:00+08:00",
            "2026-05-26",
        )
        .unwrap();
        // 模拟一次 capture 完成：调内部钩子推进计数
        service.tick_capture_count_for_test().unwrap();
    }

    // 此时已写入 THRESHOLD-1 条；max_record_count=5 但应未触发 retention，
    // 因此条目数等于 THRESHOLD-1（远超 5）
    let groups = service.list_date_groups().unwrap();
    let total: i64 = groups.iter().map(|g| g.count).sum();
    assert_eq!(
        (RETENTION_TRIGGER_THRESHOLD - 1) as i64,
        total,
        "retention 不应在阈值之前触发"
    );

    // 写第 THRESHOLD 条，应触发 retention，条目数回落到 5
    let conn = super::service_runtime::open_connection(&default_path).unwrap();
    repository::upsert_text_item(
        &conn,
        "item-final",
        "hash-final",
        "2026-05-26T10:00:00+08:00",
        "2026-05-26",
    )
    .unwrap();
    service.tick_capture_count_for_test().unwrap();

    let groups_after = service.list_date_groups().unwrap();
    let total_after: i64 = groups_after.iter().map(|g| g.count).sum();
    assert_eq!(5, total_after, "retention 触发后应裁剪到 max_record_count");
}

#[test]
fn retention_counter_resets_after_settings_update() {
    use super::service::RETENTION_TRIGGER_THRESHOLD;

    let default_path = temp_database_path("counter-reset");
    let service = ClipboardService::new(default_path).unwrap();

    // 推进计数到 THRESHOLD - 1，刚好不触发
    for _ in 0..(RETENTION_TRIGGER_THRESHOLD - 1) {
        service.tick_capture_count_for_test().unwrap();
    }

    // 更新设置：应重置计数
    service
        .update_desktop_settings(
            DesktopSettingsUpdate {
                autostart_enabled: false,
                monitor_enabled: true,
                retention_days: 30,
                max_record_count: 1000,
                max_text_length: 20_000,
                ignore_password_like_text: false,
                custom_secret_patterns: String::new(),
                storage_dir: String::new(),
            },
            false,
        )
        .unwrap();

    // 再推进 THRESHOLD - 1 次，应仍不触发；若计数未重置则会触发
    for _ in 0..(RETENTION_TRIGGER_THRESHOLD - 1) {
        service.tick_capture_count_for_test().unwrap();
    }

    let count = service.captures_count_for_test().unwrap();
    assert_eq!(
        (RETENTION_TRIGGER_THRESHOLD - 1) as u32,
        count,
        "更新设置后计数应归零，再推进 N-1 次不应触发"
    );
}
