//! User Interface module
//!
//! Provides both TUI (Terminal User Interface) and simple CLI output capabilities.

/// TUI application state and rendering
pub mod tui;

/// Simple CLI output functions
pub mod cli;

/// UI Manager for interactive interface
pub mod ui_manager;

use crate::binance::types::OrderBook;
use crate::metrics::ConnectionMetrics;
use std::collections::{HashMap, VecDeque};

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
    pub notifications: VecDeque<String>,
    pub command_buffer: String,
    pub input_mode: InputMode,
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

/// Input mode for the TUI command palette
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Command,
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
            notifications: VecDeque::with_capacity(64),
            command_buffer: String::new(),
            input_mode: InputMode::Normal,
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
    }

    /// Append a notification with bounded history
    pub fn push_notification(&mut self, message: impl Into<String>) {
        const MAX_NOTIFICATIONS: usize = 50;
        self.notifications.push_back(message.into());
        while self.notifications.len() > MAX_NOTIFICATIONS {
            self.notifications.pop_front();
        }
    }

    /// Clear the command buffer
    pub fn clear_command(&mut self) {
        self.command_buffer.clear();
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

impl Default for InputMode {
    fn default() -> Self {
        InputMode::Normal
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
}
