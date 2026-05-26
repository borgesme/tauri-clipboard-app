use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use super::error::ClipboardError;
use super::hash::preview;
use super::models::{ClipboardDateGroup, ClipboardItem};

const CONTENT_TYPE_TEXT: &str = "text";

pub fn init_database(path: &Path) -> Result<(), ClipboardError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let connection = Connection::open(path)?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS clipboard_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content_type TEXT NOT NULL,
            content TEXT NOT NULL,
            preview TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_copied_at TEXT NOT NULL,
            copy_count INTEGER NOT NULL DEFAULT 1,
            deleted_at TEXT
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_clipboard_items_hash_active
            ON clipboard_items(content_hash)
            WHERE deleted_at IS NULL;
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_created_at_active
            ON clipboard_items(created_at)
            WHERE deleted_at IS NULL;",
    )?;

    Ok(())
}

pub fn upsert_text_item(
    path: &Path,
    content: &str,
    content_hash: &str,
    now: &str,
) -> Result<ClipboardItem, ClipboardError> {
    let connection = Connection::open(path)?;
    let existing_id = find_active_id_by_hash(&connection, content_hash)?;

    if let Some(id) = existing_id {
        connection.execute(
            "UPDATE clipboard_items
             SET last_copied_at = ?1, copy_count = copy_count + 1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )?;
        return get_item_by_id_with_connection(&connection, id);
    }

    connection.execute(
        "INSERT INTO clipboard_items
         (content_type, content, preview, content_hash, created_at, last_copied_at, copy_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        params![CONTENT_TYPE_TEXT, content, preview(content), content_hash, now, now],
    )?;

    get_item_by_id_with_connection(&connection, connection.last_insert_rowid())
}

pub fn list_date_groups(path: &Path) -> Result<Vec<ClipboardDateGroup>, ClipboardError> {
    let connection = Connection::open(path)?;
    let mut statement = connection.prepare(
        "SELECT substr(created_at, 1, 10) AS date, COUNT(*) AS count
         FROM clipboard_items
         WHERE deleted_at IS NULL
         GROUP BY date
         ORDER BY date DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ClipboardDateGroup {
            date: row.get(0)?,
            count: row.get(1)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(ClipboardError::from)
}

pub fn list_items_by_date(path: &Path, date: &str) -> Result<Vec<ClipboardItem>, ClipboardError> {
    let connection = Connection::open(path)?;
    let mut statement = connection.prepare(
        "SELECT id, content_type, content, preview, content_hash, created_at, last_copied_at, copy_count
         FROM clipboard_items
         WHERE deleted_at IS NULL AND substr(created_at, 1, 10) = ?1
         ORDER BY last_copied_at DESC, id DESC",
    )?;
    let rows = statement.query_map([date], map_item)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(ClipboardError::from)
}

pub fn get_item_by_id(path: &Path, id: i64) -> Result<ClipboardItem, ClipboardError> {
    let connection = Connection::open(path)?;
    get_item_by_id_with_connection(&connection, id)
}

pub fn soft_delete_item(path: &Path, id: i64, now: &str) -> Result<(), ClipboardError> {
    let connection = Connection::open(path)?;
    let changed = connection.execute(
        "UPDATE clipboard_items SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;

    if changed == 0 {
        return Err(ClipboardError::NotFound(id));
    }

    Ok(())
}

fn find_active_id_by_hash(
    connection: &Connection,
    content_hash: &str,
) -> Result<Option<i64>, ClipboardError> {
    connection
        .query_row(
            "SELECT id FROM clipboard_items WHERE content_hash = ?1 AND deleted_at IS NULL",
            [content_hash],
            |row| row.get(0),
        )
        .optional()
        .map_err(ClipboardError::from)
}

fn get_item_by_id_with_connection(
    connection: &Connection,
    id: i64,
) -> Result<ClipboardItem, ClipboardError> {
    let item = connection
        .query_row(
            "SELECT id, content_type, content, preview, content_hash, created_at, last_copied_at, copy_count
             FROM clipboard_items
             WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_item,
        )
        .optional()?;

    item.ok_or(ClipboardError::NotFound(id))
}

fn map_item(row: &Row<'_>) -> rusqlite::Result<ClipboardItem> {
    Ok(ClipboardItem {
        id: row.get(0)?,
        content_type: row.get(1)?,
        content: row.get(2)?,
        preview: row.get(3)?,
        content_hash: row.get(4)?,
        created_at: row.get(5)?,
        last_copied_at: row.get(6)?,
        copy_count: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        get_item_by_id, init_database, list_date_groups, list_items_by_date, soft_delete_item,
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
}
