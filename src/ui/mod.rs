//! User Interface module
//!
//! Provides both TUI (Terminal User Interface) and simple CLI output capabilities.

/// TUI application state and rendering
pub mod tui;

/// UI Manager for interactive interface
pub mod ui_manager;

use crate::binance::types::OrderBook;
use crate::market_data::DailyCandle;
use crate::metrics::ConnectionMetrics;
use crate::session::alert_manager::Alert;
use crate::session::command_router::{CommandInfo, CommandRouter};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Application state for UI components
#[derive(Debug, Clone)]
pub struct AppState {
    pub should_quit: bool,
    pub selected_tab: usize,
    pub symbols: Vec<String>,
    pub market_data: HashMap<String, MarketDataState>,
    pub connection_metrics: ConnectionMetrics,
    pub paused: bool,
    pub log_messages: VecDeque<String>,
    pub log_scroll_offset: usize,
    pub notifications: VecDeque<String>,
    pub command_buffer: String,
    pub input_mode: InputMode,
    pub available_commands: Vec<CommandInfo>,
    pub filtered_commands: Vec<CommandInfo>,
    pub selected_command_index: usize,
    pub alert_form: AlertFormState,
    pub alerts: Vec<Alert>,
    pub selected_alert_index: usize,
}

/// Market data state for a single symbol
#[derive(Debug, Clone)]
pub struct MarketDataState {
    pub symbol: String,
    pub price: f64,
    pub change_percent: f64,
    pub volume_24h: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub orderbook: Option<OrderBook>,
    pub price_history: Vec<PricePoint>,
    pub daily_candles: Vec<DailyCandle>,
    pub kline_render_cache: Option<KlineRenderCache>,
    pub last_kline_refresh: Option<Instant>,
}

/// Historical price sample captured for trend chart
#[derive(Debug, Clone, Copy)]
pub struct PricePoint {
    pub timestamp_ms: u64,
    pub price: f64,
}

/// Cached candle samples prepared for rendering
#[derive(Debug, Clone)]
pub struct KlineRenderCache {
    pub width: u16,
    pub samples: Vec<CandleSample>,
    pub min_price: f64,
    pub max_price: f64,
    pub total_span_ms: u64,
}

/// Simplified candle information used for rendering
#[derive(Debug, Clone)]
pub struct CandleSample {
    pub open_time_ms: u64,
    pub close_time_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub is_closed: bool,
}

/// Input mode for the TUI command palette
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Normal,
    Command,
    AlertPopup,
    Alerts,
}

/// Simple alert form state for popup interaction
#[derive(Debug, Clone, Default)]
pub struct AlertFormState {
    pub symbol: String,
    pub direction_above: bool,
    pub price_input: String,
    pub error: Option<String>,
    pub price_dirty: bool,
}

impl AppState {
    /// Create new application state
    pub fn new(symbols: Vec<String>) -> Self {
        Self {
            should_quit: false,
            selected_tab: 0,
            symbols,
            market_data: HashMap::new(),
            connection_metrics: ConnectionMetrics::default(),
            paused: false,
            log_messages: VecDeque::with_capacity(128),
            log_scroll_offset: 0,
            notifications: VecDeque::with_capacity(64),
            command_buffer: String::new(),
            input_mode: InputMode::Normal,
            available_commands: CommandRouter::commands().to_vec(),
            filtered_commands: CommandRouter::commands().to_vec(),
            selected_command_index: 0,
            alert_form: AlertFormState::default(),
            alerts: Vec::new(),
            selected_alert_index: 0,
        }
    }

    /// Move to next tab
    pub fn next_tab(&mut self) {
        if !self.symbols.is_empty() {
            self.selected_tab = (self.selected_tab + 1) % self.symbols.len();
        }
    }

    /// Move to previous tab
    pub fn previous_tab(&mut self) {
        if !self.symbols.is_empty() {
            self.selected_tab = if self.selected_tab == 0 {
                self.symbols.len() - 1
            } else {
                self.selected_tab - 1
            };
        }
    }

    /// Get currently selected symbol
    pub fn current_symbol(&self) -> Option<&String> {
        self.symbols.get(self.selected_tab)
    }

    /// Toggle pause state
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Update focused symbol based on name
    pub fn focus_symbol(&mut self, symbol: &str) {
        if let Some(idx) = self.symbols.iter().position(|s| s == symbol) {
            self.selected_tab = idx;
        }
    }

    /// Ensure selected tab remains in range after symbol list updates
    pub fn normalize_selected_tab(&mut self) {
        if self.symbols.is_empty() {
            self.selected_tab = 0;
            return;
        }
        if self.selected_tab >= self.symbols.len() {
            self.selected_tab = self.symbols.len() - 1;
        }
    }

