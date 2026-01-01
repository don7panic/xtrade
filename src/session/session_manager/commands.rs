use anyhow::Result;
use tracing::{debug, error, info, warn};

use crate::binance::types::MarketKey;
use crate::cli::ConfigAction;

use super::SessionManager;
use crate::session::action_channel::{LogsInfo, SessionEvent, StatusInfo};
use crate::session::command_router::{AlertAction, CommandRouter, InteractiveCommand};

impl SessionManager {
    /// Handle user command
    pub(super) async fn handle_command(&mut self, command: InteractiveCommand) -> Result<()> {
        debug!("Handling command: {:?}", command);

        self.stats.commands_processed += 1;

        match command {
            InteractiveCommand::Add { symbols } => self.handle_subscribe(symbols).await,
            InteractiveCommand::Remove { symbols } => self.handle_unsubscribe(symbols).await,
            InteractiveCommand::List => self.handle_list().await,
            InteractiveCommand::Status => self.handle_status_command().await,
            InteractiveCommand::Show { symbol } => self.handle_show(symbol).await,
            InteractiveCommand::Config { action } => self.handle_config(action).await,
            InteractiveCommand::Reconnect => self.handle_reconnect().await,
            InteractiveCommand::Quit => self.handle_quit().await,
            InteractiveCommand::Logs => self.handle_logs().await,
            InteractiveCommand::Help => self.handle_help().await,
            InteractiveCommand::Alert { action } => self.handle_alert(action).await,
            InteractiveCommand::Buy { symbol, quantity } => {
                self.handle_paper_buy(symbol, quantity).await
            }
            InteractiveCommand::Sell { symbol, quantity } => {
                self.handle_paper_sell(symbol, quantity).await
            }
            InteractiveCommand::Portfolio => self.handle_paper_portfolio().await,
            InteractiveCommand::Orders => self.handle_paper_orders().await,
        }
    }

    /// Handle subscribe command
    async fn handle_subscribe(&mut self, symbols: Vec<String>) -> Result<()> {
        let market_type = self.get_market_type();
        for symbol in symbols {
            let key = MarketKey {
                market_type,
                symbol: symbol.clone(),
            };
            match self.market_manager.subscribe(key.clone()).await {
                Ok(()) => {
                    info!("Subscribed to symbol: {}", symbol);
                    self.action_channel
                        .send_event(SessionEvent::SubscriptionAdded { key })?;
                }
                Err(e) => {
                    error!("Failed to subscribe to {}: {}", symbol, e);
                    self.action_channel.send_event(SessionEvent::Error {
                        message: format!("Failed to subscribe to {}: {}", symbol, e),
                    })?;
                }
            }
        }

        Ok(())
    }

    /// Handle unsubscribe command
    async fn handle_unsubscribe(&mut self, symbols: Vec<String>) -> Result<()> {
        let market_type = self.get_market_type();
        for symbol in symbols {
            let key = MarketKey {
                market_type,
                symbol: symbol.clone(),
            };
            match self.market_manager.unsubscribe(&key).await {
                Ok(()) => {
                    info!("Unsubscribed from symbol: {}", symbol);
                    self.action_channel
                        .send_event(SessionEvent::SubscriptionRemoved { key })?;
                }
                Err(e) => {
                    error!("Failed to unsubscribe from {}: {}", symbol, e);
                    self.action_channel.send_event(SessionEvent::Error {
                        message: format!("Failed to unsubscribe from {}: {}", symbol, e),
                    })?;
                }
            }
        }

        Ok(())
    }

    /// Handle reconnect command
    async fn handle_reconnect(&mut self) -> Result<()> {
        let reconnect_window = self.app_config.binance.reconnect_interval_ms;

        let result = self
            .market_manager
            .handle_reconnection(reconnect_window)
            .await;

        match result {
            Ok(()) => {
                info!("Reconnect triggered for all active subscriptions");
                self.handle_status_command().await?;
            }
            Err(e) => {
                error!("Failed to trigger reconnect workflow: {}", e);
                self.action_channel.send_event(SessionEvent::Error {
                    message: format!("Reconnect failed: {}", e),
                })?;
            }
        }

        Ok(())
    }

