//! Binance Spot and Futures API connector

mod spot;
mod futures;

pub use spot::BinanceSpotClient;
pub use futures::BinanceFuturesClient;
