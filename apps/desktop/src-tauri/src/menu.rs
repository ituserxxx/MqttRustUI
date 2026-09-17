//! 原生菜单（三平台标准结构）。
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Wry};

/// 构建菜单。返回 `Menu<Wry>`。
pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let new_conn = MenuItem::with_id(app, "new_conn", "新建连接", true, None::<&str>)?;
    let import = MenuItem::with_id(app, "import_cfg", "导入配置…", true, None::<&str>)?;
    let export = MenuItem::with_id(app, "export_cfg", "导出配置…", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some("退出"))?;
    let file = Submenu::with_items(app, "文件", true, &[&new_conn, &sep, &import, &export, &quit])?;

    let copy = PredefinedMenuItem::copy(app, Some("复制"))?;
    let paste = PredefinedMenuItem::paste(app, Some("粘贴"))?;
    let edit = Submenu::with_items(app, "编辑", true, &[&copy, &paste])?;

    let reload = MenuItem::with_id(app, "reload", "重新加载", true, None::<&str>)?;
    let view = Submenu::with_items(app, "视图", true, &[&reload])?;

    let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let about = PredefinedMenuItem::about(app, Some("关于 MqttRustUI"), None)?;
    let help = Submenu::with_items(app, "帮助", true, &[&settings, &about])?;

    Menu::with_items(app, &[&file, &edit, &view, &help])
}

/// 菜单事件统一处理（窗口菜单与托盘菜单共用）。
pub fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "new_conn" => {
            let _ = app.emit("menu:new_conn", ());
        }
        "import_cfg" => {
            let _ = app.emit("menu:import_cfg", ());
        }
        "export_cfg" => {
            let _ = app.emit("menu:export_cfg", ());
        }
        "settings" => {
            let _ = app.emit("menu:settings", ());
        }
        "reload" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval("location.reload()");
            }
        }
        _ => {}
    }
}
