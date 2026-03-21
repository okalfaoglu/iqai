//! Binance Futures (USDT-M) API

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use iqai_core::exchange::{
    classify_binance_json, ExchangeConnector, ExchangeError, ExchangeResult, OrderResponse,
    OrderSide, RcaOpenMarketSnapshot,
};
use iqai_core::indicators::atr;
use iqai_core::market_context::{FundingRate, OpenInterest, OrderBookSnapshot};
use iqai_core::traceparent_from_uuid;
use iqai_core::types::{Candle, Exchange, MarketType, Timeframe};
use reqwest::Client;
use tokio::sync::RwLock;

use crate::http_retry::send_get_retry;
use crate::sign;

const BINANCE_FUTURES_API: &str = "https://fapi.binance.com";

/// LOT_SIZE ve MIN_NOTIONAL kurallarına uygun sembol filtreleri
#[derive(Clone, Debug)]
pub struct SymbolFilters {
    pub min_qty: f64,
    pub max_qty: f64,
    pub step_size: f64,
    pub min_notional: f64,
}

/// stepSize ondalık basamak sayısı (örn. 0.001 -> 3, 0.1 -> 1)
fn step_size_precision(step: f64) -> usize {
    if step <= 0.0 || step >= 1.0 {
        let t = step.to_string();
        if let Some(dot) = t.find('.') {
            return t[dot + 1..].trim_end_matches('0').len().min(8);
        }
        return 0;
    }
    let s = format!("{:.10e}", step);
    if let Some(e) = s.find('e') {
        let exp: i32 = s[e + 1..].trim().parse().unwrap_or(0);
        if exp < 0 {
            return (-exp) as usize;
        }
    }
    let t = step.to_string();
    if let Some(dot) = t.find('.') {
        t[dot + 1..].trim_end_matches('0').len().min(8)
    } else {
        0
    }
}

/// Miktarı stepSize'a göre aşağı yuvarla (Binance LOT_SIZE)
fn round_down_to_step(qty: f64, step_size: f64) -> f64 {
    if step_size <= 0.0 {
        return qty;
    }
    let n = (qty / step_size).floor() * step_size;
    let prec = step_size_precision(step_size);
    (n * 10_f64.powi(prec as i32)).round() / 10_f64.powi(prec as i32)
}

/// Miktarı stepSize hassasiyetinde formatla (Binance'in kabul ettiği string)
fn format_quantity(qty: f64, step_size: f64) -> String {
    let rounded = round_down_to_step(qty, step_size);
    let prec = step_size_precision(step_size);
    format!("{:.prec$}", rounded, prec = prec)
}

/// Binance USDT-M Futures client
pub struct BinanceFuturesClient {
    client: Client,
    api_key: Option<String>,
    secret_key: Option<String>,
    /// `process_signal` sırasında `ExchangeTraceScopeGuard` ile set; GET/POST `traceparent`.
    traceparent: Mutex<Option<String>>,
    /// Sembol bazlı komisyon oranı cache'i (signed USER_DATA endpoint).
    commission_cache: RwLock<HashMap<String, CommissionCacheEntry>>,
    /// Cache TTL (ms). 0 ise her çağrıda anlık fetch.
    commission_cache_ttl_ms: u64,
    /// Sembol bazlı exchangeInfo filtreleri (LOT_SIZE, MIN_NOTIONAL)
    symbol_filters: RwLock<HashMap<String, SymbolFilters>>,
    exchange_info_loaded: RwLock<bool>,
}

#[derive(Clone, Debug)]
struct CommissionCacheEntry {
    fetched_at_ms: u64,
    value: Option<u32>,
}

