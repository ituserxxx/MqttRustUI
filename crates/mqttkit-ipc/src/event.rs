//! 后端 → 前端 的事件（对应 Tauri `emit`）。
//!
//! 高频消息走 `MessageBatch`（批量 flush），不在每条到达时单独 emit——
//! 这是避免打爆 IPC 通道与 WebView 主线程的关键。事件名统一以 `mqttkit:` 前缀。
use crate::model::{ConnectionState, StoredMessage, TrafficStats};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    /// 连接状态变化
    StateChanged {
        connection_id: String,
        state: ConnectionState,
        /// 人类可读细节（错误原因等），不含凭据
        detail: Option<String>,
    },
    /// 批量消息到达（双阈值 flush：50ms / 200 条）
    MessageBatch {
        connection_id: String,
        messages: Vec<StoredMessage>,
    },
    /// 流量统计刷新
    Stats {
        connection_id: String,
        stats: TrafficStats,
    },
    /// 背压降级开关（true=已降级为只推统计）
    Backpressure {
        connection_id: String,
        degraded: bool,
    },
}

/// 各事件对应的 Tauri 事件名。
pub const EVT_STATE: &str = "mqttkit:state";
pub const EVT_BATCH: &str = "mqttkit:message_batch";
pub const EVT_STATS: &str = "mqttkit:stats";
pub const EVT_BACKPRESSURE: &str = "mqttkit:backpressure";
