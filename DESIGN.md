# MqttRustUI 架构设计方案

> Rust 负责 MQTT 通信与数据安全，前端负责交互与展示的跨平台 MQTT 桌面客户端。
> **版本：v1.0** · 状态：决策已定，待实施
>
> **本次范围**：仅桌面端（Windows / macOS / Linux）。Web 端暂不实现，但架构上保留扩展位（见 §9），届时无需重构。
>
> **已确认决策**：Ant Design Vue · MQTT 3.1.1 与 5.0 同时支持 · 不做主密码二次加密 · 不做自动更新 · 匿名遥测默认关闭且需显式授权。

---

## 0. 定位与差异化

对标 MQTTX 与 MQTT Explorer（均为 Electron）。差异化不在功能堆叠，而在三点硬指标：

| 维度 | MQTTX / MQTT Explorer | MqttRustUI 目标 |
| --- | --- | --- |
| 体积与内存 | Electron 全家桶，100MB+ 安装包 / 200MB+ 内存 | Tauri 2，安装包 8-15MB，空闲内存 40-80MB |
| 高频消息 | 千级消息后 UI 明显掉帧 | 10 万条消息常驻，批量推送 + 虚拟滚动不卡顿 |
| 本地安全 | 凭据明文存本地配置 | OS Keychain 存储，配置文件可安全备份与提交 |

---

## 1. 技术选型

| 层 | 选型 | 版本/备注 |
| --- | --- | --- |
| 桌面壳 | **Tauri 2** | 系统 WebView + Rust 后端，2.x 已稳定 |
| MQTT 客户端 | **rumqttc** | 0.25+，features: `use-rustls`、`websocket`、`proxy` |
| 异步运行时 | tokio | 多线程 runtime |
| 配置持久化 | **serde_json + tokio::fs** | 纯 JSON，无数据库 |
| 凭据存储 | **keyring** | OS 原生凭据库 |
| 加密 | argon2 + chacha20poly1305 | 主密码二次加密、配置导出 |
| 脚本引擎 | **rhai** | Rust 原生沙箱，payload 编解码与消息模拟 |
| 日志 | tracing + tracing-appender | 文件轮转 + 统一脱敏 |
| 前端 | **Vue 3 + Vite + TypeScript + Pinia** | |
| 虚拟滚动 | 自研或 vue-virtual-scroller | 消息列表核心组件 |
| 图表 | ECharts | 数值型 payload 时序图 |

**Tauri 插件**：`single-instance`（单实例）、`dialog`（文件对话框）、`notification`（通知）、`opener`（打开外部链接）。

> 已确认：**不做自动更新**、**不做主密码二次加密**。凭据仅靠 OS Keychain 保护；配置导出为明文 JSON（配置内只含 `credential_ref` 引用，无敏感明文，可安全分享）。

**为什么不用 Electron / Wails**：Electron 体积不达标；Wails 后端是 Go，与"Rust 负责通信"冲突。

**Tauri 的代价**：WebView 内核因系统而异（Windows WebView2 / macOS WKWebView / Linux WebKitGTK），需建立三平台渲染测试矩阵，Linux 需声明 WebKitGTK 依赖。

---

## 2. 整体架构

### 2.1 分层

```
┌─────────────────────────────────────────────────────────┐
│  前端 UI  ·  Vue 3 + TypeScript + Pinia                  │
│  三栏布局：连接列表 / Topic 树 / 消息区 + 发布区             │
└─────────────────────────────────────────────────────────┘
                    ↕  Tauri IPC（invoke + event）
┌─────────────────────────────────────────────────────────┐
│  IPC 命令层（薄）  ·  仅做参数校验与契约转换                 │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│  Rust 核心（mqttkit-core）  ·  不依赖 Tauri，可独立单测     │
│  连接状态机 / 会话 / 环形缓冲 / Payload 编解码 / rhai 脚本   │
└─────────────────────────────────────────────────────────┘
             ↓                              ↓
   ┌──────────────────┐          ┌────────────────────┐
   │ rumqttc          │          │ config.json        │
   │ TCP/TLS/WS/代理   │          │ + OS Keychain      │
   └──────────────────┘          └────────────────────┘
```

