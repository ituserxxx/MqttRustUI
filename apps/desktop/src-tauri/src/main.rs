//! MqttRustUI 桌面端装配入口（Tauri 2）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod logging;
mod menu;
mod state;
mod tray;

use std::sync::Arc;
use tauri::Manager;
use mqttkit_config::persist::ConfigStore;
use mqttkit_config::vault::Vault;
use mqttkit_core::conn::ConnectionManager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 二次启动：聚焦已有窗口，并可在 _args 中解析 mqtt:// 深链接
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 日志（WorkerGuard 必须存活到进程退出，这里 forget 以保活）
            let log_dir = app
                .path()
                .app_log_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let guard = logging::init("info", &log_dir);
            std::mem::forget(guard);

            // 配置存储（原子写 + 损坏兜底）
            let config_path = app
                .path()
                .app_config_dir()
                .map(|p| p.join("config.json"))
                .unwrap_or_else(|_| std::path::PathBuf::from("config.json"));
            let store = Arc::new(
                tauri::async_runtime::block_on(ConfigStore::load_or_default(config_path))
                    .expect("加载配置失败"),
            );
            store.spawn_debounced_flush();

            // 凭据保险库
            let vault = Arc::new(Vault::new());

            // 核心管理器 + 事件出口
            let settings = tauri::async_runtime::block_on(store.snapshot()).settings;
            let sink = Arc::new(state::TauriSink {
                handle: app.handle().clone(),
            });
            let manager = ConnectionManager::new(vault.clone(), sink, settings);
            // flush 循环运行在 Tauri 的 tokio 运行时里（core 不依赖 Tauri，故在此启动）
            tauri::async_runtime::spawn(manager.clone().flush_loop());

            app.manage(state::AppState {
                config: store.clone(),
                vault,
                manager: manager.clone(),
            });

            // 自动连接标记为 auto_connect 的连接
            let autoconn = tauri::async_runtime::block_on(store.snapshot())
                .connections
                .iter()
                .filter(|c| c.auto_connect)
                .cloned()
                .collect::<Vec<_>>();
            for c in autoconn {
                let m = manager.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = m.connect(c).await {
                        tracing::warn!(target: "autoconn", "自动连接失败: {e}");
                    }
                });
            }

            // 菜单 + 托盘
            let menu = menu::build(app.handle()).expect("菜单构建失败");
            let _ = app.set_menu(menu.clone());
            tray::build(app.handle(), menu).ok();

            Ok(())
        })
        .on_menu_event(|app, event| menu::handle_menu_event(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::disconnect,
            commands::subscribe,
            commands::unsubscribe,
            commands::publish,
            commands::get_messages,
            commands::clear_messages,
            commands::list_connections,
            commands::get_connection,
            commands::save_connection,
            commands::delete_connection,
            commands::get_settings,
            commands::save_settings,
            commands::export_config,
            commands::import_config,
            commands::disconnect_all
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
