//! Technical indicators - 1:1 port from Pine Script

use crate::types::Candle;

/// Exponential Moving Average
pub fn ema(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period {
        return None;
    }
    let k = 2.0 / (period as f64 + 1.0);
    let mut ema_val = prices[..period].iter().sum::<f64>() / period as f64;
    for p in prices.iter().skip(period) {
        ema_val = (p - ema_val) * k + ema_val;
    }
    Some(ema_val)
}

/// Simple Moving Average
pub fn sma(data: &[f64], period: usize) -> Option<f64> {
    if data.len() < period {
        return None;
    }
    Some(data[data.len() - period..].iter().sum::<f64>() / period as f64)
}

/// Average True Range (14-period default)
pub fn atr(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < period + 1 {
        return None;
    }
    let mut tr_sum = 0.0;
    for i in (candles.len() - period - 1)..(candles.len() - 1) {
        let high = candles[i].high;
        let low = candles[i].low;
        let prev_close = candles[i + 1].close;
        let tr = (high - low)
            .max((high - prev_close).abs())
            .max((low - prev_close).abs());
        tr_sum += tr;
    }
    Some(tr_sum / period as f64)
}

/// RSI (Relative Strength Index)
pub fn rsi(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period + 1 {
        return None;
    }
    let mut gains = 0.0;
    let mut losses = 0.0;
    for i in (prices.len() - period - 1)..(prices.len() - 1) {
        let change = prices[i] - prices[i + 1];
        if change > 0.0 {
            gains += change;
        } else {
            losses += -change;
        }
    }
    let avg_gain = gains / period as f64;
    let avg_loss = losses / period as f64;
    if avg_loss == 0.0 {
        return Some(100.0);
    }
    let rs = avg_gain / avg_loss;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

/// Pivot High - returns Some(high) if valid pivot, None otherwise
/// Pine: ta.pivothigh(high, length, length)
pub fn pivot_high(candles: &[Candle], length: usize) -> Option<f64> {
    if candles.len() < length * 2 + 1 {
        return None;
    }
    let pivot_bar = candles[candles.len() - 1 - length].high;
    for i in 1..=length {
        if candles[candles.len() - 1 - length - i].high >= pivot_bar {
            return None;
        }
        if candles[candles.len() - 1 - length + i].high >= pivot_bar {
            return None;
        }
    }
    Some(pivot_bar)
}

/// Pivot Low
pub fn pivot_low(candles: &[Candle], length: usize) -> Option<f64> {
    if candles.len() < length * 2 + 1 {
        return None;
    }
    let pivot_bar = candles[candles.len() - 1 - length].low;
    for i in 1..=length {
        if candles[candles.len() - 1 - length - i].low <= pivot_bar {
            return None;
        }
        if candles[candles.len() - 1 - length + i].low <= pivot_bar {
            return None;
        }
    }
    Some(pivot_bar)
}

/// VWAP from session start (candles assumed to be from same session)
/// cumulative(hlc3 * volume) / cumulative(volume)
pub fn vwap(candles: &[Candle]) -> Option<f64> {
    if candles.is_empty() {
        return None;
    }
    let mut sum_pv = 0.0;
    let mut sum_v = 0.0;
    for c in candles {
        sum_pv += c.hlc3() * c.volume;
        sum_v += c.volume;
    }
    if sum_v == 0.0 {
        return None;
    }
    Some(sum_pv / sum_v)
}

/// Highest value over period
pub fn highest(highs: &[f64], period: usize) -> Option<f64> {
    if highs.len() < period {
        return None;
    }
    Some(
        highs[highs.len() - period..]
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max),
    )
}

/// Lowest value over period
pub fn lowest(lows: &[f64], period: usize) -> Option<f64> {
    if lows.len() < period {
        return None;
    }
    Some(
        lows[lows.len() - period..]
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min),
    )
}
