# XTrade 第一阶段 Sprint 实现计划

## 概述

第一阶段目标：构建实时数据获取与展示系统，支持Binance Spot交易对的并发订阅，提供高质量的TUI/CLI界面。

## 技术栈确认

- 运行时：Tokio 1.35 异步运行时
- WebSocket：tokio-tungstenite 0.21
- HTTP客户端：reqwest 0.11  
- TUI界面：ratatui 0.25 + crossterm 0.27
- CLI框架：clap 4.4
- 序列化：serde + serde_json
- 错误处理：anyhow + thiserror
- 日志系统：tracing + tracing-subscriber
- 配置管理：config 0.13

---

## Week 1: 基础架构搭建 (Day 1-7)

### Day 1-2: 项目初始化与CLI框架

#### 任务详情

- **Cargo项目初始化**
  - 创建工作空间结构：`src/main.rs`, `src/lib.rs`, `src/cli/`, `src/config/`, `src/binance/`
  - 配置 `Cargo.toml` 依赖项（见架构文档技术栈）
  - 设置开发环境：rustfmt, clippy配置

- **CLI命令框架搭建**
  - 使用clap 4.4实现命令行解析
  - 实现核心命令结构：`subscribe`, `unsubscribe`, `list`, `ui`, `status`, `config`
  - 添加全局参数：`--config-file`, `--log-level`, `--verbose`

#### 技术实现要点

```rust
// src/cli/mod.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtrade")]
#[command(about = "XTrade Market Data Monitor")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    #[arg(long, default_value = "config.toml")]
    pub config_file: String,
    
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

#[derive(Subcommand)]
pub enum Commands {
    Subscribe { symbols: Vec<String> },
    Unsubscribe { symbols: Vec<String> },
    List,
    Ui { #[arg(long)] simple: bool },
    Status,
    Show { symbol: String },
}
```

#### 验收标准

- [x] `cargo run -- --help` 显示完整帮助信息
- [x] 所有子命令框架就位，可接受参数（暂时空实现）
- [x] 代码通过 `cargo clippy` 检查无警告

### Day 3-4: 配置管理与日志系统

#### 任务详情

- **配置系统实现**
  - 设计配置文件结构（TOML格式）
  - 实现配置加载、验证、默认值处理
  - 支持环境变量覆盖关键配置

- **日志系统集成**
  - 集成tracing + tracing-subscriber
  - 实现结构化日志输出（JSON格式可选）
  - 支持动态日志级别调整

#### 技术实现要点

```rust
// src/config/mod.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub symbols: Vec<String>,
    pub refresh_rate_ms: u64,
    pub orderbook_depth: usize,
    pub enable_sparkline: bool,
    pub log_level: String,
    pub binance: BinanceConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BinanceConfig {
    pub ws_url: String,
    pub rest_url: String,
    pub timeout_seconds: u64,
    pub reconnect_interval_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            symbols: vec!["BTCUSDT".to_string()],
            refresh_rate_ms: 100,
            orderbook_depth: 20,
            enable_sparkline: true,
            log_level: "info".to_string(),
            binance: BinanceConfig::default(),
        }
    }
}
```

#### 验收标准

- [x] 配置文件加载成功，支持TOML格式
- [x] 日志输出包含时间戳、级别、模块信息
- [x] 配置验证错误有清晰的错误提示
- [x] 支持 `--log-level debug` 动态调整日志级别

### Day 5-6: Binance WebSocket连接基础

#### 任务详情

- **WebSocket客户端实现**
  - 集成tokio-tungstenite，实现基础连接
  - 实现消息发送/接收的异步处理
  - 添加连接状态管理（Connected, Disconnected, Connecting）

- **消息解析框架**
  - 定义Binance消息格式的Rust结构体
  - 实现JSON消息的反序列化
  - 错误处理和消息验证

#### 技术实现要点

```rust
// src/binance/websocket.rs
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

pub struct BinanceWebSocket {
    url: String,
    status: ConnectionStatus,
    tx: Option<UnboundedSender<Message>>,
}

#[derive(Debug, Deserialize)]
pub struct BinanceMessage {
    pub stream: String,
    pub data: serde_json::Value,
}

impl BinanceWebSocket {
    pub async fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // WebSocket连接实现
    }
    
    pub async fn send_subscribe(&self, streams: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        // 订阅消息发送
    }
}
```

