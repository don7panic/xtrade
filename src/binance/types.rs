//! Binance API data types and structures

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::rest::BinanceRestClient;

/// Connection status for WebSocket
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

/// Generic Binance WebSocket message wrapper
#[derive(Debug, Deserialize)]
pub struct BinanceMessage {
    pub stream: String,
    pub data: serde_json::Value,
}

/// Trading symbol information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
}

/// OrderBook structure for managing bid/ask data
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: std::collections::BTreeMap<ordered_float::OrderedFloat<f64>, f64>,
    pub asks: std::collections::BTreeMap<ordered_float::OrderedFloat<f64>, f64>,
    pub last_update_id: u64,
    pub snapshot_time: u64,
    pub last_update_time: u64,
}

impl OrderBook {
    /// Create a new empty OrderBook
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            bids: std::collections::BTreeMap::new(),
            asks: std::collections::BTreeMap::new(),
            last_update_id: 0,
            snapshot_time: 0,
            last_update_time: 0,
        }
    }

    /// Get the best bid price
    pub fn best_bid(&self) -> Option<f64> {
        self.bids.keys().next_back().map(|k| k.0)
    }

    /// Get the best ask price
    pub fn best_ask(&self) -> Option<f64> {
        self.asks.keys().next().map(|k| k.0)
    }

    /// Get the spread between best bid and ask
    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Fetch orderbook snapshot from Binance REST API
    pub async fn fetch_snapshot(&mut self, rest_client: &BinanceRestClient) -> Result<()> {
        let snapshot = rest_client
            .get_depth_snapshot_default(&self.symbol)
            .await
            .map_err(|e| anyhow!("Failed to fetch depth snapshot for {}: {}", self.symbol, e))?;

        self.update_from_snapshot(snapshot)?;

        Ok(())
    }

    /// Update orderbook from snapshot data
    pub fn update_from_snapshot(&mut self, snapshot: DepthSnapshot) -> Result<()> {
        // Clear existing data
        self.bids.clear();
        self.asks.clear();

        // Process bids
        for bid in snapshot.bids {
            let price = bid[0]
                .parse::<f64>()
                .map_err(|e| anyhow!("Failed to parse bid price: {}", e))?;
            let quantity = bid[1]
                .parse::<f64>()
                .map_err(|e| anyhow!("Failed to parse bid quantity: {}", e))?;

            if quantity > 0.0 {
                self.bids
                    .insert(ordered_float::OrderedFloat(price), quantity);
            }
        }

        // Process asks
        for ask in snapshot.asks {
            let price = ask[0]
                .parse::<f64>()
                .map_err(|e| anyhow!("Failed to parse ask price: {}", e))?;
            let quantity = ask[1]
                .parse::<f64>()
                .map_err(|e| anyhow!("Failed to parse ask quantity: {}", e))?;

            if quantity > 0.0 {
                self.asks
                    .insert(ordered_float::OrderedFloat(price), quantity);
            }
        }

        // Update metadata
        self.last_update_id = snapshot.last_update_id;
        self.snapshot_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.last_update_time = self.snapshot_time;

        Ok(())
    }

    /// Apply incremental depth update to orderbook
    /// Implements Binance's official update sequence validation
    pub fn apply_depth_update(&mut self, update: OrderBookUpdate) -> Result<(), OrderBookError> {
        use tracing::{debug, warn};

        // Validate symbol match
        if update.symbol != self.symbol {
            return Err(OrderBookError::SymbolMismatch {
                expected: self.symbol.clone(),
                actual: update.symbol,
            });
        }

        // Sequence number validation according to Binance documentation:
        // 1. Drop any event where first_update_id <= lastUpdateId in the snapshot
        if update.first_update_id <= self.last_update_id {
            debug!(
                "Discarding stale update: first_update_id {} <= last_update_id {}",
                update.first_update_id, self.last_update_id
            );
            return Err(OrderBookError::StaleMessage {
                update_id: update.first_update_id,
                snapshot_id: self.last_update_id,
            });
        }

        // 2. The first processed event should have first_update_id <= lastUpdateId+1 AND final_update_id >= lastUpdateId+1
        if self.last_update_id > 0 && update.first_update_id > self.last_update_id + 1 {
            warn!(
                "Sequence gap detected: expected first_update_id <= {}, got {}",
                self.last_update_id + 1,
                update.first_update_id
            );
            return Err(OrderBookError::SequenceValidationFailed {
                expected: self.last_update_id + 1,
                actual: update.first_update_id,
            });
        }

        debug!(
            "Applying depth update for {}: first_id={}, final_id={}, bids={}, asks={}",
            update.symbol,
            update.first_update_id,
            update.final_update_id,
            update.bids.len(),
            update.asks.len()
        );

        // Apply bid updates
        for bid in update.bids {
            let price = bid[0]
                .parse::<f64>()
                .map_err(|e| OrderBookError::PriceParseError(format!("bid price: {}", e)))?;
            let quantity = bid[1]
                .parse::<f64>()
                .map_err(|e| OrderBookError::QuantityParseError(format!("bid quantity: {}", e)))?;

            let price_key = ordered_float::OrderedFloat(price);

            if quantity == 0.0 {
                // Remove price level when quantity is 0
                if let Some(removed_qty) = self.bids.remove(&price_key) {
                    debug!(
                        "Removed bid level at price {}: quantity {}",
                        price, removed_qty
                    );
                }
            } else if quantity > 0.0 {
                // Update or insert price level
                let previous_qty = self.bids.insert(price_key, quantity);
                debug!(
                    "Updated bid level at price {}: {} -> {}",
                    price,
                    previous_qty.unwrap_or(0.0),
                    quantity
                );
            } else {
                return Err(OrderBookError::InvalidUpdate(format!(
                    "Invalid bid quantity: {}",
                    quantity
                )));
            }
        }

        // Apply ask updates
        for ask in update.asks {
            let price = ask[0]
                .parse::<f64>()
                .map_err(|e| OrderBookError::PriceParseError(format!("ask price: {}", e)))?;
            let quantity = ask[1]
                .parse::<f64>()
                .map_err(|e| OrderBookError::QuantityParseError(format!("ask quantity: {}", e)))?;

            let price_key = ordered_float::OrderedFloat(price);

            if quantity == 0.0 {
                // Remove price level when quantity is 0
                if let Some(removed_qty) = self.asks.remove(&price_key) {
                    debug!(
                        "Removed ask level at price {}: quantity {}",
                        price, removed_qty
                    );
                }
            } else if quantity > 0.0 {
                // Update or insert price level
                let previous_qty = self.asks.insert(price_key, quantity);
                debug!(
                    "Updated ask level at price {}: {} -> {}",
                    price,
                    previous_qty.unwrap_or(0.0),
                    quantity
                );
            } else {
                return Err(OrderBookError::InvalidUpdate(format!(
                    "Invalid ask quantity: {}",
                    quantity
                )));
            }
        }

        // Update sequence tracking
        self.last_update_id = update.final_update_id;

        self.last_update_time = update.event_time;

        debug!(
            "Applied depth update successfully. New last_update_id: {}, bids: {}, asks: {}",
            self.last_update_id,
            self.bids.len(),
            self.asks.len()
        );

        Ok(())
    }

    /// Validates that the OrderBook is in a consistent state
    pub fn validate_consistency(&self) -> Result<(), OrderBookError> {
        // Check that best bid < best ask (if both exist)
        if let (Some(best_bid), Some(best_ask)) = (self.best_bid(), self.best_ask()) {
            if best_bid >= best_ask {
                return Err(OrderBookError::InvalidUpdate(format!(
                    "Invalid spread: best_bid {} >= best_ask {}",
                    best_bid, best_ask
                )));
            }
        }

        // Check for negative quantities (should never happen with our logic)
        for (price, qty) in &self.bids {
            if *qty <= 0.0 {
                return Err(OrderBookError::InvalidUpdate(format!(
                    "Invalid bid quantity {} at price {}",
                    qty, price.0
                )));
            }
        }

        for (price, qty) in &self.asks {
            if *qty <= 0.0 {
                return Err(OrderBookError::InvalidUpdate(format!(
                    "Invalid ask quantity {} at price {}",
                    qty, price.0
                )));
            }
        }

        Ok(())
    }

    /// Gets the total number of price levels in the orderbook
    pub fn total_levels(&self) -> usize {
        self.bids.len() + self.asks.len()
    }

    /// Gets the total volume on the bid side
    pub fn total_bid_volume(&self) -> f64 {
        self.bids.values().sum()
    }

    /// Gets the total volume on the ask side  
    pub fn total_ask_volume(&self) -> f64 {
        self.asks.values().sum()
    }

    /// Checks if the orderbook has sufficient data for trading decisions
    pub fn has_sufficient_depth(&self, min_levels: usize) -> bool {
        self.bids.len() >= min_levels && self.asks.len() >= min_levels
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new("".to_string())
    }
}

