//! Fake breakout (liquidity sweep) detector.
//!
//! Goal: detect "stop hunt" style moves:
//! - price sweeps recent high/low (liquidity grab),
//! - closes back inside the range (rejection),
//! - then confirms with a structure break (BOS) in the opposite direction.

use serde::{Deserialize, Serialize};

use crate::indicators::{atr, ema, highest, lowest, vwap};
use crate::types::Candle;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FakeBreakoutConfig {
    /// Liquidity sweep lookback bars (recent high/low).
    pub lookback: usize,
    /// BOS confirmation lookback bars.
    pub bos_lookback: usize,
    /// Minimum wick ratio for rejection candle (0..1).
    pub min_wick_ratio: f64,
    /// Stop buffer as ATR multiple.
    pub sl_atr_mult: f64,
    /// Fallback TP as RR multiple (if EMA/VWAP not usable).
    pub tp_rr: f64,
}

impl Default for FakeBreakoutConfig {
    fn default() -> Self {
        Self {
            lookback: 40,
            bos_lookback: 6,
            min_wick_ratio: 0.35,
            sl_atr_mult: 0.2,
            tp_rr: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FakeBreakoutSignal {
    pub is_long: bool,
    pub entry: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub reason: String,
}

fn wick_ratio_upper(c: &Candle) -> f64 {
    let range = (c.high - c.low).max(1e-12);
    let upper_wick = c.high - c.open.max(c.close);
    (upper_wick / range).clamp(0.0, 1.0)
}

fn wick_ratio_lower(c: &Candle) -> f64 {
    let range = (c.high - c.low).max(1e-12);
    let lower_wick = c.open.min(c.close) - c.low;
    (lower_wick / range).clamp(0.0, 1.0)
}

/// Detect a conservative fake breakout signal (2-candle pattern):
/// - sweep candle (prev) grabs liquidity and closes back inside
/// - confirm candle (last) breaks short-term structure in the opposite direction
pub fn detect_fake_breakout_signal(
    candles: &[Candle],
    is_long: bool,
    cfg: FakeBreakoutConfig,
) -> Option<FakeBreakoutSignal> {
    // Need enough history + 2 candles (sweep + confirm)
    let lookback = cfg.lookback.max(10);
    let bos_lookback = cfg.bos_lookback.max(3);
    if candles.len() < lookback + 3 {
        return None;
    }

    let confirm = candles.last()?;
    let sweep = candles.get(candles.len().saturating_sub(2))?;
    let hist = &candles[..candles.len().saturating_sub(2)];

    let highs: Vec<f64> = hist.iter().map(|c| c.high).collect();
    let lows: Vec<f64> = hist.iter().map(|c| c.low).collect();
    let prev_high = highest(&highs, lookback)?;
    let prev_low = lowest(&lows, lookback)?;

    let atr_val = atr(candles, 14).unwrap_or_else(|| (confirm.high - confirm.low).max(1e-6));
    let sl_buffer = atr_val * cfg.sl_atr_mult.max(0.0);

    let bos_highs: Vec<f64> = hist.iter().rev().take(bos_lookback).map(|c| c.high).collect();
    let bos_lows: Vec<f64> = hist.iter().rev().take(bos_lookback).map(|c| c.low).collect();
    let bos_recent_high = bos_highs.iter().cloned().fold(0.0_f64, f64::max);
    let bos_recent_low = bos_lows.iter().cloned().fold(f64::INFINITY, f64::min);

    if !is_long {
        // SHORT fake breakout:
        // A) sweep above recent high
        // B) closes back below prev_high (reclaim)
        // C) rejection-ish (upper wick)
        // D) confirm breaks recent low structure
        let swept = sweep.high > prev_high && sweep.close < prev_high;
        let rejection = wick_ratio_upper(sweep) >= cfg.min_wick_ratio || sweep.close < sweep.open;
        let bos = confirm.close < bos_recent_low;
        if !(swept && rejection && bos) {
            return None;
        }

        let entry = confirm.close;
        let stop_loss = sweep.high + sl_buffer;

        // TP preference: EMA200 or VWAP if sensible, otherwise 2R
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let ema200 = ema(&closes, 200.min(closes.len()));
        let vw = vwap(candles);
        let risk = (stop_loss - entry).max(1e-9);
        let mut tp = entry - cfg.tp_rr.max(0.5) * risk;
        if let Some(e) = ema200 {
            if e < entry {
                tp = e;
            }
        } else if let Some(v) = vw {
            if v < entry {
                tp = v;
            }
        }

        return Some(FakeBreakoutSignal {
            is_long: false,
            entry,
            stop_loss,
            take_profit: tp,
            reason: format!(
                "Fake breakout SHORT: sweep>prev_high {:.2}, reclaim, BOS<recent_low {:.2}",
                prev_high, bos_recent_low
            ),
        });
    }

    // LONG fake breakout:
    // A) sweep below recent low
    // B) closes back above prev_low (reclaim)
    // C) rejection-ish (lower wick)
    // D) confirm breaks recent high structure
    let swept = sweep.low < prev_low && sweep.close > prev_low;
    let rejection = wick_ratio_lower(sweep) >= cfg.min_wick_ratio || sweep.close > sweep.open;
    let bos = confirm.close > bos_recent_high;
    if !(swept && rejection && bos) {
        return None;
    }

    let entry = confirm.close;
    let stop_loss = sweep.low - sl_buffer;

    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let ema200 = ema(&closes, 200.min(closes.len()));
    let vw = vwap(candles);
    let risk = (entry - stop_loss).max(1e-9);
    let mut tp = entry + cfg.tp_rr.max(0.5) * risk;
    if let Some(e) = ema200 {
        if e > entry {
            tp = e;
        }
    } else if let Some(v) = vw {
        if v > entry {
            tp = v;
        }
    }

    Some(FakeBreakoutSignal {
        is_long: true,
        entry,
        stop_loss,
        take_profit: tp,
        reason: format!(
            "Fake breakout LONG: sweep<prev_low {:.2}, reclaim, BOS>recent_high {:.2}",
            prev_low, bos_recent_high
        ),
    })
}

