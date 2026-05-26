use std::sync::Arc;

use tauri::State;

use super::error::ClipboardError;
use super::models::{ClipboardDateGroup, ClipboardItem};
use super::service::ClipboardService;

pub struct ClipboardState(pub Arc<ClipboardService>);

#[tauri::command]
pub fn list_clipboard_dates(
    state: State<'_, ClipboardState>,
) -> Result<Vec<ClipboardDateGroup>, ClipboardError> {
    state.0.list_date_groups()
}

#[tauri::command]
pub fn list_clipboard_items(
    date: String,
    state: State<'_, ClipboardState>,
) -> Result<Vec<ClipboardItem>, ClipboardError> {
    state.0.list_items_by_date(&date)
}

#[tauri::command]
pub fn get_clipboard_item(
    id: i64,
    state: State<'_, ClipboardState>,
) -> Result<ClipboardItem, ClipboardError> {
    state.0.get_item(id)
}

#[tauri::command]
pub fn copy_clipboard_item(
    id: i64,
    state: State<'_, ClipboardState>,
) -> Result<(), ClipboardError> {
    state.0.copy_item(id)
}

#[tauri::command]
pub fn delete_clipboard_item(
    id: i64,
    state: State<'_, ClipboardState>,
) -> Result<(), ClipboardError> {
    state.0.delete_item(id)
}
