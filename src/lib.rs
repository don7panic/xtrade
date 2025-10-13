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
