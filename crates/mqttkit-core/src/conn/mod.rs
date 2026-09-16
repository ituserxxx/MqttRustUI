//! 连接管理器：多连接生命周期、EventLoop 轮询、消息缓冲、批量 flush、指数退避重连。
//!
//! 核心不依赖 Tauri：它只通过 [`EventSink`] trait 把事件交给上层（桌面端用 Tauri
//! `emit` 实现，未来 Web 端用 postMessage 实现）。上层业务与 UI 完全不感知自己跑在哪。
pub mod retry;
use retry::RetryPolicy;

use crate::buffer::MessageBuffer;
use crate::codec;
use crate::protocol::{self, from_rumq_qos};
use crate::session::Session;
use crate::stats::TrafficCounter;
use mqttkit_config::model::AppConfig;
use mqttkit_config::vault::Vault;
use mqttkit_ipc::event::AppEvent;
use mqttkit_ipc::model::{
    AppSettings, Connection, ConnectionState, Qos, StoredMessage, Subscription,
};
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, Publish};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

/// 事件出口：上层实现（桌面端 → Tauri emit；Web 端 → postMessage）。
pub trait EventSink: Send + Sync {
    fn emit(&self, event: AppEvent);
}

/// 单连接运行时（被 poll 任务与命令层共享）。
struct Runtime {
    conn: Connection,
    /// 订阅表（运行时可增删，与连接参数解耦，便于重连重放）
    subs: Mutex<Vec<Subscription>>,
    client: Arc<Mutex<AsyncClient>>,
    buffer: Arc<Mutex<MessageBuffer>>,
    state: Arc<Mutex<ConnectionState>>,
    stats: Arc<Mutex<TrafficCounter>>,
    sink: Arc<dyn EventSink>,
    stop: Arc<AtomicBool>,
    degraded: Arc<AtomicBool>,
}

/// 连接管理器（无 Tauri 依赖）。
pub struct ConnectionManager {
    runtimes: Mutex<HashMap<String, Arc<Runtime>>>,
    vault: Arc<Vault>,
    sink: Arc<dyn EventSink>,
    settings: Arc<Mutex<AppSettings>>,
    retry: RetryPolicy,
}

impl ConnectionManager {
    pub fn new(
        vault: Arc<Vault>,
        sink: Arc<dyn EventSink>,
        settings: AppSettings,
    ) -> Arc<Self> {
        // 注意：不在 new 内 spawn flush 循环——setup 阶段未必有 tokio 运行时上下文。
        // 由上层（桌面端）用其运行时调用 `flush_loop()` 启动。
        Arc::new(ConnectionManager {
            runtimes: Mutex::new(HashMap::new()),
            vault,
            sink,
            settings: Arc::new(Mutex::new(settings)),
            retry: RetryPolicy::default(),
        })
    }

    /// 建立连接并启动轮询任务。
    pub async fn connect(&self, conn: Connection) -> Result<(), String> {
        if self.runtimes.lock().await.contains_key(&conn.id) {
            return Err(format!("连接 {} 已存在", conn.id));
        }
        let password = self.vault.get_password(&conn.id).ok();
        let (client, eventloop) = protocol::open(&conn, password.as_deref())
            .map_err(|e| e.to_string())?;

        let rt = Arc::new(Runtime {
            conn: conn.clone(),
            subs: Mutex::new(conn.subscriptions.clone()),
            client: Arc::new(Mutex::new(client)),
            buffer: Arc::new(Mutex::new(MessageBuffer::new(
                self.settings.lock().await.message_buffer,
            ))),
            state: Arc::new(Mutex::new(ConnectionState::Connecting)),
            stats: Arc::new(Mutex::new(TrafficCounter::new())),
            sink: self.sink.clone(),
            stop: Arc::new(AtomicBool::new(false)),
            degraded: Arc::new(AtomicBool::new(false)),
        });
        self.set_state(&rt, ConnectionState::Connecting, None).await;
        self.runtimes.lock().await.insert(conn.id.clone(), rt.clone());

        let vault = self.vault.clone();
        let retry = self.retry.clone();
        tokio::spawn(async move { poll_loop(rt, vault, retry, eventloop).await });
        Ok(())
    }

