//! Tauri 命令层（薄）：仅做参数透传与契约转换，不含业务逻辑。
use crate::state::AppState;
use mqttkit_config::vault::credential_ref;
use mqttkit_ipc::model::{AppSettings, Connection, Qos, StoredMessage};
use tauri::State;

/// 建立连接（按 id 从配置读取完整参数）。
#[tauri::command]
pub async fn connect(state: State<'_, AppState>, connection_id: String) -> Result<(), String> {
    let cfg = state.config.snapshot().await;
    let conn = cfg
        .find_connection(&connection_id)
        .cloned()
        .ok_or_else(|| format!("连接 {connection_id} 不存在"))?;
    state.manager.connect(conn).await
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>, connection_id: String) -> Result<(), String> {
    state.manager.disconnect(&connection_id).await
}

#[tauri::command]
pub async fn subscribe(
    state: State<'_, AppState>,
    connection_id: String,
    filter: String,
    qos: Qos,
) -> Result<(), String> {
    state.manager.subscribe(&connection_id, filter, qos).await
}

#[tauri::command]
pub async fn unsubscribe(
    state: State<'_, AppState>,
    connection_id: String,
    filter: String,
) -> Result<(), String> {
    state.manager.unsubscribe(&connection_id, &filter).await
}

#[tauri::command]
pub async fn publish(
    state: State<'_, AppState>,
    connection_id: String,
    topic: String,
    payload_base64: String,
    qos: Qos,
    retain: bool,
) -> Result<(), String> {
    state
        .manager
        .publish(&connection_id, &topic, qos, retain, &payload_base64)
        .await
}

#[tauri::command]
pub async fn get_messages(
    state: State<'_, AppState>,
    connection_id: String,
    limit: usize,
) -> Result<Vec<StoredMessage>, String> {
    Ok(state.manager.get_messages(&connection_id, limit).await)
}

#[tauri::command]
pub async fn clear_messages(state: State<'_, AppState>, connection_id: String) -> Result<(), String> {
    state.manager.clear_messages(&connection_id).await;
    Ok(())
}

#[tauri::command]
pub async fn list_connections(state: State<'_, AppState>) -> Result<Vec<Connection>, String> {
    Ok(state.config.snapshot().await.connections)
}

#[tauri::command]
pub async fn get_connection(state: State<'_, AppState>, id: String) -> Result<Connection, String> {
    state
        .config
        .snapshot()
        .await
        .find_connection(&id)
        .cloned()
        .ok_or_else(|| format!("连接 {id} 不存在"))
}

/// 保存连接。`password` 为可选明文，写入 Keychain；配置文件只存 `credential_ref`。
#[tauri::command]
pub async fn save_connection(
    state: State<'_, AppState>,
    connection: Connection,
    password: Option<String>,
) -> Result<(), String> {
    if let Some(pw) = password {
        if !pw.is_empty() {
            state
                .vault
                .set_password(&connection.id, &pw)
                .map_err(|e| e.to_string())?;
        }
    }
    let mut cfg = state.config.snapshot().await;
    let mut conn = connection;
    conn.credential_ref = if conn.username.is_empty() {
        None
    } else {
        Some(credential_ref(&conn.id))
    };
    cfg.upsert_connection(conn);
    state.config.save(cfg).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut cfg = state.config.snapshot().await;
    cfg.remove_connection(&id);
    state.config.save(cfg).await.map_err(|e| e.to_string())?;
    let _ = state.vault.delete_password(&id);
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.config.snapshot().await.settings)
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    let mut cfg = state.config.snapshot().await;
    cfg.settings = settings.clone();
    state.config.save(cfg).await.map_err(|e| e.to_string())?;
    state.manager.update_settings(settings).await;
    Ok(())
}
