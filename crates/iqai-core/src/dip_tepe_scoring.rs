//! Dip / Tepe skorlama sistemi (Madde 15).
//!
//! Sinyal bazlı puan: RSI oversold, MACD divergence, Support zone, Volume spike,
//! Bullish/Bearish candle, Fibonacci level, EMA200 yakın, Market structure.
//! Toplam 0–10; tavsiye: >=8 STRONG, >=6 BUY ZONE, >=4 WATCH, <4 NO SIGNAL.

use serde::{Deserialize, Serialize};

use crate::candlestick_patterns::{any_bearish_pattern, any_bullish_pattern, detect_candle_patterns};
use crate::config::Config;
use crate::indicators::{atr, bollinger, ema, macd, macd_line_at, pivot_high, pivot_low, rsi, sma};
use crate::reversal::{DipAnalysis, PeakAnalysis};
use crate::types::Candle;

/// Tek bir sinyalin adı ve puanı (UI’de “neden bu skor” için).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalScore {
    pub name: String,
    pub points: u8,
    pub active: bool,
}

/// Madde 15: Sinyal → Puan tablosu ve toplam skor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DipTepeScore {
    /// Sinyal bazlı puanlar (RSI oversold +1, Support zone +2, …).
    pub signals: Vec<SignalScore>,
    /// Toplam puan (0–10).
    pub total: u8,
    /// Madde 17: STRONG BUY DIP / BUY ZONE / WATCH DIP / NO SIGNAL (ve tepe karşılıkları).
    pub recommendation: String,
    /// Erken uyarı: momentum dönüşü (RSI slope up veya MACD histogram yukarı).
    pub early_warning_momentum: bool,
}

// Puanlar (Madde 15 tablosu)
const PTS_RSI: u8 = 1;
const PTS_MACD_DIV: u8 = 2;
const PTS_SUPPORT_ZONE: u8 = 2;
const PTS_VOLUME_SPIKE: u8 = 1;
const PTS_BULLISH_CANDLE: u8 = 1;
const PTS_FIB_LEVEL: u8 = 1;
const PTS_EMA200_NEAR: u8 = 1;
const PTS_MARKET_STRUCTURE: u8 = 1;
const PTS_BOLLINGER: u8 = 1;
const PTS_MEAN_REVERSION: u8 = 1;
const SCORE_CAP: u8 = 10;

/// Basit Fibonacci retracement: swing_high - (swing_high - swing_low) * ratio (Madde 7).
fn fib_retracement(high: f64, low: f64, ratio: f64) -> f64 {
    high - (high - low) * ratio
}

/// Fiyat, Fib seviyelerinden birine yeterince yakın mı (band %0.5).
fn price_near_fib_level(price: f64, swing_high: f64, swing_low: f64, band_pct: f64) -> bool {
    let levels = [
        fib_retracement(swing_high, swing_low, 0.382),
        fib_retracement(swing_high, swing_low, 0.5),
        fib_retracement(swing_high, swing_low, 0.618),
        fib_retracement(swing_high, swing_low, 0.786),
    ];
    let band = (swing_high - swing_low).max(1e-9) * band_pct;
    levels
        .iter()
        .any(|&lvl| (price - lvl).abs() <= band)
}

/// Son barlarda pivot low kümelerinden destek bölgesi: mean(pivot_lows) ± ATR (Madde 3).
fn support_zone_from_pivots(candles: &[Candle], pivot_len: usize, atr_val: f64) -> Option<(f64, f64)> {
    let pl = pivot_len.max(1);
    if candles.len() < pl * 3 + 1 {
        return None;
    }
    let mut lows = Vec::new();
    for i in (pl * 2 + 1)..=candles.len().saturating_sub(pl) {
        let sub = &candles[..=i + pl];
        if sub.len() < pl * 2 + 1 {
            continue;
        }
        if let Some(low) = pivot_low(sub, pl) {
            lows.push(low);
        }
    }
    if lows.is_empty() {
        return None;
    }
    let mean = lows.iter().sum::<f64>() / lows.len() as f64;
    let half_band = atr_val;
    Some((mean - half_band, mean + half_band))
}

/// Bullish MACD divergence: son iki pivot low'ta fiyat LL, MACD line HL.
fn bullish_macd_divergence(
    candles: &[Candle],
    _pivot_len: usize,
    pivot_low_indices: &[(usize, f64)],
) -> bool {
    if pivot_low_indices.len() < 2 {
        return false;
    }
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let (idx1, price1) = pivot_low_indices[1];
    let (idx2, price2) = pivot_low_indices[0];
    if idx2 <= idx1 || price2 >= price1 {
        return false;
    }
    let macd1 = macd_line_at(&closes, idx1, 12, 26);
    let macd2 = macd_line_at(&closes, idx2, 12, 26);
    match (macd1, macd2) {
        (Some(m1), Some(m2)) => m2 > m1,
        _ => false,
    }
}