/// Binance WebSocket subscription request
#[derive(Debug, Serialize)]
pub struct SubscribeRequest {
    pub method: String,
    pub params: Vec<String>,
    pub id: u64,
}

impl SubscribeRequest {
    /// Create a new subscription request for a symbol
    pub fn new(symbol: &str, stream_type: &str) -> Self {
        let stream_name = format!("{}@{}", symbol.to_lowercase(), stream_type);
        Self {
            method: "SUBSCRIBE".to_string(),
            params: vec![stream_name],
            id: 1,
        }
    }
}

/// Binance WebSocket unsubscribe request
#[derive(Debug, Serialize)]
pub struct UnsubscribeRequest {
    pub method: String,
    pub params: Vec<String>,
    pub id: u64,
}

impl UnsubscribeRequest {
    /// Create a new unsubscribe request for a symbol
    pub fn new(symbol: &str, stream_type: &str) -> Self {
        let stream_name = format!("{}@{}", symbol.to_lowercase(), stream_type);
        Self {
            method: "UNSUBSCRIBE".to_string(),
            params: vec![stream_name],
            id: 1,
        }
    }
}

/// Binance WebSocket response message
#[derive(Debug, Deserialize)]
pub struct BinanceResponse {
    pub result: Option<serde_json::Value>,
    pub id: Option<u64>,
    pub error: Option<serde_json::Value>,
}

