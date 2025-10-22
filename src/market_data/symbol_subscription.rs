//! Symbol subscription management module

use anyhow::{Result, anyhow};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::{ControlMessage, MarketEvent};
use crate::binance::types::{
    BinanceMessage, ErrorSeverity, KlineStreamEvent, OrderBook, OrderBookError,
};
use crate::binance::{BinanceRestClient, BinanceWebSocket};
use crate::market_data::{DEFAULT_DAILY_CANDLE_LIMIT, DailyCandle};

/// Symbol subscription manager for individual trading pairs
pub struct SymbolSubscription {
    symbol: String,
    orderbook: OrderBook,
    daily_candles: Vec<DailyCandle>,
    daily_candle_limit: usize,
    control_rx: mpsc::UnboundedReceiver<ControlMessage>,
    event_tx: mpsc::UnboundedSender<MarketEvent>,
    ws: BinanceWebSocket,
    message_rx: mpsc::Receiver<Result<BinanceMessage, crate::binance::types::WebSocketError>>,
    rest_client: BinanceRestClient,
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
        let rest_client = BinanceRestClient::new("https://api.binance.com".to_string());

        // Create orderbook
        let orderbook = OrderBook::new(symbol.clone());

        Ok(Self {
            symbol,
            orderbook,
            daily_candles: Vec::new(),
            daily_candle_limit: DEFAULT_DAILY_CANDLE_LIMIT,
            control_rx,
            event_tx,
            ws,
            message_rx,
            rest_client,
        })
    }

    /// Initialize the subscription (connect and subscribe)
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing subscription for: {}", self.symbol);

        // Connect to WebSocket
        if let Err(e) = self.ws.connect().await {
            error!("Failed to connect WebSocket for {}: {}", self.symbol, e);

            // Send connection error event
            if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
                symbol: self.symbol.clone(),
                status: crate::binance::types::ConnectionStatus::Disconnected,
            }) {
                error!(
                    "Failed to send connection status event for {}: {}",
                    self.symbol, e
                );
            }

            return Err(e);
        }

        // Send connection established event
        if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
            symbol: self.symbol.clone(),
            status: crate::binance::types::ConnectionStatus::Connected,
        }) {
            error!(
                "Failed to send connection status event for {}: {}",
                self.symbol, e
            );
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

        // Subscribe to trade stream for latency/price metrics
        if let Err(e) = self.ws.subscribe_trade(&self.symbol).await {
            error!(
                "Failed to subscribe to trade stream for {}: {}",
                self.symbol, e
            );
            return Err(e);
        }

        // Subscribe to daily kline stream
        if let Err(e) = self.ws.subscribe_kline(&self.symbol, "1d").await {
            error!(
                "Failed to subscribe to kline stream for {}: {}",
                self.symbol, e
            );
            return Err(e);
        }

        // Preload historical daily candles
        if let Err(e) = self.load_initial_daily_candles().await {
            warn!("Failed to preload daily candles for {}: {}", self.symbol, e);

            if let Err(send_err) = self.event_tx.send(MarketEvent::Error {
                symbol: self.symbol.clone(),
                error: format!("Failed to preload daily candles: {}", e),
            }) {
                error!(
                    "Failed to send preload error event for {}: {}",
                    self.symbol, send_err
                );
            }
        }

        // Fetch initial snapshot
        match self.orderbook.fetch_snapshot(&self.rest_client).await {
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

        // Send subscription active event
        if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
            symbol: self.symbol.clone(),
            status: crate::binance::types::ConnectionStatus::Connected,
        }) {
            error!(
                "Failed to send connection status event for {}: {}",
                self.symbol, e
            );
        }

        Ok(())
    }

    async fn load_initial_daily_candles(&mut self) -> Result<()> {
        let limit = self.daily_candle_limit as u16;
        let candles = self
            .rest_client
            .get_daily_klines(&self.symbol, Some(limit))
            .await?;

        if candles.is_empty() {
            return Err(anyhow!("no candles returned for {}", self.symbol));
        }

        self.daily_candles = candles;

        if let Err(e) = self.event_tx.send(MarketEvent::DailyCandleUpdate {
            symbol: self.symbol.clone(),
            candles: self.daily_candles.clone(),
            is_snapshot: true,
        }) {
            error!(
                "Failed to send daily candle snapshot for {}: {}",
                self.symbol, e
            );
        }

        info!(
            "Preloaded {} daily candles for {}",
            self.daily_candles.len(),
            self.symbol
        );

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
                    match self.orderbook.apply_depth_update(depth_update) {
                        Ok(_) => {
                            if let Err(e) = self.event_tx.send(MarketEvent::OrderBookUpdate {
                                symbol: self.symbol.clone(),
                                orderbook: self.orderbook.clone(),
                            }) {
                                error!(
                                    "Failed to send orderbook update for {}: {}",
                                    self.symbol, e
                                );
                            }
                        }
                        Err(e) => {
                            self.handle_orderbook_error(e).await;
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
            stream if stream.contains("kline") => {
                match serde_json::from_value::<KlineStreamEvent>(binance_msg.data) {
                    Ok(kline_event) => {
                        self.handle_kline_event(kline_event).await;
                    }
                    Err(e) => {
                        error!("Failed to parse kline event for {}: {}", self.symbol, e);
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

    async fn handle_kline_event(&mut self, event: KlineStreamEvent) {
        if event.kline.interval != "1d" {
            debug!(
                "Ignoring non-daily kline interval {} for {}",
                event.kline.interval, self.symbol
            );
            return;
        }

        match Self::build_daily_candle(&event) {
            Ok(candle) => {
                let updated = self.upsert_daily_candle(candle);

                if let Err(e) = self.event_tx.send(MarketEvent::DailyCandleUpdate {
                    symbol: self.symbol.clone(),
                    candles: vec![updated.clone()],
                    is_snapshot: false,
                }) {
                    error!(
                        "Failed to send daily candle update for {}: {}",
                        self.symbol, e
                    );
                }
            }
            Err(e) => {
                warn!(
                    "Failed to convert kline event into candle for {}: {}",
                    self.symbol, e
                );
            }
        }
    }

    fn build_daily_candle(event: &KlineStreamEvent) -> Result<DailyCandle> {
        let kline = &event.kline;
        let open = Self::parse_f64_str(&kline.open, "open")?;
        let high = Self::parse_f64_str(&kline.high, "high")?;
        let low = Self::parse_f64_str(&kline.low, "low")?;
        let close = Self::parse_f64_str(&kline.close, "close")?;
        let volume = Self::parse_f64_str(&kline.volume, "volume")?;

        Ok(DailyCandle::new(
            kline.start_time,
            kline.close_time,
            open,
            high,
            low,
            close,
            volume,
            kline.is_final,
        ))
    }

    fn upsert_daily_candle(&mut self, candle: DailyCandle) -> DailyCandle {
        if let Some(existing) = self
            .daily_candles
            .iter_mut()
            .find(|existing| existing.open_time_ms == candle.open_time_ms)
        {
            *existing = candle;
            return existing.clone();
        }

        self.daily_candles.push(candle.clone());

        if self.daily_candles.len() > self.daily_candle_limit {
            let overflow = self.daily_candles.len() - self.daily_candle_limit;
            self.daily_candles.drain(0..overflow);
        }

        candle
    }

    fn parse_f64_str(value: &str, field: &str) -> Result<f64> {
        value
            .parse::<f64>()
            .map_err(|e| anyhow!("failed to parse {} value '{}': {}", field, value, e))
    }

    async fn handle_orderbook_error(&mut self, err: OrderBookError) {
        let severity = err.severity();

        match severity {
            ErrorSeverity::Info => {
                debug!(
                    "Orderbook update issue for {} considered informational: {}",
                    self.symbol, err
                );
            }
            ErrorSeverity::Warning => {
                warn!("Orderbook update warning for {}: {}", self.symbol, err);
            }
            ErrorSeverity::Error | ErrorSeverity::Critical => {
                error!("Orderbook update error for {}: {}", self.symbol, err);

                if let Err(send_err) = self.event_tx.send(MarketEvent::Error {
                    symbol: self.symbol.clone(),
                    error: err.to_string(),
                }) {
                    error!(
                        "Failed to forward orderbook error event for {}: {}",
                        self.symbol, send_err
                    );
                }
            }
        }

        if err.requires_resync() {
            warn!(
                "Orderbook for {} requires resync due to: {}. Fetching fresh snapshot.",
                self.symbol, err
            );
            if let Err(resync_err) = self.resync_orderbook().await {
                error!(
                    "Failed to resync orderbook for {}: {}",
                    self.symbol, resync_err
                );
            }
        }
    }

    async fn resync_orderbook(&mut self) -> Result<()> {
        self.orderbook.fetch_snapshot(&self.rest_client).await?;

        if let Err(e) = self.event_tx.send(MarketEvent::OrderBookUpdate {
            symbol: self.symbol.clone(),
            orderbook: self.orderbook.clone(),
        }) {
            error!(
                "Failed to broadcast resynced orderbook for {}: {}",
                self.symbol, e
            );
        }

        Ok(())
    }

    /// Reconnect the WebSocket connection
    async fn reconnect(&mut self) -> Result<()> {
        info!("Reconnecting WebSocket for: {}", self.symbol);

        // Send reconnecting status
        if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
            symbol: self.symbol.clone(),
            status: crate::binance::types::ConnectionStatus::Reconnecting,
        }) {
            error!(
                "Failed to send reconnection status event for {}: {}",
                self.symbol, e
            );
        }

        if let Err(e) = self.ws.reconnect().await {
            error!("Failed to reconnect WebSocket for {}: {}", self.symbol, e);

            // Send connection failed event
            if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
                symbol: self.symbol.clone(),
                status: crate::binance::types::ConnectionStatus::Error(
                    "Reconnection failed".to_string(),
                ),
            }) {
                error!(
                    "Failed to send connection failed event for {}: {}",
                    self.symbol, e
                );
            }

            return Err(e);
        }

        if let Err(e) = self.resync_orderbook().await {
            error!(
                "Failed to refresh orderbook after reconnect for {}: {}",
                self.symbol, e
            );
        }

        // Send connection reestablished event
        if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
            symbol: self.symbol.clone(),
            status: crate::binance::types::ConnectionStatus::Connected,
        }) {
            error!(
                "Failed to send connection status event for {}: {}",
                self.symbol, e
            );
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

        // Send disconnecting status
        if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
            symbol: self.symbol.clone(),
            status: crate::binance::types::ConnectionStatus::Disconnected,
        }) {
            error!(
                "Failed to send disconnecting status event for {}: {}",
                self.symbol, e
            );
        }

        // Disconnect WebSocket
        if let Err(e) = self.ws.disconnect().await {
            warn!(
                "Error during WebSocket disconnect for {}: {}",
                self.symbol, e
            );
        }

        // Send disconnected status
        if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
            symbol: self.symbol.clone(),
            status: crate::binance::types::ConnectionStatus::Disconnected,
        }) {
            error!(
                "Failed to send disconnected status event for {}: {}",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binance::types::KlineData;

    fn sample_kline_event(is_final: bool) -> KlineStreamEvent {
        KlineStreamEvent {
            event_type: "kline".to_string(),
            event_time: 1,
            symbol: "TESTUSDT".to_string(),
            kline: KlineData {
                start_time: 1,
                close_time: 2,
                symbol: "TESTUSDT".to_string(),
                interval: "1d".to_string(),
                first_trade_id: 10,
                last_trade_id: 20,
                open: "100.0".to_string(),
                close: "110.0".to_string(),
                high: "115.0".to_string(),
                low: "95.0".to_string(),
                volume: "123.45".to_string(),
                number_of_trades: 42,
                is_final,
                quote_volume: "0".to_string(),
                taker_buy_base_volume: "0".to_string(),
                taker_buy_quote_volume: "0".to_string(),
                ignore: "0".to_string(),
            },
        }
    }

    #[test]
    fn build_daily_candle_converts_kline_values() {
        let event = sample_kline_event(false);
        let candle = SymbolSubscription::build_daily_candle(&event).expect("should parse");

        assert_eq!(candle.open_time_ms, 1);
        assert_eq!(candle.close_time_ms, 2);
        assert!((candle.open - 100.0).abs() < 1e-9);
        assert!((candle.close - 110.0).abs() < 1e-9);
        assert!((candle.high - 115.0).abs() < 1e-9);
        assert!((candle.low - 95.0).abs() < 1e-9);
        assert!((candle.volume - 123.45).abs() < 1e-9);
        assert!(!candle.is_closed);
    }

    #[test]
    fn build_daily_candle_respects_final_flag() {
        let event = sample_kline_event(true);
        let candle = SymbolSubscription::build_daily_candle(&event).expect("should parse");
        assert!(candle.is_closed);
    }

    #[test]
    fn build_daily_candle_errors_on_bad_numbers() {
        let mut event = sample_kline_event(false);
        event.kline.open = "bad".to_string();
        let result = SymbolSubscription::build_daily_candle(&event);
        assert!(result.is_err());
    }
}