    /// Append a log message with bounded history
    pub fn push_log(&mut self, message: impl Into<String>) {
        const MAX_LOGS: usize = 200;
        self.log_messages.push_back(message.into());
        while self.log_messages.len() > MAX_LOGS {
            self.log_messages.pop_front();
        }
        // Clamp scroll offset when new logs arrive to avoid exceeding bounds.
        if self.log_scroll_offset > 0 {
            let max_offset = self
                .log_messages
                .len()
                .saturating_sub(1)
                .min(self.log_scroll_offset);
            self.log_scroll_offset = max_offset;
        }
    }

    /// Append a notification with bounded history
    pub fn push_notification(&mut self, message: impl Into<String>) {
        const MAX_NOTIFICATIONS: usize = 50;
        self.notifications.push_back(message.into());
        while self.notifications.len() > MAX_NOTIFICATIONS {
            self.notifications.pop_front();
        }
    }

    /// Replace alert list snapshot
    pub fn update_alerts(&mut self, alerts: Vec<Alert>) {
        self.alerts = alerts;
        self.normalize_selected_alert();
    }

    /// Enter the alerts list view
    pub fn enter_alerts_view(&mut self) {
        self.input_mode = InputMode::Alerts;
        self.normalize_selected_alert();
    }

    /// Exit the alerts list view back to normal mode
    pub fn exit_alerts_view(&mut self) {
        if matches!(self.input_mode, InputMode::Alerts) {
            self.input_mode = InputMode::Normal;
        }
    }

    /// Clear the command buffer
    pub fn clear_command(&mut self) {
        self.command_buffer.clear();
    }

    /// Enter command mode with optional preset buffer
    pub fn activate_command_mode(&mut self, preset: Option<&str>) {
        self.input_mode = InputMode::Command;
        self.clear_command();
        if let Some(preset_value) = preset {
            self.command_buffer.push_str(preset_value);
        }
        self.reset_command_suggestions();
        self.update_command_suggestions();
    }

    /// Enter alert popup mode using the current symbol and optional preset price
    pub fn activate_alert_popup(&mut self, preset_price: Option<f64>) -> Result<(), String> {
        let symbol = self
            .current_symbol()
            .cloned()
            .ok_or_else(|| "Select a symbol first".to_string())?;

        let price_string = preset_price
            .map(|p| format!("{:.2}", p))
            .unwrap_or_default();

        self.alert_form = AlertFormState {
            symbol,
            direction_above: true,
            price_input: price_string,
            error: None,
            price_dirty: false,
        };
        self.input_mode = InputMode::AlertPopup;
        Ok(())
    }

    /// Exit alert popup mode and clear state
    pub fn deactivate_alert_popup(&mut self) {
        self.alert_form = AlertFormState::default();
        self.input_mode = InputMode::Normal;
    }

    /// Cycle alert direction
    pub fn toggle_alert_direction(&mut self) {
        self.alert_form.direction_above = !self.alert_form.direction_above;
    }

    /// Attempt to parse the alert price from input
    pub fn alert_price(&self) -> Result<f64, String> {
        self.alert_form
            .price_input
            .trim()
            .parse::<f64>()
            .map_err(|_| "Price must be a number".to_string())
    }

    /// Select next alert in the list
    pub fn select_next_alert(&mut self) {
        if self.alerts.is_empty() {
            return;
        }
        let last = self.alerts.len().saturating_sub(1);
        if self.selected_alert_index < last {
            self.selected_alert_index += 1;
        }
    }

    /// Select previous alert in the list
    pub fn select_previous_alert(&mut self) {
        if self.alerts.is_empty() {
            return;
        }
        if self.selected_alert_index > 0 {
            self.selected_alert_index -= 1;
        }
    }

    /// Get selected alert, if any
    pub fn selected_alert(&self) -> Option<&Alert> {
        self.alerts.get(self.selected_alert_index)
    }

    /// Ensure selected alert index remains valid
    fn normalize_selected_alert(&mut self) {
        if self.alerts.is_empty() {
            self.selected_alert_index = 0;
        } else if self.selected_alert_index >= self.alerts.len() {
            self.selected_alert_index = self.alerts.len() - 1;
        }
    }

    /// Reset command suggestions to the full list
    pub fn reset_command_suggestions(&mut self) {
        self.filtered_commands = self.available_commands.clone();
        self.selected_command_index = 0;
    }

