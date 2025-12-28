//! Binance WebSocket 演示功能的模块化实现
//!
//! 这个模块展示了如何正确地重构复杂的 WebSocket 消息处理逻辑，避免过度的 match 嵌套。
//! 主要设计原则：
//!
//! 1. **单一职责原则**：每个结构体和函数只负责一个特定的功能
//! 2. **错误处理分离**：将错误处理逻辑从业务逻辑中分离出来
//! 3. **组合而非继承**：使用组合模式将不同功能模块组合在一起
//! 4. **可测试性**：每个组件都可以独立测试
//! 5. **可扩展性**：新功能可以轻松地添加新模块而不影响现有代码
//!
//! ## 架构组件
//!
//! - [`MessageProcessor`]: 处理 WebSocket 消息的核心逻辑
//! - [`OrderBookManager`][]: 管理订单簿的初始化和状态
//! - [`MetricsCollector`][]: 收集和展示性能指标
//! - [`WebSocketManager`]: 管理 WebSocket 连接的生命周期
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use xtrade::binance::demo::demo_websocket;
//! use xtrade::AppResult;
//!
//! #[tokio::main]
//! async fn main() -> AppResult<()> {
//!     demo_websocket().await?;
//!     Ok(())
//! }
//! ```

use anyhow::Result;
use std::time::{Duration, Instant};

use super::BinanceWebSocket;
use super::types::{BinanceMessage, OrderBook, OrderBookError, OrderBookUpdate, TradeMessage};
use crate::AppResult;
use crate::binance::BinanceRestClient;

/// 消息类型分类枚举
///
/// 用于根据消息流标识符对消息进行分类，
/// 避免使用嵌套的 if 判断。
#[derive(Debug, PartialEq)]
pub enum MessageType {
    /// 深度更新消息
    DepthUpdate,
    /// 交易消息
    Trade,
    /// 24小时行情消息
    Ticker24hr,
    /// 其他消息类型
    Other,
}

impl MessageType {
    /// 根据消息流标识符判断消息类型
    pub fn from_stream(stream: &str) -> Self {
        if stream.contains("@depth") {
            Self::DepthUpdate
        } else if stream.contains("@trade") {
            Self::Trade
        } else if stream.contains("@ticker") {
            Self::Ticker24hr
        } else {
            Self::Other
        }
    }
}

/// WebSocket 消息处理器
///
/// 负责处理从 WebSocket 接收到的消息，包括：
/// - 消息解析和验证
/// - 深度更新处理
/// - 错误处理和恢复
/// - 统计信息收集
///
/// # 设计理念
///
/// 这个处理器采用了"告知不要询问"（Tell Don't Ask）的设计模式，
/// 将复杂的消息处理逻辑封装在内部，对外提供简洁的接口。
///
/// # 错误处理策略
///
/// - **可恢复错误**：记录并继续处理
/// - **严重错误**：触发重新同步机制
/// - **解析错误**：跳过当前消息，继续处理下一个
pub struct MessageProcessor {
    /// 处理的消息总数
    message_count: u64,
    /// 成功处理的深度更新数量
    update_count: u64,
    /// 成功处理的交易消息数量
    trade_count: u64,
    /// 遇到的错误总数
    error_count: u64,
    /// 累计交易量
    trade_volume: f64,
    /// 最后交易价格
    last_trade_price: Option<f64>,
}

impl Default for MessageProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageProcessor {
    /// 创建新的消息处理器实例
    pub fn new() -> Self {
        Self {
            message_count: 0,
            update_count: 0,
            trade_count: 0,
            error_count: 0,
            trade_volume: 0.0,
            last_trade_price: None,
        }
    }