**关键约束：`mqttkit-core` 不依赖 Tauri。** 这样核心逻辑可以脱离 GUI 做纯 Rust 单元测试与集成测试（起一个本地 broker 跑端到端），不必启动 WebView。

### 2.2 工程结构

```
MqttRustUI/
├── Cargo.toml                      # workspace 根
├── crates/
│   ├── mqttkit-core/               # 核心业务逻辑（不依赖 Tauri）
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── conn/
│   │       │   ├── mod.rs          # ConnectionManager：多连接生命周期
│   │       │   ├── state.rs        # 连接状态机
│   │       │   └── retry.rs        # 指数退避重连
│   │       ├── session.rs          # 订阅表、QoS inflight 跟踪
│   │       ├── buffer.rs           # 环形缓冲区
│   │       ├── topic.rs            # topic 匹配与树构建
│   │       ├── codec/              # payload 编解码（JSON/Hex/Base64/MsgPack/Protobuf）
│   │       ├── script.rs           # rhai 引擎封装
│   │       └── stats.rs            # 收发速率统计
│   ├── mqttkit-config/             # 配置与凭据
│   │   └── src/
│   │       ├── model.rs            # AppConfig 数据结构
│   │       ├── persist.rs          # JSON 原子写 + debounce
│   │       ├── vault.rs            # keyring 保险库
│   │       └── migrate.rs          # schema_version 迁移
│   └── mqttkit-ipc/                # 前后端契约 DTO
│       └── src/{command.rs,event.rs}
├── apps/desktop/
│   └── src-tauri/
│       ├── src/
│       │   ├── main.rs             # Tauri 装配
│       │   ├── commands.rs         # #[tauri::command] 薄层
│       │   ├── menu.rs             # 原生菜单
│       │   ├── tray.rs             # 系统托盘
│       │   ├── deep_link.rs        # mqtt:// scheme
│       │   └── logging.rs          # tracing + 脱敏 Layer
│       ├── capabilities/default.json
│       ├── tauri.conf.json
│       └── icons/
└── ui/
    └── src/
        ├── api/                    # invoke 封装（按领域分文件）
        ├── stores/                 # Pinia: connections / messages / topicTree / settings
        ├── views/
        ├── components/
        └── codecs/                 # payload 渲染器注册表
```

依赖方向严格单向：`ipc ← core ← config`，`apps/desktop` 只做装配。

相比 v0.1 方案，crate 从 6 个精简到 3 个——不再需要 `mqttkit-protocol`（直接用 rumqttc 的编解码）、`mqttkit-transport`（直接用 rumqttc）、`mqttkit-store`（改为 config）。

---

## 3. 核心设计一：连接生命周期

### 3.1 状态机

```
                     connect()
      ┌──────┐ ─────────────────────▶ ┌────────────┐
      │ Idle │                        │ Connecting │
      └──────┘ ◀─────────────────────  └────────────┘
         ▲        disconnect()              │  │
         │                        CONNACK 收到│  │ 超时/拒绝
         │                                  ▼  ▼
         │                            ┌───────────┐
         │                            │ Connected │◀───────┐
         │                            └───────────┘        │
         │                                  │              │ 重连成功
         │                    网络异常 / 心跳超时 │              │
         │                                  ▼              │
         │                          ┌──────────────┐       │
         │                          │ Reconnecting │───────┘
         │                          └──────────────┘
         │                                  │
         │                          重试次数耗尽
         │                                  ▼
         │                            ┌───────────┐
         └──── disconnect() ──────────│  Failed   │
                                      └───────────┘
```

| 状态 | 含义 | UI 表现 |
| --- | --- | --- |
| Idle | 未连接 | 灰色圆点，"连接"按钮可用 |
| Connecting | TCP/TLS 握手 + CONNECT 发送中 | 黄色旋转图标，超时 10s |
| Connected | CONNACK 已收到，会话就绪 | 绿色圆点，显示会话时长 |
| Reconnecting | 断线自动重连中 | 橙色闪烁，显示"第 N 次重连 · Xs 后" |
| Failed | 重连耗尽，需用户干预 | 红色圆点，"重试"按钮 |
| Disconnecting | 主动断开中 | 灰色，短暂 |

