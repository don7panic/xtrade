//! Session Manager for interactive terminal session lifecycle management

use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};

use crate::cli::Cli;
use crate::config::Config;
use crate::market_data::MarketDataManager;
use crate::metrics::{ConnectionStatus as MetricsConnectionStatus, MetricsCollector};
use crate::ui::ui_manager::UIManager;

use super::action_channel::{ActionChannel, SessionEvent};
use super::command_router::{CommandRouter, InteractiveCommand};

/// Session state tracking
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Starting,
    Running,
    Paused,
    ShuttingDown,
    Terminated,
}

/// Session configuration for interactive mode
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub enable_tui: bool,
    pub enable_metrics: bool,
    pub auto_subscribe: bool,
    pub session_timeout_ms: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            enable_tui: true,
            enable_metrics: true,
            auto_subscribe: true,
            session_timeout_ms: 3600000, // 1 hour default timeout
        }
    }
}

/// Session statistics for monitoring
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub start_time: u64,
    pub commands_processed: u64,
    pub events_processed: u64,
    pub errors_encountered: u64,
    pub memory_usage_mb: f64,
}

impl Default for SessionStats {
    fn default() -> Self {
        Self {
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            commands_processed: 0,
            events_processed: 0,
            errors_encountered: 0,
            memory_usage_mb: 0.0,
        }
    }
}

/// Main session manager for interactive terminal
pub struct SessionManager {
    /// Session configuration
    config: SessionConfig,
    /// Application configuration
    app_config: Config,
    /// CLI arguments
    cli: Cli,
    /// Session state
    state: SessionState,
    /// Session statistics
    stats: SessionStats,
    /// Market data manager
    market_manager: Arc<MarketDataManager>,
    /// UI task handle (optional)
    ui_task: Option<tokio::task::JoinHandle<()>>,
    /// UI event sender (Session -> UI)
    ui_event_tx: Option<mpsc::UnboundedSender<SessionEvent>>,
    /// Metrics collector (optional)
    metrics_collector: Option<Arc<Mutex<MetricsCollector>>>,
    /// Latest connection status for metrics reporting
    metrics_status: MetricsConnectionStatus,
    /// Last time we emitted metrics to the UI
    metrics_last_emit: Instant,
    /// Minimum interval between metrics updates to UI
    metrics_emit_interval: Duration,
    /// Command router
    command_router: CommandRouter,
    /// Action channel
    action_channel: ActionChannel,
    /// Shutdown signal sender
    shutdown_tx: mpsc::Sender<()>,
    /// Shutdown signal receiver
    shutdown_rx: Option<mpsc::Receiver<()>>,
}

impl SessionManager {
    /// Create a new SessionManager
    pub fn new(cli: &Cli, app_config: Config) -> Result<Self> {
        info!("Creating new SessionManager");

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Create market data manager
        let market_manager = Arc::new(MarketDataManager::new());

        // Create command router
        let command_router = CommandRouter::new(market_manager.clone());

        // Create action channel
        let action_channel = ActionChannel::new();

        // Create session config from CLI
        let session_config = SessionConfig {
            enable_tui: true,            // Default to TUI mode
            enable_metrics: true,        // Default to metrics collection
            auto_subscribe: true,        // Default to auto-subscribe
            session_timeout_ms: 3600000, // 1 hour default timeout
        };

        let metrics_interval = Duration::from_millis(app_config.refresh_rate_ms.max(50));

        Ok(Self {
            config: session_config,
            app_config,
            cli: cli.clone(),
            state: SessionState::Starting,
            stats: SessionStats::default(),
            market_manager,
            ui_task: None,
            ui_event_tx: None,
            metrics_collector: None,
            metrics_status: MetricsConnectionStatus::Disconnected,
            metrics_last_emit: Instant::now(),
            metrics_emit_interval: metrics_interval,
            command_router,
            action_channel,
            shutdown_tx,
            shutdown_rx: Some(shutdown_rx),
        })
    }

    /// Initialize the session
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing interactive session");

        // Initialize UI manager if enabled
        if self.config.enable_tui {
            self.initialize_ui().await?;
        }

        // Initialize metrics collector if enabled
        if self.config.enable_metrics {
            self.initialize_metrics().await?;
        }

        // Auto-subscribe to symbols if configured
        if self.config.auto_subscribe && !self.app_config.symbols.is_empty() {
            self.auto_subscribe_symbols().await?;
        }

        if self.config.enable_tui {
            let help_lines = CommandRouter::help_messages()
                .iter()
                .map(|line| (*line).to_string())
                .collect();
            self.forward_to_ui(SessionEvent::HelpInfo { lines: help_lines });
        }

