//! 系统托盘：tooltip 反映连接状态，菜单含主窗口/全部断开/退出。
use crate::menu;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, Wry};

/// 构建托盘图标（使用窗口默认图标作为托盘图）。
pub fn build(app: &AppHandle, _menu: Menu) -> tauri::Result<()> {
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("缺少默认窗口图标（请检查 icons 配置）");

    // 托盘专用菜单（独立于窗口菜单，避免与原生菜单重复）
    let show = MenuItem::with_id(app, "tray_show_main", "显示主窗口", true, None::<&str>)?;
    let disconnect_all = MenuItem::with_id(app, "tray_disconnect_all", "全部断开", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some("退出"))?;
    let tray_menu: Menu<Wry> = Menu::with_items(app, &[&show, &disconnect_all, &sep, &quit])?;

    let _tray = tauri::tray::TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&tray_menu)
        .show_menu_on_left_click(true)
        .tooltip("MqttRustUI")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_show_main" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "tray_disconnect_all" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(state) = app.try_state::<crate::state::AppState>() {
                        state.manager.disconnect_all().await;
                    }
                });
            }
            id => menu::handle_menu_event(app, id),
        })
        .build(app)?;
    Ok(())
}

