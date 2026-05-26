use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, WindowEvent,
};

use crate::clipboard::error::ClipboardError;

const MENU_SHOW: &str = "show";
const MENU_HIDE: &str = "hide";
const MENU_QUIT: &str = "quit";

pub fn setup_desktop(app: &mut App) -> tauri::Result<()> {
    setup_close_to_tray(app);
    setup_tray(app)
}

pub fn show_main_window(app_handle: &AppHandle) -> Result<(), ClipboardError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.show().map_err(|error| ClipboardError::Runtime(error.to_string()))?;
        window.set_focus().map_err(|error| ClipboardError::Runtime(error.to_string()))?;
    }
    Ok(())
}

pub fn hide_main_window(app_handle: &AppHandle) -> Result<(), ClipboardError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide().map_err(|error| ClipboardError::Runtime(error.to_string()))?;
    }
    Ok(())
}

fn setup_close_to_tray(app: &mut App) {
    if let Some(window) = app.get_webview_window("main") {
        let close_window = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = close_window.hide() {
                    eprintln!("failed to hide window: {error}");
                }
            }
        });
    }
}

fn setup_tray(app: &mut App) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(MENU_SHOW, "显示窗口")
        .text(MENU_HIDE, "隐藏窗口")
        .text(MENU_QUIT, "退出")
        .build()?;
    let mut builder = TrayIconBuilder::new();
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("剪贴板工具箱")
        .on_menu_event(handle_tray_menu)
        .on_tray_icon_event(handle_tray_click)
        .build(app)?;
    Ok(())
}

fn handle_tray_menu(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        MENU_SHOW => handle_result(show_main_window(app)),
        MENU_HIDE => handle_result(hide_main_window(app)),
        MENU_QUIT => app.exit(0),
        _ => {}
    }
}

fn handle_tray_click(tray: &tauri::tray::TrayIcon, event: TrayIconEvent) {
    if is_left_click_up(&event) {
        handle_result(show_main_window(tray.app_handle()));
    }
}

fn is_left_click_up(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    )
}

fn handle_result(result: Result<(), ClipboardError>) {
    if let Err(error) = result {
        eprintln!("desktop action failed: {error}");
    }
}
