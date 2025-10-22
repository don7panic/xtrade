//! Binance REST API client implementation

use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde::de::{Error as DeError, IgnoredAny};
use tracing::{debug, info, warn};

use super::types::{DepthSnapshot, Symbol, Ticker24hr};
use crate::market_data::DailyCandle;

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

    /// Get daily klines (1d interval) for a symbol
    pub async fn get_daily_klines(
        &self,
        symbol: &str,
        limit: Option<u16>,
    ) -> Result<Vec<DailyCandle>> {
        let clamped_limit = limit.unwrap_or(90).clamp(1, 1000);
        let url = format!(
            "{}/api/v3/klines?symbol={}&interval=1d&limit={}",
            self.base_url, symbol, clamped_limit
        );

        debug!(
            "Fetching daily klines from: {} (limit={})",
            url, clamped_limit
        );

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

        let rows: Vec<RestKlineRow> = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse daily klines: {}", e))?;

        let candles: Vec<DailyCandle> = rows.into_iter().map(DailyCandle::from).collect();

        info!(
            "Successfully fetched {} daily klines for {}",
            candles.len(),
            symbol
        );

        Ok(candles)
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

#[derive(Debug, Deserialize)]
struct RestKlineRow(
    #[serde(deserialize_with = "deserialize_u64_from_any")] u64,
    #[serde(deserialize_with = "deserialize_f64_from_any")] f64,
    #[serde(deserialize_with = "deserialize_f64_from_any")] f64,
    #[serde(deserialize_with = "deserialize_f64_from_any")] f64,
    #[serde(deserialize_with = "deserialize_f64_from_any")] f64,
    #[serde(deserialize_with = "deserialize_f64_from_any")] f64,
    #[serde(deserialize_with = "deserialize_u64_from_any")] u64,
    IgnoredAny,
    IgnoredAny,
    IgnoredAny,
    IgnoredAny,
    IgnoredAny,
);

impl From<RestKlineRow> for DailyCandle {
    fn from(row: RestKlineRow) -> Self {
        let RestKlineRow(open_time_ms, open, high, low, close, volume, close_time_ms, ..) = row;
        DailyCandle::new(
            open_time_ms,
            close_time_ms,
            open,
            high,
            low,
            close,
            volume,
            true,
        )
    }
}

fn deserialize_u64_from_any<'de, D>(deserializer: D) -> std::result::Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(num) => num
            .as_u64()
            .ok_or_else(|| DeError::custom("failed to read numeric u64 value")),
        serde_json::Value::String(s) => s
            .parse::<u64>()
            .map_err(|e| DeError::custom(format!("failed to parse u64 '{}': {}", s, e))),
        _ => Err(DeError::custom("expected number or string for u64")),
    }
}

fn deserialize_f64_from_any<'de, D>(deserializer: D) -> std::result::Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(num) => num
            .as_f64()
            .ok_or_else(|| DeError::custom("failed to read numeric f64 value")),
        serde_json::Value::String(s) => s
            .parse::<f64>()
            .map_err(|e| DeError::custom(format!("failed to parse f64 '{}': {}", s, e))),
        _ => Err(DeError::custom("expected number or string for f64")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rest_kline_row_deserializes_from_payload() {
        let payload = json!([
            1_700_000_000_000u64,
            "100.12",
            "105.55",
            "98.01",
            "103.78",
            "1234.56",
            1_700_086_400_000u64,
            "0",
            "0",
            308,
            "0",
            "0"
        ]);

        let row: RestKlineRow =
            serde_json::from_value(payload).expect("row should deserialize from payload");
        let candle: DailyCandle = row.into();

        assert_eq!(candle.open_time_ms, 1_700_000_000_000u64);
        assert_eq!(candle.close_time_ms, 1_700_086_400_000u64);
        assert!((candle.open - 100.12).abs() < f64::EPSILON);
        assert!((candle.high - 105.55).abs() < f64::EPSILON);
        assert!((candle.low - 98.01).abs() < f64::EPSILON);
        assert!((candle.close - 103.78).abs() < f64::EPSILON);
        assert!((candle.volume - 1234.56).abs() < f64::EPSILON);
        assert!(candle.is_closed);
    }
}
