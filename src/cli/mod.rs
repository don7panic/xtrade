//! Command Line Interface module
//!
//! Implements the CLI commands and argument parsing for XTrade.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(name = "xtrade")]
#[command(about = "XTrade Market Data Monitor")]
#[command(long_about = "A high-performance cryptocurrency market data monitoring system")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Configuration file path
    #[arg(long, default_value = "config.toml")]
    pub config_file: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Dry-run mode: show welcome page and configuration without starting UI
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Start interactive terminal session
    #[command(hide = true)]
    Interactive {
        /// Use simple CLI output instead of full TUI
        #[arg(long)]
        simple: bool,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Demo WebSocket functionality (for testing)
    Demo,
}

impl Default for Commands {
    fn default() -> Self {
        Commands::Interactive { simple: false }
    }
}

#[derive(Subcommand, Debug, Clone)]
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

    /// Get the actual command, using default if none provided
    pub fn command(&self) -> Commands {
        self.command.clone().unwrap_or_default()
    }

    /// Check if we're running in interactive mode
    pub fn is_interactive_mode(&self) -> bool {
        matches!(self.command(), Commands::Interactive { .. })
    }

    /// Adjust log level based on verbose flag
    pub fn effective_log_level(&self) -> String {
        if self.verbose {
            "debug".to_string()
        } else {
            self.log_level.clone()
        }
    }

    /// Check if we're running in dry-run mode
    pub fn is_dry_run_mode(&self) -> bool {
        self.dry_run
    }
}
