//! UI Manager for interactive terminal interface

use anyhow::Result;
use std::io::{self as stdio, Write};
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};

use crate::cli::Cli;
use crate::config::Config;
use crate::market_data::MarketDataManager;
use crate::market_data::MarketEvent;
use crate::session::action_channel::{SessionEvent, StatusInfo};

use super::AppState;

/// UI Manager for managing the terminal interface
pub struct UIManager {
    /// Market data manager reference
    market_manager: Arc<Mutex<MarketDataManager>>,
    /// Event sender for session events (UI -> Session)
    session_event_tx: mpsc::UnboundedSender<SessionEvent>,
    /// Event sender for UI events (Session -> UI)
    ui_event_tx: mpsc::UnboundedSender<SessionEvent>,
    /// Event receiver for UI events
    event_rx: Option<mpsc::UnboundedReceiver<SessionEvent>>,
    /// Market event receiver
    market_event_rx: Option<mpsc::UnboundedReceiver<MarketEvent>>,
    /// Application state
    app_state: AppState,
    /// UI rendering state
    render_state: RenderState,
    /// Dry-run mode flag
    dry_run: bool,
}

/// UI rendering state
#[derive(Debug, Clone)]
pub struct RenderState {
    pub should_quit: bool,
    pub should_redraw: bool,
    pub last_render_time: u64,
    pub render_count: u64,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub pending_messages: Vec<String>,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            should_quit: false,
            should_redraw: true,
            last_render_time: 0,
            render_count: 0,
            error_message: None,
            info_message: None,
            pending_messages: Vec::new(),
        }
    }
}

impl RenderState {
    fn queue_message(&mut self, message: impl Into<String>) {
        self.pending_messages.push(message.into());
        self.should_redraw = true;
    }

    fn drain_messages(&mut self) -> Vec<String> {
        let mut messages = Vec::new();
        std::mem::swap(&mut messages, &mut self.pending_messages);
        messages
    }
}

