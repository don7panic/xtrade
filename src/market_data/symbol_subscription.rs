//! Symbol subscription management module

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::{ControlMessage, MarketEvent};
use crate::binance::types::{BinanceMessage, OrderBook};
use crate::binance::{BinanceRestClient, BinanceWebSocket};

/// Symbol subscription manager for individual trading pairs
pub struct SymbolSubscription {
    symbol: String,
    orderbook: OrderBook,
    control_rx: mpsc::UnboundedReceiver<ControlMessage>,
    event_tx: mpsc::UnboundedSender<MarketEvent>,
    ws: BinanceWebSocket,
    message_rx: mpsc::Receiver<Result<BinanceMessage, crate::binance::types::WebSocketError>>,
}

impl SymbolSubscription {
    /// Create a new SymbolSubscription
    pub async fn new(
        symbol: String,
        control_rx: mpsc::UnboundedReceiver<ControlMessage>,
        event_tx: mpsc::UnboundedSender<MarketEvent>,
    ) -> Result<Self> {
        info!("Creating symbol subscription for: {}", symbol);

        // Create WebSocket connection
        let ws_url = "wss://stream.binance.com:9443/ws".to_string();
        let (ws, message_rx) = BinanceWebSocket::new(ws_url);

        // Create orderbook
        let orderbook = OrderBook::new(symbol.clone());

        Ok(Self {
            symbol,
            orderbook,
            control_rx,
            event_tx,
            ws,
            message_rx,
        })
    }

    /// Initialize the subscription (connect and subscribe)
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing subscription for: {}", self.symbol);

        // Connect to WebSocket
        if let Err(e) = self.ws.connect().await {
            error!("Failed to connect WebSocket for {}: {}", self.symbol, e);
            return Err(e);
        }

        // Start listening for messages
        if let Err(e) = self.ws.start_listening().await {
            error!("Failed to start listening for {}: {}", self.symbol, e);
            return Err(e);
        }

        // Subscribe to depth stream
        if let Err(e) = self.ws.subscribe_depth(&self.symbol, Some(100)).await {
            error!(
                "Failed to subscribe to depth stream for {}: {}",
                self.symbol, e
            );
            return Err(e);
        }

        // Fetch initial snapshot
        match self
            .orderbook
            .fetch_snapshot(&BinanceRestClient::new(
                "https://api.binance.com".to_string(),
            ))
            .await
        {
            Ok(_) => {
                info!("Successfully fetched snapshot for {}", self.symbol);

                // Send initial orderbook state
                if let Err(e) = self.event_tx.send(MarketEvent::OrderBookUpdate {
                    symbol: self.symbol.clone(),
                    orderbook: self.orderbook.clone(),
                }) {
                    error!(
                        "Failed to send initial orderbook update for {}: {}",
                        self.symbol, e
                    );
                }
            }
            Err(e) => {
                error!("Failed to fetch snapshot for {}: {}", self.symbol, e);
                return Err(e);
            }
        }

