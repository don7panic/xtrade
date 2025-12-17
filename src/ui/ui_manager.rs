//! UI Manager for interactive terminal interface

use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crossterm::event::{self, Event};

use crate::cli::Cli;
use crate::config::Config;
use crate::market_data::{DEFAULT_DAILY_CANDLE_LIMIT, MarketDataManager, MarketEvent};
use crate::metrics::ConnectionStatus as MetricsConnectionStatus;
use crate::session::action_channel::{SessionEvent, StatusInfo};
use crate::session::session_manager::SessionStats;

use super::tui::{Tui, UiAction, handle_key_event};
use super::{AppState, PricePoint};

/// UI Manager for managing the terminal interface
pub struct UIManager {
    /// Market data manager reference
    market_manager: Arc<MarketDataManager>,
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
    /// Active configuration snapshot
    config: Config,
    /// TUI terminal handle
    tui: Option<Tui>,
    /// Desired refresh cadence
    refresh_interval: Duration,
    /// Time of the last successful render
    last_render: Instant,
    /// Latest session statistics from the backend
    session_stats: SessionStats,
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
}

impl UIManager {
    /// Create a new UIManager
    pub fn new(
        market_manager: Arc<MarketDataManager>,
        session_event_tx: mpsc::UnboundedSender<SessionEvent>,
        config: Config,
    ) -> Self {
        // Create event channels
        let (ui_event_tx, ui_event_rx) = mpsc::unbounded_channel();
        let (_market_event_tx, market_event_rx) = mpsc::unbounded_channel();

        let refresh_interval = Duration::from_millis(config.refresh_rate_ms.clamp(16, 1000));

        Self {
            market_manager,
            session_event_tx,
            ui_event_tx,
            event_rx: Some(ui_event_rx),
            market_event_rx: Some(market_event_rx),
            app_state: AppState::new(config.symbols.clone()),
            render_state: RenderState::default(),
            dry_run: false,
            config,
            tui: None,
            refresh_interval,
            last_render: Instant::now(),
            session_stats: SessionStats::default(),
        }
    }

    /// Create a new UIManager with dry-run mode
    pub fn new_with_dry_run(
        market_manager: Arc<MarketDataManager>,
        session_event_tx: mpsc::UnboundedSender<SessionEvent>,
        config: Config,
    ) -> Self {
        let mut ui_manager = Self::new(market_manager, session_event_tx, config);
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
        let symbols = self.market_manager.list_subscriptions().await;

        self.app_state.symbols = symbols;
        self.app_state.normalize_selected_tab();

        info!(
            "UI initialized with {} symbols",
            self.app_state.symbols.len()
        );

        let message = "Interactive mode ready. Press '/' for commands.";
        self.render_state.queue_message(message);
        self.app_state.push_log(message);
        self.render_state.should_redraw = true;

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

        self.tui =
            Some(Tui::new().map_err(|e| anyhow::anyhow!("Failed to initialise terminal: {}", e))?);
        self.render_state.should_redraw = true;
        self.last_render = Instant::now()
            .checked_sub(self.refresh_interval)
            .unwrap_or_else(Instant::now);

        while !self.render_state.should_quit && !self.app_state.should_quit {
            // Process async events from the session layer
            self.process_events().await?;

            // Handle terminal input (non-blocking)
            self.poll_terminal_events()?;

            // Render on dirty state or cadence tick
            let now = Instant::now();
            if self.render_state.should_redraw
                || now.duration_since(self.last_render) >= self.refresh_interval
            {
                if let Some(tui) = self.tui.as_mut() {
                    self.render_state.render_count += 1;
                    self.render_state.last_render_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                        as u64;

                    tui.draw(
                        &mut self.app_state,
                        &self.render_state,
                        &self.session_stats,
                        self.config.orderbook_depth,
                    )
                    .map_err(|e| anyhow::anyhow!("Failed to render frame: {}", e))?;
                }
                self.render_state.should_redraw = false;
                self.last_render = now;
            }

            // Prevent busy loop
            tokio::time::sleep(Duration::from_millis(16)).await;
        }

        if let Some(tui) = self.tui.as_mut() {
            tui.restore()
                .map_err(|e| anyhow::anyhow!("Failed to restore terminal state: {}", e))?;
        }

        Ok(())
    }

