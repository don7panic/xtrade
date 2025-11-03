# XTrade 第一阶段 Sprint 计划（数据获取与展示）

## Sprint 目标

- 交付一个以 `xtrade` 命令启动的长生命周期交互式会话，支持交易对订阅管理、实时行情浏览与状态查询。
- 构建可靠的 Binance Spot 市场数据管线，包含快照 + 增量深度、aggTrade、24h ticker，并对数据完整性和延迟进行监控。
- 提供多面板 TUI 体验，展示多交易对行情、OrderBook、性能指标与日志，满足 PRD 中的性能与稳定性验收标准。

## 核心交付物（对齐 PRD 验收）

- 会话入口：`xtrade` 启动欢迎页、命令帮助、订阅概览，支持 `add/remove/pairs/logs/config/quit` 等核心命令。
- 市场数据引擎：Binance 适配器、OrderBook 快照初始化 + 序列校验的 diff 管线、消息去重与延迟统计。
- 交互层：基于 `ratatui` 的多交易对面板、OrderBook 表格、Price Trend 日线蜡烛图面板、状态栏、通知区、日志视图。
- 稳定性与监控：断线重连、心跳检测、metrics（延迟、吞吐、重连次数）、结构化日志、配置热更新能力。
- 质量保障：完整的单元/集成测试、性能验证脚本、文档（使用手册 + 配置指南）。

## Week 1（Day 1-7）：基础框架与会话入口

- **工程与运行时骨架**
  - 初始化工作空间模块（`cli/`, `config/`, `binance/`, `market_data/`, `ui/`, `metrics/`）。
  - 配置 `Cargo.toml` 依赖、Makefile 目标、lint/format 管线。
- **CLI & 会话层脚手架**
  - 使用 `clap` 建立命令解析、Command Router、Session Manager 与 State Store 初版。
  - 打通命令到异步 Action 通道（`tokio::mpsc`），实现最小交互循环（帮助、退出、订阅列表占位）。
- **配置管理与日志**
  - 基于 `config` + `serde` 加载 TOML，支持默认值、环境变量覆盖、会话内读取。
  - 接入 `tracing` / `tracing-subscriber`，提供 JSON/pretty 输出、动态日志级别。
- **Binance 连接基础**
  - 实现 WebSocket 客户端（`tokio-tungstenite`），完成单交易对订阅、消息反序列化、连接状态回调。
  - 建立 REST 快照客户端（`reqwest`），验证订单簿 snapshot 结构解析。
- **观测与可测性铺底**
  - 准备 `metrics` 埋点接口、延迟时间戳记录器、基础结构体定义。
  - 为后续测试搭建 wiremock/fixtures 框架。
- **里程碑验收**
  - `cargo run -- --dry-run` 启动展示欢迎页 + 配置读取结果。
  - 成功连接单个交易对（输出原始 tick 到日志），记录连接状态。
  - CI 通过 `make fmt`, `cargo clippy -- -D warnings`。

## Week 2（Day 8-14）：市场数据引擎与韧性

- **OrderBook 管线完成度**
  - 实现 snapshot 初始化、diff 序列（`U/u`）校验与缺口修复，过时消息丢弃，零数量档位删除。
  - 构建 `OrderBook` 内存模型（`BTreeMap` + depth 限制），维护 `best bid/ask`。
- **多交易对订阅管理**
  - `MarketDataManager` 支持动态 `add/remove`，每个交易对独立任务 + 错误隔离。
  - Combined stream 或多连接策略评估并实现，保证 ≥5 个交易对的吞吐与稳定性。
- **连接可靠性**
  - 指数退避重连、心跳检测、重连后自动 snapshot + resubscribe、失败告警。
- `ReconnectAction` 输出重连次数、最近恢复时间。
- **性能指标与延迟统计**
  - 端到端延迟（event_time vs recv_time）直方图与 P50/P95/P99 计算。
  - 消息速率（msg/s）、重连计数、最后消息时间暴露给状态栏与 `stats` 面板。
