//! rumqttc 接入与协议版本抽象。
//!
//! 本模块是 `mqttkit-core` 里**唯一**直接接触 rumqttc 的地方。对外的连接参数
//! 统一用 `mqttkit_ipc::model::Connection`，绝不把 rumqttc 类型泄漏到 core 的公共
//! 接口（架构硬约束）。
//!
//! rumqttc 0.25 的 MQTT 3.1.1 与 5.0 是**两套独立实现**：
//! - v4（3.1.1）：crate 根的 `AsyncClient` / `EventLoop` / `MqttOptions`
//! - v5：`rumqttc::v5::AsyncClient` / `v5::EventLoop` / `v5::MqttOptions`
//! 两者没有公共 trait，因此这里用 [`ClientHandle`] / [`EventLoopHandle`] 做枚举封装，
//! 对 core 其余部分暴露统一接口。
use mqttkit_ipc::model::{Connection, ProtocolVersion, Qos};
use rumqttc::{
    AsyncClient as ClientV4, EventLoop as EventLoopV4, LastWill as LastWillV4,
    MqttOptions as MqttOptionsV4, QoS as QoSV4, Transport as TransportV4,
};
use rumqttc::v5::{
    AsyncClient as ClientV5, EventLoop as EventLoopV5, MqttOptions as MqttOptionsV5,
};
use rumqttc::v5::mqttbytes::QoS as QoSV5;
use rumqttc::v5::mqttbytes::v5::LastWill as LastWillV5;
use rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use rustls::{DigitallySignedStruct, SignatureScheme};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("参数非法: {0}")]
    Invalid(String),
    #[error("TLS 配置错误: {0}")]
    Tls(String),
    #[error("代理配置错误: {0}")]
    Proxy(String),
}

/// v4 QoS（rumqttc 根模块）。
fn qos_v4(q: Qos) -> QoSV4 {
    match q {
        Qos::AtMostOnce => QoSV4::AtMostOnce,
        Qos::AtLeastOnce => QoSV4::AtLeastOnce,
        Qos::ExactlyOnce => QoSV4::ExactlyOnce,
    }
}

/// v5 QoS（rumqttc::v5::mqttbytes，与 v4 是两个不同类型）。
fn qos_v5(q: Qos) -> QoSV5 {
    match q {
        Qos::AtMostOnce => QoSV5::AtMostOnce,
        Qos::AtLeastOnce => QoSV5::AtLeastOnce,
        Qos::ExactlyOnce => QoSV5::ExactlyOnce,
    }
}

/// 统一命令端：封装 v4/v5 AsyncClient，对 core 暴露同一套 subscribe/publish 接口。
#[derive(Clone)]
pub enum ClientHandle {
    V4(ClientV4),
    V5(ClientV5),
}

/// 统一事件循环：封装 v4/v5 EventLoop。`poll()` 返回归一化后的 [`PollOutcome`]，
/// 避免把 rumqttc 的 Event/Publish 类型泄漏给调用方。
pub enum EventLoopHandle {
    V4(EventLoopV4),
    V5(EventLoopV5),
}

/// 归一化的轮询结果（core 只关心这三种）。
pub enum PollOutcome {
    /// 收到一条 Publish（topic / payload / qos / retain 已转成自有类型）。
    Publish {
        topic: String,
        payload: Vec<u8>,
        qos: Qos,
        retain: bool,
    },
    /// 连接建立（v4 ConnAck / v5 ConnAck 归一化）。
    Connected,
    /// 其他事件（Ping、Ack 等），忽略。
    Other,
}

