//! IQAI Core - Smart Money Structure trading engine

pub mod aggregate;
pub mod config;
pub mod elliott;
pub mod impulse_detector;
pub mod exchange;
pub mod indicators;
pub mod signal;
pub mod trade_manager;
pub mod types;

pub use config::Config;
pub use trade_manager::{Position, PositionSide, TradeAction, TradeManager};
pub use exchange::{ExchangeConnector, ExchangeError, OrderSide, OrderResponse};
pub use signal::{CandleBuffer, SignalEngine};
pub use impulse_detector::{detect_impulse, ImpulseDetectorState, ImpulseStage};
pub use types::*;
