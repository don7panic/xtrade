use ordered_float::OrderedFloat;
use xtrade::binance::types::{OrderBook, OrderBookError, OrderBookUpdate};

fn make_update(
    symbol: &str,
    event_time: u64,
    first_id: u64,
    final_id: u64,
    bids: Vec<(&str, &str)>,
    asks: Vec<(&str, &str)>,
) -> OrderBookUpdate {
    OrderBookUpdate {
        event_type: "depthUpdate".to_string(),
        event_time,
        symbol: symbol.to_string(),
        first_update_id: first_id,
        final_update_id: final_id,
        bids: bids
            .into_iter()
            .map(|(p, q)| [p.to_string(), q.to_string()])
            .collect(),
        asks: asks
            .into_iter()
            .map(|(p, q)| [p.to_string(), q.to_string()])
            .collect(),
    }
}

#[test]
fn best_prices_update_correctly_on_incremental_changes() {
    let symbol = "TESTUSDT".to_string();
    let mut ob = OrderBook::new(symbol.clone());

    // Seed snapshot-like state
    ob.bids.insert(OrderedFloat(99.5), 1.0);
    ob.bids.insert(OrderedFloat(100.0), 1.2);
    ob.asks.insert(OrderedFloat(100.5), 0.8);
    ob.asks.insert(OrderedFloat(101.0), 2.3);
    ob.last_update_id = 10;

    // Apply incremental update: improve best bid; modify best ask upwards (still > best bid)
    let update = make_update(
        &symbol,
        1_000,
        11,
        12,
        vec![("100.8", "0.5")],                   // new best bid
        vec![("100.5", "0.0"), ("100.9", "1.0")], // remove old best ask, set new ask
    );

    ob.apply_depth_update(update)
        .expect("depth update should succeed");

    let best_bid = ob.best_bid().unwrap();
    let best_ask = ob.best_ask().unwrap();

    assert!(
        (best_bid - 100.8).abs() < 1e-9,
        "best bid should be updated to 100.8"
    );
    assert!(
        (best_ask - 100.9).abs() < 1e-9,
        "best ask should be updated to 100.9"
    );
    assert!(best_bid < best_ask, "spread must remain positive");
}

#[test]
fn zero_quantity_deletes_and_best_price_rolls_back() {
    let symbol = "TESTUSDT".to_string();
    let mut ob = OrderBook::new(symbol.clone());

    ob.bids.insert(OrderedFloat(99.0), 1.0);
    ob.bids.insert(OrderedFloat(100.0), 2.0); // best bid
    ob.asks.insert(OrderedFloat(101.0), 1.0);
    ob.last_update_id = 20;

    // Delete best bid level (quantity = 0)
    let update = make_update(&symbol, 2_000, 21, 21, vec![("100.0", "0.0")], vec![]);
    ob.apply_depth_update(update)
        .expect("delete update should succeed");

    let best_bid = ob.best_bid().unwrap();
    assert!(
        (best_bid - 99.0).abs() < 1e-9,
        "best bid should roll back to next level"
    );
}

#[test]
fn stale_message_is_rejected_and_state_unchanged() {
    let symbol = "TESTUSDT".to_string();
    let mut ob = OrderBook::new(symbol.clone());
    ob.bids.insert(OrderedFloat(100.0), 1.0);
    ob.asks.insert(OrderedFloat(101.0), 1.0);
    ob.last_update_id = 50;
    let prev_best_bid = ob.best_bid();
    let prev_best_ask = ob.best_ask();

    // first_update_id <= last_update_id => stale
    let update = make_update(&symbol, 3_000, 50, 50, vec![("100.5", "1.0")], vec![]);
    let err = ob.apply_depth_update(update).unwrap_err();

    matches!(err, OrderBookError::StaleMessage { .. });
    assert_eq!(
        ob.last_update_id, 50,
        "last_update_id should not change on stale update"
    );
    assert_eq!(ob.best_bid(), prev_best_bid);
    assert_eq!(ob.best_ask(), prev_best_ask);
}

#[test]
fn sequence_gap_triggers_validation_error() {
    let symbol = "TESTUSDT".to_string();
    let mut ob = OrderBook::new(symbol.clone());
    ob.bids.insert(OrderedFloat(100.0), 1.0);
    ob.asks.insert(OrderedFloat(101.0), 1.0);
    ob.last_update_id = 10;

    // first_update_id > last_update_id + 1 => gap
    let update = make_update(&symbol, 4_000, 20, 21, vec![("100.1", "1.0")], vec![]);
    let err = ob.apply_depth_update(update).unwrap_err();

    matches!(err, OrderBookError::SequenceValidationFailed { .. });
    assert_eq!(
        ob.last_update_id, 10,
        "last_update_id should remain unchanged on gap"
    );
}

#[test]
fn last_update_time_tracks_event_time() {
    let symbol = "TESTUSDT".to_string();
    let mut ob = OrderBook::new(symbol.clone());
    ob.last_update_id = 30;

    let update = make_update(&symbol, 5_500, 31, 31, vec![("100.0", "1.0")], vec![]);
    ob.apply_depth_update(update)
        .expect("update should succeed");

    assert_eq!(
        ob.last_update_time, 5_500,
        "last_update_time should reflect update event time"
    );
}
