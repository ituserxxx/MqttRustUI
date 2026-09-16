#!/usr/bin/env bash
# MqttRustUI 一键调试脚本（WSL / Ubuntu / Debian）
#
# 用法：
#   ./start_debug.sh              # 完整流程：检查依赖 → 前端构建 → cargo check → 起本地 broker → tauri dev
#   ./start_debug.sh --no-broker  # 不起本地 mosquitto（你连远程 broker 时用）
#   ./start_debug.sh --check-only # 只做 cargo check 编译检查，不起 GUI
#   ./start_debug.sh --release    # 用 release 模式跑 tauri dev（编译慢但运行快）
#
# 环境变量：
#   RUST_LOG=debug ./start_debug.sh   # 提日志级别（默认 info）
set -euo pipefail

# ---------- 参数 ----------
NO_BROKER=0
CHECK_ONLY=0
DEV_FLAGS=()
for arg in "$@"; do
  case "$arg" in
    --no-broker)  NO_BROKER=1 ;;
    --check-only) CHECK_ONLY=1 ;;
    --release)    DEV_FLAGS+=(--release) ;;
    -h|--help)
      grep '^#' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "未知参数: $arg（用 -h 查看用法）"; exit 1 ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UI_DIR="$ROOT/ui"
TAURI_DIR="$ROOT/apps/desktop/src-tauri"
export RUST_LOG="${RUST_LOG:-info}"

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[warn]\033[0m %s\n' "$*"; }
err()  { printf '\033[1;31m[err]\033[0m %s\n' "$*" >&2; }

# ---------- 0. 依赖自检 ----------
log "检查工具链"
missing=0
for cmd in cargo node npm; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    err "缺少 $cmd"
    missing=1
  fi
done
if ! cargo tauri --version >/dev/null 2>&1; then
  err "缺少 tauri-cli，安装：cargo install tauri-cli --version '^2'"
  missing=1
fi
if ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  err "缺少 webkit2gtk-4.1（Tauri 必需）。安装："
  err "  sudo apt install -y libwebkit2gtk-4.1-dev build-essential libgtk-3-dev \\"
  err "    libayatana-appindicator3-dev librsvg2-dev libssl-dev libsoup-3.0-dev \\"
  err "    libjavascriptcoregtk-4.1-dev pkg-config"
  missing=1
fi
[ "$missing" -eq 1 ] && { err "请先补齐依赖（完整清单见 RUN_WSL.md）"; exit 1; }
log "工具链 OK（rust: $(rustc --version | awk '{print $2}'), node: $(node -v)）"

# ---------- 1. 前端 ----------
log "前端依赖与构建"
cd "$UI_DIR"
if [ ! -d node_modules ] || [ package.json -nt node_modules/.package-lock.json 2>/dev/null ]; then
  npm install
else
  log "node_modules 已是最新，跳过 npm install"
fi
npm run build

# ---------- 2. Rust 编译检查 ----------
log "cargo check（最快暴露编译错误）"
cd "$ROOT"
cargo check

if [ "$CHECK_ONLY" -eq 1 ]; then
  log "check 通过，按 --check-only 结束"
  exit 0
fi

# ---------- 3. 本地 broker（可选） ----------
if [ "$NO_BROKER" -eq 0 ]; then
  if command -v docker >/dev/null 2>&1; then
    if docker ps --format '{{.Names}}' 2>/dev/null | grep -qx mqttkit-mosquitto; then
      log "本地 broker 已在跑（容器 mqttkit-mosquitto）"
    else
      log "启动本地 broker（docker: eclipse-mosquitto:2，端口 1883）"
      docker rm -f mqttkit-mosquitto >/dev/null 2>&1 || true
      docker run -d --name mqttkit-mosquitto -p 1883:1883 eclipse-mosquitto:2 >/dev/null
      log "broker 就绪：127.0.0.1:1883（停止：docker stop mqttkit-mosquitto）"
    fi
  else
    warn "未检测到 docker，跳过本地 broker。请自行启动 mosquitto 或连远程 broker，或用 --no-broker 抑制本提示"
  fi
fi

# ---------- 4. 启动 Tauri dev ----------
# WSL 是 headless：GUI 窗口需要 WSLg（Win11 自带）或 X Server。
if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  warn "未检测到 DISPLAY / WAYLAND_DISPLAY，GUI 窗口可能无法弹出。"
  warn "Win11 的 WSLg 一般已自带；否则请先起 X Server（如 VcXsrv）。"
fi

log "启动 tauri dev（RUST_LOG=$RUST_LOG）"
cd "$TAURI_DIR"
exec cargo tauri dev "${DEV_FLAGS[@]}"
