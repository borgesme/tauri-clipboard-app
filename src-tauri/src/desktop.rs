use tauri::{
    menu::{Menu, MenuBuilder, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Emitter, LogicalSize, Manager, WindowEvent,
};

use crate::clipboard::error::ClipboardError;

const MENU_SHOW: &str = "show";
const MENU_HIDE: &str = "hide";
const MENU_QUIT: &str = "quit";
const MENU_SYSTEM_RESTORE: &str = "system_restore";
const MENU_SYSTEM_MOVE: &str = "system_move";
const MENU_SYSTEM_SIZE: &str = "system_size";
const MENU_OPEN_SETTINGS: &str = "open_settings";
const EVENT_OPEN_SETTINGS: &str = "app:open-settings";
// 与 tauri.conf.json `app.windows[0].width/height` 保持一致。
const DEFAULT_WINDOW_WIDTH: f64 = 960.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 640.0;

pub fn setup_desktop(app: &mut App) -> tauri::Result<()> {
    setup_close_to_tray(app);
    setup_app_menu(app)?;
    setup_tray(app)
}

pub fn show_main_window(app_handle: &AppHandle) -> Result<(), ClipboardError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .show()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
        window
            .set_focus()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
    }
    Ok(())
}

pub fn hide_main_window(app_handle: &AppHandle) -> Result<(), ClipboardError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .hide()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
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

fn setup_app_menu(app: &mut App) -> tauri::Result<()> {
    let menu = create_app_menu(app)?;
    app.set_menu(menu)?;
    app.on_menu_event(handle_app_menu);
    Ok(())
}

fn create_app_menu(app: &App) -> tauri::Result<Menu<tauri::Wry>> {
    let restore = MenuItem::with_id(app, MENU_SYSTEM_RESTORE, "还原", true, None::<&str>)?;
    let move_window = MenuItem::with_id(app, MENU_SYSTEM_MOVE, "移动", true, None::<&str>)?;
    let resize_window =
        MenuItem::with_id(app, MENU_SYSTEM_SIZE, "恢复默认大小", true, None::<&str>)?;
    let system_menu = Submenu::with_items(
        app,
        "系统",
        true,
        &[
            &restore,
            &move_window,
            &resize_window,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::minimize(app, Some("最小化"))?,
            &PredefinedMenuItem::maximize(app, Some("最大化"))?,
            &PredefinedMenuItem::close_window(app, Some("关闭"))?,
        ],
    )?;
    let settings = MenuItem::with_id(app, MENU_OPEN_SETTINGS, "打开设置", true, None::<&str>)?;
    let settings_menu = Submenu::with_items(app, "设置", true, &[&settings])?;
    Menu::with_items(app, &[&system_menu, &settings_menu])
}

fn handle_app_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        MENU_SYSTEM_RESTORE => handle_result(restore_main_window(app)),
        MENU_SYSTEM_MOVE => handle_result(move_main_window(app)),
        MENU_SYSTEM_SIZE => handle_result(resize_main_window(app)),
        MENU_OPEN_SETTINGS => handle_result(open_settings(app)),
        _ => {}
    }
}

fn restore_main_window(app_handle: &AppHandle) -> Result<(), ClipboardError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .show()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
        window
            .unmaximize()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
        window
            .set_focus()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
    }
    Ok(())
}

fn move_main_window(app_handle: &AppHandle) -> Result<(), ClipboardError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .show()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
        window
            .start_dragging()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
    }
    Ok(())
}

fn resize_main_window(app_handle: &AppHandle) -> Result<(), ClipboardError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        let size = LogicalSize::new(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT);
        window
            .set_size(size)
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
        window
            .set_focus()
            .map_err(|error| ClipboardError::Runtime(error.to_string()))?;
    }
    Ok(())
}

fn open_settings(app_handle: &AppHandle) -> Result<(), ClipboardError> {
    show_main_window(app_handle)?;
    app_handle
        .emit(EVENT_OPEN_SETTINGS, ())
        .map_err(|error| ClipboardError::Runtime(error.to_string()))
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
