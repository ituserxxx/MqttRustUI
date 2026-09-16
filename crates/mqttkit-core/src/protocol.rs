//! rumqttc 接入与协议版本抽象。
//!
//! 本模块是 `mqttkit-core` 里**唯一**直接接触 rumqttc 的地方。对外的连接参数
//! 统一用 `mqttkit_ipc::model::Connection`，绝不把 rumqttc 类型泄漏到 core 的公共
//! 接口（架构硬约束）。MQTT 3.1.1 与 5.0 通过 `MqttOptions::set_protocol` 区分。
use mqttkit_ipc::model::{Connection, ProtocolVersion, Qos};
use rumqttc::{
    AsyncClient, EventLoop, LastWill as RumqLastWill, MqttOptions, Protocol, QoS as RumqQos,
    Transport,
};
use rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use rustls::{DigitallySignedStruct, RootCertStore, SignatureScheme};
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

/// 我们的 QoS → rumqttc QoS。
pub fn to_rumq_qos(q: Qos) -> RumqQos {
    match q {
        Qos::AtMostOnce => RumqQos::AtMostOnce,
        Qos::AtLeastOnce => RumqQos::AtLeastOnce,
        Qos::ExactlyOnce => RumqQos::ExactlyOnce,
    }
}

/// rumqttc QoS → 我们的 QoS。
pub fn from_rumq_qos(q: RumqQos) -> Qos {
    match q {
        RumqQos::AtMostOnce => Qos::AtMostOnce,
        RumqQos::AtLeastOnce => Qos::AtLeastOnce,
        RumqQos::ExactlyOnce => Qos::ExactlyOnce,
    }
}

/// 由连接配置 + 明文密码构造 `MqttOptions`。
///
/// 注意：rumqttc 的 TLS/代理构造函数在不同小版本间略有差异，下面用 `tls_with_config`
/// / `set_proxy` 的 0.25 形态；若你安装的版本签名不同，只需改本函数，core 其余部分不受影响。
pub fn build_mqtt_options(conn: &Connection, password: Option<&str>) -> Result<MqttOptions, ProtocolError> {
    let mut opts = MqttOptions::new(conn.client_id.clone(), conn.host.clone(), conn.port);
    // rumqttc 0.25：keep_alive 是 Duration，clean_session 由 set_clean_session 控制
    opts.set_keep_alive(Duration::from_secs(conn.keep_alive as u64));
    opts.set_clean_session(conn.clean_session);
    if !conn.username.is_empty() {
        opts.set_credentials(conn.username.clone(), password.unwrap_or(""));
    }
    opts.set_protocol(match conn.mqtt_version {
        ProtocolVersion::V311 => Protocol::MQTT3_1_1,
        ProtocolVersion::V50 => Protocol::MQTT5,
    });

    // 传输层：ws/wss 与 tcp/tls
    let transport = build_transport(conn)?;
    opts.set_transport(transport);

    // 代理（rumqttc 0.25：Proxy 结构体，proxy_type/addr/port/auth 字段）
    if let Some(p) = &conn.proxy {
        // 从 URL 拆 host:port（简单解析，http:// 前缀可省）
        let addr = p.url.trim_start_matches("http://").trim_start_matches("https://");
        let (host, port) = match addr.split_once(':') {
            Some((h, po)) => (h.to_string(), po.parse::<u16>().unwrap_or(8080)),
            None => (addr.to_string(), 8080),
        };
        opts.set_proxy(rumqttc::Proxy {
            proxy_type: rumqttc::ProxyType::Http,
            addr: host,
            port,
            auth: None,
        });
    }

    // 遗嘱
    if let Some(w) = &conn.last_will {
        let payload = crate::codec::decode_base64(&w.payload_base64)
            .map_err(|e| ProtocolError::Invalid(format!("遗嘱 payload base64 错误: {e}")))?;
        let lw = RumqLastWill::new(w.topic.clone(), payload, to_rumq_qos(w.qos), w.retain);
        opts.set_last_will(lw);
    }

    Ok(opts)
}

