//! Mock implementations for Binance WebSocket and REST clients
//! Used for testing in CI environments where real network connections are restricted

use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;

use super::types::{
    BinanceMessage, ConnectionStatus, OrderBookUpdate, TradeMessage, WebSocketError,
};

/// Mock WebSocket client for testing
pub struct MockWebSocket {
    url: String,
    status: ConnectionStatus,
    is_connected: AtomicBool,
    message_tx: Option<mpsc::Sender<Result<BinanceMessage, WebSocketError>>>,
}

impl MockWebSocket {
    /// Create a new MockWebSocket
    pub fn new(url: impl Into<String>) -> (Self, mpsc::Receiver<Result<BinanceMessage, WebSocketError>>) {
        let (message_tx, message_rx) = mpsc::channel(100);
        
        let ws = Self {
            url: url.into(),
            status: ConnectionStatus::Disconnected,
            is_connected: AtomicBool::new(false),
            message_tx: Some(message_tx),
        };
        
        (ws, message_rx)
    }

    /// Get current connection status
    pub fn status(&self) -> ConnectionStatus {
        self.status.clone()
    }

    /// Connect to mock WebSocket
    pub async fn connect(&mut self) -> anyhow::Result<()> {
        if self.is_connected.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Already connected"));
        }

        // Simulate connection delay
        sleep(Duration::from_millis(10)).await;
        
        self.is_connected.store(true, Ordering::SeqCst);
        self.status = ConnectionStatus::Connected;
        
        Ok(())
    }

    /// Disconnect from mock WebSocket
    pub async fn disconnect(&mut self) -> anyhow::Result<()> {
        if !self.is_connected.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Not connected"));
        }

        // Simulate disconnection delay
        sleep(Duration::from_millis(10)).await;
        
        self.is_connected.store(false, Ordering::SeqCst);
        self.status = ConnectionStatus::Disconnected;
        
        Ok(())
    }

    /// Subscribe to a symbol stream
    pub async fn subscribe(&self, symbol: &str, stream_type: &str) -> anyhow::Result<()> {
        if !self.is_connected.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Not connected"));
        }

        // Simulate subscription delay
        sleep(Duration::from_millis(5)).await;
        
        Ok(())
    }

    /// Unsubscribe from a symbol stream
    pub async fn unsubscribe(&self, symbol: &str, stream_type: &str) -> anyhow::Result<()> {
        if !self.is_connected.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Not connected"));
        }

        // Simulate unsubscription delay
        sleep(Duration::from_millis(5)).await;
        
        Ok(())
    }

    /// Subscribe to depth stream
    pub async fn subscribe_depth(&self, symbol: &str, _update_speed_ms: Option<u16>) -> anyhow::Result<()> {
        self.subscribe(symbol, "depth").await
    }

    /// Subscribe to trade stream
    pub async fn subscribe_trade(&self, symbol: &str) -> anyhow::Result<()> {
        self.subscribe(symbol, "trade").await
    }

    /// Start listening for messages
    pub async fn start_listening(&mut self) -> anyhow::Result<()> {
        if !self.is_connected.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Not connected"));
        }

        // Start mock message generation
        let message_tx = self.message_tx.take().expect("Message channel already taken");
        let symbol = "BTCUSDT".to_string();
        
        tokio::spawn(async move {
            let mut count = 0;
            let start_time = Instant::now();
            
            while start_time.elapsed() < Duration::from_secs(5) {
                count += 1;
                
                // Generate mock messages
                if count % 2 == 0 {
                    // Mock depth update
                    let depth_update = OrderBookUpdate {
                        event_type: "depthUpdate".to_string(),
                        event_time: count as u64,
                        symbol: symbol.clone(),
                        first_update_id: count as u64,
                        final_update_id: count as u64 + 1,
                        bids: vec![["50000.0".to_string(), "1.0".to_string()]],
                        asks: vec![["50100.0".to_string(), "1.0".to_string()]],
                    };
                    
                    let message = BinanceMessage {
                        stream: format!("{}@depth", symbol.to_lowercase()),
                        data: serde_json::json!(depth_update),
                    };
                    
                    if let Err(_) = message_tx.send(Ok(message)).await {
                        break;
                    }
                } else {
                    // Mock trade message
                    let trade_msg = TradeMessage {
                        event_type: "trade".to_string(),
                        event_time: count as u64,
                        symbol: symbol.clone(),
                        trade_id: count as u64,
                        price: "50050.0".to_string(),
                        quantity: "0.5".to_string(),
                        trade_time: count as u64,
                        is_buyer_maker: count % 3 == 0,
                    };
                    
                    let message = BinanceMessage {
                        stream: format!("{}@trade", symbol.to_lowercase()),
                        data: serde_json::json!(trade_msg),
                    };
                    
                    if let Err(_) = message_tx.send(Ok(message)).await {
                        break;
                    }
                }
                
                sleep(Duration::from_millis(100)).await;
            }
        });
        
        Ok(())
    }

    /// Shutdown the mock WebSocket
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.disconnect().await
    }

    /// Reconnect the mock WebSocket
    pub async fn reconnect(&mut self) -> anyhow::Result<()> {
        self.disconnect().await?;
        sleep(Duration::from_millis(50)).await;
        self.connect().await
    }

    /// Check if currently connected
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }
}

