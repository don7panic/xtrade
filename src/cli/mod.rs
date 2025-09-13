//! Command Line Interface module
//!
//! Implements the CLI commands and argument parsing for XTrade.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "xtrade")]
#[command(about = "XTrade Market Data Monitor")]
#[command(long_about = "A high-performance cryptocurrency market data monitoring system")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Configuration file path
    #[arg(long, default_value = "config.toml")]
    pub config_file: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Subscribe to market data for specified symbols
    Subscribe {
        /// Trading symbols (e.g., BTC-USDT,ETH-USDT)
        symbols: Vec<String>,
    },

    /// Unsubscribe from market data for specified symbols  
    Unsubscribe {
        /// Trading symbols to unsubscribe from
        symbols: Vec<String>,
    },

    /// List currently subscribed symbols
    List,

    /// Start TUI (Terminal User Interface)
    Ui {
        /// Use simple CLI output instead of full TUI
        #[arg(long)]
        simple: bool,
    },

    /// Show connection status and metrics
    Status,

    /// Show detailed information for a specific symbol
    Show {
        /// Trading symbol to display
        symbol: String,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show,

    /// Set configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value  
        value: String,
    },

    /// Reset configuration to defaults
    Reset,
}

impl Cli {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Adjust log level based on verbose flag
    pub fn effective_log_level(&self) -> String {
        if self.verbose {
            "debug".to_string()
        } else {
            self.log_level.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test basic command parsing
        let cli = Cli::try_parse_from(&["xtrade", "list"]).unwrap();
        matches!(cli.command, Commands::List);
    }

    #[test]
    fn test_subscribe_command() {
        let cli = Cli::try_parse_from(&["xtrade", "subscribe", "BTC-USDT", "ETH-USDT"]).unwrap();
        match cli.command {
            Commands::Subscribe { symbols } => {
                assert_eq!(symbols, vec!["BTC-USDT", "ETH-USDT"]);
            }
            _ => panic!("Expected Subscribe command"),
        }
    }

    #[test]
    fn test_verbose_flag() {
        let cli = Cli::try_parse_from(&["xtrade", "--verbose", "status"]).unwrap();
        assert!(cli.verbose);
        assert_eq!(cli.effective_log_level(), "debug");
    }
}
