//! User Interface module
//!
//! Provides both TUI (Terminal User Interface) and simple CLI output capabilities.

/// TUI application state and rendering
pub mod tui;

/// Simple CLI output functions
pub mod cli;

use crate::binance::types::OrderBook;
use crate::metrics::ConnectionMetrics;
use std::collections::HashMap;

/// Application state for UI components
#[derive(Debug, Clone)]
pub struct AppState {
    pub should_quit: bool,
    pub selected_tab: usize,
    pub symbols: Vec<String>,
    pub market_data: HashMap<String, MarketDataState>,
    pub connection_metrics: ConnectionMetrics,
    pub paused: bool,
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
    pub price_history: Vec<f64>,
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
        }
    }
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
}
