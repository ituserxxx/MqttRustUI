//! 原生菜单（三平台标准结构）。
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Wry};

/// 构建菜单。返回 `Menu<Wry>`。
pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let new_conn = MenuItem::with_id(app, "new_conn", "新建连接", true, None::<&str>)?;
    let quit = PredefinedMenuItem::quit(app, Some("退出"))?;
    let sep = PredefinedMenuItem::separator(app)?;
    let file = Submenu::with_items(app, "文件", true, &[&new_conn, &sep, &quit])?;

    let copy = PredefinedMenuItem::copy(app, Some("复制"))?;
    let paste = PredefinedMenuItem::paste(app, Some("粘贴"))?;
    let edit = Submenu::with_items(app, "编辑", true, &[&copy, &paste])?;

    let reload = MenuItem::with_id(app, "reload", "重新加载", true, None::<&str>)?;
    let view = Submenu::with_items(app, "视图", true, &[&reload])?;

    let about = PredefinedMenuItem::about(app, Some("关于 MqttRustUI"), None)?;
    let help = Submenu::with_items(app, "帮助", true, &[&about])?;

    Menu::with_items(app, &[&file, &edit, &view, &help])
}

/// 菜单事件统一处理（窗口菜单与托盘菜单共用）。
pub fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "new_conn" => {
            // 通知前端打开“新建连接”对话框
            let _ = app.emit("menu:new_conn", ());
        }
        "reload" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval("location.reload()");
            }
        }
        _ => {}
    }
}
