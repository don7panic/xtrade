# Prd

下面给出一份面向 CLI 的 crypto trading bot 的产品需求文档（PRD），包含明确的 MVP 要求、扩展功能建议、系统架构概览、数据模型与接口要点、验收标准、以及基于该 PRD 的技术选型与理由。

## 一、目标与背景

- 目标：实现一个以 CLI 为主的 crypto spot trading bot，首要聚焦 Binance spot，提供实时价格追踪、交易管理与告警功能，并具备可扩展的策略/风控模块，以便未来扩展自动化策略、回测与多交易所支持。
- 用户画像：熟悉命令行的交易者、量化研究员、想快速试验策略的开发者。

## 二、范围（第一阶段 - 数据获取与展示）

第一阶段必须实现的功能（优先级：高）

1. **交易对订阅管理**：可以在 CLI 中输入类似 BTC-USDT 的 spot pair 进行实时数据订阅管理（添加/移除订阅、查询当前订阅状态）。
2. **Binance Spot 实时价格订阅**：通过 WebSocket 订阅 Binance 的 spot price，并在 terminal 实时展示（清晰的 tick/盘口/深度简要、24h统计）。
3. **高质量数据展示**：Terminal/TUI 界面提供多交易对切换、实时价格走势（sparkline）、订单簿深度展示、连接状态监控。
4. **数据完整性保证**：实现健壮的 WebSocket 连接管理、断线重连、orderbook 快照+增量更新的正确性验证。
5. **性能指标监控**：展示端到端延迟、消息处理速度、连接稳定性等关键指标。
6. **基础配置管理**：支持配置文件（TOML/YAML）管理订阅参数、显示设置、连接参数等。
7. **日志与监控**：WebSocket 连接日志、数据质量日志、性能指标记录（可选持久化到文件）。

## 三、第二阶段规划（交易功能）

待第一阶段稳定后实现：

1. **API Key & Secrets 管理**：安全存储/读取用户的 Binance API Key
2. **交易操作**：下单、撤单、持仓查看（支持 Paper trading 模式）
3. **价格告警系统**：条件监控与多渠道通知
4. **基础风控**：交易限制与仓位管理
5. **数据持久化**：SQLite 存储交易记录与告警历史

## 四、建议的扩展功能（非第一阶段，但强烈建议纳入 roadmap）

- 多订单类型：Market / Limit / Stop-Limit / OCO / Trailing Stop
- 自动化策略：Grid trading、DCA、Mean reversion、Momentum（插件化策略）
- 回测框架：历史回测接口，支持 OHLC 数据下载与策略回测
- 多交易所支持：通过 Adapter 层扩展 Kraken、OKX、Coinbase 等
- UI/UX：基于 web 的仪表盘（可选），或提供 TUI（Terminal UI）
- 通知渠道：Discord、Slack、SMS、Webhook、Push（移动端）
- 高可用与部署：docker-compose / k8s 部署指南，监控（Prometheus + Grafana）
- 高级风控：最大回撤、净值保护、强制止损、仓位自动平衡
- 订单簿可视化 & depth heatmap（terminal 或 web）
- 用户管理与多策略托管（多配置文件/工作空间）

## 五、关键交互与 CLI 命令示例（第一阶段）

### 核心命令

- `xtrade subscribe BTC-USDT`        -> 添加订阅并开始实时显示
- `xtrade subscribe BTC-USDT,ETH-USDT` -> 批量添加订阅
- `xtrade unsubscribe BTC-USDT`      -> 移除订阅
- `xtrade list`                      -> 查看当前所有订阅
- `xtrade show BTC-USDT`             -> 查询特定交易对的当前价格/24h统计

### 显示控制

- `xtrade ui`                        -> 启动TUI界面（多面板显示）
- `xtrade ui --simple`               -> 简化CLI输出模式
- `xtrade config --refresh-rate 100ms` -> 设置刷新频率

### 系统管理

- `xtrade status`                    -> 连接状态和性能指标
- `xtrade logs --tail 100`           -> 查看最近日志
- `xtrade config --file config.toml` -> 指定配置文件

（注：第二阶段将添加 trading、alarm 相关命令）

## 六、数据模型（第一阶段）

### 核心数据结构

