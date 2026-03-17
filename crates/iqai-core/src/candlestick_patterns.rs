//! Candlestick pattern tespiti (Madde 9).
//!
//! Dip: Hammer, Bullish Engulfing, Morning Star, Piercing.
//! Tepe: Shooting Star, Bearish Engulfing, Evening Star, Dark Cloud Cover.

use crate::types::Candle;

/// Son 1–3 mumda tespit edilen pattern (dip için bullish, tepe için bearish).
#[derive(Debug, Clone, Default)]
pub struct CandlePatternSignals {
    pub hammer: bool,
    pub bullish_engulfing: bool,
    pub morning_star: bool,
    pub piercing: bool,
    pub shooting_star: bool,
    pub bearish_engulfing: bool,
    pub evening_star: bool,
    pub dark_cloud_cover: bool,
}

/// Son barlarda dip (bullish) veya tepe (bearish) pattern var mı; skorlama için tek sinyal.
pub fn detect_candle_patterns(candles: &[Candle], is_dip: bool) -> CandlePatternSignals {
    let mut out = CandlePatternSignals::default();
    if candles.len() < 3 {
        return out;
    }
    let last = candles.len() - 1;
    let c = &candles[last];
    let prev = &candles[last - 1];
    let prev2 = if last >= 2 { Some(&candles[last - 2]) } else { None };

    if is_dip {
        out.hammer = is_hammer_bullish(c);
        out.bullish_engulfing = is_bullish_engulfing(prev, c);
        out.piercing = is_piercing(prev, c);
        if let Some(p2) = prev2 {
            out.morning_star = is_morning_star(p2, prev, c);
        }
    } else {
        out.shooting_star = is_shooting_star(c);
        out.bearish_engulfing = is_bearish_engulfing(prev, c);
        out.dark_cloud_cover = is_dark_cloud_cover(prev, c);
        if let Some(p2) = prev2 {
            out.evening_star = is_evening_star(p2, prev, c);
        }
    }
    out
}

/// Herhangi bir dip (bullish) pattern tetikli mi?
pub fn any_bullish_pattern(signals: &CandlePatternSignals) -> bool {
    signals.hammer
        || signals.bullish_engulfing
        || signals.morning_star
        || signals.piercing
}

/// Herhangi bir tepe (bearish) pattern tetikli mi?
pub fn any_bearish_pattern(signals: &CandlePatternSignals) -> bool {
    signals.shooting_star
        || signals.bearish_engulfing
        || signals.evening_star
        || signals.dark_cloud_cover
}

fn body(c: &Candle) -> f64 {
    (c.close - c.open).abs()
}

fn upper_wick(c: &Candle) -> f64 {
    c.high - c.open.max(c.close)
}

fn lower_wick(c: &Candle) -> f64 {
    c.open.min(c.close) - c.low
}

/// Hammer: küçük gövde üstte, uzun alt gölge, dip sonrası.
fn is_hammer_bullish(c: &Candle) -> bool {
    if !c.is_bullish() {
        return false;
    }
    let range = (c.high - c.low).max(1e-9);
    let body_len = body(c);
    let lower = lower_wick(c);
    lower >= body_len * 2.0 && lower >= range * 0.5 && body_len <= range * 0.35
}

/// Bullish Engulfing: önceki mumu tamamen saran yükseliş mumu.
fn is_bullish_engulfing(prev: &Candle, c: &Candle) -> bool {
    prev.is_bearish() && c.is_bullish() && c.open < prev.close && c.close > prev.open
}

/// Morning Star: üç mum – düşüş, küçük gövde, güçlü yükseliş.
fn is_morning_star(prev2: &Candle, prev: &Candle, c: &Candle) -> bool {
    if !prev2.is_bearish() || !c.is_bullish() {
        return false;
    }
    let body2 = body(prev2);
    let body_c = body(c);
    let small_body = prev.open.min(prev.close) > prev2.close - body2 * 0.5
        && prev.high - prev.low < body2;
    small_body && c.close > (prev2.open + prev2.close) / 2.0 && body_c > body2 * 0.5
}

/// Piercing: önceki düşüş mumunun gövdesinin yarısından fazlasını kapatan yükseliş.
fn is_piercing(prev: &Candle, c: &Candle) -> bool {
    if !prev.is_bearish() || !c.is_bullish() {
        return false;
    }
    let mid = prev.open + (prev.close - prev.open) / 2.0;
    c.open < prev.close && c.close > mid && c.close < prev.open
}

/// Shooting Star: küçük gövde altta, uzun üst gölge.
fn is_shooting_star(c: &Candle) -> bool {
    if !c.is_bearish() {
        return false;
    }
    let range = (c.high - c.low).max(1e-9);
    let body_len = body(c);
    let upper = upper_wick(c);
    upper >= body_len * 2.0 && upper >= range * 0.5 && body_len <= range * 0.35
}

/// Bearish Engulfing: önceki yükseliş mumunu tamamen saran düşüş mumu.
fn is_bearish_engulfing(prev: &Candle, c: &Candle) -> bool {
    prev.is_bullish() && c.is_bearish() && c.open > prev.close && c.close < prev.open
}

/// Evening Star: üç mum – yükseliş, küçük gövde, güçlü düşüş.
fn is_evening_star(prev2: &Candle, prev: &Candle, c: &Candle) -> bool {
    if !prev2.is_bullish() || !c.is_bearish() {
        return false;
    }
    let body2 = body(prev2);
    let body_c = body(c);
    let small_body = prev.open.max(prev.close) < prev2.close + body2 * 0.5
        && prev.high - prev.low < body2;
    small_body && c.close < (prev2.open + prev2.close) / 2.0 && body_c > body2 * 0.5
}

/// Dark Cloud Cover: önceki yükseliş mumunun gövdesinin yarısından aşağı kapanan düşüş.
fn is_dark_cloud_cover(prev: &Candle, c: &Candle) -> bool {
    if !prev.is_bullish() || !c.is_bearish() {
        return false;
    }
    let mid = prev.close + (prev.open - prev.close) / 2.0;
    c.open > prev.high && c.close < mid && c.close > prev.close
}
