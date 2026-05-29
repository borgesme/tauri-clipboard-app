use rusqlite::{params, Connection, OptionalExtension, Row};

use super::error::ClipboardError;
use super::hash::preview;
use super::models::{ClipboardDateGroup, ClipboardItem};

const CONTENT_TYPE_TEXT: &str = "text";

pub fn init_schema(connection: &Connection) -> Result<(), ClipboardError> {
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
            deleted_at TEXT,
            local_date TEXT
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_clipboard_items_hash_active
            ON clipboard_items(content_hash)
            WHERE deleted_at IS NULL;
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_local_date_active
            ON clipboard_items(local_date)
            WHERE deleted_at IS NULL;
        DROP INDEX IF EXISTS idx_clipboard_items_created_at_active;
        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );",
    )?;
    Ok(())
}

pub fn upsert_text_item(
    connection: &Connection,
    content: &str,
    content_hash: &str,
    now: &str,
) -> Result<ClipboardItem, ClipboardError> {
    if let Some(id) = find_active_id_by_hash(connection, content_hash)? {
        update_existing_item(connection, id, now)?;
        return get_item_by_id_with_connection(connection, id);
    }
    insert_text_item(connection, content, content_hash, now)?;
    get_item_by_id_with_connection(connection, connection.last_insert_rowid())
}

pub fn list_date_groups(
    connection: &Connection,
) -> Result<Vec<ClipboardDateGroup>, ClipboardError> {
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
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ClipboardError::from)
}

pub fn list_items_by_date(
    connection: &Connection,
    date: &str,
) -> Result<Vec<ClipboardItem>, ClipboardError> {
    let mut statement = connection.prepare(
        "SELECT id, content_type, content, preview, content_hash, created_at, last_copied_at, copy_count
         FROM clipboard_items
         WHERE deleted_at IS NULL AND substr(created_at, 1, 10) = ?1
         ORDER BY last_copied_at DESC, id DESC",
    )?;
    let rows = statement.query_map([date], map_item)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ClipboardError::from)
}

pub fn search_items(
    connection: &Connection,
    keyword: &str,
) -> Result<Vec<ClipboardItem>, ClipboardError> {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = format!("%{trimmed}%");
    let mut statement = connection.prepare(
        "SELECT id, content_type, content, preview, content_hash, created_at, last_copied_at, copy_count
         FROM clipboard_items
         WHERE deleted_at IS NULL AND (content LIKE ?1 OR preview LIKE ?1)
         ORDER BY last_copied_at DESC, id DESC",
    )?;
    let rows = statement.query_map([pattern], map_item)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ClipboardError::from)
}

pub fn get_item_by_id(
    connection: &Connection,
    id: i64,
) -> Result<ClipboardItem, ClipboardError> {
    get_item_by_id_with_connection(connection, id)
}

pub fn soft_delete_item(
    connection: &Connection,
    id: i64,
    now: &str,
) -> Result<(), ClipboardError> {
    let changed = connection.execute(
        "UPDATE clipboard_items SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    if changed == 0 {
        return Err(ClipboardError::NotFound(id));
    }
    Ok(())
}

pub fn soft_delete_items_by_date(
    connection: &Connection,
    date: &str,
    now: &str,
) -> Result<usize, ClipboardError> {
    let changed = connection.execute(
        "UPDATE clipboard_items
         SET deleted_at = ?1
         WHERE substr(created_at, 1, 10) = ?2 AND deleted_at IS NULL",
        params![now, date],
    )?;
    Ok(changed)
}

pub fn get_i64_setting(
    connection: &Connection,
    key: &str,
    default: i64,
) -> Result<i64, ClipboardError> {
    let value = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value
        .and_then(|text: String| text.parse().ok())
        .unwrap_or(default))
}

pub fn get_string_setting(
    connection: &Connection,
    key: &str,
    default: &str,
) -> Result<String, ClipboardError> {
    let value = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.unwrap_or_else(|| default.to_string()))
}

pub fn set_setting(
    connection: &Connection,
    key: &str,
    value: &str,
    now: &str,
) -> Result<(), ClipboardError> {
    connection.execute(
        "INSERT INTO app_settings (key, value, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now],
    )?;
    Ok(())
}

pub fn cleanup_items(
    connection: &Connection,
    cutoff_date: &str,
    max_record_count: i64,
    now: &str,
) -> Result<usize, ClipboardError> {
    let by_date = cleanup_by_date(connection, cutoff_date, now)?;
    let by_count = cleanup_by_count(connection, max_record_count, now)?;
    Ok(by_date + by_count)
}

fn update_existing_item(connection: &Connection, id: i64, now: &str) -> Result<(), ClipboardError> {
    connection.execute(
        "UPDATE clipboard_items
         SET last_copied_at = ?1, copy_count = copy_count + 1
         WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    Ok(())
}

fn insert_text_item(
    connection: &Connection,
    content: &str,
    content_hash: &str,
    now: &str,
) -> Result<(), ClipboardError> {
    connection.execute(
        "INSERT INTO clipboard_items
         (content_type, content, preview, content_hash, created_at, last_copied_at, copy_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        params![
            CONTENT_TYPE_TEXT,
            content,
            preview(content),
            content_hash,
            now,
            now
        ],
    )?;
    Ok(())
}

fn cleanup_by_date(
    connection: &Connection,
    cutoff_date: &str,
    now: &str,
) -> Result<usize, ClipboardError> {
    connection
        .execute(
            "UPDATE clipboard_items
             SET deleted_at = ?1
             WHERE deleted_at IS NULL AND substr(created_at, 1, 10) < ?2",
            params![now, cutoff_date],
        )
        .map_err(ClipboardError::from)
}

fn cleanup_by_count(
    connection: &Connection,
    max_record_count: i64,
    now: &str,
) -> Result<usize, ClipboardError> {
    connection
        .execute(
            "UPDATE clipboard_items
             SET deleted_at = ?1
             WHERE id IN (
                SELECT id FROM clipboard_items
                WHERE deleted_at IS NULL
                ORDER BY last_copied_at DESC, id DESC
                LIMIT -1 OFFSET ?2
             )",
            params![now, max_record_count],
        )
        .map_err(ClipboardError::from)
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
