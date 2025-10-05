# XTrade 第一阶段 Sprint 排期（数据获取与展示）

## Sprint 纲要
- **周期**：3 周（21 天）
- **聚焦阶段**：PRD 第一阶段目标——交互式终端 + Binance 市场数据获取与展示。
- **总目标**：在单进程 Rust 架构下交付一个可长时间运行的终端会话，完成多交易对行情、OrderBook、性能指标与日志的实时呈现，并满足数据完整性与性能验收标准。
- **关键里程碑**：
  1. Week 1：会话骨架、配置与日志、基础 Binance 连接。
  2. Week 2：OrderBook 管线、重连韧性、多交易对指标。
  3. Week 3：TUI 交互体验、性能验证、发布工件。

## Workstream 与架构对齐
| Workstream | 目标 | 关键模块（参考 architecture.md） | 输出物 |
| --- | --- | --- | --- |
| 会话与配置层 | 建立 Session Manager、Command Router、State Store | `Interactive Session Layer`、`Command Router & Action Dispatcher`、`Configuration Manager` | CLI/TUI 启动流程、命令通道、配置加载、日志管线 |
| 市场数据引擎 | 搭建 Binance Adapter、Market Data Engine、Metrics | `Market Data Engine`、`Binance Adapter`、`Logging & Metrics System` | WebSocket + REST 管线、OrderBook 校验、延迟/重连指标、事件总线 |
| 展示与观测 | 构建 Display Layer、性能与日志面板 | `Display Layer`、`Notification Center`、`Metrics Pipeline` | 多面板 TUI、sparkline、状态栏、logs/stats 命令、长稳验证报告 |

## 周迭代计划

### Week 1（Day 1-7）：会话骨架与连接基础
- **目标**：交互式会话最小可运行版本 + Binance 单交易对连接。
- **任务拆分**：
  - **工程初始化**：创建模块目录、配置 `Cargo.toml`、Makefile、CI fmt/clippy 规则。
  - **CLI / Session Manager**：实现 `xtrade` 主入口、命令解析、Action channel，确保 `help/quit/pairs` 等基础命令可响应。
  - **配置与日志**：加载 `config.toml` 默认订阅、刷新率，集成 `tracing` JSON/pretty 输出，支持 `--log-level`。
  - **Binance Adapter 基础**：使用 `tokio-tungstenite` 建立连接、解析合并流消息容器；构建 `reqwest` REST 快照获取。
  - **观测铺底**：定义核心结构体（`PriceTick`, `OrderBook`, `ConnectionMetrics`），预留 metrics 埋点接口。
- **交付检查**：
  - `cargo run -- ui --dry-run` 展示欢迎页 + 配置内容。
  - 单交易对连接成功（日志展示原始 tick），连接状态可追踪。
  - `make fmt`, `cargo clippy -- -D warnings` 通过。

### Week 2（Day 8-14）：数据完整性与韧性
- **目标**：完成 OrderBook 管线、重连机制、多交易对订阅与指标采集。
- **任务拆分**：
  - **OrderBook 管线**：snapshot 初始化、diff 序列校验、缺口恢复、档位增删、best bid/ask 更新。
  - **MarketDataManager**：支持 `add/remove` 动态订阅，独立任务隔离错误，事件广播到 State Store。
  - **连接韧性**：实现指数退避重连、心跳检测、重连后自动 snapshot + resubscribe、失败告警。
  - **性能指标**：延迟（P50/P95/P99）、消息速率、重连次数统计，`stats` 命令输出。
  - **测试**：snapshot/diff 单元测试、ReconnectManager 测试、mock WebSocket 集成测试（断线/丢包恢复）。
- **交付检查**：
  - `make test` 包括市场数据测试并通过。
  - 本地运行 ≥3 个交易对，OrderBook 状态准确。
  - metrics 日志记录延迟、消息速率，异常触发告警日志。

