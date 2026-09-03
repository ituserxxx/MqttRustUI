//! mqttkit-ipc —— 前后端 IPC 契约 DTO。
//!
//! 该 crate 只定义数据结构，不依赖 Tauri，也不依赖任何平台 API，
//! 因此 Rust 桌面端、未来的 Web 端、甚至 CLI 都能共用同一套契约。
pub mod command;
pub mod event;
pub mod model;
