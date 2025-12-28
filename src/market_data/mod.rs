//! Market data processing and management module

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::binance::types::{BinanceMessage, ConnectionStatus, MarketKey, MarketType, OrderBook};
use crate::config::BinanceConfig;

mod daily_candle;
mod symbol_subscription;
pub use daily_candle::{DEFAULT_DAILY_CANDLE_LIMIT, DailyCandle};
pub use symbol_subscription::SymbolSubscription;

/// Supported market data streams for subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarketStream {
    Depth,
    Trade,
    Ticker,
    Kline1d,
    MarkPrice,
    FundingRate,
    OpenInterest,
    ForceOrder,
}

impl MarketStream {
    pub fn default_streams_for_market(market_type: MarketType) -> HashSet<Self> {
        let mut streams = HashSet::new();
        streams.insert(MarketStream::Depth);
        streams.insert(MarketStream::Trade);
        streams.insert(MarketStream::Ticker);
        streams.insert(MarketStream::Kline1d);

        if market_type == MarketType::PerpUsdt {
            streams.insert(MarketStream::MarkPrice);
            streams.insert(MarketStream::FundingRate);
            streams.insert(MarketStream::OpenInterest);
            streams.insert(MarketStream::ForceOrder);
        }

        streams
    }
}

impl std::str::FromStr for MarketStream {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "depth" | "depth@100ms" | "depth@1000ms" => Ok(MarketStream::Depth),
            "trade" | "aggtrade" => Ok(MarketStream::Trade),
            "ticker" | "24hrticker" => Ok(MarketStream::Ticker),
            "kline" | "kline_1d" | "kline1d" | "1d" => Ok(MarketStream::Kline1d),
            "markprice" => Ok(MarketStream::MarkPrice),
            "fundingrate" => Ok(MarketStream::FundingRate),
            "openinterest" => Ok(MarketStream::OpenInterest),
            "forceorder" | "liquidation" => Ok(MarketStream::ForceOrder),
            _ => Err(()),
        }
    }
}

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
        key: MarketKey,
        price: f64,
        time: u64,
    },
    TickerUpdate {
        key: MarketKey,
        last_price: f64,
        price_change_percent: f64,
        high_price: f64,
        low_price: f64,
        volume: f64,
    },
    OrderBookUpdate {
        key: MarketKey,
        orderbook: OrderBook,
    },
    ConnectionStatus {
        key: MarketKey,
        status: ConnectionStatus,
    },
    Error {
        key: MarketKey,
        error: String,
    },
    DailyCandleUpdate {
        key: MarketKey,
        candles: Vec<DailyCandle>,
        is_snapshot: bool,
    },
    MarkPriceUpdate {
        key: MarketKey,
        mark_price: f64,
        index_price: f64,
        funding_rate: f64,
        next_funding_time: u64,
        event_time: u64,
    },
    FundingRateUpdate {
        key: MarketKey,
        funding_rate: f64,
        funding_time: u64,
        event_time: u64,
    },
    OpenInterestUpdate {
        key: MarketKey,
        open_interest: f64,
        event_time: u64,
    },
    Liquidation {
        key: MarketKey,
        side: String,
        price: f64,
        quantity: f64,
        event_time: u64,
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
    pub key: MarketKey,
}

/// Market data manager for handling multiple symbol subscriptions
pub struct MarketDataManager {
    subscriptions: RwLock<HashMap<MarketKey, SubscriptionHandle>>,
    orderbooks: RwLock<HashMap<MarketKey, OrderBook>>,
    binance_config: BinanceConfig,
    event_tx: mpsc::UnboundedSender<MarketEvent>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<MarketEvent>>>,
}