### 3.2 重连策略

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| 初始间隔 | 1s | |
| 退避倍数 | 1.8 | 1s → 1.8s → 3.24s → 5.8s ... |
| 最大间隔 | 30s | 达到后保持 30s |
| 抖动 | ±20% | 避免大量客户端同时重连造成惊群 |
| 最大重试 | 10 次 | 超出进入 Failed |
| 单连超时 | 10s | CONNECT 后等 CONNACK 的上限 |

**重连成功后的行为差异**（这里容易做错）：

- `clean_session = true`：broker 丢弃会话，客户端必须**重新 SUBSCRIBE** 所有订阅。应用层需缓存订阅表并在重连后自动重放。
- `clean_session = false`（MQTT 5 为 `session_expiry_interval > 0`）：broker 保留订阅与未确认消息，客户端**不重放 SUBSCRIBE**，否则会产生重复订阅。

实现上：在 `session.rs` 维护每个连接的订阅表，重连时根据 clean_session 标志决定是否重放。

**QoS inflight**：协议层的重传由 rumqttc 的 EventLoop 内部维护，重连后自动重发未确认的 PUBLISH/PUBREL。应用层不额外持久化——未确认消息仅存内存，进程退出即丢弃，这与"消息历史不落盘"的取舍一致。

---

## 4. 核心设计二：消息流与性能

### 4.1 数据通路

```
  网络 ──▶ rumqttc EventLoop ──▶ 解码 + 分类 ──▶ 环形缓冲（10 万条/连接）
                                                        │
                                                  批量 flush
                                              （50ms / 200 条 双阈值）
                                                        ▼
                                              Tauri emit ──▶ WebView
                                                        │
                                                  虚拟滚动渲染
                                                 （仅渲染可视 ~30 行）
```

**为什么必须批量推送**：逐条 emit 会在高频场景（>1000 msg/s）下把 IPC 通道和 WebView 主线程直接打爆，这是同类工具卡顿的根本原因。批量化是本方案最重要的性能决策。

### 4.2 环形缓冲

```rust
pub struct MessageBuffer {
    inner: VecDeque<StoredMessage>,
    capacity: usize,      // 默认 100_000，可在 settings 配置
    dropped: u64,         // 累计丢弃数，用于 UI 提示
}

impl MessageBuffer {
    fn push(&mut self, m: StoredMessage) {
        if self.inner.len() == self.capacity {
            self.inner.pop_front();
            self.dropped += 1;
        }
        self.inner.push_back(m);
    }
}
```

- **按连接隔离**，每个连接有独立缓冲区
- 内存估算：单条约 200B 开销 + payload，10 万条约 20-50MB，可接受
- 溢出时丢弃**最旧**消息并累加 `dropped`，UI 在状态栏提示已丢弃条数

### 4.3 背压三档

| 条件 | 策略 |
| --- | --- |
| 使用率 < 70% | 正常：50ms 或满 200 条触发一次 flush |
| 70% - 95% | 加快 flush：间隔降到 20ms，尽快让 UI 消费 |
| > 95% | 主动丢弃最旧消息腾空间，emit Stats 提示"缓冲区接近上限" |
| IPC 队列积压 > 5000 | **降级**：只推 Stats 不发明细，UI 顶部横幅提示"消息速率过快，已暂停明细推送" |

降级是自动可逆的——积压回落到 1000 以下时恢复明细推送。

### 4.4 大 payload

- 预览截断：列表与详情默认只渲染前 1KB，超出显示"…（共 N 字节，点击加载全部）"
- 超过 64KB 的 payload 不参与自动渲染，仅保留引用
- payload 作为 `Vec<u8>` 传入，二进制安全；Hex 视图按需格式化，不预计算

---

## 5. 核心设计三：数据模型

### 5.1 配置文件

位置：`{app_data_dir}/config.json`

