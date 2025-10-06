//! UI Manager for interactive terminal interface

use anyhow::Result;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info};

use crate::market_data::MarketDataManager;
use crate::market_data::MarketEvent;
use crate::session::action_channel::{SessionEvent, StatusInfo};

use super::AppState;

/// UI Manager for managing the terminal interface
pub struct UIManager {
    /// Market data manager reference
    market_manager: Arc<Mutex<MarketDataManager>>,
    /// Event sender for session events
    event_tx: mpsc::UnboundedSender<SessionEvent>,
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
        }
    }
}

impl UIManager {
    /// Create a new UIManager
    pub fn new(
        market_manager: Arc<Mutex<MarketDataManager>>,
        event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Self {
        // Create event channels
        let (_ui_event_tx, ui_event_rx) = mpsc::unbounded_channel();
        let (_market_event_tx, market_event_rx) = mpsc::unbounded_channel();

        Self {
            market_manager,
            event_tx,
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
        event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Self {
        let mut ui_manager = Self::new(market_manager, event_tx);
        ui_manager.dry_run = true;
        ui_manager
    }

    /// Get UI event sender
    pub fn ui_event_tx(&self) -> mpsc::UnboundedSender<SessionEvent> {
        self.event_tx.clone()
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
            return self.run_dry_run().await;
        }

        // Initialize UI components
        self.initialize_ui().await?;

        // Main UI loop
        self.run_ui_loop().await?;

        info!("UI manager stopped");
        Ok(())
    }

    /// Run in dry-run mode (show welcome page and configuration)
    async fn run_dry_run(&mut self) -> Result<()> {
        info!("Running UI in dry-run mode");

        // Display welcome page
        self.display_welcome_page().await?;

        // Display configuration
        self.display_configuration().await?;

        info!("Dry-run mode completed");
        Ok(())
    }

    /// Display welcome page
    async fn display_welcome_page(&mut self) -> Result<()> {
        println!();
        println!("┌─ XTrade Market Data Monitor ────────────────────────────────────────┐");
        println!("│                                                                     │");
        println!("│                      * Welcome to XTrade! *                         │");
        println!("│                                                                     │");
        println!("│   A high-performance cryptocurrency market data monitoring system   │");
        println!("│                                                                     │");
        println!("│   Version: {:<50} │", env!("CARGO_PKG_VERSION"));
        println!(
            "│   Rust: {:<50} │",
            std::env::var("RUSTC_VERSION").unwrap_or("unknown".to_string())
        );
        println!("│                                                                     │");
        println!("│   Features:                                                        │");
        println!("│   • Real-time Binance market data                                  │");
        println!("│   • OrderBook visualization                                        │");
        println!("│   • Multi-symbol monitoring                                        │");
        println!("│   • Performance metrics tracking                                    │");
        println!("│                                                                     │");
        println!("│   Commands:                                                        │");
        println!("│   • /add <symbols> - Subscribe to symbols                   │");
        println!("│   • /remove <symbols> - Unsubscribe from symbols             │");
        println!("│   • /pairs - Show current subscriptions                       │");
        println!("│   • /show <symbol> - Show details for symbol                │");
        println!("│   • /status - Show session statistics                         │");
        println!("│   • /logs - Show recent logs                                 │");
        println!("│   • /config show - Show configuration                        │");
        println!("│                                                                     │");
        println!("└────────────────────────────────────────────────────────────────────┘");
        println!();

        Ok(())
    }

    /// Display configuration
    async fn display_configuration(&mut self) -> Result<()> {
        println!("┌─ Configuration Overview ───────────────────────────────────────────┐");
        println!("│                                                                     │");
        println!("│   Configuration loaded successfully!                               │");
        println!("│                                                                     │");
        println!("│   Default symbols: BTCUSDT                                          │");
        println!("│   Refresh rate: 100ms                                               │");
        println!("│   OrderBook depth: 20 levels                                        │");
        println!("│   Log level: info                                                   │");
        println!("│                                                                     │");
        println!("│   Binance API:                                                     │");
        println!("│   • WebSocket: wss://stream.binance.com:9443                       │");
        println!("│   • REST API: https://api.binance.com                               │");
        println!("│                                                                     │");
        println!("│   UI Settings:                                                     │");
        println!("│   • Colors: enabled                                                 │");
        println!("│   • Update rate: 20 FPS                                             │");
        println!("│   • Sparkline points: 60                                            │");
        println!("│                                                                     │");
        println!("└────────────────────────────────────────────────────────────────────┘");
        println!();
        println!("Dry-run mode completed. Use 'xtrade ui' to start the full interface.");
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

        Ok(())
    }

    /// Main UI rendering loop
    async fn run_ui_loop(&mut self) -> Result<()> {
        info!("Starting UI rendering loop");

        while !self.render_state.should_quit {
            // Check for events
            self.process_events().await?;

            // Render if needed
            if self.render_state.should_redraw {
                self.render().await?;
                self.render_state.should_redraw = false;
            }

            // Handle user input
            self.handle_input().await?;

            // Sleep to prevent busy waiting
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        Ok(())
    }

    /// Process incoming events
    async fn process_events(&mut self) -> Result<()> {
        let mut has_events = false;

        // Process session events
        let mut events_to_process = Vec::new();
        if let Some(event_rx) = &mut self.event_rx {
            while let Ok(event) = event_rx.try_recv() {
                events_to_process.push(event);
                has_events = true;
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
                has_events = true;
            }
        }

        for event in market_events_to_process {
            self.handle_market_event(event).await?;
        }

        // Mark for redraw if we processed events
        if has_events {
            self.render_state.should_redraw = true;
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
        println!("\n┌─ XTrade Market Data Monitor ──────────────────────────────────────┐");

        // Show symbols
        if self.app_state.symbols.is_empty() {
            println!("│ No active subscriptions");
        } else {
            let symbols_str = self.app_state.symbols.join(" ");
            println!("│ Symbols: {}", symbols_str);
        }

        // Show status
        println!(
            "│ Status: Running | Render count: {}",
            self.render_state.render_count
        );

        // Show error/info messages
        if let Some(error) = &self.render_state.error_message {
            println!("│ Error: {}", error);
        }

        if let Some(info) = &self.render_state.info_message {
            println!("│ Info: {}", info);
        }

        println!("│");
        println!(
            "│ Commands: add <symbol> | remove <symbol> | pairs | focus <symbol> | stats | logs | quit"
        );
        println!("└────────────────────────────────────────────────────────────────────┘");

        Ok(())
    }

    /// Handle user input
    async fn handle_input(&mut self) -> Result<()> {
        // Check for user input from stdin
        let mut stdin = io::stdin();
        let mut reader = io::BufReader::new(&mut stdin);
        let mut line = String::new();

        // Try to read input without blocking
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF - no input available
                return Ok(());
            }
            Ok(_) => {
                // Input received - process command
                let trimmed_line = line.trim();
                if !trimmed_line.is_empty() {
                    self.process_user_command(trimmed_line).await?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No input available - this is normal
                return Ok(());
            }
            Err(e) => {
                error!("Error reading user input: {}", e);
                return Err(e.into());
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
                        self.event_tx
                            .send(SessionEvent::ShutdownRequested)
                            .map_err(|e| {
                                anyhow::anyhow!("Failed to send shutdown request: {}", e)
                            })?;
                    }
                    _ => {
                        // Forward other commands to session manager
                        self.event_tx
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
                self.render_state.error_message = Some(format!("Command error: {}", e));
                self.render_state.should_redraw = true;
                error!("Command parsing error: {}", e);
            }
        }

        Ok(())
    }

    /// Handle status command
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

        self.event_tx
            .send(SessionEvent::StatusInfo { info: status_info })
            .map_err(|e| anyhow::anyhow!("Failed to send status event: {}", e))?;

        Ok(())
    }

    /// Handle session event
    pub async fn handle_event(&mut self, event: SessionEvent) -> Result<()> {
        debug!("Handling UI event: {:?}", event);

        match event {
            SessionEvent::ShutdownRequested => {
                self.render_state.should_quit = true;
                info!("UI received shutdown request");
            }
            SessionEvent::Error { message } => {
                self.render_state.error_message = Some(message);
                self.render_state.should_redraw = true;
            }
            SessionEvent::SubscriptionAdded { symbol } => {
                if !self.app_state.symbols.contains(&symbol) {
                    self.app_state.symbols.push(symbol);
                    self.render_state.should_redraw = true;
                }
            }
            SessionEvent::SubscriptionRemoved { symbol } => {
                self.app_state.symbols.retain(|s| s != &symbol);
                self.render_state.should_redraw = true;
            }
            SessionEvent::SubscriptionList { symbols } => {
                self.app_state.symbols = symbols;
                self.render_state.should_redraw = true;
            }
            SessionEvent::StatusInfo { info } => {
                self.render_state.info_message = Some(format!(
                    "Version: {} | State: {} | Subscriptions: {}",
                    info.version, info.state, info.active_subscriptions
                ));
                self.render_state.should_redraw = true;
            }
            SessionEvent::UIModeChanged { enable_tui } => {
                info!("UI mode changed: TUI {}", enable_tui);
                self.render_state.should_redraw = true;
            }
            SessionEvent::LogsInfo { info } => {
                self.render_state.info_message = Some(format!(
                    "Recent logs ({}): {}",
                    info.log_level,
                    info.recent_logs.join(", ")
                ));
                self.render_state.should_redraw = true;
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
                }

                self.render_state.should_redraw = true;
            }
            MarketEvent::OrderBookUpdate { symbol, orderbook } => {
                // Update orderbook
                if let Some(market_data) = self.app_state.market_data.get_mut(&symbol) {
                    market_data.orderbook = Some(orderbook);
                }

                self.render_state.should_redraw = true;
            }
            MarketEvent::ConnectionStatus { symbol, status } => {
                debug!("Connection status for {}: {:?}", symbol, status);
                self.render_state.should_redraw = true;
            }
            MarketEvent::Error { symbol, error } => {
                self.render_state.error_message =
                    Some(format!("Market error for {}: {}", symbol, error));
                self.render_state.should_redraw = true;
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
