use std::time::{SystemTime, UNIX_EPOCH};

use super::repository::{
    get_item_by_id, init_database, list_date_groups, list_items_by_date, search_items,
    soft_delete_item, soft_delete_items_by_date, upsert_text_item,
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
