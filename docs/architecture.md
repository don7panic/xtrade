# Architecture - XTrade 第一阶段设计

## 一、总体设计决策（Rust 技术选型）

- **高性能实时处理**：Rust 在处理高频 WebSocket 消息和内存化 OrderBook 更新方面有显著优势，确保低延迟数据流
- **内存安全保障**：Rust 的所有权系统消除了长期运行场景下的内存泄漏和悬空指针风险
- **并发模型优势**：Tokio 异步运行时提供高效的非阻塞 I/O，适合同时处理多个交易对的实时数据流
- **跨平台支持**：单个二进制文件即可在 Linux/macOS/Windows 运行，简化部署
- **未来扩展性**：为第二阶段的交易功能和多语言集成打下坚实基础

## 二、第一阶段功能范围（数据获取与展示）

### 核心功能目标

- **交互式终端会话**：一次启动进入长生命周期的交互式终端，支持命令输入、快捷键切换、状态栏提示与帮助引导。
- **实时数据流订阅**：支持多个 Binance Spot 交易对的并发订阅（aggTrade、depth、24hrTicker）。
- **高质量数据展示**：TUI 界面提供实时价格、OrderBook、24h 统计、日线 Price Trend 蜡烛图与通知区域。
- **数据完整性保障**：OrderBook 快照+增量更新验证、消息序列号检查、数据一致性监控。
- **连接稳定性**：智能重连机制、心跳检测、网络异常处理、连接状态实时显示。
- **性能监控**：端到端延迟测量、消息处理速度、内存使用情况、连接质量指标，并在 UI 中可视化。
- **会话内管理能力**：在终端内完成交易对订阅/取消、配置管理、状态查询、日志查看。

### 明确不包含（第二阶段）

- ❌ **交易操作**：下单、撤单、持仓管理
- ❌ **价格告警**：条件监控、通知系统  
- ❌ **API 认证**：密钥管理、私有接口调用
- ❌ **数据持久化**：历史数据存储、回放功能

## 三、第一阶段架构选择（单进程方案）

### 选定方案：单进程 Rust 完整实现

考虑到第一阶段的目标聚焦和快速交付要求，采用单进程架构：

**架构特点**：

- 所有功能模块在同一个 Rust 进程中运行
- WebSocket 客户端、数据处理、OrderBook 维护、交互式终端渲染统一管理
- 基于 tokio 异步运行时的多任务并发处理，交互式会话与后台订阅任务通过异步通道通信
- 配置文件驱动的模块化设计，可在会话内热更新关键参数

**优势分析**：

- ✅ **最小复杂度**：无进程间通信、无外部依赖
- ✅ **最佳性能**：内存共享、零拷贝数据传递、最低延迟
- ✅ **简化部署**：单个二进制文件、配置文件即可运行
- ✅ **快速开发**：专注核心功能、避免分布式系统复杂性

**未来扩展路径**：
第二阶段可根据需要拆分成微服务架构或添加 IPC 接口，当前设计预留了模块化扩展能力。

## 四、系统架构（第一阶段简化版）

### 核心组件

- **Interactive Session Layer**：负责会话生命周期管理、命令解析、快捷键处理、帮助提示与状态栏渲染。
- **Command Router & Action Dispatcher**：将用户输入映射为订阅/配置/查询等动作，协调后台任务执行并返回结果。
- **Market Data Engine**：WebSocket 连接管理、数据解析、OrderBook 维护、行情事件广播。
- **Display Layer**：TUI/Terminal 渲染、实时数据展示、面板布局、通知区与日志面板。
- **Binance Adapter**：WebSocket 客户端、数据格式转换、连接重试、REST 快照获取。
- **Configuration Manager**：TOML/YAML 配置文件读取、会话内动态参数更新、持久化默认订阅。
- **Logging & Metrics System**：结构化日志、性能指标收集、延迟可视化数据提供。

### 第二阶段扩展

- **Trading Engine**（下单、撤单、持仓管理）
- **Alert System**（条件监控、通知发送）  
- **Storage Layer**（SQLite 持久化）
- **Security Manager**（API 密钥管理）

## 五、交互式终端与后台协同设计

### 会话层结构

