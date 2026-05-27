use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use super::models::{
    CaptureOutcome, ClipboardChangeEvent, ClipboardSkipReason, ClipboardSkippedEvent,
};
use super::service::ClipboardService;

const POLL_INTERVAL: Duration = Duration::from_millis(800);

pub fn start_clipboard_monitor(app_handle: AppHandle, service: Arc<ClipboardService>) {
    thread::spawn(move || loop {
        match service.capture_current_clipboard() {
            Ok(CaptureOutcome::Item(item)) => {
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
            Ok(CaptureOutcome::Skipped {
                reason,
                content_length,
                max_text_length,
            }) => emit_skip_event(&app_handle, reason, content_length, max_text_length),
            Err(error) => eprintln!("clipboard monitor error: {error}"),
        }

        thread::sleep(POLL_INTERVAL);
    });
}

fn emit_skip_event(
    app_handle: &AppHandle,
    reason: ClipboardSkipReason,
    content_length: i64,
    max_text_length: i64,
) {
    if !matches!(
        reason,
        ClipboardSkipReason::TooLong | ClipboardSkipReason::SecretLike
    ) {
        return;
    }
    let event = ClipboardSkippedEvent {
        reason,
        content_length,
        max_text_length,
    };
    if let Err(error) = app_handle.emit("clipboard:item-skipped", event) {
        eprintln!("failed to emit clipboard skipped event: {error}");
    }
}