- **测试覆盖**
  - 单元测试覆盖 snapshot/diff、ReconnectManager、序列校验边界。
  - 集成测试：mock WebSocket + REST，验证断线重连、丢包恢复。
- **里程碑验收**
  - `make test` 包含市场数据管线测试并全部通过。
  - 本地运行支持 3 个交易对实时更新，OrderBook 状态正确。
  - metrics 日志可见延迟与吞吐指标，异常时触发告警日志。

## Week 3（Day 15-21）：TUI 体验与端到端验收

- **UI 布局与交互**
  - 根据架构图实现多面板布局：行情概要、OrderBook、Price Trend 蜡烛图、指标、日志/通知。
- **实时数据绑定**
  - 将 MarketDataManager 事件总线接入 State Store → UI Renderer，支持 100ms 节流与脏标记刷新。
  - 展示多交易对并发数据、颜色区分涨跌、Price Trend 蜡烛图按面板宽度抽样并基于窗口内 min/max 动态归一化纵轴。
- **会话内观测能力**
  - `stats` 面板展示延迟分布、消息速率、重连次数；`logs` 面板展示最近结构化日志。
  - 支持 `config refresh-rate` 等命令热调参数并即时反馈。
- **系统硬化与文档**
  - 长时间运行验证（≥6h soak test），记录 CPU / 内存指标，确保达成 PRD 性能目标。
  - 完成 README 使用指南、TUI 操作说明、配置示例、问题排查章节。
  - 交付发布工件：`make build` 产出二进制，必要时 Dockerfile。
- **验收检查**
  - 触发 `make fmt`, `cargo clippy -- -D warnings`, `make test` 全绿。
  - 手动脚本验证延迟 < 100ms（P95），网络断线 30s 内恢复。
- Demo 演练：从启动 → 订阅多个交易对 → 切换面板 → 查看日志 → 优雅退出。

**Week 3 实施进展（当前迭代）**

- `ratatui` 多面板布局落地，行情/OrderBook/Price Trend 蜡烛图/指标/日志区域实时刷新。
- Price Trend 面板接入 Binance 日线 kline，支持本地缓存、宽度抽样与 `ui.kline_refresh_secs` 节流（默认 60s）以避免高频抖动。
- UI 渲染改为事件驱动 + 100ms 节流，加入 Tab/Shift+Tab、Space、Shift+L、`/` 等快捷键。
- 市场事件与 `MetricsCollector` 打通，定期推送 `ConnectionMetrics` 到 `stats` 面板。
- `config set refresh_rate_ms`、`config reset` 等指令即时生效，UI 自动调节刷新节奏与深度配置。
- 日志面板聚合 Session/Market 事件，错误与告警在终端即时可见。

## 质量保障与支撑任务

- 标准化流程：每日确保 `make fmt`, `cargo test`, `cargo clippy` 通过；重要模块引入 `#[tokio::test]` 异步测试。
- 性能基准：为核心循环添加 `criterion` 或自定义基准，记录消息处理耗时。
- 监控验证：提供脚本对接 `metrics-exporter-prometheus`（可选）用于本地可视化。
- 文档更新：同步维护 `docs/architecture.md` 的接口/事件总线演进，迭代 PRD 需要的新命令与视图。

## 风险与缓解

- **高频渲染导致卡顿** → 提前实现节流 + 脏标记，提供暂停渲染开关。
- **WebSocket 序列缺口** → 构建缺口检测与快照回补流程，必要时重置订阅。
- **长连接资源泄漏** → 对任务实现超时/取消，利用 `tokio::select!` 保证优雅退出。
- **时间压力** → 每周设置中期里程碑与 Demo，尽早验证 Binance 接入与 UI 基础，减少 Week3 堆积。

## 完成 Definition of Done

- 功能满足 PRD 第一阶段范围，操作流畅、指标达标。
- 代码通过 lint/test，关键路径有文档与日志/指标观测。
- 交互式终端 Demo 可复现核心场景，具备性能数据与问题排查指引。
