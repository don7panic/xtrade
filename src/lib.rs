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

use anyhow::Result;

/// Application result type for consistent error handling
pub type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Initialize tracing subscriber for logging
pub fn init_logging(level: &str) -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("xtrade={}", level).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}
