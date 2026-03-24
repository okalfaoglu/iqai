//! Pine “EW + SMC Fusion” tarzı: EWO, confluence/not, stabilite ipuçları, SMC–W2 çakışması.
//! `elliott_detector` ile döngü yok — dalga noktaları düz struct olarak geçirilir.

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::indicators::ema;
use crate::smart_money::build_smart_money_context_for_series;
use crate::types::{Candle, Timeframe};

/// API / hesaplama için minimal dalga noktası
#[derive(Debug, Clone)]
pub struct FusionWavePoint {
    pub time: i64,
    pub price: f64,
    pub label: String,
}

/// EWO + confluence + stabilite + SMC özeti (`ElliottDetectorResult` alanlarına map edilir)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ElliottFusionExtras {
    pub ewo_value: Option<f64>,
    pub ewo_signal: Option<f64>,
    pub ewo_bull: Option<bool>,
    pub ewo_strong_long: Option<bool>,
    pub ewo_strong_short: Option<bool>,
    pub ewo_aligned_with_impulse: Option<bool>,
    /// 0–100 Pine-tarzı confluence
    pub confluence_score: Option<f64>,
    pub wave_grade: Option<String>,
    pub w2_w1_ratio: Option<f64>,
    pub pattern_stability: Option<ElliottPatternStability>,
    pub invalidate_hint: Option<String>,
    pub smc_w2_zone_overlap: Option<bool>,
    pub smc_w2_detail: Option<String>,
    /// `elliott_require_ewo_alignment` açıkken impulse ile çelişen EWO
    pub fusion_ewo_soft_fail: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElliottPatternStability {
    pub min_bar_distance_ok: bool,
    pub wave2_confirmation_ok: bool,
    pub bars_wave0_to_wave1: Option<u32>,
    pub bars_wave1_to_wave2: Option<u32>,
    pub pattern_age_bars: Option<u32>,
    pub auto_invalidate_bars: u32,
    pub timeout_warning: bool,
}

fn ewo_hist(closes: &[f64], fast: usize, slow: usize) -> Vec<f64> {
    let mut out = Vec::new();
    if closes.len() <= slow || fast == 0 || slow < fast {
        return out;
    }
    for end in slow..closes.len() {
        let sub = &closes[..=end];
        if let (Some(ef), Some(es)) = (ema(sub, fast), ema(sub, slow)) {
            if es.abs() > 1e-12 {
                out.push((ef / es - 1.0) * 100.0);
            }
        }
    }
    out
}

fn last_two_cross_bull_bear(hist: &[f64], sig: f64) -> (bool, bool) {
    if hist.len() < 2 {
        return (false, false);
    }
    let n = hist.len();
    let prev = hist[n - 2] - sig;
    let curr = hist[n - 1] - sig;
    let cross_up = prev <= 0.0 && curr > 0.0;
    let cross_dn = prev >= 0.0 && curr < 0.0;
    (cross_up, cross_dn)
}

fn compute_ewo_tail(
    closes: &[f64],
    fast: usize,
    slow: usize,
    signal_n: usize,
    strong_thresh: f64,
) -> (
    Option<f64>,
    Option<f64>,
    bool,
    bool,
    bool,
    bool,
    bool,
) {
    let hist = ewo_hist(closes, fast, slow);
    if hist.len() < signal_n.max(2) {
        return (None, None, false, false, false, false, false);
    }
    let sig = ema(&hist, signal_n);
    let Some(sig_v) = sig else {
        return (None, None, false, false, false, false, false);
    };
    let last = *hist.last().unwrap_or(&0.0);
    let bull = last > sig_v;
    let (cross_up, cross_dn) = last_two_cross_bull_bear(&hist, sig_v);
    let strong_l = cross_up && last < -strong_thresh;
    let strong_s = cross_dn && last > strong_thresh;
    (
        Some(last),
        Some(sig_v),
        bull,
        strong_l,
        strong_s,
        cross_up,
        cross_dn,
    )
}

/// Dalga noktası zamanı: API çoğunlukla saniye (`time/1000`); mumlar ms.
fn wave_point_time_ms(t: i64) -> i64 {
    if t > 1_000_000_000_000 {
        t
    } else {
        t.saturating_mul(1000)
    }
}

fn candle_index_at_or_before(candles: &[Candle], t_ms: i64) -> Option<usize> {
    if candles.is_empty() {
        return None;
    }
    let mut best = None;
    for (i, c) in candles.iter().enumerate() {
        if c.time <= t_ms {
            best = Some(i);
        } else {
            break;
        }
    }
    best
}

fn range_overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> bool {
    let al = a0.min(a1);
    let ah = a0.max(a1);
    let bl = b0.min(b1);
    let bh = b0.max(b1);
    al <= bh && ah >= bl
}

