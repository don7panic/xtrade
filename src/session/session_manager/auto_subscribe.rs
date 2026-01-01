use futures_util::{StreamExt, stream::FuturesUnordered};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Semaphore, mpsc};
use tracing::{error, info, warn};

use crate::binance::types::{MarketKey, MarketType};
use crate::market_data::{MarketDataManager, MarketStream};

use super::SessionManager;
use crate::session::action_channel::{ActionChannel, SessionEvent};

const AUTO_SUBSCRIBE_MAX_CONCURRENCY: usize = 4;

#[derive(Debug, Clone)]
pub(super) struct SubscriptionPlan {
    pub(super) key: MarketKey,
    pub(super) streams: HashSet<MarketStream>,
}

impl SessionManager {
    pub(super) fn build_subscription_plan(&self) -> Vec<SubscriptionPlan> {
        let mut plan_map: HashMap<MarketKey, HashSet<MarketStream>> = HashMap::new();

        for market in &self.app_config.markets {
            if !market.exchange.eq_ignore_ascii_case("binance") {
                warn!(
                    "Skipping unsupported exchange '{}' in markets config",
                    market.exchange
                );
                continue;
            }

            let streams = Self::resolve_streams(market.market_type, &market.streams);

            for symbol in &market.symbols {
                let key = MarketKey {
                    market_type: market.market_type,
                    symbol: symbol.clone(),
                };
                Self::merge_streams(&mut plan_map, key, streams.clone());
            }
        }

        if !self.app_config.symbols.is_empty() {
            let market_type = self.get_market_type();
            let streams = MarketStream::default_streams_for_market(market_type);

            for symbol in &self.app_config.symbols {
                let key = MarketKey {
                    market_type,
                    symbol: symbol.clone(),
                };
                plan_map.entry(key).or_insert_with(|| streams.clone());
            }
        }

        plan_map
            .into_iter()
            .map(|(key, streams)| SubscriptionPlan { key, streams })
            .collect()
    }

    fn merge_streams(
        plan_map: &mut HashMap<MarketKey, HashSet<MarketStream>>,
        key: MarketKey,
        streams: HashSet<MarketStream>,
    ) {
        plan_map
            .entry(key)
            .and_modify(|existing| {
                existing.extend(streams.iter().copied());
            })
            .or_insert(streams);
    }

    fn resolve_streams(market_type: MarketType, streams: &[String]) -> HashSet<MarketStream> {
        if streams.is_empty() {
            return MarketStream::default_streams_for_market(market_type);
        }

        let mut resolved = HashSet::new();
        let mut unknown = Vec::new();

        for stream in streams {
            if let Ok(parsed) = stream.parse::<MarketStream>() {
                let perp_only = matches!(
                    parsed,
                    MarketStream::MarkPrice
                        | MarketStream::FundingRate
                        | MarketStream::OpenInterest
                        | MarketStream::ForceOrder
                );

                if market_type == MarketType::Spot && perp_only {
                    warn!("Ignoring perp-only stream '{}' for spot market", stream);
                    continue;
                }

                resolved.insert(parsed);
            } else if !stream.trim().is_empty() {
                unknown.push(stream.clone());
            }
        }

        if !unknown.is_empty() {
            warn!("Unknown stream names in config: {:?}", unknown);
        }

        if resolved.is_empty() {
            MarketStream::default_streams_for_market(market_type)
        } else {
            resolved
        }
    }

    /// Spawn background task to auto-subscribe to configured symbols with controlled parallelism
    pub(super) fn spawn_auto_subscribe_symbols(&self) {
        let subscriptions = self.build_subscription_plan();
        if subscriptions.is_empty() {
            return;
        }

        info!(
            "Scheduling background auto-subscribe for {} symbols",
            subscriptions.len()
        );

        let market_manager = self.market_manager.clone();
        let action_channel = self.action_channel.clone();
        let ui_event_tx = self.ui_event_tx.clone();
        let enable_tui = self.config.enable_tui;

        tokio::spawn(async move {
            Self::run_auto_subscribe_workflow(
                subscriptions,
                market_manager,
                action_channel,
                ui_event_tx,
                enable_tui,
            )
            .await;
        });
    }

    async fn run_auto_subscribe_workflow(
        subscriptions: Vec<SubscriptionPlan>,
        market_manager: Arc<MarketDataManager>,
        action_channel: ActionChannel,
        ui_event_tx: Option<mpsc::UnboundedSender<SessionEvent>>,
        enable_tui: bool,
    ) {
        let symbol_count = subscriptions.len();
        if symbol_count == 0 {
            return;
        }

        info!(
            "Starting background auto-subscribe workflow for {} symbols",
            symbol_count
        );

        let max_concurrency = AUTO_SUBSCRIBE_MAX_CONCURRENCY.max(1);
        let concurrency = max_concurrency.min(symbol_count.max(1));
        let semaphore = Arc::new(Semaphore::new(concurrency));

        let mut tasks = FuturesUnordered::new();

        for subscription in subscriptions {
            let market_manager = market_manager.clone();
            let action_channel = action_channel.clone();
            let semaphore = semaphore.clone();
            let key = subscription.key.clone();
            let streams = subscription.streams.clone();
            let symbol = key.symbol.clone();

            tasks.push(async move {
                let permit = match semaphore.acquire_owned().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        error!(
                            "Auto-subscribe permit acquisition failed for {}: {}",
                            symbol, e
                        );
                        return;
                    }
                };
                match market_manager
                    .subscribe_with_streams(key.clone(), streams)
                    .await
                {
                    Ok(()) => {
                        info!("Auto-subscribed to symbol: {}", symbol);
                        if let Err(e) = action_channel
                            .send_event(SessionEvent::SubscriptionAdded { key: key.clone() })
                        {
                            error!(
                                "Failed to emit SubscriptionAdded event for {}: {}",
                                symbol, e
                            );
                        }
                    }
                    Err(e) => {
                        error!("Failed to auto-subscribe to {}: {}", symbol, e);
                        let _ = action_channel.send_event(SessionEvent::Error {
                            message: format!("Failed to auto-subscribe to {}: {}", symbol, e),
                        });
                    }
                }

                drop(permit);
            });
        }

        while tasks.next().await.is_some() {}

        info!("Background auto-subscribe workflow completed");

        if enable_tui {
            if let Some(tx) = ui_event_tx {
                let keys = market_manager.list_subscriptions().await;
                if let Err(e) = tx.send(SessionEvent::SubscriptionList { keys }) {
                    error!("Failed to send subscription list to UI: {}", e);
                }
            }
        }
    }
}
