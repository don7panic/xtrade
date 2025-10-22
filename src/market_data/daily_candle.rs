//! Daily candle data structure and helpers

use anyhow::{Result, anyhow};
use serde_json::Value;

/// Default number of daily candles to retain per symbol
pub const DEFAULT_DAILY_CANDLE_LIMIT: usize = 90;

/// Simplified daily candle representation shared across the app
#[derive(Debug, Clone, PartialEq)]
pub struct DailyCandle {
    pub open_time_ms: u64,
    pub close_time_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub is_closed: bool,
}

impl DailyCandle {
    /// Create a new candle from primitive values
    #[allow(dead_code)]
    pub fn new(
        open_time_ms: u64,
        close_time_ms: u64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        is_closed: bool,
    ) -> Self {
        Self {
            open_time_ms,
            close_time_ms,
            open,
            high,
            low,
            close,
            volume,
            is_closed,
        }
    }

    /// Build a candle from REST kline array payload
    pub fn try_from_rest_row(row: &[Value]) -> Result<Self> {
        if row.len() < 7 {
            return Err(anyhow!(
                "expected at least 7 fields for kline row, got {}",
                row.len()
            ));
        }

        let open_time_ms = row[0]
            .as_i64()
            .ok_or_else(|| anyhow!("invalid open time value"))? as u64;
        let close_time_ms = row[6]
            .as_i64()
            .ok_or_else(|| anyhow!("invalid close time value"))? as u64;

        let open = parse_f64(&row[1], "open")?;
        let high = parse_f64(&row[2], "high")?;
        let low = parse_f64(&row[3], "low")?;
        let close = parse_f64(&row[4], "close")?;
        let volume = parse_f64(&row[5], "volume")?;

        Ok(Self {
            open_time_ms,
            close_time_ms,
            open,
            high,
            low,
            close,
            volume,
            is_closed: true,
        })
    }
}

fn parse_f64(value: &Value, field: &str) -> Result<f64> {
    match value {
        Value::String(s) => s
            .parse::<f64>()
            .map_err(|e| anyhow!("failed to parse {} '{}': {}", field, s, e)),
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| anyhow!("failed to read numeric {} value", field)),
        _ => Err(anyhow!("unexpected type for {} field", field)),
    }
}
