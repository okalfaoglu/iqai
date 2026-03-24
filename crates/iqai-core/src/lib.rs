//! IQAI Core - Smart Money Structure trading engine

pub mod analysis_snapshot;
pub mod app_config;
pub mod auto_trader;
pub mod backtest;
pub mod candlestick_patterns;
pub mod market_context;
pub mod config;
pub mod classic_patterns;
pub mod dip_confluence;
pub mod dip_tepe_scoring;
pub mod logging;
pub mod elliott;
pub mod elliott_fusion;
pub mod elliott_detector;
pub mod impulse_detector;
pub mod binance_error;
pub mod exchange;
pub mod fake_breakout;
pub mod hash_util;
pub mod indicators;
pub mod q_radar_analysis;
pub mod reversal;
pub mod signal;
pub mod position_rca;
pub mod trade_db;
pub mod trade_manager;
pub mod types;
pub mod strategy;
pub mod strategy_engine;
pub mod smart_money;
pub mod sli;
pub mod trace_context;

pub use app_config::{
    AppConfig, LoggingConfig, LogTarget, NotificationConfig, SmartMoneyConfig, TradingConfig,
    WebConfig,
};
pub use backtest::{run_backtest, scan_historical_q_setups, BacktestResult, BacktestTrade};
pub use config::Config;
pub use logging::{debug, error, info, init_from_config, trace, warn};
pub use trade_manager::{Position, PositionSide, TradeAction, TradeManager};
pub use exchange::{
    classify_binance_json, prometheus_exchange_normalized_errors, AlertTier, ExchangeConnector,
    ExchangeError, ExchangeErrorCategory, ExchangeTraceScopeGuard, NormalizedExchangeError,
    OrderSide, OrderResponse, RcaOpenMarketSnapshot,
};
pub use signal::{CandleBuffer, SignalEngine};
pub use elliott_fusion::{ElliottFusionChartOverlay, ElliottFusionExtras, ElliottPatternStability};
pub use elliott_detector::{
    collect_swings, compute_elliott, ElliottDetectorResult, ElliottProjectionPathLeg,
};
pub use impulse_detector::{detect_impulse, ImpulseDetectorState, ImpulseStage};
pub use q_radar_analysis::{
    compute_q_radar_opportunity, radar_setup_alignment_score, QRadarOpportunityAnalysis,
};
pub use dip_tepe_scoring::{compute_dip_tepe_score, DipTepeScore, SignalScore};
pub use candlestick_patterns::{
    detect_candle_patterns, CandlePatternSignals, DEFAULT_CANDLESTICK_MIN_RANGE_ATR_RATIO,
    DEFAULT_CANDLESTICK_NOISE_ATR_PERIOD,
};
pub use market_context::{
    FundingRate, LiquidationZone, MarketContext, OnChainSummary, OpenInterest, OrderBookSnapshot,
};
pub use reversal::{
    compute_reversal_analysis, get_dip_price_and_index, get_peak_price_and_index,
    DipAnalysis, PeakAnalysis, ReversalAnalysis,
};
pub use analysis_snapshot::{build_analysis_snapshot, AnalysisSnapshot};
pub use hash_util::sha256_hex;
pub use trace_context::traceparent_from_uuid;
pub use position_rca::{ClosePositionRca, PositionOpenRca, close_reason_to_canonical};
pub use sli::{render_prometheus_sli, render_prometheus_sli_minimal};
pub use trade_db::{
    fingerprint_analysis_snapshots_for_audit, AiExplanationRecord, AnalysisOutcomeRecord,
    AnalysisSnapshotRecord, QAnalizDetectionRecord, SymbolPnlStats, TradeDb,
};
pub use auto_trader::{format_trade_correlation, TradingMode};
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
    SmartMoneyRadarScore,
    SmartMoneyRadarSignal,
    WyckoffTag,
    WyckoffEvent,
    WyckoffState,
    build_smart_money_context_for_series,
    compute_smart_money_radar_score,
};
pub use fake_breakout::{detect_fake_breakout_signal, FakeBreakoutConfig, FakeBreakoutSignal};
