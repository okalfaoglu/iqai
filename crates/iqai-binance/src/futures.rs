//! Binance Futures (USDT-M) API

use async_trait::async_trait;
use iqai_core::exchange::{ExchangeConnector, ExchangeError, ExchangeResult, OrderResponse, OrderSide};
use iqai_core::types::{Candle, Exchange, MarketType, Timeframe};
use reqwest::Client;

const BINANCE_FUTURES_API: &str = "https://fapi.binance.com";

/// Binance USDT-M Futures client
pub struct BinanceFuturesClient {
    client: Client,
    #[allow(dead_code)] // Reserved for authenticated endpoints (orders, balance)
    api_key: Option<String>,
    #[allow(dead_code)]
    secret_key: Option<String>,
}

impl BinanceFuturesClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: None,
            secret_key: None,
        }
    }

    pub fn with_credentials(api_key: String, secret_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: Some(api_key),
            secret_key: Some(secret_key),
        }
    }

    pub async fn fetch_klines_impl(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
    ) -> Result<Vec<Candle>, ExchangeError> {
        let symbol_futures = if symbol.ends_with("USDT") {
            symbol.to_string()
        } else {
            format!("{}USDT", symbol)
        };
        let url = format!(
            "{}/fapi/v1/klines?symbol={}&interval={}&limit={}",
            BINANCE_FUTURES_API,
            symbol_futures,
            interval,
            limit.min(1500)
        );
        let resp: Vec<Vec<serde_json::Value>> = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        let candles: Vec<Candle> = resp
            .into_iter()
            .filter_map(|k| {
                let t = k.get(0)?.as_i64()?;
                let o = k.get(1)?.as_str()?.parse().ok()?;
                let h = k.get(2)?.as_str()?.parse().ok()?;
                let l = k.get(3)?.as_str()?.parse().ok()?;
                let c = k.get(4)?.as_str()?.parse().ok()?;
                let v = k.get(5)?.as_str()?.parse().ok()?;
                Some(Candle {
                    time: t,
                    open: o,
                    high: h,
                    low: l,
                    close: c,
                    volume: v,
                })
            })
            .collect();
        Ok(candles)
    }
}

impl Default for BinanceFuturesClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExchangeConnector for BinanceFuturesClient {
    fn exchange(&self) -> Exchange {
        Exchange::Binance
    }

    fn market_type(&self) -> MarketType {
        MarketType::Futures
    }

    async fn fetch_klines(
        &self,
        symbol: &str,
        interval: Timeframe,
        limit: u32,
    ) -> ExchangeResult<Vec<Candle>> {
        self.fetch_klines_impl(symbol, interval.to_binance_interval(), limit)
            .await
    }

    async fn place_market_order(
        &self,
        _symbol: &str,
        _side: OrderSide,
        _quantity: f64,
    ) -> ExchangeResult<OrderResponse> {
        Err(ExchangeError::Api(
            "Binance Futures: configure API keys for trading".to_string(),
        ))
    }

    async fn get_balance(&self, _asset: &str) -> ExchangeResult<f64> {
        Err(ExchangeError::Api(
            "Binance Futures: configure API keys for balance".to_string(),
        ))
    }
}