/// Bearish MACD divergence: son iki pivot high'ta fiyat HH, MACD line LH.
fn bearish_macd_divergence(
    candles: &[Candle],
    _pivot_len: usize,
    pivot_high_indices: &[(usize, f64)],
) -> bool {
    if pivot_high_indices.len() < 2 {
        return false;
    }
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let (idx1, price1) = pivot_high_indices[1];
    let (idx2, price2) = pivot_high_indices[0];
    if idx2 <= idx1 || price2 <= price1 {
        return false;
    }
    let macd1 = macd_line_at(&closes, idx1, 12, 26);
    let macd2 = macd_line_at(&closes, idx2, 12, 26);
    match (macd1, macd2) {
        (Some(m1), Some(m2)) => m2 < m1,
        _ => false,
    }
}

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

/// RSI slope: son 3–5 bar RSI’da artış (dip için erken uyarı).
fn rsi_slope_up(closes: &[f64], period: usize) -> bool {
    if closes.len() < period + 4 {
        return false;
    }
    let r1 = rsi(closes, period);
    let r2 = rsi(&closes[..closes.len() - 1], period);
    let r3 = rsi(&closes[..closes.len().saturating_sub(2)], period);
    match (r1, r2, r3) {
        (Some(a), Some(b), Some(c)) => a > b && b > c,
        _ => false,
    }
}

/// MACD histogram önceki bara göre yukarı (momentum dönüşü).
fn macd_histogram_turning_up(closes: &[f64]) -> bool {
    if closes.len() < 30 {
        return false;
    }
    let cur = macd(closes, 12, 26, 9);
    let prev = macd(&closes[..closes.len() - 1], 12, 26, 9);
    match (cur, prev) {
        (Some(c), Some(p)) => c.histogram > p.histogram,
        _ => false,
    }
}

/// Dip için swing high/low: son önemli swing (basitçe son 50 bar high/low).
fn recent_swing_high_low(candles: &[Candle], lookback: usize) -> (f64, f64) {
    let start = candles.len().saturating_sub(lookback).min(candles.len());
    let slice = &candles[start..];
    let h = slice.iter().map(|c| c.high).fold(0.0_f64, f64::max);
    let l = slice.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
    (h, l)
}