impl EventLoopHandle {
    pub async fn poll(&mut self) -> Result<PollOutcome, String> {
        match self {
            EventLoopHandle::V4(el) => match el.poll().await {
                Ok(rumqttc::Event::Incoming(rumqttc::Incoming::Publish(p))) => {
                    Ok(PollOutcome::Publish {
                        topic: p.topic,
                        payload: p.payload.as_ref().to_vec(),
                        qos: match p.qos {
                            QoSV4::AtMostOnce => Qos::AtMostOnce,
                            QoSV4::AtLeastOnce => Qos::AtLeastOnce,
                            QoSV4::ExactlyOnce => Qos::ExactlyOnce,
                        },
                        retain: p.retain,
                    })
                }
                Ok(rumqttc::Event::Incoming(rumqttc::Incoming::ConnAck(_))) => {
                    Ok(PollOutcome::Connected)
                }
                Ok(_) => Ok(PollOutcome::Other),
                Err(e) => Err(e.to_string()),
            },
            EventLoopHandle::V5(el) => match el.poll().await {
                Ok(rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::Publish(p))) => {
                    Ok(PollOutcome::Publish {
                        topic: String::from_utf8_lossy(&p.topic).to_string(),
                        payload: p.payload.as_ref().to_vec(),
                        qos: match p.qos {
                            QoSV5::AtMostOnce => Qos::AtMostOnce,
                            QoSV5::AtLeastOnce => Qos::AtLeastOnce,
                            QoSV5::ExactlyOnce => Qos::ExactlyOnce,
                        },
                        retain: p.retain,
                    })
                }
                Ok(rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::ConnAck(_))) => {
                    Ok(PollOutcome::Connected)
                }
                Ok(_) => Ok(PollOutcome::Other),
                Err(e) => Err(e.to_string()),
            },
        }
    }
}

/// 由连接配置 + 明文密码构造 v4 `MqttOptions`。
fn build_options_v4(
    conn: &Connection,
    password: Option<&str>,
) -> Result<MqttOptionsV4, ProtocolError> {
    let mut opts = MqttOptionsV4::new(conn.client_id.clone(), conn.host.clone(), conn.port);
    opts.set_keep_alive(Duration::from_secs(conn.keep_alive as u64));
    opts.set_clean_session(conn.clean_session);
    if !conn.username.is_empty() {
        opts.set_credentials(conn.username.clone(), password.unwrap_or(""));
    }

    let transport = build_transport(conn)?;
    opts.set_transport(transport);

    if let Some(p) = &conn.proxy {
        let (host, port) = parse_proxy_addr(&p.url);
        opts.set_proxy(rumqttc::Proxy {
            ty: rumqttc::ProxyType::Http,
            addr: host,
            port,
            auth: rumqttc::ProxyAuth::None,
        });
    }

    if let Some(w) = &conn.last_will {
        let payload = crate::codec::decode_base64(&w.payload_base64)
            .map_err(|e| ProtocolError::Invalid(format!("遗嘱 payload base64 错误: {e}")))?;
        opts.set_last_will(LastWillV4::new(
            w.topic.clone(),
            payload,
            qos_v4(w.qos),
            w.retain,
        ));
    }
    Ok(opts)
}

/// 由连接配置 + 明文密码构造 v5 `MqttOptions`（含 MQTT5 连接属性）。
fn build_options_v5(
    conn: &Connection,
    password: Option<&str>,
) -> Result<MqttOptionsV5, ProtocolError> {
    let mut opts = MqttOptionsV5::new(conn.client_id.clone(), conn.host.clone(), conn.port);
    opts.set_keep_alive(Duration::from_secs(conn.keep_alive as u64));
    // v5 里 clean_session 对应 clean_start
    opts.set_clean_start(conn.clean_session);
    if !conn.username.is_empty() {
        opts.set_credentials(conn.username.clone(), password.unwrap_or(""));
    }

    let transport = build_transport(conn)?;
    opts.set_transport(transport);

    if let Some(p) = &conn.proxy {
        let (host, port) = parse_proxy_addr(&p.url);
        opts.set_proxy(rumqttc::v5::Proxy {
            ty: rumqttc::v5::ProxyType::Http,
            addr: host,
            port,
            auth: rumqttc::v5::ProxyAuth::None,
        });
    }

    // MQTT5 连接属性：会话过期 / 最大包 / 主题别名 / 用户属性
    if let Some(props) = &conn.properties {
        opts.set_session_expiry_interval(props.session_expiry_interval);
        opts.set_max_packet_size(props.maximum_packet_size);
        opts.set_topic_alias_max(props.topic_alias_max);
        if !props.user_properties.is_empty() {
            let pairs: Vec<(String, String)> = props
                .user_properties
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            opts.set_user_properties(pairs);
        }
    }

    if let Some(w) = &conn.last_will {
        let payload = crate::codec::decode_base64(&w.payload_base64)
            .map_err(|e| ProtocolError::Invalid(format!("遗嘱 payload base64 错误: {e}")))?;
        opts.set_last_will(LastWillV5::new(
            w.topic.clone(),
            payload,
            qos_v5(w.qos),
            w.retain,
            None,
        ));
    }
    Ok(opts)
}