    /// Poll for keyboard/terminal events and translate into session actions
    fn poll_terminal_events(&mut self) -> Result<()> {
        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(key_event) => {
                    let action = handle_key_event(&mut self.app_state, key_event);
                    self.render_state.should_redraw = true;

                    match action {
                        UiAction::None => {}
                        UiAction::QuitRequested => {
                            self.render_state.should_quit = true;
                            let _ = self.session_event_tx.send(SessionEvent::ShutdownRequested);
                        }
                        UiAction::SubmitCommand(cmd) => {
                            if let Err(e) = self.process_user_command(&cmd) {
                                let message = format!("Command error: {}", e);
                                self.render_state.error_message = Some(message.clone());
                                self.app_state.push_log(message);
                            }
                        }
                    }
                }
                Event::Resize(_, _) => {
                    self.render_state.should_redraw = true;
                }
                Event::Mouse(mouse_event) => match mouse_event.kind {
                    crossterm::event::MouseEventKind::ScrollUp => {
                        self.app_state.scroll_logs_up();
                        self.render_state.should_redraw = true;
                    }
                    crossterm::event::MouseEventKind::ScrollDown => {
                        self.app_state.scroll_logs_down();
                        self.render_state.should_redraw = true;
                    }
                    _ => {}
                },
                Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
            }
        }

        if self.app_state.should_quit {
            self.render_state.should_quit = true;
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

        let processed_session = !events_to_process.is_empty();
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

        let processed_market = !market_events_to_process.is_empty();
        for event in market_events_to_process {
            self.handle_market_event(event).await?;
        }

        if processed_session || processed_market {
            self.render_state.should_redraw = true;
        }

        Ok(())
    }

    /// Process user command from input
    fn process_user_command(&mut self, input: &str) -> Result<()> {
        debug!("Processing user command: {}", input);

        // Create a temporary command router to parse the command
        let command_router = crate::session::command_router::CommandRouter::new();
        let default_symbol = self.app_state.current_symbol().map(|s| s.as_str());
        let command_result =
            command_router.parse_interactive_command_with_default(input, default_symbol);

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
                        self.app_state.push_log("Shutdown requested via command");
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
        let symbols = self.market_manager.list_subscriptions().await;

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
        self.render_state.should_redraw = true;

        match event {
            SessionEvent::ShutdownRequested => {
                self.render_state
                    .queue_message("Shutdown requested. Exiting interactive session...");
                self.render_state.should_quit = true;
                self.app_state
                    .push_log("Shutdown requested by session manager");
                info!("UI received shutdown request");
            }
            SessionEvent::Error { message } => {
                let formatted = format!("Error: {}", message);
                self.render_state.error_message = Some(formatted.clone());
                self.render_state.queue_message(formatted);
                self.app_state.push_log(format!("Error: {}", message));
            }
            SessionEvent::SubscriptionAdded { symbol } => {
                if !self.app_state.symbols.contains(&symbol) {
                    self.app_state.symbols.push(symbol.clone());
                    self.app_state.normalize_selected_tab();
                    self.render_state
                        .queue_message(format!("Subscribed to {}", symbol));
                    self.app_state.push_log(format!("Subscribed to {}", symbol));
                }
            }
            SessionEvent::SubscriptionRemoved { symbol } => {
                self.app_state.symbols.retain(|s| s != &symbol);
                self.app_state.normalize_selected_tab();
                self.render_state
                    .queue_message(format!("Unsubscribed from {}", symbol));
                self.app_state
                    .push_log(format!("Unsubscribed from {}", symbol));
            }
            SessionEvent::SubscriptionList { symbols } => {
                self.app_state.symbols = symbols.clone();
                self.app_state.normalize_selected_tab();
                self.render_state.queue_message(format!(
                    "Active subscriptions: {}",
                    if symbols.is_empty() {
                        "none".to_string()
                    } else {
                        symbols.join(", ")
                    }
                ));
                self.app_state
                    .push_log(format!("Active subscriptions: {}", symbols.join(", ")));
            }
            SessionEvent::StatusInfo { info } => {
                let message = format!(
                    "Status → Version {} | State {} | Subscriptions {}",
                    info.version, info.state, info.active_subscriptions
                );
                self.render_state.info_message = Some(message.clone());
                self.render_state.queue_message(message);
                self.app_state.symbols = info.symbols.clone();
                self.app_state.normalize_selected_tab();
                self.session_stats = info.session_stats.clone();
                self.app_state.push_log(format!(
                    "Session status updated: {} symbols active",
                    info.active_subscriptions
                ));
            }
            SessionEvent::UIModeChanged { enable_tui } => {
                info!("UI mode changed: TUI {}", enable_tui);
                self.render_state
                    .queue_message(format!("UI mode changed: TUI {}", enable_tui));
                self.app_state
                    .push_log(format!("UI mode changed: TUI {}", enable_tui));
            }
            SessionEvent::ConfigInfo { config } => {
                self.config = config.clone();
                self.refresh_interval = Duration::from_millis(self.config.refresh_rate_ms.max(16));
                let message = format!(
                    "Config updated → refresh {}ms, depth {}",
                    self.config.refresh_rate_ms, self.config.orderbook_depth
                );
                self.render_state.info_message = Some(message.clone());
                self.render_state.queue_message(message);
                self.app_state.push_log("Configuration updated".to_string());
            }
            SessionEvent::ConfigReset => {
                self.config = Config::default();
                self.refresh_interval = Duration::from_millis(self.config.refresh_rate_ms.max(16));
                let message = "Configuration reset to defaults".to_string();
                self.render_state.info_message = Some(message.clone());
                self.render_state.queue_message(message.clone());
                self.app_state.push_log(message);
            }
            SessionEvent::ConfigHelp => {
                self.render_state.queue_message(
                    "Config commands: /config show | /config set <key> <value> | /config reset",
                );
                self.app_state.push_log("Displayed config help".to_string());
            }
            SessionEvent::HelpInfo { lines } => {
                if !lines.is_empty() {
                    self.render_state
                        .queue_message("Help commands listed in log panel (use Up/Down to scroll)");
                }
                for line in lines {
                    self.app_state.push_log(format!("[help] {}", line));
                }
            }
            SessionEvent::MetricsUpdate { metrics } => {
                self.app_state.connection_metrics = metrics;
            }
            SessionEvent::LogsInfo { mut info } => {
                let joined = info.recent_logs.join(", ");
                let message = format!("Recent logs ({}): {}", info.log_level, joined);
                self.render_state.info_message = Some(message.clone());
                self.render_state.queue_message(message);
                for log in info.recent_logs.drain(..) {
                    self.app_state.push_log(log);
                }
            }
            SessionEvent::AlertNotification { message } => {
                self.render_state.queue_message(message.clone());
                self.app_state.push_log(format!("[alert] {}", message));
                self.app_state.push_notification(message);
            }
            SessionEvent::AlertSnapshot { alerts } => {
                self.app_state.update_alerts(alerts);
            }
            SessionEvent::AlertList { entries } => {
                if entries.is_empty() {
                    self.render_state
                        .queue_message("No alerts configured".to_string());
                    self.app_state
                        .push_log("[alert] No alerts configured".to_string());
                } else {
                    self.render_state
                        .queue_message("Alerts listed in log panel".to_string());
                    for entry in entries {
                        self.app_state.push_log(format!("[alert] {}", entry));
                    }
                }
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
        let mut should_redraw = false;

        match event {
            MarketEvent::PriceUpdate {
                symbol,
                price,
                time,
            } => {
                // Update market data state
                if let Some(market_data) = self.app_state.market_data.get_mut(&symbol) {
                    market_data.price = price;
                    market_data.price_history.push(PricePoint {
                        timestamp_ms: time,
                        price,
                    });

                    // Keep history size manageable
                    let max_points = self.config.ui.sparkline_points.max(2);
                    if market_data.price_history.len() > max_points {
                        let overflow = market_data.price_history.len() - max_points;
                        market_data.price_history.drain(0..overflow);
                    }
                    should_redraw = true;
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
                            price_history: vec![PricePoint {
                                timestamp_ms: time,
                                price,
                            }],
                            daily_candles: Vec::new(),
                            kline_render_cache: None,
                            last_kline_refresh: None,
                        },
                    );
                    self.render_state
                        .queue_message(format!("Market data stream started for {}", symbol));
                    self.app_state
                        .push_log(format!("Market data stream started for {}", symbol));
                    should_redraw = true;
                }
            }
            MarketEvent::TickerUpdate {
                symbol,
                last_price,
                price_change_percent,
                high_price,
                low_price,
                volume,
            } => {
                use std::collections::hash_map::Entry;

                let entry = self.app_state.market_data.entry(symbol.clone());
                let market_data = match entry {
                    Entry::Occupied(occupied) => occupied.into_mut(),
                    Entry::Vacant(vacant) => vacant.insert(super::MarketDataState {
                        symbol: symbol.clone(),
                        ..super::MarketDataState::default()
                    }),
                };

                market_data.price = last_price;
                market_data.change_percent = price_change_percent;
                market_data.volume_24h = volume;
                market_data.high_24h = high_price;
                market_data.low_24h = low_price;
                should_redraw = true;
            }
            MarketEvent::OrderBookUpdate { symbol, orderbook } => {
                // Update orderbook
                if let Some(market_data) = self.app_state.market_data.get_mut(&symbol) {
                    market_data.orderbook = Some(orderbook);
                    should_redraw = true;
                }
            }
            MarketEvent::ConnectionStatus { symbol, status } => {
                debug!("Connection status for {}: {:?}", symbol, status);
                if !matches!(status, crate::binance::types::ConnectionStatus::Connected) {
                    self.render_state
                        .queue_message(format!("Connection status for {}: {:?}", symbol, status));
                    self.app_state
                        .push_log(format!("Connection status for {}: {:?}", symbol, status));
                }

                self.app_state.connection_metrics.status = match status {
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
                        MetricsConnectionStatus::Error(err)
                    }
                };
                should_redraw = true;
            }
            MarketEvent::DailyCandleUpdate {
                symbol,
                mut candles,
                is_snapshot,
            } => {
                use std::collections::hash_map::Entry;

                let entry = self.app_state.market_data.entry(symbol.clone());
                let market_data = match entry {
                    Entry::Occupied(occupied) => occupied.into_mut(),
                    Entry::Vacant(vacant) => vacant.insert(super::MarketDataState {
                        symbol: symbol.clone(),
                        ..Default::default()
                    }),
                };

                let mut appended_closed = false;

                if is_snapshot {
                    market_data.daily_candles = candles;
                    if market_data.daily_candles.len() > DEFAULT_DAILY_CANDLE_LIMIT {
                        let overflow = market_data.daily_candles.len() - DEFAULT_DAILY_CANDLE_LIMIT;
                        market_data.daily_candles.drain(0..overflow);
                    }

                    market_data
                        .daily_candles
                        .sort_by_key(|candle| candle.open_time_ms);

                    self.render_state.queue_message(format!(
                        "Loaded {} daily candles for {}",
                        market_data.daily_candles.len(),
                        symbol
                    ));
                    appended_closed = true; // force redraw on fresh snapshot
                } else {
                    for candle in candles.drain(..) {
                        if let Some(existing) = market_data
                            .daily_candles
                            .iter_mut()
                            .find(|existing| existing.open_time_ms == candle.open_time_ms)
                        {
                            if candle.is_closed && !existing.is_closed {
                                appended_closed = true;
                            }
                            *existing = candle;
                        } else {
                            if candle.is_closed {
                                appended_closed = true;
                            }
                            market_data.daily_candles.push(candle);
                        }
                    }

                    if market_data.daily_candles.len() > DEFAULT_DAILY_CANDLE_LIMIT {
                        let overflow = market_data.daily_candles.len() - DEFAULT_DAILY_CANDLE_LIMIT;
                        market_data.daily_candles.drain(0..overflow);
                    }

                    market_data
                        .daily_candles
                        .sort_by_key(|candle| candle.open_time_ms);
                }

                market_data.invalidate_kline_cache();

                let now = Instant::now();
                let refresh_interval =
                    Duration::from_secs(self.config.ui.kline_refresh_secs.max(1));

                if market_data.update_kline_refresh(
                    now,
                    refresh_interval,
                    is_snapshot || appended_closed,
                ) {
                    should_redraw = true;
                }
            }
            MarketEvent::Error { symbol, error } => {
                let message = format!("Market error for {}: {}", symbol, error);
                self.render_state.error_message = Some(message.clone());
                self.render_state.queue_message(message);
                self.app_state
                    .push_log(format!("Market error for {}: {}", symbol, error));
                should_redraw = true;
            }
        }

        if should_redraw {
            self.render_state.should_redraw = true;
        }
        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down UI manager");

        self.render_state.should_quit = true;
        self.app_state.should_quit = true;

        if let Some(tui) = self.tui.as_mut() {
            if let Err(e) = tui.restore() {
                warn!("Failed to restore terminal during shutdown: {}", e);
            }
        }

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
