//! Sembol × timeframe başına tek satır analiz snapshot'ı.
//! Q-Analiz daemon her turda bu yapıyı doldurup DB'ye upsert eder.

use serde::Serialize;

use crate::config::Config;
use crate::indicators::{atr, bollinger, ema, macd, rsi, vwap};
use crate::q_radar_analysis::QRadarOpportunityAnalysis;
use crate::strategy::{build_scenarios_for_series, StrategyDirection, StrategyRole};
use crate::types::{Candle, Timeframe};
use crate::elliott_detector::compute_elliott;
use crate::smart_money::{build_smart_money_context_for_series, Po3Phase};

/// Tek (symbol, timeframe) için analiz snapshot'ı – DB'ye yazılacak alanlar.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AnalysisSnapshot {
    pub symbol: String,
    pub timeframe: Timeframe,

    // Q-Analiz özet
    pub detection: String,
    pub direction: String,
    pub recommendation: String,
    pub confidence_score: f64,
    pub early_warning_score: f64,
    pub reference_price: f64,
    pub confirmation_layers: Option<String>,
    pub discrete_score: Option<f64>,
    pub sm_score: Option<f64>,
    pub confluence_layers: Option<u8>,

    // Q-RADAR detay
    pub radar_confidence: Option<f64>,
    pub radar_window_min: Option<u32>,
    pub radar_window_max: Option<u32>,
    pub radar_suggested_sl: Option<f64>,

    // Dip analizi
    pub dip_price: Option<f64>,
    pub dip_time: Option<i64>,
    pub bars_since_dip: Option<u32>,
    pub reversal_detected: Option<bool>,
    pub reversal_strength: Option<f64>,
    pub bounce_from_dip: Option<f64>,
    pub bounce_r: Option<f64>,
    pub spring_detected: Option<bool>,

    // Tepe analizi
    pub peak_price: Option<f64>,
    pub peak_time: Option<i64>,
    pub bars_since_peak: Option<u32>,
    pub peak_reversal_detected: Option<bool>,
    pub decline_strength: Option<f64>,
    pub decline_from_peak: Option<f64>,
    pub decline_r: Option<f64>,
    pub upthrust_detected: Option<bool>,

    // Confluence bayrakları (0/1)
    pub mtf_support_near: Option<bool>,
    pub ltf_structure_ok: Option<bool>,
    pub fib_elliott_zone: Option<bool>,
    pub divergence_ok: Option<bool>,
    pub confluence_spring_ok: Option<bool>,
    pub rsi_zone_ok: Option<bool>,
    pub bos_ok: Option<bool>,
    pub absorption_ok: Option<bool>,

    // Osilatör / indikatör (son bar)
    pub rsi_14: Option<f64>,
    pub atr_14: Option<f64>,
    pub macd_line: Option<f64>,
    pub macd_signal: Option<f64>,
    pub macd_hist: Option<f64>,
    pub bb_lower: Option<f64>,
    pub bb_middle: Option<f64>,
    pub bb_upper: Option<f64>,
    pub ema_20: Option<f64>,
    pub ema_50: Option<f64>,
    pub ema_200: Option<f64>,
    pub vwap_val: Option<f64>,

    // Elliott özet
    pub elliott_formation: Option<String>,
    pub elliott_type: Option<String>,
    pub elliott_in_progress: Option<bool>,
    pub elliott_validation_ok: Option<bool>,
    pub elliott_w5_t1: Option<f64>,
    pub elliott_w5_t2: Option<f64>,
    pub elliott_w5_t3: Option<f64>,

    // Strateji (en iyi senaryo)
    pub classic_pattern: Option<String>,
    pub scenario_role: Option<String>,
    pub scenario_direction: Option<String>,
    pub scenario_entry: Option<f64>,
    pub scenario_stop: Option<f64>,
    pub scenario_tp1: Option<f64>,
    pub scenario_tp2: Option<f64>,
    pub scenario_tp3: Option<f64>,
    pub scenario_qscore: Option<f64>,
    pub scenario_has_radar: Option<bool>,

    // Smart Money özet
    pub po3_phase: Option<String>,

    // Pozisyon metrikleri (PositionMetrics'ten gelen özet)
    pub position_state: Option<String>,
    pub market_mode: Option<String>,
    pub local_trend: Option<i32>,
    pub global_trend: Option<i32>,
    pub volatility_pct: Option<f64>,
    pub momentum_short: Option<f64>,
    pub momentum_long: Option<f64>,
    pub rr: Option<f64>,
    pub tmr_trend_points: Option<i32>,
    pub tmr_momentum_points: Option<i32>,
    pub tmr_rr_points: Option<i32>,
    pub tmr_strength_points: Option<i32>,
    pub trend_exhaustion: Option<bool>,
    pub structure_shift: Option<bool>,
    pub position_side: Option<String>,

    /// Sinyal detayları, hedefler listesi vb. (JSON)
    pub extra_json: Option<String>,
}

