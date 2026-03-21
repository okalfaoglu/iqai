//! Binance Spot API

use std::sync::Mutex;

use async_trait::async_trait;
use iqai_core::exchange::{
    classify_binance_json, ExchangeConnector, ExchangeError, ExchangeResult, OrderResponse,
    OrderSide, RcaOpenMarketSnapshot,
};
use iqai_core::indicators::atr;
use iqai_core::traceparent_from_uuid;
use iqai_core::types::{Candle, Exchange, MarketType, Timeframe};
use reqwest::Client;

use crate::http_retry::send_get_retry;
use crate::sign;

const BINANCE_SPOT_API: &str = "https://api.binance.com";

/// Binance Spot market client
pub struct BinanceSpotClient {
    client: Client,
    api_key: Option<String>,
    secret_key: Option<String>,
    traceparent: Mutex<Option<String>>,
}

impl BinanceSpotClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: None,
            secret_key: None,
            traceparent: Mutex::new(None),
        }
    }

    pub fn with_credentials(api_key: String, secret_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: Some(api_key),
            secret_key: Some(secret_key),
            traceparent: Mutex::new(None),
        }
    }

    fn optional_traceparent(&self) -> Option<String> {
        self.traceparent.lock().ok().and_then(|g| g.clone())
    }

    fn apply_traceparent(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(tp) = self.optional_traceparent() {
            req.header("traceparent", tp)
        } else {
            req
        }
    }

    async fn send_get<F>(&self, build: F) -> Result<reqwest::Response, ExchangeError>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let tp = self.optional_traceparent();
        send_get_retry(&self.client, tp.as_deref(), build).await
    }

    pub async fn fetch_klines_impl(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
    ) -> Result<Vec<Candle>, ExchangeError> {
        let url = format!(
            "{}/api/v3/klines?symbol={}&interval={}&limit={}",
            BINANCE_SPOT_API,
            symbol.to_uppercase(),
            interval,
            limit.min(1000)
        );
        let resp: Vec<Vec<serde_json::Value>> = self
            .send_get(|| self.client.get(&url))
            .await?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        let candles: Vec<Candle> = resp
            .into_iter()
            .filter_map(|k| Self::parse_kline(&k))
            .collect();
        Ok(candles)
    }

    fn parse_kline(k: &[serde_json::Value]) -> Option<Candle> {
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
    }

    /// Geçmiş mumları tarih aralığına göre çeker (startTime/endTime, 1000’lik chunk’lar).
    pub async fn fetch_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_time_ms: i64,
        end_time_ms: i64,
    ) -> Result<Vec<Candle>, ExchangeError> {
        let mut all: Vec<Candle> = Vec::new();
        let mut start = start_time_ms;
        const CHUNK: u32 = 1000;
        loop {
            let url = format!(
                "{}/api/v3/klines?symbol={}&interval={}&limit={}&startTime={}&endTime={}",
                BINANCE_SPOT_API,
                symbol.to_uppercase(),
                interval,
                CHUNK,
                start,
                end_time_ms
            );
            let resp: Vec<Vec<serde_json::Value>> = self
                .send_get(|| self.client.get(&url))
                .await?
                .json()
                .await
                .map_err(|e| ExchangeError::Http(e.to_string()))?;
            if resp.is_empty() {
                break;
            }
            let candles: Vec<Candle> = resp
                .iter()
                .filter_map(|k| Self::parse_kline(k))
                .collect();
            if candles.is_empty() {
                break;
            }
            let last_time = candles.last().map(|c| c.time).unwrap_or(start);
            all.extend(candles);
            if last_time >= end_time_ms || resp.len() < CHUNK as usize {
                break;
            }
            start = last_time + 1;
        }
        Ok(all)
    }

    /// GET /api/v3/ticker/bookTicker — spread'i bps (mid'e göre).
    pub async fn fetch_book_ticker_spread_bps(&self, symbol: &str) -> Result<f64, ExchangeError> {
        let url = format!(
            "{}/api/v3/ticker/bookTicker?symbol={}",
            BINANCE_SPOT_API,
            symbol.to_uppercase()
        );
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BookTicker {
            bid_price: String,
            ask_price: String,
        }
        let resp: BookTicker = self
            .send_get(|| self.client.get(&url))
            .await?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        let bid: f64 = resp
            .bid_price
            .parse()
            .map_err(|_| ExchangeError::Api("Invalid bidPrice".into()))?;
        let ask: f64 = resp
            .ask_price
            .parse()
            .map_err(|_| ExchangeError::Api("Invalid askPrice".into()))?;
        let mid = (bid + ask) / 2.0;
        if !mid.is_finite() || mid <= 0.0 {
            return Err(ExchangeError::Api("Invalid mid for spread".into()));
        }
        Ok((ask - bid) / mid * 10_000.0)
    }

    /// TFAI-Q01: spread + ATR/close (spot'ta funding yok).
    pub async fn build_rca_open_market_snapshot(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> RcaOpenMarketSnapshot {
        let (spread_res, klines_res) = tokio::join!(
            self.fetch_book_ticker_spread_bps(symbol),
            self.fetch_klines_impl(symbol, timeframe.to_binance_interval(), 50),
        );
        let spread_at_open_bps = spread_res.ok();
        let volatility_at_open = klines_res.ok().and_then(|candles| {
            if candles.len() < 15 {
                return None;
            }
            let atr_val = atr(&candles, 14)?;
            let close = candles.last()?.close;
            if close.abs() > 1e-12 && atr_val.is_finite() {
                Some(atr_val / close.abs())
            } else {
                None
            }
        });
        RcaOpenMarketSnapshot {
            volatility_at_open,
            spread_at_open_bps,
            funding_rate_at_open: None,
        }
    }
}

