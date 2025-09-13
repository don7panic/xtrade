//! Binance REST API client implementation

// use anyhow::Result; // Will be used in Day 7

/// Binance REST API client
#[allow(dead_code)] // Will be implemented in Day 7
pub struct BinanceRestClient {
    base_url: String,
    client: reqwest::Client,
}

impl BinanceRestClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
}

// Placeholder implementation - will be expanded in Day 7