/// `opp` ve aynı TF'deki mumlardan snapshot oluşturur. Elliott, senaryolar ve indikatörler burada hesaplanır.
pub fn build_analysis_snapshot(
    opp: &QRadarOpportunityAnalysis,
    candles: &[Candle],
    config: &Config,
) -> AnalysisSnapshot {
    let mut s = AnalysisSnapshot {
        symbol: opp.symbol.clone(),
        timeframe: opp.timeframe,
        detection: opp.detection.clone(),
        direction: opp.direction.clone(),
        recommendation: opp.recommendation.clone(),
        confidence_score: opp.confidence_score,
        early_warning_score: opp.early_warning_score,
        reference_price: opp.reference_price,
        confirmation_layers: opp.confirmation_layers.clone(),
        discrete_score: opp.discrete_score.as_ref().map(|d| d.total as f64),
        sm_score: opp.smart_money_score.as_ref().map(|sm| sm.total as f64),
        confluence_layers: opp.confluence.as_ref().map(|c| c.layers_passed),
        ..Default::default()
    };

    if let Some(ref r) = opp.radar {
        s.radar_confidence = Some(r.confidence);
        s.radar_window_min = Some(r.expected_window_bars.0);
        s.radar_window_max = Some(r.expected_window_bars.1);
        s.radar_suggested_sl = r.suggested_sl;
    }

    if let Some(ref d) = opp.dip {
        s.dip_price = Some(d.dip_price);
        s.dip_time = Some(d.dip_time);
        s.bars_since_dip = Some(d.bars_since_dip as u32);
        s.reversal_detected = Some(d.reversal_detected);
        s.reversal_strength = Some(d.reversal_strength);
        s.bounce_from_dip = Some(d.bounce_from_dip);
        s.bounce_r = Some(d.bounce_r);
        s.spring_detected = Some(d.spring_detected);
    }

    if let Some(ref p) = opp.peak {
        s.peak_price = Some(p.peak_price);
        s.peak_time = Some(p.peak_time);
        s.bars_since_peak = Some(p.bars_since_peak as u32);
        s.peak_reversal_detected = Some(p.reversal_detected);
        s.decline_strength = Some(p.decline_strength);
        s.decline_from_peak = Some(p.decline_from_peak);
        s.decline_r = Some(p.decline_r);
        s.upthrust_detected = Some(p.upthrust_detected);
    }

    if let Some(ref c) = opp.confluence {
        s.mtf_support_near = Some(c.mtf_support_near);
        s.ltf_structure_ok = Some(c.ltf_structure_ok);
        s.fib_elliott_zone = Some(c.fib_elliott_zone);
        s.divergence_ok = Some(c.divergence_ok);
        s.confluence_spring_ok = Some(c.spring_ok);
        s.rsi_zone_ok = Some(c.rsi_zone_ok);
        s.bos_ok = Some(c.bos_ok);
        s.absorption_ok = Some(c.absorption_ok);
    }

    // İndikatörler (son bar)
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    if !closes.is_empty() {
        s.rsi_14 = rsi(&closes, 14);
        s.atr_14 = atr(candles, 14);
        if let Some(m) = macd(&closes, 12, 26, 9) {
            s.macd_line = Some(m.line);
            s.macd_signal = Some(m.signal);
            s.macd_hist = Some(m.histogram);
        }
        if let Some((lo, mid, hi)) = bollinger(&closes, 20, 2.0) {
            s.bb_lower = Some(lo);
            s.bb_middle = Some(mid);
            s.bb_upper = Some(hi);
        }
        s.ema_20 = ema(&closes, 20);
        s.ema_50 = ema(&closes, 50);
        s.ema_200 = if closes.len() >= 200 { ema(&closes, 200) } else { None };
        s.vwap_val = vwap(candles);
    }

    // Elliott
    if candles.len() >= (config.pivot_length as usize) * 4 + 20 {
        let elliott = compute_elliott(candles, config, false);
        if !elliott.formation.is_empty() && elliott.formation != "—" {
            s.elliott_formation = Some(elliott.formation.clone());
            s.elliott_type = Some(elliott.formation_type.clone());
            s.elliott_in_progress = elliott.in_progress;
            s.elliott_validation_ok = elliott.validation_ok;
            if let Some((t1, t2, t3)) = elliott.w5_targets {
                s.elliott_w5_t1 = Some(t1);
                s.elliott_w5_t2 = Some(t2);
                s.elliott_w5_t3 = Some(t3);
            }
        }
    }

    // En iyi senaryo (Primary veya ilk plan)
    if candles.len() >= 200 {
        let scenarios = build_scenarios_for_series(&opp.symbol, opp.timeframe, candles, config);
        let best_sc = scenarios
            .iter()
            .find(|sc| sc.role == StrategyRole::Primary)
            .or_else(|| scenarios.first());
        if let Some(sc) = best_sc {
            if let Some(plan) = sc.plans.first() {
                s.classic_pattern = plan.classic_pattern_label.clone();
                s.scenario_role = Some(format!("{:?}", sc.role));
                s.scenario_direction = Some(match plan.direction {
                    StrategyDirection::Long => "LONG".to_string(),
                    StrategyDirection::Short => "SHORT".to_string(),
                });
                s.scenario_entry = Some(plan.entry);
                s.scenario_stop = Some(plan.stop_loss);
                s.scenario_tp1 = plan.targets.get(0).map(|t| t.price);
                s.scenario_tp2 = plan.targets.get(1).map(|t| t.price);
                s.scenario_tp3 = plan.targets.get(2).map(|t| t.price);
                s.scenario_qscore = Some(plan.q_score);
                s.scenario_has_radar = Some(plan.has_radar_context);
            }
        }
    }

    // Smart Money PO3
    if let Some(ctx) = build_smart_money_context_for_series(&opp.symbol, opp.timeframe, candles, config) {
        s.po3_phase = Some(match ctx.po3_phase {
            Po3Phase::Accumulation => "Accumulation".to_string(),
            Po3Phase::Manipulation => "Manipulation".to_string(),
            Po3Phase::Expansion => "Expansion".to_string(),
        });
    }

    // extra_json: discrete + SM sinyal isimleri
    let mut extra = serde_json::Map::new();
    if let Some(ref d) = opp.discrete_score {
        let signals: Vec<(&str, u8)> = d.signals.iter().filter(|x| x.active).map(|s| (s.name.as_str(), s.points)).collect();
        if !signals.is_empty() {
            let arr: Vec<serde_json::Value> = signals.iter().map(|(n, p)| serde_json::json!({"name": n, "points": p})).collect();
            extra.insert("discrete_signals".to_string(), serde_json::Value::Array(arr));
        }
        extra.insert("discrete_recommendation".to_string(), serde_json::Value::String(d.recommendation.clone()));
    }
    if let Some(ref sm) = opp.smart_money_score {
        let signals: Vec<(&str, u8)> = sm.signals.iter().filter(|x| x.active).map(|s| (s.name.as_str(), s.points)).collect();
        if !signals.is_empty() {
            let arr: Vec<serde_json::Value> = signals.iter().map(|(n, p)| serde_json::json!({"name": n, "points": p})).collect();
            extra.insert("sm_signals".to_string(), serde_json::Value::Array(arr));
        }
    }
    if let Some(a) = opp.radar_setup_alignment {
        extra.insert("radar_setup_alignment".to_string(), serde_json::json!(a));
    }
    if let Some(ref q) = opp.q_setup {
        extra.insert(
            "q_setup".to_string(),
            serde_json::json!({
                "entry": q.entry,
                "stop_loss": q.stop_loss,
                "take_profit": q.take_profit,
                "q_score": q.q_score,
            }),
        );
    }
    if let Some(tp) = opp.elliott_secondary_tp {
        extra.insert("elliott_secondary_tp".to_string(), serde_json::json!(tp));
    }
    if let Some(ref s) = opp.elliott_summary {
        extra.insert("elliott_summary".to_string(), serde_json::Value::String(s.clone()));
    }
    if let Some(ref h) = opp.abc_correction_hint {
        extra.insert("abc_correction_hint".to_string(), serde_json::Value::String(h.clone()));
    }
    if !extra.is_empty() {
        s.extra_json = Some(serde_json::to_string(&extra).unwrap_or_default());
    }

    s
}
