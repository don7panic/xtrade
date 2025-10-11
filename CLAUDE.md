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
- Subcommands: subscribe, unsubscribe, list, ui, status, show, config, demo, quit
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
- Demo and mock implementations for testing

**Market Data Engine (`src/market_data/`)**
- Concurrent symbol subscription management
- Orderbook storage with HashMap for quick lookups
- Real-time price and volume tracking
- Multi-symbol concurrent processing with rate limiting
- Connection quality monitoring and recovery

**Session Management (`src/session/`)**
- Interactive terminal session lifecycle management
- Command routing and action channel for event handling
- UI and metrics integration
- Graceful shutdown and timeout handling

**UI Framework (`src/ui/`)**
- UI manager for coordinating interface components
- App state management for TUI/CLI modes
- Event handling and state synchronization

**Metrics System (`src/metrics/`)**
- Performance monitoring and latency tracking
- Connection quality metrics
- Message processing statistics

### Key Technical Decisions

- **Async Runtime**: Tokio for high-performance async I/O
- **WebSocket**: tokio-tungstenite for Binance stream integration
- **HTTP Client**: reqwest for REST API calls
- **Error Handling**: anyhow + thiserror for comprehensive error management
- **Serialization**: serde + serde_json for Binance message parsing
- **Concurrency**: Arc<Mutex> patterns for shared state management
- **Event System**: MPSC channels for inter-component communication

### Data Flow
1. CLI command triggers session initialization
2. Session manager coordinates component initialization
3. Market data manager handles symbol subscriptions
4. Binance WebSocket connects and subscribes to streams
5. Real-time updates processed and stored in memory
6. UI manager displays data in TUI or CLI mode
7. Metrics system tracks performance and connection quality
8. Session manager handles graceful shutdown

### Development Status
Current implementation includes:
- Complete CLI framework with all subcommands
- Session management with lifecycle control
- Market data engine with concurrent subscriptions
- Binance integration framework
- UI framework foundation

Planned features:
- Full TUI implementation with ratatui
- Orderbook visualization
- Advanced metrics and analytics
- Performance optimizations