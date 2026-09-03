//! mqttkit-config —— 配置与凭据持久化（纯 JSON，无数据库）。
//!
//! 设计要点：
//! - 配置只存 `config.json`（原子写），消息历史不落盘。
//! - 凭据真值只存 OS Keychain；配置文件里只保留 `credential_ref` 引用。
//! - 无 Keychain 时降级为内存/加密字段，仍不存明文。
pub mod migrate;
pub mod model;
pub mod persist;
pub mod vault;
