//! Dip / tepe fiyatı, dipten/tepeden dönüş ve dönüş gücü tespiti.
//!
//! - **Dip fiyatı:** Son pivot low (swing low).
//! - **Dipten dönüş:** Dip oluştuktan sonra fiyat dip + margin üzerine çıkmış ve mumlar yükseliş yönünde.
//! - **Dönüş gücü:** Bounce mesafesi (ATR veya %), hacim oranı ve yapı (HL) ile skor.
//! - **Tepe fiyatı / tepeden dönüş / düşüş gücü:** Aynı mantık, pivot high ve aşağı yön için.

use serde::{Deserialize, Serialize};

use crate::indicators::{atr, pivot_high, pivot_low, sma};
use crate::types::Candle;

/// Pivot uzunluğu (varsayılan 5 = 5 bar sol, merkez, 5 bar sağ)
const DEFAULT_PIVOT_LEN: usize = 5;
const ATR_PERIOD: usize = 14;
/// Dipten dönüş için dip üzerinde en az bu kadar ATR yukarı çıkış
const REVERSAL_MARGIN_ATR: f64 = 0.2;
/// Tepeden dönüş için tepe altında en az bu kadar ATR aşağı
const REVERSAL_MARGIN_ATR_DOWN: f64 = 0.2;
/// Güç skoru: bu kadar ATR hareket = 1.0 (tam güç)
const STRENGTH_ATR_FULL: f64 = 2.0;
/// Spring: dip altına indikten sonra en fazla bu kadar bar içinde tekrar üstüne dönmeli
const SPRING_RECOVERY_BARS: usize = 4;

/// Bir sembol/timeframe için dip analizi: dip fiyatı, dipten dönüş tespiti, dönüş gücü.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DipAnalysis {
    /// Son tespit edilen dip fiyatı (pivot low).
    pub dip_price: f64,
    /// Dip barının zamanı (ms).
    pub dip_time: i64,
    /// Dip barının indeksi (candles içinde).
    pub dip_bar_index: usize,
    /// Dip oluştuktan sonra geçen bar sayısı.
    pub bars_since_dip: usize,
    /// Dipten dönüş tespit edildi mi (fiyat dip + margin üzerinde ve yükseliş mumları).
    pub reversal_detected: bool,
    /// Dönüş gücü 0–1 (yüksek = güçlü tepki): bounce/ATR + hacim oranı + yapı.
    pub reversal_strength: f64,
    /// Son kapanış – dip arasındaki fark (mutlak).
    pub bounce_from_dip: f64,
    /// Bounce’un ATR cinsinden katı (kaç R).
    pub bounce_r: f64,
    /// Wyckoff Spring: dip barından sonra fiyat dip altına inip tekrar üstüne dönmüş mü (likidite avı).
    pub spring_detected: bool,
}

/// Bir sembol/timeframe için tepe analizi: tepe fiyatı, tepeden dönüş (düşüş), düşüş gücü.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeakAnalysis {
    /// Son tespit edilen tepe fiyatı (pivot high).
    pub peak_price: f64,
    pub peak_time: i64,
    pub peak_bar_index: usize,
    pub bars_since_peak: usize,
    /// Tepeden dönüş (düşüş) tespit edildi mi.
    pub reversal_detected: bool,
    /// Düşüş gücü 0–1.
    pub decline_strength: f64,
    /// Tepe – son kapanış farkı (mutlak).
    pub decline_from_peak: f64,
    pub decline_r: f64,
    /// Wyckoff Upthrust: tepe barından sonra fiyat tepe üstüne çıkıp tekrar altına dönmüş mü.
    pub upthrust_detected: bool,
}

/// Dip ve tepe analizini bir arada döndüren sonuç.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReversalAnalysis {
    pub dip: Option<DipAnalysis>,
    pub peak: Option<PeakAnalysis>,
}

/// `candles` üzerinde dip fiyatını ve dip bar bilgisini döndürür.
/// `pivot_len`: pivot low için sol/sağ bar sayısı (varsayılan 5).
pub fn get_dip_price_and_index(candles: &[Candle], pivot_len: usize) -> Option<(f64, i64, usize)> {
    let pl = pivot_len.max(1);
    if candles.len() < pl * 2 + 1 {
        return None;
    }
    let pivot_val = pivot_low(candles, pl)?;
    let idx = candles.len() - 1 - pl;
    let c = &candles[idx];
    Some((pivot_val, c.time, idx))
}

