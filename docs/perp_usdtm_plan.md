# Binance USDT-M 永续行情支持方案

本文档细化“仅 Binance、仅 USDT-M、仅行情”的落地设计，面向当前代码结构
(`src/binance/`, `src/market_data/`, `src/config/`, `src/ui/`, `src/session/`)。

## 目标与范围

目标：
- 在现有现货行情体系上扩展出 USDT-M 永续行情订阅与展示。
- 复用当前 MarketDataManager + Session + UI 渲染链路。

范围内：
- 行情数据：trade/aggTrade、depth、24hr ticker、kline(1d)。
- 永续专有数据：mark price、index price、funding rate、open interest、liquidation。
- 多交易对并发订阅、自动重连、性能指标与 UI 展示。

范围外（后续阶段）：
- 下单/撤单/持仓/风控。
- 历史数据持久化、回放与回测。

## 关键设计原则

- 不改变主架构：在现有模块内增加 perp 变体，而不是新系统。
- “市场类型”一等公民：避免 symbol 冲突与复用逻辑混乱。
- 先保证数据正确性与可观测性，再迭代 UI 细节。

## Binance USDT-M 接口概览

- REST Base: `https://fapi.binance.com`
- WS Base: `wss://fstream.binance.com`
- 重要流类型：
  - `@aggTrade` / `@trade`
  - `@depth@100ms`
  - `@ticker`
  - `@kline_1d`
  - `@markPrice@1s` (包含 mark/index)
  - `@fundingRate`
  - `@openInterest`
  - `@forceOrder` (liquidation)

## 模块与结构调整

### 1) 统一市场类型

新增枚举 (建议放在 `src/market_data/` 或 `src/binance/types.rs`)：

```rust
pub enum MarketType {
    Spot,
    PerpUsdt,
}

pub struct MarketKey {
    pub exchange: Exchange,
    pub market_type: MarketType,
    pub symbol: String,
}
```

目的：同一 symbol 在不同市场并行订阅时不冲突。

### 2) Binance 适配层拆分

在 `src/binance/` 增加分层：

- `binance::spot`：保留现货逻辑。
- `binance::perp_usdt`：新永续行情逻辑。
- `binance::common`：共享类型、WS/REST 客户端、消息解包。

此分层只影响模块组织，不改变已有调用路径语义。

### 3) MarketDataManager 扩展

在 `MarketEvent` 增加永续事件：

- `MarkPrice { symbol, mark_price, index_price, time }`
- `FundingRate { symbol, rate, funding_time }`
- `OpenInterest { symbol, open_interest, time }`
- `Liquidation { symbol, side, price, qty, time }`

订阅句柄由 `symbol` 改为 `MarketKey`。

### 4) 数据模型补充

新增合约规格与精度结构：

- `ContractSpec { symbol, contract_size, price_precision, qty_precision, margin_asset }`

用于 UI 显示单位换算、合约面板数据解释。

## 配置与运行时设计

### 配置新增 `markets` 列表

当前 `symbols` 仅支持现货。新增结构以适配市场类型：

```toml
[[markets]]
exchange = "binance"
market_type = "perp_usdt"
symbols = ["BTCUSDT", "ETHUSDT"]
streams = ["aggTrade", "depth", "ticker", "markPrice", "fundingRate", "openInterest"]

[binance.perp_usdt]
ws_url = "wss://fstream.binance.com"
rest_url = "https://fapi.binance.com"
```

保留 `symbols` 的兼容逻辑，用于默认现货；`perp_usdt` 仅覆盖 WS/REST，
其余超时与重连配置沿用 `[binance]`。

### 环境变量扩展

- `XTRADE_BINANCE_PERP_WS_URL`
- `XTRADE_BINANCE_PERP_REST_URL`
`markets` 仍通过 `config.toml` 管理（暂无环境变量覆盖）。

## 行情处理流程（永续）

流程与现货一致，重点是接口与事件类型：

1) REST snapshot：`/fapi/v1/depth?symbol=BTCUSDT&limit=1000`
2) WS depth diff：`btcusdt@depth@100ms`
3) 事件分发：
   - trade/aggTrade -> PriceUpdate
   - ticker -> TickerUpdate
   - markPrice -> MarkPrice
   - fundingRate -> FundingRate
   - openInterest -> OpenInterest
   - forceOrder -> Liquidation

## UI 展示改动

新增或扩展 UI 面板：

- 价格区：展示 Last / Mark / Index 与 basis。
- Funding 面板：显示 funding rate 与下一次结算倒计时。
- Open Interest：当前值 + 最近变化方向。
- Liquidation：在通知区滚动提示。

不改变当前主面板布局的情况下，建议先以“状态栏 + 通知区”集成。

### 面板进入方式（启动参数）

采用启动参数选择默认面板，保持脚本化与可预期性：

- `xtrade ui --market spot`
- `xtrade ui --market perp`

运行后进入对应面板。后续如需要无参数启动，可在配置中指定默认 market。

## 测试策略

- 使用 `wiremock` 提供 USDT-M REST/WS mock。
- 单元测试覆盖：
  - `MarketKey` 合法性
  - 新事件类型解析
  - depth snapshot + diff sequence 流程
- 集成测试覆盖：
  - spot + perp 并发订阅
  - 重连场景下的事件一致性

## 里程碑拆分

1) 配置与模型改造（MarketType/MarketKey/Config）。
2) Binance USDT-M 适配层与事件解析。
3) MarketDataManager 与 UI 展示。
4) 测试与稳定性验证。
