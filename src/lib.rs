//! XTrade Market Data Monitor Library
//!
//! A high-performance cryptocurrency market data monitoring system
//! built with Rust, focusing on real-time data processing and display.

pub mod binance;
pub mod cli;
pub mod config;
pub mod market_data;
pub mod metrics;
pub mod session;
pub mod ui;

use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};

use crate::config::LogConfig;

/// Application result type for consistent error handling
pub type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Initialize tracing subscriber for logging
pub fn init_logging(level: &str, log_config: &LogConfig) -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    let directory = &log_config.file_path;
    std::fs::create_dir_all(directory)
        .with_context(|| format!("Failed to create log directory: {}", directory))?;

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::HOURLY)
        .filename_prefix("xtrade.log")
        .build(directory)
        .with_context(|| {
            format!(
                "Failed to initialize hourly log file writer at {}",
                directory
            )
        })?;

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("xtrade={}", level).into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_timer(LocalTimer),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(BufferMakeWriter)
                .with_ansi(false)
                .with_timer(LocalTimer),
        )
        .init();

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct LocalTimer;

impl tracing_subscriber::fmt::time::FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%Y-%m-%d %H:%M:%S%.3f"))
    }
}

/// Retrieve a snapshot of recent log messages captured in memory.
pub fn recent_logs(limit: usize) -> Vec<String> {
    log_buffer().recent(limit)
}

fn log_buffer() -> &'static LogBuffer {
    static LOG_BUFFER: OnceLock<LogBuffer> = OnceLock::new();
    LOG_BUFFER.get_or_init(|| LogBuffer::new(512))
}

struct LogBuffer {
    inner: Mutex<VecDeque<String>>,
    capacity: usize,
}

impl LogBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    fn push(&self, entry: String) {
        if entry.is_empty() {
            return;
        }

        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.push_back(entry);
        while guard.len() > self.capacity {
            guard.pop_front();
        }
    }

    fn recent(&self, limit: usize) -> Vec<String> {
        let guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.is_empty() || limit == 0 {
            return Vec::new();
        }

        let len = guard.len();
        let start = len.saturating_sub(limit);
        guard.iter().skip(start).cloned().collect()
    }
}

struct BufferMakeWriter;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufferMakeWriter {
    type Writer = BufferWriter;

    fn make_writer(&'a self) -> Self::Writer {
        BufferWriter { buffer: Vec::new() }
    }
}

struct BufferWriter {
    buffer: Vec<u8>,
}

impl BufferWriter {
    fn flush_into_log_buffer(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        match std::str::from_utf8(&self.buffer) {
            Ok(content) => {
                let formatted = content.trim_end_matches('\n').trim_end();
                if !formatted.is_empty() {
                    log_buffer().push(formatted.to_string());
                }
            }
            Err(_) => {
                // Ignore invalid UTF-8 payloads for the in-memory log buffer.
            }
        }

        self.buffer.clear();
        Ok(())
    }
}

impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_into_log_buffer()
    }
}

impl Drop for BufferWriter {
    fn drop(&mut self) {
        let _ = self.flush_into_log_buffer();
    }
}