/// Mock REST client for testing
pub struct MockRestClient {
    base_url: String,
}

impl MockRestClient {
    /// Create a new MockRestClient
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    /// Mock API request
    pub async fn get(&self, _endpoint: &str) -> anyhow::Result<String> {
        // Simulate API delay
        sleep(Duration::from_millis(10)).await;
        
        // Return mock response
        Ok(r#"{"lastUpdateId": 123456, "bids": [["50000.0", "1.0"]], "asks": [["50100.0", "1.0"]]}"#.to_string())
    }

    /// Mock orderbook snapshot request
    pub async fn get_orderbook_snapshot(&self, symbol: &str) -> anyhow::Result<String> {
        self.get(&format!("/api/v3/depth?symbol={}&limit=1000", symbol)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_mock_websocket_creation() {
        let (mut ws, _rx) = MockWebSocket::new("ws://localhost:8080/mock");
        assert_eq!(ws.status(), ConnectionStatus::Disconnected);
        assert!(!ws.is_connected());
    }

    #[tokio::test]
    async fn test_mock_websocket_connect_disconnect() {
        let (mut ws, _rx) = MockWebSocket::new("ws://localhost:8080/mock");
        
        // Test connect
        ws.connect().await.unwrap();
        assert_eq!(ws.status(), ConnectionStatus::Connected);
        assert!(ws.is_connected());
        
        // Test disconnect
        ws.disconnect().await.unwrap();
        assert_eq!(ws.status(), ConnectionStatus::Disconnected);
        assert!(!ws.is_connected());
    }

    #[tokio::test]
    async fn test_mock_websocket_subscribe() {
        let (mut ws, _rx) = MockWebSocket::new("ws://localhost:8080/mock");
        
        ws.connect().await.unwrap();
        
        // Test subscribe
        ws.subscribe("BTCUSDT", "depth").await.unwrap();
        ws.subscribe_depth("BTCUSDT", Some(100)).await.unwrap();
        ws.subscribe_trade("BTCUSDT").await.unwrap();
        
        // Test unsubscribe
        ws.unsubscribe("BTCUSDT", "depth").await.unwrap();
    }

    #[tokio::test]
    async fn test_mock_websocket_reconnect() {
        let (mut ws, _rx) = MockWebSocket::new("ws://localhost:8080/mock");
        
        ws.connect().await.unwrap();
        assert!(ws.is_connected());
        
        ws.reconnect().await.unwrap();
        assert!(ws.is_connected());
    }

    #[tokio::test]
    async fn test_mock_rest_client() {
        let client = MockRestClient::new("http://localhost:8080/mock");
        
        let response = client.get_orderbook_snapshot("BTCUSDT").await.unwrap();
        assert!(response.contains("lastUpdateId"));
        assert!(response.contains("bids"));
        assert!(response.contains("asks"));
    }

    #[tokio::test]
    async fn test_mock_websocket_message_generation() {
        let (mut ws, mut rx) = MockWebSocket::new("ws://localhost:8080/mock");
        
        ws.connect().await.unwrap();
        ws.start_listening().await.unwrap();
        
        // Receive a few mock messages
        for _ in 0..3 {
            if let Some(message_result) = rx.recv().await {
                assert!(message_result.is_ok());
                let message = message_result.unwrap();
                assert!(message.stream.contains("btcusdt"));
            }
        }
    }
}