/// OrderBook update message from Binance
#[derive(Debug, Deserialize, Serialize)]
pub struct OrderBookUpdate {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}

/// Depth snapshot from Binance REST API
#[derive(Debug, Deserialize)]
pub struct DepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

/// Trade message from Binance
#[derive(Debug, Deserialize, Serialize)]
pub struct TradeMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "t")]
    pub trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub quantity: String,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// 24hr ticker message from Binance
#[derive(Debug, Deserialize, Serialize)]
pub struct Ticker24hr {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub price_change: String,
    #[serde(rename = "P")]
    pub price_change_percent: String,
    #[serde(rename = "w")]
    pub weighted_avg_price: String,
    #[serde(rename = "c")]
    pub last_price: String,
    #[serde(rename = "Q")]
    pub last_quantity: String,
    #[serde(rename = "o")]
    pub open_price: String,
    #[serde(rename = "h")]
    pub high_price: String,
    #[serde(rename = "l")]
    pub low_price: String,
    #[serde(rename = "v")]
    pub volume: String,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "O")]
    pub open_time: u64,
    #[serde(rename = "C")]
    pub close_time: u64,
    #[serde(rename = "F")]
    pub first_trade_id: u64,
    #[serde(rename = "L")]
    pub last_trade_id: u64,
    #[serde(rename = "n")]
    pub total_trades: u64,
}

/// WebSocket kline stream event wrapper
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KlineStreamEvent {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: KlineData,
}

