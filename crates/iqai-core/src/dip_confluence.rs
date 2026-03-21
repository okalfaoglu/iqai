//! Dip/tepe tespitinde çoklu doğrulama (confluence) katmanları.
//!
//! Dokümandaki katmanlar: MTF destek, LTF yapı kırılımı (MSS), Elliott + Fib cluster,
//! momentum divergence. Her katman "True" ise güven/erken uyarı skorları artar.

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::elliott_detector::compute_elliott;
use crate::indicators::{atr, pivot_high, pivot_low, rsi};
use crate::reversal::{DipAnalysis, PeakAnalysis};
use crate::signal::{CandleBuffer, SignalEngine};
use crate::types::{Candle, SignalType, Timeframe};

/// Üst zaman dilimleri (chart_tf'den büyük eşit dakika).
fn higher_timeframes(chart_tf: Timeframe) -> Vec<Timeframe> {
    let all: Vec<Timeframe> = vec![
        Timeframe::M1,
        Timeframe::M5,
        Timeframe::M15,
        Timeframe::M30,
        Timeframe::H1,
        Timeframe::H4,
        Timeframe::D1,
    ];
    let chart_min = chart_tf.minutes();
    all.into_iter()
        .filter(|tf| tf.minutes() >= chart_min && *tf != chart_tf)
        .collect()
}

/// Çoklu doğrulama sonucu – Q-RADAR güven/erken uyarı artırımında kullanılır.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DipConfluenceResult {
    /// Üst TF'de destek/dip bölgesine yakın mı (pivot low + ATR bandı)
    pub mtf_support_near: bool,
    /// Alt TF'de yapı yukarı kırıldı mı (HL / BOS tarzı)
    pub ltf_structure_ok: bool,
    /// Fiyat Elliott/Fib cluster bölgesinde mi (W2/W4/C veya 0.618/0.786)
    pub fib_elliott_zone: bool,
    /// Fiyat LL + momentum (RSI) HL = pozitif uyumsuzluk
    pub divergence_ok: bool,
    /// Wyckoff Spring (dip) veya Upthrust (tepe) tespit edildi mi
    pub spring_ok: bool,
    /// Long: RSI < oversold eşiği; Short: RSI > overbought eşiği
    pub rsi_zone_ok: bool,
    /// Break of structure: long son tepe kırıldı mı, short son dip kırıldı mı
    pub bos_ok: bool,
    /// Absorption: destek/tepe bandında hacim artışı ve band tutunması
    pub absorption_ok: bool,
    /// Kaç katman geçti (0–8)
    pub     layers_passed: u8,
}

