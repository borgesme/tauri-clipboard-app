use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;

use crate::desktop::{hide_main_window as hide_window, show_main_window as show_window};

use super::error::ClipboardError;
use super::models::{
    ClipboardDateGroup, ClipboardDeletedEvent, ClipboardItem, ClipboardMonitorStatus,
    DesktopSettings, DesktopSettingsUpdate,
};
use super::service::ClipboardService;
use super::settings;

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
) -> Result<Vec<i64>, ClipboardError> {
    let ids = state.0.clear_items_by_date(&date)?;
    emit_deleted(&app_handle, None, Some(date));
    Ok(ids)
}

#[tauri::command]
pub fn restore_clipboard_items(
    ids: Vec<i64>,
    state: State<'_, ClipboardState>,
) -> Result<usize, ClipboardError> {
    state.0.restore_items(&ids)
}

#[tauri::command]
pub fn purge_deleted_clipboard_items(
    app_handle: AppHandle,
    vacuum: bool,
    state: State<'_, ClipboardState>,
) -> Result<usize, ClipboardError> {
    let removed = state.0.purge_deleted_items(vacuum)?;
    emit_deleted(&app_handle, None, None);
    Ok(removed)
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

#[tauri::command]
pub fn get_desktop_settings(
    app_handle: AppHandle,
    state: State<'_, ClipboardState>,
) -> Result<DesktopSettings, ClipboardError> {
    let autostart_enabled = autostart_enabled(&app_handle)?;
    state.0.desktop_settings(autostart_enabled)
}

#[tauri::command]
pub fn validate_storage_dir(storage_dir: String) -> Result<(), ClipboardError> {
    settings::validate_storage_dir(&storage_dir)
}

#[tauri::command]
pub fn update_desktop_settings(
    app_handle: AppHandle,
    settings: DesktopSettingsUpdate,
    state: State<'_, ClipboardState>,
) -> Result<DesktopSettings, ClipboardError> {
    let current_autostart_enabled = autostart_enabled(&app_handle)?;
    if let Some(enabled) = autostart_change(current_autostart_enabled, settings.autostart_enabled) {
        set_autostart_enabled(&app_handle, enabled)?;
    }
    let autostart_enabled = autostart_enabled(&app_handle)?;
    state.0.update_desktop_settings(settings, autostart_enabled)
}

#[tauri::command]
pub fn hide_main_window(app_handle: AppHandle) -> Result<(), ClipboardError> {
    hide_window(&app_handle)
}

#[tauri::command]
pub fn show_main_window(app_handle: AppHandle) -> Result<(), ClipboardError> {
    show_window(&app_handle)
}

fn autostart_enabled(app_handle: &AppHandle) -> Result<bool, ClipboardError> {
    app_handle
        .autolaunch()
        .is_enabled()
        .map_err(|error| ClipboardError::Runtime(error.to_string()))
}

fn set_autostart_enabled(app_handle: &AppHandle, enabled: bool) -> Result<(), ClipboardError> {
    let autostart = app_handle.autolaunch();
    let result = if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    };
    result.map_err(|error| ClipboardError::Runtime(error.to_string()))
}

fn autostart_change(current: bool, requested: bool) -> Option<bool> {
    if current == requested {
        return None;
    }
    Some(requested)
}

fn emit_deleted(app_handle: &AppHandle, id: Option<i64>, date: Option<String>) {
    let event = ClipboardDeletedEvent { id, date };
    if let Err(error) = app_handle.emit("clipboard:item-deleted", event) {
        log::warn!("failed to emit clipboard deleted event: {error}");
    }
}

fn emit_monitor_status(app_handle: &AppHandle, status: ClipboardMonitorStatus) {
    if let Err(error) = app_handle.emit("clipboard:monitor-status-changed", status) {
        log::warn!("failed to emit clipboard monitor status event: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::autostart_change;

    #[test]
    fn autostart_change_is_skipped_when_state_is_unchanged() {
        assert_eq!(None, autostart_change(false, false));
        assert_eq!(None, autostart_change(true, true));
    }

    #[test]
    fn autostart_change_is_requested_only_when_state_changes() {
        assert_eq!(Some(true), autostart_change(false, true));
        assert_eq!(Some(false), autostart_change(true, false));
    }
}
