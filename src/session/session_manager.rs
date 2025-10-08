//! Session Manager for interactive terminal session lifecycle management

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};

use crate::cli::Cli;
use crate::config::Config;
use crate::market_data::MarketDataManager;
use crate::metrics::MetricsCollector;
use crate::ui::ui_manager::UIManager;

use super::action_channel::ActionChannel;
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
    market_manager: Arc<Mutex<MarketDataManager>>,
    /// UI manager (optional)
    ui_manager: Option<Arc<Mutex<UIManager>>>,
    /// Metrics collector (optional)
    metrics_collector: Option<Arc<Mutex<MetricsCollector>>>,
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
        let market_manager = Arc::new(Mutex::new(MarketDataManager::new()));

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

        Ok(Self {
            config: session_config,
            app_config,
            cli: cli.clone(),
            state: SessionState::Starting,
            stats: SessionStats::default(),
            market_manager,
            ui_manager: None,
            metrics_collector: None,
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

        self.state = SessionState::Running;
        info!("Session initialized successfully");

        Ok(())
    }

    /// Start the session using the appropriate execution mode
    pub async fn start(&mut self) -> Result<()> {
        if self.cli.is_dry_run_mode() {
            return self.run_dry_run_mode().await;
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

        let ui_manager =
            UIManager::new(self.market_manager.clone(), self.action_channel.event_tx());

        // Get UI event sender for forwarding events
        let ui_event_tx = ui_manager.ui_event_tx();

        // Store UI manager
        self.ui_manager = Some(Arc::new(Mutex::new(ui_manager)));

        // Forward session events to UI
        let ui_event_tx_clone = ui_event_tx.clone();
        let mut action_channel = self.action_channel.clone();

        tokio::spawn(async move {
            if let Some(mut event_rx) = action_channel.event_rx() {
                while let Some(event) = event_rx.recv().await {
                    if let Err(e) = ui_event_tx_clone.send(event) {
                        error!("Failed to forward event to UI: {}", e);
                    }
                }
            }
        });

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

        let manager = self.market_manager.lock().await;

        for symbol in &self.app_config.symbols {
            if let Err(e) = manager.subscribe(symbol.clone()).await {
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

        // Display welcome page
        self.display_welcome_page().await?;

        // Start UI if enabled
        if let Some(ui_manager) = &self.ui_manager {
            let ui_manager = ui_manager.clone();
            tokio::spawn(async move {
                if let Err(e) = ui_manager.lock().await.run().await {
                    error!("UI manager error: {}", e);
                }
            });
        }

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

        while self.state != SessionState::Terminated {
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

                // Handle events from action channel
                Some(event) = self.action_channel.next_event() => {
                    self.handle_event(event).await?;
                }

                // Session timeout check
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    self.check_timeout().await?;
                }
            }

            // Handle market events separately to avoid borrowing conflicts
            let market_event = {
                let mut market_manager = self.market_manager.lock().await;
                market_manager.next_event().await
            };

            if let Some(market_event) = market_event {
                self.handle_market_event(market_event).await?;
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
            InteractiveCommand::Quit => self.handle_quit().await,
            InteractiveCommand::Logs => self.handle_logs().await,
        }
    }

    /// Handle subscribe command
    async fn handle_subscribe(&mut self, symbols: Vec<String>) -> Result<()> {
        let manager = self.market_manager.lock().await;

        for symbol in symbols {
            match manager.subscribe(symbol.clone()).await {
                Ok(()) => {
                    info!("Subscribed to symbol: {}", symbol);
                    self.action_channel.send_event(
                        super::action_channel::SessionEvent::SubscriptionAdded { symbol },
                    )?;
                }
                Err(e) => {
                    error!("Failed to subscribe to {}: {}", symbol, e);
                    self.action_channel
                        .send_event(super::action_channel::SessionEvent::Error {
                            message: format!("Failed to subscribe to {}: {}", symbol, e),
                        })?;
                }
            }
        }

        Ok(())
    }

    /// Handle unsubscribe command
    async fn handle_unsubscribe(&mut self, symbols: Vec<String>) -> Result<()> {
        let manager = self.market_manager.lock().await;

        for symbol in symbols {
            match manager.unsubscribe(&symbol).await {
                Ok(()) => {
                    info!("Unsubscribed from symbol: {}", symbol);
                    self.action_channel.send_event(
                        super::action_channel::SessionEvent::SubscriptionRemoved { symbol },
                    )?;
                }
                Err(e) => {
                    error!("Failed to unsubscribe from {}: {}", symbol, e);
                    self.action_channel
                        .send_event(super::action_channel::SessionEvent::Error {
                            message: format!("Failed to unsubscribe from {}: {}", symbol, e),
                        })?;
                }
            }
        }

        Ok(())
    }

    /// Handle list command
    async fn handle_list(&mut self) -> Result<()> {
        let manager = self.market_manager.lock().await;
        let symbols = manager.list_subscriptions().await;

        info!("Current subscriptions: {:?}", symbols);

        self.action_channel
            .send_event(super::action_channel::SessionEvent::SubscriptionList { symbols })?;

        Ok(())
    }

    /// Handle status command
    async fn handle_status(&mut self) -> Result<()> {
        let manager = self.market_manager.lock().await;
        let symbols = manager.list_subscriptions().await;

        let status_info = super::action_channel::StatusInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            state: format!("{:?}", self.state),
            active_subscriptions: symbols.len(),
            symbols,
            session_stats: self.stats.clone(),
        };

        self.action_channel
            .send_event(super::action_channel::SessionEvent::StatusInfo { info: status_info })?;

        Ok(())
    }

    /// Handle show command
    async fn handle_show(&mut self, symbol: String) -> Result<()> {
        let manager = self.market_manager.lock().await;

        if let Some(orderbook) = manager.get_orderbook(&symbol).await {
            self.action_channel
                .send_event(super::action_channel::SessionEvent::SymbolDetails {
                    symbol,
                    orderbook: Some(orderbook),
                })?;
        } else {
            self.action_channel
                .send_event(super::action_channel::SessionEvent::SymbolDetails {
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
                self.action_channel.send_event(
                    super::action_channel::SessionEvent::ConfigInfo {
                        config: self.app_config.clone(),
                    },
                )?;
            }
            Some(crate::cli::ConfigAction::Set { key, value }) => {
                warn!("Config set command not yet implemented: {}={}", key, value);
                self.action_channel
                    .send_event(super::action_channel::SessionEvent::Error {
                        message: format!("Config set not implemented: {}={}", key, value),
                    })?;
            }
            Some(crate::cli::ConfigAction::Reset) => {
                self.app_config = Config::default();
                info!("Configuration reset to defaults");
                self.action_channel
                    .send_event(super::action_channel::SessionEvent::ConfigReset)?;
            }
            None => {
                self.action_channel
                    .send_event(super::action_channel::SessionEvent::ConfigHelp)?;
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
            .send_event(super::action_channel::SessionEvent::LogsInfo { info: logs_info })?;

        Ok(())
    }

    /// Handle session event
    async fn handle_event(&mut self, event: super::action_channel::SessionEvent) -> Result<()> {
        debug!("Handling session event: {:?}", event);

        self.stats.events_processed += 1;

        match event {
            super::action_channel::SessionEvent::ShutdownRequested => {
                self.shutdown().await?;
            }
            super::action_channel::SessionEvent::Error { message } => {
                error!("Session error: {}", message);
                self.stats.errors_encountered += 1;
            }
            super::action_channel::SessionEvent::UserCommand { command } => {
                self.handle_command(command).await?;
            }
            _ => {
                // Forward event to UI if available
                if let Some(ui_manager) = &self.ui_manager {
                    ui_manager.lock().await.handle_event(event).await?;
                }
            }
        }

        Ok(())
    }

    /// Handle market event
    async fn handle_market_event(&mut self, event: crate::market_data::MarketEvent) -> Result<()> {
        debug!("Handling market event: {:?}", event);

        // Forward to UI if available
        if let Some(ui_manager) = &self.ui_manager {
            ui_manager
                .lock()
                .await
                .handle_market_event(event.clone())
                .await?;
        }

        // Forward to metrics collector if available
        if let Some(metrics_collector) = &self.metrics_collector {
            metrics_collector
                .lock()
                .await
                .handle_market_event(event)
                .await?;
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

        // Shutdown UI manager
        if let Some(ui_manager) = &self.ui_manager {
            ui_manager.lock().await.shutdown().await?;
        }

        // Shutdown metrics collector
        if let Some(metrics_collector) = &self.metrics_collector {
            metrics_collector.lock().await.shutdown().await?;
        }

        // Shutdown market data manager
        let manager = self.market_manager.lock().await;
        let symbols = manager.list_subscriptions().await;

        for symbol in symbols {
            if let Err(e) = manager.unsubscribe(&symbol).await {
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