- **PriceTick**: pair, event_time, recv_time, price, qty, side
- **OrderBook**: pair, bids: Vec<(price, qty)>, asks: Vec<(price, qty)>, last_update_id  
- **SymbolStats**: pair, price_change, price_change_percent, volume, high, low, open_time, close_time
- **ConnectionMetrics**: status, latency, reconnect_count, last_message_time

### 第二阶段规划

- **Order**: id, pair, side, type, price, qty, status, created_at, filled_at
- **Alarm**: id, pair, condition (above/below), value, notify_methods, active  
- **Position**: pair, qty, avg_price, pnl

## 七、验收标准（第一阶段）

### 功能验收

- 成功在 terminal 中以 <1s 延迟展示订阅的 spot tick（在网络正常情况下）
- 支持同时订阅并展示多个交易对（至少5个）的实时数据
- TUI界面响应流畅，支持键盘切换不同交易对面板
- OrderBook 数据完整性验证：快照+增量更新逻辑正确
- 网络断线后能在30秒内自动重连并恢复数据流

### 性能验收  

- 端到端延迟：95%的消息处理时间 < 100ms
- 内存使用：长时间运行（24小时）内存增长 < 100MB
- CPU使用：正常数据流下CPU占用 < 10%
- 渲染流畅：TUI刷新率达到10-20 FPS

### 稳定性验收

- 连续运行24小时无崩溃
- 网络异常恢复测试通过
- 配置文件修改后能正确重载

## 八、系统架构（第一阶段简化版）

### 核心组件

- **CLI Interface**：命令解析、参数验证、配置管理
- **Market Data Engine**：WebSocket 连接管理、数据解析、OrderBook 维护
- **Display Layer**：TUI/Terminal 渲染、实时数据展示、用户交互
- **Binance Adapter**：WebSocket 客户端、数据格式转换、连接重试
- **Configuration Manager**：TOML/YAML 配置文件读取、运行时参数管理
- **Logging System**：结构化日志、性能指标收集

### 第二阶段扩展

- **Trading Engine**（下单、撤单、持仓管理）
- **Alert System**（条件监控、通知发送）  
- **Storage Layer**（SQLite 持久化）
- **Security Manager**（API 密钥管理）

## 九、部署与运维（第一阶段）

### 部署方式

- 单二进制文件发布（跨平台：Linux/macOS/Windows）
- Docker 镜像支持
- 配置文件驱动（TOML格式）

### 监控与运维

- 结构化日志输出（JSON格式可选）
- 内置健康检查命令
- 性能指标实时展示

## 十、里程碑与估时（第一阶段重新规划）

### 第一阶段（2-3周）

- **Week 1**：CLI 框架、配置管理、项目骨架、Binance WebSocket 基础连接
- **Week 2**：OrderBook 维护逻辑、数据完整性验证、错误处理与重连机制  
- **Week 3**：TUI 界面开发、多交易对支持、性能优化与测试

### 第二阶段（2-3周）

- **Week 4**：API 密钥管理、REST API 集成、Paper trading 基础
- **Week 5**：下单/撤单功能、告警系统、数据持久化
- **Week 6**：完整测试、文档编写、CI/CD 设置

## 十一、风险与应对（第一阶段）

### 技术风险

- **WebSocket 连接不稳定**：实现指数退避重连、心跳检测、连接状态监控
- **数据质量问题**：OrderBook 快照验证、序列号检查、数据完整性告警  
- **性能瓶颈**：消息处理队列、渲染节流、内存管理优化

### 运维风险  

- **Binance API 变更**：抽象适配层设计、版本兼容性检查
- **网络环境限制**：代理支持、重试机制、降级策略

## 十二、第一阶段交付物

### 核心交付

- **可执行程序**：跨平台二进制文件 + Docker 镜像
- **配置文件模板**：`config.toml` 示例与文档
- **CLI 命令文档**：完整的命令行接口说明

### 技术文档

- **架构设计文档**：详细的模块设计与接口定义
- **开发环境搭建**：本地开发、测试、构建流程
- **性能测试报告**：延迟、吞吐量、稳定性测试结果

### 下一步准备

- **技术选型确认**：Rust crate 依赖列表
- **开发环境准备**：Binance Testnet 账号、开发工具配置
- **最小可运行 Demo**：单交易对订阅 + 基础 TUI 展示
