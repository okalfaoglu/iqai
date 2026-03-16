//! Exchange trait - abstraction for multi-exchange support

use async_trait::async_trait;

use crate::types::{Candle, Exchange, MarketType, Timeframe};

/// Result type for exchange operations (error type provided by connector)
pub type ExchangeResult<T, E = ExchangeError> = Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum ExchangeError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error: {0}")]
    Api(String),
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