        info!("Successfully initialized subscription for: {}", self.symbol);
        Ok(())
    }

    /// Run the subscription main loop
    pub async fn run(mut self) {
        info!("Starting subscription loop for: {}", self.symbol);

        // Main message processing loop
        loop {
            tokio::select! {
                // Handle control messages
                Some(control_msg) = self.control_rx.recv() => {
                    match control_msg {
                        ControlMessage::Shutdown => {
                            info!("Received shutdown signal for {}", self.symbol);
                            break;
                        }
                        ControlMessage::Reconnect => {
                            info!("Received reconnect signal for {}", self.symbol);
                            if let Err(e) = self.reconnect().await {
                                error!("Failed to reconnect for {}: {}", self.symbol, e);
                            }
                        }
                        ControlMessage::UpdateConfig => {
                            debug!("Received config update for {}", self.symbol);
                        }
                    }
                }

                // Handle WebSocket messages
                Some(message_result) = self.message_rx.recv() => {
                    match message_result {
                        Ok(binance_msg) => {
                            self.process_websocket_message(binance_msg).await;
                        }
                        Err(e) => {
                            error!("WebSocket error for {}: {}", self.symbol, e);

                            // Send error event
                            if let Err(e) = self.event_tx.send(MarketEvent::Error {
                                symbol: self.symbol.clone(),
                                error: e.to_string(),
                            }) {
                                error!("Failed to send error event for {}: {}", self.symbol, e);
                            }

                            // Automatic reconnection for connection-level errors
                            if Self::requires_reconnection(&e) {
                                warn!("Connection-level error detected, triggering automatic reconnection for {}", self.symbol);
                                if let Err(reconnect_err) = self.reconnect().await {
                                    error!("Automatic reconnection failed for {}: {}", self.symbol, reconnect_err);
                                }
                            }
                        }
                    }
                }
            }
        }

        info!("Subscription loop terminated for: {}", self.symbol);
    }

    /// Process WebSocket message and update orderbook
    async fn process_websocket_message(&mut self, binance_msg: BinanceMessage) {
        match binance_msg.stream.as_str() {
            stream if stream.contains("depth") => {
                // Parse depth update
                if let Ok(depth_update) = serde_json::from_value::<
                    crate::binance::types::OrderBookUpdate,
                >(binance_msg.data)
                {
                    if let Err(e) = self.orderbook.apply_depth_update(depth_update) {
                        error!("Failed to apply depth update for {}: {}", self.symbol, e);
                    } else {
                        // Send updated orderbook
                        if let Err(e) = self.event_tx.send(MarketEvent::OrderBookUpdate {
                            symbol: self.symbol.clone(),
                            orderbook: self.orderbook.clone(),
                        }) {
                            error!("Failed to send orderbook update for {}: {}", self.symbol, e);
                        }
                    }
                }
            }
            stream if stream.contains("trade") => {
                // Parse trade message
                if let Ok(trade_msg) =
                    serde_json::from_value::<crate::binance::types::TradeMessage>(binance_msg.data)
                {
                    // Send price update
                    if let Ok(price) = trade_msg.price.parse::<f64>() {
                        if let Err(e) = self.event_tx.send(MarketEvent::PriceUpdate {
                            symbol: self.symbol.clone(),
                            price,
                            time: trade_msg.event_time,
                        }) {
                            error!("Failed to send price update for {}: {}", self.symbol, e);
                        }
                    } else {
                        error!(
                            "Failed to parse price for {}: {}",
                            self.symbol, trade_msg.price
                        );
                    }
                }
            }
            _ => {
                debug!(
                    "Unhandled message type for {}: {}",
                    self.symbol, binance_msg.stream
                );
            }
        }
    }

    /// Reconnect the WebSocket connection
    async fn reconnect(&mut self) -> Result<()> {
        info!("Reconnecting WebSocket for: {}", self.symbol);

        // Disconnect first
        if let Err(e) = self.ws.disconnect().await {
            warn!(
                "Error during disconnect before reconnect for {}: {}",
                self.symbol, e
            );
        }

        // Reconnect
        if let Err(e) = self.ws.connect().await {
            error!("Failed to reconnect WebSocket for {}: {}", self.symbol, e);
            return Err(e);
        }

        // Resubscribe
        if let Err(e) = self.ws.subscribe_depth(&self.symbol, Some(100)).await {
            error!(
                "Failed to resubscribe to depth stream for {}: {}",
                self.symbol, e
            );
            return Err(e);
        }

        // Fetch fresh snapshot
        match self
            .orderbook
            .fetch_snapshot(&BinanceRestClient::new(
                "https://api.binance.com".to_string(),
            ))
            .await
        {
            Ok(_) => {
                info!(
                    "Successfully reconnected and fetched fresh snapshot for {}",
                    self.symbol
                );

                // Send updated orderbook
                if let Err(e) = self.event_tx.send(MarketEvent::OrderBookUpdate {
                    symbol: self.symbol.clone(),
                    orderbook: self.orderbook.clone(),
                }) {
                    error!(
                        "Failed to send orderbook update after reconnect for {}: {}",
                        self.symbol, e
                    );
                }
            }
            Err(e) => {
                error!(
                    "Failed to fetch snapshot after reconnect for {}: {}",
                    self.symbol, e
                );
                return Err(e);
            }
        }

        info!("Successfully reconnected for: {}", self.symbol);
        Ok(())
    }

    /// Get the symbol
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Get the current orderbook
    pub fn orderbook(&self) -> &OrderBook {
        &self.orderbook
    }

    /// Shutdown the subscription gracefully
    pub async fn shutdown(self) -> Result<()> {
        info!("Shutting down subscription for: {}", self.symbol);

        // Disconnect WebSocket
        if let Err(e) = self.ws.disconnect().await {
            warn!(
                "Error during WebSocket disconnect for {}: {}",
                self.symbol, e
            );
        }

        info!("Successfully shut down subscription for: {}", self.symbol);
        Ok(())
    }

    /// Determine if an error requires reconnection
    fn requires_reconnection(error: &crate::binance::types::WebSocketError) -> bool {
        use crate::binance::types::WebSocketError;

        match error {
            WebSocketError::ConnectionError(_) => true,
            WebSocketError::IoError(_) => true,
            WebSocketError::MessageError(_) => true,
            WebSocketError::SubscriptionError(_) => true,
            WebSocketError::ParseError(_) => false, // Parsing errors don't require reconnection
            WebSocketError::JsonError(_) => false,  // JSON errors don't require reconnection
        }
    }
}
