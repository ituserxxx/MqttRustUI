# 在 WSL 中构建与运行 MqttRustUI

> 本机（Windows）只改代码，以下命令全部在你的 WSL（Ubuntu/Debian 系）里执行。
> 路径假定你把仓库放在 WSL 可见的位置（如 `~/code/MqttRustUI` 或 `/mnt/...`）。

## 0. 前置依赖

```bash
# Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable

# 系统库（Tauri 2 + WebKitGTK 必需）
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential \
  curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev \
  librsvg2-dev pkg-config

# Node（前端）。推荐 20 LTS
sudo apt install -y nodejs npm
# 或用 nvm： nvm install 20 && nvm use 20

# Tauri CLI
cargo install tauri-cli --version "^2"
```

## 1. 准备应用图标（Tauri 打包必需）

`tauri build` 需要图标资源。先用任意一张 1024×1024 PNG 生成全套：

```bash
cd apps/desktop/src-tauri
cargo tauri icon /path/to/your-icon-1024.png   # 会生成 src-tauri/icons/*
```

> 仅 `tauri dev` 可暂不生成；但建议一次性生成，免得后面打包报错。

## 2. 安装前端依赖并构建

```bash
cd ui
npm install
npm run build      # 产出 ui/dist（Tauri 的 frontendDist）
```

## 3. 校验 Rust 编译

```bash
# 在仓库根目录
cargo check         # 只做类型检查，最快；Rust 侧大约 1–3 分钟首次
```

若 `cargo check` 报 rumqttc TLS / 代理相关错误，见末尾「已知需核对的点」。

## 4. 开发模式（热重载前端 + Rust 重新编译）

```bash
cd apps/desktop/src-tauri
cargo tauri dev
```

首次会下载并编译大量 crate，耗时较长。成功后自动打开窗口。

## 5. 打包发布（无自动更新，手动安装包）

```bash
cd apps/desktop/src-tauri
cargo tauri build
# 产物在： target/release/bundle/ （deb / AppImage / rpm）
```

## 6. 连接本地 broker 自测

```bash
# 任选其一启动一个本地 broker（另开终端）
docker run -it --rm -p 1883:1883 eclipse-mosquitto:2
# 或： mosquitto -c /etc/mosquitto/mosquitto.conf
```

在应用里「新建连接」：host=`127.0.0.1`、port=`1883`、协议=`mqtt`、协议版本选 3.1.1 或 5.0，
保存后点「连接」，再订阅 `sensors/#` 并发布测试消息。

## 已知需核对的点（按你装的 rumqttc 版本微调 `crates/mqttkit-core/src/protocol.rs`）

1. **TLS 构造器**：本代码用的是 `Transport::tls_with_config(TlsConfiguration { ca, client_auth, alpn })`（rumqttc 0.25 形态）。
   若你的版本是 `tls_with_selfsigned_certs(...)` 或其他签名，改 `build_transport` 即可，core 其余不动。
2. **代理**：`opts.set_proxy(Proxy::http(&url))`。个别版本是 `Proxy::http(String)`，按编译器提示调整。
3. **协议版本**：`opts.set_protocol(Protocol::MQTT3_1_1 / MQTT5)`。若版本无 `set_protocol`，改用对应构造参数。
4. **`AsyncClient::disconnect()`**：用于主动断开，个别旧版为 `client.disconnect()` 返回 `Request`，按提示改 `await` 即可。
5. **Tauri 权限**：`capabilities/default.json` 只开了最小权限；若用到 `global-shortcut` 等，记得补对应 permission。

> 日志脱敏、配置原子写、keyring 容错等都已实现，`cargo check` 通过即可进入功能联调。