fn build_transport(conn: &Connection) -> Result<Transport, ProtocolError> {
    let proto = conn.protocol.as_str();
    match (proto, &conn.tls.mode) {
        // rumqttc 0.25：ws()/wss() 不带 path，WS 路径放在 host 里（如 "broker/mqtt"）
        ("ws", _) => Ok(Transport::ws()),
        ("wss", _) => Ok(Transport::wss_with_default_config()),
        (_, mqttkit_ipc::model::TlsMode::None) => Ok(Transport::tcp()),
        (_, mqttkit_ipc::model::TlsMode::Tls) => build_tls_transport(conn),
        _ => Err(ProtocolError::Invalid(format!(
            "不支持的传输/TLS 组合: proto={proto}"
        ))),
    }
}

/// 构造 TLS 传输。
///
/// - `verify_hostname = true`（默认）：用 rustls 标准校验 + webpki 根证书，交给
///   rumqttc 的 `tls_with_config`。
/// - `verify_hostname = false`（已确认接线）：构造一个放行自签证书与主机名校验的
///   danger-mode `ClientConfig`，交给 rumqttc 的自定义 TLS 入口。
///   ⚠️ 放行自签意味着不做对端真实性校验，仅用于内网/测试环境。
fn build_tls_transport(conn: &Connection) -> Result<Transport, ProtocolError> {
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
        let mut cfg = rustls::ClientConfig::builder()
            .with_root_certificates(RootCertStore::empty())
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerification));
        if let Some((cert_pem, key_pem)) = client_auth {
            let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
                rustls_pemfile::certs(&mut cert_pem.as_slice())
                    .collect::<Result<_, _>>()
                    .map_err(|e| ProtocolError::Tls(format!("解析客户端证书失败: {e}")))?;
            let key = rustls_pemfile::private_key(&mut key_pem.as_slice())
                .map_err(|e| ProtocolError::Tls(format!("读取私钥失败: {e}")))?
                .ok_or_else(|| ProtocolError::Tls("私钥文件未包含 PEM 私钥块".into()))?;
            cfg = cfg
                .with_client_auth_cert(certs, key)
                .map_err(|e| ProtocolError::Tls(format!("加载客户端认证失败: {e}")))?;
        } else {
            cfg = cfg.with_no_client_auth();
        }
        // rumqttc 0.25 自定义 TLS 变体是 `Rustls(Arc<ClientConfig>)`，且有 From<ClientConfig>。
        return Ok(Transport::tls_with_config(cfg.into()));
    }

    // 标准校验路径：rumqttc 的 Simple 变体（webpki 根 + 用户 CA / 客户端认证）。
    // ca 为空时用系统默认根证书（rumqttc 内部走 rustls-native-certs）。
    let ca_bytes = tls
        .ca
        .as_ref()
        .map(std::fs::read)
        .transpose()
        .map_err(|e| ProtocolError::Tls(format!("读取 CA 失败: {e}")))?
        .unwrap_or_default();
    Ok(Transport::tls_with_config(rumqttc::TlsConfiguration::Simple {
        ca: ca_bytes,
        client_auth,
        alpn: None,
    }))
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

/// 打开一条连接，返回命令端（发布/订阅）与事件循环（由管理器轮询）。
pub fn open(
    conn: &Connection,
    password: Option<&str>,
) -> Result<(AsyncClient, EventLoop), ProtocolError> {
    let opts = build_mqtt_options(conn, password)?;
    let (client, eventloop) = AsyncClient::new(opts, 10);
    Ok((client, eventloop))
}

/// 订阅（核心通过 AsyncClient 调用）。
pub async fn subscribe(
    client: &AsyncClient,
    filter: &str,
    qos: Qos,
) -> Result<(), ProtocolError> {
    client
        .subscribe(filter, to_rumq_qos(qos))
        .await
        .map_err(|e| ProtocolError::Invalid(e.to_string()))
}

/// 取消订阅。
pub async fn unsubscribe(client: &AsyncClient, filter: &str) -> Result<(), ProtocolError> {
    client
        .unsubscribe(filter)
        .await
        .map_err(|e| ProtocolError::Invalid(e.to_string()))
}

/// 发布（payload 为原始字节）。
pub async fn publish(
    client: &AsyncClient,
    topic: &str,
    qos: Qos,
    retain: bool,
    payload: Vec<u8>,
) -> Result<(), ProtocolError> {
    client
        .publish(topic, to_rumq_qos(qos), retain, payload)
        .await
        .map_err(|e| ProtocolError::Invalid(e.to_string()))
}