/// Ardışık mumlar arası ortalama süre (ms); tek mum / yetersiz veride güvenli varsayılan.
fn avg_bar_ms(candles: &[Candle]) -> i64 {
    if candles.len() < 2 {
        return 60_000;
    }
    let mut sum = 0_i64;
    let mut n = 0_i64;
    for w in candles.windows(2) {
        let d = w[1].time.saturating_sub(w[0].time);
        if d > 0 {
            sum += d;
            n += 1;
        }
    }
    if n == 0 {
        60_000
    } else {
        (sum / n).max(1)
    }
}

fn grade_from_score(s: f64) -> String {
    if s >= 85.0 {
        "A+".to_string()
    } else if s >= 75.0 {
        "A".to_string()
    } else if s >= 65.0 {
        "B+".to_string()
    } else if s >= 55.0 {
        "B".to_string()
    } else if s >= 45.0 {
        "C".to_string()
    } else {
        "D".to_string()
    }
}

/// Dalga + mum serisi + SMC ile fusion alanlarını üret.
pub fn compute_elliott_fusion_extras(
    points: &[FusionWavePoint],
    candles: &[Candle],
    config: &Config,
    timeframe: Option<Timeframe>,
    symbol: &str,
    is_bullish_impulse: bool,
    formation: &str,
    formation_type: &str,
    validation_ok: Option<bool>,
) -> ElliottFusionExtras {
    let mut ex = ElliottFusionExtras::default();
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let vols: Vec<f64> = candles.iter().map(|c| c.volume).collect();

    let fast_u32 = config.elliott_ewo_fast.max(2);
    let slow_u32 = config.elliott_ewo_slow.max(fast_u32.saturating_add(1));
    let fast = fast_u32 as usize;
    let slow = slow_u32 as usize;
    let sig_n = config.elliott_ewo_signal.max(2) as usize;
    let thresh = config.elliott_ewo_strong_threshold;

    let (ewo_v, ewo_sig, bull, sl, ss, cross_up, cross_dn) =
        compute_ewo_tail(&closes, fast, slow, sig_n, thresh);
    ex.ewo_value = ewo_v;
    ex.ewo_signal = ewo_sig;
    ex.ewo_bull = Some(bull);
    ex.ewo_strong_long = Some(sl);
    ex.ewo_strong_short = Some(ss);

    let impulse_like =
        formation.contains("Impulse") || formation_type.contains("İtki") || formation_type.contains("Motif");
    if !impulse_like {
        return ex;
    }

    let p0 = points.iter().find(|p| p.label == "0");
    let p1 = points.iter().find(|p| p.label == "1");
    let p2 = points.iter().find(|p| p.label == "2");

    let (Some(p0), Some(p1), Some(p2)) = (p0, p1, p2) else {
        // EWO hizası yine de işaretlenebilir
        if let (Some(ev), Some(sv)) = (ewo_v, ewo_sig) {
            let aligned = if is_bullish_impulse {
                ev > sv
            } else {
                ev < sv
            };
            ex.ewo_aligned_with_impulse = Some(aligned);
        }
        return ex;
    };

    let w1_len = (p1.price - p0.price).abs();
    let w2_len = (p2.price - p1.price).abs();
    let ratio = if w1_len > 1e-12 {
        w2_len / w1_len
    } else {
        0.0
    };
    ex.w2_w1_ratio = Some(ratio);

    // --- Confluence (Pine calcConfluence ilhamlı) ---
    let mut score = 0.0_f64;
    if (0.5..=0.618).contains(&ratio) {
        score += 30.0;
    } else if (0.618..=0.764).contains(&ratio) {
        score += 25.0;
    } else if (0.382..=0.5).contains(&ratio) {
        score += 20.0;
    } else {
        score += 10.0;
    }

    // Fib guideline: 0.382 / 0.5 / 0.618 / 0.764 yakınına bonus
    let tol = (config.elliott_fib_tolerance_pct / 100.0).max(0.05).min(0.5);
    for fib in [0.382_f64, 0.5, 0.618, 0.764] {
        let lo = fib * (1.0 - tol);
        let hi = fib * (1.0 + tol);
        if ratio >= lo && ratio <= hi {
            score += 8.0;
            break;
        }
    }

    let vol_avg_n = 20_usize;
    if vols.len() >= vol_avg_n {
        let avg: f64 = vols[vols.len() - vol_avg_n..].iter().sum::<f64>() / vol_avg_n as f64;
        let last_v = *vols.last().unwrap_or(&0.0);
        if avg > 1e-9 {
            if last_v > avg * 1.5 {
                score += 15.0;
            } else if last_v > avg {
                score += 10.0;
            }
        }
    }

    if let Some(e50) = ema(&closes, 50.min(closes.len().saturating_sub(1).max(1))) {
        let last_c = *closes.last().unwrap_or(&0.0);
        if is_bullish_impulse && last_c > e50 {
            score += 10.0;
        } else if !is_bullish_impulse && last_c < e50 {
            score += 10.0;
        }
    }

    if let (Some(ev), Some(sv)) = (ewo_v, ewo_sig) {
        if is_bullish_impulse {
            if ev > sv {
                score += 10.0;
            }
            if sl {
                score += 10.0;
            } else if cross_up {
                score += 5.0;
            }
        } else {
            if ev < sv {
                score += 10.0;
            }
            if ss {
                score += 10.0;
            } else if cross_dn {
                score += 5.0;
            }
        }
    }

    let tf = timeframe.unwrap_or(Timeframe::M5);
    if let Some(ctx) = build_smart_money_context_for_series(symbol, tf, candles, config) {
        let w2_lo = p1.price.min(p2.price);
        let w2_hi = p1.price.max(p2.price);
        let smc_ob = ctx.order_blocks.iter().any(|ob| {
            range_overlap(w2_lo, w2_hi, ob.low, ob.high)
        });
        let smc_fvg = ctx.fair_value_gaps.iter().any(|f| {
            let fl = f.lower.min(f.upper);
            let fh = f.lower.max(f.upper);
            range_overlap(w2_lo, w2_hi, fl, fh)
        });
        if smc_ob {
            score += 15.0;
        }
        if smc_fvg {
            score += 10.0;
        }
        ex.smc_w2_zone_overlap = Some(smc_ob || smc_fvg);
        let mut parts = Vec::new();
        if smc_ob {
            parts.push("OB");
        }
        if smc_fvg {
            parts.push("FVG");
        }
        if !parts.is_empty() {
            ex.smc_w2_detail = Some(format!("W2 bölgesi: {}", parts.join("+")));
        }
    }

    let score = score.min(100.0);
    ex.confluence_score = Some(score);
    ex.wave_grade = Some(grade_from_score(score));

    // EWO impulse hizası
    if let (Some(ev), Some(sv)) = (ewo_v, ewo_sig) {
        let aligned = if is_bullish_impulse {
            ev > sv
        } else {
            ev < sv
        };
        ex.ewo_aligned_with_impulse = Some(aligned);
        if config.elliott_require_ewo_alignment && validation_ok == Some(true) && !aligned {
            ex.fusion_ewo_soft_fail = Some(true);
        }
    }

    // --- Stabilite (Pine signal stability ilhamlı, stateless ölçüm) ---
    let i0 = candle_index_at_or_before(candles, wave_point_time_ms(p0.time));
    let i1 = candle_index_at_or_before(candles, wave_point_time_ms(p1.time));
    let i2 = candle_index_at_or_before(candles, wave_point_time_ms(p2.time));
    let last_i = candles.len().saturating_sub(1);

    let b01 = i0.zip(i1).map(|(a, b)| b.saturating_sub(a) as u32);
    let b12 = i1.zip(i2).map(|(a, b)| b.saturating_sub(a) as u32);
    let min_d = config.elliott_stability_min_wave_bars.max(1);
    let min_ok = b01.map(|d| d >= min_d).unwrap_or(false)
        && b12.map(|d| d >= min_d).unwrap_or(false);

    let confirm_need = config.elliott_stability_confirm_bars.max(1);
    let conf_ok = i2
        .map(|i2| (last_i as i64 - i2 as i64) >= confirm_need as i64)
        .unwrap_or(false);

    let age = i0.map(|i0| (last_i.saturating_sub(i0)) as u32);
    let auto_inv = config.elliott_stability_auto_invalidate_bars.max(20);
    let timeout = age.map(|a| a > auto_inv).unwrap_or(false);
    let bar_ms = avg_bar_ms(candles);

    ex.pattern_stability = Some(ElliottPatternStability {
        min_bar_distance_ok: min_ok,
        wave2_confirmation_ok: conf_ok,
        bars_wave0_to_wave1: b01,
        bars_wave1_to_wave2: b12,
        pattern_age_bars: age,
        auto_invalidate_bars: auto_inv,
        timeout_warning: timeout,
    });

    // Invalidate ipuçları (ek bilgi; `validation_ok` yerine geçmez)
    if let Some(lc) = candles.last() {
        if is_bullish_impulse && lc.low < p0.price {
            ex.invalidate_hint = Some("W0 altı (bull): yapı zayıfladı".to_string());
        } else if !is_bullish_impulse && lc.high > p0.price {
            ex.invalidate_hint = Some("W0 üstü (bear): yapı zayıfladı".to_string());
        } else if timeout {
            let approx_mins =
                (auto_inv as f64 * bar_ms as f64 / 60_000.0).max(0.0);
            ex.invalidate_hint = Some(format!(
                "Uzun süre aktif (>{auto_inv} bar, ~{approx_mins:.0} dk) — sayım yeniden değerlendirilmeli"
            ));
        }
    }

    ex
}
