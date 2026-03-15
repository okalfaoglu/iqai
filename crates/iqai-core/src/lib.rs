//! IQAI Core - Smart Money Structure trading engine

pub mod app_config;
pub mod auto_trader;
pub mod backtest;
pub mod config;
pub mod dip_confluence;
pub mod logging;
pub mod elliott;
pub mod elliott_detector;
pub mod impulse_detector;
pub mod exchange;
pub mod indicators;
pub mod q_radar_analysis;
pub mod reversal;
pub mod signal;
pub mod trade_db;
pub mod trade_manager;
pub mod types;

pub use app_config::{AppConfig, LoggingConfig, LogTarget, NotificationConfig};
pub use backtest::scan_historical_q_setups;
pub use config::Config;
pub use logging::{debug, error, info, init_from_config, trace, warn};
pub use trade_manager::{Position, PositionSide, TradeAction, TradeManager};
pub use exchange::{ExchangeConnector, ExchangeError, OrderSide, OrderResponse};
pub use signal::{CandleBuffer, SignalEngine};
pub use elliott_detector::{collect_swings, compute_elliott, ElliottDetectorResult};
pub use impulse_detector::{detect_impulse, ImpulseDetectorState, ImpulseStage};
pub use q_radar_analysis::{compute_q_radar_opportunity, QRadarOpportunityAnalysis};
pub use reversal::{
    compute_reversal_analysis, get_dip_price_and_index, get_peak_price_and_index,
    DipAnalysis, PeakAnalysis, ReversalAnalysis,
};
pub use trade_db::{QAnalizDetectionRecord, SymbolPnlStats, TradeDb};
pub use auto_trader::TradingMode;
pub use types::*;
