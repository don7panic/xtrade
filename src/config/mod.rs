//! Configuration management module
//!
//! Handles loading, validation, and management of application configuration.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// List of trading symbols to monitor
    pub symbols: Vec<String>,

    /// UI refresh rate in milliseconds
    pub refresh_rate_ms: u64,

    /// OrderBook depth to display
    pub orderbook_depth: usize,

    /// Enable price sparkline charts
    pub enable_sparkline: bool,

    /// Logging level
    pub log_level: String,

    /// File-based logging configuration
    pub log: LogConfig,

    /// Binance-specific configuration
    pub binance: BinanceConfig,

    /// UI-specific configuration
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BinanceConfig {
    /// WebSocket base URL
    pub ws_url: String,

    /// REST API base URL
    pub rest_url: String,

    /// Request timeout in seconds
    pub timeout_seconds: u64,

    /// Reconnect interval in milliseconds
    pub reconnect_interval_ms: u64,

    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct UiConfig {
    /// Enable colors in TUI
    pub enable_colors: bool,

    /// TUI update rate in FPS
    pub update_rate_fps: u32,

    /// Maximum price history points for sparkline
    pub sparkline_points: usize,

    /// Minimum seconds between kline redraws from streaming updates
    pub kline_refresh_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogConfig {
    /// Absolute or relative path to the rolling log file
    pub file_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            symbols: vec!["BTCUSDT".to_string()],
            refresh_rate_ms: 100,
            orderbook_depth: 20,
            enable_sparkline: true,
            log_level: "info".to_string(),
            log: LogConfig::default(),
            binance: BinanceConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Default for BinanceConfig {
    fn default() -> Self {
        Self {
            ws_url: "wss://stream.binance.com:9443".to_string(),
            rest_url: "https://api.binance.com".to_string(),
            timeout_seconds: 10,
            reconnect_interval_ms: 1000,
            max_reconnect_attempts: 10,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            enable_colors: true,
            update_rate_fps: 20,
            sparkline_points: 60,
            kline_refresh_secs: 60,
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            file_path: "logs/xtrade.log".to_string(),
        }
    }
}

impl Config {
    /// Load configuration from file with environment variable overrides
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        let mut config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.as_ref().display()))?;

        // Apply environment variable overrides
        config.apply_env_overrides();

        config.validate()?;
        Ok(config)
    }

    /// Apply environment variable overrides to configuration
    pub fn apply_env_overrides(&mut self) {
        // XTRADE_SYMBOLS - comma-separated list of symbols
        if let Ok(symbols) = env::var("XTRADE_SYMBOLS") {
            self.symbols = symbols
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // XTRADE_REFRESH_RATE_MS - UI refresh rate
        if let Ok(refresh_rate) = env::var("XTRADE_REFRESH_RATE_MS") {
            if let Ok(value) = refresh_rate.parse::<u64>() {
                self.refresh_rate_ms = value;
            }
        }

        // XTRADE_ORDERBOOK_DEPTH - orderbook depth
        if let Ok(depth) = env::var("XTRADE_ORDERBOOK_DEPTH") {
            if let Ok(value) = depth.parse::<usize>() {
                self.orderbook_depth = value;
            }
        }

        // XTRADE_ENABLE_SPARKLINE - enable sparkline
        if let Ok(sparkline) = env::var("XTRADE_ENABLE_SPARKLINE") {
            self.enable_sparkline = sparkline.parse().unwrap_or(self.enable_sparkline);
        }

        // XTRADE_LOG_LEVEL - logging level
        if let Ok(log_level) = env::var("XTRADE_LOG_LEVEL") {
            self.log_level = log_level;
        }

        // XTRADE_LOG_FILE_PATH - logging destination file
        if let Ok(file_path) = env::var("XTRADE_LOG_FILE_PATH") {
            if !file_path.trim().is_empty() {
                self.log.file_path = file_path;
            }
        }

        // Binance-specific environment variables
        // XTRADE_BINANCE_WS_URL - WebSocket URL
        if let Ok(ws_url) = env::var("XTRADE_BINANCE_WS_URL") {
            self.binance.ws_url = ws_url;
        }

        // XTRADE_BINANCE_REST_URL - REST API URL
        if let Ok(rest_url) = env::var("XTRADE_BINANCE_REST_URL") {
            self.binance.rest_url = rest_url;
        }

        // XTRADE_BINANCE_TIMEOUT_SECONDS - timeout
        if let Ok(timeout) = env::var("XTRADE_BINANCE_TIMEOUT_SECONDS") {
            if let Ok(value) = timeout.parse::<u64>() {
                self.binance.timeout_seconds = value;
            }
        }

        // XTRADE_BINANCE_RECONNECT_INTERVAL_MS - reconnect interval
        if let Ok(interval) = env::var("XTRADE_BINANCE_RECONNECT_INTERVAL_MS") {
            if let Ok(value) = interval.parse::<u64>() {
                self.binance.reconnect_interval_ms = value;
            }
        }

        // XTRADE_BINANCE_MAX_RECONNECT_ATTEMPTS - max reconnect attempts
        if let Ok(attempts) = env::var("XTRADE_BINANCE_MAX_RECONNECT_ATTEMPTS") {
            if let Ok(value) = attempts.parse::<u32>() {
                self.binance.max_reconnect_attempts = value;
            }
        }

        // UI-specific environment variables
        // XTRADE_UI_ENABLE_COLORS - enable colors
        if let Ok(enable_colors) = env::var("XTRADE_UI_ENABLE_COLORS") {
            self.ui.enable_colors = enable_colors.parse().unwrap_or(self.ui.enable_colors);
        }

        // XTRADE_UI_UPDATE_RATE_FPS - UI update rate
        if let Ok(fps) = env::var("XTRADE_UI_UPDATE_RATE_FPS") {
            if let Ok(value) = fps.parse::<u32>() {
                self.ui.update_rate_fps = value;
            }
        }

        // XTRADE_UI_SPARKLINE_POINTS - sparkline points
        if let Ok(points) = env::var("XTRADE_UI_SPARKLINE_POINTS") {
            if let Ok(value) = points.parse::<usize>() {
                self.ui.sparkline_points = value;
            }
        }

        // XTRADE_UI_KLINE_REFRESH_SECS - kline refresh throttle
        if let Ok(refresh) = env::var("XTRADE_UI_KLINE_REFRESH_SECS") {
            if let Ok(value) = refresh.parse::<u64>() {
                self.ui.kline_refresh_secs = value.max(1);
            }
        }
    }

    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;

        Ok(())
    }

    /// Load configuration with fallback to default
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        Self::load_from_file(path).unwrap_or_else(|err| {
            tracing::warn!("Failed to load config: {}, using defaults", err);
            Self::default()
        })
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        if self.symbols.is_empty() {
            anyhow::bail!("At least one symbol must be specified");
        }

        if self.refresh_rate_ms == 0 {
            anyhow::bail!("Refresh rate must be greater than 0");
        }

        if self.orderbook_depth == 0 {
            anyhow::bail!("OrderBook depth must be greater than 0");
        }

        if self.binance.timeout_seconds == 0 {
            anyhow::bail!("Timeout must be greater than 0");
        }

        if self.log.file_path.trim().is_empty() {
            anyhow::bail!("Log file path must not be empty");
        }

        if self.ui.kline_refresh_secs == 0 {
            anyhow::bail!("ui.kline_refresh_secs must be greater than 0");
        }

        // Validate symbol format (basic check)
        for symbol in &self.symbols {
            if symbol.is_empty() || symbol.len() < 3 {
                anyhow::bail!("Invalid symbol format: {}", symbol);
            }
        }

        Ok(())
    }

    /// Normalize symbol format for Binance API
    pub fn normalize_symbol(symbol: &str) -> String {
        // Convert BTC-USDT to BTCUSDT format
        symbol.replace('-', "").to_uppercase()
    }

    /// Display formatted configuration
    pub fn display(&self) -> Result<()> {
        println!("Current configuration:");
        println!("{:#?}", self);
        Ok(())
    }

    /// Display configuration summary
    pub fn display_summary(&self) -> Result<()> {
        println!("Configuration loaded successfully");
        Ok(())
    }

    /// Display configuration management help
    pub fn display_help() -> Result<()> {
        println!("Configuration management commands:");
        println!("  xtrade config show    - Show current configuration");
        println!("  xtrade config set <key> <value> - Set configuration value");
        println!("  xtrade config reset   - Reset to default configuration");
        Ok(())
    }

    /// Handle configuration command
    pub fn handle_command(action: &Option<crate::cli::ConfigAction>) -> Result<()> {
        match action {
            Some(crate::cli::ConfigAction::Show) => {
                let config = Config::load_or_default("config.toml");
                config.display()?;
            }
            Some(crate::cli::ConfigAction::Set { key, value }) => {
                println!("Config set command: {} = {}", key, value);
                println!("Note: Config set functionality not yet implemented");
            }
            Some(crate::cli::ConfigAction::Reset) => {
                let default_config = Config::default();
                default_config.display()?;
            }
            None => {
                Config::display_help()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.symbols, vec!["BTCUSDT"]);
    }

    #[test]
    fn test_symbol_normalization() {
        assert_eq!(Config::normalize_symbol("BTC-USDT"), "BTCUSDT");
        assert_eq!(Config::normalize_symbol("btc-usdt"), "BTCUSDT");
        assert_eq!(Config::normalize_symbol("ETHUSDT"), "ETHUSDT");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(config.symbols, deserialized.symbols);
    }

    #[test]
    fn test_config_file_operations() {
        let config = Config::default();
        let temp_file = NamedTempFile::new().unwrap();

        // Test save
        config.save_to_file(temp_file.path()).unwrap();

        // Test load
        let loaded_config = Config::load_from_file(temp_file.path()).unwrap();
        assert_eq!(config.symbols, loaded_config.symbols);
    }
}
