# MqttRustUI

轻量级跨平台 MQTT 桌面客户端。Rust 负责 MQTT 通信与数据安全，Vue 3 负责交互与展示。对标 MQTTX / MQTT Explorer，差异化在体积、高频消息性能、本地凭据安全。

> **当前状态**：M0 骨架 + M1 核心链路已落地（本机只编写，待编译联调）。
> 完整架构方案见 [DESIGN.md](DESIGN.md)，构建运行清单见 [RUN_WSL.md](RUN_WSL.md)。

## 已实现功能

### 连接管理
- 多连接并存、增删改查、克隆（自动生成新 ClientID）
- 完整连接表单：传输协议（mqtt / mqtts / ws / wss）、主机、端口、WS 路径、协议版本（**MQTT 3.1.1 与 5.0 同时支持**）、ClientID、用户名密码、Keep Alive、Clean Session、自动连接
- **订阅管理**：过滤器、QoS 0/1/2、着色、启用开关、增删
- **TLS**：单向/双向（客户端证书 + 私钥）、自定义 CA、主机名校验开关（关闭时放行自签证书，用于内网/测试）
- **HTTP 代理**、**遗嘱消息**（LWT）、**MQTT5 属性**（会话过期、主题别名上限）
- 连接状态机 6 态：Idle / Connecting / Connected / Reconnecting / Failed / Disconnecting
- **指数退避重连**（初始 1s → 上限 30s，±20% 抖动，最多 10 次）
- **订阅重放策略**：clean_session=true 时重连后自动重放，clean_session=false / session_expiry>0 不重放（避免重复订阅）
- 启动时自动连接标记为 auto_connect 的连接

### 消息收发
- **批量推送**（50ms / 200 条双阈值 flush）—— 高频场景不打爆 UI
- **环形缓冲**（默认 10 万条/连接，溢出丢弃最旧并计数）
- **三档背压**：<70% 正常 / 70–95% 加快 flush / >95% 降级为只推统计（积压回落自动恢复，UI 横幅提示）
- 消息发布：QoS 0/1/2、Retain、UTF-8 payload
- **消息列表虚拟滚动**（高频下仅渲染可视区）
- 切换连接自动回填最近消息快照、清屏
- **消息过滤**：按 topic / payload 文本搜索；Topic 树节点点击做前缀过滤（再点取消）
- payload 预览截断 1KB，二进制安全（base64 传输）

### 展示
- 三栏布局：连接列表（240px）/ Topic 树（260px）/ 消息区 + 底部发布面板
- 工具栏：连接状态标签、收发速率（**每秒 Stats 推送**）、搜索框、清屏
- Topic 树：按 `/` 分层，实时显示各 topic 消息计数
- payload 格式探测：JSON / 文本 / 二进制，前端自动选视图

### 数据持久化与安全
- **纯 JSON 配置**（无数据库）：`config.json` 原子写（tmp+rename）+ 防抖 + 损坏自动 `.bak` 备份并回落默认，绝不阻断启动
- **凭据安全**：真值只存 OS Keychain（Windows Credential Manager / macOS Keychain / Linux Secret Service），配置文件里只有 `credential_ref` 引用——**配置文件可安全备份/同步/提交版本库**；无 Keychain 环境降级为内存存储（重启即丢）
- **schema 版本迁移**：`schema_version` + 逐级联迁框架
- **日志脱敏**：统一出口正则遮蔽 password / token / secret / authorization / PEM 私钥块，文件轮转

### 桌面集成
- 系统托盘：显示主窗口 / **全部断开** / 退出
- 原生菜单：新建连接、导入/导出配置、设置、重新加载、关于
- **单实例**：二次启动聚焦已有窗口
- 窗口最小化到托盘（可配置）

### 配置导入导出
- 导出为 JSON（仅含 `credential_ref` 引用，无敏感明文，可安全分享）
- 导入时 JSON 校验 + 落盘

### 设置界面
- 主题（跟随系统/浅色/深色）、消息缓冲区大小、推送间隔/批量条数、日志级别、最小化到托盘、匿名遥测开关（**默认关闭，需显式开启**）

## 技术栈

| 层 | 选型 |
| --- | --- |
| 桌面壳 | Tauri 2（系统 WebView） |
| MQTT 客户端 | rumqttc 0.25+（rustls / websocket / proxy） |
| 异步运行时 | tokio |
| 配置 | serde_json + 原子写，keyring 凭据 |
| TLS 自签放行 | rustls danger-mode 自定义 verifier |
| 脚本引擎 | rhai（Rust 原生沙箱，骨架已就位） |
| 日志 | tracing + tracing-appender 轮转 + 脱敏 Layer |
| 前端 | Vue 3 + Vite + TypeScript + Pinia + **Ant Design Vue 4** |

## 工程结构

```
crates/
  mqttkit-ipc/     # 前后端契约 DTO（唯一接口层）
  mqttkit-config/  # 配置模型 + JSON 原子写 + keyring + 迁移
  mqttkit-core/    # 核心业务（不依赖 Tauri）：连接状态机/会话/环形缓冲/编解码/rhai
apps/desktop/      # Tauri 2 壳（装配 + 薄命令层 + 菜单/托盘/日志）
ui/                # Vue 3 前端（api 层封装 invoke，业务组件不直接调用）
```

**架构硬约束**：`mqttkit-core` 不依赖 Tauri、rumqttc 类型不泄漏到 core 对外接口——核心可脱离 GUI 单测，也为将来 Web 端（WASM）留扩展位。

## 路线图

| 里程碑 | 内容 | 状态 |
| --- | --- | --- |
| M0 骨架 | workspace + Tauri + Vue + IPC 契约 | ✅ |
| M1 核心链路 | rumqttc 接入、状态机、订阅/发布、消息列表、环形缓冲、批量推送 | ✅ |
| M2 持久化与安全 | config.json 原子写、keyring、日志脱敏、导入导出 | ✅ |
| M3 稳定性 | 重连、订阅重放、背压降级、托盘、单实例 | ✅ |
| M4 增强 | rhai 脚本、数值图表、Topic 树形拓扑、压测、$SYS、i18n | ⏳ 待做 |
| M5 分发 | 深链接、代码签名、安装包（**无自动更新**） | ⏳ 待做 |

## 已确认的设计取舍

- **Web 端暂不实现**（rumqttc 依赖 tokio TcpStream 无法编译到 wasm），架构保留扩展位
- **消息历史不落盘**：内存环形缓冲 + 手动导出，退出即清空（轻量优先）
- **不做主密码二次加密**：凭据仅靠 OS Keychain
- **不做自动更新**：手动下载发布包
- **匿名遥测默认关闭**，需显式授权

## 构建与运行

见 [RUN_WSL.md](RUN_WSL.md)（WSL 环境下的依赖安装、`cargo check`、`cargo tauri dev` / `build` 清单）。
