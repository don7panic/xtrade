//! Market data processing and management module

use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

use crate::binance::BinanceRestClient;
use crate::binance::types::OrderBook;

/// Subscription status for a symbol
#[derive(Debug, Clone)]
pub enum SubscriptionStatus {
    Active,
    Reconnecting,
    Failed,
}

/// Extended subscription information
#[derive(Debug)]
struct SubscriptionInfo {
    pub orderbook: OrderBook,
    pub status: SubscriptionStatus,
    pub last_successful_update: AtomicU64,
    pub reconnect_count: AtomicU64,
}

/// Market data manager for handling multiple symbol subscriptions
pub struct MarketDataManager {
    subscriptions: HashMap<String, OrderBook>,
    rest_client: BinanceRestClient,
}

impl MarketDataManager {
    /// Create a new MarketDataManager
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
            rest_client: BinanceRestClient::new("https://api.binance.com".to_string()),
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
        orderbook.fetch_snapshot(&self.rest_client).await?;

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

    /// Recover subscription state after reconnection
    pub async fn recover_subscription(&mut self, symbol: &str) -> Result<()> {
        if let Some(orderbook) = self.subscriptions.get_mut(symbol) {
            info!("Recovering subscription for symbol: {}", symbol);

            // Fetch fresh snapshot to ensure data consistency
            match orderbook.fetch_snapshot(&self.rest_client).await {
                Ok(_) => {
                    info!("Successfully recovered subscription for symbol: {}", symbol);
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to recover subscription for {}: {}", symbol, e);
                    Err(e)
                }
            }
        } else {
            warn!(
                "Cannot recover subscription for unsubscribed symbol: {}",
                symbol
            );
            Err(anyhow::anyhow!("Symbol {} not subscribed", symbol))
        }
    }

    /// Check if subscription needs recovery (based on last update time)
    pub fn needs_recovery(&self, symbol: &str, max_stale_time_ms: u64) -> bool {
        if let Some(orderbook) = self.subscriptions.get(symbol) {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            let stale_time = current_time.saturating_sub(orderbook.snapshot_time);
            stale_time > max_stale_time_ms
        } else {
            false
        }
    }

    /// Handle reconnection event - recover all stale subscriptions
    pub async fn handle_reconnection(&mut self, max_stale_time_ms: u64) -> Result<()> {
        info!("Handling reconnection event, recovering stale subscriptions");

        let symbols_to_recover: Vec<String> = self
            .subscriptions
            .keys()
            .filter(|symbol| self.needs_recovery(symbol, max_stale_time_ms))
            .cloned()
            .collect();

        info!(
            "Found {} subscriptions that need recovery",
            symbols_to_recover.len()
        );

        for symbol in symbols_to_recover {
            match self.recover_subscription(&symbol).await {
                Ok(_) => {
                    info!("Successfully recovered subscription for {}", symbol);
                }
                Err(e) => {
                    warn!("Failed to recover subscription for {}: {}", symbol, e);
                }
            }
        }

        info!("Reconnection recovery process completed");
        Ok(())
    }

    /// Get connection quality metrics for a symbol
    pub fn get_connection_quality(&self, symbol: &str) -> Option<ConnectionQuality> {
        self.subscriptions.get(symbol).map(|orderbook| {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            let time_since_update = current_time.saturating_sub(orderbook.snapshot_time);
            let data_freshness = if time_since_update < 1000 {
                "excellent"
            } else if time_since_update < 5000 {
                "good"
            } else if time_since_update < 30000 {
                "fair"
            } else {
                "poor"
            };

            ConnectionQuality {
                symbol: symbol.to_string(),
                data_freshness: data_freshness.to_string(),
                time_since_last_update_ms: time_since_update,
                orderbook_depth: orderbook.total_levels(),
                spread: orderbook.spread().unwrap_or(0.0),
            }
        })
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

impl Default for MarketDataManager {
    fn default() -> Self {
        Self::new()
    }
}