    /// 处理接收到的 WebSocket 消息
    ///
    /// # 参数
    ///
    /// - `message_result`: WebSocket 消息或错误
    /// - `orderbook`: 要更新的订单簿引用
    /// - `rest_client`: REST 客户端，用于重新同步
    ///
    /// # 返回值
    ///
    /// 返回 `Ok(true)` 表示应该继续处理消息，`Ok(false)` 表示应该停止。
    /// 错误情况返回 `Err`。
    ///
    /// # 错误处理
    ///
    /// 这个方法实现了多层错误处理：
    /// 1. WebSocket 传输错误
    /// 2. JSON 解析错误  
    /// 3. 订单簿更新错误
    /// 4. 一致性验证错误
    pub async fn process_message(
        &mut self,
        message_result: Result<BinanceMessage, super::types::WebSocketError>,
        orderbook: &mut OrderBook,
        rest_client: &BinanceRestClient,
    ) -> Result<bool> {
        self.message_count += 1;

        // 使用 Result 组合子简化错误处理
        let message = message_result.map_err(|error| {
            self.error_count += 1;
            if self.error_count <= 3 {
                println!("❌ Error receiving message: {}", error);
            }
            error
        })?;

        // 根据消息类型进行不同的处理
        match MessageType::from_stream(&message.stream) {
            MessageType::DepthUpdate => {
                // 处理深度更新消息
                match serde_json::from_value::<OrderBookUpdate>(message.data) {
                    Ok(depth_update) => {
                        self.handle_depth_update(depth_update, orderbook, rest_client)
                            .await
                    }
                    Err(e) => {
                        if self.error_count <= 3 {
                            println!("❌ Failed to parse depth update: {}", e);
                        }
                        self.error_count += 1;
                        Ok(true) // 解析错误时继续处理
                    }
                }
            }
            MessageType::Trade => {
                // 处理交易消息
                match serde_json::from_value::<TradeMessage>(message.data) {
                    Ok(trade_msg) => self.handle_trade_message(trade_msg).await,
                    Err(e) => {
                        if self.error_count <= 3 {
                            println!("❌ Failed to parse trade message: {}", e);
                        }
                        self.error_count += 1;
                        Ok(true) // 解析错误时继续处理
                    }
                }
            }
            MessageType::Ticker24hr => {
                // 处理 ticker 消息
                if self.message_count <= 3 {
                    println!("📊 Ticker message: {}", message.stream);
                }
                Ok(true)
            }
            MessageType::Other => {
                // 处理其他消息
                if self.message_count <= 3 {
                    println!("📨 Other message: {}", message.stream);
                }
                Ok(true)
            }
        }
    }

    /// 处理深度更新
    async fn handle_depth_update(
        &mut self,
        depth_update: OrderBookUpdate,
        orderbook: &mut OrderBook,
        rest_client: &BinanceRestClient,
    ) -> Result<bool> {
        self.update_count += 1;

        match orderbook.apply_depth_update(depth_update) {
            Ok(()) => {
                self.log_successful_update(orderbook);
                self.validate_consistency_periodically(orderbook).await?;
                Ok(true)
            }
            Err(e) => self.handle_orderbook_error(e, orderbook, rest_client).await,
        }
    }

    /// 处理交易消息
    async fn handle_trade_message(&mut self, trade_msg: TradeMessage) -> Result<bool> {
        self.trade_count += 1;

        // 解析价格和数量
        let price = trade_msg.price.parse::<f64>().unwrap_or(0.0);
        let quantity = trade_msg.quantity.parse::<f64>().unwrap_or(0.0);

        self.last_trade_price = Some(price);
        self.trade_volume += quantity;

        // 选择性日志记录
        if self.trade_count <= 5 || self.trade_count % 10 == 0 {
            println!(
                "💰 Trade #{}: {} {} @ {}, maker: {}",
                self.trade_count, trade_msg.symbol, quantity, price, trade_msg.is_buyer_maker
            );
        }

        Ok(true)
    }

    /// 记录成功的更新
    fn log_successful_update(&self, orderbook: &OrderBook) {
        if self.update_count <= 5 || self.update_count % 10 == 0 {
            println!(
                "✅ Update #{}: bid={:?}, ask={:?}, spread={:?}, levels={}",
                self.update_count,
                orderbook.best_bid(),
                orderbook.best_ask(),
                orderbook.spread(),
                orderbook.total_levels()
            );
        }
    }

    /// 定期验证一致性
    async fn validate_consistency_periodically(&mut self, orderbook: &OrderBook) -> Result<()> {
        if self.update_count % 10 == 0 {
            if let Err(e) = orderbook.validate_consistency() {
                println!("⚠️  Consistency check failed: {}", e);
                self.error_count += 1;
            }
        }
        Ok(())
    }

    /// 处理订单簿错误
    async fn handle_orderbook_error(
        &mut self,
        error: OrderBookError,
        orderbook: &mut OrderBook,
        rest_client: &BinanceRestClient,
    ) -> Result<bool> {
        self.error_count += 1;

        match &error {
            OrderBookError::StaleMessage { .. } => {
                if self.error_count <= 3 {
                    println!("ℹ️  Stale message (expected): {}", error);
                }
            }
            _ => {
                self.log_critical_error(&error);
                if error.requires_resync() {
                    self.resync_orderbook(orderbook, rest_client).await?;
                }
            }
        }

        Ok(true)
    }

    /// 记录严重错误
    fn log_critical_error(&self, error: &OrderBookError) {
        println!("❌ OrderBook update error: {}", error);
        println!("   Severity: {:?}", error.severity());
        println!("   Recoverable: {}", error.is_recoverable());
        println!("   Requires resync: {}", error.requires_resync());
    }

