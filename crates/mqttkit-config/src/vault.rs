//! 凭据保险库。
//!
//! 真值只存 OS Keychain（Windows Credential Manager / macOS Keychain /
//! Linux Secret Service），配置文件里只有 `credential_ref` 引用。
//!
//! 目标环境若无可用 Keychain 后端（如 headless Linux / WSL），降级为
//! 进程内存存储——凭据仍在，但重启即丢失，并给出明确警告。本项目已确认
//! **不做主密码二次加密**，因此无法做"配置内加密落盘"的安全兜底，无 Keychain
//! 环境下请以明文环境变量或重新输入替代。
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;

const SERVICE: &str = "mqttkit";

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("Keychain 访问失败: {0}")]
    Keyring(String),
    #[error("凭据不存在")]
    NotFound,
}

/// 凭据存储：优先 keyring，失败降级内存。
pub struct Vault {
    memory: Mutex<HashMap<String, String>>,
    degraded: bool,
}

impl Vault {
    pub fn new() -> Self {
        // 探测 keyring 是否可用：尝试写入再删除一个探针条目。
        let degraded = !probe_keyring();
        if degraded {
            tracing::warn!(
                target: "vault",
                "未检测到 OS Keychain 后端，凭据将仅存于内存（重启即丢失）"
            );
        }
        Vault {
            memory: Mutex::new(HashMap::new()),
            degraded,
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// 写入某个连接对应的密码。
    pub fn set_password(&self, conn_id: &str, password: &str) -> Result<(), VaultError> {
        if !self.degraded {
            match keyring::Entry::new(SERVICE, conn_id) {
                Ok(e) => {
                    if let Err(err) = e.set_password(password) {
                        tracing::warn!(target: "vault", "keyring 写入失败，降级内存: {err}");
                        self.memory.lock().unwrap().insert(conn_id.to_string(), password.to_string());
                        return Ok(());
                    }
                    return Ok(());
                }
                Err(err) => {
                    tracing::warn!(target: "vault", "keyring 条目创建失败，降级内存: {err}");
                }
            }
        }
        self.memory.lock().unwrap().insert(conn_id.to_string(), password.to_string());
        Ok(())
    }

    /// 读取密码。
    pub fn get_password(&self, conn_id: &str) -> Result<String, VaultError> {
        if !self.degraded {
            if let Ok(e) = keyring::Entry::new(SERVICE, conn_id) {
                if let Ok(p) = e.get_password() {
                    return Ok(p);
                }
            }
        }
        self.memory
            .lock()
            .unwrap()
            .get(conn_id)
            .cloned()
            .ok_or(VaultError::NotFound)
    }

    /// 删除密码。
    pub fn delete_password(&self, conn_id: &str) -> Result<(), VaultError> {
        if !self.degraded {
            if let Ok(e) = keyring::Entry::new(SERVICE, conn_id) {
                let _ = e.delete_credential();
            }
        }
        self.memory.lock().unwrap().remove(conn_id);
        Ok(())
    }
}

impl Default for Vault {
    fn default() -> Self {
        Self::new()
    }
}

/// 探测 keyring 后端是否可用。
fn probe_keyring() -> bool {
    const PROBE: &str = "__mqttkit_probe__";
    match keyring::Entry::new(SERVICE, PROBE) {
        Ok(e) => {
            let _ = e.set_password("probe");
            let ok = e.get_password().is_ok();
            let _ = e.delete_credential();
            ok
        }
        Err(_) => false,
    }
}

/// 由连接 id 生成配置中引用的 `credential_ref`。
pub fn credential_ref(conn_id: &str) -> String {
    format!("keyring://conn/{conn_id}")
}
