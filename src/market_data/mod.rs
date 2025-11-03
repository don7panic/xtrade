//! Market data processing and management module

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::binance::BinanceRestClient;
use crate::binance::types::{BinanceMessage, ConnectionStatus, OrderBook};

mod daily_candle;
mod symbol_subscription;
pub use daily_candle::{DEFAULT_DAILY_CANDLE_LIMIT, DailyCandle};
pub use symbol_subscription::SymbolSubscription;

/// Subscription status for a symbol
#[derive(Debug, Clone)]
pub enum SubscriptionStatus {
    Active,
    Reconnecting,
    Failed,
    Disconnected,
}

/// Extended subscription information
#[derive(Debug)]
#[allow(dead_code)]
struct SubscriptionInfo {
    pub orderbook: OrderBook,
    pub status: SubscriptionStatus,
    pub last_successful_update: AtomicU64,
    pub reconnect_count: AtomicU64,
}

/// Market event for communication between subscription tasks and manager
#[derive(Debug, Clone)]
pub enum MarketEvent {
    PriceUpdate {
        symbol: String,
        price: f64,
        time: u64,
    },
    TickerUpdate {
        symbol: String,
        last_price: f64,
        price_change_percent: f64,
        high_price: f64,
        low_price: f64,
        volume: f64,
    },
    OrderBookUpdate {
        symbol: String,
        orderbook: OrderBook,
    },
    ConnectionStatus {
        symbol: String,
        status: ConnectionStatus,
    },
    Error {
        symbol: String,
        error: String,
    },
    DailyCandleUpdate {
        symbol: String,
        candles: Vec<DailyCandle>,
        is_snapshot: bool,
    },
}

/// Control message for managing subscription tasks
#[derive(Debug)]
pub enum ControlMessage {
    Shutdown,
    Reconnect,
    UpdateConfig,
}

/// Handle for managing individual symbol subscriptions
pub struct SubscriptionHandle {
    pub task: JoinHandle<()>,
    pub control_tx: mpsc::UnboundedSender<ControlMessage>,
    pub symbol: String,
}

/// Market data manager for handling multiple symbol subscriptions
pub struct MarketDataManager {
    subscriptions: RwLock<HashMap<String, SubscriptionHandle>>,
    orderbooks: RwLock<HashMap<String, OrderBook>>,
    _rest_client: BinanceRestClient,
    event_tx: mpsc::UnboundedSender<MarketEvent>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<MarketEvent>>>,
}