impl MarketDataManager {
    /// Create a new MarketDataManager
    pub fn new(config: BinanceConfig) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            subscriptions: RwLock::new(HashMap::new()),
            orderbooks: RwLock::new(HashMap::new()),
            binance_config: config,
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
        }
    }

    /// Subscribe to a symbol with concurrent WebSocket connection
    pub async fn subscribe(&self, key: MarketKey) -> Result<()> {
        let streams = MarketStream::default_streams_for_market(key.market_type);
        self.subscribe_with_streams(key, streams).await
    }

    /// Subscribe to a symbol with explicit stream selection
    pub async fn subscribe_with_streams(
        &self,
        key: MarketKey,
        streams: HashSet<MarketStream>,
    ) -> Result<()> {
        // Acquire write lock briefly to validate and capture state
        let subscriptions = self.subscriptions.write().await;

        if subscriptions.contains_key(&key) {
            debug!("Symbol {:?} is already subscribed", key);
            return Ok(());
        }

        info!("Subscribing to symbol: {:?}", key);

        // Performance optimization: Limit concurrent subscriptions
        if subscriptions.len() >= 20 {
            // Increased limit for multi-market
            warn!(
                "Maximum concurrent subscriptions reached ({}), rejecting subscription for {:?}",
                subscriptions.len(),
                key
            );
            return Err(anyhow::anyhow!(
                "Maximum concurrent subscriptions (20) reached"
            ));
        }

        // Performance optimization: Validate symbol format
        if !Self::is_valid_symbol_format(&key.symbol) {
            error!("Invalid symbol format: {}", key.symbol);
            return Err(anyhow::anyhow!("Invalid symbol format: {}", key.symbol));
        }

        let should_delay = Self::is_subscribing_too_fast(&subscriptions);
        drop(subscriptions);

        // Performance optimization: Rate limiting - check if we're subscribing too fast
        if should_delay {
            warn!(
                "Subscription rate limit reached, delaying subscription for {:?}",
                key
            );
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        // Determine URLs based on MarketType
        let (ws_url_base, rest_url) = match key.market_type {
            crate::binance::types::MarketType::Spot => (
                self.binance_config.ws_url.clone(),
                self.binance_config.rest_url.clone(),
            ),
            crate::binance::types::MarketType::PerpUsdt => {
                if let Some(ref perp_config) = self.binance_config.perp_usdt {
                    (perp_config.ws_url.clone(), perp_config.rest_url.clone())
                } else {
                    (
                        "wss://fstream.binance.com".to_string(),
                        "https://fapi.binance.com".to_string(),
                    )
                }
            }
        };

        // Ensure WebSocket URL has the correct path (/ws) for subscription mode
        let ws_url = if ws_url_base.ends_with("/ws") || ws_url_base.ends_with("/stream") {
            ws_url_base
        } else {
            format!("{}/ws", ws_url_base.trim_end_matches('/'))
        };

        // Create control channel for this subscription
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let event_tx = self.event_tx.clone();

        // Create symbol subscription outside of the lock to avoid blocking other readers
        let mut symbol_subscription = SymbolSubscription::new(
            key.market_type,
            key.symbol.clone(),
            ws_url,
            rest_url,
            streams,
            control_rx,
            event_tx,
        )
        .await?;

        // Initialize the subscription (network calls)
        if let Err(e) = symbol_subscription.initialize().await {
            error!("Failed to initialize subscription for {:?}: {}", key, e);
            return Err(e);
        }

        // Reacquire write lock to register the subscription handle
        let mut subscriptions = self.subscriptions.write().await;

        if subscriptions.contains_key(&key) {
            warn!(
                "Subscription for {:?} was registered while initializing; dropping duplicate",
                key
            );
            return Ok(());
        }

        let key_clone = key.clone();
        let task = tokio::spawn(async move {
            // Yield once to allow scheduler fairness
            tokio::task::yield_now().await;
            symbol_subscription.run().await;
            debug!("Subscription task for {:?} completed normally", key_clone);
        });

        subscriptions.insert(
            key.clone(),
            SubscriptionHandle {
                task,
                control_tx,
                key: key.clone(),
            },
        );

        info!("Successfully subscribed to symbol: {:?}", key);
        Ok(())
    }

    /// Check if we're subscribing too fast (rate limiting)
    fn is_subscribing_too_fast(subscriptions: &HashMap<MarketKey, SubscriptionHandle>) -> bool {
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
    pub async fn batch_unsubscribe(&self, keys: Vec<MarketKey>) -> Result<()> {
        info!("Batch unsubscribing from keys: {:?}", keys);

        let mut subscriptions = self.subscriptions.write().await;

        for key in keys {
            let key_clone = key.clone();
            if let Some(handle) = subscriptions.remove(&key) {
                info!("Unsubscribing from key: {:?}", key);

                // Send shutdown signal to subscription task
                if let Err(e) = handle.control_tx.send(ControlMessage::Shutdown) {
                    warn!("Failed to send shutdown signal for {:?}: {}", key, e);
                }

                let task = handle.task;
                task.abort();
                tokio::spawn(async move {
                    match task.await {
                        Ok(_) => debug!("Subscription task for {:?} terminated", key_clone),
                        Err(e) if e.is_cancelled() => {
                            debug!(
                                "Subscription task for {:?} cancelled during shutdown",
                                key_clone
                            )
                        }
                        Err(e) => {
                            error!("Subscription task for {:?} failed: {}", key_clone, e);
                        }
                    }
                });

                info!("Successfully unsubscribed from key: {:?}", key);
            } else {
                debug!("Key {:?} was not subscribed", key);
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
            symbols: subscriptions.keys().map(|k| format!("{:?}", k)).collect(),
            memory_usage_estimate: subscriptions.len() * 1024 * 1024, // Rough estimate: 1MB per subscription
        }
    }

    /// Process WebSocket message and update orderbook
    #[allow(dead_code)]
    async fn process_websocket_message(
        orderbook: &mut OrderBook,
        symbol: &str,
        binance_msg: BinanceMessage,
        _event_tx: &mpsc::UnboundedSender<MarketEvent>,
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
                        // NOTE: This function is marked dead_code and not updated to MarketKey as per instructions.
                        // If it were active, it would need a MarketKey to construct the MarketEvent.
                        // For now, it remains as is.
                        // if let Err(e) = event_tx.send(MarketEvent::OrderBookUpdate {
                        //     symbol: symbol.to_string(),
                        //     orderbook: orderbook.clone(),
                        // }) {
                        //     error!("Failed to send orderbook update for {}: {}", symbol, e);
                        // }
                    }
                }
            }
            stream if stream.contains("trade") => {
                // Parse trade message
                if let Ok(_trade_msg) =
                    serde_json::from_value::<crate::binance::types::TradeMessage>(binance_msg.data)
                {
                    // Send price update
                    // NOTE: This function is marked dead_code and not updated to MarketKey as per instructions.
                    // If it were active, it would need a MarketKey to construct the MarketEvent.
                    // For now, it remains as is.
                    // if let Ok(price) = trade_msg.price.parse::<f64>() {
                    //     if let Err(e) = event_tx.send(MarketEvent::PriceUpdate {
                    //         symbol: symbol.to_string(),
                    //         price,
                    //         time: trade_msg.event_time,
                    //     }) {
                    //         error!("Failed to send price update for {}: {}", symbol, e);
                    //     }
                    // } else {
                    //     error!("Failed to parse price for {}: {}", symbol, trade_msg.price);
                    // }
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
    pub async fn unsubscribe(&self, key: &MarketKey) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(handle) = subscriptions.remove(key) {
            info!("Unsubscribing from key: {:?}", key);

            // Send shutdown signal to subscription task
            if let Err(e) = handle.control_tx.send(ControlMessage::Shutdown) {
                warn!("Failed to send shutdown signal for {:?}: {}", key, e);
            }

            let task = handle.task;
            task.abort();

            // Wait for task to complete
            match task.await {
                Ok(_) => debug!("Subscription task for {:?} terminated", key),
                Err(e) if e.is_cancelled() => {
                    debug!("Subscription task for {:?} cancelled during shutdown", key)
                }
                Err(e) => {
                    error!("Subscription task for {:?} failed: {}", key, e);
                }
            }

            info!("Successfully unsubscribed from key: {:?}", key);
        } else {
            debug!("Key {:?} was not subscribed", key);
        }

        Ok(())
    }

    /// Get list of subscribed symbols
    pub async fn list_subscriptions(&self) -> Vec<MarketKey> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.keys().cloned().collect()
    }

    /// Get orderbook for a symbol
    pub async fn get_orderbook(&self, key: &MarketKey) -> Option<OrderBook> {
        let orderbooks = self.orderbooks.read().await;
        orderbooks.get(key).cloned()
    }

    /// Recover subscription state after reconnection
    pub async fn recover_subscription(&self, key: &MarketKey) -> Result<()> {
        info!("Recovering subscription for key: {:?}", key);

        // Check if symbol is subscribed
        let subscriptions = self.subscriptions.read().await;
        if !subscriptions.contains_key(key) {
            return Err(anyhow::anyhow!("Key {:?} is not subscribed", key));
        }

        // Send reconnect signal to subscription
        if let Some(handle) = subscriptions.get(key) {
            if let Err(e) = handle.control_tx.send(ControlMessage::Reconnect) {
                error!("Failed to send reconnect signal for {:?}: {}", key, e);
                return Err(e.into());
            }
        }

        info!("Successfully initiated recovery for key: {:?}", key);
        Ok(())
    }

    /// Check if subscription needs recovery
    pub async fn needs_recovery(&self, key: &MarketKey, max_stale_time_ms: u64) -> bool {
        let orderbooks = self.orderbooks.read().await;

        if let Some(orderbook) = orderbooks.get(key) {
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
        let keys: Vec<MarketKey> = subscriptions.keys().cloned().collect();

        for key in keys {
            if self.needs_recovery(&key, max_stale_time_ms).await {
                info!("Key {:?} needs recovery, triggering reconnect", key);

                // Send reconnect signal
                if let Some(handle) = subscriptions.get(&key) {
                    if let Err(e) = handle.control_tx.send(ControlMessage::Reconnect) {
                        error!("Failed to send reconnect signal for {:?}: {}", key, e);
                    }
                }
            }
        }

        info!("Reconnection event handled");
        Ok(())
    }

    /// Get connection quality metrics
    pub async fn get_connection_quality(&self, key: &MarketKey) -> Option<ConnectionQuality> {
        let orderbooks = self.orderbooks.read().await;

        if let Some(orderbook) = orderbooks.get(key) {
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
                symbol: key.symbol.clone(), // ConnectionQuality struct has symbol: String. Leave as string?
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
        if let MarketEvent::OrderBookUpdate { key, orderbook } = event {
            let mut orderbooks = self.orderbooks.write().await;
            orderbooks.insert(key.clone(), orderbook.clone());
        }
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
