# XTrade

XTrade is a Rust-based market data monitor for Binance Spot and USDT-M perp. It launches as a long-lived interactive terminal session, combining resilient WebSocket ingestion, data integrity checks, and a ratatui-powered TUI for real-time visualization of prices, order books, alerts, metrics, and logs.

## Features

- Interactive TUI session with command palette and multi-panel layout (overview, order book, metrics + price trend, logs, alerts).
- Binance Spot + USDT-M perp streams (trade, depth, 24h ticker, daily klines; perp adds mark price, funding rate, open interest, liquidation).
- Order book snapshot + diff reconciliation with sequence validation and auto-resync.
- Resilience primitives: heartbeats, exponential backoff reconnects, and action-triggered reconnects.
- Alerts with in-TUI creation and desktop notifications.
- Observability built in through `tracing` logs and `metrics` instrumentation (latency, throughput, reconnect counters).
- Config-driven behavior with runtime tuning via `/config set` for `refresh_rate_ms`, `orderbook_depth`, and `ui.sparkline_points`.

## Getting Started

### Prerequisites

- Rust 1.70 or newer with Cargo.
- `make` (for convenience targets).
- Network access to Binance public APIs and a terminal with UTF-8/color support.

### Build and Run

```bash
# Build optimized binary
make build

# Launch the interactive session (release binary)
make run

# Alternatively, run via cargo directly
cargo run -- ui

# Start in perp mode
cargo run -- ui --market perp
```

## Configuration

Default settings live in `config.toml`; copy `config.toml.example` to get started. Key options include:

```toml
symbols = ["BTCUSDT", "ETHUSDT", "BNBUSDT"]  # default subscriptions
refresh_rate_ms = 100                        # UI/poller cadence
orderbook_depth = 20                         # levels rendered per side
enable_sparkline = true                      # legacy flag (currently unused)
log_level = "info"                           # tracing filter

[binance]
ws_url = "wss://stream.binance.com:9443"     # streaming endpoint
rest_url = "https://api.binance.com"         # snapshot endpoint
reconnect_interval_ms = 5000                 # base backoff

[log]
file_path = "logs"                           # directory for hourly log files

[ui]
enable_colors = true
update_rate_fps = 20
kline_refresh_secs = 60                      # throttle K-line redraws

[[markets]]
exchange = "binance"
market_type = "perp_usdt"
symbols = ["BTCUSDT", "ETHUSDT"]
streams = ["aggTrade", "depth", "ticker", "markPrice", "fundingRate", "openInterest", "forceOrder"]
```

`symbols` apply to the market selected by `--market`; use `[[markets]]` for explicit multi-market subscriptions.

## Project Layout

- `src/main.rs`, `src/lib.rs`: entrypoints wiring CLI to session runtime.
- `src/session/`: session manager, action channels, command routing, shared state.
- `src/market_data/`: Binance subscriptions, order book model, daily candles.
- `src/binance/`: REST client, WebSocket adapter, data types, reconnect policy.
- `src/ui/`: ratatui layout, widgets, UI manager, Price Trend panel.
- `tests/`: integration and order book pipeline tests using mocked boundaries.
- `docs/`: architecture notes, sprint plan, user guide, design docs.

## Architecture Overview

The system follows a single-process, tokio-driven design where interactive session tasks, market data ingestion, and UI rendering coordinate via async channels. Core components—Session Layer, Command Router, Market Data Engine, Display Layer, Binance Adapter, Configuration Manager, and Metrics stack—are detailed in `docs/architecture.md`. Consult that document for component boundaries, data models, and planned second-phase extensions.

## Development Workflow

```bash
# Format code
make fmt

# Lint with Clippy (CI parity)
cargo clippy --all-targets -- -D warnings

# Run unit + integration tests
make test
```

Additional developer guidance, troubleshooting tips, and CLI usage examples are available in `docs/user_guide.md`. Sprint tasks and roadmap checkpoints are tracked in `docs/agent/PLAN.md`.

## Roadmap

Planned next-phase capabilities include:

- Trading actions (order entry, cancel, position tracking).
- Alert history, persistence, and external notification channels.
- Persistent storage (e.g., SQLite) for historical data and replay.
- API credential management and secure configuration handling.

See `docs/architecture.md` and `docs/agent/PLAN.md` for the latest milestone updates.
