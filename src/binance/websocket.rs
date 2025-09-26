//! Binance WebSocket client implementation

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::protocol::Message,
};
use tracing::{debug, error, info, warn};

use super::types::{
    BinanceMessage, BinanceResponse, ConnectionStatus, OrderBookUpdate, SubscribeRequest,
    TradeMessage, UnsubscribeRequest, WebSocketError,
};

/// Binance WebSocket client
pub struct BinanceWebSocket {
    url: String,
    status: Arc<Mutex<ConnectionStatus>>,
    connection: Arc<Mutex<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    message_tx: mpsc::Sender<Result<BinanceMessage, WebSocketError>>,
}

impl BinanceWebSocket {
    /// Create a new Binance WebSocket client
    pub fn new(
        url: impl Into<String>,
    ) -> (Self, mpsc::Receiver<Result<BinanceMessage, WebSocketError>>) {
        let (message_tx, message_rx) = mpsc::channel(1000); // Increased capacity for high-frequency data

        let ws = Self {
            url: url.into(),
            status: Arc::new(Mutex::new(ConnectionStatus::Disconnected)),
            connection: Arc::new(Mutex::new(None)),
            message_tx,
        };

        (ws, message_rx)
    }

    /// Get current connection status
    pub async fn status(&self) -> ConnectionStatus {
        let status = self.status.lock().await;
        status.clone()
    }

    /// Connect to Binance WebSocket
    pub async fn connect(&self) -> Result<()> {
        self.update_status(ConnectionStatus::Connecting).await;

        match connect_async(&self.url).await {
            Ok((ws_stream, _)) => {
                let mut connection = self.connection.lock().await;
                *connection = Some(ws_stream);
                self.update_status(ConnectionStatus::Connected).await;
                info!("Connected to Binance WebSocket at {}", self.url);
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to connect to WebSocket: {}", e);
                self.update_status(ConnectionStatus::Error(error_msg.clone()))
                    .await;
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

        self.update_status(ConnectionStatus::Disconnected).await;
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
    pub async fn start_listening(&self) -> Result<()> {
        let connection = self.connection.clone();
        let message_tx = self.message_tx.clone();
        let status = self.status.clone();

        tokio::spawn(async move {
            loop {
                // Get connection temporarily for each message
                let mut connection = connection.lock().await;
                if let Some(ws) = connection.as_mut() {
                    // Process messages without dropping connection while using ws
                    if let Some(message) = ws.next().await {
                        debug!("Raw WebSocket message received");
                        match message {
                            Ok(msg) => {
                                debug!("Processing WebSocket message");
                                match Self::process_message(msg) {
                                    Ok(binance_msg) => {
                                        debug!("Sending processed message to channel");
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
                            Err(e) => {
                                let error_msg = format!("WebSocket message error: {}", e);
                                error!("{}", error_msg);
                                let mut status = status.lock().await;
                                *status = ConnectionStatus::Error(error_msg.clone());

                                if let Err(e) = message_tx
                                    .send(Err(WebSocketError::MessageError(error_msg)))
                                    .await
                                {
                                    error!("Failed to send error to channel: {}", e);
                                }
                            }
                        }
                    } else {
                        // Connection closed
                        break;
                    }
                } else {
                    // No connection, wait briefly and retry
                    drop(connection);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        });

        Ok(())
    }

    /// Process incoming WebSocket message
    fn process_message(msg: Message) -> Result<BinanceMessage, WebSocketError> {
        match msg {
            Message::Text(text) => {
                debug!("Received WebSocket message: {}", text);

                // Try to parse as depth update message first (most common for orderbook)
                match serde_json::from_str::<OrderBookUpdate>(&text) {
                    Ok(depth_update) => {
                        debug!(
                            "Received depth update message: symbol={}, first_id={}, final_id={}, bids={}, asks={}",
                            depth_update.symbol,
                            depth_update.first_update_id,
                            depth_update.final_update_id,
                            depth_update.bids.len(),
                            depth_update.asks.len()
                        );
                        // Create a BinanceMessage for depth events
                        Ok(BinanceMessage {
                            stream: format!("{}@depth", depth_update.symbol.to_lowercase()),
                            data: serde_json::json!(depth_update),
                        })
                    }
                    Err(e) => {
                        debug!("Not a depth update message: {}", e);
                        // Try to parse as trade message
                        match serde_json::from_str::<TradeMessage>(&text) {
                            Ok(trade_msg) => {
                                debug!(
                                    "Received trade message: symbol={}, price={}, quantity={}",
                                    trade_msg.symbol, trade_msg.price, trade_msg.quantity
                                );
                                // Create a BinanceMessage for trade events
                                Ok(BinanceMessage {
                                    stream: format!("{}@trade", trade_msg.symbol.to_lowercase()),
                                    data: serde_json::json!(trade_msg),
                                })
                            }
                            Err(e) => {
                                debug!("Not a trade message: {}", e);
                                // Try to parse as response message
                                match serde_json::from_str::<BinanceResponse>(&text) {
                                    Ok(response) => {
                                        debug!("Received response: {:?}", response);
                                        if response.error.is_some() {
                                            return Err(WebSocketError::SubscriptionError(
                                                format!("Subscription error: {:?}", response.error),
                                            ));
                                        }
                                        // Create a generic BinanceMessage for responses
                                        Ok(BinanceMessage {
                                            stream: "response".to_string(),
                                            data: serde_json::json!({ "result": response.result }),
                                        })
                                    }
                                    Err(e) => {
                                        debug!("Not a response message: {}", e);
                                        // Try to parse as generic Binance message last
                                        match serde_json::from_str::<BinanceMessage>(&text) {
                                            Ok(binance_msg) => {
                                                debug!(
                                                    "Received generic Binance message: stream={}",
                                                    binance_msg.stream
                                                );
                                                Ok(binance_msg)
                                            }
                                            Err(e) => {
                                                warn!("Failed to parse message: {} - {}", text, e);
                                                Err(WebSocketError::ParseError(format!(
                                                    "Failed to parse message: {} - {}",
                                                    text, e
                                                )))
                                            }
                                        }
                                    }
                                }
                            }
                        }
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
                // We'll handle pong responses in the connection loop
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

    /// Update connection status
    async fn update_status(&self, new_status: ConnectionStatus) {
        let mut status = self.status.lock().await;
        *status = new_status;
    }

    /// Reconnect with exponential backoff
    pub async fn reconnect(&self) -> Result<()> {
        self.update_status(ConnectionStatus::Reconnecting).await;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test::block_on;

    #[test]
    fn test_websocket_creation() {
        let (ws, _rx) = BinanceWebSocket::new("wss://test.binance.com/ws");
        block_on(async {
            let status = ws.status().await;
            assert_eq!(status, ConnectionStatus::Disconnected);
        });
    }

    #[test]
    fn test_subscribe_request() {
        let request = SubscribeRequest::new("BTCUSDT", "depth");
        assert_eq!(request.method, "SUBSCRIBE");
        assert_eq!(request.params, vec!["btcusdt@depth"]);
        assert_eq!(request.id, 1);
    }

    #[test]
    fn test_unsubscribe_request() {
        let request = UnsubscribeRequest::new("ETHUSDT", "trade");
        assert_eq!(request.method, "UNSUBSCRIBE");
        assert_eq!(request.params, vec!["ethusdt@trade"]);
        assert_eq!(request.id, 1);
    }
}
