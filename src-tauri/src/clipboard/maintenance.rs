use rusqlite::Connection;

use super::error::ClipboardError;

pub fn purge_deleted_items(connection: &Connection) -> Result<usize, ClipboardError> {
    connection
        .execute(
            "DELETE FROM clipboard_items WHERE deleted_at IS NOT NULL",
            [],
        )
        .map_err(ClipboardError::from)
}

pub fn vacuum_database(connection: &Connection) -> Result<(), ClipboardError> {
    connection.execute_batch("VACUUM")?;
    Ok(())
}
