//! Exchange trait - abstraction for multi-exchange support

use async_trait::async_trait;

pub use crate::binance_error::{
    classify_binance_json, prometheus_exchange_normalized_errors, AlertTier,
    ExchangeErrorCategory, NormalizedExchangeError,
};

use crate::types::{Candle, Exchange, MarketType, Timeframe};

/// Açılış anı RCA için opsiyonel piyasa göstergeleri (TFAI-Q01 feed).
/// Borsa bağlayıcısı yoksa veya hata olursa alanlar `None` kalır.
#[derive(Debug, Clone, Default)]
pub struct RcaOpenMarketSnapshot {
    /// ATR(14) / son kapanış — boyutsuz (ör. 0.02 ≈ %2).
    pub volatility_at_open: Option<f64>,
    /// (ask − bid) / mid × 10_000 (basis points).
    pub spread_at_open_bps: Option<f64>,
    /// Perpetual funding oranı (yoksa `None`, spot için genelde `None`).
    pub funding_rate_at_open: Option<f64>,
}

/// Result type for exchange operations (error type provided by connector)
pub type ExchangeResult<T, E = ExchangeError> = Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum ExchangeError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error: {0}")]
    Api(String),
    /// Binance (ve benzeri) normalize edilmiş API hatası — TFAI-Q04.
    #[error(transparent)]
    Normalized(#[from] NormalizedExchangeError),
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),
}

/// Exchange connector trait - implemented by Binance, etc.
#[async_trait]
pub trait ExchangeConnector: Send + Sync {
    fn exchange(&self) -> Exchange;
    fn market_type(&self) -> MarketType;

    /// Fetch OHLCV candles
    async fn fetch_klines(
        &self,
        symbol: &str,
        interval: Timeframe,
        limit: u32,
    ) -> ExchangeResult<Vec<Candle>>;

    /// Place market order (optional - for CLI execution)
    async fn place_market_order(
        &self,
        symbol: &str,
        side: OrderSide,
        quantity: f64,
    ) -> ExchangeResult<OrderResponse>;

    /// Limit emir (IOC): en fazla limit fiyattan doldurulur; dolmayan kısım iptal.
    async fn place_limit_order_ioc(
        &self,
        symbol: &str,
        side: OrderSide,
        quantity: f64,
        limit_price: f64,
    ) -> ExchangeResult<OrderResponse> {
        let _ = (symbol, side, quantity, limit_price);
        Err(ExchangeError::Api("Limit order (IOC) not supported".into()))
    }

    /// Kullanıcı komisyon oranı (basis points). None ise config'deki commission_bps kullanılır.
    async fn get_commission_bps(&self, _symbol: &str) -> Option<u32> {
        None
    }

    /// Get account balance (optional)
    async fn get_balance(&self, asset: &str) -> ExchangeResult<f64>;

    /// `process_signal` içinde [`ExchangeTraceScopeGuard`] ile set edilir; Binance istemcisi W3C `traceparent` üretir.
    fn set_trace_id_for_request(&self, _trace_id: Option<&str>) {}

    /// Açılış RCA için spread / funding / ATR tabanlı volatilite (best-effort; hata → `None` alanları).
    async fn fetch_rca_open_market_snapshot(
        &self,
        _symbol: &str,
        _timeframe: Timeframe,
    ) -> RcaOpenMarketSnapshot {
        RcaOpenMarketSnapshot::default()
    }
}

/// RAII: Binance GET/POST isteklerine W3C `traceparent` (`trace_id` → [`crate::traceparent_from_uuid`]).
pub struct ExchangeTraceScopeGuard<'a> {
    exchange: &'a dyn ExchangeConnector,
}

impl<'a> ExchangeTraceScopeGuard<'a> {
    pub fn new(exchange: &'a dyn ExchangeConnector, trace_id: &str) -> Self {
        exchange.set_trace_id_for_request(Some(trace_id));
        Self { exchange }
    }
}

impl Drop for ExchangeTraceScopeGuard<'_> {
    fn drop(&mut self) {
        self.exchange.set_trace_id_for_request(None);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct OrderResponse {
    pub order_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub executed_qty: f64,
    pub avg_price: f64,
}
