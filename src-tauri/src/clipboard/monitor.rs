use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use super::models::ClipboardChangeEvent;
use super::service::ClipboardService;

const POLL_INTERVAL: Duration = Duration::from_millis(800);

pub fn start_clipboard_monitor(app_handle: AppHandle, service: Arc<ClipboardService>) {
    thread::spawn(move || loop {
        match service.capture_current_clipboard() {
            Ok(Some(item)) => {
                let event_name = if item.copy_count > 1 {
                    "clipboard:item-updated"
                } else {
                    "clipboard:item-created"
                };
                let event = ClipboardChangeEvent { item };
                if let Err(error) = app_handle.emit(event_name, event) {
                    eprintln!("failed to emit clipboard event: {error}");
                }
            }
            Ok(None) => {}
            Err(error) => eprintln!("clipboard monitor error: {error}"),
        }

        thread::sleep(POLL_INTERVAL);
    });
}
