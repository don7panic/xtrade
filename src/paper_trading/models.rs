//! Data models for Paper Trading
//!
//! This module contains the core data structures for paper trading:
//! - `PaperOrder`: Represents a simulated order
//! - `PaperPosition`: Represents a position in a trading pair
//! - `PaperPortfolio`: Manages all positions and order history
//!
//! All monetary values use `rust_decimal::Decimal` for precise financial calculations.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::{HashMap, VecDeque};

/// Order direction (Buy or Sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "BUY"),
            OrderSide::Sell => write!(f, "SELL"),
        }
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    /// Order is pending execution
    Pending,
    /// Order has been filled
    Filled,
    /// Order has been cancelled
    Cancelled,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::Pending => write!(f, "PENDING"),
            OrderStatus::Filled => write!(f, "FILLED"),
            OrderStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

/// A paper trading order
#[derive(Debug, Clone)]
pub struct PaperOrder {
    /// Unique order ID (auto-incremented)
    pub id: u64,
    /// Trading pair symbol (e.g., "BTCUSDT")
    pub symbol: String,
    /// Order direction (Buy/Sell)
    pub side: OrderSide,
    /// Order quantity
    pub quantity: Decimal,
    /// Fill price (for market orders, this is the current price at execution)
    pub fill_price: Decimal,
    /// Order status
    pub status: OrderStatus,
    /// Order creation timestamp (Unix milliseconds)
    pub created_at: u64,
    /// Order fill timestamp (Unix milliseconds), None if not filled
    pub filled_at: Option<u64>,
}

impl PaperOrder {
    /// Create a new pending order
    pub fn new(id: u64, symbol: String, side: OrderSide, quantity: Decimal) -> Self {
        Self {
            id,
            symbol,
            side,
            quantity,
            fill_price: Decimal::ZERO,
            status: OrderStatus::Pending,
            created_at: Self::now_ms(),
            filled_at: None,
        }
    }

    /// Create a filled order (for immediate market order execution)
    pub fn new_filled(
        id: u64,
        symbol: String,
        side: OrderSide,
        quantity: Decimal,
        price: Decimal,
    ) -> Self {
        let now = Self::now_ms();
        Self {
            id,
            symbol,
            side,
            quantity,
            fill_price: price,
            status: OrderStatus::Filled,
            created_at: now,
            filled_at: Some(now),
        }
    }

    /// Get current timestamp in milliseconds
    fn now_ms() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Calculate the total value of this order
    pub fn total_value(&self) -> Decimal {
        self.quantity * self.fill_price
    }
}

/// A paper trading position
#[derive(Debug, Clone)]
pub struct PaperPosition {
    /// Trading pair symbol
    pub symbol: String,
    /// Position quantity (positive for long positions)
    pub quantity: Decimal,
    /// Average cost price
    pub avg_cost: Decimal,
    /// Current market price (updated by price ticks)
    pub current_price: Decimal,
    /// Unrealized profit/loss in quote currency
    pub unrealized_pnl: Decimal,
    /// Unrealized profit/loss percentage
    pub unrealized_pnl_pct: Decimal,
}

impl PaperPosition {
    /// Create a new empty position for a symbol
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            quantity: Decimal::ZERO,
            avg_cost: Decimal::ZERO,
            current_price: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            unrealized_pnl_pct: Decimal::ZERO,
        }
    }

    /// Update position with current market price and recalculate PnL
    pub fn update_price(&mut self, price: Decimal) {
        self.current_price = price;
        self.recalculate_pnl();
    }

    /// Recalculate unrealized PnL based on current price
    fn recalculate_pnl(&mut self) {
        if self.quantity.is_zero() || self.avg_cost.is_zero() {
            self.unrealized_pnl = Decimal::ZERO;
            self.unrealized_pnl_pct = Decimal::ZERO;
            return;
        }

        self.unrealized_pnl = (self.current_price - self.avg_cost) * self.quantity;
        self.unrealized_pnl_pct = (self.current_price - self.avg_cost) / self.avg_cost * dec!(100);
    }

    /// Add to position (buy)
    pub fn add(&mut self, quantity: Decimal, price: Decimal) {
        if quantity.is_zero() || quantity.is_sign_negative() {
            return;
        }

        let total_cost = self.avg_cost * self.quantity + price * quantity;
        self.quantity += quantity;
        if !self.quantity.is_zero() {
            self.avg_cost = total_cost / self.quantity;
        }
        self.current_price = price;
        self.recalculate_pnl();
    }

    /// Reduce position (sell) and return realized PnL
    pub fn reduce(&mut self, quantity: Decimal, price: Decimal) -> Decimal {
        if quantity.is_zero() || quantity.is_sign_negative() || self.quantity.is_zero() {
            return Decimal::ZERO;
        }

        // Cap reduction to available quantity
        let actual_qty = quantity.min(self.quantity);
        let realized_pnl = (price - self.avg_cost) * actual_qty;

        self.quantity -= actual_qty;
        self.current_price = price;

        // If position is closed, reset avg_cost
        if self.quantity.is_zero() {
            self.avg_cost = Decimal::ZERO;
        }

        self.recalculate_pnl();
        realized_pnl
    }

    /// Check if position is empty (no holdings)
    pub fn is_empty(&self) -> bool {
        self.quantity.is_zero()
    }

    /// Get the total position value at current price
    pub fn market_value(&self) -> Decimal {
        self.quantity * self.current_price
    }

    /// Get the total cost basis
    pub fn cost_basis(&self) -> Decimal {
        self.quantity * self.avg_cost
    }
}

