use std::sync::Arc;

use clipboard::commands::{
    copy_clipboard_item, delete_clipboard_item, get_clipboard_item, list_clipboard_dates,
    list_clipboard_items, ClipboardState,
};
use clipboard::monitor::start_clipboard_monitor;
use clipboard::service::ClipboardService;
use tauri::Manager;

mod clipboard;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let database_path = data_dir.join("clipboard.sqlite");
            let service = Arc::new(ClipboardService::new(database_path)?);

            start_clipboard_monitor(app.handle().clone(), Arc::clone(&service));
            app.manage(ClipboardState(service));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_clipboard_dates,
            list_clipboard_items,
            get_clipboard_item,
            copy_clipboard_item,
            delete_clipboard_item
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
