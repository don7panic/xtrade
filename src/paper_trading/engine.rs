//! Paper Trading Engine
//!
//! This module implements the core trading engine for paper trading.
//! It handles order execution, position management, and PnL calculations.

use anyhow::{Result, anyhow};
use rust_decimal::Decimal;
#[cfg(test)]
use rust_decimal_macros::dec;

use super::models::{OrderSide, PaperOrder, PaperPortfolio, PaperPosition};

/// Paper Trading Engine
///
/// Manages the paper trading portfolio, handling buy/sell orders
/// and price updates for PnL calculation.
#[derive(Debug)]
pub struct PaperTradingEngine {
    /// Portfolio containing all positions and order history
    portfolio: PaperPortfolio,
}

impl Default for PaperTradingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PaperTradingEngine {
    /// Create a new PaperTradingEngine
    pub fn new() -> Self {
        Self {
            portfolio: PaperPortfolio::new(),
        }
    }

    /// Execute a buy order
    ///
    /// Creates a market order that is immediately filled at the given price.
    /// Updates the position with the new quantity and recalculates average cost.
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "BTCUSDT")
    /// * `quantity` - Amount to buy (must be positive)
    /// * `price` - Current market price for execution
    ///
    /// # Returns
    /// * `Ok(PaperOrder)` - The filled order
    /// * `Err` - If quantity is zero or negative
    pub fn buy(&mut self, symbol: &str, quantity: Decimal, price: Decimal) -> Result<PaperOrder> {
        if quantity <= Decimal::ZERO {
            return Err(anyhow!("Buy quantity must be positive"));
        }
        if price <= Decimal::ZERO {
            return Err(anyhow!("Price must be positive"));
        }

        Ok(self.execute_order(OrderSide::Buy, symbol, quantity, price))
    }

    /// Execute a sell order
    ///
    /// Creates a market order that is immediately filled at the given price.
    /// Reduces the position and calculates realized PnL.
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "BTCUSDT")
    /// * `quantity` - Amount to sell (must be positive, capped at current position)
    /// * `price` - Current market price for execution
    ///
    /// # Returns
    /// * `Ok(PaperOrder)` - The filled order
    /// * `Err` - If quantity is zero or negative, or no position exists
    pub fn sell(&mut self, symbol: &str, quantity: Decimal, price: Decimal) -> Result<PaperOrder> {
        if quantity <= Decimal::ZERO {
            return Err(anyhow!("Sell quantity must be positive"));
        }
        if price <= Decimal::ZERO {
            return Err(anyhow!("Price must be positive"));
        }

        // Check if we have a position to sell
        let position = self.portfolio.position(symbol);
        if position.is_none() || position.unwrap().quantity.is_zero() {
            return Err(anyhow!("No position to sell for {}", symbol));
        }

        let available_qty = position.unwrap().quantity;
        if quantity > available_qty {
            return Err(anyhow!(
                "Insufficient position: requested {} but only {} available",
                quantity,
                available_qty
            ));
        }

        Ok(self.execute_order(OrderSide::Sell, symbol, quantity, price))
    }

    /// Execute an order (internal implementation)
    fn execute_order(
        &mut self,
        side: OrderSide,
        symbol: &str,
        quantity: Decimal,
        price: Decimal,
    ) -> PaperOrder {
        // Create filled order
        let order_id = self.portfolio.next_order_id();
        let order = PaperOrder::new_filled(order_id, symbol.to_string(), side, quantity, price);

        // Update position
        let position = self.portfolio.position_mut(symbol);

        match side {
            OrderSide::Buy => {
                position.add(quantity, price);
            }
            OrderSide::Sell => {
                let realized_pnl = position.reduce(quantity, price);
                self.portfolio.realized_pnl += realized_pnl;
            }
        }

        // Record order in history
        self.portfolio.add_order(order.clone());

        order
    }

    /// Handle price update from market data
    ///
    /// Updates the current price for a position and recalculates unrealized PnL.
    /// This is called when new market data arrives.
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol
    /// * `price` - New market price
    pub fn on_price_update(&mut self, symbol: &str, price: Decimal) {
        self.portfolio.update_price(symbol, price);
    }

    /// Get reference to the current portfolio
    pub fn portfolio(&self) -> &PaperPortfolio {
        &self.portfolio
    }

    /// Get mutable reference to the current portfolio
    pub fn portfolio_mut(&mut self) -> &mut PaperPortfolio {
        &mut self.portfolio
    }

    /// Get a specific position by symbol
    pub fn position(&self, symbol: &str) -> Option<&PaperPosition> {
        self.portfolio.position(symbol)
    }

    /// Get total unrealized PnL across all positions
    pub fn total_unrealized_pnl(&self) -> Decimal {
        self.portfolio.total_unrealized_pnl()
    }

    /// Get total realized PnL
    pub fn total_realized_pnl(&self) -> Decimal {
        self.portfolio.realized_pnl
    }

    /// Reset the engine, clearing all positions and history
    pub fn reset(&mut self) {
        self.portfolio.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sell_closes_position() {
        let mut engine = PaperTradingEngine::new();

        engine.buy("BTCUSDT", dec!(0.1), dec!(45000)).unwrap();
        engine.sell("BTCUSDT", dec!(0.1), dec!(44000)).unwrap();

        let position = engine.position("BTCUSDT").unwrap();
        assert!(position.is_empty());

        // Realized PnL = (44000 - 45000) * 0.1 = -100
        assert_eq!(engine.total_realized_pnl(), dec!(-100));
    }

    #[test]
    fn test_price_update_refreshes_pnl() {
        let mut engine = PaperTradingEngine::new();

        engine.buy("BTCUSDT", dec!(1), dec!(45000)).unwrap();
        assert_eq!(engine.total_unrealized_pnl(), dec!(0));

        // Price goes up
        engine.on_price_update("BTCUSDT", dec!(46000));
        assert_eq!(engine.total_unrealized_pnl(), dec!(1000));

        // Price goes down
        engine.on_price_update("BTCUSDT", dec!(44000));
        assert_eq!(engine.total_unrealized_pnl(), dec!(-1000));
    }

    #[test]
    fn test_order_history() {
        let mut engine = PaperTradingEngine::new();

        engine.buy("BTCUSDT", dec!(0.1), dec!(45000)).unwrap();
        engine.buy("BTCUSDT", dec!(0.2), dec!(46000)).unwrap();
        engine.sell("BTCUSDT", dec!(0.1), dec!(47000)).unwrap();

        assert_eq!(engine.portfolio().order_history.len(), 3);

        let recent = engine.portfolio().recent_orders(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].side, OrderSide::Sell); // Most recent first
        assert_eq!(recent[1].side, OrderSide::Buy);
    }
}