/// Çoklu doğrulama katmanlarını hesapla. LONG için dip, SHORT için tepe.
pub fn compute_dip_confluence(
    buffer: &CandleBuffer,
    chart_tf: Timeframe,
    config: &Config,
    reference_price: f64,
    is_long: bool,
    dip: Option<&DipAnalysis>,
    peak: Option<&PeakAnalysis>,
) -> DipConfluenceResult {
    let candles = match buffer.get(chart_tf) {
        Some(c) if c.len() >= config.pivot_length as usize * 3 + 20 => c,
        _ => return DipConfluenceResult::default(),
    };
    let pivot_len = config.pivot_length as usize;
    let atr_val = atr(candles, config.dip_confluence_atr_period as usize).unwrap_or_else(|| {
        candles
            .last()
            .map(|c| (c.high - c.low).max(1e-6))
            .unwrap_or(1.0)
    });

    let mut mtf_support_near = false;
    for htf in higher_timeframes(chart_tf) {
        if let Some(htf_candles) = buffer.get(htf) {
            if htf_candles.len() < pivot_len * 2 + 1 {
                continue;
            }
            let support = if is_long {
                pivot_low(htf_candles, pivot_len)
            } else {
                pivot_high(htf_candles, pivot_len)
            };
            if let Some(sup) = support {
                let band = atr_val * config.dip_confluence_mtf_atr_band;
                let in_zone = if is_long {
                    reference_price >= sup - band && reference_price <= sup + band
                } else {
                    reference_price >= sup - band && reference_price <= sup + band
                };
                if in_zone {
                    mtf_support_near = true;
                    break;
                }
            }
        }
    }

    let engine = SignalEngine::new(config.clone());
    let side = if is_long {
        SignalType::Buy
    } else {
        SignalType::Sell
    };
    let structure_score = engine.structure_score(candles, side, pivot_len);
    let ltf_structure_ok = structure_score >= config.dip_confluence_structure_score_min;

    let mut fib_elliott_zone = false;
    let elliott = compute_elliott(candles, config, false);
    if elliott.validation_ok == Some(true) || elliott.corr_setup.is_some() {
        let mut ref_levels: Vec<f64> = Vec::new();
        if let Some(ref imp) = elliott.impulse_state {
            if imp.is_bullish == is_long {
                if let Some(ref w3) = imp.setup_w3 {
                    if let Some(e) = w3.get("entry").and_then(|v| v.as_f64()) {
                        ref_levels.push(e);
                    }
                }
                if let Some(ref w5) = imp.setup_w5 {
                    if let Some(e) = w5.get("entry").and_then(|v| v.as_f64()) {
                        ref_levels.push(e);
                    }
                }
            }
        }
        if let Some(ref corr) = elliott.corr_setup {
            if corr.is_long == is_long {
                ref_levels.push(corr.entry);
            }
        }
        for level in elliott.fibo_levels.iter() {
            ref_levels.push(level.price);
        }
        let band_pct = config.dip_confluence_fib_price_band_pct;
        for lvl in ref_levels {
            if (reference_price - lvl).abs() / lvl.max(1e-10) <= band_pct {
                fib_elliott_zone = true;
                break;
            }
        }
    }

    let rsi_p = config.dip_tepe_rsi_period as usize;
    let divergence_ok = if is_long {
        bullish_divergence(candles, pivot_len, rsi_p)
    } else {
        bearish_divergence(candles, pivot_len, rsi_p)
    };

    let spring_ok = if is_long {
        dip.map(|d| d.spring_detected).unwrap_or(false)
    } else {
        peak.map(|p| p.upthrust_detected).unwrap_or(false)
    };

    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let last_rsi = rsi(&closes, rsi_p);
    let rsi_zone_ok = match last_rsi {
        Some(r) => {
            if is_long {
                r < config.q_rsi_oversold
            } else {
                r > config.q_rsi_overbought
            }
        }
        None => false,
    };

    let last = candles.last().unwrap();
    let bos_ok = if is_long {
        let phs = last_two_pivot_highs(candles, pivot_len);
        phs.first()
            .map(|(_, ph)| last.close > *ph)
            .unwrap_or(false)
    } else {
        let pls = last_two_pivot_lows(candles, pivot_len);
        pls.first()
            .map(|(_, pl)| last.close < *pl)
            .unwrap_or(false)
    };

    let band_center = if is_long {
        dip.map(|d| d.dip_price)
    } else {
        peak.map(|p| p.peak_price)
    };
    let abs_bars = config.dip_confluence_absorption_bars as usize;
    let vol_avg_nbars = config.dip_confluence_absorption_vol_avg_bars as usize;
    let absorption_ok = band_center.map_or(false, |center| {
        let band = atr_val * config.dip_confluence_absorption_atr_margin;
        let band_low = center - band;
        let band_high = center + band;
        if candles.len() < abs_bars + vol_avg_nbars {
            return false;
        }
        let start = candles.len().saturating_sub(vol_avg_nbars);
        let vol_avg_ref: f64 = candles[start..].iter().map(|c| c.volume).sum::<f64>()
            / vol_avg_nbars as f64;
        let last_n = candles.len().saturating_sub(abs_bars);
        let vol_sum_n: f64 = candles[last_n..].iter().map(|c| c.volume).sum();
        let vol_avg_n = vol_sum_n / abs_bars as f64;
        if vol_avg_ref <= 0.0 || vol_avg_n < vol_avg_ref * config.dip_confluence_absorption_volume_ratio {
            return false;
        }
        if is_long {
            !candles[last_n..].iter().any(|c| c.close < band_low)
        } else {
            !candles[last_n..].iter().any(|c| c.close > band_high)
        }
    });

    let layers_passed = [
        mtf_support_near,
        ltf_structure_ok,
        fib_elliott_zone,
        divergence_ok,
        spring_ok,
        rsi_zone_ok,
        bos_ok,
        absorption_ok,
    ]
    .into_iter()
    .filter(|&x| x)
    .count() as u8;

    DipConfluenceResult {
        mtf_support_near,
        ltf_structure_ok,
        fib_elliott_zone,
        divergence_ok,
        spring_ok,
        rsi_zone_ok,
        bos_ok,
        absorption_ok,
        layers_passed,
    }
}

