//! Symbol subscription management module

use anyhow::{Result, anyhow};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::{ControlMessage, MarketEvent, MarketStream};
use crate::binance::BinanceWebSocket;
use crate::binance::client_wrapper::RestClientWrapper;
use crate::binance::types::{
    BinanceMessage, ErrorSeverity, ForceOrder, FundingRateUpdate, KlineStreamEvent,
    MarkPriceUpdate, MarketKey, MarketType, OpenInterestStream, OrderBook, OrderBookError,
    Ticker24hr, WebSocketError,
};
use crate::market_data::{DEFAULT_DAILY_CANDLE_LIMIT, DailyCandle};
use std::collections::HashSet;

/// Symbol subscription manager for individual trading pairs
pub struct SymbolSubscription {
    symbol: String,
    market_type: MarketType,
    orderbook: OrderBook,
    daily_candles: Vec<DailyCandle>,
    daily_candle_limit: usize,
    enabled_streams: HashSet<MarketStream>,
    control_rx: mpsc::UnboundedReceiver<ControlMessage>,
    event_tx: mpsc::UnboundedSender<MarketEvent>,
    ws: BinanceWebSocket,
    message_rx: mpsc::Receiver<Result<BinanceMessage, crate::binance::types::WebSocketError>>,
    rest_client: RestClientWrapper,
    key: MarketKey,
}