/// Detailed kline payload
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KlineData {
    #[serde(rename = "t")]
    pub start_time: u64,
    #[serde(rename = "T")]
    pub close_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub interval: String,
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    #[serde(rename = "L")]
    pub last_trade_id: u64,
    #[serde(rename = "o")]
    pub open: String,
    #[serde(rename = "c")]
    pub close: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    #[serde(rename = "v")]
    pub volume: String,
    #[serde(rename = "n")]
    pub number_of_trades: u64,
    #[serde(rename = "x")]
    pub is_final: bool,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "V")]
    pub taker_buy_base_volume: String,
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String,
    #[serde(rename = "B")]
    pub ignore: String,
}

/// Error types for WebSocket operations
#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
pub enum WebSocketError {
    #[error("WebSocket connection error: {0}")]
    ConnectionError(String),
    #[error("WebSocket message error: {0}")]
    MessageError(String),
    #[error("Subscription error: {0}")]
    SubscriptionError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Error types for REST API operations
#[derive(Debug, thiserror::Error)]
pub enum RestApiError {
    #[error("HTTP request error: {0}")]
    HttpRequestError(String),
    #[error("HTTP status error: {0} - {1}")]
    HttpStatusError(u16, String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),
}

/// Error types for OrderBook operations
#[derive(Debug, thiserror::Error)]
pub enum OrderBookError {
    #[error("Sequence validation failed: expected first_update_id > {expected}, got {actual}")]
    SequenceValidationFailed { expected: u64, actual: u64 },
    #[error(
        "Stale message received: update_id {update_id} <= snapshot last_update_id {snapshot_id}"
    )]
    StaleMessage { update_id: u64, snapshot_id: u64 },
    #[error("Price parse error: {0}")]
    PriceParseError(String),
    #[error("Quantity parse error: {0}")]
    QuantityParseError(String),
    #[error("Symbol mismatch: expected {expected}, got {actual}")]
    SymbolMismatch { expected: String, actual: String },
    #[error("Invalid update: {0}")]
    InvalidUpdate(String),
}

impl OrderBookError {
    /// Returns true if this error is recoverable (e.g., can continue processing other messages)
    pub fn is_recoverable(&self) -> bool {
        match self {
            OrderBookError::StaleMessage { .. } => true, // Can safely ignore stale messages
            OrderBookError::PriceParseError(_) => false, // Data corruption, need fresh snapshot
            OrderBookError::QuantityParseError(_) => false, // Data corruption, need fresh snapshot
            OrderBookError::SequenceValidationFailed { .. } => false, // Need to re-sync
            OrderBookError::SymbolMismatch { .. } => true, // Wrong symbol, can ignore
            OrderBookError::InvalidUpdate(_) => false,   // Data integrity issue
        }
    }

    /// Returns true if this error requires fetching a new orderbook snapshot
    pub fn requires_resync(&self) -> bool {
        matches!(
            self,
            OrderBookError::SequenceValidationFailed { .. }
                | OrderBookError::PriceParseError(_)
                | OrderBookError::QuantityParseError(_)
                | OrderBookError::InvalidUpdate(_)
        )
    }

    /// Returns the error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            OrderBookError::StaleMessage { .. } => ErrorSeverity::Info,
            OrderBookError::SymbolMismatch { .. } => ErrorSeverity::Warning,
            OrderBookError::SequenceValidationFailed { .. } => ErrorSeverity::Error,
            OrderBookError::PriceParseError(_) => ErrorSeverity::Critical,
            OrderBookError::QuantityParseError(_) => ErrorSeverity::Critical,
            OrderBookError::InvalidUpdate(_) => ErrorSeverity::Error,
        }
    }
}

/// Error severity levels for OrderBook operations
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Info,     // Can be ignored
    Warning,  // Should be logged but processing can continue
    Error,    // Requires action but system can recover
    Critical, // Requires immediate resync/restart
}

/// Binance WebSocket event types
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub enum BinanceEventType {
    #[serde(rename = "depthUpdate")]
    DepthUpdate,
    #[serde(rename = "trade")]
    Trade,
    #[serde(rename = "24hrTicker")]
    Ticker24hr,
    #[serde(rename = "kline")]
    Kline,
    #[serde(rename = "aggTrade")]
    AggregatedTrade,
}

// Additional types will be added in subsequent days
