//! Configuration matching Pine Script Smart Money Structure inputs

use serde::{Deserialize, Serialize};

use crate::{app_config::SmartMoneyConfig, types::Timeframe};

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

    // Elliott Wave görselleştirme ayarları
    /// Fibo seviyelerinin dalga çiziminden sonra sağa bırakacağı boşluk (bar sayısı)
    pub elliott_fibo_gap_bars: u32,
    /// Fibo seviyelerinin yatay çizgi uzunluğu (bar sayısı)
    pub elliott_fibo_length_bars: u32,
    /// Elliott setup minimum R/R eşiği (altındaki sinyaller zayıf işaretlenir)
    pub elliott_min_rr: f64,

    // Q-ANALİZ / Q-RADAR parametreleri
    /// Q-Setup minimum skoru (0-100)
    pub q_score_threshold: f64,
    /// Elit Q-Setup skoru (0-100) – istatistiksel olarak daha güçlü
    pub q_elite_threshold: f64,
    /// Minimum risk/ödül oranı (TP mesafesi / SL mesafesi)
    pub q_min_rr: f64,
    /// Radar için erken faz minimum (0-1)
    pub q_radar_phase_min: f64,
    /// Radar için erken faz maksimum (0-1)
    pub q_radar_phase_max: f64,
    /// Giriş için ideal faz minimum (0-1)
    pub q_entry_phase_min: f64,
    /// Giriş için ideal faz maksimum (0-1)
    pub q_entry_phase_max: f64,
    /// Geç faz eşiği (poz koruma / çıkış alanı)
    pub q_late_phase: f64,
    /// Poz koruma için minimum kâr (R cinsinden)
    pub q_protect_min_r: f64,
    /// Poz koruma geldiğinde kilitlenecek minimum kâr (R cinsinden)
    pub q_protect_lock_r: f64,

    // Q-Setup giriş bölgesi (pivot + ATR): L_pivot + α·ATR .. β·ATR, SL = L_pivot − γ·ATR (long)
    pub q_entry_atr_alpha: f64,
    pub q_entry_atr_beta: f64,
    pub q_sl_atr_gamma: f64,

    /// Yapıya uygun TP: pivot–swing mesafesi bu oranla projekte edilir (örn. 1.618)
    pub q_tp_structure_ext: f64,
    /// Yapı TP’nin üst sınırı (R cinsinden; örn. 5.0)
    pub q_tp_max_r: f64,

    /// Dip/tepe tespitinde MTF destek zorunlu olsun (true ise sadece confluence.mtf_support_near iken DİP/TEPE BÖLGESİ verilir)
    pub q_require_mtf_for_dip_zone: bool,
    /// RSI dip bölgesi eşiği: long için RSI < bu değer ise rsi_zone_ok (varsayılan 35)
    pub q_rsi_oversold: f64,
    /// RSI tepe bölgesi eşiği: short için RSI > bu değer ise rsi_zone_ok (varsayılan 65)
    pub q_rsi_overbought: f64,

    // Q-Skor ağırlıkları (w1..w5, toplam 1): trend, structure, time, rr, momentum
    pub q_weight_trend: f64,
    pub q_weight_structure: f64,
    pub q_weight_time: f64,
    pub q_weight_rr: f64,
    pub q_weight_momentum: f64,
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

            // Elliott Wave görselleştirme
            elliott_fibo_gap_bars: 2,
            elliott_fibo_length_bars: 12,
            elliott_min_rr: 1.5,

            // Q-ANALİZ varsayılanları
            q_score_threshold: 70.0,
            q_elite_threshold: 85.0,
            q_min_rr: 1.5,
            q_radar_phase_min: 0.1,
            q_radar_phase_max: 0.3,
            q_entry_phase_min: 0.2,
            q_entry_phase_max: 0.6,
            q_late_phase: 0.7,
            q_protect_min_r: 1.5,
            q_protect_lock_r: 0.5,

            q_entry_atr_alpha: 0.2,
            q_entry_atr_beta: 0.8,
            q_sl_atr_gamma: 1.5,

            q_tp_structure_ext: 1.618,
            q_tp_max_r: 5.0,

            q_require_mtf_for_dip_zone: false,
            q_rsi_oversold: 35.0,
            q_rsi_overbought: 65.0,

            q_weight_trend: 0.35,
            q_weight_structure: 0.20,
            q_weight_time: 0.25,
            q_weight_rr: 0.10,
            q_weight_momentum: 0.10,
        }
    }
}

impl Config {
    /// Build configuration from optional SmartMoneyConfig (typically loaded from config.json).
    /// Missing fields fall back to the hard-coded defaults above, so existing behaviour is preserved
    /// when no external config is provided.
    pub fn from_smart_money(cfg: Option<&SmartMoneyConfig>) -> Self {
        let mut base = Config::default();
        if let Some(c) = cfg {
            if let Some(v) = c.pivot_length { base.pivot_length = v; }
            if let Some(v) = c.momentum_threshold_base { base.momentum_threshold_base = v; }
            if let Some(v) = c.tp_points { base.tp_points = v; }
            if let Some(v) = c.sl_points { base.sl_points = v; }
            if let Some(v) = c.min_signal_distance { base.min_signal_distance = v; }
            if let Some(v) = c.tp_box_height_pct { base.tp_box_height_pct = v; }
            if let Some(v) = c.pre_momentum_factor_base { base.pre_momentum_factor_base = v; }
            if let Some(v) = c.short_trend_period { base.short_trend_period = v; }
            if let Some(v) = c.long_trend_period { base.long_trend_period = v; }

            if let Some(v) = c.use_momentum_filter { base.use_momentum_filter = v; }
            if let Some(v) = c.use_trend_filter { base.use_trend_filter = v; }
            if let Some(v) = c.higher_tf { base.higher_tf = v; }
            if let Some(v) = c.use_lower_tf_filter { base.use_lower_tf_filter = v; }
            if let Some(v) = c.lower_tf { base.lower_tf = v; }
            if let Some(v) = c.use_volume_filter { base.use_volume_filter = v; }
            if let Some(v) = c.use_breakout_filter { base.use_breakout_filter = v; }
            if let Some(v) = c.show_get_ready { base.show_get_ready = v; }
            if let Some(v) = c.restrict_repeated_signals { base.restrict_repeated_signals = v; }
            if let Some(v) = c.restrict_trend_tf { base.restrict_trend_tf = v; }

            if let Some(v) = c.enable_liquidity_zones { base.enable_liquidity_zones = v; }
            if let Some(v) = c.enable_market_profile { base.enable_market_profile = v; }
            if let Some(v) = c.enable_divergence_scanner { base.enable_divergence_scanner = v; }
            if let Some(v) = c.enable_trend_analysis { base.enable_trend_analysis = v; }

            if let Some(v) = c.volume_long_period { base.volume_long_period = v; }
            if let Some(v) = c.volume_short_period { base.volume_short_period = v; }
            if let Some(v) = c.breakout_period { base.breakout_period = v; }
        }
        base
    }
}
