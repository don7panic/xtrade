# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Build Commands
- `cargo build` - Build the project in debug mode
- `make build` or `cargo build --release` - Build optimized release binary
- `make run` or `cargo run --release` - Run the optimized binary
- `cargo run -- ui` - Run interactive TUI session (most common usage)
- `cargo run -- --help` - Show CLI help
- `make clean` - Clean build artifacts

### Test Commands
- `make test` or `cargo test` - Run all tests
- `cargo test -- --nocapture` - Run tests with output
- `cargo test <test_name>` - Run specific test
- `cargo test --test integration_test` - Run integration tests

### Lint and Formatting
- `cargo clippy --all-targets -- -D warnings` - Run clippy lint checks (CI parity)
- `make fmt` or `cargo fmt` - Format code with rustfmt
- `cargo fmt --check` - Check formatting without applying

### Documentation
- `cargo doc --open` - Build and open documentation
- `cargo test --doc` - Run documentation tests

## Architecture Overview

XTrade is a high-performance cryptocurrency market data monitor built in Rust with a focus on Binance exchange integration.

### Core Components

**CLI Interface (`src/cli/`)**
- Command parsing with clap
- Main subcommands: `ui`, `config`, `demo`
- Configuration file management and logging level controls
- Entry point adapter for different application modes

**Configuration System (`src/config/`)**
- Flexible configuration management with TOML-based files
- Environment variable overrides using `XTRADE_*` pattern
- Runtime validation with comprehensive error messages
- Hot-reload support for configuration changes without restart
- Symbol subscriptions, refresh rates, orderbook depth settings
- UI preferences, Binance endpoints, and logging configuration

**Binance Integration (`src/binance/`)**
- Production-ready exchange adapter with resilience patterns
- WebSocket client for real-time data streams with exponential backoff reconnection
- REST API client for orderbook snapshots for recovery/synchronization
- Comprehensive type system for orderbooks, trades, tickers
- Sequence number validation for data integrity
- Connection management and automatic recovery logic
- Demo and mock implementations for isolated testing

**Market Data Engine (`src/market_data/`)**
- High-throughput data processing with concurrent per-symbol tasks (max 10 symbols)
- Concurrent symbol subscription management with resource limits
- Orderbook storage with BTreeMap for ordered price levels
- Real-time price and volume tracking with event-driven architecture
- Multi-symbol concurrent processing with automatic rate limiting (>5 symbols)
- Connection quality monitoring and automatic recovery
- Daily candle data and price trend visualization
- Event types: PriceUpdate, OrderBookUpdate, TickerUpdate, ConnectionStatus, Error

**Session Management (`src/session/`)**
- Central orchestrator implementing sophisticated state machine: Starting → Running → Paused → ShuttingDown → Terminated
- Action channel using tokio::mpsc for SessionEvent communication
- Interactive command system: `/add`, `/remove`, `/status`, `/logs`, `/config show`
- CommandRouter for processing user input into structured actions
- Inter-component coordination and graceful shutdown handling
- TPS (transactions per second) monitoring and performance tracking

**UI Framework (`src/ui/`)**
- Professional terminal interface using ratatui with multi-panel layout
- CLI mode fallback for simple command-line output
- Real-time data visualization with interactive commands
- Centralized state management with `AppState` and command buffer
- Smart k-line render cache with width-aware sampling
- Bounded collections for logs and notifications
- Responsive layout management and state synchronization

**Metrics System (`src/metrics/`)**
- Observability built-in with comprehensive performance tracking
- `MetricsCollector` for latency tracking (P50, P95, P99 percentiles)
- Connection quality assessment with graded quality levels
- Message throughput monitoring and reconnection counters
- Real-time performance visualization in UI
- Error statistics and system health monitoring

### Key Technical Decisions

- **Async Runtime**: Tokio for high-performance async I/O
- **WebSocket**: tokio-tungstenite for Binance stream integration
- **HTTP Client**: reqwest for REST API calls
- **Error Handling**: anyhow + thiserror for comprehensive error management
- **Serialization**: serde + serde_json for Binance message parsing
- **Concurrency**: Arc<RwLock<T>> patterns for thread-safe shared state access
- **Event System**: MPSC channels for loose coupling between components
- **TUI Framework**: ratatui for professional terminal interfaces
- **Config Management**: config crate + TOML with environment variable support

### Application Startup Flow
```
main() → parse CLI → load config → init logging →
├── Config command → Config::handle_command()
├── Demo command → demo::demo_websocket()
└── Default → SessionManager::start() → TUI/CLI session
```

### Data Flow
```
CLI Command → SessionManager → MarketDataManager → SymbolSubscription → BinanceWebSocket
     ↓                    ↓                      ↓                    ↓
UI Manager ← SessionEvent ← MarketEvent ← WebSocket Message ← Binance API
```