    /// Update command suggestions based on the current buffer
    pub fn update_command_suggestions(&mut self) {
        let raw_query = self.command_buffer.trim();
        if raw_query.is_empty() || raw_query == "/" {
            self.reset_command_suggestions();
            return;
        }

        let query = raw_query.trim_start_matches('/').to_ascii_lowercase();
        self.filtered_commands = self
            .available_commands
            .iter()
            .copied()
            .filter(|cmd| {
                cmd.trigger.to_ascii_lowercase().contains(&query)
                    || cmd.usage.to_ascii_lowercase().contains(&query)
                    || cmd.description.to_ascii_lowercase().contains(&query)
            })
            .collect();

        if self.filtered_commands.is_empty() {
            self.selected_command_index = 0;
        } else if self.selected_command_index >= self.filtered_commands.len() {
            self.selected_command_index = self.filtered_commands.len() - 1;
        }
    }

    /// Select next command suggestion
    pub fn select_next_suggestion(&mut self) {
        if self.filtered_commands.is_empty() {
            return;
        }
        let last = self.filtered_commands.len().saturating_sub(1);
        if self.selected_command_index < last {
            self.selected_command_index += 1;
        }
    }

    /// Select previous command suggestion
    pub fn select_previous_suggestion(&mut self) {
        if self.filtered_commands.is_empty() {
            return;
        }
        if self.selected_command_index > 0 {
            self.selected_command_index -= 1;
        }
    }

    /// Apply the selected suggestion to the command buffer
    pub fn apply_selected_suggestion(&mut self) {
        if let Some(cmd) = self.selected_command() {
            self.command_buffer = cmd.trigger.to_string();
            self.reset_command_suggestions();
            self.update_command_suggestions();
        }
    }

    /// Get the currently selected suggestion, if any
    pub fn selected_command(&self) -> Option<&CommandInfo> {
        self.filtered_commands.get(self.selected_command_index)
    }

    /// Scroll logs up (toward older entries)
    pub fn scroll_logs_up(&mut self) {
        if self.log_scroll_offset + 1 < self.log_messages.len() {
            self.log_scroll_offset += 1;
        }
    }

    /// Scroll logs down (toward the newest entries)
    pub fn scroll_logs_down(&mut self) {
        if self.log_scroll_offset > 0 {
            self.log_scroll_offset -= 1;
        }
    }

    /// Reset log scroll to the newest messages
    pub fn reset_log_scroll(&mut self) {
        self.log_scroll_offset = 0;
    }
}

impl MarketDataState {
    /// Invalidate the cached kline render data
    pub fn invalidate_kline_cache(&mut self) {
        self.kline_render_cache = None;
    }

    /// Ensure the render cache is populated for the given width
    pub fn ensure_kline_cache(&mut self, width: u16) -> Option<&KlineRenderCache> {
        if width < 4 {
            self.kline_render_cache = None;
            return None;
        }

        let needs_rebuild = self
            .kline_render_cache
            .as_ref()
            .map_or(true, |cache| cache.width != width);

        if needs_rebuild {
            self.rebuild_kline_cache(width);
        }

        self.kline_render_cache.as_ref()
    }

    fn rebuild_kline_cache(&mut self, width: u16) {
        if width < 4 || self.daily_candles.is_empty() {
            self.kline_render_cache = None;
            return;
        }

        let max_candles = std::cmp::max(1, (width / 2) as usize);
        let len = self.daily_candles.len();
        let take = len.min(max_candles);
        let start = len - take;

        let samples: Vec<CandleSample> = self.daily_candles[start..]
            .iter()
            .map(CandleSample::from)
            .collect();

        self.kline_render_cache = KlineRenderCache::from_samples(width, samples);
    }

    /// Update kline refresh bookkeeping and return whether a redraw is due.
    pub fn update_kline_refresh(&mut self, now: Instant, interval: Duration, force: bool) -> bool {
        if force {
            self.last_kline_refresh = Some(now);
            return true;
        }

        let due = self
            .last_kline_refresh
            .map(|last| now.duration_since(last) >= interval)
            .unwrap_or(true);

        if due {
            self.last_kline_refresh = Some(now);
        }

        due
    }
}

impl KlineRenderCache {
    fn from_samples(width: u16, samples: Vec<CandleSample>) -> Option<KlineRenderCache> {
        if samples.is_empty() {
            return None;
        }

        let mut min_price = f64::INFINITY;
        let mut max_price = f64::NEG_INFINITY;

        for sample in &samples {
            if sample.low < min_price {
                min_price = sample.low;
            }
            if sample.high > max_price {
                max_price = sample.high;
            }
        }

        let first = samples.first().unwrap();
        let last = samples.last().unwrap();
        let total_span_ms = last.close_time_ms.saturating_sub(first.open_time_ms).max(1);

        Some(KlineRenderCache {
            width,
            samples,
            min_price,
            max_price,
            total_span_ms,
        })
    }
}

impl From<&DailyCandle> for CandleSample {
    fn from(value: &DailyCandle) -> Self {
        Self {
            open_time_ms: value.open_time_ms,
            close_time_ms: value.close_time_ms,
            open: value.open,
            high: value.high,
            low: value.low,
            close: value.close,
            is_closed: value.is_closed,
        }
    }
}

