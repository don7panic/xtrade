# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Build Commands
- `cargo build` - Build the project in debug mode
- `cargo build --release` - Build optimized release binary
- `cargo run -- <args>` - Run the application with arguments
- `cargo run -- --help` - Show CLI help

### Test Commands
- `cargo test` - Run all tests
- `cargo test -- --nocapture` - Run tests with output
- `cargo test <test_name>` - Run specific test
- `cargo test --test integration_test` - Run integration tests

### Lint and Formatting
- `cargo clippy` - Run clippy lint checks
- `cargo fmt` - Format code with rustfmt
- `cargo fmt --check` - Check formatting without applying

### Documentation
- `cargo doc --open` - Build and open documentation
- `cargo test --doc` - Run documentation tests

## Architecture Overview

XTrade is a high-performance cryptocurrency market data monitor built in Rust with a focus on Binance exchange integration.

### Core Components

**CLI Interface (`src/cli/`)**
- Command parsing with clap
- Subcommands: subscribe, unsubscribe, list, ui, status, show, config
- Configuration management and logging setup

**Configuration System (`src/config/`)**
- TOML-based configuration files
- Default configuration with environment variable overrides
- Symbol subscriptions, refresh rates, orderbook depth settings

**Binance Integration (`src/binance/`)**
- WebSocket client for real-time data streams
- REST API client for orderbook snapshots
- Message parsing and serialization
- Connection management and reconnection logic

**Market Data Engine (`src/market_data/`)**
- Orderbook management with BTreeMap for price sorting
- Snapshot + incremental update validation
- Real-time price and volume tracking
- Multi-symbol concurrent processing

**TUI Interface (`src/ui/`)**
- Terminal UI with ratatui and crossterm
- Real-time data display with sparklines
- Orderbook visualization with color-coded bids/asks
- Keyboard navigation and interaction

**Metrics System (`src/metrics/`)**
- Performance monitoring and latency tracking
- Connection quality metrics
- Message processing statistics

### Key Technical Decisions

- **Async Runtime**: Tokio for high-performance async I/O
- **WebSocket**: tokio-tungstenite for Binance stream integration
- **HTTP Client**: reqwest for REST API calls
- **TUI Framework**: ratatui + crossterm for cross-platform terminal UI
- **Error Handling**: anyhow + thiserror for comprehensive error management
- **Serialization**: serde + serde_json for Binance message parsing
- **Orderbook Storage**: BTreeMap with ordered-float for price sorting

### Data Flow
1. CLI command triggers subscription to symbols
2. Binance WebSocket connects and subscribes to streams
3. Orderbook snapshots fetched via REST API
4. Real-time updates applied incrementally
5. Market data processed and stored in memory
6. TUI displays real-time data with configurable refresh
7. Metrics system tracks performance and connection quality

### Development Status
Based on the sprint plan, this is a work-in-progress project:
- Week 1: Basic CLI and configuration framework complete
- Week 2: Binance integration and market data processing (in progress)
- Week 3: TUI interface and performance optimization (planned)

Current implementation shows placeholder functionality with detailed TODO comments indicating planned features according to the sprint timeline.