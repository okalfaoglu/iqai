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