- **Session Manager**：启动/关闭交互式终端，维护运行状态与生命周期，负责恢复默认订阅并协调资源释放。
- **Command Router**：解析用户输入（命令/快捷键），将其转换为内部 `Action`，并投递给后台任务或会话状态机。
- **State Store**：集中管理订阅列表、当前聚焦交易对、UI 布局状态、性能指标缓存，向渲染层提供一致视图。
- **UI Renderer**：基于 `ratatui` 渲染多面板布局（行情、订单簿、指标、日志），响应状态变化触发增量刷新。
- **Price Trend Panel**：使用日线蜡烛图呈现趋势，开高低收由 Binance 1d kline 提供；面板维护本地缓存并按面板宽度抽样，渲染时按价格区间动态归一化；`enable_sparkline` 仍控制面板开关，同时新增 `ui.kline_refresh_secs` 用于限制流式刷新频率，避免高频抖动。
- **Notification Center**：对后台事件（重连、错误、告警）进行分类并推送到 UI 的消息区域。

### 事件流与通信

- **Action Channel**：会话层通过 `tokio::mpsc` 将命令投递到 `MarketDataManager` 或配置管理器。
- **Market Event Bus**：市场数据引擎向会话层广播 `PriceTick`、`OrderBookUpdate`、`ConnectionEvent` 等结构化事件。
- **State Update Loop**：会话层消费事件后更新 `State Store`，并触发 UI 局部刷新，确保界面实时响应且避免阻塞后台订阅。
- **Metrics Pipeline**：后台任务持续产出延迟、速率、错误计数，汇聚到会话状态并在状态栏中展示。
- **Graceful Shutdown**：`quit/exit` 命令触发停止信号，等待所有后台任务结束、关闭 WebSocket，并持久化最新配置。

## 六、Binance WebSocket 集成设计

- symbol 映射：用户输入 BTC-USDT -> Binance 要求格式如 BTCUSDT（通常小写用于 stream：btcusdt）。  
- WebSocket 源：使用 Binance 公共 stream，例如：wss://stream.binance.com:9443/stream?streams=btcusdt@aggTrade 或 bbc combined stream 支持多流。  
- Order book 初始化流程（必须）：
  1. 通过 REST 获取 snapshot（GET /api/v3/depth?symbol=BTCUSDT&limit=1000）。记录 snapshot.lastUpdateId。
  2. 从 WebSocket 接收 depth diff，丢弃序列号 <= snapshot.lastUpdateId，按文档顺序应用更新。
  3. 建立基于 sequence 的��证，避免缺失导致错乱（遵循 Binance 官方建议的流程）。  
- 消息类型优先级：aggTrade/trade 用于成交价与延迟计量；depth 用于 best bid/ask 与 top N 显示。  
- 延迟测量：事件中含有 eventTime 或 tradeTime，计算 now - eventTime 做端到端延迟统计，另外记录本地 receive timestamp。展示平均/95%/max 延迟。  
- 健壮性：心跳、ping/pong、指数退避重连（backoff）、并在重连后重新获取 snapshot 完成状态恢复。记录断连次数与最近恢复时间并在 UI 呈现。

## 七、第一阶段数据模型设计

### 核心数据结构（Rust）

```rust
// 价格tick数据
struct PriceTick {
    pair: String,
    event_time: u64,
    recv_time: u64, 
    price: f64,
    qty: f64,
    side: Side,
}

// OrderBook状态
struct OrderBook {
    pair: String,
    bids: BTreeMap<OrderedFloat<f64>, f64>,
    asks: BTreeMap<OrderedFloat<f64>, f64>, 
    last_update_id: u64,
    snapshot_time: u64,
}

// 24小时统计
struct SymbolStats {
    pair: String,
    price_change: f64,
    price_change_percent: f64,
    volume: f64,
    high: f64,
    low: f64,
    count: u64,
}

// 连接指标
struct ConnectionMetrics {
    status: ConnectionStatus,
    latency_p50: u64,
    latency_p95: u64, 
    latency_p99: u64,
    reconnect_count: u32,
    last_message_time: u64,
    messages_per_second: f64,
}
```

### 配置模型

```rust
struct Config {
    symbols: Vec<String>,
    refresh_rate_ms: u64,
    orderbook_depth: usize,
    enable_sparkline: bool,
    log_level: String,
    ui: UiConfig,
}

struct UiConfig {
    enable_colors: bool,
    update_rate_fps: u32,
    sparkline_points: usize,
    kline_refresh_secs: u64,
}
```

