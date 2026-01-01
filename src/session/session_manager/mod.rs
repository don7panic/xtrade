//! Session Manager for interactive terminal session lifecycle management

mod alerts;
mod auto_subscribe;
mod commands;
mod events;
mod lifecycle;
mod metrics;
mod paper;
mod ui;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::binance::types::MarketType;
use crate::cli::Cli;
use crate::config::Config;
use crate::market_data::MarketDataManager;

use super::action_channel::{ActionChannel, SessionEvent};
use super::command_router::CommandRouter;

use self::alerts::AlertingState;
use self::metrics::MetricsState;
use self::paper::PaperTradingState;

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
            enable_tui: true,            // Default to TUI mode
            enable_metrics: true,        // Default to metrics collection
            auto_subscribe: true,        // Default to auto-subscribe
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
    /// Command router
    command_router: CommandRouter,
    /// Action channel
    action_channel: ActionChannel,
    /// Price alert coordinator
    alerts: AlertingState,
    /// Metrics coordinator
    metrics: MetricsState,
    /// Shutdown signal sender
    shutdown_tx: mpsc::Sender<()>,
    /// Shutdown signal receiver
    shutdown_rx: Option<mpsc::Receiver<()>>,
    /// Paper trading coordinator
    paper_trading: PaperTradingState,
}

impl SessionManager {
    /// Create a new SessionManager
    pub fn new(cli: &Cli, app_config: Config) -> Result<Self> {
        info!("Creating new SessionManager");

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Create market data manager
        let market_manager = Arc::new(MarketDataManager::new(app_config.binance.clone()));

        // Create command router
        let command_router = CommandRouter::new();

        // Create action channel
        let action_channel = ActionChannel::new();

        Ok(Self {
            config: SessionConfig::default(),
            app_config,
            cli: cli.clone(),
            state: SessionState::Starting,
            stats: SessionStats::default(),
            market_manager,
            ui_task: None,
            ui_event_tx: None,
            command_router,
            action_channel,
            alerts: AlertingState::new(env!("CARGO_PKG_NAME")),
            metrics: MetricsState::new(),
            shutdown_tx,
            shutdown_rx: Some(shutdown_rx),
            paper_trading: PaperTradingState::new(),
        })
    }

    /// Determine market type from CLI args
    fn get_market_type(&self) -> MarketType {
        match &self.cli.command {
            Some(crate::cli::Commands::Ui { market, .. }) => match market.to_lowercase().as_str() {
                "perp" | "perp_usdt" | "perp-usdt" => MarketType::PerpUsdt,
                _ => MarketType::Spot,
            },
            _ => MarketType::Spot,
        }
    }

    /// Forward an event to the UI if the channel is available
    fn forward_to_ui(&self, event: SessionEvent) {
        if let Some(ui_event_tx) = &self.ui_event_tx {
            if let Err(e) = ui_event_tx.send(event) {
                error!("Failed to forward event to UI: {}", e);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::binance::types::MarketKey;
    use crate::market_data::MarketStream;

    fn make_cli(market: &str) -> Cli {
        Cli {
            command: Some(crate::cli::Commands::Ui {
                simple: false,
                market: market.to_string(),
            }),
            config_file: "config.toml".to_string(),
            log_level: "info".to_string(),
            verbose: false,
            dry_run: false,
        }
    }

    #[test]
    fn get_market_type_accepts_perp_aliases() {
        let cli = make_cli("perp");
        let session = SessionManager::new(&cli, Config::default()).unwrap();
        assert_eq!(session.get_market_type(), MarketType::PerpUsdt);

        let cli = make_cli("perp_usdt");
        let session = SessionManager::new(&cli, Config::default()).unwrap();
        assert_eq!(session.get_market_type(), MarketType::PerpUsdt);

        let cli = make_cli("spot");
        let session = SessionManager::new(&cli, Config::default()).unwrap();
        assert_eq!(session.get_market_type(), MarketType::Spot);
    }

    #[test]
    fn build_subscription_plan_includes_spot_and_perp_markets() {
        let cli = make_cli("spot");
        let mut config = Config::default();
        config.symbols = Vec::new();
        config.markets = vec![
            crate::config::MarketConfig {
                exchange: "binance".to_string(),
                market_type: MarketType::Spot,
                symbols: vec!["BTCUSDT".to_string()],
                streams: vec!["trade".to_string()],
            },
            crate::config::MarketConfig {
                exchange: "binance".to_string(),
                market_type: MarketType::PerpUsdt,
                symbols: vec!["BTCUSDT".to_string()],
                streams: vec!["markPrice".to_string(), "openInterest".to_string()],
            },
        ];

        let session = SessionManager::new(&cli, config).unwrap();
        let plans = session.build_subscription_plan();
        let mut plan_map = HashMap::new();
        for plan in plans {
            plan_map.insert(plan.key, plan.streams);
        }

        let spot_key = MarketKey::new(MarketType::Spot, "BTCUSDT".to_string());
        let perp_key = MarketKey::new(MarketType::PerpUsdt, "BTCUSDT".to_string());

        assert!(plan_map.contains_key(&spot_key));
        assert!(plan_map.contains_key(&perp_key));

        let spot_streams = plan_map.get(&spot_key).expect("spot streams");
        assert!(spot_streams.contains(&MarketStream::Trade));
        assert!(!spot_streams.contains(&MarketStream::MarkPrice));

        let perp_streams = plan_map.get(&perp_key).expect("perp streams");
        assert!(perp_streams.contains(&MarketStream::MarkPrice));
        assert!(perp_streams.contains(&MarketStream::OpenInterest));
    }
}
