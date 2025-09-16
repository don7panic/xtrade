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

        Ok(())
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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
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

/// Error types for WebSocket operations
#[derive(Debug, thiserror::Error)]
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

// Additional types will be added in subsequent days
