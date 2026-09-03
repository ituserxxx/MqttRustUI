//! 配置根模型（落盘形状）。
//!
//! 内层结构（Connection / Subscription / TlsConfig / AppSettings 等）复用
//! `mqttkit_ipc::model` —— 它们是 IPC 契约与配置文件的同一份定义。
use mqttkit_ipc::model::{AppSettings, Connection};
use serde::{Deserialize, Serialize};

/// 当前 schema 版本，配合 `migrate.rs` 逐级联迁。
pub const SCHEMA_VERSION: u32 = 1;

/// 一个用户脚本（rhai）。P2 阶段功能，先定义结构。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScriptDef {
    pub id: String,
    pub name: String,
    #[serde(default = "default_rhai")]
    pub lang: String,
    pub source: String,
}

fn default_rhai() -> String {
    "rhai".into()
}

/// 书签（topic 备注）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Bookmark {
    pub topic: String,
    #[serde(default)]
    pub note: String,
}

/// 应用配置根。落盘于 `{app_data_dir}/config.json`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub settings: AppSettings,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub scripts: Vec<ScriptDef>,
    #[serde(default)]
    pub bookmarks: Vec<Bookmark>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            schema_version: SCHEMA_VERSION,
            settings: AppSettings::default(),
            connections: Vec::new(),
            scripts: Vec::new(),
            bookmarks: Vec::new(),
        }
    }
}

impl AppConfig {
    /// 按 id 查找连接。
    pub fn find_connection(&self, id: &str) -> Option<&Connection> {
        self.connections.iter().find(|c| c.id == id)
    }

    /// 按 id 查找可变连接。
    pub fn find_connection_mut(&mut self, id: &str) -> Option<&mut Connection> {
        self.connections.iter_mut().find(|c| c.id == id)
    }

    /// 新增或替换一条连接（按 id 去重）。
    pub fn upsert_connection(&mut self, conn: Connection) {
        if let Some(slot) = self.connections.iter_mut().find(|c| c.id == conn.id) {
            *slot = conn;
        } else {
            self.connections.push(conn);
        }
    }

    /// 删除连接，返回是否真的删到了。
    pub fn remove_connection(&mut self, id: &str) -> bool {
        let before = self.connections.len();
        self.connections.retain(|c| c.id != id);
        before != self.connections.len()
    }
}
