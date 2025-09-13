# Architecture - XTrade 第一阶段设计

## 一、总体设计决策（Rust 技术选型）

- **高性能实时处理**：Rust 在处理高频 WebSocket 消息和内存化 OrderBook 更新方面有显著优势，确保低延迟数据流
- **内存安全保障**：Rust 的所有权系统消除了长期运行场景下的内存泄漏和悬空指针风险
- **并发模型优势**：Tokio 异步运行时提供高效的非阻塞 I/O，适合同时处理多个交易对的实时数据流
- **跨平台支持**：单个二进制文件即可在 Linux/macOS/Windows 运行，简化部署
- **未来扩展性**：为第二阶段的交易功能和多语言集成打下坚实基础

## 二、第一阶段功能范围（数据获取与展示）

### 核心功能目标

- **实时数据流订阅**：支持多个 Binance Spot 交易对的并发订阅（aggTrade、depth、24hrTicker）
- **高质量数据展示**：TUI/CLI 界面提供实时价格、OrderBook、24h 统计、价格走势图（sparkline）
- **数据完整性保障**：OrderBook 快照+增量更新验证、消息序列号检查、数据一致性监控
- **连接稳定性**：智能重连机制、心跳检测、网络异常处理、连接状态实时显示
- **性能监控**：端到端延迟测量、消息处理速度、内存使用情况、连接质量指标
- **CLI 管理**：交易对订阅/取消、配置管理、状态查询、日志查看

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
- WebSocket 客户端、数据处理、OrderBook 维护、TUI 渲染统一管理
- 基于 tokio 异步运行时的多任务并发处理
- 配置文件驱动的模块化设计

**优势分析**：

- ✅ **最小复杂度**：无进程间通信、无外部依赖
- ✅ **最佳性能**：内存共享、零拷贝数据传递、最低延迟
- ✅ **简化部署**：单个二进制文件、配置文件即可运行
- ✅ **快速开发**：专注核心功能、避免分布式系统复杂性

**未来扩展路径**：
第二阶段可根据需要拆分成微服务架构或添加 IPC 接口，当前设计预留了模块化扩展能力。

## 四、Binance WebSocket 集成设计

- symbol 映射：用户输入 BTC-USDT -> Binance 要求格式如 BTCUSDT（通常小写用于 stream：btcusdt）。  
- WebSocket 源：使用 Binance 公共 stream，例如：wss://stream.binance.com:9443/stream?streams=btcusdt@aggTrade 或 bbc combined stream 支持多流。  
- Order book 初始化流程（必须）：
  1. 通过 REST 获取 snapshot（GET /api/v3/depth?symbol=BTCUSDT&limit=1000）。记录 snapshot.lastUpdateId。
  2. 从 WebSocket 接收 depth diff，丢弃序列号 <= snapshot.lastUpdateId，按文档顺序应用更新。
  3. 建立基于 sequence 的��证，避免缺失导致错乱（遵循 Binance 官方建议的流程）。  
- 消息类型优先级：aggTrade/trade 用于成交价与延迟计量；depth 用于 best bid/ask 与 top N 显示。  
- 延迟测量：事件中含有 eventTime 或 tradeTime，计算 now - eventTime 做端到端延迟统计，另外记录本地 receive timestamp。展示平均/95%/max 延迟。  
- 健壮性：心跳、ping/pong、指数退避重连（backoff）、并在重连后重新获取 snapshot 完成状态恢复。记录断连次数与最近恢复时间并在 UI 呈现。

