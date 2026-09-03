//! payload 编解码与多视图渲染。
//!
//! payload 在内部与网络上统一为 `Vec<u8>`；对外展示提供 文本 / Hex / Base64
//! 三种视图，并探测 JSON 以便前端做树形渲染。大 payload 只算预览，不预计算全量。
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

pub fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    B64.decode(s).map_err(|e| e.to_string())
}

pub fn encode_base64(b: &[u8]) -> String {
    B64.encode(b)
}

/// 把字节渲染为 UTF-8 文本（含替换字符）。
pub fn to_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

/// 渲染为十六进制（带空格分隔，每行 16 字节）。
pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && i % 16 == 0 {
            out.push('\n');
        } else if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// 探测 payload 格式，用于前端自动选择视图。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadFormat {
    Json,
    Text,
    Binary,
}

pub fn detect(bytes: &[u8]) -> PayloadFormat {
    if let Ok(s) = std::str::from_utf8(bytes) {
        let t = s.trim_start();
        if (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']')) {
            return PayloadFormat::Json;
        }
        if t.chars().all(|c| !c.is_control() || c == '\n' || c == '\t' || c == '\r') {
            return PayloadFormat::Text;
        }
    }
    PayloadFormat::Binary
}

/// 截断预览（默认 1KB）。
pub fn preview(bytes: &[u8], max: usize) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let truncated = if bytes.len() > max { &bytes[..max] } else { bytes };
    let s = to_text(truncated);
    if bytes.len() > max {
        Some(format!("{s}…"))
    } else {
        Some(s)
    }
}