    /// 重新同步订单簿
    async fn resync_orderbook(
        &self,
        orderbook: &mut OrderBook,
        rest_client: &BinanceRestClient,
    ) -> Result<()> {
        println!("🔄 Re-fetching snapshot due to error...");

        match orderbook.fetch_snapshot(rest_client).await {
            Ok(()) => {
                println!("✅ Snapshot re-fetched successfully");
            }
            Err(snapshot_err) => {
                println!("❌ Failed to re-fetch snapshot: {}", snapshot_err);
            }
        }

        Ok(())
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> MessageStats {
        MessageStats {
            message_count: self.message_count,
            update_count: self.update_count,
            trade_count: self.trade_count,
            error_count: self.error_count,
            total_trade_volume: self.trade_volume,
            last_trade_price: self.last_trade_price,
        }
    }
}

/// 消息处理统计信息
///
/// 包含了消息处理过程中的各种统计数据，用于性能监控和调试。
#[derive(Debug, Clone)]
pub struct MessageStats {
    /// 接收到的消息总数（包括错误消息）
    pub message_count: u64,
    /// 成功处理的深度更新数量
    pub update_count: u64,
    /// 成功处理的交易消息数量
    pub trade_count: u64,
    /// 遇到的错误总数
    pub error_count: u64,
    /// 累计交易量
    pub total_trade_volume: f64,
    /// 最后交易价格
    pub last_trade_price: Option<f64>,
}

/// 订单簿管理器
///
/// 负责订单簿的生命周期管理，包括：
/// - 初始化快照获取
/// - 状态日志记录
/// - 订单簿封装和访问控制
///
/// # 设计考虑
///
/// 这个管理器将订单簿的初始化逻辑从主流程中分离出来，
/// 使得代码更加模块化和可测试。
pub struct OrderBookManager {
    /// 被管理的订单簿实例
    pub orderbook: OrderBook,
}

impl OrderBookManager {
    pub fn new(symbol: String) -> Self {
        Self {
            orderbook: OrderBook::new(symbol),
        }
    }

    /// 初始化订单簿快照
    pub async fn initialize(&mut self, rest_client: &BinanceRestClient) -> Result<()> {
        println!(
            "📊 Fetching initial OrderBook snapshot for {}...",
            self.orderbook.symbol
        );

        self.orderbook.fetch_snapshot(rest_client).await?;

        println!("✅ OrderBook snapshot fetched successfully!");
        self.log_initial_state();

        Ok(())
    }

    /// 记录初始状态
    fn log_initial_state(&self) {
        println!("   📈 Best bid: {:?}", self.orderbook.best_bid());
        println!("   📉 Best ask: {:?}", self.orderbook.best_ask());
        println!("   📏 Spread: {:?}", self.orderbook.spread());
        println!(
            "   🏗️  Levels: bids={}, asks={}",
            self.orderbook.bids.len(),
            self.orderbook.asks.len()
        );
        println!("   🔢 Last update ID: {}", self.orderbook.last_update_id);
    }
}

/// 性能指标收集器
///
/// 负责收集和展示应用程序的性能指标，包括：
/// - 吞吐量统计（每秒更新数）
/// - 错误率计算
/// - 执行时间追踪
///
/// # 使用模式
///
/// ```rust,no_run
/// use xtrade::binance::demo::{MetricsCollector, MessageStats};
/// use xtrade::binance::types::OrderBook;
///
/// let metrics = MetricsCollector::new();
/// let stats = MessageStats {
///     message_count: 100,
///     update_count: 50,
///     trade_count: 25,
///     error_count: 0,
///     total_trade_volume: 1000.0,
///     last_trade_price: Some(50000.0),
/// };
/// let orderbook = OrderBook::new("BTCUSDT".to_string());
/// metrics.print_summary(&stats, &orderbook);
/// ```
pub struct MetricsCollector {
    /// 开始计时的时间点
    start_time: Instant,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    /// 打印测试结果摘要
    pub fn print_summary(&self, stats: &MessageStats, orderbook: &OrderBook) {
        let elapsed = self.start_time.elapsed();

        println!("\n📊 Test Results Summary:");
        println!("   📬 Total messages: {}", stats.message_count);
        println!("   🔄 Depth updates processed: {}", stats.update_count);
        println!("   ❌ Errors encountered: {}", stats.error_count);
        println!("   📈 Final best bid: {:?}", orderbook.best_bid());
        println!("   📉 Final best ask: {:?}", orderbook.best_ask());
        println!("   📏 Final spread: {:?}", orderbook.spread());
        println!("   🏗️  Final levels: {}", orderbook.total_levels());
        println!(
            "   💰 Total bid volume: {:.2}",
            orderbook.total_bid_volume()
        );
        println!(
            "   💰 Total ask volume: {:.2}",
            orderbook.total_ask_volume()
        );

        // 性能指标
        let updates_per_second = stats.update_count as f64 / elapsed.as_secs_f64();
        println!("   ⚡ Updates per second: {:.1}", updates_per_second);

        if stats.error_count == 0 {
            println!("✅ All updates processed successfully!");
        } else {
            let error_rate = (stats.error_count as f64 / stats.message_count as f64) * 100.0;
            println!("⚠️  Error rate: {:.1}%", error_rate);
        }
    }
}

/// WebSocket 连接管理器
///
/// 封装了 WebSocket 连接的完整生命周期管理，包括：
/// - 连接建立和状态检查
/// - 消息监听启动
/// - 订阅管理
/// - 清理和断开连接
///
/// # 设计优势
///
/// - **资源管理**：确保连接正确建立和清理
/// - **错误传播**：使用 `?` 操作符简化错误处理
/// - **状态封装**：隐藏复杂的连接状态管理
pub struct WebSocketManager {
    /// 底层 WebSocket 客户端
    ws: BinanceWebSocket,
}

impl WebSocketManager {
    pub fn new(
        url: &str,
    ) -> (
        Self,
        tokio::sync::mpsc::Receiver<Result<BinanceMessage, super::types::WebSocketError>>,
    ) {
        let (ws, message_rx) = BinanceWebSocket::new(url);
        (Self { ws }, message_rx)
    }

