//! 会话层：每连接的订阅表，以及重连后是否重放订阅的决策。
use mqttkit_ipc::model::{Connection, Subscription};

#[derive(Debug, Default, Clone)]
pub struct Session {
    subscriptions: Vec<Subscription>,
}

impl Session {
    pub fn from_connection(conn: &Connection) -> Self {
        Session {
            subscriptions: conn.subscriptions.clone(),
        }
    }

    /// 由运行时订阅表构造（重连重放用）。
    pub fn from_subs(subs: &[Subscription]) -> Self {
        Session {
            subscriptions: subs.to_vec(),
        }
    }

    pub fn subscriptions(&self) -> &[Subscription] {
        &self.subscriptions
    }

    pub fn add(&mut self, sub: Subscription) {
        if let Some(slot) = self.subscriptions.iter_mut().find(|s| s.filter == sub.filter) {
            *slot = sub;
        } else {
            self.subscriptions.push(sub);
        }
    }

    pub fn remove(&mut self, filter: &str) {
        self.subscriptions.retain(|s| s.filter != filter);
    }

    /// 重连成功后是否需要重放订阅。
    ///
    /// 关键规则（容易做错）：
    /// - `clean_session = true`：broker 丢弃了会话，**必须重放**，否则订阅全丢。
    /// - `clean_session = false`（MQTT5 `session_expiry_interval > 0`）：broker 保留了
    ///   订阅与未确认消息，**不能重放**，否则产生重复订阅。
    pub fn should_replay(&self, conn: &Connection) -> bool {
        if conn.clean_session {
            return true;
        }
        // MQTT 5：session_expiry > 0 表示 broker 保留会话
        if let Some(props) = &conn.properties {
            if props.session_expiry_interval.unwrap_or(0) > 0 {
                return false;
            }
        }
        // clean_session = false 但无 session_expiry 信息：保守地不重放
        false
    }
}
