use std::time::{SystemTime, UNIX_EPOCH};

use super::repository::{
    get_item_by_id, init_database, list_date_groups, list_items_by_date, search_items,
    cleanup_items, get_i64_setting, set_setting, soft_delete_item, soft_delete_items_by_date,
    upsert_text_item,
};

fn temp_database_path(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("clipboard-{name}-{unique}.sqlite"))
}

#[test]
fn inserts_and_lists_items_by_date() {
    let path = temp_database_path("insert-list");
    init_database(&path).unwrap();

    let item = upsert_text_item(&path, "hello", "hash-1", "2026-05-26T10:00:00+08:00")
        .unwrap();
    let groups = list_date_groups(&path).unwrap();
    let items = list_items_by_date(&path, "2026-05-26").unwrap();

    assert_eq!("hello", item.content);
    assert_eq!("2026-05-26", groups[0].date);
    assert_eq!(1, groups[0].count);
    assert_eq!(item.id, items[0].id);
}

#[test]
fn deduplicates_active_hashes() {
    let path = temp_database_path("dedupe");
    init_database(&path).unwrap();

    let first = upsert_text_item(&path, "hello", "hash-1", "2026-05-26T10:00:00+08:00")
        .unwrap();
    let second = upsert_text_item(&path, "hello", "hash-1", "2026-05-26T10:01:00+08:00")
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(2, second.copy_count);
    assert_eq!("2026-05-26T10:01:00+08:00", second.last_copied_at);
}

#[test]
fn soft_deleted_items_are_hidden() {
    let path = temp_database_path("delete");
    init_database(&path).unwrap();

    let item = upsert_text_item(&path, "secret", "hash-2", "2026-05-26T10:00:00+08:00")
        .unwrap();
    soft_delete_item(&path, item.id, "2026-05-26T10:02:00+08:00").unwrap();

    assert!(get_item_by_id(&path, item.id).is_err());
    assert!(list_items_by_date(&path, "2026-05-26").unwrap().is_empty());
}

#[test]
fn searches_active_content_across_dates() {
    let path = temp_database_path("search");
    init_database(&path).unwrap();

    upsert_text_item(&path, "alpha code", "hash-1", "2026-05-25T10:00:00+08:00").unwrap();
    upsert_text_item(&path, "beta note", "hash-2", "2026-05-26T10:00:00+08:00").unwrap();
    upsert_text_item(&path, "alpha docs", "hash-3", "2026-05-27T10:00:00+08:00").unwrap();

    let results = search_items(&path, "alpha").unwrap();

    assert_eq!(2, results.len());
    assert_eq!("alpha docs", results[0].content);
    assert_eq!("alpha code", results[1].content);
}

#[test]
fn search_ignores_deleted_items_and_blank_keywords() {
    let path = temp_database_path("search-hidden");
    init_database(&path).unwrap();

    let item = upsert_text_item(&path, "temporary token", "hash-1", "2026-05-26T10:00:00+08:00")
        .unwrap();
    soft_delete_item(&path, item.id, "2026-05-26T10:02:00+08:00").unwrap();

    assert!(search_items(&path, "temporary").unwrap().is_empty());
    assert!(search_items(&path, "   ").unwrap().is_empty());
}

#[test]
fn soft_deletes_all_items_by_date() {
    let path = temp_database_path("clear-date");
    init_database(&path).unwrap();

    upsert_text_item(&path, "today one", "hash-1", "2026-05-26T10:00:00+08:00").unwrap();
    upsert_text_item(&path, "today two", "hash-2", "2026-05-26T11:00:00+08:00").unwrap();
    upsert_text_item(&path, "other day", "hash-3", "2026-05-27T10:00:00+08:00").unwrap();

    let changed = soft_delete_items_by_date(&path, "2026-05-26", "2026-05-26T12:00:00+08:00")
        .unwrap();

    assert_eq!(2, changed);
    assert!(list_items_by_date(&path, "2026-05-26").unwrap().is_empty());
    assert_eq!(1, list_items_by_date(&path, "2026-05-27").unwrap().len());
}


#[test]
fn settings_default_and_update_roundtrip() {
    let path = temp_database_path("settings");
    init_database(&path).unwrap();

    assert_eq!(30, get_i64_setting(&path, "retention_days", 30).unwrap());
    set_setting(&path, "retention_days", "7", "2026-05-26T10:00:00+08:00").unwrap();

    assert_eq!(7, get_i64_setting(&path, "retention_days", 30).unwrap());
}

#[test]
fn cleanup_items_removes_old_dates() {
    let path = temp_database_path("cleanup-date");
    init_database(&path).unwrap();

    upsert_text_item(&path, "old", "hash-1", "2026-05-01T10:00:00+08:00").unwrap();
    upsert_text_item(&path, "new", "hash-2", "2026-05-26T10:00:00+08:00").unwrap();

    let changed = cleanup_items(&path, "2026-05-10", 100, "2026-05-27T10:00:00+08:00").unwrap();

    assert_eq!(1, changed);
    assert!(search_items(&path, "old").unwrap().is_empty());
    assert_eq!(1, search_items(&path, "new").unwrap().len());
}

#[test]
fn cleanup_items_respects_max_record_count() {
    let path = temp_database_path("cleanup-count");
    init_database(&path).unwrap();

    upsert_text_item(&path, "first", "hash-1", "2026-05-26T10:00:00+08:00").unwrap();
    upsert_text_item(&path, "second", "hash-2", "2026-05-26T11:00:00+08:00").unwrap();
    upsert_text_item(&path, "third", "hash-3", "2026-05-26T12:00:00+08:00").unwrap();

    let changed = cleanup_items(&path, "2026-05-01", 2, "2026-05-27T10:00:00+08:00").unwrap();
    let items = list_items_by_date(&path, "2026-05-26").unwrap();

    assert_eq!(1, changed);
    assert_eq!(2, items.len());
    assert_eq!("third", items[0].content);
    assert_eq!("second", items[1].content);
}