/// `candles` üzerinde tepe fiyatını ve tepe bar bilgisini döndürür.
pub fn get_peak_price_and_index(candles: &[Candle], pivot_len: usize) -> Option<(f64, i64, usize)> {
    let pl = pivot_len.max(1);
    if candles.len() < pl * 2 + 1 {
        return None;
    }
    let pivot_val = pivot_high(candles, pl)?;
    let idx = candles.len() - 1 - pl;
    let c = &candles[idx];
    Some((pivot_val, c.time, idx))
}

/// Dipten dönüş var mı: dip barından sonra fiyat dip + margin üzerinde ve son mum(lar) yükseliş yönünde.
fn is_reversal_from_dip(
    candles: &[Candle],
    dip_price: f64,
    dip_bar_index: usize,
    atr_val: f64,
) -> bool {
    if candles.is_empty() || dip_bar_index >= candles.len() {
        return false;
    }
    let last = candles.last().unwrap();
    let margin = atr_val * REVERSAL_MARGIN_ATR;
    if last.close <= dip_price + margin {
        return false;
    }
    let bars_after = candles.len() - 1 - dip_bar_index;
    if bars_after < 1 {
        return false;
    }
    let prev = candles.get(candles.len() - 2).unwrap();
    last.is_bullish() && last.close >= prev.close
}

/// Dipten dönüş gücü: 0–1. Bounce mesafesi (ATR katı), hacim oranı, son mum gövdesi.
fn reversal_strength_from_dip(
    candles: &[Candle],
    dip_price: f64,
    atr_val: f64,
) -> (f64, f64, f64) {
    if candles.is_empty() || atr_val <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let last = candles.last().unwrap();
    let bounce = (last.close - dip_price).max(0.0);
    let bounce_r = bounce / atr_val;
    let strength_atr = (bounce_r / STRENGTH_ATR_FULL).min(1.0);

    let vols: Vec<f64> = candles.iter().map(|c| c.volume).collect();
    let vol_avg = sma(&vols, 20.min(vols.len())).unwrap_or(last.volume);
    let vol_ratio = if vol_avg > 0.0 {
        (last.volume / vol_avg).min(2.0) / 2.0
    } else {
        0.5
    };

    let body = (last.close - last.open).max(0.0);
    let body_ratio = if atr_val > 0.0 {
        (body / atr_val).min(1.0)
    } else {
        0.0
    };

    let combined = 0.5 * strength_atr + 0.3 * vol_ratio + 0.2 * body_ratio;
    (combined.min(1.0), bounce, bounce_r)
}

/// Wyckoff Spring: dip barından sonra herhangi bir barın low'u dip altına inmiş, ardından en fazla SPRING_RECOVERY_BARS içinde close dip üstüne dönmüş mü.
fn detect_spring(candles: &[Candle], dip_price: f64, dip_bar_index: usize) -> bool {
    if dip_bar_index + 1 >= candles.len() {
        return false;
    }
    for j in (dip_bar_index + 1)..candles.len() {
        if candles[j].low < dip_price {
            let end = (j + SPRING_RECOVERY_BARS).min(candles.len());
            for k in (j + 1)..end {
                if candles[k].close > dip_price {
                    return true;
                }
            }
            break;
        }
    }
    false
}

/// Wyckoff Upthrust: tepe barından sonra herhangi bir barın high'ı tepe üstüne çıkmış, ardından en fazla SPRING_RECOVERY_BARS içinde close tepe altına dönmüş mü.
fn detect_upthrust(candles: &[Candle], peak_price: f64, peak_bar_index: usize) -> bool {
    if peak_bar_index + 1 >= candles.len() {
        return false;
    }
    for j in (peak_bar_index + 1)..candles.len() {
        if candles[j].high > peak_price {
            let end = (j + SPRING_RECOVERY_BARS).min(candles.len());
            for k in (j + 1)..end {
                if candles[k].close < peak_price {
                    return true;
                }
            }
            break;
        }
    }
    false
}

