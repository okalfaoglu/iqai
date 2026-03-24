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

    // Elliott projeksiyon (Pine-tarzı ayarlanabilir hedefler; kuralları değiştirmez)
    /// Impulse (1-2) aşamasında W3 ana uzatma çarpanı (örn. 1.618)
    pub elliott_wave3_extension: f64,
    /// W5 = W1 hedefinde W1 uzunluğu çarpanı (varsayılan 1.0 = klasik eşitlik)
    pub elliott_wave5_w1_multiple: f64,
    /// W2/W1 oranı “yönerge” yakınlık toleransı % (0.382/0.5/0.618/0.764 bantları)
    pub elliott_fib_tolerance_pct: f64,
    /// Potansiyel W4 fiyatı: W3 hareketinin geri çekilme oranı (Pine `wave4_retrace` benzeri)
    pub elliott_wave4_retrace_path: f64,
    /// Son mumdan itibaren W3 uç noktası için ileri bar sayısı (çapraz projeksiyon uzunluğu)
    pub elliott_projection_horizon_bars: u32,
    /// W3→W4 ve W4→W5 çapraz segmentleri arası bar (Pine `futBar+10` benzeri)
    pub elliott_projection_segment_gap_bars: u32,

    // Elliott Wave Oscillator (EWO) + fusion confluence (`elliott_fusion.rs`)
    pub elliott_ewo_fast: u32,
    pub elliott_ewo_slow: u32,
    pub elliott_ewo_signal: u32,
    pub elliott_ewo_strong_threshold: f64,
    /// true ise geçerli impulse’ta EWO yönü zıt ise uyarı mesajı eklenir (kuralları iptal etmez)
    pub elliott_require_ewo_alignment: bool,

    /// Dalga pivotları arası minimum bar (stabilite ölçümü)
    pub elliott_stability_min_wave_bars: u32,
    /// W2 pivotundan sonra onay için minimum bar
    pub elliott_stability_confirm_bars: u32,
    /// Bu kadar bar sonra “timeout” uyarısı (stateless; Pine auto-invalidate ilhamı)
    pub elliott_stability_auto_invalidate_bars: u32,
    /// İç dalga (nested) tespiti için ikinci ölçek pivot uzunluğu.
    /// `pivot_length`'ten küçük önerilir; eşitse tek ölçek davranışı.
    pub elliott_inner_pivot_length: u32,
    /// true: itki W1–W5 ve düzeltme A–B–C **iç dalga** sayımlarında 1:1 tez uyumu (5/5 veya 3/3 bacak).
    /// false (varsayılan): en az 3/5 itki veya 2/3 düzeltme — gürültülü grafikler için toleranslı.
    pub elliott_subwave_strict: bool,
    /// `content.txt` §2.5.3: W3 bitiş > W1 bitiş, |W4|≤|W3|; §2.5.4.2 zigzag: B≤%61.8, C≥B — `formation_valid` / zigzag validasyonuna dahil.
    pub elliott_thesis_te_y_rules: bool,

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

    /// Candlestick pattern gürültü filtresi: ATR periyodu (varsayılan 14).
    pub candlestick_noise_atr_period: u32,
    /// Son mum range / ATR minimum oranı (varsayılan 0.15).
    pub candlestick_noise_min_range_atr_ratio: f64,
    /// Q-RADAR: confluence katmanı başına skor artışı (varsayılan 0.6).
    pub q_confluence_boost_per_layer: f64,
    /// Q-RADAR: maksimum confluence boost (varsayılan 2.5).
    pub q_confluence_boost_cap: f64,

    /// Q-RADAR fırsat analizine Q-Setup + Elliott özetini ekle; RADAR↔Setup hizalama ve ikinci hedef (G05).
    /// `false` ile eski davranışa yakın (yalnız çekirdek alanlar).
    pub q_enrich_opportunity_with_setup_elliott: bool,

    // Dip/tepe confluence (`dip_confluence.rs`)
    pub dip_confluence_mtf_atr_band: f64,
    pub dip_confluence_fib_price_band_pct: f64,
    pub dip_confluence_structure_score_min: f64,
    pub dip_confluence_absorption_atr_margin: f64,
    pub dip_confluence_absorption_bars: u32,
    pub dip_confluence_absorption_volume_ratio: f64,
    pub dip_confluence_absorption_vol_avg_bars: u32,
    pub dip_confluence_atr_period: u32,

    // Dip/tepe discrete skor (`dip_tepe_scoring.rs`)
    pub dip_tepe_pts_rsi: u8,
    pub dip_tepe_pts_rsi_divergence: u8,
    pub dip_tepe_pts_macd_div: u8,
    pub dip_tepe_pts_support_zone: u8,
    pub dip_tepe_pts_volume_spike: u8,
    pub dip_tepe_pts_liquidity_sweep: u8,
    pub dip_tepe_pts_atr_filter: u8,
    pub dip_tepe_pts_vwap_mean_reversion: u8,
    pub dip_tepe_pts_bullish_candle: u8,
    pub dip_tepe_pts_fib_level: u8,
    pub dip_tepe_pts_ema200_near: u8,
    pub dip_tepe_pts_market_structure: u8,
    pub dip_tepe_pts_bollinger: u8,
    pub dip_tepe_pts_mean_reversion: u8,
    pub dip_tepe_score_cap: u8,

    pub dip_tepe_ma_period: u32,
    pub dip_tepe_vol_spike_mult: f64,
    pub dip_tepe_liquidity_lookback: u32,
    pub dip_tepe_swing_lookback: u32,
    pub dip_tepe_fib_band_pct: f64,
    pub dip_tepe_ema_near_dist_pct: f64,
    pub dip_tepe_structure_score_min: f64,
    pub dip_tepe_bollinger_period: u32,
    pub dip_tepe_bollinger_std: f64,
    pub dip_tepe_mean_rev_dist: f64,
    pub dip_tepe_vwap_mean_rev_dist: f64,
    pub dip_tepe_atr_vol_norm_min: f64,
    pub dip_tepe_atr_vol_norm_max: f64,
    pub dip_tepe_rsi_div_min_bars: u32,
    pub dip_tepe_macd_fast: u32,
    pub dip_tepe_macd_slow: u32,
    pub dip_tepe_macd_signal: u32,
    pub dip_tepe_rsi_period: u32,
    pub dip_tepe_ema_period: u32,
    pub dip_tepe_rec_strong_min: u8,
    pub dip_tepe_rec_buy_zone_min: u8,
    pub dip_tepe_rec_watch_min: u8,

    // Dip/tepe reversal (`reversal.rs`)
    pub reversal_atr_period: u32,
    pub reversal_margin_atr_up: f64,
    pub reversal_margin_atr_down: f64,
    pub reversal_strength_atr_full: f64,
    pub reversal_spring_recovery_bars: u32,
    pub reversal_weight_strength_atr: f64,
    pub reversal_weight_vol_ratio: f64,
    pub reversal_weight_body_ratio: f64,
    pub reversal_volume_ma_period: u32,

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

            elliott_wave3_extension: 1.618,
            elliott_wave5_w1_multiple: 1.0,
            elliott_fib_tolerance_pct: 35.0,
            elliott_wave4_retrace_path: 0.382,
            elliott_projection_horizon_bars: 30,
            elliott_projection_segment_gap_bars: 10,

            elliott_ewo_fast: 5,
            elliott_ewo_slow: 34,
            elliott_ewo_signal: 5,
            elliott_ewo_strong_threshold: 13.0,
            elliott_require_ewo_alignment: false,

            elliott_stability_min_wave_bars: 5,
            elliott_stability_confirm_bars: 3,
            elliott_stability_auto_invalidate_bars: 100,
            elliott_inner_pivot_length: 3,
            elliott_subwave_strict: false,
            elliott_thesis_te_y_rules: false,

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

            candlestick_noise_atr_period: 14,
            candlestick_noise_min_range_atr_ratio: 0.15,
            q_confluence_boost_per_layer: 0.6,
            q_confluence_boost_cap: 2.5,
            q_enrich_opportunity_with_setup_elliott: true,

            dip_confluence_mtf_atr_band: 0.5,
            dip_confluence_fib_price_band_pct: 0.003,
            dip_confluence_structure_score_min: 0.55,
            dip_confluence_absorption_atr_margin: 0.3,
            dip_confluence_absorption_bars: 5,
            dip_confluence_absorption_volume_ratio: 1.5,
            dip_confluence_absorption_vol_avg_bars: 20,
            dip_confluence_atr_period: 14,

            dip_tepe_pts_rsi: 1,
            dip_tepe_pts_rsi_divergence: 2,
            dip_tepe_pts_macd_div: 2,
            dip_tepe_pts_support_zone: 2,
            dip_tepe_pts_volume_spike: 1,
            dip_tepe_pts_liquidity_sweep: 1,
            dip_tepe_pts_atr_filter: 1,
            dip_tepe_pts_vwap_mean_reversion: 1,
            dip_tepe_pts_bullish_candle: 1,
            dip_tepe_pts_fib_level: 1,
            dip_tepe_pts_ema200_near: 1,
            dip_tepe_pts_market_structure: 1,
            dip_tepe_pts_bollinger: 1,
            dip_tepe_pts_mean_reversion: 1,
            dip_tepe_score_cap: 10,

            dip_tepe_ma_period: 20,
            dip_tepe_vol_spike_mult: 1.5,
            dip_tepe_liquidity_lookback: 20,
            dip_tepe_swing_lookback: 50,
            dip_tepe_fib_band_pct: 0.005,
            dip_tepe_ema_near_dist_pct: 0.01,
            dip_tepe_structure_score_min: 0.55,
            dip_tepe_bollinger_period: 20,
            dip_tepe_bollinger_std: 2.0,
            dip_tepe_mean_rev_dist: 0.1,
            dip_tepe_vwap_mean_rev_dist: 0.03,
            dip_tepe_atr_vol_norm_min: 0.003,
            dip_tepe_atr_vol_norm_max: 0.08,
            dip_tepe_rsi_div_min_bars: 30,
            dip_tepe_macd_fast: 12,
            dip_tepe_macd_slow: 26,
            dip_tepe_macd_signal: 9,
            dip_tepe_rsi_period: 14,
            dip_tepe_ema_period: 200,
            dip_tepe_rec_strong_min: 8,
            dip_tepe_rec_buy_zone_min: 6,
            dip_tepe_rec_watch_min: 4,

            reversal_atr_period: 14,
            reversal_margin_atr_up: 0.2,
            reversal_margin_atr_down: 0.2,
            reversal_strength_atr_full: 2.0,
            reversal_spring_recovery_bars: 4,
            reversal_weight_strength_atr: 0.5,
            reversal_weight_vol_ratio: 0.3,
            reversal_weight_body_ratio: 0.2,
            reversal_volume_ma_period: 20,

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
            if let Some(v) = c.q_rsi_oversold { base.q_rsi_oversold = v; }
            if let Some(v) = c.q_rsi_overbought { base.q_rsi_overbought = v; }
            if let Some(v) = c.candlestick_noise_atr_period { base.candlestick_noise_atr_period = v.max(1); }
            if let Some(v) = c.candlestick_noise_min_range_atr_ratio {
                base.candlestick_noise_min_range_atr_ratio = v;
            }
            if let Some(v) = c.q_confluence_boost_per_layer {
                base.q_confluence_boost_per_layer = v;
            }
            if let Some(v) = c.q_confluence_boost_cap {
                base.q_confluence_boost_cap = v;
            }
            if let Some(v) = c.q_enrich_opportunity_with_setup_elliott {
                base.q_enrich_opportunity_with_setup_elliott = v;
            }

            if let Some(v) = c.dip_confluence_mtf_atr_band {
                base.dip_confluence_mtf_atr_band = v;
            }
            if let Some(v) = c.dip_confluence_fib_price_band_pct {
                base.dip_confluence_fib_price_band_pct = v;
            }
            if let Some(v) = c.dip_confluence_structure_score_min {
                base.dip_confluence_structure_score_min = v;
            }
            if let Some(v) = c.dip_confluence_absorption_atr_margin {
                base.dip_confluence_absorption_atr_margin = v;
            }
            if let Some(v) = c.dip_confluence_absorption_bars {
                base.dip_confluence_absorption_bars = v.max(1);
            }
            if let Some(v) = c.dip_confluence_absorption_volume_ratio {
                base.dip_confluence_absorption_volume_ratio = v;
            }
            if let Some(v) = c.dip_confluence_absorption_vol_avg_bars {
                base.dip_confluence_absorption_vol_avg_bars = v.max(1);
            }
            if let Some(v) = c.dip_confluence_atr_period {
                base.dip_confluence_atr_period = v.max(1);
            }

            if let Some(v) = c.dip_tepe_pts_rsi { base.dip_tepe_pts_rsi = v; }
            if let Some(v) = c.dip_tepe_pts_rsi_divergence { base.dip_tepe_pts_rsi_divergence = v; }
            if let Some(v) = c.dip_tepe_pts_macd_div { base.dip_tepe_pts_macd_div = v; }
            if let Some(v) = c.dip_tepe_pts_support_zone { base.dip_tepe_pts_support_zone = v; }
            if let Some(v) = c.dip_tepe_pts_volume_spike { base.dip_tepe_pts_volume_spike = v; }
            if let Some(v) = c.dip_tepe_pts_liquidity_sweep { base.dip_tepe_pts_liquidity_sweep = v; }
            if let Some(v) = c.dip_tepe_pts_atr_filter { base.dip_tepe_pts_atr_filter = v; }
            if let Some(v) = c.dip_tepe_pts_vwap_mean_reversion { base.dip_tepe_pts_vwap_mean_reversion = v; }
            if let Some(v) = c.dip_tepe_pts_bullish_candle { base.dip_tepe_pts_bullish_candle = v; }
            if let Some(v) = c.dip_tepe_pts_fib_level { base.dip_tepe_pts_fib_level = v; }
            if let Some(v) = c.dip_tepe_pts_ema200_near { base.dip_tepe_pts_ema200_near = v; }
            if let Some(v) = c.dip_tepe_pts_market_structure { base.dip_tepe_pts_market_structure = v; }
            if let Some(v) = c.dip_tepe_pts_bollinger { base.dip_tepe_pts_bollinger = v; }
            if let Some(v) = c.dip_tepe_pts_mean_reversion { base.dip_tepe_pts_mean_reversion = v; }
            if let Some(v) = c.dip_tepe_score_cap { base.dip_tepe_score_cap = v.max(1); }

            if let Some(v) = c.dip_tepe_ma_period { base.dip_tepe_ma_period = v.max(1); }
            if let Some(v) = c.dip_tepe_vol_spike_mult { base.dip_tepe_vol_spike_mult = v; }
            if let Some(v) = c.dip_tepe_liquidity_lookback { base.dip_tepe_liquidity_lookback = v.max(2); }
            if let Some(v) = c.dip_tepe_swing_lookback { base.dip_tepe_swing_lookback = v.max(1); }
            if let Some(v) = c.dip_tepe_fib_band_pct { base.dip_tepe_fib_band_pct = v; }
            if let Some(v) = c.dip_tepe_ema_near_dist_pct { base.dip_tepe_ema_near_dist_pct = v; }
            if let Some(v) = c.dip_tepe_structure_score_min { base.dip_tepe_structure_score_min = v; }
            if let Some(v) = c.dip_tepe_bollinger_period { base.dip_tepe_bollinger_period = v.max(1); }
            if let Some(v) = c.dip_tepe_bollinger_std { base.dip_tepe_bollinger_std = v; }
            if let Some(v) = c.dip_tepe_mean_rev_dist { base.dip_tepe_mean_rev_dist = v; }
            if let Some(v) = c.dip_tepe_vwap_mean_rev_dist { base.dip_tepe_vwap_mean_rev_dist = v; }
            if let Some(v) = c.dip_tepe_atr_vol_norm_min { base.dip_tepe_atr_vol_norm_min = v; }
            if let Some(v) = c.dip_tepe_atr_vol_norm_max { base.dip_tepe_atr_vol_norm_max = v; }
            if let Some(v) = c.dip_tepe_rsi_div_min_bars { base.dip_tepe_rsi_div_min_bars = v.max(2); }
            if let Some(v) = c.dip_tepe_macd_fast { base.dip_tepe_macd_fast = v.max(1); }
            if let Some(v) = c.dip_tepe_macd_slow { base.dip_tepe_macd_slow = v.max(1); }
            if let Some(v) = c.dip_tepe_macd_signal { base.dip_tepe_macd_signal = v.max(1); }
            if let Some(v) = c.dip_tepe_rsi_period { base.dip_tepe_rsi_period = v.max(1); }
            if let Some(v) = c.dip_tepe_ema_period { base.dip_tepe_ema_period = v.max(1); }
            if let Some(v) = c.dip_tepe_rec_strong_min { base.dip_tepe_rec_strong_min = v; }
            if let Some(v) = c.dip_tepe_rec_buy_zone_min { base.dip_tepe_rec_buy_zone_min = v; }
            if let Some(v) = c.dip_tepe_rec_watch_min { base.dip_tepe_rec_watch_min = v; }

            if let Some(v) = c.reversal_atr_period { base.reversal_atr_period = v.max(1); }
            if let Some(v) = c.reversal_margin_atr_up { base.reversal_margin_atr_up = v; }
            if let Some(v) = c.reversal_margin_atr_down { base.reversal_margin_atr_down = v; }
            if let Some(v) = c.reversal_strength_atr_full { base.reversal_strength_atr_full = v.max(1e-9); }
            if let Some(v) = c.reversal_spring_recovery_bars { base.reversal_spring_recovery_bars = v.max(1); }
            if let Some(v) = c.reversal_weight_strength_atr { base.reversal_weight_strength_atr = v; }
            if let Some(v) = c.reversal_weight_vol_ratio { base.reversal_weight_vol_ratio = v; }
            if let Some(v) = c.reversal_weight_body_ratio { base.reversal_weight_body_ratio = v; }
            if let Some(v) = c.reversal_volume_ma_period { base.reversal_volume_ma_period = v.max(1); }

            if let Some(v) = c.elliott_fibo_gap_bars { base.elliott_fibo_gap_bars = v.max(1); }
            if let Some(v) = c.elliott_fibo_length_bars { base.elliott_fibo_length_bars = v.max(1); }
            if let Some(v) = c.elliott_min_rr { base.elliott_min_rr = v; }
            if let Some(v) = c.elliott_wave3_extension { base.elliott_wave3_extension = v.clamp(1.0, 4.0); }
            if let Some(v) = c.elliott_wave5_w1_multiple { base.elliott_wave5_w1_multiple = v.clamp(0.618, 2.618); }
            if let Some(v) = c.elliott_fib_tolerance_pct { base.elliott_fib_tolerance_pct = v.clamp(5.0, 50.0); }
            if let Some(v) = c.elliott_wave4_retrace_path { base.elliott_wave4_retrace_path = v.clamp(0.09, 0.95); }
            if let Some(v) = c.elliott_projection_horizon_bars { base.elliott_projection_horizon_bars = v.max(5); }
            if let Some(v) = c.elliott_projection_segment_gap_bars { base.elliott_projection_segment_gap_bars = v.max(1); }
            if let Some(v) = c.elliott_ewo_fast { base.elliott_ewo_fast = v.max(2); }
            if let Some(v) = c.elliott_ewo_slow { base.elliott_ewo_slow = v.max(3); }
            if let Some(v) = c.elliott_ewo_signal { base.elliott_ewo_signal = v.max(2); }
            if let Some(v) = c.elliott_ewo_strong_threshold { base.elliott_ewo_strong_threshold = v; }
            if let Some(v) = c.elliott_require_ewo_alignment { base.elliott_require_ewo_alignment = v; }
            if let Some(v) = c.elliott_stability_min_wave_bars { base.elliott_stability_min_wave_bars = v.max(1); }
            if let Some(v) = c.elliott_stability_confirm_bars { base.elliott_stability_confirm_bars = v.max(1); }
            if let Some(v) = c.elliott_stability_auto_invalidate_bars { base.elliott_stability_auto_invalidate_bars = v.max(20); }
            if let Some(v) = c.elliott_inner_pivot_length { base.elliott_inner_pivot_length = v.max(2); }
            if let Some(v) = c.elliott_subwave_strict { base.elliott_subwave_strict = v; }
            if let Some(v) = c.elliott_thesis_te_y_rules {
                base.elliott_thesis_te_y_rules = v;
            }
        }
        base
    }
}
