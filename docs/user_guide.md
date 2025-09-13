# XTrade User Guide

XTrade is a high-performance cryptocurrency market data monitoring system built in Rust with a focus on Binance exchange integration. This guide provides comprehensive documentation for installing, configuring, and using XTrade.

## Table of Contents

1. [Installation and Prerequisites](#installation-and-prerequisites)
2. [Command Line Interface](#command-line-interface)
3. [Global Flags](#global-flags)
4. [Subcommands](#subcommands)
5. [Configuration File](#configuration-file)
6. [Environment Variables](#environment-variables)
7. [TUI Keyboard Shortcuts](#tui-keyboard-shortcuts)
8. [Troubleshooting](#troubleshooting)
9. [Performance Tips](#performance-tips)
10. [Development Usage](#development-usage)

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
xtrade [GLOBAL_FLAGS] COMMAND [COMMAND_FLAGS]
```

### Getting Help

```bash
# General help
xtrade --help

# Command-specific help
xtrade subscribe --help
xtrade config --help
```

## Global Flags

XTrade provides several global flags that apply to all commands:

### `--config-file <PATH>`

Specify a custom configuration file path. Defaults to `config.toml` in the current directory.

```bash
xtrade --config-file /path/to/custom/config.toml subscribe BTCUSDT
xtrade --config-file ~/.config/xtrade/config.toml status
```

### `--log-level <LEVEL>`

Set the logging level. Available levels: `trace`, `debug`, `info`, `warn`, `error`. Default: `info`.

```bash
xtrade --log-level debug subscribe BTCUSDT
xtrade --log-level trace ui
```

### `--verbose`

Enable verbose output (equivalent to `--log-level debug`). This flag takes precedence over `--log-level`.

```bash
xtrade --verbose subscribe BTCUSDT
xtrade --verbose --log-level info status  # Uses debug level due to --verbose
```

## Subcommands

### `subscribe` - Subscribe to Market Data

Subscribe to one or more trading symbols for real-time market data monitoring.

```bash
# Subscribe to single symbol
xtrade subscribe BTCUSDT

# Subscribe to multiple symbols
xtrade subscribe BTCUSDT ETHUSDT BNBUSDT

# Subscribe with custom config
xtrade --config-file custom.toml subscribe BTCUSDT
```

**Note**: This feature is planned for Week 2 implementation.

### `unsubscribe` - Unsubscribe from Market Data

Unsubscribe from one or more trading symbols.

```bash
# Unsubscribe from single symbol
xtrade unsubscribe BTCUSDT

# Unsubscribe from multiple symbols
xtrade unsubscribe ETHUSDT BNBUSDT
```

**Note**: This feature is planned for Week 2 implementation.

### `list` - List Subscribed Symbols

Display currently subscribed trading symbols.

```bash
xtrade list
```

**Output**:

```text
📋 Currently subscribed symbols:
- BTCUSDT
- ETHUSDT
- BNBUSDT
```

**Note**: This feature is planned for Week 2 implementation.

### `ui` - Start Terminal User Interface

Launch the interactive terminal user interface for real-time market data visualization.

```bash
# Start full TUI mode
xtrade ui

# Start simple CLI mode
xtrade ui --simple
```

**Options**:

- `--simple`: Use simple CLI output instead of full TUI

**Note**: This feature is planned for Week 3 implementation.

### `status` - Show System Status

Display system status including connection information and active subscriptions.

```bash
xtrade status
```

**Output**:

```text
🔍 XTrade Status:
   Version: 0.1.0
   Status: Connected to Binance
   Active subscriptions: 3
   Uptime: 2h 15m
   Messages processed: 12,456
   Average latency: 45ms
```

**Note**: This feature is planned for Week 2 implementation.

### `show` - Show Symbol Details

Display detailed information for a specific trading symbol.

```bash
# Show BTCUSDT details
xtrade show BTCUSDT

# Show ETHUSDT details
xtrade show ETHUSDT
```

**Output**:

```sh
📊 BTCUSDT Details:
   Current Price: $42,123.45
   24h Change: +2.34%
   24h Volume: $1.2B
   Bid/Ask Spread: $0.50
   OrderBook Depth: 20 levels
```

**Note**: This feature is planned for Week 2-3 implementation.

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

**Note**: CLI configuration modification is planned for future versions. Currently, edit the config file directly.

#### Reset Configuration

```bash
# Reset to default values
xtrade config reset
```

## Configuration File

XTrade uses TOML format configuration files. The default configuration file is `config.toml` in the current working directory.

### Default Configuration Location

- **Linux/macOS**: `./config.toml` or `~/.config/xtrade/config.toml`
- **Windows**: `config.toml` or `%APPDATA%\xtrade\config.toml`

### Configuration Structure

```toml
# XTrade Configuration File

# Trading symbols to monitor by default
symbols = ["BTCUSDT", "ETHUSDT", "BNBUSDT"]

# UI refresh rate in milliseconds
refresh_rate_ms = 100

# OrderBook depth to display (number of price levels)
orderbook_depth = 20

# Enable price sparkline charts in TUI
enable_sparkline = true

# Logging level (trace, debug, info, warn, error)
log_level = "info"

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

[ui]
# Enable colors in terminal output
enable_colors = true

# TUI update rate in FPS
update_rate_fps = 20

# Sparkline history points
sparkline_points = 60
```

### Configuration Options

#### Global Settings

- `symbols`: Array of trading symbols to monitor (e.g., `["BTCUSDT", "ETHUSDT"]`)
- `refresh_rate_ms`: UI refresh interval in milliseconds (100-1000 recommended)
- `orderbook_depth`: Number of price levels to display in orderbook (10-50)
- `enable_sparkline`: Enable/disable price sparkline charts
- `log_level`: Logging verbosity level

#### Binance Settings

- `ws_url`: Binance WebSocket endpoint
- `rest_url`: Binance REST API endpoint
- `timeout_seconds`: HTTP request timeout
- `reconnect_interval_ms`: Delay between reconnection attempts
- `max_reconnect_attempts`: Maximum reconnection attempts before giving up

#### UI Settings

- `enable_colors`: Enable colored terminal output
- `update_rate_fps`: TUI refresh rate in frames per second
- `sparkline_points`: Number of historical points for sparkline charts

### Example Configurations

#### Minimal Configuration

```toml
symbols = ["BTCUSDT"]
refresh_rate_ms = 500
log_level = "info"
```

#### High-Frequency Trading Configuration

```toml
symbols = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT"]
refresh_rate_ms = 50
orderbook_depth = 30
enable_sparkline = true
log_level = "warn"

[binance]
timeout_seconds = 5
reconnect_interval_ms = 500
max_reconnect_attempts = 20

[ui]
update_rate_fps = 30
sparkline_points = 120
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

# Enable colors in UI
export XTRADE_UI_ENABLE_COLORS=true

# UI update rate in FPS
export XTRADE_UI_UPDATE_RATE_FPS=20

# Sparkline history points
export XTRADE_UI_SPARKLINE_POINTS=60
```

### Usage Examples

```bash
# Temporary configuration override
XTRADE_SYMBOLS=BTCUSDT,ETHUSDT XTRADE_LOG_LEVEL=debug xtrade subscribe

# Persistent configuration
export XTRADE_SYMBOLS=BTCUSDT,ETHUSDT
export XTRADE_REFRESH_RATE_MS=150
xtrade ui
```

## TUI Keyboard Shortcuts

When using the Terminal User Interface (`xtrade ui`), the following keyboard shortcuts are available:

### Navigation

- `Tab` / `Shift+Tab`: Switch between symbol tabs
- `←` / `→`: Navigate between orderbook sections
- `↑` / `↓`: Scroll through orderbook levels

### Control

- `q` or `Esc`: Quit the application
- `r`: Force reconnect to Binance
- `p`: Pause/resume data updates
- `s`: Save current snapshot to file
- `h`: Show help screen

### View Management

- `+` / `-`: Increase/decrease orderbook depth
- `c`: Toggle color mode
- `f`: Toggle fullscreen mode

### Data Display

- `1`-`9`: Switch to specific symbol tab
- `a`: Show/hide asks (sell orders)
- `b`: Show/hide bids (buy orders)
- `l`: Show/hide latency statistics

## Troubleshooting

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
xtrade --log-level debug subscribe BTCUSDT

# Enable trace logging for maximum detail
xtrade --log-level trace ui

# Log to file
xtrade --log-level debug subscribe BTCUSDT 2> xtrade.log
```

### Common Error Messages

- **"Failed to connect to Binance"**: Network or firewall issue
- **"Invalid symbol format"**: Trading symbol format incorrect
- **"Configuration validation failed"**: Invalid config values
- **"WebSocket connection closed"**: Network interruption or Binance API issue

## Performance Tips

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

- **CPU**: Each symbol subscription uses ~1-2% CPU
- **Memory**: ~5-10MB per symbol for orderbook data
- **Network**: ~1-2KB/s per symbol for WebSocket data

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
cargo run -- subscribe BTCUSDT
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
RUST_LOG=debug cargo run -- subscribe BTCUSDT

# Run with backtrace on error
RUST_BACKTRACE=1 cargo run -- subscribe BTCUSDT

# Profile CPU usage
cargo flamegraph --bin xtrade -- subscribe BTCUSDT
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

*This documentation is based on the current development state. Features marked as "planned" are scheduled for implementation according to the sprint timeline.*