impl SymbolSubscription {
    /// Create a new SymbolSubscription
    pub async fn new(
        market_type: MarketType,
        symbol: String,
        ws_url: String,
        rest_url: String,
        streams: HashSet<MarketStream>,
        control_rx: mpsc::UnboundedReceiver<ControlMessage>,
        event_tx: mpsc::UnboundedSender<MarketEvent>,
    ) -> Result<Self> {
        info!(
            "Creating symbol subscription for: {} ({:?})",
            symbol, market_type
        );

        // Create WebSocket connection
        let (ws, message_rx) = BinanceWebSocket::new(ws_url);

        let rest_client = match market_type {
            MarketType::Spot => RestClientWrapper::new_spot(rest_url),
            MarketType::PerpUsdt => RestClientWrapper::new_perp_usdt(rest_url),
        };

        // Create orderbook
        let orderbook = OrderBook::new(symbol.clone());

        Ok(Self {
            symbol: symbol.clone(),
            market_type,
            orderbook,
            daily_candles: Vec::new(),
            daily_candle_limit: DEFAULT_DAILY_CANDLE_LIMIT,
            enabled_streams: streams,
            control_rx,
            event_tx,
            ws,
            message_rx,
            rest_client,
            key: MarketKey {
                symbol: symbol.clone(),
                market_type,
            },
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
                key: self.key.clone(),
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
            key: self.key.clone(),
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
        if self.enabled_streams.contains(&MarketStream::Depth) {
            if let Err(e) = self.ws.subscribe_depth(&self.symbol, Some(100)).await {
                error!(
                    "Failed to subscribe to depth stream for {}: {}",
                    self.symbol, e
                );
                return Err(e);
            }
        }

        // Subscribe to trade stream for latency/price metrics
        if self.enabled_streams.contains(&MarketStream::Trade) {
            if let Err(e) = self.ws.subscribe_trade(&self.symbol).await {
                error!(
                    "Failed to subscribe to trade stream for {}: {}",
                    self.symbol, e
                );
                return Err(e);
            }
        }

        // Subscribe to 24hr ticker stream for 24h stats updates
        if self.enabled_streams.contains(&MarketStream::Ticker) {
            if let Err(e) = self.ws.subscribe_ticker(&self.symbol).await {
                error!(
                    "Failed to subscribe to ticker stream for {}: {}",
                    self.symbol, e
                );
                return Err(e);
            }
        }

        // Subscribe to daily kline stream
        if self.enabled_streams.contains(&MarketStream::Kline1d) {
            if let Err(e) = self.ws.subscribe_kline(&self.symbol, "1d").await {
                error!(
                    "Failed to subscribe to kline stream for {}: {}",
                    self.symbol, e
                );
                return Err(e);
            }
        }

        // Subscribe to Perp-specific streams
        if self.market_type == MarketType::PerpUsdt {
            // Subscribe to Mark Price (1s for better responsiveness)
            if self.enabled_streams.contains(&MarketStream::MarkPrice) {
                if let Err(e) = self.ws.subscribe_mark_price(&self.symbol, true).await {
                    error!(
                        "Failed to subscribe to mark price stream for {}: {}",
                        self.symbol, e
                    );
                    return Err(e);
                }
            }

            // Subscribe to Force Order (Liquidation)
            if self.enabled_streams.contains(&MarketStream::ForceOrder) {
                if let Err(e) = self.ws.subscribe_force_order(&self.symbol).await {
                    error!(
                        "Failed to subscribe to force order stream for {}: {}",
                        self.symbol, e
                    );
                    return Err(e);
                }
            }

            // Subscribe to Open Interest
            if self.enabled_streams.contains(&MarketStream::OpenInterest) {
                if let Err(e) = self.ws.subscribe_open_interest(&self.symbol).await {
                    error!(
                        "Failed to subscribe to open interest stream for {}: {}",
                        self.symbol, e
                    );
                    return Err(e);
                }
            }

            // Subscribe to Funding Rate
            if self.enabled_streams.contains(&MarketStream::FundingRate) {
                if let Err(e) = self.ws.subscribe_funding_rate(&self.symbol).await {
                    error!(
                        "Failed to subscribe to funding rate stream for {}: {}",
                        self.symbol, e
                    );
                    return Err(e);
                }
            }
        }

        // Preload historical daily candles
        if self.enabled_streams.contains(&MarketStream::Kline1d) {
            if let Err(e) = self.load_initial_daily_candles().await {
                warn!("Failed to preload daily candles for {}: {}", self.symbol, e);

                if let Err(send_err) = self.event_tx.send(MarketEvent::Error {
                    key: self.key.clone(),
                    error: format!("Failed to preload daily candles: {}", e),
                }) {
                    error!(
                        "Failed to send preload error event for {}: {}",
                        self.symbol, send_err
                    );
                }
            }
        }

        // Fetch initial snapshot
        if self.enabled_streams.contains(&MarketStream::Depth) {
            match self
                .rest_client
                .get_depth_snapshot(&self.symbol, None)
                .await
            {
                Ok(snapshot) => {
                    if let Err(e) = self.orderbook.update_from_snapshot(snapshot) {
                        error!(
                            "Failed to update orderbook from snapshot for {}: {}",
                            self.symbol, e
                        );
                        return Err(e);
                    }
                    info!("Successfully fetched snapshot for {}", self.symbol);

                    // Send initial orderbook state
                    if let Err(e) = self.event_tx.send(MarketEvent::OrderBookUpdate {
                        key: self.key.clone(),
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
        }

        info!("Successfully initialized subscription for: {}", self.symbol);

        // Send subscription active event
        if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
            key: self.key.clone(),
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
            key: self.key.clone(),
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
                            let is_closed = matches!(&e, WebSocketError::ConnectionError(msg)
                                if msg.contains("Connection closed"))
                                || matches!(&e, WebSocketError::MessageError(msg)
                                    if msg.contains("Connection closed"));

                            if is_closed {
                                warn!("WebSocket connection closed for {}", self.symbol);
                                let _ = self.event_tx.send(MarketEvent::ConnectionStatus {
                                    key: self.key.clone(),
                                    status: crate::binance::types::ConnectionStatus::Disconnected,
                                });
                            } else {
                                error!("WebSocket error for {}: {}", self.symbol, e);
                                if let Err(send_err) = self.event_tx.send(MarketEvent::Error {
                                    key: self.key.clone(),
                                    error: e.to_string(),
                                }) {
                                    error!(
                                        "Failed to send error event for {}: {}",
                                        self.symbol, send_err
                                    );
                                }
                            }

                            // Automatic reconnection for connection-level errors
                            if Self::requires_reconnection(&e) {
                                warn!(
                                    "Connection-level error detected, triggering automatic reconnection for {}",
                                    self.symbol
                                );
                                if let Err(reconnect_err) = self.reconnect().await {
                                    error!(
                                        "Automatic reconnection failed for {}: {}",
                                        self.symbol, reconnect_err
                                    );
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
                                key: self.key.clone(),
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
                            key: self.key.clone(),
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
            stream if stream.contains("ticker") => {
                match serde_json::from_value::<Ticker24hr>(binance_msg.data) {
                    Ok(ticker) => match Self::parse_ticker_stats(&ticker) {
                        Ok(stats) => {
                            if let Err(e) = self.event_tx.send(MarketEvent::TickerUpdate {
                                key: self.key.clone(),
                                last_price: stats.last_price,
                                price_change_percent: stats.price_change_percent,
                                high_price: stats.high_price,
                                low_price: stats.low_price,
                                volume: stats.volume,
                            }) {
                                error!("Failed to send ticker update for {}: {}", self.symbol, e);
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Failed to convert ticker payload for {}: {}",
                                self.symbol, e
                            );
                        }
                    },
                    Err(e) => {
                        error!("Failed to parse ticker event for {}: {}", self.symbol, e);
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
            stream if stream.contains("markPrice") => {
                match serde_json::from_value::<MarkPriceUpdate>(binance_msg.data) {
                    Ok(update) => {
                        // Parse values
                        // Note: MarkPriceUpdate fields are Strings, need parsing.
                        // But for now let's just emit the event if parsing succeeds.
                        // Or utilize helper to parse?
                        // struct MarkPriceUpdate has string fields.
                        // MarketEvent needs f64.
                        if let (Ok(mark_price), Ok(index_price), Ok(funding_rate)) = (
                            update.mark_price.parse::<f64>(),
                            update.index_price.parse::<f64>(),
                            update.funding_rate.parse::<f64>(),
                        ) {
                            if let Err(e) = self.event_tx.send(MarketEvent::MarkPriceUpdate {
                                key: self.key.clone(),
                                mark_price,
                                index_price,
                                funding_rate,
                                next_funding_time: update.next_funding_time,
                                event_time: update.event_time,
                            }) {
                                error!(
                                    "Failed to send mark price update for {}: {}",
                                    self.symbol, e
                                );
                            }
                        } else {
                            warn!("Failed to parse mark price values for {}", self.symbol);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse mark price event for {}: {}",
                            self.symbol, e
                        );
                    }
                }
            }
            stream if stream.contains("forceOrder") => {
                match serde_json::from_value::<ForceOrder>(binance_msg.data) {
                    Ok(force_order) => {
                        let order = force_order.order;
                        if let (Ok(price), Ok(quantity)) = (
                            order.price.parse::<f64>(),
                            order.original_quantity.parse::<f64>(),
                        ) {
                            if let Err(e) = self.event_tx.send(MarketEvent::Liquidation {
                                key: self.key.clone(),
                                side: order.side,
                                price,
                                quantity,
                                event_time: force_order.event_time,
                            }) {
                                error!(
                                    "Failed to send liquidation event for {}: {}",
                                    self.symbol, e
                                );
                            }
                        } else {
                            warn!("Failed to parse force order values for {}", self.symbol);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse force order event for {}: {}",
                            self.symbol, e
                        );
                    }
                }
            }
            stream if stream.contains("openInterest") => {
                match serde_json::from_value::<OpenInterestStream>(binance_msg.data) {
                    Ok(update) => {
                        if let Ok(open_interest) = update.open_interest.parse::<f64>() {
                            if let Err(e) = self.event_tx.send(MarketEvent::OpenInterestUpdate {
                                key: self.key.clone(),
                                open_interest,
                                event_time: update.time,
                            }) {
                                error!(
                                    "Failed to send open interest update for {}: {}",
                                    self.symbol, e
                                );
                            }
                        } else {
                            warn!("Failed to parse open interest value for {}", self.symbol);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse open interest event for {}: {}",
                            self.symbol, e
                        );
                    }
                }
            }
            stream if stream.contains("fundingRate") => {
                match serde_json::from_value::<FundingRateUpdate>(binance_msg.data) {
                    Ok(update) => {
                        if let Ok(rate) = update.funding_rate.parse::<f64>() {
                            if let Err(e) = self.event_tx.send(MarketEvent::FundingRateUpdate {
                                key: self.key.clone(),
                                funding_rate: rate,
                                funding_time: update.funding_time,
                                event_time: update.event_time,
                            }) {
                                error!(
                                    "Failed to send funding rate update for {}: {}",
                                    self.symbol, e
                                );
                            }
                        } else {
                            warn!("Failed to parse funding rate for {}", self.symbol);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse funding rate event for {}: {}",
                            self.symbol, e
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
                    key: self.key.clone(),
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

    fn parse_ticker_stats(ticker: &Ticker24hr) -> Result<ParsedTickerStats> {
        Ok(ParsedTickerStats {
            last_price: Self::parse_f64_str(&ticker.last_price, "last_price")?,
            price_change_percent: Self::parse_f64_str(
                &ticker.price_change_percent,
                "price_change_percent",
            )?,
            high_price: Self::parse_f64_str(&ticker.high_price, "high_price")?,
            low_price: Self::parse_f64_str(&ticker.low_price, "low_price")?,
            volume: Self::parse_f64_str(&ticker.volume, "volume")?,
        })
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
                    key: self.key.clone(),
                    error: err.to_string(),
                }) {
                    error!(
                        "Failed to forward orderbook error event for {}: {}",
                        self.symbol, send_err
                    );
                }
            }
        }

        if err.requires_resync() && self.enabled_streams.contains(&MarketStream::Depth) {
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
        if !self.enabled_streams.contains(&MarketStream::Depth) {
            return Ok(());
        }

        let snapshot = self
            .rest_client
            .get_depth_snapshot(&self.symbol, None)
            .await?;
        self.orderbook.update_from_snapshot(snapshot)?;

        if let Err(e) = self.event_tx.send(MarketEvent::OrderBookUpdate {
            key: self.key.clone(),
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
            key: self.key.clone(),
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
                key: self.key.clone(),
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

        if let Err(e) = self.ws.start_listening().await {
            error!(
                "Failed to restart listener after reconnect for {}: {}",
                self.symbol, e
            );
            return Err(e);
        }

        if self.enabled_streams.contains(&MarketStream::Depth) {
            if let Err(e) = self.resync_orderbook().await {
                error!(
                    "Failed to refresh orderbook after reconnect for {}: {}",
                    self.symbol, e
                );
            }
        }

        // Send connection reestablished event
        if let Err(e) = self.event_tx.send(MarketEvent::ConnectionStatus {
            key: self.key.clone(),
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
            key: self.key.clone(),
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
            key: self.key.clone(),
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

struct ParsedTickerStats {
    last_price: f64,
    price_change_percent: f64,
    high_price: f64,
    low_price: f64,
    volume: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binance::types::KlineData;
    use serde_json::json;
    use std::collections::HashSet;
    use tokio::sync::mpsc;
    use tokio::time::{Duration, timeout};

    async fn new_perp_subscription() -> (SymbolSubscription, mpsc::UnboundedReceiver<MarketEvent>) {
        let (_control_tx, control_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let subscription = SymbolSubscription::new(
            MarketType::PerpUsdt,
            "BTCUSDT".to_string(),
            "wss://example.com".to_string(),
            "https://example.com".to_string(),
            HashSet::new(),
            control_rx,
            event_tx,
        )
        .await
        .expect("subscription should be created");

        (subscription, event_rx)
    }

    async fn next_event(rx: &mut mpsc::UnboundedReceiver<MarketEvent>) -> MarketEvent {
        timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timed out waiting for market event")
            .expect("market event channel closed")
    }

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

    #[tokio::test]
    async fn mark_price_stream_emits_mark_price_event() {
        let (mut subscription, mut event_rx) = new_perp_subscription().await;
        let message = BinanceMessage {
            stream: "btcusdt@markPrice".to_string(),
            data: json!({
                "e": "markPriceUpdate",
                "E": 1_700_000,
                "s": "BTCUSDT",
                "p": "40000.5",
                "i": "39950.2",
                "P": "0",
                "r": "0.00025",
                "T": 1_700_600
            }),
        };

        subscription.process_websocket_message(message).await;

        match next_event(&mut event_rx).await {
            MarketEvent::MarkPriceUpdate {
                key,
                mark_price,
                index_price,
                funding_rate,
                next_funding_time,
                event_time,
            } => {
                assert_eq!(key.market_type, MarketType::PerpUsdt);
                assert_eq!(key.symbol, "BTCUSDT");
                assert!((mark_price - 40000.5).abs() < 1e-9);
                assert!((index_price - 39950.2).abs() < 1e-9);
                assert!((funding_rate - 0.00025).abs() < 1e-9);
                assert_eq!(next_funding_time, 1_700_600);
                assert_eq!(event_time, 1_700_000);
            }
            other => panic!("unexpected market event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn funding_rate_stream_emits_funding_rate_event() {
        let (mut subscription, mut event_rx) = new_perp_subscription().await;
        let message = BinanceMessage {
            stream: "btcusdt@fundingRate".to_string(),
            data: json!({
                "e": "fundingRate",
                "E": 1_700_010,
                "s": "BTCUSDT",
                "r": "0.00015",
                "T": 1_700_800
            }),
        };

        subscription.process_websocket_message(message).await;

        match next_event(&mut event_rx).await {
            MarketEvent::FundingRateUpdate {
                key,
                funding_rate,
                funding_time,
                event_time,
            } => {
                assert_eq!(key.market_type, MarketType::PerpUsdt);
                assert_eq!(key.symbol, "BTCUSDT");
                assert!((funding_rate - 0.00015).abs() < 1e-9);
                assert_eq!(funding_time, 1_700_800);
                assert_eq!(event_time, 1_700_010);
            }
            other => panic!("unexpected market event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn open_interest_stream_emits_open_interest_event() {
        let (mut subscription, mut event_rx) = new_perp_subscription().await;
        let message = BinanceMessage {
            stream: "btcusdt@openInterest".to_string(),
            data: json!({
                "openInterest": "123456.7",
                "symbol": "BTCUSDT",
                "time": 1_700_020
            }),
        };

        subscription.process_websocket_message(message).await;

        match next_event(&mut event_rx).await {
            MarketEvent::OpenInterestUpdate {
                key,
                open_interest,
                event_time,
            } => {
                assert_eq!(key.market_type, MarketType::PerpUsdt);
                assert_eq!(key.symbol, "BTCUSDT");
                assert!((open_interest - 123456.7).abs() < 1e-9);
                assert_eq!(event_time, 1_700_020);
            }
            other => panic!("unexpected market event: {:?}", other),
        }
    }
}
