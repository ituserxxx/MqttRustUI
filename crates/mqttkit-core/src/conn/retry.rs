//! 指数退避重连策略。
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub initial: Duration,
    pub factor: f64,
    pub max: Duration,
    /// 抖动比例 ±jitter，避免大量客户端同时重连（惊群）
    pub jitter: f64,
    pub max_attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy {
            initial: Duration::from_secs(1),
            factor: 1.8,
            max: Duration::from_secs(30),
            jitter: 0.2,
            max_attempts: 10,
        }
    }
}

impl RetryPolicy {
    /// 第 `attempt` 次（从 0 开始）重连的等待时长（含抖动）。
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let base = (self.initial.as_secs_f64() * self.factor.powi(attempt as i32)).min(self.max.as_secs_f64());
        let jitter = if self.jitter > 0.0 {
            // [-jitter, +jitter] 均匀分布
            let r = fastrand_like();
            base * self.jitter * (2.0 * r - 1.0)
        } else {
            0.0
        };
        let total = (base + jitter).clamp(0.0, self.max.as_secs_f64());
        Duration::from_secs_f64(total)
    }

    pub fn exhausted(&self, attempt: u32) -> bool {
        attempt >= self.max_attempts
    }
}

/// 轻量随机源（避免引入 rand 依赖）。
fn fastrand_like() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static S: AtomicU64 = AtomicU64::new(0x9E3779B97F4A7C15);
    let x = S.fetch_add(0x2545F4914F6CDD1D, Ordering::Relaxed);
    let x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    let x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    let x = x ^ (x >> 31);
    (x >> 11) as f64 / (1u64 << 53) as f64
}
