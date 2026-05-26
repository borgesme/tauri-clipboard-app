use std::time::{SystemTime, UNIX_EPOCH};

use super::models::DesktopSettingsUpdate;
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
        retention_days: 30,
        max_record_count: 1000,
        storage_dir,
    }
}

#[test]
fn update_settings_switches_to_custom_storage_database() {
    let default_path = temp_database_path("switch-default");
    let custom_dir = temp_storage_dir("switch-custom");
    let custom_dir_text = custom_dir.to_string_lossy().into_owned();
    let custom_path = custom_dir.join("clipboard.sqlite");
    let service = ClipboardService::new(default_path.clone()).unwrap();

    repository::upsert_text_item(&default_path, "default", "hash-1", "2026-05-26T10:00:00+08:00")
        .unwrap();
    let settings = service
        .update_desktop_settings(desktop_update(custom_dir_text.clone()), false)
        .unwrap();
    repository::upsert_text_item(&custom_path, "custom", "hash-2", "2026-05-26T11:00:00+08:00")
        .unwrap();

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
    repository::init_database(&default_path).unwrap();
    settings::update_stored_settings(&default_path, 15, 50, &custom_dir_text).unwrap();

    let service = ClipboardService::new(default_path).unwrap();
    repository::upsert_text_item(&custom_path, "custom", "hash-1", "2026-05-26T11:00:00+08:00")
        .unwrap();
    let settings = service.desktop_settings(false).unwrap();

    assert_eq!(15, settings.retention_days);
    assert_eq!(50, settings.max_record_count);
    assert_eq!(custom_dir_text, settings.storage_dir);
    assert_eq!(1, service.search_items("custom").unwrap().len());
}
