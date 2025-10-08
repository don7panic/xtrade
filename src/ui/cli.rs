//! Simple CLI output implementation
//!
//! Provides simple command-line output for market data.

use super::{AppState, MarketDataState};
use crate::AppResult;
use crate::session::session_manager::SessionStats;

/// Display market data in simple CLI format
pub fn display_market_data(symbol: &str, data: &MarketDataState) -> AppResult<()> {
    println!("📊 {}", symbol);
    println!(
        "   Price: ${:.2} ({:+.2}%)",
        data.price, data.change_percent
    );
    println!("   24h High: ${:.2}", data.high_24h);
    println!("   24h Low:  ${:.2}", data.low_24h);
    println!("   Volume: {:.2}", data.volume_24h);

    if let Some(ref orderbook) = data.orderbook {
        println!("   Best Ask: ${:.2}", orderbook.best_ask().unwrap_or(0.0));
        println!("   Best Bid: ${:.2}", orderbook.best_bid().unwrap_or(0.0));
    }

    Ok(())
}

/// Display connection status in CLI format
pub fn display_status(app: &AppState) -> AppResult<()> {
    println!("🔍 XTrade Status:");
    println!("   Connection: {:?}", app.connection_metrics.status);
    println!("   Subscriptions: {}", app.symbols.len());
    println!("   Active symbols: {}", app.symbols.join(", "));
    println!("   Latency P95: {}ms", app.connection_metrics.latency_p95);
    println!(
        "   Messages/sec: {:.1}",
        app.connection_metrics.messages_per_second
    );
    println!("   Reconnects: {}", app.connection_metrics.reconnect_count);

    if app.paused {
        println!("   ⏸️  Data feed is PAUSED");
    }

    Ok(())
}

/// Display list of subscribed symbols
pub fn display_symbol_list(symbols: &[String]) -> AppResult<()> {
    println!("📋 Subscribed symbols:");
    if symbols.is_empty() {
        println!("   (No active subscriptions)");
    } else {
        for (i, symbol) in symbols.iter().enumerate() {
            println!("   {}. {}", i + 1, symbol);
        }
    }

    Ok(())
}

/// Render CLI dashboard with consistent formatting matching welcome page
pub fn render_cli_dashboard(
    app_state: &AppState,
    render_state: &crate::ui::ui_manager::RenderState,
    session_stats: SessionStats,
) -> AppResult<()> {
    println!();
    println!("┌─ XTrade Market Data Monitor ────────────────────────────────────────┐");
    println!("│                                                                     │");

    // Render symbols section
    if app_state.symbols.is_empty() {
        println!("│   Active Subscriptions: None                                         │");
    } else {
        let symbols_str = app_state.symbols.join(", ");
        println!("│   Active Subscriptions: {:<50} │", symbols_str);
    }
    println!("│                                                                     │");

    // Render status section
    println!(
        "│   Status: Running | Render count: {:<35} │",
        render_state.render_count
    );
    println!(
        "│   Commands processed: {:<45} │",
        session_stats.commands_processed
    );
    println!(
        "│   Events processed: {:<46} │",
        session_stats.events_processed
    );
    println!("│                                                                     │");

    // Render messages section
    if let Some(error) = &render_state.error_message {
        println!("│   Error: {:<58} │", error);
    }

    if let Some(info) = &render_state.info_message {
        println!("│   Info: {:<59} │", info);
    }

    if render_state.error_message.is_some() || render_state.info_message.is_some() {
        println!("│                                                                     │");
    }

    // Render commands section
    println!("│   Commands:                                                        │");
    println!("│   • /add <symbols> - Subscribe to symbols                          │");
    println!("│   • /remove <symbols> - Unsubscribe from symbols                   │");
    println!("│   • /list - Show current subscriptions                              │");
    println!("│   • /show <symbol> - Show details for symbol                        │");
    println!("│   • /status - Show session statistics                               │");
    println!("│   • /logs - Show recent logs                                        │");
    println!("│   • /config show - Show configuration                               │");
    println!("│   • /quit - Exit the application                                    │");
    println!("│                                                                     │");
    println!("└────────────────────────────────────────────────────────────────────┘");
    println!();

    Ok(())
}
