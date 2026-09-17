//! 配置持久化：JSON 原子写 + debounce + 容错。无数据库。
use crate::migrate;
use crate::model::AppConfig;
use serde_json::Error as JsonError;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::fs;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum PersistError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON 序列化/解析错误: {0}")]
    Json(#[from] JsonError),
}

/// 配置存储。封装路径、原子写、debounce 与损坏兜底。
pub struct ConfigStore {
    path: PathBuf,
    inner: Arc<Mutex<AppConfig>>,
}

impl ConfigStore {
    /// 从给定路径加载配置。损坏则备份为 `.bak` 并回落默认，绝不阻断启动。
    pub async fn load_or_default(path: PathBuf) -> Result<Self, PersistError> {
        let cfg = match fs::read_to_string(&path).await {
            Ok(text) => match serde_json::from_str::<AppConfig>(&text) {
                Ok(mut cfg) => {
                    // 逐级迁移（v1→v2→...），迁移失败也回落默认，不阻断
                    if let Err(e) = migrate::run(&mut cfg) {
                        tracing::warn!(target: "config", "配置迁移失败，回落默认: {e}");
                        Self::backup_and_default(&path).await
                    } else {
                        cfg
                    }
                }
                Err(e) => {
                    tracing::warn!(target: "config", "配置文件解析失败，备份并回落默认: {e}");
                    Self::backup_and_default(&path).await
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppConfig::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            path,
            inner: Arc::new(Mutex::new(cfg)),
        })
    }

    async fn backup_and_default(path: &Path) -> AppConfig {
        let bak = path.with_extension("json.bak");
        if let Err(e) = fs::copy(path, &bak).await {
            tracing::warn!(target: "config", "备份损坏配置失败: {e}");
        } else {
            tracing::info!(target: "config", "已备份损坏配置到 {:?}", bak);
        }
        AppConfig::default()
    }

    /// 读取当前内存中的配置快照。
    pub async fn snapshot(&self) -> AppConfig {
        self.inner.lock().await.clone()
    }

    /// 用新配置替换内存并原子落盘（写入 `{path}.tmp` 再 rename）。
    pub async fn save(&self, cfg: AppConfig) -> Result<(), PersistError> {
        *self.inner.lock().await = cfg.clone();
        self.atomic_write(&cfg).await
    }

    /// 仅落盘，不替换内存（用于 debounce 触发）。
    pub async fn flush(&self) -> Result<(), PersistError> {
        let cfg = self.inner.lock().await.clone();
        self.atomic_write(&cfg).await
    }

    async fn atomic_write(&self, cfg: &AppConfig) -> Result<(), PersistError> {
        let tmp = self.path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(cfg)?;
        fs::write(&tmp, json).await?;
        fs::rename(&tmp, &self.path).await?;
        Ok(())
    }

    /// 防抖落盘循环：高频修改期间只更新内存，每 500ms 落盘一次。
    ///
    /// 返回 future，由调用方在 Tokio runtime 内 spawn（如 `tauri::async_runtime::spawn`）。
    /// 不在本函数内 `tokio::spawn`，以免脱离 runtime 上下文导致 panic，
    /// 也避免让 config crate 直接依赖 Tauri 的 runtime。
    pub async fn debounced_flush_loop(self: Arc<Self>) {
        let mut ticker = tokio::time::interval(Duration::from_millis(500));
        loop {
            ticker.tick().await;
            if let Err(e) = self.flush().await {
                tracing::warn!(target: "config", "防抖落盘失败: {e}");
            }
        }
    }
}
