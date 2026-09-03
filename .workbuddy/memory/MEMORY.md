# MqttRustUI 项目长期记忆

## 项目定位
跨平台 MQTT 桌面 GUI 客户端。Rust 承担 MQTT 通信与数据安全，前端（Vue 3）承担交互与展示。对标 MQTTX / MQTT Explorer，差异化在体积、高频消息性能、本地凭据安全。

**当前范围：仅桌面端**（Windows / macOS / Linux）。Web 端暂不实现，但架构保留扩展位（见约束 6）。

## 既定的技术决策
- 桌面壳：**Tauri 2**（非 Electron、非 Wails）
- MQTT 客户端：**rumqttc 0.25+**，features 启用 use-rustls / websocket / proxy
- 持久化：**纯 JSON，不用任何数据库**（用户明确要求）。`config.json` 原子写于 app_data_dir；凭据走 keyring；**消息历史不落盘**，仅内存环形缓冲（10 万条/连接）+ 手动导出 JSONL
- 脚本引擎：rhai（Rust 原生沙箱，优于内嵌 JS 引擎）
- 前端：Vue 3 + Vite + TypeScript + Pinia
- crate 结构（3 个）：`mqttkit-core`（连接状态机/会话/环形缓冲/payload 编解码/rhai）、`mqttkit-config`（AppConfig 模型 + 原子写 + keyring + 迁移）、`mqttkit-ipc`（契约 DTO）

## 不可违背的架构约束
1. **`mqttkit-core` 不得依赖 Tauri**，也不得让 rumqttc 类型泄漏到 core 的对外接口——保证核心可脱离 GUI 单测，也为将来 wasm 化留口子。
2. crate 依赖方向严格单向：`ipc ← core ← config`，`apps/desktop` 只做装配。
3. 消息推送必须批量化（50ms / 200 条双阈值 flush）+ 环形缓冲 + 虚拟滚动。逐条推送会打爆 WebView，这是同类工具卡顿的根因。
4. 前端通过 `ui/src/api/` 层调用后端，业务组件不得直接 `invoke`。
5. 凭据、私钥、token 永不进入日志；日志出口统一脱敏。
6. **配置 JSON 里只存 `credential_ref` 引用，凭据真值只在 OS Keychain / 内存**——保证配置文件可安全备份与入库。
7. 配置文件写入必须原子化（tmp + rename）+ debounce；解析失败备份 `.bak` 并回落默认值，绝不阻断启动。
8. **重连后是否重放订阅取决于 clean_session**：true 必须重放，false / session_expiry>0 由 broker 保留订阅，重放会导致重复订阅。
9. 背压必须分级降级：缓冲区 >95% 丢弃最旧；IPC 积压 >5000 转为只推 Stats，回落到 1000 以下自动恢复。

## 交付物位置
- 架构方案：`DESIGN.md`（项目根目录），v1.0 已确认五决策
- WSL 构建/运行清单：`RUN_WSL.md`（项目根目录）
- 已实现代码：workspace + `crates/{mqttkit-ipc, mqttkit-config, mqttkit-core}` + `apps/desktop/src-tauri`（Tauri 2）+ `ui/`（Vue3+AntDV）
- 状态：M0 骨架 + M1 核心链路已落地（本机只编写，未在 WSL 编译）