impl Default for MarketDataState {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            price: 0.0,
            change_percent: 0.0,
            volume_24h: 0.0,
            high_24h: 0.0,
            low_24h: 0.0,
            orderbook: None,
            price_history: Vec::new(),
            daily_candles: Vec::new(),
            kline_render_cache: None,
            last_kline_refresh: None,
        }
    }
}

/// Display welcome page with consistent formatting
/// This function is shared between SessionManager and UIManager
pub fn display_welcome_page() -> Result<(), std::io::Error> {
    println!();
    println!("┌─ XTrade Market Data Monitor ────────────────────────────────────────┐");
    println!("│                                                                     │");
    println!("│                      * Welcome to XTrade! *                         │");
    println!("│                                                                     │");
    println!("│   A high-performance cryptocurrency market data monitoring system   │");
    println!("│                                                                     │");
    println!("│   Version: {:<50} │", env!("CARGO_PKG_VERSION"));
    println!("│                                                                     │");
    println!("│   Features:                                                         │");
    println!("│   • Real-time Binance market data                                   │");
    println!("│   • OrderBook visualization                                         │");
    println!("│   • Multi-symbol monitoring                                         │");
    println!("│   • Performance metrics tracking                                    │");
    println!("│                                                                     │");
    println!("│   Commands:                                                         │");
    println!("│   • /add <symbols> - Subscribe to symbols                           │");
    println!("│   • /remove <symbols> - Unsubscribe from symbols                    │");
    println!("│   • /list - Show current subscriptions                              │");
    println!("│   • /show <symbol> - Show details for symbol                        │");
    println!("│   • /status - Show session statistics                               │");
    println!("│   • /logs - Show recent logs                                        │");
    println!("│   • /config show - Show configuration                               │");
    println!("│                                                                     │");
    println!("└────────────────────────────────────────────────────────────────────┘");
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_navigation() {
        let mut app = AppState::new(vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
        assert_eq!(app.selected_tab, 0);

        app.next_tab();
        assert_eq!(app.selected_tab, 1);

        app.next_tab();
        assert_eq!(app.selected_tab, 0); // Wrap around

        app.previous_tab();
        assert_eq!(app.selected_tab, 1);
    }

    #[test]
    fn test_toggle_pause() {
        let mut app = AppState::new(vec![]);
        assert!(!app.paused);

        app.toggle_pause();
        assert!(app.paused);

        app.toggle_pause();
        assert!(!app.paused);
    }

    #[test]
    fn kline_cache_limits_samples_by_width() {
        let mut state = MarketDataState::default();
        state.symbol = "TESTUSDT".to_string();

        for idx in 0..20 {
            state.daily_candles.push(DailyCandle::new(
                1_000 + idx * 1_000,
                1_500 + idx * 1_000,
                100.0 + idx as f64,
                105.0 + idx as f64,
                95.0 + idx as f64,
                102.0 + idx as f64,
                10.0,
                true,
            ));
        }

        let cache = state.ensure_kline_cache(12).expect("cache should build");
        // Width 12 allows at most 6 candles (width/2)
        assert_eq!(cache.samples.len(), 6);
        // Expect the cache to use the most recent candles
        assert_eq!(
            cache.samples.first().unwrap().open_time_ms,
            1_000 + 14 * 1_000
        );
        assert_eq!(
            cache.samples.last().unwrap().close_time_ms,
            1_500 + 19 * 1_000
        );
    }

    #[test]
    fn kline_cache_invalidates_on_request() {
        let mut state = MarketDataState::default();
        state.daily_candles.push(DailyCandle::new(
            1_000, 2_000, 100.0, 110.0, 90.0, 105.0, 50.0, true,
        ));

        assert!(state.ensure_kline_cache(10).is_some());
        assert!(state.kline_render_cache.is_some());

        state.invalidate_kline_cache();
        assert!(state.kline_render_cache.is_none());
    }

    #[test]
    fn kline_cache_handles_tiny_width() {
        let mut state = MarketDataState::default();
        state.daily_candles.push(DailyCandle::new(
            1_000, 2_000, 100.0, 110.0, 90.0, 105.0, 50.0, true,
        ));

        assert!(state.ensure_kline_cache(2).is_none());
    }

    #[test]
    fn kline_refresh_throttles_and_forces() {
        let mut state = MarketDataState::default();
        let interval = Duration::from_secs(60);
        let base = Instant::now();

        assert!(state.update_kline_refresh(base, interval, false));
        assert!(!state.update_kline_refresh(base + Duration::from_secs(10), interval, false,));
        assert!(state.update_kline_refresh(base + Duration::from_secs(61), interval, false,));
        assert!(state.update_kline_refresh(base + Duration::from_secs(62), interval, true,));
    }
}
