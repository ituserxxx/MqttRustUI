//! 收发流量统计（滑动 1s 窗口速率）。
use mqttkit_ipc::model::TrafficStats;
// 注意：必须显式限定 std::time::Instant——rhai 也导出一个 Instant 类型，
// 同名遮蔽会让 #[derive(Default)] 解析到 rhai::Instant（无 Default impl）。
use std::time::{Duration, Instant};

pub struct TrafficCounter {
    received: u64,
    sent: u64,
    window_start: Instant,
    recv_in_window: u64,
    send_in_window: u64,
}

impl Default for TrafficCounter {
    fn default() -> Self {
        TrafficCounter {
            received: 0,
            sent: 0,
            window_start: Instant::now(),
            recv_in_window: 0,
            send_in_window: 0,
        }
    }
}

impl TrafficCounter {
    pub fn new() -> Self {
        TrafficCounter {
            window_start: Instant::now(),
            ..Default::default()
        }
    }

    pub fn mark_recv(&mut self, n: u64) {
        self.received += n;
        self.recv_in_window += n;
    }

    pub fn mark_send(&mut self, n: u64) {
        self.sent += n;
        self.send_in_window += n;
    }

    /// 取快照并重置 1s 窗口（应每秒调用一次）。返回 0 速率若窗口未到 1s。
    pub fn snapshot(&mut self) -> TrafficStats {
        let elapsed = self.window_start.elapsed();
        let (r_rate, s_rate) = if elapsed >= Duration::from_secs(1) {
            let secs = elapsed.as_secs_f64();
            let r = self.recv_in_window as f64 / secs;
            let s = self.send_in_window as f64 / secs;
            self.window_start = Instant::now();
            self.recv_in_window = 0;
            self.send_in_window = 0;
            (r, s)
        } else {
            (0.0, 0.0)
        };
        TrafficStats {
            received: self.received,
            sent: self.sent,
            recv_rate: r_rate,
            send_rate: s_rate,
        }
    }

    pub fn total_received(&self) -> u64 {
        self.received
    }

    pub fn total_sent(&self) -> u64 {
        self.sent
    }
}
