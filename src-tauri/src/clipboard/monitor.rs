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
                let event = ClipboardChangeEvent { item };
                if let Err(error) = app_handle.emit("clipboard:item-created", event) {
                    eprintln!("failed to emit clipboard event: {error}");
                }
            }
            Ok(None) => {}
            Err(error) => eprintln!("clipboard monitor error: {error}"),
        }

        thread::sleep(POLL_INTERVAL);
    });
}