    /// 连接并设置 WebSocket
    pub async fn connect_and_setup(&mut self, symbol: &str) -> Result<()> {
        // 检查初始状态
        let status = self.ws.status();
        println!("📡 Initial status: {:?}", status);

        // 连接
        self.ws.connect().await?;
        println!("✅ Connected successfully!");

        // 开始监听消息
        self.ws.start_listening().await?;
        println!("👂 Started listening for messages...");

        // 订阅交易流
        println!("📈 Subscribing to {} trade stream...", symbol);
        self.ws.subscribe_trade(symbol).await?;

        // 订阅深度流
        println!(
            "📈 Subscribing to {} depth stream (100ms updates)...",
            symbol
        );
        self.ws.subscribe_depth(symbol, Some(100)).await?;

        Ok(())
    }

    /// 清理和断开连接
    pub async fn cleanup(&self, symbol: &str) -> Result<()> {
        // 取消订阅交易流
        self.ws.unsubscribe(symbol, "trade").await?;
        println!("📉 Unsubscribed from {} trade stream", symbol);

        // 取消订阅深度流
        self.ws.unsubscribe(symbol, "depth@100ms").await?;
        println!("📉 Unsubscribed from {} depth stream", symbol);

        self.ws.disconnect().await?;
        println!("🔌 Disconnected successfully");

        Ok(())
    }
}

/// 重构后的 WebSocket 演示函数
pub async fn demo_websocket() -> AppResult<()> {
    println!("🔌 Testing Binance WebSocket OrderBook incremental updates...");

    const SYMBOL: &str = "BTCUSDT";
    const TEST_DURATION: Duration = Duration::from_secs(5);

    // 创建组件
    let (mut ws_manager, mut message_rx) =
        WebSocketManager::new("wss://stream.binance.com:9443/ws");
    let rest_client = BinanceRestClient::new("https://api.binance.com".to_string());
    let mut orderbook_manager = OrderBookManager::new(SYMBOL.to_string());
    let mut message_processor = MessageProcessor::new();
    let metrics_collector = MetricsCollector::new();

    // 连接和设置
    ws_manager.connect_and_setup(SYMBOL).await?;

    // 初始化订单簿
    orderbook_manager.initialize(&rest_client).await?;

    // 处理消息
    println!(
        "⏳ Processing depth updates for {} seconds...",
        TEST_DURATION.as_secs()
    );

    let start_time = Instant::now();
    while start_time.elapsed() < TEST_DURATION {
        if let Some(message_result) = message_rx.recv().await {
            message_processor
                .process_message(
                    message_result,
                    &mut orderbook_manager.orderbook,
                    &rest_client,
                )
                .await?;
        } else {
            // 没有消息时短暂休眠
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    // 打印结果和清理
    metrics_collector.print_summary(&message_processor.get_stats(), &orderbook_manager.orderbook);
    ws_manager.cleanup(SYMBOL).await?;

    Ok(())
}
