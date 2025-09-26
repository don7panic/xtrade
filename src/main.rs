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
    use xtrade::binance::BinanceWebSocket;
    use xtrade::binance::rest::BinanceRestClient;
    use xtrade::binance::types::{OrderBook, OrderBookUpdate};

    println!("🔌 Testing Binance WebSocket OrderBook incremental updates...");

    // Create WebSocket client and get message receiver
    let (ws, mut message_rx) = BinanceWebSocket::new("wss://stream.binance.com:9443/ws");

    // Create REST client for fetching initial snapshot
    let rest_client = BinanceRestClient::new("https://api.binance.com".to_string());

    // Check initial status
    let status = ws.status().await;
    println!("📡 Initial status: {:?}", status);

    // Try to connect
    match ws.connect().await {
        Ok(()) => {
            println!("✅ Connected successfully!");

            // Start listening for messages
            ws.start_listening().await?;
            println!("👂 Started listening for messages...");

            // Create OrderBook and fetch initial snapshot
            let mut orderbook = OrderBook::new("BTCUSDT".to_string());
            println!("📊 Fetching initial OrderBook snapshot for BTCUSDT...");

            match orderbook.fetch_snapshot(&rest_client).await {
                Ok(()) => {
                    println!("✅ OrderBook snapshot fetched successfully!");
                    println!("   📈 Best bid: {:?}", orderbook.best_bid());
                    println!("   📉 Best ask: {:?}", orderbook.best_ask());
                    println!("   📏 Spread: {:?}", orderbook.spread());
                    println!(
                        "   🏗️  Levels: bids={}, asks={}",
                        orderbook.bids.len(),
                        orderbook.asks.len()
                    );
                    println!("   🔢 Last update ID: {}", orderbook.last_update_id);
                }
                Err(e) => {
                    println!("❌ Failed to fetch snapshot: {}", e);
                    return Ok(());
                }
            }

            // Subscribe to BTCUSDT depth stream at 100ms updates
            println!("📈 Subscribing to BTCUSDT depth stream (100ms updates)...");
            ws.subscribe_depth("BTCUSDT", Some(100)).await?;

            // Wait for and process depth updates
            println!("⏳ Processing depth updates for 10 seconds...");

            let start_time = std::time::Instant::now();
            let mut message_count = 0;
            let mut update_count = 0;
            let mut error_count = 0;

            while start_time.elapsed() < std::time::Duration::from_secs(10) {
                if let Some(message_result) = message_rx.recv().await {
                    message_count += 1;

                    match message_result {
                        Ok(message) => {
                            // Check if this is a depth update message
                            if message.stream.contains("@depth") {
                                // Try to parse as OrderBookUpdate
                                match serde_json::from_value::<OrderBookUpdate>(message.data) {
                                    Ok(depth_update) => {
                                        update_count += 1;

                                        // Apply the update to our OrderBook
                                        match orderbook.apply_depth_update(depth_update) {
                                            Ok(()) => {
                                                if update_count <= 5 || update_count % 10 == 0 {
                                                    println!(
                                                        "✅ Update #{}: bid={:?}, ask={:?}, spread={:?}, levels={}",
                                                        update_count,
                                                        orderbook.best_bid(),
                                                        orderbook.best_ask(),
                                                        orderbook.spread(),
                                                        orderbook.total_levels()
                                                    );
                                                }

                                                // Validate consistency every 10 updates
                                                if update_count % 10 == 0 {
                                                    match orderbook.validate_consistency() {
                                                        Ok(()) => {}
                                                        Err(e) => {
                                                            println!(
                                                                "⚠️  Consistency check failed: {}",
                                                                e
                                                            );
                                                            error_count += 1;
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error_count += 1;
                                                use xtrade::binance::types::OrderBookError;

                                                match &e {
                                                    OrderBookError::StaleMessage { .. } => {
                                                        if error_count <= 3 {
                                                            println!(
                                                                "ℹ️  Stale message (expected): {}",
                                                                e
                                                            );
                                                        }
                                                    }
                                                    _ => {
                                                        println!(
                                                            "❌ OrderBook update error: {}",
                                                            e
                                                        );
                                                        println!("   Severity: {:?}", e.severity());
                                                        println!(
                                                            "   Recoverable: {}",
                                                            e.is_recoverable()
                                                        );
                                                        println!(
                                                            "   Requires resync: {}",
                                                            e.requires_resync()
                                                        );

                                                        if e.requires_resync() {
                                                            println!(
                                                                "🔄 Re-fetching snapshot due to error..."
                                                            );
                                                            if let Err(snapshot_err) = orderbook
                                                                .fetch_snapshot(&rest_client)
                                                                .await
                                                            {
                                                                println!(
                                                                    "❌ Failed to re-fetch snapshot: {}",
                                                                    snapshot_err
                                                                );
                                                            } else {
                                                                println!(
                                                                    "✅ Snapshot re-fetched successfully"
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        if error_count <= 3 {
                                            println!("❌ Failed to parse depth update: {}", e);
                                        }
                                        error_count += 1;
                                    }
                                }
                            } else {
                                // Non-depth message (response, etc.)
                                if message_count <= 3 {
                                    println!("📨 Non-depth message: {}", message.stream);
                                }
                            }
                        }
                        Err(error) => {
                            error_count += 1;
                            if error_count <= 3 {
                                println!("❌ Error receiving message: {}", error);
                            }
                        }
                    }
                } else {
                    // No messages available, sleep briefly
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }

            println!("\n📊 Test Results Summary:");
            println!("   📬 Total messages: {}", message_count);
            println!("   🔄 Depth updates processed: {}", update_count);
            println!("   ❌ Errors encountered: {}", error_count);
            println!("   📈 Final best bid: {:?}", orderbook.best_bid());
            println!("   📉 Final best ask: {:?}", orderbook.best_ask());
            println!("   📏 Final spread: {:?}", orderbook.spread());
            println!("   🏗️  Final levels: {}", orderbook.total_levels());
            println!(
                "   💰 Total bid volume: {:.2}",
                orderbook.total_bid_volume()
            );
            println!(
                "   💰 Total ask volume: {:.2}",
                orderbook.total_ask_volume()
            );

            // Performance metrics
            let updates_per_second = update_count as f64 / 10.0;
            println!("   ⚡ Updates per second: {:.1}", updates_per_second);

            if error_count == 0 {
                println!("✅ All updates processed successfully!");
            } else {
                let error_rate = (error_count as f64 / message_count as f64) * 100.0;
                println!("⚠️  Error rate: {:.1}%", error_rate);
            }

            // Unsubscribe and disconnect
            ws.unsubscribe("BTCUSDT", "depth@100ms").await?;
            println!("📉 Unsubscribed from BTCUSDT depth stream");

            ws.disconnect().await?;
            println!("🔌 Disconnected successfully");
        }
        Err(e) => {
            println!("❌ Connection failed: {}", e);
        }
    }

    Ok(())
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