impl Default for BinanceSpotClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExchangeConnector for BinanceSpotClient {
    fn exchange(&self) -> Exchange {
        Exchange::Binance
    }

    fn market_type(&self) -> MarketType {
        MarketType::Spot
    }

    fn set_trace_id_for_request(&self, trace_id: Option<&str>) {
        let tp = trace_id.and_then(traceparent_from_uuid);
        if let Ok(mut g) = self.traceparent.lock() {
            *g = tp;
        }
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
            ExchangeError::Api("Binance Spot: API key not configured".into())
        })?;
        let secret = self.secret_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Spot: Secret key not configured".into())
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
            "{}/api/v3/order?{}&signature={}",
            BINANCE_SPOT_API, query, signature
        );

        log::info!("[SPOT] Market order: {} {} qty={:.6}", side_str, symbol, quantity);

        let resp = self
            .apply_traceparent(self.client.post(&url).header("X-MBX-APIKEY", api_key))
            .send()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        if !status.is_success() {
            return Err(classify_binance_json("binance_spot", status.as_u16(), &body).into());
        }

        let fills = body["fills"].as_array();
        let avg_price = fills
            .and_then(|f| {
                let (total_qty, total_cost) = f.iter().fold((0.0_f64, 0.0_f64), |(q, c), fill| {
                    let fq: f64 = fill["qty"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let fp: f64 = fill["price"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    (q + fq, c + fq * fp)
                });
                if total_qty > 0.0 { Some(total_cost / total_qty) } else { None }
            })
            .unwrap_or(0.0);

        Ok(OrderResponse {
            order_id: body["orderId"].to_string(),
            symbol: body["symbol"].as_str().unwrap_or(symbol).to_string(),
            side,
            executed_qty: body["executedQty"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(quantity),
            avg_price,
        })
    }

    async fn get_balance(&self, asset: &str) -> ExchangeResult<f64> {
        let api_key = self.api_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Spot: API key not configured".into())
        })?;
        let secret = self.secret_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Spot: Secret key not configured".into())
        })?;

        let ts = sign::timestamp_ms();
        let query = format!("timestamp={}", ts);
        let signature = sign::sign(&query, secret);
        let url = format!(
            "{}/api/v3/account?{}&signature={}",
            BINANCE_SPOT_API, query, signature
        );

        let resp = self
            .send_get(|| self.client.get(&url).header("X-MBX-APIKEY", api_key))
            .await?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        if !status.is_success() {
            return Err(classify_binance_json("binance_spot", status.as_u16(), &body).into());
        }

        let asset_upper = asset.to_uppercase();
        if let Some(balances) = body["balances"].as_array() {
            for b in balances {
                if b["asset"].as_str() == Some(&asset_upper) {
                    let free: f64 = b["free"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    return Ok(free);
                }
            }
        }
        Ok(0.0)
    }

    async fn fetch_rca_open_market_snapshot(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> RcaOpenMarketSnapshot {
        self.build_rca_open_market_snapshot(symbol, timeframe).await
    }
}