impl MarketDataManager {
    /// Create a new MarketDataManager
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            subscriptions: RwLock::new(HashMap::new()),
            orderbooks: RwLock::new(HashMap::new()),
            _rest_client: BinanceRestClient::new("https://api.binance.com".to_string()),
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
        }
    }

    /// Subscribe to a symbol with concurrent WebSocket connection
    pub async fn subscribe(&self, symbol: String) -> Result<()> {
        // Acquire write lock briefly to validate and capture state
        let subscriptions = self.subscriptions.write().await;

        if subscriptions.contains_key(&symbol) {
            debug!("Symbol {} is already subscribed", symbol);
            return Ok(());
        }

        info!("Subscribing to symbol: {}", symbol);

        // Performance optimization: Limit concurrent subscriptions
        if subscriptions.len() >= 10 {
            warn!(
                "Maximum concurrent subscriptions reached ({}), rejecting subscription for {}",
                subscriptions.len(),
                symbol
            );
            return Err(anyhow::anyhow!(
                "Maximum concurrent subscriptions (10) reached"
            ));
        }

        // Performance optimization: Validate symbol format
        if !Self::is_valid_symbol_format(&symbol) {
            error!("Invalid symbol format: {}", symbol);
            return Err(anyhow::anyhow!("Invalid symbol format: {}", symbol));
        }

        let should_delay = Self::is_subscribing_too_fast(&subscriptions);
        drop(subscriptions);

        // Performance optimization: Rate limiting - check if we're subscribing too fast
        if should_delay {
            warn!(
                "Subscription rate limit reached, delaying subscription for {}",
                symbol
            );
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        // Create control channel for this subscription
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let event_tx = self.event_tx.clone();

        // Create symbol subscription outside of the lock to avoid blocking other readers
        let mut symbol_subscription =
            SymbolSubscription::new(symbol.clone(), control_rx, event_tx).await?;

        // Initialize the subscription (network calls)
        if let Err(e) = symbol_subscription.initialize().await {
            error!("Failed to initialize subscription for {}: {}", symbol, e);
            return Err(e);
        }

        // Reacquire write lock to register the subscription handle
        let mut subscriptions = self.subscriptions.write().await;

        if subscriptions.contains_key(&symbol) {
            warn!(
                "Subscription for {} was registered while initializing; dropping duplicate",
                symbol
            );
            return Ok(());
        }

        let symbol_clone = symbol.clone();
        let task = tokio::spawn(async move {
            // Yield once to allow scheduler fairness
            tokio::task::yield_now().await;
            symbol_subscription.run().await;
            debug!("Subscription task for {} completed normally", symbol_clone);
        });

        subscriptions.insert(
            symbol.clone(),
            SubscriptionHandle {
                task,
                control_tx,
                symbol: symbol.clone(),
            },
        );

        info!("Successfully subscribed to symbol: {}", symbol);
        Ok(())
    }

    /// Check if we're subscribing too fast (rate limiting)
    fn is_subscribing_too_fast(subscriptions: &HashMap<String, SubscriptionHandle>) -> bool {
        // Simple rate limiting: if we have more than 5 subscriptions, slow down
        subscriptions.len() > 5
    }

    /// Validate symbol format (basic validation)
    fn is_valid_symbol_format(symbol: &str) -> bool {
        // Simple validation: should be uppercase and contain only letters and numbers
        symbol
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && symbol.len() >= 3
    }

    /// Performance optimization: Batch unsubscribe multiple symbols
    pub async fn batch_unsubscribe(&self, symbols: Vec<String>) -> Result<()> {
        info!("Batch unsubscribing from symbols: {:?}", symbols);

        let mut subscriptions = self.subscriptions.write().await;

        for symbol in symbols {
            let symbol_clone = symbol.clone();
            if let Some(handle) = subscriptions.remove(&symbol) {
                info!("Unsubscribing from symbol: {}", symbol);

                // Send shutdown signal to subscription task
                if let Err(e) = handle.control_tx.send(ControlMessage::Shutdown) {
                    warn!("Failed to send shutdown signal for {}: {}", symbol, e);
                }

                let task = handle.task;
                task.abort();
                tokio::spawn(async move {
                    match task.await {
                        Ok(_) => debug!("Subscription task for {} terminated", symbol_clone),
                        Err(e) if e.is_cancelled() => {
                            debug!(
                                "Subscription task for {} cancelled during shutdown",
                                symbol_clone
                            )
                        }
                        Err(e) => {
                            error!("Subscription task for {} failed: {}", symbol_clone, e);
                        }
                    }
                });

                info!("Successfully unsubscribed from symbol: {}", symbol);
            } else {
                debug!("Symbol {} was not subscribed", symbol);
            }
        }

        info!("Batch unsubscribe completed");
        Ok(())
    }

    /// Performance optimization: Get subscription statistics
    pub async fn get_subscription_stats(&self) -> SubscriptionStats {
        let subscriptions = self.subscriptions.read().await;

        SubscriptionStats {
            total_subscriptions: subscriptions.len(),
            symbols: subscriptions.keys().cloned().collect(),
            memory_usage_estimate: subscriptions.len() * 1024 * 1024, // Rough estimate: 1MB per subscription
        }
    }

    /// Process WebSocket message and update orderbook
    #[allow(dead_code)]
    async fn process_websocket_message(
        orderbook: &mut OrderBook,
        symbol: &str,
        binance_msg: BinanceMessage,
        event_tx: &mpsc::UnboundedSender<MarketEvent>,
    ) {
        match binance_msg.stream.as_str() {
            stream if stream.contains("depth") => {
                // Parse depth update
                if let Ok(depth_update) = serde_json::from_value::<
                    crate::binance::types::OrderBookUpdate,
                >(binance_msg.data)
                {
                    if let Err(e) = orderbook.apply_depth_update(depth_update) {
                        error!("Failed to apply depth update for {}: {}", symbol, e);
                    } else {
                        // Send updated orderbook
                        if let Err(e) = event_tx.send(MarketEvent::OrderBookUpdate {
                            symbol: symbol.to_string(),
                            orderbook: orderbook.clone(),
                        }) {
                            error!("Failed to send orderbook update for {}: {}", symbol, e);
                        }
                    }
                }
            }
            stream if stream.contains("trade") => {
                // Parse trade message
                if let Ok(trade_msg) =
                    serde_json::from_value::<crate::binance::types::TradeMessage>(binance_msg.data)
                {
                    // Send price update
                    if let Ok(price) = trade_msg.price.parse::<f64>() {
                        if let Err(e) = event_tx.send(MarketEvent::PriceUpdate {
                            symbol: symbol.to_string(),
                            price,
                            time: trade_msg.event_time,
                        }) {
                            error!("Failed to send price update for {}: {}", symbol, e);
                        }
                    } else {
                        error!("Failed to parse price for {}: {}", symbol, trade_msg.price);
                    }
                }
            }
            _ => {
                debug!(
                    "Unhandled message type for {}: {}",
                    symbol, binance_msg.stream
                );
            }
        }
    }

    /// Unsubscribe from a symbol
    pub async fn unsubscribe(&self, symbol: &str) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(handle) = subscriptions.remove(symbol) {
            info!("Unsubscribing from symbol: {}", symbol);

            // Send shutdown signal to subscription task
            if let Err(e) = handle.control_tx.send(ControlMessage::Shutdown) {
                warn!("Failed to send shutdown signal for {}: {}", symbol, e);
            }

            let task = handle.task;
            task.abort();

            // Wait for task to complete
            match task.await {
                Ok(_) => debug!("Subscription task for {} terminated", symbol),
                Err(e) if e.is_cancelled() => {
                    debug!("Subscription task for {} cancelled during shutdown", symbol)
                }
                Err(e) => {
                    error!("Subscription task for {} failed: {}", symbol, e);
                }
            }

            info!("Successfully unsubscribed from symbol: {}", symbol);
        } else {
            debug!("Symbol {} was not subscribed", symbol);
        }

        Ok(())
    }

    /// Get list of subscribed symbols
    pub async fn list_subscriptions(&self) -> Vec<String> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.keys().cloned().collect()
    }

    /// Get orderbook for a symbol
    pub async fn get_orderbook(&self, symbol: &str) -> Option<OrderBook> {
        let orderbooks = self.orderbooks.read().await;
        orderbooks.get(symbol).cloned()
    }

    /// Recover subscription state after reconnection
    pub async fn recover_subscription(&self, symbol: &str) -> Result<()> {
        info!("Recovering subscription for symbol: {}", symbol);

        // Check if symbol is subscribed
        let subscriptions = self.subscriptions.read().await;
        if !subscriptions.contains_key(symbol) {
            return Err(anyhow::anyhow!("Symbol {} is not subscribed", symbol));
        }

        // Send reconnect signal to subscription
        if let Some(handle) = subscriptions.get(symbol) {
            if let Err(e) = handle.control_tx.send(ControlMessage::Reconnect) {
                error!("Failed to send reconnect signal for {}: {}", symbol, e);
                return Err(e.into());
            }
        }

        info!("Successfully initiated recovery for symbol: {}", symbol);
        Ok(())
    }

    /// Check if subscription needs recovery
    pub async fn needs_recovery(&self, symbol: &str, max_stale_time_ms: u64) -> bool {
        let orderbooks = self.orderbooks.read().await;

        if let Some(orderbook) = orderbooks.get(symbol) {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            if orderbook.last_update_time == 0 {
                return true;
            }

            let time_since_last_update = current_time.saturating_sub(orderbook.last_update_time);

            time_since_last_update > max_stale_time_ms
        } else {
            // No orderbook data available, assume recovery needed
            true
        }
    }

    /// Handle reconnection event
    pub async fn handle_reconnection(&self, max_stale_time_ms: u64) -> Result<()> {
        info!("Handling reconnection event");

        let subscriptions = self.subscriptions.read().await;
        let symbols: Vec<String> = subscriptions.keys().cloned().collect();

        for symbol in symbols {
            if self.needs_recovery(&symbol, max_stale_time_ms).await {
                info!("Symbol {} needs recovery, triggering reconnect", symbol);

                // Send reconnect signal
                if let Some(handle) = subscriptions.get(&symbol) {
                    if let Err(e) = handle.control_tx.send(ControlMessage::Reconnect) {
                        error!("Failed to send reconnect signal for {}: {}", symbol, e);
                    }
                }
            }
        }

        info!("Reconnection event handled");
        Ok(())
    }

    /// Get connection quality metrics
    pub async fn get_connection_quality(&self, symbol: &str) -> Option<ConnectionQuality> {
        let orderbooks = self.orderbooks.read().await;

        if let Some(orderbook) = orderbooks.get(symbol) {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let last_update_time = if orderbook.last_update_time > 0 {
                orderbook.last_update_time
            } else {
                orderbook.snapshot_time
            };
            let time_since_last_update_ms = current_time.saturating_sub(last_update_time);

            let data_freshness = if time_since_last_update_ms < 1000 {
                "fresh".to_string()
            } else if time_since_last_update_ms < 5000 {
                "stale".to_string()
            } else {
                "outdated".to_string()
            };

            let spread = orderbook.spread().unwrap_or(0.0);

            Some(ConnectionQuality {
                symbol: symbol.to_string(),
                data_freshness,
                time_since_last_update_ms,
                orderbook_depth: orderbook.bids.len() + orderbook.asks.len(),
                spread,
            })
        } else {
            None
        }
    }

    /// Clone the market event receiver.
    pub fn event_receiver(&self) -> Arc<Mutex<mpsc::UnboundedReceiver<MarketEvent>>> {
        self.event_rx.clone()
    }

    /// Update internal state for a processed market event.
    pub async fn process_market_event(&self, event: &MarketEvent) {
        if let MarketEvent::OrderBookUpdate { symbol, orderbook } = event {
            let mut orderbooks = self.orderbooks.write().await;
            orderbooks.insert(symbol.clone(), orderbook.clone());
        }
    }
}

impl Default for MarketDataManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Connection quality metrics
#[derive(Debug)]
pub struct ConnectionQuality {
    pub symbol: String,
    pub data_freshness: String,
    pub time_since_last_update_ms: u64,
    pub orderbook_depth: usize,
    pub spread: f64,
}

/// Subscription statistics for performance monitoring
#[derive(Debug)]
pub struct SubscriptionStats {
    pub total_subscriptions: usize,
    pub symbols: Vec<String>,
    pub memory_usage_estimate: usize,
}