```json
{
  "schema_version": 1,
  "settings": {
    "theme": "auto",
    "locale": "zh-CN",
    "message_buffer": 100000,
    "flush_interval_ms": 50,
    "flush_batch": 200,
    "log_level": "info",
    "minimize_to_tray": true,
    "auto_update_channel": "stable"
  },
  "connections": [
    {
      "id": "0f3c1a2e",
      "name": "本地 broker",
      "protocol": "mqtt",
      "host": "127.0.0.1",
      "port": 1883,
      "path": null,
      "client_id": "mqttkit-9f2a",
      "username": "dev",
      "credential_ref": "keyring://conn/0f3c1a2e",
      "mqtt_version": "v311",
      "clean_session": true,
      "keep_alive": 60,
      "tls": { "mode": "none" },
      "proxy": null,
      "last_will": null,
      "properties": null,
      "auto_connect": false,
      "subscriptions": [
        { "filter": "sensors/#", "qos": 1, "color": "#378ADD", "enabled": true }
      ]
    }
  ],
  "scripts": [
    { "id": "a1", "name": "温度解析", "lang": "rhai", "source": "..." }
  ],
  "bookmarks": [
    { "topic": "sensors/temp", "note": "车间温度" }
  ]
}
```

TLS 配置变体：

```json
{ "mode": "none" }
{ "mode": "tls", "ca": null, "client_auth": null, "verify_hostname": true }
{ "mode": "tls", "ca": "/path/ca.pem",
  "client_auth": { "cert": "/path/client.pem", "key": "/path/client.key" },
  "verify_hostname": true }
```

### 5.2 原子写与容错

```rust
async fn save(&self, cfg: &AppConfig) -> Result<()> {
    let tmp = self.path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(cfg)?;
    tokio::fs::write(&tmp, json).await?;
    tokio::fs::rename(&tmp, &self.path).await?;   // 原子替换
    Ok(())
}
```

- 写操作 debounce 500ms，合并高频修改
- 启动加载失败时：备份为 `config.json.bak`，回落默认配置，**不阻断启动**，并在 UI 提示已备份路径
- `schema_version` 配合 `migrate.rs` 逐级联迁（v1 → v2 → v3）

### 5.3 凭据

`credential_ref` 只是引用，真值在 OS Keychain：

| 平台 | 实现 |
| --- | --- |
| Windows | Credential Manager（DPAPI） |
| macOS | Keychain |
| Linux | Secret Service（libsecret） |

目标环境无可用保险库时降级为配置内加密字段（Argon2id + XChaCha20-Poly1305），但仍不存明文。

**配置文件因此可以安全地备份、同步网盘甚至提交进版本库。**

---

## 6. 数据安全

| 层面 | 措施 |
| --- | --- |
| 凭据存储 | OS Keychain；**不做主密码二次加密**（已确认） |
| 传输 | 默认 rustls + webpki 根证书；支持单向/双向 TLS、PEM 与 PKCS#12；支持 WS/WSS 与 HTTP 代理 |
| 密钥格式 | rumqttc 0.24+ 自动识别 PKCS#1 / PKCS#8 / SEC1 / RFC5915，无需用户指定类型 |
| 日志 | 统一出口脱敏：正则遮蔽 password / token / secret / authorization 及 PEM 私钥块 |
| 配置导出 | 明文 `config.json` 直接导出（仅含 `credential_ref` 引用，无敏感明文）；无 Keychain 环境下凭据仅存内存 |
| Tauri 权限 | capabilities 最小权限；配置 CSP；生产构建剥离 devtools |
| 更新 | **不做自动更新**（已确认），改为手动下载发布包 |

capabilities 示例（Tauri 2）：

```json
{
  "identifier": "default",
  "description": "最小权限",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:allow-start-dragging",
    "core:window:allow-minimize",
    "core:window:allow-close",
    "dialog:allow-open",
    "dialog:allow-save",
    "notification:default",
    {
      "identifier": "fs:allow-read-file",
      "allow": [{ "path": "$APPDATA/**" }, { "path": "$HOME/.config/**" }]
    }
  ]
}
```

---

## 7. 桌面端原生能力

这些是 Web 端方案给不了、也是本阶段可以放心做深的部分：