/// 代理解析：从 URL 拆 host:port（简单解析，http:// 前缀可省）。
fn parse_proxy_addr(url: &str) -> (String, u16) {
    let addr = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    match addr.split_once(':') {
        Some((h, po)) => (h.to_string(), po.parse::<u16>().unwrap_or(8080)),
        None => (addr.to_string(), 8080),
    }
}

/// 构造 TLS `ClientConfig`（v4/v5 共用，两版 Transport/TlsConfiguration 结构一致）。
///
/// - `verify_hostname = true`（默认）：webpki 根证书 + 用户 CA，标准校验。
/// - `verify_hostname = false`：danger-mode 放行自签证书与主机名校验。
///   ⚠️ 仅用于内网/测试环境。
/// - 返回值第二个元素为 `Simple` 变体所需的 (ca, client_auth)。
fn build_tls_config(
    conn: &Connection,
) -> Result<TlsParts, ProtocolError> {
    let tls = &conn.tls;

    // 客户端双向认证（可选）。
    let client_auth: Option<(Vec<u8>, Vec<u8>)> = match &tls.client_auth {
        Some(auth) => {
            let cert = std::fs::read(&auth.cert)
                .map_err(|e| ProtocolError::Tls(format!("读取客户端证书失败: {e}")))?;
            let key = std::fs::read(&auth.key)
                .map_err(|e| ProtocolError::Tls(format!("读取客户端私钥失败: {e}")))?;
            Some((cert, key))
        }
        None => None,
    };

    if !tls.verify_hostname {
        // 自签放行路径：danger-mode ClientConfig，跳过证书链与主机名校验。
        // rustls 0.23 类型状态机：WantsVerifier 阶段二选一——
        //   with_root_certificates() 或 dangerous().with_custom_certificate_verifier()
        // 之后 with_no_client_auth() / with_client_auth_cert() 返回**最终 ClientConfig**，
        // 两个分支类型不同（builder vs ClientConfig），必须各自构造、不能赋回同一变量。
        let builder = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerification));
        let cfg = if let Some((cert_pem, key_pem)) = &client_auth {
            let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
                rustls_pemfile::certs(&mut cert_pem.as_slice())
                    .collect::<Result<_, _>>()
                    .map_err(|e| ProtocolError::Tls(format!("解析客户端证书失败: {e}")))?;
            let key = rustls_pemfile::private_key(&mut key_pem.as_slice())
                .map_err(|e| ProtocolError::Tls(format!("读取私钥失败: {e}")))?
                .ok_or_else(|| ProtocolError::Tls("私钥文件未包含 PEM 私钥块".into()))?;
            builder
                .with_client_auth_cert(certs, key)
                .map_err(|e| ProtocolError::Tls(format!("加载客户端认证失败: {e}")))?
        } else {
            builder.with_no_client_auth()
        };
        return Ok(TlsParts::Custom(cfg));
    }

    // 标准校验路径：webpki 根 + 用户 CA / 客户端认证。
    let ca_bytes = tls
        .ca
        .as_ref()
        .map(std::fs::read)
        .transpose()
        .map_err(|e| ProtocolError::Tls(format!("读取 CA 失败: {e}")))?
        .unwrap_or_default();
    Ok(TlsParts::Simple {
        ca: ca_bytes,
        client_auth,
    })
}

enum TlsParts {
    Custom(rustls::ClientConfig),
    Simple {
        ca: Vec<u8>,
        client_auth: Option<(Vec<u8>, Vec<u8>)>,
    },
}

