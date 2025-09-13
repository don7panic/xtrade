//! Binance API integration module
//!
//! Handles WebSocket connections, REST API calls, and data parsing for Binance.

pub mod rest;
pub mod types;
pub mod websocket;

// Re-export commonly used types
pub use rest::BinanceRestClient;
pub use types::*;
pub use websocket::BinanceWebSocket;