| 能力 | 实现 | 说明 |
| --- | --- | --- |
| 系统托盘 | `tray.rs` | 托盘图标显示连接状态角标；菜单含"显示主窗口 / 全部断开 / 退出" |
| 原生菜单 | `menu.rs` | 三平台标准菜单（文件/编辑/视图/帮助），macOS 走应用菜单惯例 |
| 全局快捷键 | Tauri global-shortcut | 新建连接、快速发布、清屏、切换连接 |
| 单实例 | `tauri-plugin-single-instance` | 二次启动时聚焦已有窗口，并可传递深链接参数 |
| 深链接 | 注册 `mqtt://` scheme | 支持 `mqtt://user@host:1883/topic` 一键建连，便于文档与工单里点击 |
| 文件关联 | `.mqttkit` 双击打开 | 导入配置包 |
| 窗口记忆 | 持久化尺寸与位置 | 最小 1024×680；关闭主窗口最小化到托盘（可配置） |
| 通知 | `notification` 插件 | 连接断开、脚本执行完毕等事件提醒（可关闭） |
| 日志落盘 | tracing-appender | 日志轮转存于 `app_log_dir`，设置页可一键打开目录 |

---

## 8. UI 布局与交互

### 8.1 三栏布局

```
┌─────────────┬─────────────────────────────────────────────┐
│  连接列表    │  ┌───────────────────────────────────────┐  │
│  (240px)    │  │ 工具栏：状态 · 收发速率 · 搜索 · 清屏    │  │
│             │  ├──────────────┬────────────────────────┤  │
│ ● 本地       │  │ Topic 树      │  消息列表              │  │
│ ● 测试环境   │  │ (260px)       │  （虚拟滚动）           │  │
│ ○ 生产      │  │               │                        │  │
│             │  │ sensors/      │  10:23:01 sensors/temp │  │
│ [+ 新建连接] │  │  ├ temp      │  10:23:02 sensors/humi │  │
│             │  │  └ humi      │  10:23:03 sensors/temp │  │
│             │  ├──────────────┴────────────────────────┤  │
│             │  │ 发布区：topic / payload / QoS / Retain  │  │
└─────────────┴──┴───────────────────────────────────────┴──┘
```

### 8.2 关键交互

- **多连接多标签**：左侧连接列表支持多连接并存，右侧以标签页切换工作区
- **消息过滤**：支持按 topic 通配符、payload 文本、正则搜索；过滤在 Rust 侧执行，UI 只拿结果
- **Payload 多视图**：自动探测格式，提供 树形 JSON / 原始文本 / Hex / Base64 视图切换
- **订阅着色**：每个订阅可指定颜色，消息列表中同色标记，便于肉眼区分
- **payload 模板**：支持 `{{ts}}` `{{random 1 100}}` `{{uuid}}` 等占位符，配合定时发布做设备模拟
- **导出**：当前视图导出为 CSV / JSON / JSONL

---

## 9. 为 Web 端预留的扩展位

现阶段不实现，但架构上有三处刻意留的口子，将来接 Web 端不需要重构：

1. **`mqttkit-core` 不依赖 Tauri** —— 可以整体编译到 wasm，核心逻辑零改动
2. **前端通过 `api/` 层调用后端** —— 将来只需替换 `api/` 的底层实现（Tauri invoke → Worker postMessage），Vue 组件不用动
3. **配置层用 trait 抽象** —— `trait ConfigStore` 已有 `persist` 实现，将来加一个浏览器 Storage 实现即可

需要注意的前置约束：Web 端无法使用 rumqttc（依赖 tokio TcpStream），届时传输层需要改用 `mqttbytes`（纯编解码 crate）+ 自研 WebSocket transport。core 层若保持"不直接引用 rumqttc 类型、只用自己的领域模型"，这层替换会平滑很多——**现在就应避免让 rumqttc 的类型泄漏到 core 的对外接口里**。

---

## 10. 功能模块与优先级

