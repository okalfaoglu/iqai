//! Dip / tepe arama ve dönüş analizi.
//!
//! Mantık ve formüller `docs/Q_ANALIZ_DIP_TEPE_YONTEM.md` ile uyumludur.
//!
//! **Pipeline (Doc §16):**
//! 1. **Pivot tespiti (Doc §2)** – Fractal pivot low/high (TradingView uyumlu).
//! 2. **Dönüş tespiti** – Fiyat dip/tepe + ATR margin dışında ve son mum yönü uyumlu.
//! 3. **Dönüş gücü (Doc §14)** – Bounce/decline (ATR), hacim oranı, mum gövdesi → 0–1 skor.
//! 4. **Wyckoff (Doc §10)** – Spring (dip) / Upthrust (tepe): sahte kırılım sonrası geri dönüş.

use serde::{Deserialize, Serialize};

use crate::indicators::{atr, pivot_high, pivot_low, sma};
use crate::types::Candle;

// -----------------------------------------------------------------------------
// Sabitler (Doc §2, §10, §14)
// -----------------------------------------------------------------------------

/// Pivot penceresi: sol bar sayısı = sağ bar sayısı (varsayılan 5 → 5+1+5 = 11 bar).
/// Doc §2: config.pivot_length ile override edilir.
const DEFAULT_PIVOT_LEN: usize = 5;

/// ATR periyodu (dönüş margin ve güç hesabında).
const ATR_PERIOD: usize = 14;

/// Dipten dönüş: fiyat en az bu kadar ATR dip üzerinde olmalı. Doc §2.
const REVERSAL_MARGIN_ATR: f64 = 0.2;

/// Tepeden dönüş: fiyat en az bu kadar ATR tepe altında olmalı.
const REVERSAL_MARGIN_ATR_DOWN: f64 = 0.2;

/// Doc §14: Bu kadar ATR hareket = tam güç (1.0). Bounce/decline bu değere bölünür, cap 1.0.
const STRENGTH_ATR_FULL: f64 = 2.0;

/// Doc §10: Spring/Upthrust – dip altına (tepe üstüne) gittikten sonra en fazla bu bar içinde geri dönmeli.
const SPRING_RECOVERY_BARS: usize = 4;

/// Doc §14: Dönüş gücü ağırlıkları – strength_atr, vol_ratio, body_ratio.
const WEIGHT_STRENGTH_ATR: f64 = 0.5;
const WEIGHT_VOL_RATIO: f64 = 0.3;
const WEIGHT_BODY_RATIO: f64 = 0.2;

/// Hacim oranı için kullanılan SMA periyodu (son N bar ortalama hacim).
const VOLUME_MA_PERIOD: usize = 20;

// -----------------------------------------------------------------------------
// Çıktı yapıları
// -----------------------------------------------------------------------------

/// Tek timeframe için dip analizi (Doc §2, §14, §10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DipAnalysis {
    /// Son tespit edilen dip fiyatı (pivot low).
    pub dip_price: f64,
    pub dip_time: i64,
    pub dip_bar_index: usize,
    pub bars_since_dip: usize,
    /// Dipten dönüş: fiyat dip + margin üzerinde, son mum yükseliş.
    pub reversal_detected: bool,
    /// Doc §14: 0–1, reversal_strength = 0.5×strength_atr + 0.3×vol_ratio + 0.2×body_ratio.
    pub reversal_strength: f64,
    pub bounce_from_dip: f64,
    pub bounce_r: f64,
    /// Doc §10: Wyckoff Spring – dip altına inip en fazla SPRING_RECOVERY_BARS içinde tekrar üstüne dönmüş mü.
    pub spring_detected: bool,
}

/// Tek timeframe için tepe analizi (Doc §2, §14, §10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeakAnalysis {
    /// Son tespit edilen tepe fiyatı (pivot high).
    pub peak_price: f64,
    pub peak_time: i64,
    pub peak_bar_index: usize,
    pub bars_since_peak: usize,
    pub reversal_detected: bool,
    /// Doc §14: decline_strength, aynı formül (decline/ATR + vol + body).
    pub decline_strength: f64,
    pub decline_from_peak: f64,
    pub decline_r: f64,
    /// Doc §10: Wyckoff Upthrust.
    pub upthrust_detected: bool,
}

/// Dip ve tepe analizini birlikte döndüren sonuç.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReversalAnalysis {
    pub dip: Option<DipAnalysis>,
    pub peak: Option<PeakAnalysis>,
}

// -----------------------------------------------------------------------------
// §2 Dip / tepe matematiksel tespiti (Pivot)
// -----------------------------------------------------------------------------

/// Son geçerli pivot low (dip) fiyatını, zamanını ve bar indeksini döndürür.
///
/// Doc §2: Merkez barın `low` değeri sol ve sağdaki `pivot_len` barın low'larından
/// kesinlikle düşük olmalı (TradingView `ta.pivotlow` uyumlu). Pivot `indicators::pivot_low` ile hesaplanır.
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

