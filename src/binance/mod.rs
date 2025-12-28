//! Binance API integration module
//!
//! Handles WebSocket connections, REST API calls, and data parsing for Binance.

pub mod client_wrapper;
pub mod common;
pub mod perp_usdt;
pub mod spot;

pub use common::types;
pub use common::websocket::BinanceWebSocket;
pub use spot::rest::BinanceRestClient;

// Legacy
pub mod demo;