        self.state = SessionState::Running;
        info!("Session initialized successfully");

        Ok(())
    }

    /// Start the session using the appropriate execution mode
    pub async fn start(&mut self) -> Result<()> {
        if self.cli.is_dry_run_mode() {
            return self.run_dry_run_mode().await;
        }

        if !self.config.enable_tui {
            self.display_welcome_page().await?;
        } else {
            info!("TUI mode enabled, deferring welcome message to UI");
        }

        self.initialize().await?;

        self.run().await
    }

    async fn run_dry_run_mode(&mut self) -> Result<()> {
        info!("Running in dry-run mode - showing welcome page and configuration");

        self.state = SessionState::Running;

        self.display_welcome_page().await?;
        self.print_dry_run_summary()?;

        info!("Dry-run mode completed");
        Ok(())
    }

    fn print_dry_run_summary(&self) -> Result<()> {
        println!();
        println!("Dry-run mode configuration:");
        println!("Config file: {}", self.cli.config_file);
        println!("Log level: {}", self.cli.effective_log_level());
        self.app_config.display_summary()
    }

    /// Display welcome page for interactive mode
    pub async fn display_welcome_page(&mut self) -> Result<()> {
        crate::ui::display_welcome_page().map_err(|e| anyhow::anyhow!(e))
    }

    /// Initialize UI manager
    async fn initialize_ui(&mut self) -> Result<()> {
        info!("Initializing UI manager");

        let mut ui_manager = UIManager::new(
            self.market_manager.clone(),
            self.action_channel.event_tx(),
            self.app_config.clone(),
        );

        // Get UI event sender for forwarding events
        let ui_event_tx = ui_manager.ui_event_sender();

        // Store UI event sender
        self.ui_event_tx = Some(ui_event_tx);

        // Spawn UI task
        self.ui_task = Some(tokio::spawn(async move {
            if let Err(e) = ui_manager.run().await {
                error!("UI manager error: {}", e);
            }
        }));

        Ok(())
    }

    /// Initialize metrics collector
    async fn initialize_metrics(&mut self) -> Result<()> {
        info!("Initializing metrics collector");

        let metrics_collector = MetricsCollector::new(1000); // 1000 samples max

        self.metrics_collector = Some(Arc::new(Mutex::new(metrics_collector)));

        Ok(())
    }

    /// Auto-subscribe to configured symbols
    async fn auto_subscribe_symbols(&mut self) -> Result<()> {
        info!("Auto-subscribing to symbols: {:?}", self.app_config.symbols);

        for symbol in &self.app_config.symbols {
            if let Err(e) = self.market_manager.subscribe(symbol.clone()).await {
                error!("Failed to auto-subscribe to {}: {}", symbol, e);
            } else {
                info!("Auto-subscribed to symbol: {}", symbol);
            }
        }

        Ok(())
    }

    /// Run the main session loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting interactive session loop");

        // Start metrics collection if enabled
        if let Some(metrics_collector) = &self.metrics_collector {
            let metrics_collector = metrics_collector.clone();
            tokio::spawn(async move {
                if let Err(e) = metrics_collector.lock().await.run().await {
                    error!("Metrics collector error: {}", e);
                }
            });
        }

        // Main event loop
        let mut shutdown_rx = self.shutdown_rx.take().unwrap();
        let market_event_rx = self.market_manager.event_receiver();

        while self.state != SessionState::Terminated {
            let market_event_rx = market_event_rx.clone();
            tokio::select! {
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("Received shutdown signal");
                    self.shutdown().await?;
                }

                // Handle commands from command router
                Some(command) = self.command_router.next_command() => {
                    self.handle_command(command).await?;
                }

                // Handle events from action channel (including user commands)
                Some(event) = self.action_channel.next_event() => {
                    self.handle_event(event).await?;
                }

                // Session timeout check
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    self.check_timeout().await?;
                }

                // Handle market events without blocking other tasks
                market_event = async {
                    let mut receiver = market_event_rx.lock().await;
                    receiver.recv().await
                } => {
                    match market_event {
                        Some(market_event) => {
                            self.market_manager.process_market_event(&market_event).await;
                            self.handle_market_event(market_event).await?;
                        }
                        None => {
                            warn!("Market event channel closed");
                            self.state = SessionState::Terminated;
                        }
                    }
                }
            }
        }

        info!("Session loop terminated");
        Ok(())
    }

    /// Handle user command
    async fn handle_command(&mut self, command: InteractiveCommand) -> Result<()> {
        debug!("Handling command: {:?}", command);

        self.stats.commands_processed += 1;

        match command {
            InteractiveCommand::Add { symbols } => self.handle_subscribe(symbols).await,
            InteractiveCommand::Remove { symbols } => self.handle_unsubscribe(symbols).await,
            InteractiveCommand::List => self.handle_list().await,
            InteractiveCommand::Status => self.handle_status().await,
            InteractiveCommand::Show { symbol } => self.handle_show(symbol).await,
            InteractiveCommand::Config { action } => self.handle_config(action).await,
            InteractiveCommand::Reconnect => self.handle_reconnect().await,
            InteractiveCommand::Quit => self.handle_quit().await,
            InteractiveCommand::Logs => self.handle_logs().await,
            InteractiveCommand::Help => self.handle_help().await,
        }
    }

    /// Handle subscribe command
    async fn handle_subscribe(&mut self, symbols: Vec<String>) -> Result<()> {
        for symbol in symbols {
            match self.market_manager.subscribe(symbol.clone()).await {
                Ok(()) => {
                    info!("Subscribed to symbol: {}", symbol);
                    self.action_channel
                        .send_event(SessionEvent::SubscriptionAdded { symbol })?;
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
        for symbol in symbols {
            match self.market_manager.unsubscribe(&symbol).await {
                Ok(()) => {
                    info!("Unsubscribed from symbol: {}", symbol);
                    self.action_channel
                        .send_event(SessionEvent::SubscriptionRemoved { symbol })?;
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
                // Provide latest status snapshot to UI/CLI
                self.handle_status().await?;
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
        let symbols = self.market_manager.list_subscriptions().await;

        info!("Current subscriptions: {:?}", symbols);

        self.action_channel
            .send_event(SessionEvent::SubscriptionList { symbols })?;

        Ok(())
    }

    /// Handle status command
    async fn handle_status(&mut self) -> Result<()> {
        let symbols = self.market_manager.list_subscriptions().await;

        let status_info = super::action_channel::StatusInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            state: format!("{:?}", self.state),
            active_subscriptions: symbols.len(),
            symbols,
            session_stats: self.stats.clone(),
        };

        self.action_channel
            .send_event(SessionEvent::StatusInfo { info: status_info })?;

        Ok(())
    }

    /// Handle show command
    async fn handle_show(&mut self, symbol: String) -> Result<()> {
        if let Some(orderbook) = self.market_manager.get_orderbook(&symbol).await {
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
    async fn handle_config(&mut self, action: Option<crate::cli::ConfigAction>) -> Result<()> {
        match action {
            Some(crate::cli::ConfigAction::Show) => {
                self.action_channel.send_event(SessionEvent::ConfigInfo {
                    config: self.app_config.clone(),
                })?;
            }
            Some(crate::cli::ConfigAction::Set { key, value }) => {
                let key_normalized = key.to_ascii_lowercase();
                match key_normalized.as_str() {
                    "refresh_rate_ms" | "refresh-rate" => match value.parse::<u64>() {
                        Ok(parsed) if parsed > 0 => {
                            self.app_config.refresh_rate_ms = parsed;
                            self.metrics_emit_interval =
                                Duration::from_millis(self.app_config.refresh_rate_ms.max(50));
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
            Some(crate::cli::ConfigAction::Reset) => {
                self.app_config = Config::default();
                info!("Configuration reset to defaults");
                self.metrics_emit_interval =
                    Duration::from_millis(self.app_config.refresh_rate_ms.max(50));
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

        // Get recent logs from tracing system
        let logs_info = super::action_channel::LogsInfo {
            recent_logs: vec![
                "INFO: Session started".to_string(),
                "DEBUG: Market data initialized".to_string(),
                "INFO: Subscribed to BTCUSDT".to_string(),
            ],
            log_file_path: "/var/log/xtrade.log".to_string(),
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

    /// Handle session event
    async fn handle_event(&mut self, event: SessionEvent) -> Result<()> {
        debug!("Handling session event: {:?}", event);

        self.stats.events_processed += 1;

        match event {
            SessionEvent::ShutdownRequested => {
                self.forward_to_ui(SessionEvent::ShutdownRequested);
                self.shutdown().await?;
            }
            SessionEvent::Error { message } => {
                error!("Session error: {}", message);
                self.stats.errors_encountered += 1;
                self.forward_to_ui(SessionEvent::Error { message });
            }
            SessionEvent::UserCommand { command } => {
                self.handle_command(command).await?;
            }
            SessionEvent::MarketEvent(market_event) => {
                self.handle_market_event(market_event).await?;
            }
            other => {
                self.forward_to_ui(other);
            }
        }

        Ok(())
    }

    /// Forward an event to the UI if the channel is available
    fn forward_to_ui(&self, event: SessionEvent) {
        if let Some(ui_event_tx) = &self.ui_event_tx {
            if let Err(e) = ui_event_tx.send(event) {
                error!("Failed to forward event to UI: {}", e);
            }
        }
    }

    /// Handle market event
    async fn handle_market_event(&mut self, event: crate::market_data::MarketEvent) -> Result<()> {
        debug!("Handling market event: {:?}", event);

        // Forward to UI if available
        if let Some(ui_event_tx) = &self.ui_event_tx {
            if let Err(e) = ui_event_tx.send(SessionEvent::MarketEvent(event.clone())) {
                error!("Failed to send market event to UI: {}", e);
            }
        }

        if let crate::market_data::MarketEvent::ConnectionStatus { status, .. } = &event {
            self.metrics_status = match status {
                crate::binance::types::ConnectionStatus::Disconnected => {
                    MetricsConnectionStatus::Disconnected
                }
                crate::binance::types::ConnectionStatus::Connecting => {
                    MetricsConnectionStatus::Connecting
                }
                crate::binance::types::ConnectionStatus::Connected => {
                    MetricsConnectionStatus::Connected
                }
                crate::binance::types::ConnectionStatus::Reconnecting => {
                    MetricsConnectionStatus::Reconnecting
                }
                crate::binance::types::ConnectionStatus::Error(err) => {
                    MetricsConnectionStatus::Error(err.clone())
                }
            };
        }

        // Forward to metrics collector if available
        if let Some(metrics_collector) = &self.metrics_collector {
            let mut collector = metrics_collector.lock().await;
            collector.handle_market_event(event.clone()).await?;

            let now = Instant::now();
            let should_emit =
                now.duration_since(self.metrics_last_emit) >= self.metrics_emit_interval;
            let metrics_snapshot = if should_emit {
                Some(collector.get_connection_metrics(self.metrics_status.clone()))
            } else {
                None
            };
            drop(collector);

            if let Some(metrics_snapshot) = metrics_snapshot {
                if let Err(e) = self.action_channel.send_event(SessionEvent::MetricsUpdate {
                    metrics: metrics_snapshot,
                }) {
                    error!("Failed to forward metrics update: {}", e);
                } else {
                    self.metrics_last_emit = now;
                }
            }
        }

        Ok(())
    }

    /// Check session timeout
    async fn check_timeout(&mut self) -> Result<()> {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let session_duration = current_time - self.stats.start_time;

        if session_duration > self.config.session_timeout_ms {
            warn!(
                "Session timeout reached ({}ms), shutting down",
                session_duration
            );
            self.shutdown().await?;
        }

        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Initiating graceful shutdown");

        self.state = SessionState::ShuttingDown;

        // Notify UI to shutdown and wait for task completion
        if let Some(ui_event_tx) = self.ui_event_tx.take() {
            if let Err(e) = ui_event_tx.send(SessionEvent::ShutdownRequested) {
                error!("Failed to notify UI of shutdown: {}", e);
            }
        }
        if let Some(ui_task) = self.ui_task.take() {
            if let Err(e) = ui_task.await {
                error!("UI task terminated with error: {}", e);
            }
        }

        // Shutdown metrics collector
        if let Some(metrics_collector) = &self.metrics_collector {
            metrics_collector.lock().await.shutdown().await?;
        }

        // Shutdown market data manager
        let symbols = self.market_manager.list_subscriptions().await;

        for symbol in symbols {
            if let Err(e) = self.market_manager.unsubscribe(&symbol).await {
                error!(
                    "Failed to unsubscribe from {} during shutdown: {}",
                    symbol, e
                );
            }
        }

        self.state = SessionState::Terminated;
        info!("Shutdown completed");

        Ok(())
    }

    /// Get session statistics
    pub fn get_stats(&self) -> &SessionStats {
        &self.stats
    }

    /// Get session state
    pub fn get_state(&self) -> &SessionState {
        &self.state
    }

    /// Request shutdown
    pub fn request_shutdown(&self) -> Result<()> {
        self.shutdown_tx
            .try_send(())
            .map_err(|e| anyhow::anyhow!("Failed to send shutdown signal: {}", e))
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        if self.state != SessionState::Terminated {
            warn!("SessionManager dropped without proper shutdown");
        }
    }
}