/// Son geçerli pivot high (tepe) fiyatını, zamanını ve bar indeksini döndürür.
///
/// Doc §2: Merkez barın `high` değeri sol ve sağdaki `pivot_len` barın high'larından
/// kesinlikle yüksek olmalı (TradingView `ta.pivothigh` uyumlu).
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

// -----------------------------------------------------------------------------
// Dönüş tespiti (fiyat + margin + mum yönü)
// -----------------------------------------------------------------------------

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

// -----------------------------------------------------------------------------
// §14 Dönüş gücü (reversal_strength / decline_strength)
// -----------------------------------------------------------------------------

/// Doc §14: Dip için 0–1. Bounce/ATR (2 ATR = 1.0), son 20 bar hacim ort. ile son mum hacmi oranı,
/// son mum gövdesi/ATR. combined = 0.5×strength_atr + 0.3×vol_ratio + 0.2×body_ratio.
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
    let vol_avg = sma(&vols, VOLUME_MA_PERIOD.min(vols.len())).unwrap_or(last.volume);
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

    let combined = WEIGHT_STRENGTH_ATR * strength_atr
        + WEIGHT_VOL_RATIO * vol_ratio
        + WEIGHT_BODY_RATIO * body_ratio;
    (combined.min(1.0), bounce, bounce_r)
}

/// Doc §14: Tepe için düşüş gücü – decline/ATR + vol_ratio + bearish body.
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
    let vol_avg = sma(&vols, VOLUME_MA_PERIOD.min(vols.len())).unwrap_or(last.volume);
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

    let combined = WEIGHT_STRENGTH_ATR * strength_atr
        + WEIGHT_VOL_RATIO * vol_ratio
        + WEIGHT_BODY_RATIO * body_ratio;
    (combined.min(1.0), decline, decline_r)
}

// -----------------------------------------------------------------------------
// §10 Wyckoff Spring / Upthrust
// -----------------------------------------------------------------------------

/// Doc §10: Dip barından sonra fiyat dip altına inmiş, en fazla SPRING_RECOVERY_BARS içinde
/// close tekrar dip üstüne dönmüş mü (likidite avı).
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

/// Doc §10: Tepe barından sonra fiyat tepe üstüne çıkmış, en fazla SPRING_RECOVERY_BARS içinde
/// close tekrar tepe altına dönmüş mü.
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

// -----------------------------------------------------------------------------
// Ana dip/tepe arama ve analiz (Doc §16 pipeline)
// -----------------------------------------------------------------------------

/// Tek timeframe için dip ve tepe aramasını yapar; dönüş tespiti, dönüş gücü ve Spring/Upthrust hesaplanır.
///
/// Doc §16: OHLCV → Pivot Low/High → Reversal analizi (dip/tepe fiyatı, reversal_detected,
/// reversal_strength, spring/upthrust). `pivot_len` None ise DEFAULT_PIVOT_LEN (5) kullanılır.
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

    let dip = find_dip_analysis(candles, pl, atr_val);
    let peak = find_peak_analysis(candles, pl, atr_val);

    ReversalAnalysis { dip, peak }
}

fn find_dip_analysis(
    candles: &[Candle],
    pl: usize,
    atr_val: f64,
) -> Option<DipAnalysis> {
    let (dip_price, dip_time, dip_bar_index) = get_dip_price_and_index(candles, pl)?;
    let bars_since_dip = candles.len().saturating_sub(dip_bar_index + 1);
    let reversal_detected = is_reversal_from_dip(candles, dip_price, dip_bar_index, atr_val);
    let (reversal_strength, bounce_from_dip, bounce_r) =
        reversal_strength_from_dip(candles, dip_price, atr_val);
    let spring_detected = detect_spring(candles, dip_price, dip_bar_index);

    Some(DipAnalysis {
        dip_price,
        dip_time,
        dip_bar_index,
        bars_since_dip,
        reversal_detected,
        reversal_strength,
        bounce_from_dip,
        bounce_r,
        spring_detected,
    })
}

fn find_peak_analysis(
    candles: &[Candle],
    pl: usize,
    atr_val: f64,
) -> Option<PeakAnalysis> {
    let (peak_price, peak_time, peak_bar_index) = get_peak_price_and_index(candles, pl)?;
    let bars_since_peak = candles.len().saturating_sub(peak_bar_index + 1);
    let reversal_detected = is_reversal_from_peak(candles, peak_price, peak_bar_index, atr_val);
    let (decline_strength, decline_from_peak, decline_r) =
        decline_strength_from_peak(candles, peak_price, atr_val);
    let upthrust_detected = detect_upthrust(candles, peak_price, peak_bar_index);

    Some(PeakAnalysis {
        peak_price,
        peak_time,
        peak_bar_index,
        bars_since_peak,
        reversal_detected,
        decline_strength,
        decline_from_peak,
        decline_r,
        upthrust_detected,
    })
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