impl BinanceFuturesClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: None,
            secret_key: None,
            traceparent: Mutex::new(None),
            commission_cache: RwLock::new(HashMap::new()),
            commission_cache_ttl_ms: 600_000,
            symbol_filters: RwLock::new(HashMap::new()),
            exchange_info_loaded: RwLock::new(false),
        }
    }

    pub fn with_credentials(api_key: String, secret_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: Some(api_key),
            secret_key: Some(secret_key),
            traceparent: Mutex::new(None),
            commission_cache: RwLock::new(HashMap::new()),
            commission_cache_ttl_ms: 600_000,
            symbol_filters: RwLock::new(HashMap::new()),
            exchange_info_loaded: RwLock::new(false),
        }
    }

    /// Binance komisyon oranı cache TTL ayarı.
    ///
    /// - `ttl_ms=0`  => her çağrıda anlık fetch
    /// - `ttl_ms>0` => TTL içinde cache kullan
    pub fn with_commission_cache_ttl_ms(mut self, ttl_ms: u64) -> Self {
        self.commission_cache_ttl_ms = ttl_ms;
        self
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

    /// GET /fapi/v1/exchangeInfo — tüm sembollerin LOT_SIZE ve MIN_NOTIONAL filtrelerini önbelleğe alır
    async fn ensure_exchange_info(&self) -> Result<(), ExchangeError> {
        if *self.exchange_info_loaded.read().await {
            return Ok(());
        }
        let url = format!("{}/fapi/v1/exchangeInfo", BINANCE_FUTURES_API);
        let body: serde_json::Value = self
            .send_get(|| self.client.get(&url))
            .await?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        let symbols = body["symbols"]
            .as_array()
            .ok_or_else(|| ExchangeError::Api("exchangeInfo: missing symbols".into()))?;
        let mut filters = self.symbol_filters.write().await;
        for sym in symbols {
            let symbol = match sym["symbol"].as_str() {
                Some(s) => s.to_string(),
                None => continue,
            };
            let mut min_qty = 0.0_f64;
            let mut max_qty = 1e15;
            let mut step_size = 1e-8;
            let mut min_notional = 5.0_f64;
            let filters_arr = match sym["filters"].as_array() {
                Some(a) => a,
                None => continue,
            };
            for f in filters_arr {
                let ft = f["filterType"].as_str().unwrap_or("");
                if ft == "LOT_SIZE" {
                    min_qty = f["minQty"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    max_qty = f["maxQty"].as_str().and_then(|s| s.parse().ok()).unwrap_or(1e15);
                    step_size = f["stepSize"].as_str().and_then(|s| s.parse().ok()).unwrap_or(1e-8);
                } else if ft == "MIN_NOTIONAL" {
                    min_notional = f["notional"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .or_else(|| f["minNotional"].as_str().and_then(|s| s.parse().ok()))
                        .unwrap_or(5.0);
                }
            }
            filters.insert(
                symbol.clone(),
                SymbolFilters {
                    min_qty,
                    max_qty,
                    step_size,
                    min_notional,
                },
            );
        }
        *self.exchange_info_loaded.write().await = true;
        log::debug!("[FUTURES] exchangeInfo loaded, {} symbols", filters.len());
        Ok(())
    }

    /// Miktarı sembol kurallarına göre yuvarlar; min_notional için fiyat gerekir (None ise sadece LOT_SIZE uygulanır)
    async fn adjust_quantity(
        &self,
        symbol: &str,
        quantity: f64,
        price: Option<f64>,
    ) -> Result<String, ExchangeError> {
        self.ensure_exchange_info().await?;
        let filters = self
            .symbol_filters
            .read()
            .await
            .get(symbol)
            .cloned()
            .ok_or_else(|| ExchangeError::Api(format!("Symbol {} not found in exchangeInfo", symbol)))?;
        let mut qty = round_down_to_step(quantity, filters.step_size);
        if qty < filters.min_qty {
            return Err(ExchangeError::Api(format!(
                "Quantity {} below minQty {} for {}",
                qty, filters.min_qty, symbol
            )));
        }
        if qty > filters.max_qty {
            qty = filters.max_qty;
            qty = round_down_to_step(qty, filters.step_size);
        }
        if let Some(p) = price {
            let notional = qty * p;
            if notional < filters.min_notional {
                return Err(ExchangeError::Api(format!(
                    "Notional {} below min {} for {} (qty={}, price={})",
                    notional, filters.min_notional, symbol, qty, p
                )));
            }
        }
        Ok(format_quantity(qty, filters.step_size))
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

    /// Geçmiş mumları tarih aralığına göre çeker (startTime/endTime, 1500’lük chunk’lar).
    pub async fn fetch_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_time_ms: i64,
        end_time_ms: i64,
    ) -> Result<Vec<Candle>, ExchangeError> {
        let symbol_futures = if symbol.ends_with("USDT") {
            symbol.to_string()
        } else {
            format!("{}USDT", symbol)
        };
        let mut all: Vec<Candle> = Vec::new();
        let mut start = start_time_ms;
        const CHUNK: u32 = 1500;
        loop {
            let url = format!(
                "{}/fapi/v1/klines?symbol={}&interval={}&limit={}&startTime={}&endTime={}",
                BINANCE_FUTURES_API,
                symbol_futures,
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

    /// Funding rate (son periyot) – GET /fapi/v1/fundingRate
    pub async fn fetch_funding_rate(&self, symbol: &str) -> Result<FundingRate, ExchangeError> {
        let symbol_futures = if symbol.ends_with("USDT") {
            symbol.to_string()
        } else {
            format!("{}USDT", symbol)
        };
        let url = format!(
            "{}/fapi/v1/fundingRate?symbol={}&limit=1",
            BINANCE_FUTURES_API,
            symbol_futures
        );
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct FundingRow {
            funding_rate: String,
            funding_time: i64,
        }
        let resp: Vec<FundingRow> = self
            .send_get(|| self.client.get(&url))
            .await?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        let row = resp.into_iter().next().ok_or_else(|| {
            ExchangeError::Api("fundingRate: empty response".into())
        })?;
        let rate: f64 = row
            .funding_rate
            .parse()
            .map_err(|_| ExchangeError::Api("Invalid funding rate".into()))?;
        Ok(FundingRate {
            rate,
            next_funding_time: Some(row.funding_time),
        })
    }

    /// GET /fapi/v1/ticker/bookTicker — spread'i bps (mid'e göre) döner.
    pub async fn fetch_book_ticker_spread_bps(&self, symbol: &str) -> Result<f64, ExchangeError> {
        let symbol_futures = if symbol.ends_with("USDT") {
            symbol.to_string()
        } else {
            format!("{}USDT", symbol)
        };
        let url = format!(
            "{}/fapi/v1/ticker/bookTicker?symbol={}",
            BINANCE_FUTURES_API, symbol_futures
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

    /// TFAI-Q01: funding + spread + ATR/close (sinyal TF mumları).
    pub async fn build_rca_open_market_snapshot(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> RcaOpenMarketSnapshot {
        let (funding_res, spread_res, klines_res) = tokio::join!(
            self.fetch_funding_rate(symbol),
            self.fetch_book_ticker_spread_bps(symbol),
            self.fetch_klines_impl(symbol, timeframe.to_binance_interval(), 50),
        );
        if funding_res.is_err() {
            log::debug!(
                "[FUTURES] RCA open: funding fetch skipped/err for {}",
                symbol
            );
        }
        if spread_res.is_err() {
            log::debug!("[FUTURES] RCA open: bookTicker skipped/err for {}", symbol);
        }
        let klines_len = klines_res.as_ref().ok().map(|c| c.len());
        let funding_rate_at_open = funding_res.ok().map(|f| f.rate);
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
        if volatility_at_open.is_none() {
            log::debug!(
                "[FUTURES] RCA open: ATR vol skipped for {} (klines len {:?})",
                symbol,
                klines_len
            );
        }
        RcaOpenMarketSnapshot {
            volatility_at_open,
            spread_at_open_bps,
            funding_rate_at_open,
        }
    }

    /// Açık pozisyon – GET /fapi/v1/openInterest
    pub async fn fetch_open_interest(&self, symbol: &str) -> Result<OpenInterest, ExchangeError> {
        let symbol_futures = if symbol.ends_with("USDT") {
            symbol.to_string()
        } else {
            format!("{}USDT", symbol)
        };
        let url = format!(
            "{}/fapi/v1/openInterest?symbol={}",
            BINANCE_FUTURES_API,
            symbol_futures
        );
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct OiResp {
            open_interest: String,
        }
        let resp: OiResp = self
            .send_get(|| self.client.get(&url))
            .await?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        let value: f64 = resp
            .open_interest
            .parse()
            .map_err(|_| ExchangeError::Api("Invalid open interest".into()))?;
        Ok(OpenInterest {
            value,
            change_pct: None,
        })
    }

    /// Order book derinliği – GET /fapi/v1/depth; ilk `levels` seviyeye göre notional ve imbalance.
    pub async fn fetch_order_book_snapshot(
        &self,
        symbol: &str,
        levels: u32,
    ) -> Result<OrderBookSnapshot, ExchangeError> {
        let symbol_futures = if symbol.ends_with("USDT") {
            symbol.to_string()
        } else {
            format!("{}USDT", symbol)
        };
        let limit = levels.min(100);
        let url = format!(
            "{}/fapi/v1/depth?symbol={}&limit={}",
            BINANCE_FUTURES_API,
            symbol_futures,
            limit
        );
        #[derive(serde::Deserialize)]
        struct DepthResp {
            bids: Vec<[String; 2]>,
            asks: Vec<[String; 2]>,
        }
        let resp: DepthResp = self
            .send_get(|| self.client.get(&url))
            .await?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        let bid_notional: f64 = resp
            .bids
            .iter()
            .filter_map(|b| {
                let price: f64 = b[0].parse().ok()?;
                let qty: f64 = b[1].parse().ok()?;
                Some(price * qty)
            })
            .sum();
        let ask_notional: f64 = resp
            .asks
            .iter()
            .filter_map(|a| {
                let price: f64 = a[0].parse().ok()?;
                let qty: f64 = a[1].parse().ok()?;
                Some(price * qty)
            })
            .sum();
        let total = bid_notional + ask_notional;
        let imbalance = if total > 0.0 {
            (bid_notional - ask_notional) / total
        } else {
            0.0
        };
        Ok(OrderBookSnapshot {
            bid_notional,
            ask_notional,
            imbalance,
        })
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
            .send_get(|| self.client.get(&url))
            .await?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        resp.price
            .parse()
            .map_err(|_| ExchangeError::Api("Invalid ticker price".to_string()))
    }

    /// GET /fapi/v1/commissionRate (USER_DATA) — taker oranı basis points (örn. 4 = %0.04)
    async fn fetch_commission_rate(&self, symbol: &str) -> Result<u32, ExchangeError> {
        let secret = self
            .secret_key
            .as_deref()
            .ok_or_else(|| ExchangeError::Api("Secret key required for commission rate".into()))?;
        let symbol_upper = symbol.to_uppercase();
        let ts = sign::timestamp_ms();
        let query = format!("symbol={}&timestamp={}", symbol_upper, ts);
        let signature = sign::sign(&query, secret);
        let url = format!(
            "{}/fapi/v1/commissionRate?{}&signature={}",
            BINANCE_FUTURES_API, query, signature
        );
        let api_key = self
            .api_key
            .as_deref()
            .ok_or_else(|| ExchangeError::Api("API key required".into()))?;
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CommissionRate {
            taker_commission_rate: String,
        }
        let resp: CommissionRate = self
            .send_get(|| self.client.get(&url).header("X-MBX-APIKEY", api_key))
            .await?
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        let rate: f64 = resp
            .taker_commission_rate
            .parse()
            .map_err(|_| ExchangeError::Api("Invalid commission rate".into()))?;
        Ok((rate * 10_000.0).round() as u32)
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
            ExchangeError::Api("Binance Futures: API key not configured".into())
        })?;
        let secret = self.secret_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Futures: Secret key not configured".into())
        })?;

        let symbol_upper = symbol.to_uppercase();
        let price = self.fetch_ticker_price(&symbol_upper).await.ok();
        let quantity_str = self
            .adjust_quantity(&symbol_upper, quantity, price)
            .await?;
        let quantity_parsed: f64 = quantity_str.parse().unwrap_or(quantity);

        let side_str = match side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };
        let ts = sign::timestamp_ms();
        let query = format!(
            "symbol={}&side={}&type=MARKET&quantity={}&timestamp={}",
            symbol_upper,
            side_str,
            quantity_str,
            ts
        );
        let signature = sign::sign(&query, secret);
        let url = format!(
            "{}/fapi/v1/order?{}&signature={}",
            BINANCE_FUTURES_API, query, signature
        );

        log::info!("[FUTURES] Market order: {} {} qty={} (requested {:.6})", side_str, symbol, quantity_str, quantity);

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
            return Err(
                classify_binance_json("binance_futures", status.as_u16(), &body).into(),
            );
        }

        Ok(OrderResponse {
            order_id: body["orderId"].to_string(),
            symbol: body["symbol"].as_str().unwrap_or(symbol).to_string(),
            side,
            executed_qty: body["executedQty"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(quantity_parsed),
            avg_price: body["avgPrice"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
        })
    }

    async fn place_limit_order_ioc(
        &self,
        symbol: &str,
        side: OrderSide,
        quantity: f64,
        limit_price: f64,
    ) -> ExchangeResult<OrderResponse> {
        let api_key = self.api_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Futures: API key not configured".into())
        })?;
        let secret = self.secret_key.as_deref().ok_or_else(|| {
            ExchangeError::Api("Binance Futures: Secret key not configured".into())
        })?;
        let symbol_upper = symbol.to_uppercase();
        let price = self.fetch_ticker_price(&symbol_upper).await.ok();
        let quantity_str = self
            .adjust_quantity(&symbol_upper, quantity, price)
            .await?;
        let quantity_parsed: f64 = quantity_str.parse().unwrap_or(quantity);
        let side_str = match side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };
        let ts = sign::timestamp_ms();
        let query = format!(
            "symbol={}&side={}&type=LIMIT&timeInForce=IOC&quantity={}&price={}&timestamp={}",
            symbol_upper,
            side_str,
            quantity_str,
            format!("{:.2}", limit_price),
            ts
        );
        let signature = sign::sign(&query, secret);
        let url = format!(
            "{}/fapi/v1/order?{}&signature={}",
            BINANCE_FUTURES_API, query, signature
        );
        log::info!(
            "[FUTURES] Limit IOC: {} {} qty={} @ {}",
            side_str, symbol, quantity_str, limit_price
        );
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
            return Err(
                classify_binance_json("binance_futures", status.as_u16(), &body).into(),
            );
        }
        Ok(OrderResponse {
            order_id: body["orderId"].to_string(),
            symbol: body["symbol"].as_str().unwrap_or(symbol).to_string(),
            side,
            executed_qty: body["executedQty"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(quantity_parsed),
            avg_price: body["avgPrice"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(limit_price),
        })
    }

    async fn get_commission_bps(&self, symbol: &str) -> Option<u32> {
        let now_ms = sign::timestamp_ms();
        let symbol_upper = symbol.to_uppercase();

        // TTL=0 => her çağrıda anlık fetch
        if self.commission_cache_ttl_ms > 0 {
            if let Some(entry) = self
                .commission_cache
                .read()
                .await
                .get(&symbol_upper)
                .cloned()
            {
                let age = now_ms.saturating_sub(entry.fetched_at_ms);
                if age < self.commission_cache_ttl_ms {
                    return entry.value;
                }
            }
        }

        let value = self.fetch_commission_rate(symbol).await.ok();
        let entry = CommissionCacheEntry {
            fetched_at_ms: now_ms,
            value,
        };
        self.commission_cache
            .write()
            .await
            .insert(symbol_upper, entry);
        value
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
            .send_get(|| self.client.get(&url).header("X-MBX-APIKEY", api_key))
            .await?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;

        if !status.is_success() {
            return Err(
                classify_binance_json("binance_futures", status.as_u16(), &body).into(),
            );
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

    async fn fetch_rca_open_market_snapshot(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> RcaOpenMarketSnapshot {
        self.build_rca_open_market_snapshot(symbol, timeframe).await
    }
}