    /// 主动断开（停止轮询任务，状态回到 Idle）。
    pub async fn disconnect(&self, id: &str) -> Result<(), String> {
        let rt = self.runtimes.lock().await.get(id).cloned();
        let rt = rt.ok_or_else(|| format!("连接 {id} 不存在"))?;
        // 先发出 Disconnecting，UI 给出"断开中"的短暂反馈
        self.set_state(&rt, ConnectionState::Disconnecting, None).await;
        rt.stop.store(true, Ordering::SeqCst);
        // 触发一次正常断开
        let _ = rt.client.lock().await.disconnect().await;
        self.set_state(&rt, ConnectionState::Idle, None).await;
        self.runtimes.lock().await.remove(id);
        Ok(())
    }

    /// 断开所有连接（托盘"全部断开"用）。
    pub async fn disconnect_all(&self) {
        let ids: Vec<String> = self.runtimes.lock().await.keys().cloned().collect();
        for id in ids {
            let _ = self.disconnect(&id).await;
        }
    }

    pub async fn subscribe(&self, id: &str, filter: String, qos: Qos) -> Result<(), String> {
        let rt = self.runtimes.lock().await.get(id).cloned();
        let rt = rt.ok_or_else(|| format!("连接 {id} 不存在"))?;
        protocol::subscribe(&rt.client.lock().await, &filter, qos)
            .await
            .map_err(|e| e.to_string())?;
        rt.subs.lock().await.push(Subscription {
            filter,
            qos,
            color: None,
            enabled: true,
        });
        Ok(())
    }

    pub async fn unsubscribe(&self, id: &str, filter: &str) -> Result<(), String> {
        let rt = self.runtimes.lock().await.get(id).cloned();
        let rt = rt.ok_or_else(|| format!("连接 {id} 不存在"))?;
        protocol::unsubscribe(&rt.client.lock().await, filter)
            .await
            .map_err(|e| e.to_string())?;
        rt.subs.lock().await.retain(|s| s.filter != filter);
        Ok(())
    }

    pub async fn publish(
        &self,
        id: &str,
        topic: &str,
        qos: Qos,
        retain: bool,
        payload_base64: &str,
    ) -> Result<(), String> {
        let rt = self.runtimes.lock().await.get(id).cloned();
        let rt = rt.ok_or_else(|| format!("连接 {id} 不存在"))?;
        let payload = codec::decode_base64(payload_base64).map_err(|e| e.to_string())?;
        protocol::publish(&rt.client.lock().await, topic, qos, retain, payload)
            .await
            .map_err(|e| e.to_string())?;
        rt.stats.lock().await.mark_send(1);
        Ok(())
    }

    /// 拉取最近 N 条消息快照（切换连接/重连后回填）。
    pub async fn get_messages(&self, id: &str, limit: usize) -> Vec<StoredMessage> {
        let rt = self.runtimes.lock().await.get(id).cloned();
        match rt {
            Some(rt) => rt.buffer.lock().await.recent(limit),
            None => Vec::new(),
        }
    }

    pub async fn clear_messages(&self, id: &str) {
        if let Some(rt) = self.runtimes.lock().await.get(id).cloned() {
            rt.buffer.lock().await.clear();
        }
    }

    pub async fn state(&self, id: &str) -> Option<ConnectionState> {
        let rt = self.runtimes.lock().await.get(id).cloned();
        match rt {
            Some(rt) => Some(rt.state.lock().await.clone()),
            None => None,
        }
    }

    pub async fn update_settings(&self, s: AppSettings) {
        *self.settings.lock().await = s;
    }

    /// 批量 flush 主循环（全局单任务）。由上层运行时 spawn。
    /// 附带 1s 一次的 Stats 推送（收发速率），保证前端"收/发速率"有数。
    pub async fn flush_loop(self: Arc<Self>) {
        let interval = self.settings.lock().await.flush_interval_ms;
        let mut ticker = tokio::time::interval(Duration::from_millis(interval.max(1)));
        let mut stats_ticker = tokio::time::interval(Duration::from_secs(1));
        // 立即触发一次，让首屏 stats 不为空
        stats_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let runtimes = self.runtimes.lock().await.clone();
                    let batch = self.settings.lock().await.flush_batch.max(1);
                    for rt in runtimes.values() {
                        let mut buf = rt.buffer.lock().await;
                        let usage = buf.usage();
                        // 背压降级：内存压力 > 95% 时只发统计、不发明细
                        if usage > 0.95 {
                            if !rt.degraded.swap(true, Ordering::SeqCst) {
                                rt.sink.emit(AppEvent::Backpressure {
                                    connection_id: rt.conn.id.clone(),
                                    degraded: true,
                                });
                            }
                            continue;
                        } else if rt.degraded.swap(false, Ordering::SeqCst) {
                            rt.sink.emit(AppEvent::Backpressure {
                                connection_id: rt.conn.id.clone(),
                                degraded: false,
                            });
                        }
                        let all: Vec<StoredMessage> = buf.drain_all();
                        drop(buf);
                        if all.is_empty() { continue; }
                        for chunk in all.chunks(batch) {
                            rt.sink.emit(AppEvent::MessageBatch {
                                connection_id: rt.conn.id.clone(),
                                messages: chunk.to_vec(),
                            });
                        }
                    }
                }
                _ = stats_ticker.tick() => {
                    let runtimes = self.runtimes.lock().await.clone();
                    for rt in runtimes.values() {
                        let stats = rt.stats.lock().await.snapshot();
                        rt.sink.emit(AppEvent::Stats {
                            connection_id: rt.conn.id.clone(),
                            stats,
                        });
                    }
                }
            }
        }
    }

    async fn set_state(&self, rt: &Runtime, state: ConnectionState, detail: Option<String>) {
        *rt.state.lock().await = state;
        rt.sink.emit(AppEvent::StateChanged {
            connection_id: rt.conn.id.clone(),
            state,
            detail,
        });
    }
}

