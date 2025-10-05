//! Command Router for interactive command processing

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info};

use crate::cli::{Cli, Commands};
use crate::market_data::MarketDataManager;

/// Command router for processing interactive commands
pub struct CommandRouter {
    /// Market data manager reference
    market_manager: Arc<Mutex<MarketDataManager>>,
    /// Command input channel
    command_tx: mpsc::UnboundedSender<Commands>,
    /// Command input receiver
    command_rx: Option<mpsc::UnboundedReceiver<Commands>>,
    /// Interactive mode flag
    interactive_mode: bool,
}

impl CommandRouter {
    /// Create a new CommandRouter
    pub fn new(market_manager: Arc<Mutex<MarketDataManager>>) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        Self {
            market_manager,
            command_tx,
            command_rx: Some(command_rx),
            interactive_mode: false,
        }
    }

    /// Start interactive mode
    pub fn start_interactive_mode(&mut self) -> Result<()> {
        self.interactive_mode = true;
        info!("Command router started in interactive mode");
        Ok(())
    }

    /// Stop interactive mode
    pub fn stop_interactive_mode(&mut self) -> Result<()> {
        self.interactive_mode = false;
        info!("Command router stopped interactive mode");
        Ok(())
    }

    /// Send command to router
    pub fn send_command(&self, command: Commands) -> Result<()> {
        self.command_tx
            .send(command)
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))
    }

    /// Get next command from input
    pub async fn next_command(&mut self) -> Option<Commands> {
        if let Some(command_rx) = &mut self.command_rx {
            command_rx.recv().await
        } else {
            None
        }
    }

    /// Process CLI arguments and convert to commands
    pub async fn process_cli_args(&self, cli: &Cli) -> Result<()> {
        debug!("Processing CLI arguments: {:?}", cli);

        // Handle CLI commands
        match &cli.command {
            Commands::Subscribe { symbols } => {
                self.send_command(Commands::Subscribe {
                    symbols: symbols.clone(),
                })?;
            }
            Commands::Unsubscribe { symbols } => {
                self.send_command(Commands::Unsubscribe {
                    symbols: symbols.clone(),
                })?;
            }
            Commands::List => {
                self.send_command(Commands::List)?;
            }
            Commands::Ui { simple } => {
                self.send_command(Commands::Ui { simple: *simple })?;
            }
            Commands::Status => {
                self.send_command(Commands::Status)?;
            }
            Commands::Show { symbol } => {
                self.send_command(Commands::Show {
                    symbol: symbol.clone(),
                })?;
            }
            Commands::Config { action } => {
                self.send_command(Commands::Config {
                    action: action.as_ref().cloned(),
                })?;
            }
            Commands::Demo => {
                self.send_command(Commands::Demo)?;
            }
        }

        Ok(())
    }

    /// Parse interactive command from string input
    pub fn parse_interactive_command(&self, input: &str) -> Result<Option<Commands>> {
        let input = input.trim();

        if input.is_empty() {
            return Ok(None);
        }

        let parts: Vec<&str> = input.split_whitespace().collect();

        match parts[0] {
            "add" | "subscribe" => {
                if parts.len() < 2 {
                    return Err(anyhow::anyhow!("Usage: add <symbol1> [symbol2] ..."));
                }
                let symbols = parts[1..].iter().map(|s| s.to_string()).collect();
                Ok(Some(Commands::Subscribe { symbols }))
            }
            "remove" | "unsubscribe" => {
                if parts.len() < 2 {
                    return Err(anyhow::anyhow!("Usage: remove <symbol1> [symbol2] ..."));
                }
                let symbols = parts[1..].iter().map(|s| s.to_string()).collect();
                Ok(Some(Commands::Unsubscribe { symbols }))
            }
            "list" | "pairs" => Ok(Some(Commands::List)),
            "focus" => {
                if parts.len() < 2 {
                    return Err(anyhow::anyhow!("Usage: focus <symbol>"));
                }
                Ok(Some(Commands::Show {
                    symbol: parts[1].to_string(),
                }))
            }
            "stats" => Ok(Some(Commands::Status)),
            "config" => {
                if parts.len() == 1 {
                    Ok(Some(Commands::Config { action: None }))
                } else if parts.len() == 2 && parts[1] == "show" {
                    Ok(Some(Commands::Config {
                        action: Some(crate::cli::ConfigAction::Show),
                    }))
                } else if parts.len() == 2 && parts[1] == "reset" {
                    Ok(Some(Commands::Config {
                        action: Some(crate::cli::ConfigAction::Reset),
                    }))
                } else if parts.len() >= 3 && parts[1] == "set" {
                    let key = parts[2].to_string();
                    let value = if parts.len() > 3 {
                        parts[3..].join(" ")
                    } else {
                        "".to_string()
                    };
                    Ok(Some(Commands::Config {
                        action: Some(crate::cli::ConfigAction::Set { key, value }),
                    }))
                } else {
                    Err(anyhow::anyhow!(
                        "Usage: config [show|set <key> <value>|reset]"
                    ))
                }
            }
            "help" | "?" => {
                self.show_help();
                Ok(None)
            }
            "quit" | "exit" | "q" => {
                Ok(Some(Commands::Ui { simple: true })) // Use UI command as shutdown signal
            }
            _ => Err(anyhow::anyhow!(
                "Unknown command: {}. Type 'help' for available commands.",
                parts[0]
            )),
        }
    }

    /// Show interactive command help
    fn show_help(&self) {
        println!("\nXTrade Interactive Commands:");
        println!("  add <symbol1> [symbol2] ...  - Subscribe to symbols");
        println!("  remove <symbol1> [symbol2] ... - Unsubscribe from symbols");
        println!("  list                         - Show current subscriptions");
        println!("  focus <symbol>               - Show details for symbol");
        println!("  stats                        - Show session statistics");
        println!("  config [show|set|reset]      - Configuration management");
        println!("  help                         - Show this help");
        println!("  quit                         - Exit the application");
        println!();
    }

    /// Get interactive mode status
    pub fn is_interactive_mode(&self) -> bool {
        self.interactive_mode
    }

    /// Get command sender for external use
    pub fn command_sender(&self) -> mpsc::UnboundedSender<Commands> {
        self.command_tx.clone()
    }
}
