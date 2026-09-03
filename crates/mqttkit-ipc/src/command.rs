//! 前端 → 后端 的命令（对应 Tauri `#[tauri::command]`）。
//!
//! 命令层只做参数透传与契约转换，不含业务逻辑。所有 `payload` 字段为 base64 字符串。
use crate::model::{Connection, AppSettings, Qos, StoredMessage};
use serde::{Deserialize, Serialize};

/// 连接控制。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// 建立连接
    Connect { connection_id: String },
    /// 主动断开
    Disconnect { connection_id: String },
    /// 订阅
    Subscribe {
        connection_id: String,
        filter: String,
        qos: Qos,
    },
    /// 取消订阅
    Unsubscribe { connection_id: String, filter: String },
    /// 发布消息
    Publish {
        connection_id: String,
        topic: String,
        payload_base64: String,
        qos: Qos,
        retain: bool,
    },
    /// 拉取最近 N 条消息快照（用于切换连接/重连后回填）
    GetMessages {
        connection_id: String,
        limit: usize,
    },
    /// 清空某连接消息缓冲
    ClearMessages { connection_id: String },
    /// 读取全部连接概要
    ListConnections,
    /// 读取单条连接配置
    GetConnection { id: String },
    /// 保存（新增或更新）连接配置
    SaveConnection { connection: Connection },
    /// 删除连接配置
    DeleteConnection { id: String },
    /// 读取应用配置
    GetSettings,
    /// 保存应用配置
    SaveSettings { settings: AppSettings },
}

/// 命令返回：统一用 `Result<CommandResult, String>` 透传错误字符串。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResult {
    Ok,
    ConnectionList { connections: Vec<Connection> },
    Connection { connection: Connection },
    Messages { messages: Vec<StoredMessage> },
    Settings { settings: AppSettings },
}

/// 保存连接时附带的明文凭据（仅用于写入 Keychain，不在返回里出现）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialInput {
    pub connection_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}