#### 验收标准

- [ ] 成功连接到Binance WebSocket (wss://stream.binance.com:9443/ws)
- [ ] 能够发送订阅消息并接收响应
- [ ] 基础消息解析无错误，能打印接收到的消息
- [ ] 连接状态变化有日志记录

### Day 7: OrderBook快照获取与验证

#### 任务详情

- **REST API客户端**
  - 使用reqwest实现HTTP客户端
  - 实现OrderBook快照获取接口
  - 添加请求超时和错误处理

- **OrderBook数据结构**
  - 设计OrderBook内存结构（BTreeMap存储）
  - 实现快照数据的初始化
  - 添加数据验证逻辑

#### 技术实现要点

```rust
// src/binance/orderbook.rs
use std::collections::BTreeMap;
use ordered_float::OrderedFloat;

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: BTreeMap<OrderedFloat<f64>, f64>,
    pub asks: BTreeMap<OrderedFloat<f64>, f64>,
    pub last_update_id: u64,
    pub snapshot_time: u64,
}

#[derive(Debug, Deserialize)]
pub struct DepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

impl OrderBook {
    pub async fn fetch_snapshot(symbol: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let url = format!("https://api.binance.com/api/v3/depth?symbol={}&limit=1000", symbol);
        // REST API调用实现
    }
    
    pub fn update_from_snapshot(&mut self, snapshot: DepthSnapshot) {
        // 快照数据应用逻辑
    }
}
```

#### 验收标准

- [ ] 成功获取BTCUSDT的OrderBook快照
- [ ] 快照数据正确解析到BTreeMap结构
- [ ] 价格排序正确（bids降序，asks升序）
- [ ] 包含last_update_id字段用于后续增量更新验证

---

## Week 2: 核心功能实现 (Day 8-14)

### Day 8-9: OrderBook增量更新与数据完整性

#### 任务详情

- **增量更新逻辑**
  - 实现WebSocket depth消息处理
  - 按照Binance官方文档实现序列号验证
  - 处理价格档位的增加、更新、删除

- **数据完整性保障**
  - 实现消息序列号检查
  - 丢弃过时消息（序列号 <= snapshot.lastUpdateId）
  - 添加数据一致性监控

#### 技术实现要点

```rust
// src/binance/depth_update.rs
#[derive(Debug, Deserialize)]
pub struct DepthUpdate {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}

impl OrderBook {
    pub fn apply_depth_update(&mut self, update: DepthUpdate) -> Result<(), OrderBookError> {
        // 序列号验证
        if update.first_update_id <= self.last_update_id {
            return Ok(()); // 丢弃过时消息
        }
        
        // 应用bids更新
        for [price_str, qty_str] in update.bids {
            let price = OrderedFloat(price_str.parse::<f64>()?);
            let qty = qty_str.parse::<f64>()?;
            
            if qty == 0.0 {
                self.bids.remove(&price);
            } else {
                self.bids.insert(price, qty);
            }
        }
        
        // 应用asks更新（同样逻辑）
        
        self.last_update_id = update.final_update_id;
        Ok(())
    }
}
```

#### 验收标准

- [ ] OrderBook增量更新正确应用到内存结构
- [ ] 序列号验证逻辑正确，过时消息被丢弃
- [ ] 零数量档位正确删除
- [ ] Best bid/ask价格实时更新准确

### Day 10-11: 重连机制与错误恢复

#### 任务详情

- **智能重连策略**
  - 实现指数退避算法（backoff策略）
  - 添加心跳检测和ping/pong处理
  - 网络异常时的自动重连

- **状态恢复机制**
  - 重连后重新获取OrderBook快照
  - 重新订阅交易对流
  - 连接质量监控

#### 技术实现要点

```rust
// src/binance/reconnect.rs
use backoff::{ExponentialBackoff, Backoff};

pub struct ReconnectManager {
    backoff: ExponentialBackoff,
    max_retries: u32,
    current_retries: u32,
}

impl ReconnectManager {
    pub fn new() -> Self {
        Self {
            backoff: ExponentialBackoff {
                initial_interval: Duration::from_millis(1000),
                max_interval: Duration::from_secs(60),
                max_elapsed_time: Some(Duration::from_secs(300)),
                ..Default::default()
            },
            max_retries: 10,
            current_retries: 0,
        }
    }
    
    pub async fn reconnect_with_backoff<F, Fut>(&mut self, reconnect_fn: F) -> Result<(), ReconnectError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<(), Box<dyn std::error::Error>>>,
    {
        // 指数退避重连实现
    }
}
```

#### 验收标准

- [ ] 网络断开时自动触发重连
- [ ] 重连间隔按指数退避增长（1s, 2s, 4s, 8s...）
- [ ] 重连成功后OrderBook状态正确恢复
- [ ] 重连次数和状态有日志记录

### Day 12-13: 多交易对并发订阅与性能优化

#### 任务详情

- **并发订阅架构**
  - 每个交易对使用独立的tokio任务
  - 实现交易对动态添加/移除
  - 任务间错误隔离

- **性能优化**
  - 实现消息队列化处理
  - 添加批量更新机制
  - 内存使用优化

#### 技术实现要点

```rust
// src/market_data/manager.rs
use std::collections::HashMap;
use tokio::sync::mpsc;

pub struct MarketDataManager {
    subscriptions: HashMap<String, SubscriptionHandle>,
    event_tx: mpsc::UnboundedSender<MarketEvent>,
}

pub struct SubscriptionHandle {
    task: JoinHandle<()>,
    control_tx: mpsc::UnboundedSender<ControlMessage>,
}

#[derive(Debug)]
pub enum MarketEvent {
    PriceUpdate { symbol: String, price: f64, time: u64 },
    OrderBookUpdate { symbol: String, orderbook: OrderBook },
    ConnectionStatus { symbol: String, status: ConnectionStatus },
}

impl MarketDataManager {
    pub async fn subscribe(&mut self, symbol: String) -> Result<(), SubscriptionError> {
        if self.subscriptions.contains_key(&symbol) {
            return Ok(()); // 已订阅
        }
        
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let event_tx = self.event_tx.clone();
        
        let task = tokio::spawn(async move {
            let mut subscription = SymbolSubscription::new(symbol.clone(), control_rx, event_tx).await;
            subscription.run().await;
        });
        
        self.subscriptions.insert(symbol, SubscriptionHandle { task, control_tx });
        Ok(())
    }
}
```

#### 验收标准

- [ ] 同时订阅3-5个交易对数据流正常
- [ ] 单个交易对异常不影响其他订阅
- [ ] 内存使用稳定，无明显泄漏
- [ ] CPU占用 < 5%（正常数据流）

### Day 14: 延迟测量与指标收集

#### 任务详情

- **延迟监控系统**
  - 实现端到端延迟测量（事件时间 vs 接收时间）
  - 统计P50, P95, P99延迟分布
  - 消息处理速度统计

- **指标收集框架**
  - 集成metrics库收集关键指标
  - 实现连接质量评估
  - 添加性能告警阈值

#### 技术实现要点

```rust
// src/metrics/mod.rs
use metrics::{histogram, counter, gauge};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ConnectionMetrics {
    pub status: ConnectionStatus,
    pub latency_p50: u64,
    pub latency_p95: u64,
    pub latency_p99: u64,
    pub reconnect_count: u32,
    pub last_message_time: u64,
    pub messages_per_second: f64,
}

pub struct MetricsCollector {
    latency_samples: Vec<u64>,
    message_count: u64,
    last_reset: SystemTime,
}

impl MetricsCollector {
    pub fn record_message_latency(&mut self, event_time: u64) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let latency = now.saturating_sub(event_time);
        
        self.latency_samples.push(latency);
        histogram!("message_latency_ms").record(latency as f64);
        counter!("messages_received").increment(1);
    }
    
    pub fn calculate_percentiles(&self) -> (u64, u64, u64) {
        // P50, P95, P99计算实现
    }
}
```

#### 验收标准

- [ ] 延迟统计准确，显示P50/P95/P99数值
- [ ] 消息处理速度实时计算（msgs/s）
- [ ] 连接状态变化有指标记录
- [ ] 延迟超过阈值时有日志告警

---

## Week 3: 用户界面开发 (Day 15-21)

### Day 15-16: TUI基础框架与布局

#### 任务详情

- **TUI框架搭建**
  - 集成ratatui + crossterm
  - 实现基础界面布局（见架构文档设计）
  - 添加键盘事件处理

- **界面组件设计**
  - 实现交易对标签页切换
  - OrderBook展示组件
  - 状态栏和日志区域

#### 技术实现要点

```rust
// src/ui/mod.rs
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph, Tabs},
    Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub struct App {
    pub should_quit: bool,
    pub selected_tab: usize,
    pub symbols: Vec<String>,
    pub market_data: HashMap<String, MarketDataState>,
}

#[derive(Debug, Clone)]
pub struct MarketDataState {
    pub price: f64,
    pub change_percent: f64,
    pub orderbook: OrderBook,
    pub volume_24h: f64,
    pub high_24h: f64,
    pub low_24h: f64,
}

impl App {
    pub fn new(symbols: Vec<String>) -> Self {
        Self {
            should_quit: false,
            selected_tab: 0,
            symbols,
            market_data: HashMap::new(),
        }
    }
    
    pub fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.previous_tab(),
            _ => {}
        }
    }
}
```

#### 验收标准

- [ ] TUI界面正常启动，显示基础布局
- [ ] 交易对标签页可以通过Tab键切换
- [ ] 键盘事件响应正常（q退出，Tab切换）
- [ ] 界面布局符合架构文档设计

### Day 17-18: 实时数据展示与OrderBook渲染

#### 任务详情

- **数据绑定**
  - 将后端市场数据流连接到TUI界面
  - 实现实时价格更新显示
  - OrderBook数据格式化和渲染

- **界面更新优化**
  - 实现渲染节流（100ms时间窗）
  - 脏标记机制，仅更新变化数据
  - 颜色主题和样式优化

#### 技术实现要点

```rust
// src/ui/widgets/orderbook.rs
use ratatui::{
    layout::{Constraint, Layout, Margin},
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

pub fn render_orderbook(f: &mut Frame, area: ratatui::layout::Rect, orderbook: &OrderBook) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // 渲染Asks（卖单）
    let asks: Vec<Row> = orderbook
        .asks
        .iter()
        .take(10)
        .map(|(price, qty)| {
            Row::new(vec![
                Cell::from(format!("{:.4}", qty)),
                Cell::from(format!("{:.2}", price.0)),
                Cell::from(format!("{:.2}", price.0 * qty)),
            ])
        })
        .collect();

    let asks_table = Table::new(asks)
        .header(Row::new(vec!["Volume", "Price", "Total"]).style(Style::default().fg(Color::Red)))
        .block(Block::default().borders(Borders::ALL).title("Asks"))
        .widths(&[Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)]);

    f.render_widget(asks_table, chunks[0]);

    // 渲染Bids（买单）- 类似实现
}
```

#### 验收标准

- [ ] OrderBook实时更新，价格变化实时反映
- [ ] Asks/Bids颜色区分（红/绿）
- [ ] 价格和数量格式化正确显示
- [ ] 界面刷新流畅，无闪烁现象

### Day 19-20: 键盘交互与多功能完善

#### 任务详情

- **完整键盘交互**
  - 实现所有快捷键功能（见架构文档）
  - 添加暂停/恢复数据流功能
  - 强制重连和状态查看

- **Sparkline价格走势**
  - 实现价格历史数据缓存
  - 使用ratatui绘制价格走势图
  - 环形缓冲区限制内存使用

#### 技术实现要点

```rust
// src/ui/widgets/sparkline.rs
use ratatui::widgets::{Sparkline, Block, Borders};
use std::collections::VecDeque;

pub struct PriceHistory {
    prices: VecDeque<f64>,
    max_size: usize,
}

impl PriceHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            prices: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
    
    pub fn add_price(&mut self, price: f64) {
        if self.prices.len() >= self.max_size {
            self.prices.pop_front();
        }
        self.prices.push_back(price);
    }
    
    pub fn render_sparkline(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let data: Vec<u64> = self.prices.iter().map(|&p| p as u64).collect();
        let sparkline = Sparkline::default()
            .block(Block::default().borders(Borders::ALL).title("Price Trend (24h)"))
            .data(&data)
            .style(Style::default().fg(Color::Yellow));
            
        f.render_widget(sparkline, area);
    }
}

// 键盘事件扩展
impl App {
    pub fn on_key(&mut self, key: KeyCode) -> AppResult<()> {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.previous_tab(),
            KeyCode::Char('r') => self.force_reconnect().await?,
            KeyCode::Char('p') => self.toggle_pause(),
            KeyCode::Char('s') => self.save_snapshot().await?,
            KeyCode::Up => self.scroll_up(),
            KeyCode::Down => self.scroll_down(),
            _ => {}
        }
        Ok(())
    }
}
```

#### 验收标准

- [ ] 所有快捷键功能正常工作
- [ ] Sparkline价格走势图正确显示
- [ ] 暂停功能正确停止/恢复数据更新
- [ ] 强制重连功能触发重新连接

### Day 21: 集成测试与性能验证

#### 任务详情

- **集成测试**
  - 端到端测试：从WebSocket到UI显示
  - 多交易对并发测试
  - 网络异常恢复测试

- **性能验证**
  - 验证性能目标达成情况
  - 内存使用和CPU占用测试
  - 长期运行稳定性测试

- **文档完善**
  - 更新README使用说明
  - 配置文件示例
  - 常见问题排查指南

#### 技术实现要点

```rust
// tests/integration_test.rs
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_end_to_end_data_flow() {
    // 启动应用
    let app = XTradeApp::new_test_mode().await;
    
    // 订阅BTCUSDT
    app.subscribe("BTCUSDT").await.unwrap();
    
    // 等待数据接收
    let result = timeout(Duration::from_secs(10), async {
        loop {
            if let Some(orderbook) = app.get_orderbook("BTCUSDT").await {
                if !orderbook.bids.is_empty() && !orderbook.asks.is_empty() {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }).await;
    
    assert!(result.is_ok(), "应该在10秒内收到OrderBook数据");
}

#[tokio::test] 
async fn test_reconnection_recovery() {
    // 测试网络断开恢复逻辑
}

#[tokio::test]
async fn test_multiple_symbols_concurrency() {
    // 测试多交易对并发订阅
}
```

#### 验收标准

- [ ] 所有集成测试通过
- [ ] 内存使用 < 50MB（5个交易对运行）
- [ ] CPU占用 < 5%（正常数据流）
- [ ] 消息处理延迟 < 1ms
- [ ] TUI渲染 15-20 FPS
- [ ] 可执行二进制文件正常工作

---

## 交付物检查清单

### 功能完整性

- [ ] CLI命令全部实现并可用
- [ ] TUI界面符合设计规范
- [ ] 多交易对并发订阅正常
- [ ] OrderBook实时更新准确
- [ ] 重连机制工作正常
- [ ] 性能指标达标

### 代码质量

- [ ] 代码通过clippy检查
- [ ] 单元测试覆盖率 > 80%
- [ ] 集成测试通过
- [ ] 错误处理完善
- [ ] 日志信息清晰

### 文档与部署

- [ ] README完整，包含安装和使用说明
- [ ] 配置文件示例齐全
- [ ] 构建脚本可用
- [ ] 跨平台二进制文件
- [ ] Docker镜像（可选）

### 扩展性预留

- [ ] 模块化架构清晰
- [ ] 配置驱动的功能开关
- [ ] 内部事件总线
- [ ] Exchange适配器模式

---

## 风险点与应对策略

### 技术风险

1. **WebSocket连接稳定性**
   - 风险：Binance连接不稳定导致数据丢失
   - 应对：完善重连机制，添加数据完整性验证

2. **TUI性能问题**
   - 风险：高频数据更新导致界面卡顿
   - 应对：实现渲染节流和脏标记优化

3. **内存使用增长**
   - 风险：长期运行内存泄漏
   - 应对：定期清理历史数据，设置缓存大小限制

### 进度风险

1. **TUI开发复杂度**
   - 风险：TUI开发经验不足导致进度延误
   - 应对：预留额外缓冲时间，先实现基础功能

2. **WebSocket API理解**
   - 风险：Binance API文档理解偏差
   - 应对：早期验证，多参考社区实现

### 质量风险

1. **数据准确性**
   - 风险：OrderBook更新逻辑错误
   - 应对：对比官方数据验证，添加一致性检查

2. **异常处理不完善**
   - 风险：边界情况处理不当导致崩溃
   - 应对：完善错误处理，添加降级机制
