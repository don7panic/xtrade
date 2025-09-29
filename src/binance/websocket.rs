//! Binance WebSocket client implementation

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::{Mutex, watch};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::protocol::Message,
};
use tracing::{debug, error, info, warn};

use super::types::{
    BinanceEventType, BinanceMessage, ConnectionStatus, OrderBookUpdate, SubscribeRequest,
    Ticker24hr, TradeMessage, UnsubscribeRequest, WebSocketError,
};

/// Binance WebSocket client
pub struct BinanceWebSocket {
    url: String,
    status_tx: watch::Sender<ConnectionStatus>,
    status_rx: watch::Receiver<ConnectionStatus>,
    connection: Arc<Mutex<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    message_tx: mpsc::Sender<Result<BinanceMessage, WebSocketError>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl BinanceWebSocket {
    /// Create a new Binance WebSocket client
    pub fn new(
        url: impl Into<String>,
    ) -> (Self, mpsc::Receiver<Result<BinanceMessage, WebSocketError>>) {
        let (message_tx, message_rx) = mpsc::channel(1000); // Increased capacity for high-frequency data
        let (status_tx, status_rx) = watch::channel(ConnectionStatus::Disconnected);

        let ws = Self {
            url: url.into(),
            status_tx,
            status_rx,
            connection: Arc::new(Mutex::new(None)),
            message_tx,
            shutdown_tx: None,
        };

        (ws, message_rx)
    }

    /// Get current connection status
    pub fn status(&self) -> ConnectionStatus {
        self.status_rx.borrow().clone()
    }

    /// Connect to Binance WebSocket
    pub async fn connect(&self) -> Result<()> {
        self.status_tx.send(ConnectionStatus::Connecting)?;

        match connect_async(&self.url).await {
            Ok((ws_stream, _)) => {
                let mut connection = self.connection.lock().await;
                *connection = Some(ws_stream);
                self.status_tx.send(ConnectionStatus::Connected)?;
                info!("Connected to Binance WebSocket at {}", self.url);
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to connect to WebSocket: {}", e);
                self.status_tx
                    .send(ConnectionStatus::Error(error_msg.clone()))?;
                error!("{}", error_msg);
                Err(anyhow::anyhow!(error_msg))
            }
        }
    }

    /// Disconnect from WebSocket
    pub async fn disconnect(&self) -> Result<()> {
        let mut connection = self.connection.lock().await;
        if let Some(mut ws) = connection.take() {
            if let Err(e) = ws.close(None).await {
                warn!("Error closing WebSocket connection: {}", e);
            }
        }

        self.status_tx.send(ConnectionStatus::Disconnected)?;
        info!("Disconnected from Binance WebSocket");
        Ok(())
    }

    /// Subscribe to a symbol stream
    pub async fn subscribe(&self, symbol: &str, stream_type: &str) -> Result<()> {
        let subscribe_request = SubscribeRequest::new(symbol, stream_type);
        let message = serde_json::to_string(&subscribe_request)?;

        self.send_message(Message::Text(message)).await?;
        info!("Subscribed to {}@{}", symbol, stream_type);
        Ok(())
    }

    /// Unsubscribe from a symbol stream
    pub async fn unsubscribe(&self, symbol: &str, stream_type: &str) -> Result<()> {
        let unsubscribe_request = UnsubscribeRequest::new(symbol, stream_type);
        let message = serde_json::to_string(&unsubscribe_request)?;

        self.send_message(Message::Text(message)).await?;
        info!("Unsubscribed from {}@{}", symbol, stream_type);
        Ok(())
    }

    /// Subscribe to depth stream for a symbol with specified update speed
    /// Binance supports depth streams with different update speeds:
    /// - 1000ms: "depth"
    /// - 100ms: "depth@100ms"
    pub async fn subscribe_depth(&self, symbol: &str, update_speed_ms: Option<u16>) -> Result<()> {
        let stream_type = match update_speed_ms {
            Some(100) => "depth@100ms",
            Some(1000) | None => "depth",
            Some(speed) => {
                return Err(anyhow::anyhow!(
                    "Unsupported depth update speed: {}ms. Supported: 100ms, 1000ms",
                    speed
                ));
            }
        };

        self.subscribe(symbol, stream_type).await?;
        info!(
            "Subscribed to depth stream for {} with {}ms updates",
            symbol,
            update_speed_ms.unwrap_or(1000)
        );
        Ok(())
    }

    /// Subscribe to trade stream for a symbol  
    pub async fn subscribe_trade(&self, symbol: &str) -> Result<()> {
        self.subscribe(symbol, "trade").await?;
        info!("Subscribed to trade stream for {}", symbol);
        Ok(())
    }

    /// Send a message through the WebSocket
    async fn send_message(&self, message: Message) -> Result<()> {
        let mut connection = self.connection.lock().await;
        match connection.as_mut() {
            Some(ws) => {
                ws.send(message).await?;
                Ok(())
            }
            None => Err(anyhow::anyhow!("WebSocket not connected")),
        }
    }

    /// Start listening for incoming messages
    pub async fn start_listening(&mut self) -> Result<()> {
        let connection = self.connection.clone();
        let message_tx = self.message_tx.clone();
        let status_tx = self.status_tx.clone();

        // Create shutdown channel for graceful termination
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        tokio::spawn(async move {
            loop {
                // Take connection for message processing only when needed
                let mut connection_guard = connection.lock().await;
                if let Some(ws_stream) = connection_guard.as_mut() {
                    // Use select! to handle both messages and shutdown
                    tokio::select! {
                        message = ws_stream.next() => {
                            match message {
                                Some(Ok(msg)) => {
                                    debug!("Processing WebSocket message");
                                    match Self::process_message(msg) {
                                        Ok(binance_msg) => {
                                            if let Err(e) = message_tx.send(Ok(binance_msg)).await {
                                                error!("Failed to send message to channel: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            if let Err(e) = message_tx.send(Err(e)).await {
                                                error!("Failed to send error to channel: {}", e);
                                            }
                                        }
                                    }
                                }
                                Some(Err(e)) => {
                                    let error_msg = format!("WebSocket message error: {}", e);
                                    error!("{}", error_msg);
                                    let _ = status_tx.send(ConnectionStatus::Error(error_msg.clone()));
                                    let _ = message_tx.send(Err(WebSocketError::MessageError(error_msg))).await;
                                    // Don't drop the connection on error - let it attempt recovery
                                }
                                None => {
                                    // Connection closed
                                    info!("WebSocket connection closed");
                                    let _ = status_tx.send(ConnectionStatus::Disconnected);
                                    // Connection will be released when guard goes out of scope
                                    break;
                                }
                            }
                            // Connection guard released automatically here
                        }
                        _ = shutdown_rx.recv() => {
                            info!("Received shutdown signal");
                            // Connection guard released automatically here
                            break;
                        }
                    }
                } else {
                    // No connection available, wait and check shutdown
                    drop(connection_guard);
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                        _ = shutdown_rx.recv() => {
                            info!("Received shutdown signal");
                            break;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Classify message based on event type
    fn classify_message(value: &serde_json::Value) -> Option<BinanceEventType> {
        value
            .get("e")
            .and_then(|v| v.as_str())
            .and_then(|event_type| match event_type {
                "depthUpdate" => Some(BinanceEventType::DepthUpdate),
                "trade" => Some(BinanceEventType::Trade),
                "24hrTicker" => Some(BinanceEventType::Ticker24hr),
                "kline" => Some(BinanceEventType::Kline),
                "aggTrade" => Some(BinanceEventType::AggregatedTrade),
                _ => None,
            })
    }

    /// Process incoming WebSocket message
    fn process_message(msg: Message) -> Result<BinanceMessage, WebSocketError> {
        match msg {
            Message::Text(text) => {
                debug!("Received WebSocket message: {}", text);

                // Single parse to JSON value first
                let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
                    WebSocketError::ParseError(format!("Failed to parse JSON: {}", e))
                })?;

                // Classify message based on event type
                match Self::classify_message(&value) {
                    Some(BinanceEventType::DepthUpdate) => {
                        let depth_update: OrderBookUpdate =
                            serde_json::from_value(value).map_err(|e| {
                                WebSocketError::ParseError(format!(
                                    "Failed to parse depth update: {}",
                                    e
                                ))
                            })?;

                        debug!(
                            "Received depth update message: symbol={}, first_id={}, final_id={}, bids={}, asks={}",
                            depth_update.symbol,
                            depth_update.first_update_id,
                            depth_update.final_update_id,
                            depth_update.bids.len(),
                            depth_update.asks.len()
                        );

                        Ok(BinanceMessage {
                            stream: format!("{}@depth", depth_update.symbol.to_lowercase()),
                            data: serde_json::json!(depth_update),
                        })
                    }
                    Some(BinanceEventType::Trade) => {
                        let trade_msg: TradeMessage =
                            serde_json::from_value(value).map_err(|e| {
                                WebSocketError::ParseError(format!(
                                    "Failed to parse trade message: {}",
                                    e
                                ))
                            })?;

                        debug!(
                            "Received trade message: symbol={}, price={}, quantity={}",
                            trade_msg.symbol, trade_msg.price, trade_msg.quantity
                        );

                        Ok(BinanceMessage {
                            stream: format!("{}@trade", trade_msg.symbol.to_lowercase()),
                            data: serde_json::json!(trade_msg),
                        })
                    }
                    Some(BinanceEventType::Ticker24hr) => {
                        let ticker: Ticker24hr = serde_json::from_value(value).map_err(|e| {
                            WebSocketError::ParseError(format!("Failed to parse ticker: {}", e))
                        })?;

                        debug!(
                            "Received 24hr ticker: symbol={}, last_price={}",
                            ticker.symbol, ticker.last_price
                        );

                        Ok(BinanceMessage {
                            stream: format!("{}@ticker", ticker.symbol.to_lowercase()),
                            data: serde_json::json!(ticker),
                        })
                    }
                    Some(BinanceEventType::Kline) => {
                        // Kline messages are not yet fully supported
                        debug!("Received kline message (not fully supported)");
                        Ok(BinanceMessage {
                            stream: "kline".to_string(),
                            data: value,
                        })
                    }
                    Some(BinanceEventType::AggregatedTrade) => {
                        // Aggregated trade messages are not yet fully supported
                        debug!("Received aggregated trade message (not fully supported)");
                        Ok(BinanceMessage {
                            stream: "aggTrade".to_string(),
                            data: value,
                        })
                    }
                    None => {
                        // Fallback to other message types
                        Ok(BinanceMessage {
                            stream: "unknown".to_string(),
                            data: value,
                        })
                    }
                }
            }
            Message::Close(_) => {
                info!("WebSocket connection closed");
                Err(WebSocketError::ConnectionError(
                    "Connection closed".to_string(),
                ))
            }
            Message::Ping(data) => {
                debug!("Received ping, sending pong");
                Ok(BinanceMessage {
                    stream: "ping".to_string(),
                    data: serde_json::json!({ "data": data }),
                })
            }
            Message::Pong(_) => {
                debug!("Received pong");
                Ok(BinanceMessage {
                    stream: "pong".to_string(),
                    data: serde_json::json!({}),
                })
            }
            _ => Err(WebSocketError::ParseError(
                "Unsupported message type".to_string(),
            )),
        }
    }

    /// Gracefully stop listening and cleanup
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            if let Err(e) = shutdown_tx.send(()).await {
                warn!("Failed to send shutdown signal: {}", e);
            }
        }

        self.status_tx.send(ConnectionStatus::Disconnected)?;
        info!("WebSocket client shutdown initiated");
        Ok(())
    }

    /// Reconnect with exponential backoff
    pub async fn reconnect(&self) -> Result<()> {
        self.status_tx.send(ConnectionStatus::Reconnecting)?;

        // Simple retry logic - will be enhanced with backoff crate later
        for attempt in 1..=3 {
            if let Err(e) = self.disconnect().await {
                warn!(
                    "Error disconnecting during reconnect attempt {}: {}",
                    attempt, e
                );
            }

            tokio::time::sleep(Duration::from_secs(attempt * 2)).await;

            if let Ok(()) = self.connect().await {
                info!("Reconnected successfully after {} attempts", attempt);
                return Ok(());
            }
        }

        Err(anyhow::anyhow!("Failed to reconnect after 3 attempts"))
    }

    /// Check if currently connected
    pub fn is_connected(&self) -> bool {
        matches!(self.status(), ConnectionStatus::Connected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    use tokio_test::block_on;

    #[test]
    fn test_websocket_creation() {
        let (ws, _rx) = BinanceWebSocket::new("wss://test.binance.com/ws");
        assert_eq!(ws.status(), ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_status_watch_channel() {
        let (ws, _rx) = BinanceWebSocket::new("wss://test.binance.com/ws");

        block_on(async {
            // Initial status should be Disconnected
            assert_eq!(ws.status(), ConnectionStatus::Disconnected);

            // Simulate connection
            ws.status_tx.send(ConnectionStatus::Connecting).unwrap();
            assert_eq!(ws.status(), ConnectionStatus::Connecting);

            ws.status_tx.send(ConnectionStatus::Connected).unwrap();
            assert_eq!(ws.status(), ConnectionStatus::Connected);
            assert!(ws.is_connected());

            ws.status_tx.send(ConnectionStatus::Disconnected).unwrap();
            assert_eq!(ws.status(), ConnectionStatus::Disconnected);
            assert!(!ws.is_connected());
        });
    }

    #[test]
    fn test_shutdown_method() {
        let (mut ws, _rx) = BinanceWebSocket::new("wss://test.binance.com/ws");

        block_on(async {
            // Start listening to create shutdown channel
            ws.start_listening().await.unwrap();

            // Test shutdown
            ws.shutdown().await.unwrap();
            assert_eq!(ws.status(), ConnectionStatus::Disconnected);

            // Shutdown should be idempotent
            ws.shutdown().await.unwrap();
        });
    }

    #[test]
    fn test_process_message_pong() {
        let msg = Message::Pong(b"test".to_vec());
        let result = BinanceWebSocket::process_message(msg).unwrap();

        assert_eq!(result.stream, "pong");
        assert!(result.data.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_process_message_close() {
        let msg = Message::Close(None);
        let result = BinanceWebSocket::process_message(msg);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WebSocketError::ConnectionError(_)
        ));
    }

    #[test]
    fn test_classify_message_depth_update() {
        let json_data = serde_json::json!({
            "e": "depthUpdate",
            "s": "BTCUSDT",
            "b": [],
            "a": []
        });
        let result = BinanceWebSocket::classify_message(&json_data);
        assert_eq!(result, Some(BinanceEventType::DepthUpdate));
    }

    #[test]
    fn test_classify_message_trade() {
        let json_data = serde_json::json!({
            "e": "trade",
            "s": "BTCUSDT",
            "p": "50000.0"
        });
        let result = BinanceWebSocket::classify_message(&json_data);
        assert_eq!(result, Some(BinanceEventType::Trade));
    }

    #[test]
    fn test_classify_message_ticker_24hr() {
        let json_data = serde_json::json!({
            "e": "24hrTicker",
            "s": "BTCUSDT",
            "c": "50000.0"
        });
        let result = BinanceWebSocket::classify_message(&json_data);
        assert_eq!(result, Some(BinanceEventType::Ticker24hr));
    }

    #[test]
    fn test_classify_message_unknown() {
        let json_data = serde_json::json!({
            "e": "unknownEvent",
            "s": "BTCUSDT"
        });
        let result = BinanceWebSocket::classify_message(&json_data);
        assert_eq!(result, None);
    }

    #[test]
    fn test_classify_message_no_event_type() {
        let json_data = serde_json::json!({
            "method": "SUBSCRIBE",
            "params": ["btcusdt@depth"]
        });
        let result = BinanceWebSocket::classify_message(&json_data);
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_reconnect_logic() {
        let (ws, _rx) = BinanceWebSocket::new("wss://invalid-test-url");

        // Reconnect should fail with invalid URL but complete gracefully
        let result = ws.reconnect().await;
        assert!(result.is_err());

        // Status should reflect reconnection attempts
        assert!(matches!(
            ws.status(),
            ConnectionStatus::Error(_) | ConnectionStatus::Disconnected
        ));
    }
}
