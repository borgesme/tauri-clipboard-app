use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use super::error::ClipboardError;
use super::models::{
    ClipboardDateGroup, ClipboardDeletedEvent, ClipboardItem, ClipboardMonitorStatus,
};
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
pub fn search_clipboard_items(
    keyword: String,
    state: State<'_, ClipboardState>,
) -> Result<Vec<ClipboardItem>, ClipboardError> {
    state.0.search_items(&keyword)
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
    app_handle: AppHandle,
    id: i64,
    state: State<'_, ClipboardState>,
) -> Result<(), ClipboardError> {
    state.0.delete_item(id)?;
    emit_deleted(&app_handle, Some(id), None);
    Ok(())
}

#[tauri::command]
pub fn clear_clipboard_items_by_date(
    app_handle: AppHandle,
    date: String,
    state: State<'_, ClipboardState>,
) -> Result<(), ClipboardError> {
    state.0.clear_items_by_date(&date)?;
    emit_deleted(&app_handle, None, Some(date));
    Ok(())
}

#[tauri::command]
pub fn set_clipboard_monitor_enabled(
    app_handle: AppHandle,
    enabled: bool,
    state: State<'_, ClipboardState>,
) -> Result<ClipboardMonitorStatus, ClipboardError> {
    let status = state.0.set_monitor_enabled(enabled)?;
    emit_monitor_status(&app_handle, status.clone());
    Ok(status)
}

#[tauri::command]
pub fn get_clipboard_monitor_status(
    state: State<'_, ClipboardState>,
) -> Result<ClipboardMonitorStatus, ClipboardError> {
    state.0.monitor_status()
}

fn emit_deleted(app_handle: &AppHandle, id: Option<i64>, date: Option<String>) {
    let event = ClipboardDeletedEvent { id, date };
    if let Err(error) = app_handle.emit("clipboard:item-deleted", event) {
        eprintln!("failed to emit clipboard deleted event: {error}");
    }
}

fn emit_monitor_status(app_handle: &AppHandle, status: ClipboardMonitorStatus) {
    if let Err(error) = app_handle.emit("clipboard:monitor-status-changed", status) {
        eprintln!("failed to emit clipboard monitor status event: {error}");
    }
}
