//! Binance Spot and Futures API connector

mod http_retry;
mod sign;
mod spot;
mod futures;

pub use spot::BinanceSpotClient;
pub use futures::BinanceFuturesClient;