/// Ana skorlama: tüm sinyalleri topla, 0–10 döndür, tavsiye üret (Madde 15 + 17).
pub fn compute_dip_tepe_score(
    candles: &[Candle],
    config: &Config,
    is_long: bool,
    _dip: Option<&DipAnalysis>,
    _peak: Option<&PeakAnalysis>,
    structure_score_01: f64,
    mtf_support_near: bool,
) -> DipTepeScore {
    let pivot_len = config.pivot_length as usize;
    let mut signals = Vec::new();
    let mut total: i32 = 0;

    let atr_val = atr(candles, 14).unwrap_or_else(|| {
        candles
            .last()
            .map(|c| (c.high - c.low).max(1e-6))
            .unwrap_or(1.0)
    });
    let last = candles.last().map(|c| c.close).unwrap_or(0.0);
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

    // 1. RSI oversold (dip) / overbought (tepe) – Madde 4
    let rsi_val = rsi(&closes, 14);
    let rsi_ok = match rsi_val {
        Some(r) => {
            if is_long {
                r < config.q_rsi_oversold
            } else {
                r > config.q_rsi_overbought
            }
        }
        None => false,
    };
    if rsi_ok {
        total += PTS_RSI as i32;
    }
    signals.push(SignalScore {
        name: if is_long {
            "RSI aşırı satım".to_string()
        } else {
            "RSI aşırı alım".to_string()
        },
        points: PTS_RSI,
        active: rsi_ok,
    });

    // 2. MACD divergence – Madde 5
    let plows = last_two_pivot_lows(candles, pivot_len);
    let phighs = last_two_pivot_highs(candles, pivot_len);
    let macd_div = if is_long {
        bullish_macd_divergence(candles, pivot_len, &plows)
    } else {
        bearish_macd_divergence(candles, pivot_len, &phighs)
    };
    if macd_div {
        total += PTS_MACD_DIV as i32;
    }
    signals.push(SignalScore {
        name: "MACD divergence".to_string(),
        points: PTS_MACD_DIV,
        active: macd_div,
    });

    // 3. Support zone (pivot cluster ± ATR) veya MTF destek – Madde 3
    let support_local = support_zone_from_pivots(candles, pivot_len, atr_val)
        .map_or(false, |(lo, hi)| last >= lo && last <= hi);
    let support_ok = support_local || mtf_support_near;
    if support_ok {
        total += PTS_SUPPORT_ZONE as i32;
    }
    signals.push(SignalScore {
        name: "Destek/direnç bölgesi (MTF dahil)".to_string(),
        points: PTS_SUPPORT_ZONE,
        active: support_ok,
    });

    // 4. Volume spike – Madde 8: volume > volume_MA * 1.5
    let vols: Vec<f64> = candles.iter().map(|c| c.volume).collect();
    let vol_ma = sma(&vols, 20.min(vols.len())).unwrap_or(0.0);
    let vol_spike = vol_ma > 0.0 && *vols.last().unwrap_or(&0.0) >= vol_ma * 1.5;
    if vol_spike {
        total += PTS_VOLUME_SPIKE as i32;
    }
    signals.push(SignalScore {
        name: "Hacim spike".to_string(),
        points: PTS_VOLUME_SPIKE,
        active: vol_spike,
    });

    // 5. Bullish/Bearish candle (reversal candle) – Madde 9
    let candle_ok = if let Some(c) = candles.last() {
        if is_long {
            c.is_bullish()
        } else {
            c.is_bearish()
        }
    } else {
        false
    };
    let patterns = detect_candle_patterns(candles, is_long);
    let pattern_ok = if is_long {
        any_bullish_pattern(&patterns)
    } else {
        any_bearish_pattern(&patterns)
    };
    let candle_or_pattern = candle_ok || pattern_ok;
    if candle_or_pattern {
        total += PTS_BULLISH_CANDLE as i32;
    }
    signals.push(SignalScore {
        name: if is_long {
            "Yükseliş mumu / pattern".to_string()
        } else {
            "Düşüş mumu / pattern".to_string()
        },
        points: PTS_BULLISH_CANDLE,
        active: candle_or_pattern,
    });

    // 6. Fibonacci level – Madde 7
    let (swing_h, swing_l) = recent_swing_high_low(candles, 50);
    let fib_ok = price_near_fib_level(last, swing_h, swing_l, 0.005);
    if fib_ok {
        total += PTS_FIB_LEVEL as i32;
    }
    signals.push(SignalScore {
        name: "Fibonacci seviyesi (0.382–0.786)".to_string(),
        points: PTS_FIB_LEVEL,
        active: fib_ok,
    });

    // 7. EMA200 yakın – Madde 10
    let ema200 = ema(&closes, 200.min(closes.len()));
    let near_ema200 = ema200.map_or(false, |e| {
        let dist = ((last - e) / e.max(1e-9)).abs();
        dist <= 0.01
    });
    if near_ema200 {
        total += PTS_EMA200_NEAR as i32;
    }
    signals.push(SignalScore {
        name: "Fiyat EMA200 yakın".to_string(),
        points: PTS_EMA200_NEAR,
        active: near_ema200,
    });

    // 8. Market structure (HL / LH) – Madde 11
    let structure_ok = structure_score_01 >= 0.55;
    if structure_ok {
        total += PTS_MARKET_STRUCTURE as i32;
    }
    signals.push(SignalScore {
        name: "Piyasa yapısı (HL/LH)".to_string(),
        points: PTS_MARKET_STRUCTURE,
        active: structure_ok,
    });

    // 9. Bollinger reversion – Madde 6: dip Close < lower_band, tepe Close > upper_band
    let bb = bollinger(&closes, 20, 2.0);
    let bollinger_ok = bb.map_or(false, |(lower, _mid, upper)| {
        if is_long {
            last <= lower
        } else {
            last >= upper
        }
    });
    if bollinger_ok {
        total += PTS_BOLLINGER as i32;
    }
    signals.push(SignalScore {
        name: "Bollinger reversion".to_string(),
        points: PTS_BOLLINGER,
        active: bollinger_ok,
    });

    // 10. Mean reversion distance – Madde 12: (price - MA) / MA, dip < -0.1
    let ma = sma(&closes, 20.min(closes.len())).unwrap_or(last);
    let dist = if ma > 0.0 { (last - ma) / ma } else { 0.0 };
    let mean_rev_ok = if is_long {
        dist < -0.1
    } else {
        dist > 0.1
    };
    if mean_rev_ok {
        total += PTS_MEAN_REVERSION as i32;
    }
    signals.push(SignalScore {
        name: "Ortalamadan sapma (mean reversion)".to_string(),
        points: PTS_MEAN_REVERSION,
        active: mean_rev_ok,
    });

    let total_u8 = total.max(0).min(SCORE_CAP as i32) as u8;

    // Madde 17: Tavsiye motoru
    let recommendation = if total_u8 >= 8 {
        if is_long {
            "STRONG BUY DIP".to_string()
        } else {
            "STRONG SELL TEPE".to_string()
        }
    } else if total_u8 >= 6 {
        if is_long {
            "BUY ZONE".to_string()
        } else {
            "SELL ZONE".to_string()
        }
    } else if total_u8 >= 4 {
        if is_long {
            "WATCH DIP".to_string()
        } else {
            "WATCH TEPE".to_string()
        }
    } else {
        "NO SIGNAL".to_string()
    };

    // Madde 16: Erken uyarı – momentum dönüşü
    let early_warning = if is_long {
        rsi_slope_up(&closes, 14) || macd_histogram_turning_up(&closes)
    } else {
        macd_histogram_turning_up(&closes)
    };

    DipTepeScore {
        signals,
        total: total_u8,
        recommendation,
        early_warning_momentum: early_warning,
    }
}
