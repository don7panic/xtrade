//! Binance REST API client implementation

use anyhow::{Result, anyhow};
use tracing::{debug, info, warn};

use super::types::{DepthSnapshot, Symbol, Ticker24hr};

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

    /// Get 24hr ticker information for a symbol
    pub async fn get_24hr_ticker(&self, symbol: &str) -> Result<Ticker24hr> {
        let url = format!("{}/api/v3/ticker/24hr?symbol={}", self.base_url, symbol);

        debug!("Fetching 24hr ticker from: {}", url);

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

        let ticker: Ticker24hr = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse 24hr ticker: {}", e))?;

        info!("Successfully fetched 24hr ticker for {}", symbol);

        Ok(ticker)
    }

    /// Get exchange information
    pub async fn get_exchange_info(&self) -> Result<ExchangeInfo> {
        let url = format!("{}/api/v3/exchangeInfo", self.base_url);

        debug!("Fetching exchange info from: {}", url);

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

        let exchange_info: ExchangeInfo = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse exchange info: {}", e))?;

        info!("Successfully fetched exchange info");

        Ok(exchange_info)
    }

    /// Get server time
    pub async fn get_server_time(&self) -> Result<u64> {
        let url = format!("{}/api/v3/time", self.base_url);

        debug!("Fetching server time from: {}", url);

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

        let time_response: TimeResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse server time: {}", e))?;

        Ok(time_response.server_time)
    }

    /// Get all symbols from exchange
    pub async fn get_all_symbols(&self) -> Result<Vec<Symbol>> {
        let exchange_info = self.get_exchange_info().await?;
        Ok(exchange_info.symbols)
    }

    /// Validate symbol exists
    pub async fn validate_symbol(&self, symbol: &str) -> Result<bool> {
        let symbols = self.get_all_symbols().await?;
        Ok(symbols.iter().any(|s| s.symbol == symbol))
    }

    /// Get current price for a symbol
    pub async fn get_price(&self, symbol: &str) -> Result<f64> {
        let ticker = self.get_24hr_ticker(symbol).await?;
        ticker
            .last_price
            .parse::<f64>()
            .map_err(|e| anyhow!("Failed to parse price: {}", e))
    }

    /// Batch get prices for multiple symbols
    pub async fn get_prices(&self, symbols: &[String]) -> Result<Vec<(String, f64)>> {
        let mut prices = Vec::new();

        for symbol in symbols {
            match self.get_price(symbol).await {
                Ok(price) => {
                    prices.push((symbol.clone(), price));
                }
                Err(e) => {
                    warn!("Failed to get price for {}: {}", symbol, e);
                }
            }
        }

        Ok(prices)
    }
}

/// Exchange information response
#[derive(Debug, serde::Deserialize)]
pub struct ExchangeInfo {
    pub symbols: Vec<Symbol>,
}

/// Server time response
#[derive(Debug, serde::Deserialize)]
pub struct TimeResponse {
    pub server_time: u64,
}
