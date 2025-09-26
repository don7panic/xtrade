use colored::Colorize;
use xtrade::cli::{Commands, ConfigAction};
use xtrade::{AppResult, cli::Cli, init_logging};

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse_args();

    // Initialize logging
    init_logging(&cli.effective_log_level())?;

    tracing::info!("XTrade Market Data Monitor starting...");
    tracing::debug!("CLI arguments: {:?}", cli);

    // Handle commands
    match cli.command {
        Commands::Subscribe { symbols } => handle_subscribe(symbols).await,
        Commands::Unsubscribe { symbols } => handle_unsubscribe(symbols).await,
        Commands::List => handle_list().await,
        Commands::Ui { simple } => handle_ui(simple).await,
        Commands::Status => handle_status().await,
        Commands::Show { symbol } => handle_show(symbol).await,
        Commands::Config { action } => handle_config(action, &cli.config_file).await,
        Commands::Demo => demo_websocket().await,
    }
}

async fn handle_subscribe(symbols: Vec<String>) -> AppResult<()> {
    tracing::info!("Subscribing to symbols: {:?}", symbols);

    // TODO: Implement subscription logic
    println!("📈 Subscribing to market data for: {}", symbols.join(", "));
    println!("💡 This feature will be implemented in Week 2 of the sprint plan.");

    Ok(())
}

async fn handle_unsubscribe(symbols: Vec<String>) -> AppResult<()> {
    tracing::info!("Unsubscribing from symbols: {:?}", symbols);

    // TODO: Implement unsubscription logic
    println!(
        "📉 Unsubscribing from market data for: {}",
        symbols.join(", ")
    );
    println!("💡 This feature will be implemented in Week 2 of the sprint plan.");

    Ok(())
}

async fn handle_list() -> AppResult<()> {
    tracing::info!("Listing subscribed symbols");

    // TODO: Implement list logic
    println!("📋 Currently subscribed symbols:");
    println!("💡 This feature will be implemented in Week 2 of the sprint plan.");
    println!("    (No active subscriptions yet)");

    Ok(())
}

async fn handle_ui(simple: bool) -> AppResult<()> {
    tracing::info!("Starting UI mode, simple: {}", simple);

    if simple {
        // TODO: Implement simple CLI output
        println!("🖥️  Starting simple CLI mode...");
        println!("💡 This feature will be implemented in Week 3 of the sprint plan.");
    } else {
        // TODO: Implement full TUI
        println!("🎨 Starting Terminal UI (TUI) mode...");
        println!("💡 This feature will be implemented in Week 3 of the sprint plan.");
    }

    Ok(())
}

async fn handle_status() -> AppResult<()> {
    tracing::info!("Showing status information");

    // TODO: Implement status display
    println!("🔍 XTrade Status:");
    println!("   Version: {}", env!("CARGO_PKG_VERSION"));
    println!("   Status: Not connected");
    println!("   Active subscriptions: 0");
    println!("   💡 Full status reporting will be implemented in Week 2 of the sprint plan.");

    Ok(())
}

async fn handle_show(symbol: String) -> AppResult<()> {
    tracing::info!("Showing details for symbol: {}", symbol);

    // TODO: Implement symbol detail display
    println!("📊 Showing details for: {}", symbol);
    println!("💡 This feature will be implemented in Week 2-3 of the sprint plan.");

    Ok(())
}

/// Demo function showing WebSocket usage (for testing)
async fn demo_websocket() -> AppResult<()> {
    // 使用重构后的模块化实现
    xtrade::binance::demo::demo_websocket().await
}

async fn handle_config(action: Option<ConfigAction>, config_file: &str) -> AppResult<()> {
    tracing::info!("Handling config action: {:?}", action);

    match action {
        Some(ConfigAction::Show) => {
            let config = xtrade::config::Config::load_or_default(config_file);
            println!("⚙️  Configuration from: {}", config_file);
            println!("{} = {:?}", "symbols".bold(), config.symbols);
            println!(
                "{} = {} ms",
                "refresh_rate_ms".bold(),
                config.refresh_rate_ms
            );
            println!("{} = {}", "orderbook_depth".bold(), config.orderbook_depth);
            println!(
                "{} = {}",
                "enable_sparkline".bold(),
                config.enable_sparkline
            );
            println!("{} = {}", "log_level".bold(), config.log_level);
            println!("\n📊 Binance Configuration:");
            println!("{} = {}", "ws_url".bold(), config.binance.ws_url);
            println!("{} = {}", "rest_url".bold(), config.binance.rest_url);
            println!(
                "  {} = {} s",
                "timeout_seconds".bold(),
                config.binance.timeout_seconds
            );
            println!(
                "{} = {} ms",
                "reconnect_interval_ms".bold(),
                config.binance.reconnect_interval_ms
            );
            println!(
                "{} = {}",
                "max_reconnect_attempts".bold(),
                config.binance.max_reconnect_attempts
            );
            println!("\n🎨 UI Configuration:");
            println!("{} = {}", "enable_colors".bold(), config.ui.enable_colors);
            println!(
                "{} = {} FPS",
                "update_rate_fps".bold(),
                config.ui.update_rate_fps
            );
            println!(
                "{} = {}",
                "sparkline_points".bold(),
                config.ui.sparkline_points
            );
        }
        Some(ConfigAction::Set { key, value }) => {
            println!("⚙️  Setting {}={}", key, value);
            println!(
                "💡 Configuration modification via CLI will be implemented in future versions."
            );
            println!(
                "   For now, please edit the config file directly: {}",
                config_file
            );
        }
        Some(ConfigAction::Reset) => {
            let default_config = xtrade::config::Config::default();
            default_config.save_to_file(config_file)?;
            println!(
                "✅ Configuration reset to defaults and saved to: {}",
                config_file
            );
        }
        None => {
            println!("⚙️  Configuration management");
            println!("Use 'xtrade config show' to view current configuration");
            println!("Use 'xtrade config set <key> <value>' to modify settings");
            println!("Use 'xtrade config reset' to restore defaults");
            println!("\n📝 Environment variables can override config:");
            println!("  XTRADE_SYMBOLS=ETHUSDT,BTCUSDT");
            println!("  XTRADE_REFRESH_RATE_MS=200");
            println!("  XTRADE_LOG_LEVEL=debug");
        }
    }

    Ok(())
}