### Week 3（Day 15-21）：TUI 体验与验收
- **目标**：实现完整 TUI、交互命令、性能验证、文档与发布。
- **任务拆分**：
  - **UI 布局**：基于 `ratatui` 实现行情概要、OrderBook、sparkline、指标栏、日志/通知面板。
  - **交互逻辑**：Tab/上下键导航，`focus`、`r`、`p`、`s` 快捷键，`config` 命令热调参数。
  - **数据绑定**：State Store → UI Renderer 事件驱动刷新，100ms 节流、脏标记、颜色/主题优化。
  - **观测能力**：`stats` 面板展示延迟/吞吐/重连，`logs` 面板显示结构化日志，`config` 更新实时生效。
  - **系统硬化**：>6h soak test，CPU/内存监控；完成 README、配置示例、TUI 操作指南；准备 `make build` 二进制及可选 Dockerfile。
- **交付检查**：
  - `make fmt`, `cargo clippy -- -D warnings`, `make test` 全部通过。
  - 验证延迟 P95 < 100ms，网络断线 30s 内恢复。
  - Demo：启动 → 订阅多交易对 → 切换面板 → 查看 stats/logs → 优雅退出。

## 日程建议（按工作日拆分）
| 周 | Day | 关键活动 | 说明 |
| --- | --- | --- | --- |
| 1 | 1-2 | 工程脚手架、依赖、CI 管线 | 保障基础设施稳定；同步团队开发指南。 |
| 1 | 3-4 | Session Manager & CLI 子命令 | 完成命令到 Action 通道闭环，准备最小 UI loop。 |
| 1 | 5 | 配置加载 & 日志结构化输出 | 验证默认订阅、动态日志级别。 |
| 1 | 6 | Binance WS 连接 & 消息解析 | 打通 `connect -> subscribe -> recv` 流程。 |
| 1 | 7 | REST 快照解析 & 基础测试 | 建立 OrderBook 结构与初步单测。 |
| 2 | 8-9 | snapshot+diff 管线 & 序列校验 | 覆盖正常/丢包/乱序场景。 |
| 2 | 10-11 | ReconnectManager & 指数退避 | 包含心跳、自动 resubscribe、告警。 |
| 2 | 12 | 多交易对订阅 & 任务隔离 | 评估 combined stream vs 多连接策略。 |
| 2 | 13 | 延迟/吞吐指标集成 | 接入 metrics、构建 `stats` 命令。 |
| 2 | 14 | 集成测试 & 回归 | mock 流/REST，`make test` 稳定。 |
| 3 | 15-16 | TUI 布局 + 组件 | 面板、状态栏、日志区域初版。 |
| 3 | 17-18 | 数据绑定 & 渲染优化 | 事件总线对接、节流/脏标记、样式调优。 |
| 3 | 19 | 快捷键、sparkline、配置热更新 | 覆盖 PRD 命令要求。 |
| 3 | 20 | Soak test & 性能验收 | 长稳运行、记录指标、修复瓶颈。 |
| 3 | 21 | 文档、发布工件、演示彩排 | README、配置指南、demo 演练。 |

## 质量与验证策略
- **自动化**：每日运行 `make fmt`, `cargo clippy -- -D warnings`, `make test`；关键模块使用 `#[tokio::test]`；OrderBook 管线编写属性测试或参数化测试。
- **性能监测**：构建基准测试或模拟高频消息脚本；引入 `criterion` 或自定义测量以记录消息处理耗时。
- **稳定性测试**：网络断连/延迟注入、长时间运行 soak test、重连次数与内存曲线可视化。
- **文档同步**：更新 `docs/architecture.md` 与配置说明，确保与实现一致；维护问题排查清单。

## 风险与缓解
- **高频渲染导致卡顿**：优先实现节流、脏标记，提供 `p` 键暂停渲染。
- **Binance 序列缺口**：构建缺口检测 + 快照补偿，必要时重置订阅。
- **多任务协程泄漏**：统一使用 `tokio::select!` + `Cancellation`，测试多订阅频繁增删场景。
- **时间压力**：周中安排小型 Demo 检视（Day 4、Day 11、Day 18），及时发现偏差。

## Definition of Done
- 会话入口、市场数据引擎、TUI 展示完整闭环，满足 PRD 验收标准。
- 数据完整性、性能、稳定性指标达标，并有可视化/日志/metrics 证据。
- 代码通过 lint/test，关键路径具备文档、日志、监控支撑，交付二进制可直接运行。