/// Son iki pivot low bar indeksleri ve fiyatları (en sondan geriye).
fn last_two_pivot_lows(candles: &[Candle], pivot_len: usize) -> Vec<(usize, f64)> {
    let mut out = Vec::with_capacity(2);
    let pl = pivot_len.max(1);
    if candles.len() < pl * 2 + 1 {
        return out;
    }
    for i in (pl..=candles.len().saturating_sub(1 + pl)).rev() {
        let sub = &candles[..=i + pl];
        if sub.len() < pl * 2 + 1 {
            continue;
        }
        if let Some(low) = pivot_low(sub, pl) {
            let idx = sub.len() - 1 - pl;
            if (candles[idx].low - low).abs() < 1e-9 {
                out.push((idx, low));
                if out.len() >= 2 {
                    break;
                }
            }
        }
    }
    out
}

/// Son iki pivot high bar indeksleri ve fiyatları.
fn last_two_pivot_highs(candles: &[Candle], pivot_len: usize) -> Vec<(usize, f64)> {
    let mut out = Vec::with_capacity(2);
    let pl = pivot_len.max(1);
    if candles.len() < pl * 2 + 1 {
        return out;
    }
    for i in (pl..=candles.len().saturating_sub(1 + pl)).rev() {
        let sub = &candles[..=i + pl];
        if sub.len() < pl * 2 + 1 {
            continue;
        }
        if let Some(high) = pivot_high(sub, pl) {
            let idx = sub.len() - 1 - pl;
            if (candles[idx].high - high).abs() < 1e-9 {
                out.push((idx, high));
                if out.len() >= 2 {
                    break;
                }
            }
        }
    }
    out
}

/// Bullish divergence: fiyat LL (lower low), RSI HL (higher low).
fn bullish_divergence(candles: &[Candle], pivot_len: usize, rsi_period: usize) -> bool {
    let pivots = last_two_pivot_lows(candles, pivot_len);
    if pivots.len() < 2 {
        return false;
    }
    let (idx1, price1) = pivots[1];
    let (idx2, price2) = pivots[0];
    if idx2 <= idx1 || price2 >= price1 {
        return false;
    }
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let rsi1 = rsi(&closes[..=idx1], rsi_period);
    let rsi2 = rsi(&closes[..=idx2], rsi_period);
    match (rsi1, rsi2) {
        (Some(r1), Some(r2)) => r2 > r1,
        _ => false,
    }
}

/// Bearish divergence: fiyat HH (higher high), RSI LH (lower high).
fn bearish_divergence(candles: &[Candle], pivot_len: usize, rsi_period: usize) -> bool {
    let pivots = last_two_pivot_highs(candles, pivot_len);
    if pivots.len() < 2 {
        return false;
    }
    let (idx1, price1) = pivots[1];
    let (idx2, price2) = pivots[0];
    if idx2 <= idx1 || price2 <= price1 {
        return false;
    }
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let rsi1 = rsi(&closes[..=idx1], rsi_period);
    let rsi2 = rsi(&closes[..=idx2], rsi_period);
    match (rsi1, rsi2) {
        (Some(r1), Some(r2)) => r2 < r1,
        _ => false,
    }
}
