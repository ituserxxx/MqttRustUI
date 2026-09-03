//! mqttkit-core —— MQTT 客户端核心业务逻辑。
//!
//! **硬约束：本 crate 不得依赖 Tauri**，也不得让 rumqttc 的类型泄漏到
//! 对外的公共接口。核心通过 [`conn::ConnectionManager`] 暴露事件回调，
//! 上层（桌面端 Tauri 命令层 / 未来的 Web Worker）只消费 `mqttkit-ipc` 定义
//! 的 DTO。这样核心可以脱离 GUI 跑纯 Rust 单测与端到端集成测试。
pub mod buffer;
pub mod codec;
pub mod conn;
pub mod protocol;
pub mod script;
pub mod session;
pub mod stats;
pub mod topic;
