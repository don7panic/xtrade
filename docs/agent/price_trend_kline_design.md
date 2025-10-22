# Price Trend 日线 K 线改造技术方案

## 背景与目标

- 当前 TUI 的 Price Trend 面板基于即时成交价绘制折线图，渲染成本高且无法体现日级趋势。
- 目标是改为 **Binance 1d K 线图**，仅在新日线生成或当前日线更新时刷新，降低渲染压力，并提供开高收低信息。

## 范围

- 数据源：接入 Binance 日线 K 线（历史 + 增量）。
- 状态管理：为每个交易对维护日线缓存。
- UI 展示：以蜡烛图样式渲染（日线 K 线），不再实时跟随每笔成交。
- 配置与命令：新增/复用配置项控制缓存长度及面板开关。

## 数据管线设计

### 1. 数据来源

| 场景 | 接口 | 用途 |
| --- | --- | --- |
| 启动加载 | REST `GET /api/v3/klines?interval=1d&limit=N` | 拉取最近 N 根日线填充缓存 |
| 实时更新 | WS `kline@1d` stream | 更新当日未收盘 K 线并在收盘时添加新 K 线 |

### 2. 数据类型

新增统一结构：

```rust
pub struct DailyCandle {
    pub open_time_ms: u64,
    pub close_time_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub is_closed: bool,
}
```

### 3. 管线步骤

1. **订阅准备**
   - Session 初始化或 `add` 新交易对时：
     - REST 拉取 `limit = config.ui.kline_history` 的日线列表。
     - 将历史日线写入 `MarketDataState.daily_candles`。
   - 启动 `kline@1d` websocket：复用现有 `SymbolSubscription` 模块，增加新的事件类型。
2. **增量更新**
   - WebSocket 下发 `kline` 消息后：
     - 若 `kline.is_final` 为 `false`：更新缓存中最后一根蜡烛。
     - 若为 `true`：将当前蜡烛标记收盘，再追加新蜡烛。
   - 触发 UI 重绘（但仍保持节流逻辑）。
3. **缓存维护**
   - 缓存长度受配置限制，超过后按时间丢弃最早蜡烛。
   - 暴露给 UI 的结构只读。

## UI 设计

1. **数据结构调整**
   - `MarketDataState` 增加 `daily_candles: Vec<DailyCandle>`.
2. **渲染组件**
   - 使用 `ratatui::widgets::Candlestick`。
   - 每根蜡烛的宽度至少 2 个字符；当面板宽度有限时按比例抽样（例如跳采），保持视觉稳定。
3. **刷新策略**
   - UI 只在以下情况重绘 Price Trend：
     - 收到新的 kline 事件。
     - 窗口尺寸变化。
   - 通过缓存渲染数据（预计算 `Vec<ratatui::widgets::CandlestickData>`）。
4. **视觉方案**
   - 涨：绿色、跌：红色。
   - 支持可选的成交量柱状图（后续增强）。

## 配置与命令扩展

| 配置项 | 默认 | 说明 |
| --- | --- | --- |
| `ui.enable_sparkline` | true | 沿用为 Price Trend 面板开关 |
| `ui.kline_history` | 90 | 缓存日线数量 |
| `ui.kline_refresh_secs` | 60 | 最小刷新间隔，限制 WS 抖动 |

命令扩展（可选）：

- `/trend window <days>` 动态调整显示长度。
- `/trend reload` 强制重新加载历史数据。

## 错误与退化策略

- REST 拉取失败：记录错误、展示占位文本，并周期性重试（指数退避）。
- WebSocket 中断：沿用现有重连流程，重连后重新获取当日最新蜡烛。
- 当所有数据不可用时，面板显示 “Daily candle data unavailable”。

## 依赖与影响

- 新增 REST 调用可能受限于 API 速率，需要在 `BinanceRestClient` 中复用连接与速率控制。
- `SymbolSubscription`、`MarketEvent`、`UIManager` 需扩展 `DailyCandleUpdate` 事件。
- 需更新 `architecture.md` 与用户文档说明 Price Trend 的展示方式。

## 风险评估

| 风险 | 等级 | 缓解 |
| --- | --- | --- |
| Binance kline 接口限流 | 中 | 限制一次加载数量，必要时加入本地缓存或退回折线模式 |
| UI 可视宽度不足 | 低 | 实现抽样与滚动窗口 |
| 与现有 tick 逻辑冲突 | 低 | Price Trend 面板与 tick 流分离，事件互不影响 |

## 验收标准

- 面板默认展示最近 `ui.kline_history` 根 1d 蜡烛。
- 日线更新时 UI 在 ≤1s 内刷新，无明显卡顿。
- 断线重连后面板可恢复最新蜡烛。
