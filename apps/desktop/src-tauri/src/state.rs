//! 应用状态与事件出口。
//!
//! `AppState` 被 Tauri `manage`，命令层通过 `State` 取出。
//! `TauriSink` 把 core 的事件转成 Tauri `emit`，是 core 与 GUI 之间唯一的桥。
use std::sync::Arc;
use mqttkit_config::persist::ConfigStore;
use mqttkit_config::vault::Vault;
use mqttkit_core::conn::{ConnectionManager, EventSink};
use mqttkit_ipc::event::{AppEvent, EVT_BACKPRESSURE, EVT_BATCH, EVT_STATE, EVT_STATS};
use tauri::{AppHandle, Emitter};

/// 托管在 Tauri 中的共享状态。
pub struct AppState {
    pub config: Arc<ConfigStore>,
    pub vault: Arc<Vault>,
    pub manager: Arc<ConnectionManager>,
}

/// 事件出口：core → Tauri emit。
pub struct TauriSink {
    pub handle: AppHandle,
}

impl EventSink for TauriSink {
    fn emit(&self, event: AppEvent) {
        let name = match &event {
            AppEvent::StateChanged { .. } => EVT_STATE,
            AppEvent::MessageBatch { .. } => EVT_BATCH,
            AppEvent::Stats { .. } => EVT_STATS,
            AppEvent::Backpressure { .. } => EVT_BACKPRESSURE,
        };
        // 高频消息走 batch 通道；emit 失败（窗口未就绪）静默忽略
        if let Err(e) = self.handle.emit(name, event) {
            tracing::trace!(target: "sink", "emit 失败（可忽略）: {e}");
        }
    }
}