| 模块 | 功能点 | 优先级 |
| --- | --- | --- |
| 连接管理 | 多连接并存、多标签、增删改、克隆、导入导出 | P0 |
| 连接参数 | MQTT 3.1.1 / 5.0、ClientID、用户名密码、Keep Alive、Clean Session、遗嘱 | P0 |
| 安全连接 | TLS 单向 / 双向、CA 与自签证书、WS/WSS、HTTP 代理 | P0 |
| 订阅管理 | Topic 过滤器、QoS、通配符高亮、按订阅着色 | P0 |
| 消息发布 | QoS 0/1/2、Retain、payload 编辑、历史模板 | P0 |
| 消息展示 | 虚拟滚动、时间/Topic/QoS/Retain、过滤与正则搜索 | P0 |
| 重连 | 自动重连、指数退避、订阅表重放 | P0 |
| Payload 编解码 | Plain / JSON / Hex / Base64 / MsgPack | P0 |
| 配置持久化 | config.json 原子写、keyring 凭据、schema 迁移 | P0 |
| 桌面集成 | 托盘、原生菜单、单实例、窗口记忆 | P1 |
| 高级连接 | MQTT 5 属性（会话/消息过期、User Properties、Topic Alias）、共享订阅 `$share` | P1 |
| 数据可视化 | 数值型 payload 时序图、Topic 树形拓扑 | P1 |
| 数据导出 | CSV / JSON / JSONL | P1 |
| 脚本模拟 | rhai 脚本、定时发布、多连接并发压测、速率统计 | P2 |
| 调试增强 | Retained 值差异对比、`$SYS` 自动订阅、连接日志 | P2 |
| 分发 | 深链接、自动更新、文件关联、i18n（中/英） | P2 |

**MVP（P0）目标**：一个能连、能订阅、能发、能看、能存、凭据安全、断线能自动恢复的可用客户端。

---

## 11. 开发路线图

| 里程碑 | 内容 | 产出 |
| --- | --- | --- |
| **M0 骨架** | workspace 初始化、Tauri 跑通、Vue 壳、IPC 契约、CI（三平台构建） | 空壳可在三平台启动 |
| **M1 核心链路** | rumqttc 接入、连接状态机、订阅/发布、消息列表、虚拟滚动、环形缓冲 | **可连可用** |
| **M2 持久化与安全** | config.json 原子写、keyring、日志脱敏、配置导入导出 | 数据安全达标 |
| **M3 稳定性** | 指数退避重连、订阅重放、背压降级、窗口记忆、托盘、单实例 | **MVP 发布** |
| **M4 增强** | rhai 脚本、图表、Topic 树、压测、$SYS、i18n | 完整功能 |
| **M5 分发** | 深链接、代码签名、安装包（**无自动更新**） | 正式发布 |

---

## 12. 风险登记册

| 风险 | 影响 | 概率 | 应对 |
| --- | --- | --- | --- |
| 高频消息打爆 UI | 卡顿、假死 | 高 | 批量 flush + 环形缓冲 + 虚拟滚动 + 三档背压降级 |
| 重连后订阅丢失 | 收不到消息 | 中 | 应用层缓存订阅表，按 clean_session 决定重放 |
| 配置文件损坏 | 配置丢失 | 低 | 原子写（tmp + rename）+ 解析失败自动 `.bak` 备份并回落默认 |
| WebView 内核差异 | 渲染不一致 | 中 | 锁定 WebView2 最低版本，建立三平台渲染测试矩阵 |
| keyring 在某些 Linux 环境不可用 | 凭据无法保存 | 中 | 检测失败时自动降级为配置内加密字段，并提示用户 |
| 证书密钥格式不兼容 | 连接失败 | 中 | 依赖 rustls 多格式支持，错误提示中给出格式检测结果 |
| 打包与代码签名 | 分发受阻 | 中 | 优先跑通 Windows + macOS 签名，Linux 走 AppImage / deb |
| tokio 与 Tauri 运行时冲突 | 启动异常 | 低 | 使用 Tauri 托管的 tokio runtime，不自建 runtime |

---

## 13. 已确认决策（原待拍板）

1. **前端组件库** —— ✅ Ant Design Vue
2. **协议范围** —— ✅ MQTT 3.1.1 与 5.0 同时支持（rumqttc `set_protocol` 切换，会话策略按 `clean_session` / `session_expiry_interval` 区分重放）
3. **主密码** —— ❌ 不做二次加密；凭据仅靠 OS Keychain
4. **自动更新** —— ❌ 不做；改为手动下载发布包
5. **遥测** —— 默认关闭，需用户在设置里显式开启（`settings.telemetry_enabled`，默认 `false`）

> 当前已落地的代码范围：**M0 骨架 + M1 核心链路**（workspace / 3 个 crate / Tauri 2 壳 / Vue 前端 + 连接状态机 + 指数退避重连 + 订阅重放 + 环形缓冲 + 批量 flush + 三档背压 + 配置原子写 + keyring + 日志脱敏）。M2–M5 按路线图推进。
