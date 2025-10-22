//! Daily candle data structure and helpers

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
    #[allow(clippy::too_many_arguments)]
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
}
