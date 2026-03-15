//! Binance Futures (USDT-M) API

use async_trait::async_trait;
use iqai_core::exchange::{ExchangeConnector, ExchangeError, ExchangeResult, OrderResponse, OrderSide};
use iqai_core::types::{Candle, Exchange, MarketType, Timeframe};
use reqwest::Client;

use crate::sign;

const BINANCE_FUTURES_API: &str = "https://fapi.binance.com";

/// Binance USDT-M Futures client
pub struct BinanceFuturesClient {
    client: Client,
    api_key: Option<String>,
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

    /// Anlık fiyat (ticker) – REST; canlı mum için WebSocket kullanılmaz.
    pub async fn fetch_ticker_price(&self, symbol: &str) -> Result<f64, ExchangeError> {
        let symbol_futures = if symbol.ends_with("USDT") {
            symbol.to_string()
        } else {
            format!("{}USDT", symbol)
        };
        let url = format!(
            "{}/fapi/v1/ticker/price?symbol={}",
            BINANCE_FUTURES_API,
            symbol_futures
        );
        #[derive(serde::Deserialize)]
        struct TickerPrice {
            price: String,
        }
        let resp: TickerPrice = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        resp.price
            .parse()
            .map_err(|_| ExchangeError::Api("Invalid ticker price".to_string()))
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
        symbol: &str,
        side: OrderSide,
        quantity: f64,
    ) -> ExchangeResult<OrderResponse> {
        let api_key = self.api_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Futures: API key not configured".into())
        })?;
        let secret = self.secret_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Futures: Secret key not configured".into())
        })?;

        let side_str = match side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };
        let ts = sign::timestamp_ms();
        let query = format!(
            "symbol={}&side={}&type=MARKET&quantity={:.6}&timestamp={}",
            symbol.to_uppercase(),
            side_str,
            quantity,
            ts
        );
        let signature = sign::sign(&query, secret);
        let url = format!(
            "{}/fapi/v1/order?{}&signature={}",
            BINANCE_FUTURES_API, query, signature
        );

        log::info!("[FUTURES] Market order: {} {} qty={:.6}", side_str, symbol, quantity);

        let resp = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        if !status.is_success() {
            let msg = body["msg"].as_str().unwrap_or("unknown error");
            return Err(ExchangeError::Api(format!("Binance Futures {}: {}", status, msg)));
        }

        Ok(OrderResponse {
            order_id: body["orderId"].to_string(),
            symbol: body["symbol"].as_str().unwrap_or(symbol).to_string(),
            side,
            executed_qty: body["executedQty"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(quantity),
            avg_price: body["avgPrice"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
        })
    }

    async fn get_balance(&self, asset: &str) -> ExchangeResult<f64> {
        let api_key = self.api_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Futures: API key not configured".into())
        })?;
        let secret = self.secret_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Futures: Secret key not configured".into())
        })?;

        let ts = sign::timestamp_ms();
        let query = format!("timestamp={}", ts);
        let signature = sign::sign(&query, secret);
        let url = format!(
            "{}/fapi/v2/balance?{}&signature={}",
            BINANCE_FUTURES_API, query, signature
        );

        let resp = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        if !status.is_success() {
            let msg = body["msg"].as_str().unwrap_or("unknown error");
            return Err(ExchangeError::Api(format!("Binance Futures {}: {}", status, msg)));
        }

        let arr = body.as_array().ok_or_else(|| {
            ExchangeError::Api("Unexpected balance response format".into())
        })?;
        let asset_upper = asset.to_uppercase();
        for item in arr {
            if item["asset"].as_str() == Some(&asset_upper) {
                let balance = item["balance"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                return Ok(balance);
            }
        }
        Ok(0.0)
    }
}
