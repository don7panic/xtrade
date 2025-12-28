use crate::binance::perp_usdt::rest::BinancePerpRestClient as PerpClient;
use crate::binance::spot::rest::BinanceRestClient as SpotClient;
use crate::binance::types::DepthSnapshot;
use crate::market_data::DailyCandle;
use anyhow::Result;

pub enum RestClientWrapper {
    Spot(SpotClient),
    PerpUsdt(PerpClient),
}

impl RestClientWrapper {
    pub fn new_spot(base_url: String) -> Self {
        Self::Spot(SpotClient::new(base_url))
    }

    pub fn new_perp_usdt(base_url: String) -> Self {
        Self::PerpUsdt(PerpClient::new(base_url))
    }

    pub async fn get_depth_snapshot(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<DepthSnapshot> {
        match self {
            Self::Spot(client) => client.get_depth_snapshot(symbol, limit).await,
            Self::PerpUsdt(client) => client.get_depth_snapshot(symbol, limit).await,
        }
    }

    pub async fn get_daily_klines(
        &self,
        symbol: &str,
        limit: Option<u16>,
    ) -> Result<Vec<DailyCandle>> {
        match self {
            Self::Spot(client) => client.get_daily_klines(symbol, limit).await,
            Self::PerpUsdt(client) => client.get_daily_klines(symbol, limit).await,
        }
    }
}