impl UIManager {
    /// Create a new UIManager
    pub fn new(
        market_manager: Arc<Mutex<MarketDataManager>>,
        session_event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Self {
        // Create event channels
        let (ui_event_tx, ui_event_rx) = mpsc::unbounded_channel();
        let (_market_event_tx, market_event_rx) = mpsc::unbounded_channel();

        Self {
            market_manager,
            session_event_tx,
            ui_event_tx,
            event_rx: Some(ui_event_rx),
            market_event_rx: Some(market_event_rx),
            app_state: AppState::new(Vec::new()),
            render_state: RenderState::default(),
            dry_run: false,
        }
    }

    /// Create a new UIManager with dry-run mode
    pub fn new_with_dry_run(
        market_manager: Arc<Mutex<MarketDataManager>>,
        session_event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Self {
        let mut ui_manager = Self::new(market_manager, session_event_tx);
        ui_manager.dry_run = true;
        ui_manager
    }

    /// Get session event sender (UI -> Session)
    pub fn session_event_sender(&self) -> mpsc::UnboundedSender<SessionEvent> {
        self.session_event_tx.clone()
    }

    /// Get UI event sender (Session -> UI)
    pub fn ui_event_sender(&self) -> mpsc::UnboundedSender<SessionEvent> {
        self.ui_event_tx.clone()
    }

    /// Get market event sender
    pub fn market_event_tx(&self) -> mpsc::UnboundedSender<MarketEvent> {
        // Create a new sender since we don't store it
        let (tx, _) = mpsc::unbounded_channel();
        tx
    }

    /// Run the UI manager
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting UI manager");

        // Handle dry-run mode
        if self.dry_run {
            // For internal UI manager dry-run mode, use default CLI and config
            let cli = crate::cli::Cli::parse_args();
            let config = crate::config::Config::default();
            return self.run_dry_run(&cli, &config).await;
        }

        // Initialize UI components
        self.initialize_ui().await?;

        // Main UI loop
        self.run_ui_loop().await?;

        info!("UI manager stopped");
        Ok(())
    }

    /// Run in dry-run mode (show welcome page and configuration)
    pub async fn run_dry_run(&mut self, cli: &Cli, config: &Config) -> Result<()> {
        info!("Running UI in dry-run mode");

        // Display welcome page
        self.display_welcome_page().await?;

        // Display configuration with actual data
        self.display_configuration_with_data(config).await?;

        // Display dry-run configuration
        self.display_dry_run_config(cli).await?;

        info!("Dry-run mode completed");
        Ok(())
    }

    /// Display welcome page
    async fn display_welcome_page(&mut self) -> Result<()> {
        crate::ui::display_welcome_page().map_err(|e| anyhow::anyhow!(e))
    }

    /// Display dry-run configuration
    pub async fn display_dry_run_config(&self, cli: &Cli) -> Result<()> {
        println!("\nDry-run mode configuration:");
        println!("Config file: {}", cli.config_file);
        println!("Log level: {}", cli.effective_log_level());
        println!("Configuration loaded successfully");
        Ok(())
    }

    /// Display configuration with actual config data
    pub async fn display_configuration_with_data(&self, config: &Config) -> Result<()> {
        println!("┌─ Configuration Overview ───────────────────────────────────────────┐");
        println!("│                                                                     │");
        println!("│   Configuration loaded successfully!                               │");
        println!("│                                                                     │");
        println!("│   Default symbols: {:<40} │", config.symbols.join(", "));
        println!(
            "│   Refresh rate: {}ms                                               │",
            config.refresh_rate_ms
        );
        println!(
            "│   OrderBook depth: {} levels                                        │",
            config.orderbook_depth
        );
        println!("│   Log level: {:<50} │", config.log_level);
        println!("│                                                                     │");
        println!("│   Binance API:                                                     │");
        println!("│   • WebSocket: {:<50} │", config.binance.ws_url);
        println!("│   • REST API: {:<50} │", config.binance.rest_url);
        println!("│                                                                     │");
        println!("│   UI Settings:                                                     │");
        println!(
            "│   • Colors: {:<50} │",
            if config.ui.enable_colors {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!(
            "│   • Update rate: {} FPS                                             │",
            config.ui.update_rate_fps
        );
        println!(
            "│   • Sparkline points: {:<40} │",
            config.ui.sparkline_points
        );
        println!("│                                                                     │");
        println!("└────────────────────────────────────────────────────────────────────┘");
        println!();

        Ok(())
    }

    /// Initialize UI components
    async fn initialize_ui(&mut self) -> Result<()> {
        info!("Initializing UI components");

        // Load initial symbols from market manager
        let manager = self.market_manager.lock().await;
        let symbols = manager.list_subscriptions().await;

        self.app_state.symbols = symbols;

        info!(
            "UI initialized with {} symbols",
            self.app_state.symbols.len()
        );

        self.render_state
            .queue_message("Interactive mode ready. Type /help for commands.");

        Ok(())
    }

    /// Main UI rendering loop
    async fn run_ui_loop(&mut self) -> Result<()> {
        info!("Starting UI rendering loop");

        let ui_shutdown_tx = self.ui_event_tx.clone();
        let session_shutdown_tx = self.session_event_tx.clone();

        tokio::spawn(async move {
            if let Err(e) = tokio::signal::ctrl_c().await {
                tracing::error!("Failed to listen for Ctrl+C: {}", e);
                return;
            }

            tracing::info!("Ctrl+C received, initiating shutdown");
            let _ = ui_shutdown_tx.send(SessionEvent::ShutdownRequested);
            let _ = session_shutdown_tx.send(SessionEvent::ShutdownRequested);
        });

        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<String>();
        let input_task = tokio::spawn(async move {
            let mut reader = io::BufReader::new(tokio::io::stdin());

            loop {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        // EOF or stdin closed; stop reading.
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim().to_string();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if input_tx.send(trimmed).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to read user input: {}", e);
                        break;
                    }
                }
            }
        });

        while !self.render_state.should_quit {
            // Check for events
            self.process_events().await?;

            // Render if needed
            if self.render_state.should_redraw {
                self.render().await?;
                self.render_state.should_redraw = false;
            }

            // Handle user input
            if !self.render_state.should_quit {
                self.handle_input(&mut input_rx).await?;
            }

            // Sleep to prevent busy waiting
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        input_task.abort();
        if let Err(e) = input_task.await {
            if !e.is_cancelled() {
                warn!("Input task ended with error: {}", e);
            }
        }

        Ok(())
    }

    /// Process incoming events
    async fn process_events(&mut self) -> Result<()> {
        // Process session events
        let mut events_to_process = Vec::new();
        if let Some(event_rx) = &mut self.event_rx {
            while let Ok(event) = event_rx.try_recv() {
                events_to_process.push(event);
            }
        }

        for event in events_to_process {
            self.handle_event(event).await?;
        }

        // Process market events
        let mut market_events_to_process = Vec::new();
        if let Some(market_event_rx) = &mut self.market_event_rx {
            while let Ok(event) = market_event_rx.try_recv() {
                market_events_to_process.push(event);
            }
        }

        for event in market_events_to_process {
            self.handle_market_event(event).await?;
        }

        Ok(())
    }

    /// Render the UI
    async fn render(&mut self) -> Result<()> {
        debug!("Rendering UI (render #{})", self.render_state.render_count);

        // Update render statistics
        self.render_state.render_count += 1;
        self.render_state.last_render_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Simple CLI output for now
        self.render_cli_output().await?;

        Ok(())
    }

    /// Render simple CLI output (placeholder for TUI)
    async fn render_cli_output(&mut self) -> Result<()> {
        if self.render_state.render_count == 1 {
            println!("XTrade Market Data Monitor (interactive mode)");
        }

        let _ = self.render_state.error_message.take();
        let _ = self.render_state.info_message.take();

        for message in self.render_state.drain_messages() {
            println!("{}", message);
        }

        stdio::stdout()
            .flush()
            .map_err(|e| anyhow::anyhow!("Failed to flush stdout: {}", e))?;

        Ok(())
    }

    /// Handle user input from the buffered channel
    async fn handle_input(&mut self, input_rx: &mut mpsc::UnboundedReceiver<String>) -> Result<()> {
        loop {
            match input_rx.try_recv() {
                Ok(line) => {
                    self.process_user_command(&line).await?;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    warn!("Input channel disconnected");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Process user command from input
    async fn process_user_command(&mut self, input: &str) -> Result<()> {
        debug!("Processing user command: {}", input);

        // Create a temporary command router to parse the command
        let command_router =
            crate::session::command_router::CommandRouter::new(self.market_manager.clone());
        let command_result = command_router.parse_interactive_command(input);

        match command_result {
            Ok(Some(command)) => {
                // Send the command to the session manager via event channel
                match command {
                    crate::session::command_router::InteractiveCommand::Quit => {
                        // Handle quit command
                        info!("User requested quit");
                        self.render_state.should_quit = true;
                        self.session_event_tx
                            .send(SessionEvent::ShutdownRequested)
                            .map_err(|e| {
                                anyhow::anyhow!("Failed to send shutdown request: {}", e)
                            })?;
                    }
                    _ => {
                        // Forward other commands to session manager
                        self.session_event_tx
                            .send(SessionEvent::UserCommand { command })
                            .map_err(|e| anyhow::anyhow!("Failed to send user command: {}", e))?;
                    }
                }
            }
            Ok(None) => {
                // Empty command or help command (help returns None)
                debug!("Empty command or help requested");
            }
            Err(e) => {
                // Command parsing error
                let message = format!("Command error: {}", e);
                self.render_state.error_message = Some(message.clone());
                self.render_state.queue_message(message);
                error!("Command parsing error: {}", e);
            }
        }

        Ok(())
    }

    /// Handle status command
    #[allow(dead_code)]
    async fn handle_status_command(&mut self) -> Result<()> {
        let manager = self.market_manager.lock().await;
        let symbols = manager.list_subscriptions().await;

        let status_info = StatusInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            state: "Running".to_string(),
            active_subscriptions: symbols.len(),
            symbols: symbols.clone(),
            session_stats: crate::session::session_manager::SessionStats::default(),
        };

        self.session_event_tx
            .send(SessionEvent::StatusInfo { info: status_info })
            .map_err(|e| anyhow::anyhow!("Failed to send status event: {}", e))?;

        Ok(())
    }

    /// Handle session event
    pub async fn handle_event(&mut self, event: SessionEvent) -> Result<()> {
        debug!("Handling UI event: {:?}", event);

        match event {
            SessionEvent::ShutdownRequested => {
                self.render_state
                    .queue_message("Shutdown requested. Exiting interactive session...");
                self.render_state.should_quit = true;
                info!("UI received shutdown request");
            }
            SessionEvent::Error { message } => {
                let formatted = format!("Error: {}", message);
                self.render_state.error_message = Some(formatted.clone());
                self.render_state.queue_message(formatted);
            }
            SessionEvent::SubscriptionAdded { symbol } => {
                if !self.app_state.symbols.contains(&symbol) {
                    self.app_state.symbols.push(symbol.clone());
                    self.render_state
                        .queue_message(format!("Subscribed to {}", symbol));
                }
            }
            SessionEvent::SubscriptionRemoved { symbol } => {
                self.app_state.symbols.retain(|s| s != &symbol);
                self.render_state
                    .queue_message(format!("Unsubscribed from {}", symbol));
            }
            SessionEvent::SubscriptionList { symbols } => {
                self.app_state.symbols = symbols.clone();
                self.render_state.queue_message(format!(
                    "Active subscriptions: {}",
                    if symbols.is_empty() {
                        "none".to_string()
                    } else {
                        symbols.join(", ")
                    }
                ));
            }
            SessionEvent::StatusInfo { info } => {
                let message = format!(
                    "Status → Version {} | State {} | Subscriptions {}",
                    info.version, info.state, info.active_subscriptions
                );
                self.render_state.info_message = Some(message.clone());
                self.render_state.queue_message(message);
            }
            SessionEvent::UIModeChanged { enable_tui } => {
                info!("UI mode changed: TUI {}", enable_tui);
                self.render_state
                    .queue_message(format!("UI mode changed: TUI {}", enable_tui));
            }
            SessionEvent::LogsInfo { info } => {
                let message = format!(
                    "Recent logs ({}): {}",
                    info.log_level,
                    info.recent_logs.join(", ")
                );
                self.render_state.info_message = Some(message.clone());
                self.render_state.queue_message(message);
            }
            SessionEvent::MarketEvent(event) => {
                self.handle_market_event(event).await?;
            }
            _ => {
                debug!("Unhandled UI event: {:?}", event);
            }
        }

        Ok(())
    }

    /// Handle market event
    pub async fn handle_market_event(&mut self, event: MarketEvent) -> Result<()> {
        debug!("Handling market event: {:?}", event);

        match event {
            MarketEvent::PriceUpdate {
                symbol,
                price,
                time: _,
            } => {
                // Update market data state
                if let Some(market_data) = self.app_state.market_data.get_mut(&symbol) {
                    market_data.price = price;
                    market_data.price_history.push(price);

                    // Keep history size manageable
                    if market_data.price_history.len() > 100 {
                        market_data.price_history.remove(0);
                    }
                } else {
                    // Create new market data entry
                    self.app_state.market_data.insert(
                        symbol.clone(),
                        super::MarketDataState {
                            symbol: symbol.clone(),
                            price,
                            change_percent: 0.0,
                            volume_24h: 0.0,
                            high_24h: 0.0,
                            low_24h: 0.0,
                            orderbook: None,
                            price_history: vec![price],
                        },
                    );
                    self.render_state
                        .queue_message(format!("Market data stream started for {}", symbol));
                }
            }
            MarketEvent::OrderBookUpdate { symbol, orderbook } => {
                // Update orderbook
                if let Some(market_data) = self.app_state.market_data.get_mut(&symbol) {
                    market_data.orderbook = Some(orderbook);
                }
            }
            MarketEvent::ConnectionStatus { symbol, status } => {
                debug!("Connection status for {}: {:?}", symbol, status);
                if !matches!(status, crate::binance::types::ConnectionStatus::Connected) {
                    self.render_state
                        .queue_message(format!("Connection status for {}: {:?}", symbol, status));
                }
            }
            MarketEvent::Error { symbol, error } => {
                let message = format!("Market error for {}: {}", symbol, error);
                self.render_state.error_message = Some(message.clone());
                self.render_state.queue_message(message);
            }
        }

        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down UI manager");

        self.render_state.should_quit = true;

        Ok(())
    }

    /// Get render statistics
    pub fn get_render_stats(&self) -> &RenderState {
        &self.render_state
    }

    /// Get application state
    pub fn get_app_state(&self) -> &AppState {
        &self.app_state
    }
}
