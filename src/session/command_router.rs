//! Command Router for interactive command processing

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::cli::{Cli, Commands};
use crate::session::alert_manager::AlertDirection;
use tracing::warn;

/// Interactive commands for the terminal session
#[derive(Debug, Clone)]
pub enum InteractiveCommand {
    /// Add symbol to subscription (alias for subscribe)
    Add { symbols: Vec<String> },
    /// Remove symbol from subscription (alias for unsubscribe)
    Remove { symbols: Vec<String> },
    /// List currently subscribed symbols
    List,
    /// Show connection status and metrics
    Status,
    /// Show detailed information for a specific symbol
    Show { symbol: String },
    /// Force reconnection & resync workflow
    Reconnect,
    /// Quit the application
    Quit,
    /// Show recent logs
    Logs,
    /// Configuration management
    Config {
        action: Option<crate::cli::ConfigAction>,
    },
    /// Show interactive help
    Help,
    /// Manage price alerts
    Alert { action: AlertAction },
}

/// Alert subcommands
#[derive(Debug, Clone)]
pub enum AlertAction {
    Add {
        symbol: String,
        direction: AlertDirection,
        price: f64,
    },
    List,
    Clear {
        target: ClearTarget,
    },
}

/// Target for alert clear command
#[derive(Debug, Clone)]
pub enum ClearTarget {
    Id(u64),
    All,
}

/// Metadata describing an interactive command (used for UI hints)
#[derive(Debug, Clone, Copy)]
pub struct CommandInfo {
    pub trigger: &'static str,
    pub usage: &'static str,
    pub description: &'static str,
}

/// Static help descriptions used for interactive commands
const HELP_LINES: [&str; 14] = [
    "XTrade Interactive Commands:",
    "  /add <symbol1> [symbol2] ...  - Subscribe to symbols",
    "  /remove <symbol1> [symbol2] ... - Unsubscribe from symbols",
    "  /list                         - Show current subscriptions",
    "  /show  <symbol>               - Show details for symbol",
    "  /status                       - Show session statistics",
    "  /reconnect                    - Force reconnection for all subscriptions",
    "  /logs                         - Show recent logs",
    "  /config [show|set|reset]      - Configuration management",
    "  /alert add <symbol> <above|below> <price> - Add price alert",
    "  /alert list                   - List configured alerts",
    "  /alert clear <id|all>         - Clear alerts",
    "  /help                         - Show this help",
    "  /quit                         - Exit the application",
];

/// Static list of interactive commands with descriptions for UI surfaces
const COMMANDS: [CommandInfo; 11] = [
    CommandInfo {
        trigger: "/add",
        usage: "/add <symbol1> [symbol2] ...",
        description: "Subscribe to symbols",
    },
    CommandInfo {
        trigger: "/remove",
        usage: "/remove <symbol1> [symbol2] ...",
        description: "Unsubscribe from symbols",
    },
    CommandInfo {
        trigger: "/list",
        usage: "/list",
        description: "Show current subscriptions",
    },
    CommandInfo {
        trigger: "/show",
        usage: "/show <symbol>",
        description: "Show details for symbol",
    },
    CommandInfo {
        trigger: "/status",
        usage: "/status",
        description: "Show session statistics",
    },
    CommandInfo {
        trigger: "/reconnect",
        usage: "/reconnect",
        description: "Force reconnection for all subscriptions",
    },
    CommandInfo {
        trigger: "/logs",
        usage: "/logs",
        description: "Show recent logs",
    },
    CommandInfo {
        trigger: "/config",
        usage: "/config [show|set <key> <value>|reset]",
        description: "Configuration management",
    },
    CommandInfo {
        trigger: "/alert",
        usage: "/alert add <symbol> <above|below> <price> | /alert list | /alert clear <id|all>",
        description: "Manage price alerts",
    },
    CommandInfo {
        trigger: "/help",
        usage: "/help",
        description: "Show help",
    },
    CommandInfo {
        trigger: "/quit",
        usage: "/quit",
        description: "Exit the application",
    },
];

