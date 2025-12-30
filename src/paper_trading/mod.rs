//! Paper Trading module for simulated trading
//!
//! This module provides paper trading functionality that allows users to
//! simulate trading without real funds. It includes order management,
//! position tracking, and PnL calculations.
//!
//! All monetary values use `rust_decimal::Decimal` for precise calculations.

mod engine;
mod models;

// Re-export Decimal for convenience
pub use rust_decimal::Decimal;
pub use rust_decimal_macros::dec;

pub use engine::PaperTradingEngine;
pub use models::{OrderSide, OrderStatus, PaperOrder, PaperPortfolio, PaperPosition};
