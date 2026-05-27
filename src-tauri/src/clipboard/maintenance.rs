use std::path::Path;

use rusqlite::Connection;

use super::error::ClipboardError;

pub fn purge_deleted_items(path: &Path) -> Result<usize, ClipboardError> {
    let connection = Connection::open(path)?;
    connection
        .execute(
            "DELETE FROM clipboard_items WHERE deleted_at IS NOT NULL",
            [],
        )
        .map_err(ClipboardError::from)
}

pub fn vacuum_database(path: &Path) -> Result<(), ClipboardError> {
    let connection = Connection::open(path)?;
    connection.execute_batch("VACUUM")?;
    Ok(())
}
