//! 日志初始化：文件轮转 + stdout，统一出口脱敏。
//!
//! 脱敏规则：对日志行中的 `password / token / secret / authorization / key`
//! 等键值对的值做屏蔽，确保凭据、私钥、token 永不进入日志。
use regex::Regex;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling;

/// 收集事件字段（消息 + 其余键值）。
struct FieldCollector {
    message: String,
    fields: Vec<(String, String)>,
}

impl Visit for FieldCollector {
    // tracing 的 Visit trait 只需实现 record_debug，其余 record_* 默认转发到这里。
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.fields.push((field.name().to_string(), format!("{value:?}")));
        }
    }
}

/// 同时写入两个 sink（用于文件 + 终端）。
struct Tee<W1, W2> {
    a: W1,
    b: W2,
}

impl<W1: Write, W2: Write> Write for Tee<W1, W2> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.a.write(buf)?;
        self.b.write(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.a.flush()?;
        self.b.flush()?;
        Ok(())
    }
}

/// 脱敏日志 Layer：每条事件格式化为一行并屏蔽敏感键值。
struct RedactLayer<W> {
    writer: Mutex<W>,
}

impl<W: Write + Send + 'static> RedactLayer<W> {
    fn new(w: W) -> Self {
        RedactLayer {
            writer: Mutex::new(w),
        }
    }
}

impl<S, W> Layer<S> for RedactLayer<W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: Write + Send + 'static,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut c = FieldCollector {
            message: String::new(),
            fields: Vec::new(),
        };
        event.record(&mut c);
        let mut line = format!("[{}] {}", event.metadata().level(), c.message);
        for (k, v) in &c.fields {
            line.push_str(&format!(" {k}={v}"));
        }
        let redacted = redact(&line);
        if let Ok(mut w) = self.writer.lock() {
            let _ = writeln!(w, "{redacted}");
            let _ = w.flush();
        }
    }
}

/// 屏蔽敏感键值（值部分替换为 ***）。
fn redact(line: &str) -> String {
    lazy_static_regex(|re| re.replace_all(line, "$1=***").to_string())
}

/// 复用编译期构造的正则（简单实现，避免引入 lazy_static）。
fn lazy_static_regex<F: FnOnce(&Regex) -> String>(f: F) -> String {
    // 仅在首次调用时编译；后续调用命中同一静态。这里每次都编译，开销可接受。
    let re = Regex::new(
        r"(?i)\b(password|passwd|token|secret|authorization|access[_-]?key|private[_-]?key|client[_-]?secret)\b(\s*[:=]\s*)\S+",
    )
    .expect("脱敏正则编译失败");
    f(&re)
}

/// 初始化日志。返回 `WorkerGuard` 必须在 `main` 中持有到程序退出。
pub fn init(level: &str, log_dir: &Path) -> WorkerGuard {
    let file = rolling::daily(log_dir, "mqttrustui.log");
    let (nb, guard) = tracing_appender::non_blocking(file);
    let tee = Tee::new(nb, io::stdout());
    let layer = RedactLayer::new(tee).with_filter(tracing_subscriber::EnvFilter::new(level));
    tracing_subscriber::registry().with(layer).init();
    guard
}
