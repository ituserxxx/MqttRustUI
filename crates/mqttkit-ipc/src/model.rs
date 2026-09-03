//! 前后端共用的可序列化领域模型。
//!
//! 这些结构体既是 IPC 线上的契约，也是配置文件的落盘形状——两边复用同一份定义。
//! payload 在网络与磁盘上统一以 base64 字符串表示，避免 `Vec<u8>` 被序列化成
//! 巨型数字数组。文本预览单独保留，前端列表默认只渲染预览，避免大 payload 上屏。
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// MQTT 协议版本。两端（桌面/未来 Web）统一用这个枚举，不泄漏 rumqttc 类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolVersion {
    /// MQTT 3.1.1
    V311,
    /// MQTT 5.0
    V50,
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        ProtocolVersion::V311
    }
}

/// 服务质量等级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qos {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}

impl Qos {
    pub fn level(&self) -> u8 {
        *self as u8
    }
}

impl Default for Qos {
    fn default() -> Self {
        Qos::AtLeastOnce
    }
}

/// TLS 模式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode {
    /// 明文（也可用于 ws://）
    None,
    /// 单向/双向 TLS
    Tls,
}

impl Default for TlsMode {
    fn default() -> Self {
        TlsMode::None
    }
}

/// 客户端证书认证（双向 TLS）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClientAuth {
    /// 客户端证书 PEM 路径
    pub cert: String,
    /// 客户端私钥 PEM 路径
    pub key: String,
}

/// TLS 配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TlsConfig {
    #[serde(default)]
    pub mode: TlsMode,
    /// 自定义 CA 证书 PEM 路径（None 用系统根证书）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca: Option<String>,
    /// 双向 TLS 客户端认证
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_auth: Option<ClientAuth>,
    /// 是否校验主机名
    #[serde(default = "default_true")]
    pub verify_hostname: bool,
}

fn default_true() -> bool {
    true
}

/// HTTP/ SOCKS 代理。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    pub url: String,
}

/// 遗嘱消息（Last Will and Testament）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LastWill {
    pub topic: String,
    /// payload base64
    pub payload_base64: String,
    #[serde(default)]
    pub qos: Qos,
    #[serde(default)]
    pub retain: bool,
}

/// MQTT 5 用户属性（User Properties）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Mqtt5Props {
    /// 会话过期时间（秒），0 = 不保留
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_expiry_interval: Option<u32>,
    /// 接收最大（字节）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_packet_size: Option<u32>,
    /// 主题别名最大值
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic_alias_max: Option<u16>,
    /// 用户属性键值对
    #[serde(default)]
    pub user_properties: std::collections::HashMap<String, String>,
}

/// 单个订阅。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub filter: String,
    #[serde(default)]
    pub qos: Qos,
    /// 着色（消息列表中用同色标记）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for Subscription {
    fn default() -> Self {
        Subscription {
            filter: String::new(),
            qos: Qos::default(),
            color: None,
            enabled: true,
        }
    }
}

/// 一条连接配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub name: String,
    /// 传输协议：mqtt / mqtts / ws / wss
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub host: String,
    pub port: u16,
    /// WebSocket 路径（ws/wss 时可用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub client_id: String,
    #[serde(default)]
    pub username: String,
    /// 凭据引用：keyring://conn/{id}，真值只在 OS Keychain
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
    #[serde(default)]
    pub mqtt_version: ProtocolVersion,
    #[serde(default = "default_true")]
    pub clean_session: bool,
    #[serde(default = "default_keep_alive")]
    pub keep_alive: u16,
    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_will: Option<LastWill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<Mqtt5Props>,
    #[serde(default)]
    pub auto_connect: bool,
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
}

fn default_protocol() -> String {
    "mqtt".to_string()
}
fn default_keep_alive() -> u16 {
    60
}

impl Connection {
    /// 构造一条带随机 id 的空白连接。
    pub fn new_blank(name: &str) -> Self {
        Connection {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            protocol: "mqtt".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1883,
            path: None,
            client_id: format!("mqttkit-{}", &Uuid::new_v4().to_string()[..8]),
            username: String::new(),
            credential_ref: None,
            mqtt_version: ProtocolVersion::default(),
            clean_session: true,
            keep_alive: 60,
            tls: TlsConfig::default(),
            proxy: None,
            last_will: None,
            properties: None,
            auto_connect: false,
            subscriptions: Vec::new(),
        }
    }
}

/// 连接状态机状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Idle,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
    Disconnecting,
}

/// 收到的消息（IPC DTO）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: String,
    pub connection_id: String,
    pub topic: String,
    /// payload base64
    pub payload_base64: String,
    /// 文本预览（截断至 1KB），用于列表快速渲染
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// 原始字节长度
    pub size: usize,
    #[serde(default)]
    pub qos: Qos,
    #[serde(default)]
    pub retain: bool,
    /// 到达时间（Unix 毫秒）
    pub timestamp: i64,
}

/// 收发流量统计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TrafficStats {
    pub received: u64,
    pub sent: u64,
    /// 最近 1s 接收速率
    pub recv_rate: f64,
    /// 最近 1s 发送速率
    pub send_rate: f64,
}

/// 应用设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_buffer")]
    pub message_buffer: usize,
    #[serde(default = "default_flush_interval")]
    pub flush_interval_ms: u64,
    #[serde(default = "default_flush_batch")]
    pub flush_batch: usize,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    /// 遥测：默认关闭，需显式授权开启
    #[serde(default)]
    pub telemetry_enabled: bool,
}

fn default_theme() -> String {
    "auto".into()
}
fn default_locale() -> String {
    "zh-CN".into()
}
fn default_buffer() -> usize {
    100_000
}
fn default_flush_interval() -> u64 {
    50
}
fn default_flush_batch() -> usize {
    200
}
fn default_log_level() -> String {
    "info".into()
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            theme: default_theme(),
            locale: default_locale(),
            message_buffer: default_buffer(),
            flush_interval_ms: default_flush_interval(),
            flush_batch: default_flush_batch(),
            log_level: default_log_level(),
            minimize_to_tray: true,
            telemetry_enabled: false,
        }
    }
}
