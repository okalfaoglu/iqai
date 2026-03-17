//! IQAI Core - Smart Money Structure trading engine

pub mod app_config;
pub mod auto_trader;
pub mod backtest;
pub mod candlestick_patterns;
pub mod config;
pub mod classic_patterns;
pub mod dip_confluence;
pub mod dip_tepe_scoring;
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
pub mod strategy;
pub mod strategy_engine;
pub mod smart_money;

pub use app_config::{AppConfig, LoggingConfig, LogTarget, NotificationConfig};
pub use backtest::{run_backtest, scan_historical_q_setups, BacktestResult, BacktestTrade};
pub use config::Config;
pub use logging::{debug, error, info, init_from_config, trace, warn};
pub use trade_manager::{Position, PositionSide, TradeAction, TradeManager};
pub use exchange::{ExchangeConnector, ExchangeError, OrderSide, OrderResponse};
pub use signal::{CandleBuffer, SignalEngine};
pub use elliott_detector::{collect_swings, compute_elliott, ElliottDetectorResult};
pub use impulse_detector::{detect_impulse, ImpulseDetectorState, ImpulseStage};
pub use q_radar_analysis::{compute_q_radar_opportunity, QRadarOpportunityAnalysis};
pub use dip_tepe_scoring::{compute_dip_tepe_score, DipTepeScore, SignalScore};
pub use candlestick_patterns::{detect_candle_patterns, CandlePatternSignals};
pub use reversal::{
    compute_reversal_analysis, get_dip_price_and_index, get_peak_price_and_index,
    DipAnalysis, PeakAnalysis, ReversalAnalysis,
};
pub use trade_db::{QAnalizDetectionRecord, SymbolPnlStats, TradeDb};
pub use auto_trader::TradingMode;
pub use types::*;
pub use classic_patterns::{
    ClassicPatternDetection,
    ClassicPatternKind,
    ClassicPatternTarget,
    PatternDirection,
    detect_classic_patterns,
};
pub use strategy::{
    StrategyDirection,
    StrategyPlan,
    StrategyRole,
    StrategyScenario,
    StrategyScenarioKind,
    StrategyTarget,
    build_strategies_for_series,
    build_scenarios_for_series,
};
pub use strategy_engine::{
    StrategyPlanBacktestResult,
    run_strategy_plan_backtest,
};
pub use smart_money::{
    LiquidityKind,
    LiquidityLevel,
    OrderBlockSide,
    OrderBlockZone,
    Po3Phase,
    SmartMoneyContext,
    WyckoffTag,
    WyckoffEvent,
    WyckoffState,
    build_smart_money_context_for_series,
};