/// Tepeden dönüş (düşüş) var mı.
fn is_reversal_from_peak(
    candles: &[Candle],
    peak_price: f64,
    peak_bar_index: usize,
    atr_val: f64,
) -> bool {
    if candles.is_empty() || peak_bar_index >= candles.len() {
        return false;
    }
    let last = candles.last().unwrap();
    let margin = atr_val * REVERSAL_MARGIN_ATR_DOWN;
    if last.close >= peak_price - margin {
        return false;
    }
    let bars_after = candles.len() - 1 - peak_bar_index;
    if bars_after < 1 {
        return false;
    }
    let prev = candles.get(candles.len() - 2).unwrap();
    last.is_bearish() && last.close <= prev.close
}

/// Tepeden düşüş gücü: 0–1.
fn decline_strength_from_peak(
    candles: &[Candle],
    peak_price: f64,
    atr_val: f64,
) -> (f64, f64, f64) {
    if candles.is_empty() || atr_val <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let last = candles.last().unwrap();
    let decline = (peak_price - last.close).max(0.0);
    let decline_r = decline / atr_val;
    let strength_atr = (decline_r / STRENGTH_ATR_FULL).min(1.0);

    let vols: Vec<f64> = candles.iter().map(|c| c.volume).collect();
    let vol_avg = sma(&vols, 20.min(vols.len())).unwrap_or(last.volume);
    let vol_ratio = if vol_avg > 0.0 {
        (last.volume / vol_avg).min(2.0) / 2.0
    } else {
        0.5
    };

    let body = (last.open - last.close).max(0.0);
    let body_ratio = if atr_val > 0.0 {
        (body / atr_val).min(1.0)
    } else {
        0.0
    };

    let combined = 0.5 * strength_atr + 0.3 * vol_ratio + 0.2 * body_ratio;
    (combined.min(1.0), decline, decline_r)
}

/// Tek timeframe için dip ve tepe analizini hesaplar.
pub fn compute_reversal_analysis(
    candles: &[Candle],
    pivot_len: Option<usize>,
) -> ReversalAnalysis {
    let pl = pivot_len.unwrap_or(DEFAULT_PIVOT_LEN);
    let atr_val = atr(candles, ATR_PERIOD).unwrap_or_else(|| {
        candles
            .last()
            .map(|c| (c.high - c.low).max(1e-6))
            .unwrap_or(1.0)
    });

    let mut dip = None;
    if let Some((dip_price, dip_time, dip_bar_index)) = get_dip_price_and_index(candles, pl) {
        let bars_since_dip = candles.len().saturating_sub(dip_bar_index + 1);
        let reversal_detected = is_reversal_from_dip(candles, dip_price, dip_bar_index, atr_val);
        let (reversal_strength, bounce_from_dip, bounce_r) =
            reversal_strength_from_dip(candles, dip_price, atr_val);

        let spring_detected = detect_spring(candles, dip_price, dip_bar_index);
        dip = Some(DipAnalysis {
            dip_price,
            dip_time,
            dip_bar_index,
            bars_since_dip,
            reversal_detected,
            reversal_strength,
            bounce_from_dip,
            bounce_r,
            spring_detected,
        });
    }

    let mut peak = None;
    if let Some((peak_price, peak_time, peak_bar_index)) = get_peak_price_and_index(candles, pl) {
        let bars_since_peak = candles.len().saturating_sub(peak_bar_index + 1);
        let reversal_detected = is_reversal_from_peak(candles, peak_price, peak_bar_index, atr_val);
        let (decline_strength, decline_from_peak, decline_r) =
            decline_strength_from_peak(candles, peak_price, atr_val);

        let upthrust_detected = detect_upthrust(candles, peak_price, peak_bar_index);
        peak = Some(PeakAnalysis {
            peak_price,
            peak_time,
            peak_bar_index,
            bars_since_peak,
            reversal_detected,
            decline_strength,
            decline_from_peak,
            decline_r,
            upthrust_detected,
        });
    }

    ReversalAnalysis { dip, peak }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candle;

    fn candle(t: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Candle {
        Candle {
            time: t,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: v,
        }
    }

    #[test]
    fn test_get_dip_peak() {
        let mut candles = Vec::new();
        for i in 0..15 {
            let c = if i == 9 {
                candle(i * 1000, 100.0, 101.0, 98.0, 99.0, 1000.0)
            } else {
                candle(i * 1000, 99.0, 102.0, 99.5, 100.0, 1000.0)
            };
            candles.push(c);
        }
        let (price, time, idx) = get_dip_price_and_index(&candles, 5).unwrap();
        assert_eq!(idx, 9);
        assert!((price - 98.0).abs() < 1e-6);
        assert_eq!(time, 9000);
    }
}
