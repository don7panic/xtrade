use anyhow::Result;
use std::collections::{HashMap, VecDeque};
use tracing::info;

use crate::market_data::MarketEvent;
use crate::paper_trading::{Decimal, PaperOrder, PaperPortfolio, PaperTradingEngine};

pub(super) struct PaperTradeOutcome {
    pub(super) order: PaperOrder,
    pub(super) portfolio: PaperPortfolio,
}

pub(super) struct PaperTradingState {
    engine: PaperTradingEngine,
    last_prices: HashMap<String, Decimal>,
}

impl PaperTradingState {
    pub(super) fn new() -> Self {
        Self {
            engine: PaperTradingEngine::new(),
            last_prices: HashMap::new(),
        }
    }

    pub(super) fn handle_market_event(&mut self, event: &MarketEvent) -> Option<(String, f64)> {
        match event {
            MarketEvent::PriceUpdate { key, price, .. } => {
                let symbol = key.symbol.clone();

                let decimal_price = Decimal::from_f64_retain(*price).unwrap_or(Decimal::ZERO);
                self.last_prices.insert(symbol.clone(), decimal_price);
                self.engine.on_price_update(&symbol, decimal_price);

                Some((symbol, *price))
            }
            MarketEvent::TickerUpdate {
                key, last_price, ..
            } => {
                let symbol = key.symbol.clone();
                let decimal_price = Decimal::from_f64_retain(*last_price).unwrap_or(Decimal::ZERO);
                self.last_prices.insert(symbol.clone(), decimal_price);
                self.engine.on_price_update(&symbol, decimal_price);
                Some((symbol, *last_price))
            }
            _ => None,
        }
    }

    pub(super) fn buy(&mut self, symbol: &str, quantity: f64) -> Result<PaperTradeOutcome> {
        let price = self.get_market_price(symbol)?;
        let qty = Decimal::from_f64_retain(quantity)
            .ok_or_else(|| anyhow::anyhow!("Invalid quantity"))?;

        info!("Paper Buy: {} {} @ {}", quantity, symbol, price);
        let order = self.engine.buy(symbol, qty, price)?;

        Ok(PaperTradeOutcome {
            order,
            portfolio: self.engine.portfolio().clone(),
        })
    }

    pub(super) fn sell(&mut self, symbol: &str, quantity: f64) -> Result<PaperTradeOutcome> {
        let price = self.get_market_price(symbol)?;
        let qty = Decimal::from_f64_retain(quantity)
            .ok_or_else(|| anyhow::anyhow!("Invalid quantity"))?;

        info!("Paper Sell: {} {} @ {}", quantity, symbol, price);
        let order = self.engine.sell(symbol, qty, price)?;

        Ok(PaperTradeOutcome {
            order,
            portfolio: self.engine.portfolio().clone(),
        })
    }

    pub(super) fn portfolio_snapshot(&self) -> PaperPortfolio {
        self.engine.portfolio().clone()
    }

    pub(super) fn order_history_snapshot(&self) -> VecDeque<PaperOrder> {
        self.engine.portfolio().order_history.clone()
    }

    fn get_market_price(&self, symbol: &str) -> Result<Decimal> {
        if let Some(price) = self.last_prices.get(symbol) {
            return Ok(*price);
        }
        Err(anyhow::anyhow!(
            "No price data available for {}. Please wait for market data.",
            symbol
        ))
    }
}
