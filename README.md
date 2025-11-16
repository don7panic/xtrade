# XTrade

XTrade is a Rust-based market data monitor focused on high-throughput Binance Spot streams. It launches as a long-lived interactive terminal session, combining resilient WebSocket ingestion, data integrity checks, and a ratatui-powered TUI for real-time visualization of prices, order books, metrics, and logs.

## Features

- Interactive session lifecycle with command router, action channels, and runtime help.
- Concurrent Binance subscriptions (aggTrade, order book depth, 24h ticker) with snapshot + diff reconciliation and sequence validation.
- Multi-panel terminal UI showing per-symbol quotes, top-of-book ladders, daily K-line trend panel, status bar indicators, and structured log panes.
- Resilience primitives: heartbeats, exponential backoff reconnects, automatic re-sync via REST snapshots, and action-triggered reconnects.
- Observability built in through `tracing` logs and `metrics` instrumentation (latency percentiles, throughput, reconnect counters).
- Config-driven behavior supporting hot updates to refresh cadence, depth, color scheme, and Price Trend throttling.

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
```

## Configuration

Default settings live in `config.toml`; copy `config.toml.example` to get started. Key options include:

```toml
symbols = ["BTCUSDT", "ETHUSDT", "BNBUSDT"]  # default subscriptions
refresh_rate_ms = 100                        # UI/poller cadence
orderbook_depth = 20                         # levels rendered per side
enable_sparkline = true                      # enable Price Trend panel
log_level = "info"                           # tracing filter

[binance]
ws_url = "wss://stream.binance.com:9443"     # streaming endpoint
rest_url = "https://api.binance.com"         # snapshot endpoint
reconnect_interval_ms = 5000                 # base backoff

[ui]
enable_colors = true
update_rate_fps = 20
kline_refresh_secs = 60                      # throttle K-line redraws
```

Override values per environment using CLI flags or standard config sources supported by the `config` crate.

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

First-phase deliverables focus on market data visualization and observability. Planned second-phase capabilities include:

- Trading actions (order entry, cancel, position tracking).
- Alerting system for conditional price triggers and notifications.
- Persistent storage (e.g., SQLite) for historical data and replay.
- API credential management and secure configuration handling.

See `docs/architecture.md` and `docs/agent/PLAN.md` for the latest milestone updates.
