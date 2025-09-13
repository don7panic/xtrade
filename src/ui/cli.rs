//! Simple CLI output implementation
//!
//! Provides simple command-line output for market data.

use super::{AppState, MarketDataState};
use crate::AppResult;

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
