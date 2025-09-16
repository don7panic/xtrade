//! Binance REST API client implementation

use anyhow::{Result, anyhow};
use tracing::{debug, info};

use super::types::DepthSnapshot;

/// Binance REST API client
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

    /// Get orderbook depth snapshot for a symbol
    pub async fn get_depth_snapshot(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<DepthSnapshot> {
        let url = format!(
            "{}/api/v3/depth?symbol={}&limit={}",
            self.base_url,
            symbol,
            limit.unwrap_or(1000)
        );

        debug!("Fetching depth snapshot from: {}", url);

        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send HTTP request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("HTTP error {}: {}", status, body));
        }

        let snapshot: DepthSnapshot = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse depth snapshot: {}", e))?;

        info!(
            "Successfully fetched depth snapshot for {}: {} bids, {} asks, lastUpdateId: {}",
            symbol,
            snapshot.bids.len(),
            snapshot.asks.len(),
            snapshot.last_update_id
        );

        Ok(snapshot)
    }

    /// Get orderbook depth snapshot with default limit of 1000
    pub async fn get_depth_snapshot_default(&self, symbol: &str) -> Result<DepthSnapshot> {
        self.get_depth_snapshot(symbol, None).await
    }
}
