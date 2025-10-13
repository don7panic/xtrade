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
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::config::LogConfig;

/// Application result type for consistent error handling
pub type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Initialize tracing subscriber for logging
pub fn init_logging(level: &str, log_config: &LogConfig) -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let (directory, file_prefix, file_suffix) = resolve_log_destination(&log_config.file_path);

    std::fs::create_dir_all(&directory)
        .with_context(|| format!("Failed to create log directory: {}", directory.display()))?;

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::HOURLY)
        .filename_prefix(file_prefix.clone())
        .filename_suffix(file_suffix.clone())
        .build(&directory)
        .with_context(|| {
            format!(
                "Failed to initialize hourly log file writer at {}/{}-{{timestamp}}{}",
                directory.display(),
                file_prefix,
                file_suffix
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

fn resolve_log_destination(path: &str) -> (PathBuf, String, String) {
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
        return (directory, "xtrade".to_string(), ".log".to_string());
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

        let (prefix, suffix) = split_prefix_suffix(&file_name);

        (directory, prefix, suffix)
    } else {
        (
            requested.to_path_buf(),
            "xtrade".to_string(),
            ".log".to_string(),
        )
    }
}

fn split_prefix_suffix(file_name: &str) -> (String, String) {
    if let Some((prefix, extension)) = file_name.rsplit_once('.') {
        if !prefix.is_empty() && !extension.is_empty() {
            return (prefix.to_string(), format!(".{extension}"));
        }
    }

    if file_name.is_empty() {
        return ("xtrade".to_string(), ".log".to_string());
    }

    (file_name.to_string(), ".log".to_string())
}

#[derive(Debug, Clone, Copy)]
struct LocalTimer;

impl tracing_subscriber::fmt::time::FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%Y-%m-%d %H:%M:%S%.3f"))
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_log_destination;
    use std::path::PathBuf;

    #[test]
    fn defaults_to_current_directory_when_path_is_empty() {
        let (directory, file_prefix, file_suffix) = resolve_log_destination("");

        assert_eq!(directory, PathBuf::from("."));
        assert_eq!(file_prefix, "xtrade");
        assert_eq!(file_suffix, ".log");
    }

    #[test]
    fn preserves_explicit_file_name() {
        let (directory, file_prefix, file_suffix) =
            resolve_log_destination("/var/log/xtrade/output.log");

        assert_eq!(directory, PathBuf::from("/var/log/xtrade"));
        assert_eq!(file_prefix, "output");
        assert_eq!(file_suffix, ".log");
    }

    #[test]
    fn treats_directory_paths_as_log_roots() {
        let (directory, file_prefix, file_suffix) = resolve_log_destination("logs/");

        assert_eq!(directory, PathBuf::from("logs/"));
        assert_eq!(file_prefix, "xtrade");
        assert_eq!(file_suffix, ".log");
    }

    #[test]
    fn defaults_suffix_when_missing_extension() {
        let (directory, file_prefix, file_suffix) = resolve_log_destination("/tmp/xtrade_output");

        assert_eq!(directory, PathBuf::from("/tmp"));
        assert_eq!(file_prefix, "xtrade_output");
        assert_eq!(file_suffix, ".log");
    }
}
