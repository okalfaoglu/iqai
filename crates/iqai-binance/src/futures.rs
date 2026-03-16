//! Binance Futures (USDT-M) API

use std::collections::HashMap;

use async_trait::async_trait;
use iqai_core::exchange::{ExchangeConnector, ExchangeError, ExchangeResult, OrderResponse, OrderSide};
use iqai_core::types::{Candle, Exchange, MarketType, Timeframe};
use reqwest::Client;
use tokio::sync::RwLock;

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
    /// Sembol bazlı exchangeInfo filtreleri (LOT_SIZE, MIN_NOTIONAL)
    symbol_filters: RwLock<HashMap<String, SymbolFilters>>,
    exchange_info_loaded: RwLock<bool>,
}

impl BinanceFuturesClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: None,
            secret_key: None,
            symbol_filters: RwLock::new(HashMap::new()),
            exchange_info_loaded: RwLock::new(false),
        }
    }

    pub fn with_credentials(api_key: String, secret_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: Some(api_key),
            secret_key: Some(secret_key),
            symbol_filters: RwLock::new(HashMap::new()),
            exchange_info_loaded: RwLock::new(false),
        }
    }

    /// GET /fapi/v1/exchangeInfo — tüm sembollerin LOT_SIZE ve MIN_NOTIONAL filtrelerini önbelleğe alır
    async fn ensure_exchange_info(&self) -> Result<(), ExchangeError> {
        if *self.exchange_info_loaded.read().await {
            return Ok(());
        }
        let url = format!("{}/fapi/v1/exchangeInfo", BINANCE_FUTURES_API);
        let body: serde_json::Value = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?
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
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ExchangeError::Http(e.to_string()))?
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
            .client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?
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
                .unwrap_or(quantity_parsed),
            avg_price: body["avgPrice"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(limit_price),
        })
    }

    async fn get_commission_bps(&self, symbol: &str) -> Option<u32> {
        self.fetch_commission_rate(symbol).await.ok()
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