## 八、第一阶段技术栈选择

### 核心运行时与网络

- **异步运行时**：`tokio = "1.35"` - 成熟稳定的异步运行时
- **WebSocket 客户端**：`tokio-tungstenite = "0.21"` - 高性能WebSocket实现  
- **HTTP 客户端**：`reqwest = "0.11"` - OrderBook快照获取
- **错误处理**：`anyhow = "1.0"` + `thiserror = "1.0"` - 现代错误处理

### 数据处理与序列化

- **JSON序列化**：`serde = "1.0"` + `serde_json = "1.0"` - 标准序列化方案
- **数值精度**：`ordered-float = "4.0"` - OrderBook价格排序  
- **时间处理**：`chrono = "0.4"` - 时间戳处理与格式化

### 交互层与用户界面

- **命令行解析**：`clap = "4.4"` - 现代CLI框架，支持子命令  
- **TUI框架**：`ratatui = "0.25"` - 强大的终端UI库
- **跨平台终端**：`crossterm = "0.27"` - 跨平台终端控制
- **配置管理**：`config = "0.13"` + `serde` - TOML配置文件支持

### 监控与日志

- **结构化日志**：`tracing = "0.1"` + `tracing-subscriber = "0.3"`
- **指标收集**：`metrics = "0.21"` - 性能指标统计
- **重连策略**：`backoff = "0.4"` - 指数退避重试

### 开发与测试

- **单元测试**：内置 `#[cfg(test)]` + `tokio-test`
- **集成测试**：`wiremock = "0.5"` - Mock Binance API服务器
- **基准测试**：`criterion = "0.5"` - 性能基准测试

## 九、第一阶段交互式终端设计

### 会话入口与模式

- `xtrade` 命令启动后立即进入交互式终端，加载默认配置与订阅列表。
- 会话分为命令行模式（底部输入框）与快捷键模式（全局导航），默认显示欢迎面板与状态栏。
- 状态栏持续显示连接状态、订阅数量、平均延迟、消息速率等核心指标。
- 支持热重载：当配置文件更新或用户执行 `config set` 命令时，会话层触发后台任务更新。

### 命令体系与内部 Action

- `add <pairs>`：新增订阅；解析多个交易对后向 `MarketDataManager` 发送 `SubscribeAction`。
- `remove <pairs>`：取消订阅；触发 `UnsubscribeAction` 并清理状态缓存。
- `pairs`：查询当前订阅状态、最新行情快照、连接健康度。
- `logs [--tail N]`：读取最近日志缓冲区或写入文件。
- `config <key> <value>`：动态调整刷新频率、展示模式等配置，必要时触发 UI 重绘。
- `help` / `?`：输出命令说明与快捷键列表。
- `quit` / `exit`：执行优雅退出流程，等待后台任务结束。

### 布局与面板

```
┌─ XTrade Market Data Monitor ──────────────────────────────────────┐
│ [BTC-USDT] [ETH-USDT] [BNB-USDT]              Status: Connected   │
├────────────────────────────────────────────────────────────────────┤
│ BTC-USDT                               │ OrderBook (Top 10)       │
│ Price: $45,234.56 (+2.34%)             │                          │
│ 24h High: $46,123.45                   │ Asks    Price    Volume  │
│ 24h Low:  $44,012.34                   │ 0.234   45,245   1.2345  │
│ Volume:   1,234.56 BTC                 │ 0.123   45,244   0.9876  │
│                                        │ 0.456   45,243   2.1234  │
│ Price Chart (24h):                     │                          │
│ ▁▂▃▅▆▇█▇▆▅▃▂▁                          │ ─────────────────────── │
│                                        │ 1.234   45,242   1.5678  │
│                                        │ 0.567   45,241   0.8901  │
├────────────────────────────────────────────────────────────────────┤
│ Metrics: Latency P95: 85ms | Rate: 50msgs/s | Reconnects: 0       │
│ Logs: [14:23:45] Connected to stream | [14:23:46] OrderBook synced │
└────────────────────────────────────────────────────────────────────┘
```

### 键盘与命令交互

