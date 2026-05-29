use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use super::models::{
    CaptureOutcome, ClipboardChangeEvent, ClipboardMonitorErrorEvent, ClipboardSkipReason,
    ClipboardSkippedEvent,
};
use super::service::ClipboardService;

const POLL_INTERVAL: Duration = Duration::from_millis(800);

#[derive(Debug, PartialEq)]
pub(crate) enum MonitorSignal {
    Quiet,
    Failing(String),
    Recovered,
}

pub(crate) fn monitor_signal(last_healthy: bool, capture_err: Option<&str>) -> MonitorSignal {
    match (last_healthy, capture_err) {
        (true, Some(msg)) => MonitorSignal::Failing(msg.to_string()),
        (false, None) => MonitorSignal::Recovered,
        _ => MonitorSignal::Quiet,
    }
}

pub fn start_clipboard_monitor(app_handle: AppHandle, service: Arc<ClipboardService>) {
    thread::spawn(move || {
        let mut last_poll_healthy = true;
        loop {
            let capture_err = match service.capture_current_clipboard() {
                Ok(CaptureOutcome::Item(item)) => {
                    let event_name = if item.copy_count > 1 {
                        "clipboard:item-updated"
                    } else {
                        "clipboard:item-created"
                    };
                    let event = ClipboardChangeEvent { item };
                    if let Err(error) = app_handle.emit(event_name, event) {
                        log::warn!("failed to emit clipboard event: {error}");
                    }
                    None
                }
                Ok(CaptureOutcome::Skipped {
                    reason,
                    content_length,
                    max_text_length,
                }) => {
                    emit_skip_event(&app_handle, reason, content_length, max_text_length);
                    None
                }
                Err(error) => Some(error.to_string()),
            };

            match monitor_signal(last_poll_healthy, capture_err.as_deref()) {
                MonitorSignal::Failing(message) => {
                    log::error!("clipboard monitor error: {message}");
                    emit_monitor_error(&app_handle, true, Some(message));
                }
                MonitorSignal::Recovered => {
                    log::info!("clipboard monitor recovered");
                    emit_monitor_error(&app_handle, false, None);
                }
                MonitorSignal::Quiet => {}
            }

            last_poll_healthy = capture_err.is_none();
            thread::sleep(POLL_INTERVAL);
        }
    });
}

fn emit_monitor_error(app_handle: &AppHandle, failing: bool, message: Option<String>) {
    let event = ClipboardMonitorErrorEvent { failing, message };
    if let Err(error) = app_handle.emit("clipboard:monitor-error", event) {
        log::warn!("failed to emit clipboard monitor error event: {error}");
    }
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
        log::warn!("failed to emit clipboard skipped event: {error}");
    }
}
