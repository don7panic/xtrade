# XTrade User Guide

XTrade is a high-performance cryptocurrency market data monitoring system built in Rust with a focus on Binance exchange integration. This guide provides comprehensive documentation for installing, configuring, and using XTrade.

## Table of Contents

1. [Installation and Prerequisites](#installation-and-prerequisites)
2. [Command Line Interface](#command-line-interface)
3. [Global Flags](#global-flags)
4. [Subcommands](#subcommands)
5. [Interactive Commands (TUI)](#interactive-commands-tui)
6. [Configuration File](#configuration-file)
7. [Environment Variables](#environment-variables)
8. [TUI Keyboard Shortcuts](#tui-keyboard-shortcuts)
9. [Troubleshooting](#troubleshooting)
10. [Performance Tips](#performance-tips)
11. [Development Usage](#development-usage)

## Installation and Prerequisites

### Prerequisites

- **Rust Toolchain**: XTrade requires Rust 1.70+ and Cargo
- **Operating System**: Linux, macOS, or Windows (WSL recommended for Windows)
- **Network**: Stable internet connection for Binance API access
- **Terminal**: Terminal emulator with UTF-8 and color support

### Installation Methods

#### Method 1: From Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/your-username/xtrade.git
cd xtrade

# Build in release mode
cargo build --release

# Install globally (optional)
cargo install --path .
```

#### Method 2: From Cargo

```bash
# Once published to crates.io
cargo install xtrade
```

#### Method 3: Pre-built Binaries

Download pre-built binaries from the [Releases page](https://github.com/your-username/xtrade/releases) for your platform.

### Verifying Installation

```bash
# Check version
xtrade --version

# Show help
xtrade --help
```

## Command Line Interface

XTrade uses a command-line interface built with `clap`. The basic syntax is:

```bash
xtrade [GLOBAL_FLAGS] [COMMAND] [COMMAND_FLAGS]
```

If no command is provided, XTrade defaults to `ui`.

### Getting Help

```bash
# General help
xtrade --help

# Command-specific help
xtrade ui --help
xtrade config --help
```

## Global Flags

XTrade provides several global flags that apply to all commands:

### `--config-file <PATH>`

Specify a custom configuration file path. Defaults to `config.toml` in the current directory.

```bash
xtrade --config-file /path/to/custom/config.toml ui
```

### `--log-level <LEVEL>`

Set the logging level. Available levels: `trace`, `debug`, `info`, `warn`, `error`. Default: `info`.

```bash
xtrade --log-level debug ui
xtrade --log-level trace ui
```

### `--verbose`

Enable verbose output (equivalent to `--log-level debug`). This flag takes precedence over `--log-level`.

```bash
xtrade --verbose ui
xtrade --verbose --log-level info ui  # Uses debug level due to --verbose
```

### `--dry-run`

Show the welcome page and configuration summary without starting the TUI.

```bash
xtrade --dry-run
```

## Subcommands

### `ui` - Start Terminal User Interface

Launch the interactive terminal user interface for real-time market data visualization.

```bash
# Start full TUI mode (default command)
xtrade

# Explicit TUI mode
xtrade ui

# Start perp UI mode
xtrade ui --market perp
```

**Options**:

- `--market`: `spot` (default), `perp`, `perp_usdt`, or `perp-usdt`
- `--simple`: Parsed but currently runs the same TUI (reserved for future simple output)

### `config` - Configuration Management

Manage XTrade configuration settings.

#### Show Current Configuration

```bash
xtrade config show
```

#### Set Configuration Value

```bash
# Set refresh rate
xtrade config set refresh_rate_ms 200

# Set log level
xtrade config set log_level debug

# Set multiple symbols
xtrade config set symbols '["BTCUSDT","ETHUSDT"]'
```

**Note**: The CLI `config set` command is parsed but not implemented yet. Use the config file or environment variables for persistent changes.

#### Reset Configuration

```bash
# Reset to default values
xtrade config reset
```

**Note**: `config reset` prints defaults to stdout; it does not write to disk.
**Note**: `xtrade config` currently reads `./config.toml` regardless of `--config-file`.

### `demo` - WebSocket Demo

Run a WebSocket demo workflow useful for debugging or validation.

```bash
xtrade demo
```

## Interactive Commands (TUI)

Press `/` or `:` in the TUI to open the command palette, then enter commands.

### Subscription and Status

- `/add <symbol1> [symbol2] ...` - Subscribe to symbols
- `/remove <symbol1> [symbol2] ...` - Unsubscribe from symbols
- `/list` or `pairs` - List active subscriptions
- `/status` - Show session statistics and connection status
- `/reconnect` or `/r` - Force reconnection and resync
- `/logs` - Show recent logs

### Configuration (in-memory)

- `/config show` - Show current configuration
- `/config set <key> <value>` - Update in-memory config (supported keys: `refresh_rate_ms`, `orderbook_depth`, `ui.sparkline_points` (min 10))
- `/config reset` - Reset to defaults in-memory

Changes from `/config set` are not persisted to `config.toml`.

### Alerts

- `a` (in normal mode) opens the alert popup for the current symbol
- `/alert:list` - List alerts in the log panel
- `/alert:clear <id|all>` - Clear an alert by id or clear all

### Help and Exit

- `/help` or `?` - Show help in the log panel
- `/quit` `/exit` `/q` - Exit the application

### Notes

- `/show <symbol>` is accepted but does not render a dedicated details panel yet.

## Configuration File

XTrade uses TOML format configuration files. The default configuration file is `config.toml` in the current working directory.

**Implementation Status**: Configuration system is fully implemented with file loading, environment variable overrides, and validation.

### Default Configuration Location

- Default: `./config.toml` (or any path passed via `--config-file`)

### Configuration Structure

```toml
# XTrade Configuration File

# Trading symbols to monitor by default (applies to the `--market` selection)
symbols = ["BTCUSDT", "ETHUSDT", "BNBUSDT"]

# Optional explicit market subscriptions (spot/perp)
[[markets]]
exchange = "binance"
market_type = "perp_usdt"
symbols = ["BTCUSDT", "ETHUSDT"]
streams = ["aggTrade", "depth", "ticker", "markPrice", "fundingRate", "openInterest", "forceOrder"]

# UI refresh rate in milliseconds
refresh_rate_ms = 100

# OrderBook depth to display (number of price levels)
orderbook_depth = 20

# Legacy sparkline flag (currently unused)
enable_sparkline = true

# Logging level (trace, debug, info, warn, error)
log_level = "info"

[log]
# Directory for hourly log files
file_path = "logs"

[binance]
# Binance WebSocket URL
ws_url = "wss://stream.binance.com:9443"

# Binance REST API URL  
rest_url = "https://api.binance.com"

# Request timeout in seconds
timeout_seconds = 10

# Reconnect interval in milliseconds
reconnect_interval_ms = 1000

# Maximum reconnection attempts
max_reconnect_attempts = 10

[binance.perp_usdt]
ws_url = "wss://fstream.binance.com"
rest_url = "https://fapi.binance.com"

[ui]
# Enable colors in terminal output
enable_colors = true

# TUI update rate in FPS
update_rate_fps = 20

# Sparkline history points
sparkline_points = 60

# Minimum seconds between daily kline redraws
kline_refresh_secs = 60
```

### Configuration Options

#### Global Settings

- `symbols`: Array of trading symbols (applies to the `--market` selection)
- `markets`: Explicit multi-market subscriptions (`exchange`, `market_type` = `spot` or `perp_usdt`, `symbols`, optional `streams`)
- `refresh_rate_ms`: UI refresh interval in milliseconds (100-1000 recommended)
- `orderbook_depth`: Number of price levels to display in orderbook (10-50)
- `enable_sparkline`: Legacy flag (currently unused)
- `log_level`: Logging verbosity level
- `log.file_path`: Directory for hourly log files (prefix `xtrade.log`)

#### Binance Settings

- `ws_url`: Binance WebSocket endpoint
- `rest_url`: Binance REST API endpoint
- `timeout_seconds`: HTTP request timeout
- `reconnect_interval_ms`: Delay between reconnection attempts
- `max_reconnect_attempts`: Maximum reconnection attempts before giving up
- `binance.perp_usdt.*`: Optional overrides for USDT-M perp endpoints

**Implementation Status**: Binance REST API and WebSocket clients are fully implemented with connection management, error handling, and reconnection logic.

#### UI Settings

- `enable_colors`: Enable colored terminal output
- `update_rate_fps`: TUI refresh rate in frames per second
- `sparkline_points`: Number of historical points cached for price history
- `kline_refresh_secs`: Minimum seconds between daily kline redraws

### Example Configurations

#### Minimal Configuration

```toml
symbols = ["BTCUSDT"]
refresh_rate_ms = 500
log_level = "info"

[log]
file_path = "logs"
```

#### Perp Configuration

```toml
[[markets]]
exchange = "binance"
market_type = "perp_usdt"
symbols = ["BTCUSDT", "ETHUSDT"]
streams = ["aggTrade", "depth", "ticker", "markPrice", "fundingRate", "openInterest", "forceOrder"]

[binance.perp_usdt]
ws_url = "wss://fstream.binance.com"
rest_url = "https://fapi.binance.com"
```

## Environment Variables

XTrade supports environment variables to override configuration settings. Environment variables take precedence over config file values.

### Available Environment Variables

```bash
# Trading symbols (comma-separated)
export XTRADE_SYMBOLS=BTCUSDT,ETHUSDT,BNBUSDT

# Refresh rate in milliseconds
export XTRADE_REFRESH_RATE_MS=200

# Orderbook depth
export XTRADE_ORDERBOOK_DEPTH=25

# Log level
export XTRADE_LOG_LEVEL=debug

# Log directory (hourly rotation)
export XTRADE_LOG_FILE_PATH=/var/log

# Binance WebSocket URL
export XTRADE_BINANCE_WS_URL=wss://stream.binance.com:9443

# Binance REST API URL
export XTRADE_BINANCE_REST_URL=https://api.binance.com

# Request timeout in seconds
export XTRADE_BINANCE_TIMEOUT_SECONDS=10

# Reconnect interval in milliseconds
export XTRADE_BINANCE_RECONNECT_INTERVAL_MS=1000

# Maximum reconnection attempts
export XTRADE_BINANCE_MAX_RECONNECT_ATTEMPTS=10

# Perp WebSocket URL override
export XTRADE_BINANCE_PERP_WS_URL=wss://fstream.binance.com

# Perp REST API URL override
export XTRADE_BINANCE_PERP_REST_URL=https://fapi.binance.com

# Enable legacy sparkline flag
export XTRADE_ENABLE_SPARKLINE=true

# Enable colors in UI
export XTRADE_UI_ENABLE_COLORS=true

# UI update rate in FPS
export XTRADE_UI_UPDATE_RATE_FPS=20

# Sparkline history points
export XTRADE_UI_SPARKLINE_POINTS=60

# Minimum seconds between kline redraws
export XTRADE_UI_KLINE_REFRESH_SECS=60
```

### Usage Examples

```bash
# Temporary configuration override
XTRADE_SYMBOLS=BTCUSDT,ETHUSDT XTRADE_LOG_LEVEL=debug xtrade ui

# Persistent configuration
export XTRADE_SYMBOLS=BTCUSDT,ETHUSDT
export XTRADE_REFRESH_RATE_MS=150
xtrade ui
```

## TUI Keyboard Shortcuts

When using the Terminal User Interface (`xtrade ui`), the following keyboard shortcuts are available:

### Navigation and Command Palette

- `/` or `:`: Open the command palette
- `←` / `→` / `↑` / `↓`: Switch between symbol tabs
- `j` / `k`: Scroll through logs
- `A` (Shift + A): Open alerts list (also runs `/alert:list`)

### Control

- `q`, `Ctrl+C`, or `Ctrl+D`: Quit the application
- `p`, `Space`, or `Ctrl+P`: Toggle pause state (currently UI indicator only)
- `Esc`: Exit command/alert modes

### Quick Actions

- `s`: Prefill `/status` in the command palette
- `L` (Shift + L): Prefill `/logs` in the command palette
- `a`: Open the alert popup for the current symbol

### Alerts View

- `j` / `k` or `↑` / `↓`: Move selection
- `d` or `Delete`: Remove selected alert
- `C` (Shift + C): Clear all alerts
- `r`: Refresh alert list
- `q` or `Esc`: Exit alerts view

### Alert Popup

- `↑` / `↓`: Cycle through fields
- `Tab`: Toggle direction/mode when the field is active
- `Enter`: Submit alert
- `Esc`: Cancel

## Troubleshooting

**Implementation Status**: Troubleshooting information is based on actual error handling and debugging experience with the current implementation.

### Common Issues

#### Connection Issues

```bash
# Check if Binance API is accessible
curl https://api.binance.com/api/v3/ping

# Check WebSocket connectivity
# (This requires websocat or similar tool)
```

**Solutions**:

- Verify internet connection
- Check firewall settings
- Try different DNS servers
- Use `--log-level debug` for detailed connection logs
- Check Binance API status page for service outages

#### Performance Issues

**Symptoms**: High CPU usage, laggy UI, delayed updates

**Solutions**:

- Reduce `refresh_rate_ms` in configuration
- Monitor fewer symbols
- Increase `orderbook_depth` only if needed
- Use `--log-level warn` to reduce logging overhead

#### Memory Issues

**Symptoms**: High memory usage, application crashes

**Solutions**:

- Reduce `sparkline_points` in UI configuration
- Monitor fewer symbols concurrently
- Restart application periodically for long-running sessions

### Logging and Debugging

```bash
# Enable debug logging
xtrade --log-level debug ui

# Enable trace logging for maximum detail
xtrade --log-level trace ui

# Logs are written to the directory in log.file_path (hourly rotation)
XTRADE_LOG_FILE_PATH=logs xtrade --log-level debug ui
```

### Common Error Messages

- **"Failed to connect to Binance"**: Network or firewall issue
- **"Invalid symbol format"**: Trading symbol format incorrect
- **"Configuration validation failed"**: Invalid config values
- **"WebSocket connection closed"**: Network interruption or Binance API issue

## Performance Tips

**Implementation Status**: Performance characteristics are based on actual implementation testing with the current codebase.

### Optimal Configuration

For best performance, use these recommended settings:

```toml
# For low-latency trading
refresh_rate_ms = 50
orderbook_depth = 20
log_level = "warn"

[binance]
timeout_seconds = 5
reconnect_interval_ms = 500

[ui]
update_rate_fps = 30
sparkline_points = 30
```

### Resource Management

- **CPU**: Each symbol subscription uses ~1-2% CPU (based on actual WebSocket processing)
- **Memory**: ~5-10MB per symbol for orderbook data (BTreeMap-based storage)
- **Network**: ~1-2KB/s per symbol for WebSocket data (Binance stream optimization)

### Monitoring Performance

```bash
# Check system resources
top  # Linux/macOS
taskmgr  # Windows

# Monitor network connections
netstat -an | grep 9443  # Linux/macOS
netstat -an | find "9443"  # Windows
```

## Development Usage

### Building from Source

```bash
# Clone repository
git clone https://github.com/your-username/xtrade.git
cd xtrade

# Build in debug mode (for development)
cargo build

# Build in release mode (for production)
cargo build --release

# Run directly with cargo
cargo run -- ui
```

### Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_subscription_logic

# Run integration tests
cargo test --test integration_test
```

### Linting and Formatting

```bash
# Run clippy lint checks
cargo clippy

# Format code
cargo fmt

# Check formatting without applying
cargo fmt --check
```

### Documentation

```bash
# Build and open documentation
cargo doc --open

# Run documentation tests
cargo test --doc
```

### Debugging

```bash
# Run with debug logging
RUST_LOG=debug cargo run -- ui

# Run with backtrace on error
RUST_BACKTRACE=1 cargo run -- ui

# Profile CPU usage
cargo flamegraph --bin xtrade -- ui
```

## Support and Resources

- **GitHub Repository**: <https://github.com/your-username/xtrade>
- **Issue Tracker**: <https://github.com/your-username/xtrade/issues>
- **Documentation**: <https://github.com/your-username/xtrade/docs>
- **Binance API Documentation**: <https://binance-docs.github.io/apidocs/spot/en/>

## Version Information

- **Current Version**: 0.1.0 (Development)
- **Rust Version**: 1.70+
- **License**: MIT/Apache-2.0

---

*This documentation reflects the current implementation status of XTrade. The system supports real-time market data processing, TUI visualization, alerts, and configuration management. Future development will focus on trading workflows, alert persistence, and data storage.*
