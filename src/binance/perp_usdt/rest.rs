//! Binance USDT-M Perpetual REST API client implementation

use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde::de::{Error as DeError, IgnoredAny};
use tracing::{debug, info};

use crate::binance::types::{DepthSnapshot, MarkPriceUpdate};
use crate::market_data::DailyCandle;

/// Binance USDT-M Perpetual REST API client
pub struct BinancePerpRestClient {
    base_url: String,
    client: reqwest::Client,
}

impl BinancePerpRestClient {
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
            "{}/fapi/v1/depth?symbol={}&limit={}",
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

    /// Get premium index (Mark Price)
    pub async fn get_premium_index(&self, symbol: &str) -> Result<MarkPriceUpdate> {
        let url = format!("{}/fapi/v1/premiumIndex?symbol={}", self.base_url, symbol);

        debug!("Fetching premium index from: {}", url);

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

        // Note: The REST response structure for premiumIndex might differ slightly from logic in types.rs which assumes WS event structure.
        // But let's check definition of MarkPriceUpdate in types.rs.
        // It has serde renames like "p", "i", "r", "T".
        // REST response: { "symbol": "BTCUSDT", "markPrice": "...", "indexPrice": "...", "lastFundingRate": "...", "nextFundingTime": ... }
        // So I cannot reuse `MarkPriceUpdate` struct directly if it uses short field names "p", "i", etc.
        // `MarkPriceUpdate` in `types.rs` uses `[serde(rename = "p")]`.
        // I should create a separate struct or use `#[serde(alias = "markPrice")]`.
        // For now, I'll create a local struct or update `types.rs`.
        // Let's create a local struct to parse and convert.

        #[derive(Deserialize)]
        #[allow(non_snake_case)]
        struct PremiumIndexResponse {
            symbol: String,
            markPrice: String,
            indexPrice: String,
            estimatedSettlePrice: String,
            lastFundingRate: String,
            nextFundingTime: u64,
            // time: u64?
        }

        let pi: PremiumIndexResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse premium index: {}", e))?;

        Ok(MarkPriceUpdate {
            event_type: "premiumIndex".to_string(), // Synthetic
            event_time: 0,                          // Using 0 or request time
            symbol: pi.symbol,
            mark_price: pi.markPrice,
            index_price: pi.indexPrice,
            estimated_settle_price: pi.estimatedSettlePrice,
            funding_rate: pi.lastFundingRate,
            next_funding_time: pi.nextFundingTime,
        })
    }

    /// Get daily klines (1d interval) for a symbol
    pub async fn get_daily_klines(
        &self,
        symbol: &str,
        limit: Option<u16>,
    ) -> Result<Vec<DailyCandle>> {
        let clamped_limit = limit.unwrap_or(90).clamp(1, 1000);
        let url = format!(
            "{}/fapi/v1/klines?symbol={}&interval=1d&limit={}",
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