## 五、第一阶段数据模型设计

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
}
```

## 六、第一阶段技术栈选择

### 核心运行时与网络

- **异步运行时**：`tokio = "1.35"` - 成熟稳定的异步运行时
- **WebSocket 客户端**：`tokio-tungstenite = "0.21"` - 高性能WebSocket实现  
- **HTTP 客户端**：`reqwest = "0.11"` - OrderBook快照获取
- **错误处理**：`anyhow = "1.0"` + `thiserror = "1.0"` - 现代错误处理

### 数据处理与序列化

- **JSON序列化**：`serde = "1.0"` + `serde_json = "1.0"` - 标准序列化方案
- **数值精度**：`ordered-float = "4.0"` - OrderBook价格排序  
- **时间处理**：`chrono = "0.4"` - 时间戳处理与格式化

### CLI与用户界面

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

## 七、第一阶段CLI与TUI设计

### CLI命令接口

```bash
# 基础订阅命令
xtrade subscribe BTC-USDT                    # 单个交易对订阅  
xtrade subscribe BTC-USDT,ETH-USDT,BNB-USDT # 多交易对订阅
xtrade unsubscribe BTC-USDT                  # 取消订阅
xtrade list                                  # 查看当前订阅

# 显示模式
xtrade ui                                    # 启动TUI界面
xtrade ui --simple                           # 简化CLI输出
xtrade show BTC-USDT                         # 单交易对详情

# 配置与监控
xtrade status                                # 连接状态和指标
xtrade config --file custom.toml             # 指定配置文件
xtrade logs --tail 100                       # 查看日志
```

### TUI界面布局设计

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

### 键盘交互设计

- `Tab` / `Shift+Tab`: 切换交易对标签页
- `↑` / `↓`: 滚动OrderBook或日志
- `r`: 强制重连WebSocket  
- `p`: 暂停/恢复数据流
- `s`: 保存当前快照到文件
- `q` / `Ctrl+C`: 退出程序

## 八、第一阶段性能优化策略

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

## 九、第一阶段开发计划

### 开发里程碑（3周计划）

#### Week 1: 基础架构（Day 1-7）

- **Day 1-2**: 项目初始化、Cargo配置、CLI框架搭建（clap）
- **Day 3-4**: 配置管理系统、日志系统、基础错误处理  
- **Day 5-6**: Binance WebSocket连接、基础消息解析
- **Day 7**: 单交易对OrderBook快照获取与验证

#### Week 2: 核心功能（Day 8-14）  

- **Day 8-9**: OrderBook增量更新逻辑、数据完整性验证
- **Day 10-11**: 重连机制、错误恢复、连接监控
- **Day 12-13**: 多交易对并发订阅、性能优化
- **Day 14**: 延迟测量、指标收集系统

#### Week 3: 用户界面（Day 15-21）

- **Day 15-16**: TUI基础框架、布局设计（ratatui）  
- **Day 17-18**: 实时数据展示、OrderBook渲染
- **Day 19-20**: 键盘交互、多交易对切换、Sparkline
- **Day 21**: 集成测试、性能测试、文档完善

### 第二阶段预留接口

考虑到未来扩展需求，第一阶段设计中预留以下扩展点：

- **模块化架构**：核心组件接口化，便于后续拆分
- **配置驱动**：通过配置文件控制功能开关
- **事件系统**：内部事件总线，便于添加新的数据消费者
- **数据抽象**：Exchange适配器模式，便于支持多交易所

## 十、第一阶段测试策略

- 使用 Binance Testnet 或搭建本地 mock server（返回预先录制的流）进行稳定性测试。  
- 压力测试：模拟高频 trade/depth 消息，观察内存、CPU 与延迟（可用 wrk-like 工具或自写模拟器）。  
- 恢复测试：中断网络，验证重连逻辑、snapshot 重新初始化是否正确。  
- 延迟验证：对比事件中的 eventTime 与接收时间的统计分布。

## 十二、交付物（建议）

- 一个可交付二进制（Linux/macOS/Windows）或 Docker image（单进程模式）。  
- README：启动方式、配置、常见故障排查。  
- 示例配置与常见 pair 列表、Testnet 快速对接指引。  
- 若选分离模式：protobuf/gRPC 接口定义或 JSON schema。

## 十三、风险与缓解

- API 变更：保持模块化的 adapter 层，便于未来切换或升级。  
- 资源限制：大 pair 时注意内存增长，必要时做限流或分布式采集。  
- 时间同步问题：事件 timestamp 依赖交易所时间，需注意本地时钟偏差（可用 NTP 校时）。
