//! Market data processing and management module

use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info};

use crate::binance::BinanceRestClient;
use crate::binance::types::OrderBook;

/// Market data manager for handling multiple symbol subscriptions
pub struct MarketDataManager {
    subscriptions: HashMap<String, OrderBook>,
}

impl MarketDataManager {
    /// Create a new MarketDataManager
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
        }
    }

    /// Subscribe to a symbol
    pub async fn subscribe(&mut self, symbol: String) -> Result<()> {
        if self.subscriptions.contains_key(&symbol) {
            debug!("Symbol {} is already subscribed", symbol);
            return Ok(());
        }

        info!("Subscribing to symbol: {}", symbol);

        // Create orderbook and fetch initial snapshot
        let mut orderbook = OrderBook::new(symbol.clone());
        let rest_client = BinanceRestClient::new("https://api.binance.com".to_string());
        orderbook.fetch_snapshot(&rest_client).await?;

        self.subscriptions.insert(symbol.clone(), orderbook);

        info!("Successfully subscribed to symbol: {}", symbol);
        Ok(())
    }

    /// Unsubscribe from a symbol
    pub async fn unsubscribe(&mut self, symbol: &str) -> Result<()> {
        if let Some(_) = self.subscriptions.remove(symbol) {
            info!("Successfully unsubscribed from symbol: {}", symbol);
        } else {
            debug!("Symbol {} was not subscribed", symbol);
        }

        Ok(())
    }

    /// Get list of subscribed symbols
    pub fn list_subscriptions(&self) -> Vec<String> {
        self.subscriptions.keys().cloned().collect()
    }

    /// Get orderbook for a symbol
    pub async fn get_orderbook(&self, symbol: &str) -> Option<OrderBook> {
        self.subscriptions.get(symbol).cloned()
    }
}

impl Default for MarketDataManager {
    fn default() -> Self {
        Self::new()
    }
}