1. CLI command triggers session initialization via `src/main.rs` (#[tokio::main] entry point)
2. Session manager (central orchestrator) coordinates component initialization
3. Market data manager handles symbol subscriptions with resource limits (max 10 symbols)
4. Binance WebSocket connects and subscribes to streams with resilience patterns
5. Real-time updates processed incrementally and stored in memory using BTreeMap for ordering
6. UI manager displays data in TUI (ratatui) or CLI mode with smart caching
7. Metrics system tracks performance, connection quality, and system health
8. Session manager handles graceful shutdown with signal handling (SIGTERM/SIGINT)

### Architecture Patterns

- **Single-Process Design**: All components in one Tokio process for minimal latency
- **Event-Driven Communication**: MPSC channels for inter-component coordination
- **Concurrent Per-Symbol Tasks**: Independent tokio tasks for each trading pair (max 10)
- **State Management**: Shared state using Arc<RwLock<T>> for thread-safe access
- **Graceful Degradation**: Individual symbol failures don't crash the application
- **Incremental Updates**: OrderBook updates applied incrementally, not full rebuilds

### Performance Optimizations & Resilience Patterns

**Performance Optimizations:**
- **Rate Limiting**: Automatic throttling when subscribing to >5 symbols
- **Incremental Updates**: OrderBook updates applied incrementally, not full rebuilds
- **Smart Caching**: K-line render cache with width-aware sampling
- **Memory Management**: Bounded collections for logs and notifications
- **Concurrent Processing**: Independent tokio tasks for each symbol (max 10)

**Resilience Patterns:**
- **Exponential Backoff**: WebSocket reconnection with backoff strategy
- **Graceful Degradation**: Individual symbol failures don't crash the application
- **Data Integrity**: Sequence number validation for orderbook updates
- **Automatic Recovery**: REST snapshot synchronization when WebSocket gaps occur
- **Connection Quality Monitoring**: Graded quality levels with health assessment
- **Signal Handling**: Graceful shutdown on SIGTERM/SIGINT

## Configuration Files

- `config.toml` - Main configuration file (copy from `config.toml.example`)
- Environment variable overrides using `XTRADE_*` pattern (e.g., `XTRADE_SYMBOLS`, `XTRADE_REFRESH_RATE_MS`)
- Hot-reload support for runtime configuration changes without restart
- Key settings: symbols list, refresh rates, UI preferences, Binance endpoints
- Runtime validation with comprehensive error messages

### Configuration Hierarchy
```
Environment Variables (highest priority)
├── XTRADE_SYMBOLS
├── XTRADE_REFRESH_RATE_MS
├── XTRADE_BINANCE_*
└── XTRADE_UI_*
↓
TOML Configuration File (config.toml)
├── symbols
├── refresh_rate_ms
├── binance.*
└── ui.*
↓
Default Values (fallback)
```

### Development Status

## Current Implementation (Complete)
- Full TUI implementation with ratatui and multi-panel layout
- Complete CLI framework with `ui`, `config`, `demo` subcommands
- Session management with state machine and action channels
- Market data engine with concurrent subscriptions and recovery logic
- Comprehensive Binance integration with WebSocket + REST clients
- Metrics system with latency tracking and performance visualization
- Daily candle data and price trend visualization

## Testing Infrastructure & Observability

**Testing Strategy:**
- **Unit Tests**: Standard cargo test framework for individual components
- **Integration Tests**: Located in `tests/` directory using `wiremock` for API mocking
- **Test Utilities**: `tokio-test`, `tempfile`, `criterion` for comprehensive testing
- **Mock Systems**: Complete demo/mock implementations in `src/binance/` for isolated testing

**Observability Features:**
- **Structured Logging**: `tracing` with file rotation and in-memory buffering
- **Metrics Collection**: Built-in performance monitoring with percentile tracking
- **Health Checks**: Connection quality monitoring with graded assessment
- **Error Tracking**: Comprehensive error statistics and system health monitoring

## Development Tips & Operational Features

**Development Workflow:**
- Use `make run` or `cargo run -- ui` to start the interactive TUI session
- Use `cargo run --release` for production builds with optimizations
- Configuration changes in `config.toml` are hot-reloaded during runtime
- Environment variables override TOML settings using `XTRADE_*` pattern

**Debugging and Testing:**
- Use `RUST_LOG=debug` environment variable for verbose logging during development
- Mock implementations in `src/binance/` allow testing without external dependencies
- Integration tests in `tests/` use `wiremock` to simulate Binance API responses
- OrderBook update logic has comprehensive unit tests for incremental updates

**Operational Features:**
- Interactive commands: `/add`, `/remove`, `/status`, `/logs`, `/config show`
- The application maintains up to 10 concurrent symbol subscriptions with resource limits
- WebSocket connections use exponential backoff for automatic reconnection
- OrderBook data integrity is maintained through sequence number validation
- All market data is processed incrementally for optimal performance
- Signal handling for graceful shutdown (SIGTERM/SIGINT)
- Real-time TPS monitoring and performance tracking

## Planning/Roadmap
- Trading actions (order entry, cancel, position tracking)
- Alerting system for conditional price triggers
- Persistent storage (SQLite) for historical data
- API credential management and secure configuration handling