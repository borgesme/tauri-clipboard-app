use std::time::{SystemTime, UNIX_EPOCH};

use super::settings::validate_storage_dir;

fn unique_name(name: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("clipboard-storage-{name}-{unique}")
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(unique_name(name))
}

#[test]
fn blank_storage_dir_is_valid() {
    validate_storage_dir("").unwrap();
    validate_storage_dir("   ").unwrap();
}

#[test]
fn existing_directory_is_valid_and_does_not_create_database() {
    let directory = temp_path("existing");
    std::fs::create_dir_all(&directory).unwrap();

    validate_storage_dir(&directory.to_string_lossy()).unwrap();

    assert!(!directory.join("clipboard.sqlite").exists());
}

#[test]
fn new_directory_can_be_created_for_validation() {
    let directory = temp_path("new");

    validate_storage_dir(&directory.to_string_lossy()).unwrap();

    assert!(directory.is_dir());
    assert!(!directory.join("clipboard.sqlite").exists());
}

#[test]
fn path_with_file_parent_is_invalid() {
    let file_path = temp_path("file-parent");
    std::fs::write(&file_path, "not a dir").unwrap();
    let invalid = file_path.join("child");

    assert!(validate_storage_dir(&invalid.to_string_lossy()).is_err());
}
