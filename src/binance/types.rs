//! Binance API data types and structures

use serde::{Deserialize, Serialize};

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
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new("".to_string())
    }
}

// Additional types will be added in subsequent days