/// Command router for processing interactive commands
pub struct CommandRouter {
    /// Command input channel
    command_tx: mpsc::UnboundedSender<InteractiveCommand>,
    /// Command input receiver
    command_rx: Option<mpsc::UnboundedReceiver<InteractiveCommand>>,
    /// Interactive mode flag
    interactive_mode: bool,
}

impl Default for CommandRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRouter {
    /// Create a new CommandRouter
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        Self {
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
    pub fn send_command(&self, command: InteractiveCommand) -> Result<()> {
        self.command_tx
            .send(command)
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))
    }

    /// Get next command from input
    pub async fn next_command(&mut self) -> Option<InteractiveCommand> {
        if let Some(command_rx) = &mut self.command_rx {
            command_rx.recv().await
        } else {
            None
        }
    }

    /// Process CLI arguments and convert to commands
    pub async fn process_cli_args(&self, cli: &Cli) -> Result<()> {
        debug!("Processing CLI arguments: {:?}", cli);

        // For interactive mode, we don't need to process CLI args
        // All commands will be handled through interactive input
        if cli.is_interactive_mode() {
            info!("Starting in interactive mode - commands will be processed interactively");
            return Ok(());
        }

        // Handle non-interactive commands (should only be config/demo)
        match &cli.command() {
            Commands::Config { action: _ } => {
                // Config command is handled directly in main.rs
                info!("Config command handled directly in main.rs");
            }
            Commands::Demo => {
                // Demo command is handled directly in main.rs
                info!("Demo command handled directly in main.rs");
            }
            _ => {
                // This should not happen - only config/demo should be available
                warn!(
                    "Unexpected command in non-interactive mode: {:?}",
                    cli.command()
                );
            }
        }

        Ok(())
    }

    /// Parse interactive command from string input
    pub fn parse_interactive_command(&self, input: &str) -> Result<Option<InteractiveCommand>> {
        let input = input.trim();

        if input.is_empty() {
            return Ok(None);
        }

        let parts: Vec<&str> = input.split_whitespace().collect();

        match parts[0] {
            "/add" => {
                if parts.len() < 2 {
                    return Err(anyhow::anyhow!("Usage: add <symbol1> [symbol2] ..."));
                }
                let symbols = parts[1..].iter().map(|s| s.to_string()).collect();
                Ok(Some(InteractiveCommand::Add { symbols }))
            }
            "/remove" => {
                if parts.len() < 2 {
                    return Err(anyhow::anyhow!("Usage: remove <symbol1> [symbol2] ..."));
                }
                let symbols = parts[1..].iter().map(|s| s.to_string()).collect();
                Ok(Some(InteractiveCommand::Remove { symbols }))
            }
            "/list" | "pairs" => Ok(Some(InteractiveCommand::List)),
            "/show" => {
                if parts.len() < 2 {
                    return Err(anyhow::anyhow!("Usage: focus <symbol>"));
                }
                Ok(Some(InteractiveCommand::Show {
                    symbol: parts[1].to_string(),
                }))
            }
            "/status" => Ok(Some(InteractiveCommand::Status)),
            "/reconnect" | "/r" => Ok(Some(InteractiveCommand::Reconnect)),
            "/config" => {
                if parts.len() == 1 {
                    Ok(Some(InteractiveCommand::Config { action: None }))
                } else if parts.len() == 2 && parts[1] == "show" {
                    Ok(Some(InteractiveCommand::Config {
                        action: Some(crate::cli::ConfigAction::Show),
                    }))
                } else if parts.len() == 2 && parts[1] == "reset" {
                    Ok(Some(InteractiveCommand::Config {
                        action: Some(crate::cli::ConfigAction::Reset),
                    }))
                } else if parts.len() >= 3 && parts[1] == "set" {
                    let key = parts[2].to_string();
                    let value = if parts.len() > 3 {
                        parts[3..].join(" ")
                    } else {
                        "".to_string()
                    };
                    Ok(Some(InteractiveCommand::Config {
                        action: Some(crate::cli::ConfigAction::Set { key, value }),
                    }))
                } else {
                    Err(anyhow::anyhow!(
                        "Usage: config [show|set <key> <value>|reset]"
                    ))
                }
            }
            "/alert" => self.parse_alert_command(&parts),
            "/help" | "?" => Ok(Some(InteractiveCommand::Help)),
            "/logs" => Ok(Some(InteractiveCommand::Logs)),
            "/quit" | "/exit" | "/q" => Ok(Some(InteractiveCommand::Quit)),
            _ => Err(anyhow::anyhow!(
                "Unknown command: {}. Type '/help' for available commands.",
                parts[0]
            )),
        }
    }

    /// Return interactive command help text
    pub fn help_messages() -> &'static [&'static str] {
        &HELP_LINES
    }

    /// Get interactive mode status
    pub fn is_interactive_mode(&self) -> bool {
        self.interactive_mode
    }

    /// Get command sender for external use
    pub fn command_sender(&self) -> mpsc::UnboundedSender<InteractiveCommand> {
        self.command_tx.clone()
    }

    /// Return the catalog of interactive commands for UI surfaces
    pub fn commands() -> &'static [CommandInfo] {
        &COMMANDS
    }

    fn parse_alert_command(&self, parts: &[&str]) -> Result<Option<InteractiveCommand>> {
        if parts.len() < 2 {
            return Err(anyhow::anyhow!(
                "Usage: /alert <add|list|clear> [...]. Example: /alert add BTCUSDT above 45000"
            ));
        }

        match parts[1] {
            "add" => {
                if parts.len() < 5 {
                    return Err(anyhow::anyhow!(
                        "Usage: /alert add <symbol> <above|below> <price>"
                    ));
                }
                let symbol = parts[2].to_string();
                let direction = match parts[3].to_ascii_lowercase().as_str() {
                    "above" => AlertDirection::Above,
                    "below" => AlertDirection::Below,
                    other => {
                        return Err(anyhow::anyhow!(
                            "Invalid direction '{}'. Use 'above' or 'below'.",
                            other
                        ));
                    }
                };
                let price: f64 = parts[4].parse().map_err(|_| {
                    anyhow::anyhow!("Invalid price '{}'. Price must be a number.", parts[4])
                })?;
                Ok(Some(InteractiveCommand::Alert {
                    action: AlertAction::Add {
                        symbol,
                        direction,
                        price,
                    },
                }))
            }
            "list" => Ok(Some(InteractiveCommand::Alert {
                action: AlertAction::List,
            })),
            "clear" => {
                if parts.len() < 3 {
                    return Err(anyhow::anyhow!(
                        "Usage: /alert clear <id|all>. Example: /alert clear 1"
                    ));
                }
                let target = if parts[2].eq_ignore_ascii_case("all") {
                    ClearTarget::All
                } else {
                    let id = parts[2].parse::<u64>().map_err(|_| {
                        anyhow::anyhow!("Invalid alert id '{}'. Expected a number.", parts[2])
                    })?;
                    ClearTarget::Id(id)
                };
                Ok(Some(InteractiveCommand::Alert {
                    action: AlertAction::Clear { target },
                }))
            }
            other => Err(anyhow::anyhow!(format!(
                "Unknown alert action '{}'. Use add, list, or clear.",
                other
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_alert_add_command() {
        let router = CommandRouter::new();
        let cmd = router
            .parse_interactive_command("/alert add btcusdt above 45000")
            .unwrap()
            .unwrap();
        match cmd {
            InteractiveCommand::Alert {
                action:
                    AlertAction::Add {
                        symbol,
                        direction,
                        price,
                    },
            } => {
                assert_eq!(symbol, "btcusdt");
                assert_eq!(direction, AlertDirection::Above);
                assert_eq!(price, 45000.0);
            }
            other => panic!("Unexpected command parsed: {:?}", other),
        }
    }

    #[test]
    fn rejects_invalid_alert_direction() {
        let router = CommandRouter::new();
        let err = router
            .parse_interactive_command("/alert add btcusdt around 45000")
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("Invalid direction 'around'. Use 'above' or 'below'.")
        );
    }
}
