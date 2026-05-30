use std::sync::Arc;

use clipboard::commands::{
    clear_clipboard_items_by_date, copy_clipboard_item, delete_clipboard_item, get_clipboard_item,
    get_clipboard_monitor_status, get_desktop_settings, hide_main_window, list_clipboard_dates,
    list_clipboard_items, purge_deleted_clipboard_items, restore_clipboard_items,
    search_clipboard_items, set_clipboard_monitor_enabled, show_main_window, update_desktop_settings,
    validate_storage_dir, ClipboardState,
};
use clipboard::monitor::start_clipboard_monitor;
use clipboard::service::ClipboardService;
use desktop::setup_desktop;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

mod clipboard;
mod desktop;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: None,
                    }),
                ])
                .build(),
        )
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let database_path = data_dir.join("clipboard.sqlite");
            let service = Arc::new(ClipboardService::new(database_path)?);

            setup_desktop(app)?;
            start_clipboard_monitor(app.handle().clone(), Arc::clone(&service));
            app.manage(ClipboardState(service));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_clipboard_dates,
            list_clipboard_items,
            search_clipboard_items,
            get_clipboard_item,
            copy_clipboard_item,
            delete_clipboard_item,
            clear_clipboard_items_by_date,
            purge_deleted_clipboard_items,
            restore_clipboard_items,
            set_clipboard_monitor_enabled,
            get_clipboard_monitor_status,
            get_desktop_settings,
            validate_storage_dir,
            update_desktop_settings,
            hide_main_window,
            show_main_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
