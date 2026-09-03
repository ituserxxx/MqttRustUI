//! 系统托盘：显示连接状态角标（占位为应用图标），菜单含主窗口/退出。
use crate::menu;
use tauri::menu::Menu;
use tauri::{AppHandle, Manager};

/// 构建托盘图标（使用窗口默认图标作为托盘图）。
pub fn build(app: &AppHandle, menu: Menu) -> tauri::Result<()> {
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("缺少默认窗口图标（请检查 icons 配置）");
    let _tray = tauri::tray::TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("MqttRustUI")
        .on_menu_event(|app, event| menu::handle_menu_event(app, event.id().as_ref()))
        .build(app)?;
    Ok(())
}
