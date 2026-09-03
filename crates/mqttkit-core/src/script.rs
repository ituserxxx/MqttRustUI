//! rhai 脚本引擎封装（P2 功能骨架）。
//!
//! 用于在收到消息后做 payload 变换、设备模拟、定时发布脚本。沙箱由 rhai 提供，
//! 不暴露任何文件系统 / 网络 API。当前为最小可用封装。
use rhai::{Engine, Scope};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("脚本编译失败: {0}")]
    Compile(String),
    #[error("脚本执行失败: {0}")]
    Run(String),
}

/// 脚本运行时。全局共享一个 Engine 即可（rhai Engine 线程安全）。
pub struct ScriptRuntime {
    engine: Engine,
}

impl Default for ScriptRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptRuntime {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        // 沙箱：禁用不安全的系统函数
        engine.disable_symbol("eval");
        ScriptRuntime { engine }
    }

    /// 以 `payload`（字符串）为入参执行脚本，返回脚本最后表达式的值（字符串）。
    pub fn run(&self, source: &str, payload: &str) -> Result<String, ScriptError> {
        let ast = self
            .engine
            .compile(source)
            .map_err(|e| ScriptError::Compile(e.to_string()))?;
        let mut scope = Scope::new();
        scope.push("payload", payload.to_string());
        let r: rhai::Dynamic = self
            .engine
            .eval_ast_with_scope(&mut scope, &ast)
            .map_err(|e| ScriptError::Run(e.to_string()))?;
        Ok(r.to_string())
    }
}
