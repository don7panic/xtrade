# Price Trend 日线 K 线改造开发计划

## 概览

- **目标**：将 Price Trend 面板从实时折线改造为 1d K 线展示，降低渲染开销并提升趋势阅读能力。
- **里程碑周期**：约 1.5 周（8 个工作日）。
- **关键交付物**：数据接入、状态管理、TUI 蜡烛图渲染、文档与测试。

## 里程碑

### Milestone 1：数据接入与缓存（Day 1-3）

1. **REST 历史拉取**
   - 扩展 `BinanceRestClient` 支持 `klines`.
   - 新增 `DailyCandle` 结构与解析逻辑。
   - 在订阅流程中加载最近 `ui.kline_history` 根蜡烛。
2. **WebSocket 增量**
   - 扩展 `SymbolSubscription`，订阅 `kline@1d`.
   - 定义 `MarketEvent::DailyCandleUpdate`.
3. **测试**
   - 单元测试：REST 解析、事件转换。
   - 集成测试：mock WebSocket, 验证蜡烛更新与收盘。

### Milestone 2：状态管理与刷新策略（Day 4-5）

1. **状态扩展**
   - `MarketDataState` 添加 `daily_candles`、`kline_render_cache`.
   - 在 `UIManager` 中消费 `DailyCandleUpdate`, 更新缓存并触发节流刷新。
2. **缓存策略**
   - 限制长度、处理窗口变更重建。
   - 抽样算法（按面板宽度）。
3. **测试**
   - 单元测试：缓存裁剪、抽样输出。
   - 端到端测试：模拟窗口尺寸变化。

### Milestone 3：TUI 渲染与体验（Day 6-7）

1. **蜡烛图渲染**
   - 替换 `Chart` 折线为 `Candlestick` 组件。
   - 绘制涨跌颜色、时间/价格刻度。
2. **刷新调度**
   - 在渲染层新增最小刷新间隔（如 60s），避免无效重绘。
3. **UI 验收**
   - 本地运行多交易对，确认面板更新顺畅。

### Milestone 4：文档与收尾（Day 8）

1. 更新 `architecture.md`、`user_guide.md`、`docs/agent/PLAN.md`.
2. 增补配置说明、操作指南。
3. 回归 `make fmt`, `cargo clippy -- -D warnings`, `make test`.

## 资源需求

- 一名工程师负责端到端开发。
- 需要访问 Binance 公共 API（REST + WebSocket）。
- Mock/测试需要现有 `wiremock` 基础。

## 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| API 限流或不稳定 | 对历史拉取加入重试与速率限制；必要时缓存到本地文件。 |
| 终端宽度过窄导致可读性差 | 引入最小宽度判断并显示提示。 |
| 复杂度上升导致延期 | 每个里程碑后进行代码评审，确保增量交付。 |

## 完成标准

- 任意交易对 Price Trend 面板展示最近日线 K 线，不出现实时报错或严重闪烁。
- 柱形颜色与价格方向一致，坐标标签正确。
- 文档与配置项同步更新，用户可通过命令调整窗口长度。
- CI 通过且新增测试覆盖关键路径。