    /// Handle list command
    async fn handle_list(&mut self) -> Result<()> {
        let keys = self.market_manager.list_subscriptions().await;

        info!("Current subscriptions: {:?}", keys);

        self.action_channel
            .send_event(SessionEvent::SubscriptionList { keys })?;

        Ok(())
    }

    /// Handle status command
    async fn handle_status_command(&mut self) -> Result<()> {
        let keys = self.market_manager.list_subscriptions().await;

        let status_info = StatusInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            state: "Running".to_string(),
            active_subscriptions: keys.len(),
            keys: keys.clone(),
            session_stats: self.stats.clone(),
        };

        self.action_channel
            .send_event(SessionEvent::StatusInfo { info: status_info })?;

        Ok(())
    }

    /// Handle show command
    async fn handle_show(&mut self, symbol: String) -> Result<()> {
        let key = MarketKey {
            market_type: self.get_market_type(),
            symbol: symbol.clone(),
        };
        if let Some(orderbook) = self.market_manager.get_orderbook(&key).await {
            self.action_channel
                .send_event(SessionEvent::SymbolDetails {
                    symbol,
                    orderbook: Some(orderbook),
                })?;
        } else {
            self.action_channel
                .send_event(SessionEvent::SymbolDetails {
                    symbol,
                    orderbook: None,
                })?;
        }

        Ok(())
    }

    /// Handle config command
    async fn handle_config(&mut self, action: Option<ConfigAction>) -> Result<()> {
        match action {
            Some(ConfigAction::Show) => {
                self.action_channel.send_event(SessionEvent::ConfigInfo {
                    config: self.app_config.clone(),
                })?;
            }
            Some(ConfigAction::Set { key, value }) => {
                let key_normalized = key.to_ascii_lowercase();
                match key_normalized.as_str() {
                    "refresh_rate_ms" | "refresh-rate" => match value.parse::<u64>() {
                        Ok(parsed) if parsed > 0 => {
                            self.app_config.refresh_rate_ms = parsed;
                            self.metrics
                                .set_emit_interval(self.app_config.refresh_rate_ms);
                            info!("Updated refresh_rate_ms to {}", parsed);
                            self.action_channel.send_event(SessionEvent::ConfigInfo {
                                config: self.app_config.clone(),
                            })?;
                        }
                        _ => {
                            let message = format!("Invalid refresh_rate_ms value: {}", value);
                            warn!("{}", message);
                            self.action_channel
                                .send_event(SessionEvent::Error { message })?;
                        }
                    },
                    "orderbook_depth" | "orderbook-depth" => match value.parse::<usize>() {
                        Ok(parsed) if parsed > 0 => {
                            self.app_config.orderbook_depth = parsed;
                            info!("Updated orderbook_depth to {}", parsed);
                            self.action_channel.send_event(SessionEvent::ConfigInfo {
                                config: self.app_config.clone(),
                            })?;
                        }
                        _ => {
                            let message = format!("Invalid orderbook_depth value: {}", value);
                            warn!("{}", message);
                            self.action_channel
                                .send_event(SessionEvent::Error { message })?;
                        }
                    },
                    "ui.sparkline_points" | "ui.sparkline-points" => match value.parse::<usize>() {
                        Ok(parsed) if parsed >= 10 => {
                            self.app_config.ui.sparkline_points = parsed;
                            info!("Updated ui.sparkline_points to {}", parsed);
                            self.action_channel.send_event(SessionEvent::ConfigInfo {
                                config: self.app_config.clone(),
                            })?;
                        }
                        _ => {
                            let message =
                                format!("Invalid ui.sparkline_points value: {} (min 10)", value);
                            warn!("{}", message);
                            self.action_channel
                                .send_event(SessionEvent::Error { message })?;
                        }
                    },
                    other => {
                        let message = format!("Unsupported config key: {}", other);
                        warn!("{}", message);
                        self.action_channel
                            .send_event(SessionEvent::Error { message })?;
                    }
                }
            }
            Some(ConfigAction::Reset) => {
                self.app_config = crate::config::Config::default();
                info!("Configuration reset to defaults");
                self.metrics
                    .set_emit_interval(self.app_config.refresh_rate_ms);
                self.action_channel.send_event(SessionEvent::ConfigReset)?;
                self.action_channel.send_event(SessionEvent::ConfigInfo {
                    config: self.app_config.clone(),
                })?;
            }
            None => {
                self.action_channel.send_event(SessionEvent::ConfigHelp)?;
            }
        }

        Ok(())
    }

    /// Handle quit command
    async fn handle_quit(&mut self) -> Result<()> {
        info!("User requested quit");
        self.shutdown().await
    }

    /// Handle logs command
    async fn handle_logs(&mut self) -> Result<()> {
        info!("User requested logs");

        let mut recent_logs = crate::recent_logs(100);
        if recent_logs.is_empty() {
            recent_logs.push("(no log entries captured yet)".to_string());
        }

        let logs_info = LogsInfo {
            recent_logs,
            log_file_path: self.app_config.log.file_path.clone(),
            log_level: self.cli.effective_log_level(),
        };

        self.action_channel
            .send_event(SessionEvent::LogsInfo { info: logs_info })?;

        Ok(())
    }

    /// Handle help command
    async fn handle_help(&mut self) -> Result<()> {
        info!("User requested interactive help");

        if self.config.enable_tui && self.ui_event_tx.is_some() {
            let lines = CommandRouter::help_messages()
                .iter()
                .map(|line| (*line).to_string())
                .collect();
            self.forward_to_ui(SessionEvent::HelpInfo { lines });
        } else {
            println!();
            for line in CommandRouter::help_messages() {
                println!("{}", line);
            }
            println!();
        }

        Ok(())
    }

    /// Handle alert command
    async fn handle_alert(&mut self, action: AlertAction) -> Result<()> {
        let enable_tui = self.config.enable_tui;
        let ui_event_tx = self.ui_event_tx.as_ref();
        self.alerts
            .handle_action(action, enable_tui, ui_event_tx, &self.action_channel)
    }

    /// Handle paper buy command
    async fn handle_paper_buy(&mut self, symbol: String, quantity: f64) -> Result<()> {
        let outcome = self.paper_trading.buy(&symbol, quantity)?;

        self.action_channel.send_event(SessionEvent::OrderFilled {
            order: outcome.order.clone(),
        })?;

        self.forward_to_ui(SessionEvent::PortfolioUpdate {
            portfolio: outcome.portfolio,
        });

        Ok(())
    }

    /// Handle paper sell command
    async fn handle_paper_sell(&mut self, symbol: String, quantity: f64) -> Result<()> {
        let outcome = self.paper_trading.sell(&symbol, quantity)?;

        self.action_channel.send_event(SessionEvent::OrderFilled {
            order: outcome.order.clone(),
        })?;

        self.forward_to_ui(SessionEvent::PortfolioUpdate {
            portfolio: outcome.portfolio,
        });

        Ok(())
    }

    /// Handle portfolio view
    async fn handle_paper_portfolio(&mut self) -> Result<()> {
        let portfolio = self.paper_trading.portfolio_snapshot();
        self.forward_to_ui(SessionEvent::PortfolioSnapshot { portfolio });
        Ok(())
    }

    /// Handle orders view
    async fn handle_paper_orders(&mut self) -> Result<()> {
        let orders = self.paper_trading.order_history_snapshot();
        self.forward_to_ui(SessionEvent::OrderHistorySnapshot {
            orders: orders.into(),
        });
        Ok(())
    }
}
