# Prd

下面给出一份面向交互式终端体验的 crypto trading bot 的产品需求文档（PRD），包含明确的 MVP 要求、扩展功能建议、系统架构概览、数据模型与接口要点、验收标准、以及基于该 PRD 的技术选型与理由。

## 一、目标与背景

- 目标：实现一个以交互式终端为核心体验的 crypto spot trading bot。用户通过一次 `xtrade` 命令即可进入长期运行的交互式会话，在同一终端中完成交易对订阅管理、实时行情监控与状态查询。首要聚焦 Binance spot，提供可靠的数据追踪能力，并具备可扩展的策略/风控模块，以便未来扩展自动化策略、回测与多交易所支持。
- 用户画像：熟悉命令行的交易者、量化研究员、想快速试验策略的开发者。

## 二、范围（第一阶段 - 数据获取与展示）

第一阶段必须实现的功能（优先级：高）

1. **交互式会话入口**：`xtrade` 启动后进入长生命周期的交互式终端（REPL/TUI 混合），提供命令提示、实时输出和帮助指引，默认持续运行直到用户显式退出。
2. **会话内交易对订阅管理**：在交互式终端中输入类似 `add BTC-USDT`、`remove ETH-USDT` 的指令完成 spot pair 的订阅增删、批量管理以及当前订阅概览。
3. **Binance Spot 实时价格订阅**：会话后台自动维护 WebSocket 连接，实时拉取 ticker/深度/24h 统计并以面板形式刷新展示。
4. **高质量交互式展示**：终端界面支持多交易对面板切换、聚合行情摘要、订单簿深度、sparkline/mini chart，以及订阅事件的通知区。
5. **数据完整性保证**：实现健壮的 WebSocket 连接管理、断线重连、orderbook 快照+增量更新的正确性验证，异常状态通过 UI 呈现并允许用户重试。
6. **性能与运行状态监控**：在会话内可随时查看端到端延迟、消息处理速度、连接稳定性等关键指标，并支持在状态栏中实时刷新。
7. **基础配置管理**：支持配置文件（TOML/YAML）初始化默认订阅、刷新频率、展示模式；会话内允许动态调整部分参数（如 refresh rate、显示列）。
8. **日志与监控**：提供日志面板或 `logs` 命令，展示 WebSocket 连接日志、数据质量日志、性能指标记录（可选持久化到文件）。

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

## 五、交互式终端体验（第一阶段）

### 会话生命周期

- 用户执行 `xtrade` 后进入交互式终端，会话自动加载配置文件中的默认订阅并启动数据流。
- 欢迎页展示当前订阅、快捷帮助和系统状态；顶部状态栏持续显示连接情况和性能指标。
- 会话内支持命令模式（输入指令）与快捷键模式（上下左右/Tab 切换面板），输出区域实时刷新行情。
- 会话默认常驻运行，支持 `quit`/`exit` 或 `Ctrl+C` 安全退出，退出前会进行资源清理和订阅持久化。

### 会话命令模型

- `xtrade`                            -> 启动交互式终端，显示欢迎页、当前订阅概览与帮助提示
- `add BTC-USDT` / `add BTC-USDT,ETH-USDT` -> 添加单个或多个交易对到订阅列表
- `remove BTC-USDT`                   -> 移除指定交易对
- `pairs`                             -> 查看当前所有订阅、连接状态、最近行情摘要
- `focus BTC-USDT`                    -> 将主显示面板切换到指定交易对
- `stats`                             -> 展示性能指标、延迟、消息速率、重连次数
- `logs`                              -> 查看最近日志或导出
- `config refresh-rate 100ms`         -> 动态调整刷新频率
- `help` / `?`                        -> 查看命令帮助
- `quit` / `exit`                     -> 退出交互式终端

（注：第二阶段将添加 trading、alarm 相关会话命令）

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

- 成功在交互式终端中以 <1s 延迟展示订阅的 spot tick（在网络正常情况下）
- 支持同时订阅并展示多个交易对（至少5个）的实时数据
- 交互式终端响应流畅，支持键盘/命令切换不同交易对面板
- OrderBook 数据完整性验证：快照+增量更新逻辑正确
- 网络断线后能在30秒内自动重连并恢复数据流

### 性能验收

- 端到端延迟：95%的消息处理时间 < 100ms，并在 UI 中可视化
- 内存使用：长时间运行（24小时）内存增长 < 100MB
- CPU使用：正常数据流下CPU占用 < 10%
- 渲染流畅：TUI刷新率达到10-20 FPS，交互命令平均响应 < 200ms

### 稳定性验收

- 连续运行24小时无崩溃，交互会话保持可用
- 网络异常恢复测试通过，并在 UI 中提供状态提示
- 配置文件修改或会话内动态调整后能正确生效

## 八、系统架构（第一阶段简化版）

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

- **Week 1**：交互式会话框架、命令路由器、配置管理、Binance WebSocket 基础连接
- **Week 2**：OrderBook 维护逻辑、数据完整性验证、后台任务与会话协同、错误处理与重连机制
- **Week 3**：TUI 面板与状态栏、交互优化、多交易对展示、性能与稳定性测试

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
- **交互式终端使用手册**：会话命令、快捷键与常见操作说明

### 技术文档

- **架构设计文档**：详细的模块设计与接口定义
- **开发环境搭建**：本地开发、测试、构建流程
- **性能测试报告**：延迟、吞吐量、稳定性测试结果

### 下一步准备

- **技术选型确认**：Rust crate 依赖列表
- **开发环境准备**：Binance Testnet 账号、开发工具配置
- **最小可运行 Demo**：单交易对订阅 + 基础 TUI 展示
