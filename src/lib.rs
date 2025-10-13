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
use std::path::{Path, PathBuf};

use crate::config::LogConfig;

/// Application result type for consistent error handling
pub type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Initialize tracing subscriber for logging
pub fn init_logging(level: &str, log_config: &LogConfig) -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let (directory, file_prefix) = resolve_log_destination(&log_config.file_path);

    std::fs::create_dir_all(&directory)
        .with_context(|| format!("Failed to create log directory: {}", directory.display()))?;

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::HOURLY)
        .filename_prefix(file_prefix.clone())
        .build(&directory)
        .with_context(|| {
            format!(
                "Failed to initialize hourly log file writer at {}/{}",
                directory.display(),
                file_prefix
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
                .with_ansi(false),
        )
        .init();

    Ok(())
}

fn resolve_log_destination(path: &str) -> (PathBuf, String) {
    let requested = Path::new(path);

    let treat_as_directory = path.ends_with('/')
        || path.ends_with('\\')
        || requested.file_name().is_none()
        || matches!(
            requested.file_name().and_then(|f| f.to_str()),
            Some(".") | Some("..")
        );

    if treat_as_directory {
        let directory = if path.trim().is_empty() {
            PathBuf::from(".")
        } else {
            requested.to_path_buf()
        };
        return (directory, "xtrade.log".to_string());
    }

    if let Some(file_name) = requested.file_name().and_then(|f| {
        let name = f.to_string_lossy();
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }) {
        let directory = requested
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        (directory, file_name)
    } else {
        (requested.to_path_buf(), "xtrade.log".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_log_destination;
    use std::path::PathBuf;

    #[test]
    fn defaults_to_current_directory_when_path_is_empty() {
        let (directory, file_name) = resolve_log_destination("");

        assert_eq!(directory, PathBuf::from("."));
        assert_eq!(file_name, "xtrade.log");
    }

    #[test]
    fn preserves_explicit_file_name() {
        let (directory, file_name) = resolve_log_destination("/var/log/xtrade/output.log");

        assert_eq!(directory, PathBuf::from("/var/log/xtrade"));
        assert_eq!(file_name, "output.log");
    }

    #[test]
    fn treats_directory_paths_as_log_roots() {
        let (directory, file_name) = resolve_log_destination("logs/");

        assert_eq!(directory, PathBuf::from("logs/"));
        assert_eq!(file_name, "xtrade.log");
    }
}
