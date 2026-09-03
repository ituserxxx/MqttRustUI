//! schema 版本迁移。
//!
//! 每次结构变更递增 `SCHEMA_VERSION`，并在这里登记从旧版本到新版本的迁移步骤。
//! 当前为 v1，暂无历史版本，迁移函数为占位以保持扩展位。
use crate::model::AppConfig;

/// 运行迁移：把 `cfg.schema_version` 逐级提升到当前版本。
/// 返回 Err 表示迁移失败（调用方应回落默认配置，不阻断启动）。
pub fn run(cfg: &mut AppConfig) -> Result<(), String> {
    // 示例（未来）：while cfg.schema_version < SCHEMA_VERSION { match cfg.schema_version { 1 => v1_to_v2(cfg)?, _ => {} } }
    if cfg.schema_version == 0 {
        cfg.schema_version = 1;
    }
    if cfg.schema_version > crate::model::SCHEMA_VERSION {
        return Err(format!(
            "配置文件版本 {} 高于本程序支持的 {}",
            cfg.schema_version,
            crate::model::SCHEMA_VERSION
        ));
    }
    cfg.schema_version = crate::model::SCHEMA_VERSION;
    Ok(())
}
