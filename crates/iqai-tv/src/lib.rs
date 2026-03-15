//! TradingView connector: varsayılan saf Rust (WebSocket). İsteğe bağlı HTTP veya Python subprocess.

mod client;
mod native;

pub use client::TvConnectorClient;
pub use native::fetch_klines_native;
