use std::time::{SystemTime, UNIX_EPOCH};

use super::repository::{
    cleanup_items, get_i64_setting, get_item_by_id, get_string_setting, init_schema,
    list_date_groups, list_items_by_date, migrate_schema, search_items, set_setting,
    soft_delete_item, soft_delete_items_by_date, upsert_text_item,
};
use super::service_runtime::open_connection;

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
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let item = upsert_text_item(&conn, "hello", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    let groups = list_date_groups(&conn).unwrap();
    let items = list_items_by_date(&conn, "2026-05-26").unwrap();

    assert_eq!("hello", item.content);
    assert_eq!("2026-05-26", groups[0].date);
    assert_eq!(1, groups[0].count);
    assert_eq!(item.id, items[0].id);
}

#[test]
fn deduplicates_active_hashes() {
    let path = temp_database_path("dedupe");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let first = upsert_text_item(&conn, "hello", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    let second = upsert_text_item(&conn, "hello", "hash-1", "2026-05-26T10:01:00+08:00", "2026-05-26").unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(2, second.copy_count);
    assert_eq!("2026-05-26T10:01:00+08:00", second.last_copied_at);
}

#[test]
fn soft_deleted_items_are_hidden() {
    let path = temp_database_path("delete");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let item = upsert_text_item(&conn, "secret", "hash-2", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    soft_delete_item(&conn, item.id, "2026-05-26T10:02:00+08:00").unwrap();

    assert!(get_item_by_id(&conn, item.id).is_err());
    assert!(list_items_by_date(&conn, "2026-05-26").unwrap().is_empty());
}

#[test]
fn searches_active_content_across_dates() {
    let path = temp_database_path("search");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    upsert_text_item(&conn, "alpha code", "hash-1", "2026-05-25T10:00:00+08:00", "2026-05-25").unwrap();
    upsert_text_item(&conn, "beta note", "hash-2", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    upsert_text_item(&conn, "alpha docs", "hash-3", "2026-05-27T10:00:00+08:00", "2026-05-27").unwrap();

    let results = search_items(&conn, "alpha").unwrap();

    assert_eq!(2, results.len());
    assert_eq!("alpha docs", results[0].content);
    assert_eq!("alpha code", results[1].content);
}

#[test]
fn search_ignores_deleted_items_and_blank_keywords() {
    let path = temp_database_path("search-hidden");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let item = upsert_text_item(
        &conn,
        "temporary token",
        "hash-1",
        "2026-05-26T10:00:00+08:00",
        "2026-05-26",
    )
    .unwrap();
    soft_delete_item(&conn, item.id, "2026-05-26T10:02:00+08:00").unwrap();

    assert!(search_items(&conn, "temporary").unwrap().is_empty());
    assert!(search_items(&conn, "   ").unwrap().is_empty());
}

#[test]
fn soft_deletes_all_items_by_date() {
    let path = temp_database_path("clear-date");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    upsert_text_item(&conn, "today one", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    upsert_text_item(&conn, "today two", "hash-2", "2026-05-26T11:00:00+08:00", "2026-05-26").unwrap();
    upsert_text_item(&conn, "other day", "hash-3", "2026-05-27T10:00:00+08:00", "2026-05-27").unwrap();

    let changed =
        soft_delete_items_by_date(&conn, "2026-05-26", "2026-05-26T12:00:00+08:00").unwrap();

    assert_eq!(2, changed.len());
    assert!(list_items_by_date(&conn, "2026-05-26").unwrap().is_empty());
    assert_eq!(1, list_items_by_date(&conn, "2026-05-27").unwrap().len());
}

#[test]
fn settings_default_and_update_roundtrip() {
    let path = temp_database_path("settings");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    assert_eq!(30, get_i64_setting(&conn, "retention_days", 30).unwrap());
    assert_eq!("", get_string_setting(&conn, "storage_dir", "").unwrap());
    set_setting(&conn, "retention_days", "7", "2026-05-26T10:00:00+08:00").unwrap();
    set_setting(&conn, "storage_dir", "D:/clip", "2026-05-26T10:00:00+08:00").unwrap();

    assert_eq!(7, get_i64_setting(&conn, "retention_days", 30).unwrap());
    assert_eq!(
        "D:/clip",
        get_string_setting(&conn, "storage_dir", "").unwrap()
    );
}

#[test]
fn cleanup_items_removes_old_dates() {
    let path = temp_database_path("cleanup-date");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    upsert_text_item(&conn, "old", "hash-1", "2026-05-01T10:00:00+08:00", "2026-05-01").unwrap();
    upsert_text_item(&conn, "new", "hash-2", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();

    let changed = cleanup_items(&conn, "2026-05-10", 100, "2026-05-27T10:00:00+08:00").unwrap();

    assert_eq!(1, changed);
    assert!(search_items(&conn, "old").unwrap().is_empty());
    assert_eq!(1, search_items(&conn, "new").unwrap().len());
}

#[test]
fn cleanup_items_respects_max_record_count() {
    let path = temp_database_path("cleanup-count");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    upsert_text_item(&conn, "first", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    upsert_text_item(&conn, "second", "hash-2", "2026-05-26T11:00:00+08:00", "2026-05-26").unwrap();
    upsert_text_item(&conn, "third", "hash-3", "2026-05-26T12:00:00+08:00", "2026-05-26").unwrap();

    let changed = cleanup_items(&conn, "2026-05-01", 2, "2026-05-27T10:00:00+08:00").unwrap();
    let items = list_items_by_date(&conn, "2026-05-26").unwrap();

    assert_eq!(1, changed);
    assert_eq!(2, items.len());
    assert_eq!("third", items[0].content);
    assert_eq!("second", items[1].content);
}

#[test]
fn purge_deleted_items_removes_only_soft_deleted_rows() {
    let path = temp_database_path("purge-deleted");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let active = upsert_text_item(&conn, "active", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    let deleted =
        upsert_text_item(&conn, "deleted", "hash-2", "2026-05-26T11:00:00+08:00", "2026-05-26").unwrap();
    soft_delete_item(&conn, deleted.id, "2026-05-26T12:00:00+08:00").unwrap();

    let removed = super::maintenance::purge_deleted_items(&conn).unwrap();
    let items = list_items_by_date(&conn, "2026-05-26").unwrap();

    assert_eq!(1, removed);
    assert_eq!(1, items.len());
    assert_eq!(active.id, items[0].id);
}

#[test]
fn init_then_migrate_creates_local_date_column_and_index() {
    let path = temp_database_path("local-date-schema");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let has_local_date = conn
        .prepare("PRAGMA table_info(clipboard_items)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(Result::ok)
        .any(|name| name == "local_date");
    assert!(has_local_date, "local_date column should exist");

    let has_index = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name=?1")
        .unwrap()
        .query_map(["idx_clipboard_items_local_date_active"], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(Result::ok)
        .next()
        .is_some();
    assert!(has_index, "local_date active index should exist");
}

#[test]
fn migrate_schema_backfills_local_date_and_converts_to_utc() {
    let path = temp_database_path("migrate-backfill");
    let conn = open_connection(&path).unwrap();
    // Build an OLD-format table WITHOUT local_date, matching pre-migration schema.
    conn.execute_batch(
        "CREATE TABLE clipboard_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content_type TEXT NOT NULL,
            content TEXT NOT NULL,
            preview TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_copied_at TEXT NOT NULL,
            copy_count INTEGER NOT NULL DEFAULT 1,
            deleted_at TEXT
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO clipboard_items
            (content_type, content, preview, content_hash, created_at, last_copied_at, copy_count, deleted_at)
         VALUES ('text', 'hi', 'hi', 'h',
            '2026-05-29T08:53:00.123456789+08:00',
            '2026-05-29T09:00:00.000000000+08:00', 1, NULL)",
        [],
    )
    .unwrap();

    migrate_schema(&conn).unwrap();

    let (local_date, created_at, last_copied_at): (String, String, String) = conn
        .query_row(
            "SELECT local_date, created_at, last_copied_at FROM clipboard_items WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!("2026-05-29", local_date);
    assert_eq!("2026-05-29T00:53:00Z", created_at);
    assert_eq!("2026-05-29T01:00:00Z", last_copied_at);
}

#[test]
fn migrate_schema_is_idempotent() {
    let path = temp_database_path("migrate-idempotent");
    let conn = open_connection(&path).unwrap();
    // 从真正的旧格式表（无 local_date）出发，验证重复迁移不改值
    conn.execute_batch(
        "CREATE TABLE clipboard_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content_type TEXT NOT NULL,
            content TEXT NOT NULL,
            preview TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_copied_at TEXT NOT NULL,
            copy_count INTEGER NOT NULL DEFAULT 1,
            deleted_at TEXT
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO clipboard_items
            (content_type, content, preview, content_hash, created_at, last_copied_at, copy_count, deleted_at)
         VALUES ('text', 'hi', 'hi', 'h',
            '2026-05-29T08:53:00.123456789+08:00',
            '2026-05-29T09:00:00.000000000+08:00', 1, NULL)",
        [],
    )
    .unwrap();

    migrate_schema(&conn).unwrap();
    let after_first: (String, String, String) = conn
        .query_row(
            "SELECT local_date, created_at, last_copied_at FROM clipboard_items WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    // 第二次迁移走 PRAGMA 早退路径，值不应再被改动
    migrate_schema(&conn).unwrap();
    let after_second: (String, String, String) = conn
        .query_row(
            "SELECT local_date, created_at, last_copied_at FROM clipboard_items WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(("2026-05-29".to_string(), "2026-05-29T00:53:00Z".to_string(), "2026-05-29T01:00:00Z".to_string()), after_first);
    assert_eq!(after_first, after_second, "重复迁移不应改变已迁移的值");
}

#[test]
fn init_then_migrate_upgrades_legacy_db_without_local_date() {
    let path = temp_database_path("legacy-upgrade");
    let conn = open_connection(&path).unwrap();
    // 模拟旧版本遗留的库：clipboard_items 表已存在但没有 local_date 列。
    conn.execute_batch(
        "CREATE TABLE clipboard_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content_type TEXT NOT NULL,
            content TEXT NOT NULL,
            preview TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_copied_at TEXT NOT NULL,
            copy_count INTEGER NOT NULL DEFAULT 1,
            deleted_at TEXT
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO clipboard_items
            (content_type, content, preview, content_hash, created_at, last_copied_at, copy_count, deleted_at)
         VALUES ('text', 'legacy', 'legacy', 'h-legacy',
            '2026-05-20T08:00:00.000000000+08:00',
            '2026-05-20T08:00:00.000000000+08:00', 1, NULL)",
        [],
    )
    .unwrap();

    // 生产启动顺序（service.rs）：先 init_schema 再 migrate_schema。
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let has_local_date = conn
        .prepare("PRAGMA table_info(clipboard_items)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(Result::ok)
        .any(|name| name == "local_date");
    assert!(has_local_date, "local_date column should exist after upgrade");

    let has_index = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name=?1")
        .unwrap()
        .query_map(["idx_clipboard_items_local_date_active"], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(Result::ok)
        .next()
        .is_some();
    assert!(has_index, "local_date active index should exist after upgrade");

    let local_date: String = conn
        .query_row(
            "SELECT local_date FROM clipboard_items WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!("2026-05-20", local_date, "existing rows should backfill local_date");
}

#[test]
fn list_date_groups_uses_local_date_column() {
    let path = temp_database_path("groups-local-date");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    upsert_text_item(&conn, "a", "ha", "2026-05-26T10:00:00Z", "2026-05-26").unwrap();
    upsert_text_item(&conn, "b", "hb", "2026-05-27T10:00:00Z", "2026-05-27").unwrap();

    let groups = list_date_groups(&conn).unwrap();
    assert_eq!("2026-05-27", groups[0].date);
    assert_eq!("2026-05-26", groups[1].date);
}

#[test]
fn list_items_by_date_orders_by_real_utc_time() {
    let path = temp_database_path("order-utc");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    // Same local_date; 07:00Z is later than 00:53Z.
    upsert_text_item(&conn, "early", "he", "2026-05-29T00:53:00Z", "2026-05-29").unwrap();
    upsert_text_item(&conn, "late", "hl", "2026-05-29T07:00:00Z", "2026-05-29").unwrap();

    let items = list_items_by_date(&conn, "2026-05-29").unwrap();
    assert_eq!("late", items[0].content);
    assert_eq!("early", items[1].content);
}

#[test]
fn migrate_schema_creates_fts_table_and_triggers() {
    let path = temp_database_path("fts-schema");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let has_fts: bool = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='clipboard_fts'")
        .unwrap()
        .exists([])
        .unwrap();
    assert!(has_fts, "clipboard_fts virtual table should exist");

    let trigger_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger'
             AND name IN ('clipboard_fts_ai','clipboard_fts_ad','clipboard_fts_au')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(3, trigger_count, "three FTS sync triggers should exist");
}

#[test]
fn migrate_schema_rebuilds_fts_for_existing_rows() {
    let path = temp_database_path("fts-rebuild");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    // 在建 FTS 之前插入存量数据（绕过触发器，模拟旧库）
    conn.execute(
        "INSERT INTO clipboard_items
            (content_type, content, preview, content_hash, created_at, last_copied_at, copy_count, local_date)
         VALUES ('text', '历史归档内容', '历史归档内容', 'h-legacy',
            '2026-05-20T08:00:00Z', '2026-05-20T08:00:00Z', 1, '2026-05-20')",
        [],
    )
    .unwrap();

    migrate_schema(&conn).unwrap();

    // rebuild 后存量行应可被 FTS MATCH 命中
    let matched: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE clipboard_fts MATCH '\"归档内\"'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(1, matched, "rebuild should index pre-existing rows");
}

#[test]
fn search_matches_chinese_substring() {
    let path = temp_database_path("search-cn");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    upsert_text_item(&conn, "今天天气很好", "hash-cn", "2026-05-28T10:00:00Z", "2026-05-28").unwrap();

    let results = search_items(&conn, "天气很").unwrap();
    assert_eq!(1, results.len());
    assert_eq!("今天天气很好", results[0].content);
}

#[test]
fn search_short_keyword_falls_back_to_like() {
    let path = temp_database_path("search-short");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    upsert_text_item(&conn, "今天天气", "hash-short", "2026-05-28T10:00:00Z", "2026-05-28").unwrap();

    // 2 字查询低于 trigram 3 字下限，经 LIKE 回退仍应命中
    let results = search_items(&conn, "天气").unwrap();
    assert_eq!(1, results.len());
    assert_eq!("今天天气", results[0].content);
}

#[test]
fn search_excludes_soft_deleted_via_fts() {
    let path = temp_database_path("search-fts-deleted");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    let item = upsert_text_item(&conn, "错误日志记录", "hash-del", "2026-05-28T10:00:00Z", "2026-05-28").unwrap();
    assert_eq!(1, search_items(&conn, "错误日").unwrap().len());

    soft_delete_item(&conn, item.id, "2026-05-28T11:00:00Z").unwrap();
    assert!(search_items(&conn, "错误日").unwrap().is_empty());
}

#[test]
fn search_is_case_insensitive_via_fts() {
    let path = temp_database_path("search-case");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();
    migrate_schema(&conn).unwrap();

    upsert_text_item(&conn, "AlphaCode", "hash-case", "2026-05-28T10:00:00Z", "2026-05-28").unwrap();

    let results = search_items(&conn, "alphac").unwrap();
    assert_eq!(1, results.len());
    assert_eq!("AlphaCode", results[0].content);
}

#[test]
fn clear_returns_soft_deleted_ids() {
    let path = temp_database_path("clear-returns-ids");
    let conn = open_connection(&path).unwrap();
    init_schema(&conn).unwrap();

    let one = upsert_text_item(&conn, "one", "hash-1", "2026-05-26T10:00:00+08:00", "2026-05-26").unwrap();
    let two = upsert_text_item(&conn, "two", "hash-2", "2026-05-26T11:00:00+08:00", "2026-05-26").unwrap();
    let already = upsert_text_item(&conn, "gone", "hash-3", "2026-05-26T09:00:00+08:00", "2026-05-26").unwrap();
    soft_delete_item(&conn, already.id, "2026-05-26T09:30:00+08:00").unwrap();

    let mut ids = soft_delete_items_by_date(&conn, "2026-05-26", "2026-05-26T12:00:00+08:00").unwrap();
    ids.sort();
    let mut expected = vec![one.id, two.id];
    expected.sort();

    assert_eq!(expected, ids);
    assert!(!ids.contains(&already.id));
}