/// Maximum number of orders to keep in history
const ORDER_HISTORY_CAPACITY: usize = 100;

/// Paper trading portfolio managing all positions and orders
#[derive(Debug, Clone, Default)]
pub struct PaperPortfolio {
    /// All open positions indexed by symbol
    pub positions: HashMap<String, PaperPosition>,
    /// Recent order history (bounded)
    pub order_history: VecDeque<PaperOrder>,
    /// Next order ID counter
    next_order_id: u64,
    /// Total realized profit/loss
    pub realized_pnl: Decimal,
}

impl PaperPortfolio {
    /// Create a new empty portfolio
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            order_history: VecDeque::with_capacity(ORDER_HISTORY_CAPACITY),
            next_order_id: 1,
            realized_pnl: Decimal::ZERO,
        }
    }

    /// Get and increment the next order ID
    pub fn next_order_id(&mut self) -> u64 {
        let id = self.next_order_id;
        self.next_order_id += 1;
        id
    }

    /// Get a position by symbol (if exists)
    pub fn position(&self, symbol: &str) -> Option<&PaperPosition> {
        self.positions.get(symbol)
    }

    /// Get a mutable position by symbol, creating if not exists
    pub fn position_mut(&mut self, symbol: &str) -> &mut PaperPosition {
        self.positions
            .entry(symbol.to_string())
            .or_insert_with(|| PaperPosition::new(symbol))
    }

    /// Update price for a symbol, refreshing PnL
    pub fn update_price(&mut self, symbol: &str, price: Decimal) {
        if let Some(position) = self.positions.get_mut(symbol) {
            position.update_price(price);
        }
    }

    /// Add an order to history
    pub fn add_order(&mut self, order: PaperOrder) {
        self.order_history.push_back(order);
        // Trim history if over capacity
        while self.order_history.len() > ORDER_HISTORY_CAPACITY {
            self.order_history.pop_front();
        }
    }

    /// Get total unrealized PnL across all positions
    pub fn total_unrealized_pnl(&self) -> Decimal {
        self.positions.values().map(|p| p.unrealized_pnl).sum()
    }

    /// Get total market value of all positions
    pub fn total_market_value(&self) -> Decimal {
        self.positions.values().map(|p| p.market_value()).sum()
    }

    /// Get total cost basis of all positions
    pub fn total_cost_basis(&self) -> Decimal {
        self.positions.values().map(|p| p.cost_basis()).sum()
    }

    /// Get number of open positions (non-zero quantity)
    pub fn open_position_count(&self) -> usize {
        self.positions.values().filter(|p| !p.is_empty()).count()
    }

    /// Get recent orders (most recent first)
    pub fn recent_orders(&self, limit: usize) -> Vec<&PaperOrder> {
        self.order_history.iter().rev().take(limit).collect()
    }

    /// Clear all positions and reset portfolio
    pub fn reset(&mut self) {
        self.positions.clear();
        self.order_history.clear();
        self.next_order_id = 1;
        self.realized_pnl = Decimal::ZERO;
    }

    /// Remove empty positions from the map
    pub fn cleanup_empty_positions(&mut self) {
        self.positions.retain(|_, p| !p.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_position_add() {
        let mut pos = PaperPosition::new("BTCUSDT");

        // Buy 0.1 BTC at $45,000
        pos.add(dec!(0.1), dec!(45000));
        assert_eq!(pos.quantity, dec!(0.1));
        assert_eq!(pos.avg_cost, dec!(45000));
        assert!(!pos.is_empty());

        // Buy 0.2 BTC at $46,000 -> avg cost = (0.1*45000 + 0.2*46000) / 0.3 = 45666.67
        pos.add(dec!(0.2), dec!(46000));
        assert_eq!(pos.quantity, dec!(0.3));
        // (4500 + 9200) / 0.3 = 13700 / 0.3 = 45666.666...
        let expected_avg = (dec!(0.1) * dec!(45000) + dec!(0.2) * dec!(46000)) / dec!(0.3);
        assert_eq!(pos.avg_cost, expected_avg);
    }

    #[test]
    fn test_paper_position_reduce() {
        let mut pos = PaperPosition::new("BTCUSDT");

        // Buy 0.3 BTC at $45,000
        pos.add(dec!(0.3), dec!(45000));

        // Sell 0.1 BTC at $46,000 -> realized PnL = (46000 - 45000) * 0.1 = 100
        let realized = pos.reduce(dec!(0.1), dec!(46000));
        assert_eq!(realized, dec!(100));
        assert_eq!(pos.quantity, dec!(0.2));
        assert_eq!(pos.avg_cost, dec!(45000)); // avg cost unchanged

        // Sell remaining 0.2 BTC at $44,000 -> realized PnL = (44000 - 45000) * 0.2 = -200
        let realized = pos.reduce(dec!(0.2), dec!(44000));
        assert_eq!(realized, dec!(-200));
        assert!(pos.is_empty());
    }

    #[test]
    fn test_paper_position_pnl() {
        let mut pos = PaperPosition::new("BTCUSDT");
        pos.add(dec!(1), dec!(45000));

        // Price goes up to $46,000
        pos.update_price(dec!(46000));
        assert_eq!(pos.unrealized_pnl, dec!(1000));

        // Price goes down to $44,000
        pos.update_price(dec!(44000));
        assert_eq!(pos.unrealized_pnl, dec!(-1000));
    }

    #[test]
    fn test_paper_portfolio_positions() {
        let mut portfolio = PaperPortfolio::new();

        // Add position
        let pos = portfolio.position_mut("BTCUSDT");
        pos.add(dec!(0.1), dec!(45000));

        assert_eq!(portfolio.open_position_count(), 1);
        assert!(portfolio.position("BTCUSDT").is_some());

        // Update price
        portfolio.update_price("BTCUSDT", dec!(46000));
        let pos = portfolio.position("BTCUSDT").unwrap();
        assert_eq!(pos.unrealized_pnl, dec!(100));
    }

    #[test]
    fn test_paper_portfolio_order_history() {
        let mut portfolio = PaperPortfolio::new();

        // Add some orders
        for i in 1..=5 {
            let order = PaperOrder::new_filled(
                i,
                "BTCUSDT".to_string(),
                OrderSide::Buy,
                dec!(0.1),
                Decimal::from(45000 + i * 100),
            );
            portfolio.add_order(order);
        }

        assert_eq!(portfolio.order_history.len(), 5);

        // Recent orders should be in reverse order
        let recent = portfolio.recent_orders(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].id, 5);
        assert_eq!(recent[1].id, 4);
        assert_eq!(recent[2].id, 3);
    }

    #[test]
    fn test_paper_portfolio_reset() {
        let mut portfolio = PaperPortfolio::new();

        portfolio
            .position_mut("BTCUSDT")
            .add(dec!(0.1), dec!(45000));
        portfolio.add_order(PaperOrder::new_filled(
            1,
            "BTCUSDT".to_string(),
            OrderSide::Buy,
            dec!(0.1),
            dec!(45000),
        ));
        portfolio.realized_pnl = dec!(100);

        portfolio.reset();

        assert!(portfolio.positions.is_empty());
        assert!(portfolio.order_history.is_empty());
        assert_eq!(portfolio.next_order_id, 1);
        assert_eq!(portfolio.realized_pnl, Decimal::ZERO);
    }

    #[test]
    fn test_decimal_precision() {
        // This test demonstrates the precision advantage of Decimal
        // With f64: 0.1 + 0.2 != 0.3
        // With Decimal: 0.1 + 0.2 == 0.3 ✓
        let a = dec!(0.1);
        let b = dec!(0.2);
        let c = dec!(0.3);
        assert_eq!(a + b, c); // This would fail with f64!
    }
}