/// 单连接 EventLoop 轮询 + 重连任务。
async fn poll_loop(
    rt: Arc<Runtime>,
    vault: Arc<Vault>,
    retry: RetryPolicy,
    mut eventloop: EventLoop,
) {
    let conn = &rt.conn;
    // 初始 CONNECT 已发出；等待 Connected 或错误
    loop {
        if rt.stop.load(Ordering::SeqCst) {
            break;
        }
        match eventloop.poll().await {
            Ok(Event::Incoming(Incoming::Publish(p))) => {
                handle_publish(&rt, p).await;
            }
            Ok(Event::Incoming(Incoming::Connected)) => {
                // 重连成功后按会话策略决定是否重放订阅
                let subs = rt.subs.lock().await.clone();
                let session = Session::from_subs(&subs);
                if session.should_replay(&rt.conn) {
                    for sub in session.subscriptions() {
                        if sub.enabled {
                            let _ = protocol::subscribe(
                                &rt.client.lock().await,
                                &sub.filter,
                                sub.qos,
                            )
                            .await;
                        }
                    }
                }
                set_state_remote(&rt, ConnectionState::Connected, None).await;
            }
            Ok(_) => {}
            Err(e) => {
                if rt.stop.load(Ordering::SeqCst) {
                    break;
                }
                tracing::warn!(target: "conn", "连接 {} 异常: {e}", conn.id);
                set_state_remote(&rt, ConnectionState::Reconnecting, Some(e.to_string())).await;
                // 指数退避重连
                let mut attempt: u32 = 0;
                let mut new_loop = None;
                loop {
                    if rt.stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let delay = retry.delay_for(attempt);
                    tokio::time::sleep(delay).await;
                    let password = vault.get_password(&conn.id).ok();
                    match protocol::open(conn, password.as_deref()) {
                        Ok((c, el)) => {
                            *rt.client.lock().await = c;
                            new_loop = Some(el);
                            break;
                        }
                        Err(err) => {
                            attempt += 1;
                            if retry.exhausted(attempt) {
                                set_state_remote(
                                    &rt,
                                    ConnectionState::Failed,
                                    Some(err.to_string()),
                                )
                                .await;
                                return;
                            }
                        }
                    }
                }
                match new_loop {
                    Some(el) => eventloop = el,
                    None => break,
                }
            }
        }
    }
}

async fn handle_publish(rt: &Runtime, p: Publish) {
    let payload = p.payload.as_ref().to_vec();
    let size = payload.len();
    let preview = codec::preview(&payload, 1024);
    let msg = StoredMessage {
        id: uuid::Uuid::new_v4().to_string(),
        connection_id: rt.conn.id.clone(),
        topic: p.topic,
        payload_base64: codec::encode_base64(&payload),
        preview,
        size,
        qos: from_rumq_qos(p.qos),
        retain: p.retain,
        timestamp: now_ms(),
    };
    rt.buffer.lock().await.push(msg);
    rt.stats.lock().await.mark_recv(1);
}

async fn set_state_remote(rt: &Runtime, state: ConnectionState, detail: Option<String>) {
    *rt.state.lock().await = state;
    rt.sink.emit(AppEvent::StateChanged {
        connection_id: rt.conn.id.clone(),
        state,
        detail,
    });
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 由 AppConfig 生成默认设置（供管理器初始化）。
pub fn settings_from_config(cfg: &AppConfig) -> AppSettings {
    cfg.settings.clone()
}
