//! Binance WebSocket client implementation

use super::types::ConnectionStatus;
// use anyhow::Result; // Will be used in Day 5-6

/// Binance WebSocket client
#[allow(dead_code)] // Will be implemented in Day 5-6
pub struct BinanceWebSocket {
    url: String,
    status: ConnectionStatus,
}

impl BinanceWebSocket {
    pub fn new(url: String) -> Self {
        Self {
            url,
            status: ConnectionStatus::Disconnected,
        }
    }

    pub fn status(&self) -> &ConnectionStatus {
        &self.status
    }
}

// Placeholder implementation - will be expanded in Day 5-6
