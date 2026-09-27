//! Logging: `tracing` console + daily-rotated file + in-memory ring for the GUI.
//!
//! - File location: `%APPDATA%/Mocoslime/logs/mocoslime.log` (daily rotation,
//!   e.g. `mocoslime.log.2026-09-05`; only the last 7 files are kept).
//! - GUI: [`LogBuffer`] keeps the last 500 entries; the FFI layer drains it
//!   via `mocoslime_poll_logs()` so tracking throughput is never blocked.
//! - Privacy: only level/target/message are stored. No Bluetooth addresses,
//!   quaternions or raw packet bytes are logged.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};
use tracing::{level_filters::LevelFilter, Level};
use tracing_subscriber::{
    filter::Targets, layer::SubscriberExt, reload, util::SubscriberInitExt, Layer,
};

/// One GUI-visible log line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// RFC 3339 timestamp.
    pub timestamp: String,
    /// `TRACE`..`ERROR`.
    pub level: String,
    /// Tracing target (module path).
    pub target: String,
    /// Rendered message (fields appended as `key=value`, packet bytes never).
    pub message: String,
}

/// Thread-safe ring buffer (cap 500) shared with the FFI poll function.
#[derive(Debug, Clone, Default)]
pub struct LogBuffer {
    inner: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, entry: LogEntry) {
        if let Ok(mut guard) = self.inner.lock() {
            if guard.len() >= 500 {
                guard.pop_front();
            }
            guard.push_back(entry);
        }
    }

    /// Drain all buffered entries (GUI poll).
    pub fn drain(&self) -> Vec<LogEntry> {
        if let Ok(mut guard) = self.inner.lock() {
            guard.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Non-draining copy of the newest `limit` entries (diagnostics bundle).
    pub fn snapshot(&self, limit: usize) -> Vec<LogEntry> {
        if let Ok(guard) = self.inner.lock() {
            let skip = guard.len().saturating_sub(limit);
            guard.iter().skip(skip).cloned().collect()
        } else {
            Vec::new()
        }
    }
}

/// `tracing` layer forwarding formatted events into the [`LogBuffer`].
struct RingLayer {
    buffer: LogBuffer,
}

impl<S> Layer<S> for RingLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let meta = event.metadata();
        self.buffer.push(LogEntry {
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level: meta.level().to_string(),
            target: meta.target().to_string(),
            message: visitor.message,
        });
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write as _;
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else if !self.message.is_empty() {
            let _ = write!(self.message, " {}={value:?}", field.name());
        } else {
            let _ = write!(self.message, "{}={value:?}", field.name());
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        use std::fmt::Write as _;
        if field.name() == "message" {
            self.message = value.to_string();
        } else if !self.message.is_empty() {
            let _ = write!(self.message, " {}={value}", field.name());
        } else {
            let _ = write!(self.message, "{}={value}", field.name());
        }
    }
}

/// Parse `TRACE|DEBUG|INFO|WARN|ERROR` (case-insensitive, default INFO).
pub fn parse_level(level: &str) -> LevelFilter {
    match level.to_ascii_uppercase().as_str() {
        "TRACE" => LevelFilter::TRACE,
        "DEBUG" => LevelFilter::DEBUG,
        "WARN" | "WARNING" => LevelFilter::WARN,
        "ERROR" => LevelFilter::ERROR,
        _ => LevelFilter::INFO,
    }
}

/// Directory for rotated log files.
pub fn log_dir() -> PathBuf {
    configuration::AppConfig::get_config_dir().join("logs")
}

/// Ensure log and config directories exist.
/// Call this early (e.g., in `mocoslime_init`) to guarantee directories exist.
pub fn ensure_dirs() {
    let config_dir = configuration::AppConfig::get_config_dir();
    let log_dir = log_dir();
    let _ = std::fs::create_dir_all(&config_dir);
    let _ = std::fs::create_dir_all(&log_dir);
}

/// Delete rotated files older than the newest 7 (best effort).
fn prune_old_logs(dir: &std::path::Path) {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("mocoslime.log."))
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|e| e.file_name());
    while files.len() > 7 {
        if let Some(oldest) = files.first() {
            let _ = std::fs::remove_file(oldest.path());
        }
        files.remove(0);
    }
}

