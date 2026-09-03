//! rumqttc 接入与协议版本抽象。
//!
//! 本模块是 `mqttkit-core` 里**唯一**直接接触 rumqttc 的地方。对外的连接参数
//! 统一用 `mqttkit_ipc::model::Connection`，绝不把 rumqttc 类型泄漏到 core 的公共
//! 接口（架构硬约束）。MQTT 3.1.1 与 5.0 通过 `MqttOptions::set_protocol` 区分。
use mqttkit_ipc::model::{Connection, ProtocolVersion, Qos};
use rumqttc::{
    AsyncClient, EventLoop, LastWill as RumqLastWill, MqttOptions, Protocol, QoS as RumqQos,
    Request, Transport,
};
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
    opts.set_keep_alive(conn.keep_alive);
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

    // 代理
    if let Some(p) = &conn.proxy {
        let proxy = rumqttc::Proxy::http(p.url.as_str())
            .map_err(|_| ProtocolError::Proxy("代理 URL 非法".into()))?;
        opts.set_proxy(proxy);
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
        ("ws", _) => Ok(Transport::ws(conn.path.clone().unwrap_or_else(|| "/mqtt".into()))),
        ("wss", _) => Ok(Transport::wss(conn.path.clone().unwrap_or_else(|| "/mqtt".into()))),
        (_, mqttkit_ipc::model::TlsMode::None) => Ok(Transport::tcp()),
        (_, mqttkit_ipc::model::TlsMode::Tls) => {
            // 加载 CA / 客户端证书到内存字节，交给 rustls
            let ca = match &conn.tls.ca {
                Some(path) => Some(
                    std::fs::read(path)
                        .map_err(|e| ProtocolError::Tls(format!("读取 CA 失败: {e}")))?,
                ),
                None => None,
            };
            let client_auth = match &conn.tls.client_auth {
                Some(auth) => {
                    let cert = std::fs::read(&auth.cert)
                        .map_err(|e| ProtocolError::Tls(format!("读取客户端证书失败: {e}")))?;
                    let key = std::fs::read(&auth.key)
                        .map_err(|e| ProtocolError::Tls(format!("读取客户端私钥失败: {e}")))?;
                    Some((cert, key))
                }
                None => None,
            };
            // rumqttc 0.25：`tls_with_config` 接收已解析的字节。
            // verify_hostname=false 时请改用 `tls_with_selfsigned_certs`（放行自签）。
            Ok(Transport::tls_with_config(rumqttc::TlsConfiguration {
                ca,
                client_auth,
                alpn: None,
            }))
        }
        _ => Err(ProtocolError::Invalid(format!(
            "不支持的传输/TLS 组合: proto={proto}"
        ))),
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

/// 构造一条 PUBLISH 请求（MQTT5 可带 properties，此处仅 3.1.1 形态，P1 扩展）。
#[allow(dead_code)]
pub fn publish_request(topic: &str, qos: Qos, retain: bool, payload: Vec<u8>) -> Request {
    Request::Publish(rumqttc::Publish {
        topic: topic.into(),
        qos: to_rumq_qos(qos),
        retain,
        payload: payload.into(),
        pkid: 0,
        properties: None,
    })
}
