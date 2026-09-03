//! 消息环形缓冲：按连接隔离，溢出丢弃最旧。
use mqttkit_ipc::model::StoredMessage;

/// 单连接的消息环形缓冲。
pub struct MessageBuffer {
    inner: std::collections::VecDeque<StoredMessage>,
    capacity: usize,
    /// 累计丢弃条数（UI 提示用）
    dropped: u64,
}

impl MessageBuffer {
    pub fn new(capacity: usize) -> Self {
        MessageBuffer {
            inner: std::collections::VecDeque::with_capacity(capacity.min(1024)),
            capacity: capacity.max(1),
            dropped: 0,
        }
    }

    /// 写入一条消息；满则丢最旧并累加 dropped。
    pub fn push(&mut self, msg: StoredMessage) {
        if self.inner.len() >= self.capacity {
            self.inner.pop_front();
            self.dropped += 1;
        }
        self.inner.push_back(msg);
    }

    /// 当前使用率（0.0 ~ 1.0）。
    pub fn usage(&self) -> f64 {
        self.inner.len() as f64 / self.capacity as f64
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// 取出最近 `limit` 条（用于切换连接后回填）。返回从旧到新。
    pub fn recent(&self, limit: usize) -> Vec<StoredMessage> {
        let start = self.inner.len().saturating_sub(limit);
        self.inner.iter().skip(start).cloned().collect()
    }

    /// 清空。
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// 取出全部消息（从旧到新），缓冲清空。
    pub fn drain_all(&mut self) -> Vec<StoredMessage> {
        self.inner.drain(..).collect()
    }
}
