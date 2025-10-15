//! Action Channel for asynchronous event processing

use anyhow::Result;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::market_data::MarketEvent;
use crate::metrics::ConnectionMetrics;
use crate::session::command_router::InteractiveCommand;

/// Session events for communication between components
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Shutdown request
    ShutdownRequested,
    /// Error event
    Error { message: String },
    /// Subscription added
    SubscriptionAdded { symbol: String },
    /// Subscription removed
    SubscriptionRemoved { symbol: String },
    /// Subscription list
    SubscriptionList { symbols: Vec<String> },
    /// UI mode changed
    UIModeChanged { enable_tui: bool },
    /// Status information
    StatusInfo { info: StatusInfo },
    /// Symbol details
    SymbolDetails {
        symbol: String,
        orderbook: Option<crate::binance::types::OrderBook>,
    },
    /// Configuration information
    ConfigInfo { config: Config },
    /// Configuration reset
    ConfigReset,
    /// Configuration help
    ConfigHelp,
    /// Demo started
    DemoStarted,
    /// Demo completed
    DemoCompleted,
    /// Logs information
    LogsInfo { info: LogsInfo },
    /// Metrics snapshot update
    MetricsUpdate { metrics: ConnectionMetrics },
    /// Market data event
    MarketEvent(MarketEvent),
    /// User command from interactive input
    UserCommand { command: InteractiveCommand },
}

/// Status information for session
#[derive(Debug, Clone)]
pub struct StatusInfo {
    pub version: String,
    pub state: String,
    pub active_subscriptions: usize,
    pub symbols: Vec<String>,
    pub session_stats: super::session_manager::SessionStats,
}

/// Logs information for session
#[derive(Debug, Clone)]
pub struct LogsInfo {
    pub recent_logs: Vec<String>,
    pub log_file_path: String,
    pub log_level: String,
}

/// Action channel for event processing
pub struct ActionChannel {
    /// Event sender
    event_tx: mpsc::UnboundedSender<SessionEvent>,
    /// Event receiver
    event_rx: Option<mpsc::UnboundedReceiver<SessionEvent>>,
}

impl Clone for ActionChannel {
    fn clone(&self) -> Self {
        Self {
            event_tx: self.event_tx.clone(),
            event_rx: None, // Receivers cannot be cloned
        }
    }
}

impl ActionChannel {
    /// Create a new ActionChannel
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    /// Send event to channel
    pub fn send_event(&self, event: SessionEvent) -> Result<()> {
        self.event_tx
            .send(event)
            .map_err(|e| anyhow::anyhow!("Failed to send event: {}", e))
    }

    /// Get next event from channel
    pub async fn next_event(&mut self) -> Option<SessionEvent> {
        if let Some(event_rx) = &mut self.event_rx {
            event_rx.recv().await
        } else {
            None
        }
    }

    /// Get event sender for external use
    pub fn event_tx(&self) -> mpsc::UnboundedSender<SessionEvent> {
        self.event_tx.clone()
    }

    /// Get event receiver for external use
    pub fn event_rx(&mut self) -> Option<mpsc::UnboundedReceiver<SessionEvent>> {
        self.event_rx.take()
    }

    /// Broadcast event to multiple receivers
    pub fn broadcast_event(&self, event: SessionEvent) -> Result<()> {
        // For now, just send to the main channel
        // In the future, we can implement fan-out to multiple receivers
        self.send_event(event)
    }

    /// Send error event
    pub fn send_error(&self, message: String) -> Result<()> {
        self.send_event(SessionEvent::Error { message })
    }

    /// Send shutdown request
    pub fn request_shutdown(&self) -> Result<()> {
        self.send_event(SessionEvent::ShutdownRequested)
    }

    /// Send market event
    pub fn send_market_event(&self, event: MarketEvent) -> Result<()> {
        self.send_event(SessionEvent::MarketEvent(event))
    }

    /// Check if channel is closed
    pub fn is_closed(&self) -> bool {
        self.event_tx.is_closed()
    }
}

impl Default for ActionChannel {
    fn default() -> Self {
        Self::new()
    }
}