- 方向键：在交易对标签页间切换，更新当前选中交易对。
- `↑` / `↓`: 滚动 OrderBook 或日志面板。
- `r`: 向后台发送 `ReconnectAction`，强制重连所有订阅。
- `p`: 切换暂停/恢复渲染节流，用于定位性能问题。
- `s`: 保存当前交易对的 OrderBook 快照至文件。
- `q` / `Ctrl+C`: 与 `quit` 命令等效，触发优雅退出。

## 十、第一阶段性能优化策略

### 核心性能目标

- **消息处理延迟**：< 1ms（解析→内存更新→渲染队列）
- **内存使用**：< 50MB（5个交易对24小时运行）
- **CPU占用**：< 5%（正常数据流，现代4核CPU）
- **TUI渲染**：15-20 FPS 流畅体验

### 关键优化技术

1. **消息处理优化**
   - 零拷贝JSON解析（使用`simd-json`可选优化）
   - OrderBook增量更新，避免全量重建
   - 价格数据采用有序映射（BTreeMap）提升查询效率

2. **渲染性能优化**  
   - 渲染节流：批量更新UI（100ms时间窗）
   - 脏标记机制：仅重绘变化区域
   - Sparkline数据环形缓冲区，固定内存占用

3. **网络优化**
   - 连接池复用HTTP连接（快照获取）
   - WebSocket消息队列化处理，避免阻塞
   - 智能重连策略，减少无效重试

### 资源管理

- **内存管理**：定期清理过期价格历史数据
- **并发控制**：每个交易对独立tokio任务，避免互相影响  
- **错误隔离**：单个交易对异常不影响其他数据流

## 十一、第一阶段开发计划

### 开发里程碑（3周计划）

#### Week 1: 基础架构（Day 1-7）

- **Day 1-2**: 项目初始化、Cargo 配置、交互式会话骨架（tokio runtime + session loop）
- **Day 3-4**: 命令解析与路由器、配置管理系统、日志系统、基础错误处理
- **Day 5-6**: Binance WebSocket 连接、基础消息解析、Action 通道打通
- **Day 7**: 单交易对 OrderBook 快照获取与验证、State Store 初版

#### Week 2: 核心功能（Day 8-14）

- **Day 8-9**: OrderBook 增量更新逻辑、数据完整性验证、State Store 同步
- **Day 10-11**: 重连机制、错误恢复、连接监控事件推送至通知中心
- **Day 12-13**: 多交易对并发订阅、性能优化、Action/事件通道压测
- **Day 14**: 延迟测量、指标收集系统、状态栏数据接入

#### Week 3: 用户界面（Day 15-21）

- **Day 15-16**: TUI 面板框架、布局设计（ratatui）、状态栏雏形
- **Day 17-18**: 实时数据展示、OrderBook 渲染、通知面板
- **Day 19-20**: 键盘交互、多交易对切换、命令输入框体验优化、Sparkline
- **Day 21**: 集成测试、性能测试、文档完善（含交互式终端指南）

### 第二阶段预留接口

考虑到未来扩展需求，第一阶段设计中预留以下扩展点：

- **模块化架构**：核心组件接口化，便于后续拆分
- **配置驱动**：通过配置文件控制功能开关
- **事件系统**：内部事件总线，便于添加新的数据消费者
- **数据抽象**：Exchange适配器模式，便于支持多交易所

## 十二、第一阶段测试策略

- 使用 Binance Testnet 或搭建本地 mock server（返回预先录制的流）进行稳定性测试。  
- 压力测试：模拟高频 trade/depth 消息，观察内存、CPU 与延迟（可用 wrk-like 工具或自写模拟器）。  
- 恢复测试：中断网络，验证重连逻辑、snapshot 重新初始化是否正确。  
- 延迟验证：对比事件中的 eventTime 与接收时间的统计分布。

## 十三、交付物（建议）

- 一个可交付二进制（Linux/macOS/Windows）或 Docker image（单进程模式）。  
- README：启动方式、交互式终端使用说明、配置、常见故障排查。
- 示例配置与常见 pair 列表、Testnet 快速对接指引。  
- 若选分离模式：protobuf/gRPC 接口定义或 JSON schema。

## 十四、风险与缓解

- API 变更：保持模块化的 adapter 层，便于未来切换或升级。  
- 资源限制：大 pair 时注意内存增长，必要时做限流或分布式采集。  
- 时间同步问题：事件 timestamp 依赖交易所时间，需注意本地时钟偏差（可用 NTP 校时）。
