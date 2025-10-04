//! Concurrent subscription tests for XTrade

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;
use xtrade::market_data::{MarketDataManager, MarketEvent};

#[tokio::test]
async fn test_single_symbol_subscription() -> Result<()> {
    let manager = Arc::new(Mutex::new(MarketDataManager::new()));

    // Subscribe to a single symbol
    {
        let manager = manager.lock().await;
        manager.subscribe("BTCUSDT".to_string()).await?;
    }

    // Wait for initial snapshot
    let mut manager = manager.lock().await;
    let result = timeout(Duration::from_secs(10), async {
        loop {
            if let Some(event) = manager.next_event().await {
                match event {
                    MarketEvent::OrderBookUpdate { symbol, .. } if symbol == "BTCUSDT" => {
                        break;
                    }
                    MarketEvent::Error { symbol, error } => {
                        panic!("Error for {}: {}", symbol, error);
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "Should receive orderbook update within 10 seconds"
    );

    Ok(())
}

#[tokio::test]
async fn test_multiple_symbol_subscription() -> Result<()> {
    let manager = Arc::new(Mutex::new(MarketDataManager::new()));

    let symbols = vec!["BTCUSDT", "ETHUSDT", "ADAUSDT", "DOTUSDT", "LINKUSDT"];

    // Subscribe to multiple symbols
    {
        let manager = manager.lock().await;
        for symbol in &symbols {
            manager.subscribe(symbol.to_string()).await?;
        }
    }

    // Wait for initial snapshots
    let mut manager = manager.lock().await;
    let mut received_symbols = std::collections::HashSet::new();

    let result = timeout(Duration::from_secs(15), async {
        while received_symbols.len() < symbols.len() {
            if let Some(event) = manager.next_event().await {
                match event {
                    MarketEvent::OrderBookUpdate { symbol, .. } => {
                        if symbols.contains(&symbol.as_str()) {
                            received_symbols.insert(symbol);
                        }
                    }
                    MarketEvent::Error { symbol, error } => {
                        panic!("Error for {}: {}", symbol, error);
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "Should receive orderbook updates for all symbols within 15 seconds"
    );
    assert_eq!(
        received_symbols.len(),
        symbols.len(),
        "Should receive updates for all symbols"
    );

    Ok(())
}

#[tokio::test]
async fn test_subscription_error_isolation() -> Result<()> {
    let manager = Arc::new(Mutex::new(MarketDataManager::new()));

    // Subscribe to valid symbol
    {
        let manager = manager.lock().await;
        manager.subscribe("BTCUSDT".to_string()).await?;
    }

    // Try to subscribe to invalid symbol (should not affect valid subscription)
    {
        let manager = manager.lock().await;
        let result = manager.subscribe("INVALID_SYMBOL".to_string()).await;
        assert!(result.is_err(), "Should reject invalid symbol");
    }

    // Verify valid subscription still works
    let mut manager = manager.lock().await;
    let result = timeout(Duration::from_secs(10), async {
        loop {
            if let Some(event) = manager.next_event().await {
                match event {
                    MarketEvent::OrderBookUpdate { symbol, .. } if symbol == "BTCUSDT" => {
                        break;
                    }
                    MarketEvent::Error { symbol, error } => {
                        panic!("Error for {}: {}", symbol, error);
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "Valid subscription should continue working despite invalid subscription attempt"
    );

    Ok(())
}

#[tokio::test]
async fn test_concurrent_subscription_limit() -> Result<()> {
    let manager = Arc::new(Mutex::new(MarketDataManager::new()));

    // Try to subscribe to more than the limit
    let symbols: Vec<String> = (0..15).map(|i| format!("SYMBOL{}", i)).collect();

    let mut manager = manager.lock().await;

    for (i, symbol) in symbols.iter().enumerate() {
        let result = manager.subscribe(symbol.to_string()).await;

        if i < 10 {
            assert!(result.is_ok(), "Should allow first 10 subscriptions");
        } else {
            assert!(result.is_err(), "Should reject subscriptions beyond limit");
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_subscription_listing() -> Result<()> {
    let manager = Arc::new(Mutex::new(MarketDataManager::new()));

    let symbols = vec!["BTCUSDT", "ETHUSDT", "ADAUSDT"];

    // Subscribe to symbols
    {
        let manager = manager.lock().await;
        for symbol in &symbols {
            manager.subscribe(symbol.to_string()).await?;
        }
    }

    // Check subscription list
    let manager = manager.lock().await;
    let subscribed_symbols = manager.list_subscriptions().await;

    assert_eq!(
        subscribed_symbols.len(),
        symbols.len(),
        "Should have correct number of subscriptions"
    );

    for symbol in &symbols {
        assert!(
            subscribed_symbols.contains(&symbol.to_string()),
            "Should contain symbol: {}",
            symbol
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_unsubscribe_functionality() -> Result<()> {
    let manager = Arc::new(Mutex::new(MarketDataManager::new()));

    // Subscribe to symbol
    {
        let manager = manager.lock().await;
        manager.subscribe("BTCUSDT".to_string()).await?;
    }

    // Wait for initial snapshot
    {
        let mut manager = manager.lock().await;
        let result = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(event) = manager.next_event().await {
                    match event {
                        MarketEvent::OrderBookUpdate { symbol, .. } if symbol == "BTCUSDT" => {
                            break;
                        }
                        MarketEvent::Error { symbol, error } => {
                            panic!("Error for {}: {}", symbol, error);
                        }
                        _ => {}
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        assert!(result.is_ok(), "Should receive initial orderbook update");
    }

    // Unsubscribe
    {
        let manager = manager.lock().await;
        manager.unsubscribe("BTCUSDT").await?;
    }

    // Verify subscription is removed
    let manager = manager.lock().await;
    let subscribed_symbols = manager.list_subscriptions().await;

    assert!(
        subscribed_symbols.is_empty(),
        "Should have no subscriptions after unsubscribe"
    );

    Ok(())
}

#[tokio::test]
async fn test_batch_unsubscribe() -> Result<()> {
    let manager = Arc::new(Mutex::new(MarketDataManager::new()));

    let symbols = vec!["BTCUSDT", "ETHUSDT", "ADAUSDT"];

    // Subscribe to symbols
    {
        let manager = manager.lock().await;
        for symbol in &symbols {
            manager.subscribe(symbol.to_string()).await?;
        }
    }

    // Batch unsubscribe
    {
        let manager = manager.lock().await;
        manager
            .batch_unsubscribe(symbols.iter().map(|s| s.to_string()).collect())
            .await?;
    }

    // Verify all subscriptions are removed
    let manager = manager.lock().await;
    let subscribed_symbols = manager.list_subscriptions().await;

    assert!(
        subscribed_symbols.is_empty(),
        "Should have no subscriptions after batch unsubscribe"
    );

    Ok(())
}

#[tokio::test]
async fn test_subscription_stats() -> Result<()> {
    let manager = Arc::new(Mutex::new(MarketDataManager::new()));

    let symbols = vec!["BTCUSDT", "ETHUSDT"];

    // Subscribe to symbols
    {
        let manager = manager.lock().await;
        for symbol in &symbols {
            manager.subscribe(symbol.to_string()).await?;
        }
    }

    // Get stats
    let manager = manager.lock().await;
    let stats = manager.get_subscription_stats().await;

    assert_eq!(
        stats.total_subscriptions,
        symbols.len(),
        "Should have correct subscription count"
    );
    assert_eq!(
        stats.symbols.len(),
        symbols.len(),
        "Should have correct symbol count"
    );
    assert!(
        stats.memory_usage_estimate > 0,
        "Should have positive memory usage estimate"
    );

    Ok(())
}