/// 构造传输层（v4/v5 共用——v5 模块的 `Transport` 是 crate 根 `Transport` 的
/// 私有 re-export，所以直接用 `rumqttc::Transport` 一个版本即可）。
fn build_transport(conn: &Connection) -> Result<TransportV4, ProtocolError> {
    let proto = conn.protocol.as_str();
    match (proto, &conn.tls.mode) {
        // rumqttc 0.25：ws()/wss() 不带 path，WS 路径放在 host 里（如 "broker/mqtt"）
        ("ws", _) => Ok(TransportV4::ws()),
        ("wss", _) => Ok(TransportV4::wss_with_default_config()),
        (_, mqttkit_ipc::model::TlsMode::None) => Ok(TransportV4::tcp()),
        (_, mqttkit_ipc::model::TlsMode::Tls) => match build_tls_config(conn)? {
            // rumqttc 0.25 自定义 TLS 变体是 Rustls(Arc<ClientConfig>)，有 From<ClientConfig>
            TlsParts::Custom(cfg) => Ok(TransportV4::tls_with_config(cfg.into())),
            TlsParts::Simple { ca, client_auth } => Ok(TransportV4::tls_with_config(
                rumqttc::TlsConfiguration::Simple {
                    ca,
                    client_auth,
                    alpn: None,
                },
            )),
        },
    }
}

/// 放行所有服务端证书的 verifier（仅在 verify_hostname=false 时使用）。
#[derive(Debug)]
struct NoVerification;

impl ServerCertVerifier for NoVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

/// 打开一条连接，返回统一命令端与统一事件循环（按 mqtt_version 分流 v4/v5）。
pub fn open(
    conn: &Connection,
    password: Option<&str>,
) -> Result<(ClientHandle, EventLoopHandle), ProtocolError> {
    match conn.mqtt_version {
        ProtocolVersion::V311 => {
            let opts = build_options_v4(conn, password)?;
            let (client, eventloop) = ClientV4::new(opts, 10);
            Ok((ClientHandle::V4(client), EventLoopHandle::V4(eventloop)))
        }
        ProtocolVersion::V50 => {
            let opts = build_options_v5(conn, password)?;
            let (client, eventloop) = ClientV5::new(opts, 10);
            Ok((ClientHandle::V5(client), EventLoopHandle::V5(eventloop)))
        }
    }
}

/// 订阅。
pub async fn subscribe(
    client: &ClientHandle,
    filter: &str,
    qos: Qos,
) -> Result<(), ProtocolError> {
    match client {
        ClientHandle::V4(c) => c
            .subscribe(filter, qos_v4(qos))
            .await
            .map_err(|e| ProtocolError::Invalid(e.to_string())),
        ClientHandle::V5(c) => c
            .subscribe(filter, qos_v5(qos))
            .await
            .map_err(|e| ProtocolError::Invalid(e.to_string())),
    }
}

/// 取消订阅。
pub async fn unsubscribe(client: &ClientHandle, filter: &str) -> Result<(), ProtocolError> {
    match client {
        ClientHandle::V4(c) => c
            .unsubscribe(filter)
            .await
            .map_err(|e| ProtocolError::Invalid(e.to_string())),
        ClientHandle::V5(c) => c
            .unsubscribe(filter)
            .await
            .map_err(|e| ProtocolError::Invalid(e.to_string())),
    }
}

/// 发布（payload 为原始字节）。
pub async fn publish(
    client: &ClientHandle,
    topic: &str,
    qos: Qos,
    retain: bool,
    payload: Vec<u8>,
) -> Result<(), ProtocolError> {
    match client {
        ClientHandle::V4(c) => c
            .publish(topic, qos_v4(qos), retain, payload)
            .await
            .map_err(|e| ProtocolError::Invalid(e.to_string())),
        // v5 的 publish payload 参数是 Into<Bytes>，Vec<u8> 可直接转
        ClientHandle::V5(c) => c
            .publish(topic, qos_v5(qos), retain, bytes::Bytes::from(payload))
            .await
            .map_err(|e| ProtocolError::Invalid(e.to_string())),
    }
}

/// 主动断开（通知 broker 后关闭）。
pub async fn disconnect(client: &ClientHandle) -> Result<(), ProtocolError> {
    match client {
        ClientHandle::V4(c) => c
            .disconnect()
            .await
            .map_err(|e| ProtocolError::Invalid(e.to_string())),
        ClientHandle::V5(c) => c
            .disconnect()
            .await
            .map_err(|e| ProtocolError::Invalid(e.to_string())),
    }
}
