//! Configuration matching Pine Script Smart Money Structure inputs

use serde::{Deserialize, Serialize};

use crate::types::Timeframe;

/// Full configuration for Smart Money Structure engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // General
    pub pivot_length: u32,
    pub momentum_threshold_base: f64,
    pub tp_points: i32,
    pub sl_points: i32,
    pub min_signal_distance: u32,
    pub tp_box_height_pct: f64,
    pub pre_momentum_factor_base: f64,
    pub short_trend_period: u32,
    pub long_trend_period: u32,

    // Signal Filters
    pub use_momentum_filter: bool,
    pub use_trend_filter: bool,
    pub higher_tf: Timeframe,
    pub use_lower_tf_filter: bool,
    pub lower_tf: Timeframe,
    pub use_volume_filter: bool,
    pub use_breakout_filter: bool,
    pub show_get_ready: bool,
    pub restrict_repeated_signals: bool,
    pub restrict_trend_tf: Timeframe,

    // Advanced Analysis
    pub enable_liquidity_zones: bool,
    pub enable_market_profile: bool,
    pub enable_divergence_scanner: bool,
    pub enable_trend_analysis: bool,

    // Volume & Breakout
    pub volume_long_period: u32,
    pub volume_short_period: u32,
    pub breakout_period: u32,

    // Trade Management (kar koruma, trailing stop, kısmi çıkış)
    pub enable_trade_management: bool,
    pub breakeven_r: f64,           // 1R kârda SL'i girişe taşı (1.0 = risk miktarı)
    pub tp1_r: f64,                 // İlk kısmi hedef (örn. 1.0 = 1R)
    pub tp2_r: f64,                 // İkinci kısmi hedef (örn. 2.0 = 2R)
    pub partial_tp1_pct: f64,       // TP1'de kapatılacak pozisyon % (örn. 0.33)
    pub partial_tp2_pct: f64,       // TP2'de kapatılacak pozisyon % (örn. 0.33)
    pub atr_trailing_period: u32,   // Chandelier/ATR periyodu (22 önerilir)
    pub atr_trailing_mult: f64,     // ATR çarpanı (2-3, 3 daha geniş stop)
    pub use_chandelier_exit: bool,  // true = Chandelier, false = basit ATR trailing
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pivot_length: 5,
            momentum_threshold_base: 0.01,
            tp_points: 10,
            sl_points: 10,
            min_signal_distance: 5,
            tp_box_height_pct: 0.5,
            pre_momentum_factor_base: 0.5,
            short_trend_period: 30,
            long_trend_period: 100,

            use_momentum_filter: true,
            use_trend_filter: true,
            higher_tf: Timeframe::M5,
            use_lower_tf_filter: true,
            lower_tf: Timeframe::M5,
            use_volume_filter: true,
            use_breakout_filter: true,
            show_get_ready: false,
            restrict_repeated_signals: true,
            restrict_trend_tf: Timeframe::M5,

            enable_liquidity_zones: false,
            enable_market_profile: true,
            enable_divergence_scanner: true,
            enable_trend_analysis: true,

            volume_long_period: 50,
            volume_short_period: 5,
            breakout_period: 5,

            enable_trade_management: true,
            breakeven_r: 1.0,
            tp1_r: 1.0,
            tp2_r: 2.0,
            partial_tp1_pct: 0.33,
            partial_tp2_pct: 0.33,
            atr_trailing_period: 22,
            atr_trailing_mult: 3.0,
            use_chandelier_exit: true,
        }
    }
}