/// Reload handle for the global log filter (set once in [`init_logging`]).
static FILTER_HANDLE: OnceLock<reload::Handle<Targets, tracing_subscriber::Registry>> =
    OnceLock::new();

fn base_targets(level: &str) -> Targets {
    Targets::new()
        .with_default(parse_level(level))
        .with_target("h2", Level::WARN)
        .with_target("hyper", Level::WARN)
}

/// Install console + daily file + ring-buffer layers. Safe to call once;
/// later calls only adjust the level filter via [`set_log_level`].
pub fn init_logging(buffer: &LogBuffer, level: &str) {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    prune_old_logs(&dir);

    let file_appender = tracing_appender::rolling::daily(&dir, "mocoslime.log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);
    // Leak the guard for process lifetime so background flushing continues.
    std::mem::forget(_guard);

    // One reloadable filter in front: console, file and GUI ring all
    // follow it, so Settings → Log level applies immediately everywhere.
    let (reload_filter, handle) = reload::Layer::new(base_targets(level));
    let _ = FILTER_HANDLE.set(handle);

    let ring = RingLayer {
        buffer: buffer.clone(),
    };

    let _ = tracing_subscriber::registry()
        .with(reload_filter)
        .with(ring)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(file_writer),
        )
        .try_init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        log_dir = %dir.display(),
        "Mocoslime logging initialized"
    );
}

/// Change verbosity at runtime (called from Settings → Log level).
/// Applies immediately to console, file and GUI ring buffer.
pub fn set_log_level(level: &str) {
    match FILTER_HANDLE.get() {
        Some(handle) => match handle.modify(|targets| *targets = base_targets(level)) {
            Ok(()) => tracing::info!("log level set to {level}"),
            Err(e) => tracing::warn!("log level reload failed: {e}"),
        },
        None => tracing::info!("log level change requested before init ({level})"),
    }
}

/// Panic hook: a Rust panic must never vanish silently. It is logged via
/// `tracing` (console + file + GUI ring) and appended to today's log file
/// directly in case the async file writer is already torn down.
pub fn init_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg = format!("PANIC: {info}");
            tracing::error!("{msg}");
            // Best-effort synchronous append to today's log file.
            let path = log_dir().join(format!(
                "mocoslime.log.{}",
                Utc::now().format("%Y-%m-%d")
            ));
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                use std::io::Write as _;
                // `{:?}` on a String yields a valid JSON string literal.
                let _ = writeln!(
                    file,
                    "{{\"timestamp\":\"{}\",\"level\":\"ERROR\",\"target\":\"panic\",\"message\":{msg:?}}}",
                    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                );
            }
            previous(info);
        }));
    });
}

/// Current time helper for log-adjacent code.
#[allow(dead_code)]
pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_level() {
        assert_eq!(parse_level("debug"), LevelFilter::DEBUG);
        assert_eq!(parse_level("bogus"), LevelFilter::INFO);
    }

    #[test]
    fn test_ring_buffer_caps_at_500() {
        let buf = LogBuffer::new();
        for i in 0..600 {
            buf.push(LogEntry {
                timestamp: "t".into(),
                level: "INFO".into(),
                target: "test".into(),
                message: format!("m{i}"),
            });
        }
        let drained = buf.drain();
        assert_eq!(drained.len(), 500);
        assert_eq!(drained[0].message, "m100");
        assert!(buf.drain().is_empty());
    }

    #[test]
    fn test_snapshot_does_not_drain() {
        let buf = LogBuffer::new();
        for i in 0..10 {
            buf.push(LogEntry {
                timestamp: "t".into(),
                level: "INFO".into(),
                target: "test".into(),
                message: format!("m{i}"),
            });
        }
        let snap = buf.snapshot(4);
        assert_eq!(snap.len(), 4);
        assert_eq!(snap[0].message, "m6");
        // Buffer untouched: full drain still yields all 10.
        assert_eq!(buf.drain().len(), 10);
    }

    #[test]
    fn test_set_log_level_without_init_is_noop() {
        // Must never panic even when init_logging was never called
        // (e.g. in unit tests without a global subscriber).
        set_log_level("DEBUG");
        set_log_level("bogus-level");
    }
}
