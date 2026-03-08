//! Impulse (İtki) formasyonu tespiti – 3 aşamalı event-driven algoritma
//!
//! Aşama 1: Erken Uyarı (W1 Aday)
//! Aşama 2: W2 Validasyonu
//! Aşama 3: Kesin Onay (W3 Başlangıcı / BOS)

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::elliott::{compute_setup_w3, compute_setup_w5, SetupW3, SetupW5};
use crate::indicators::{pivot_high, pivot_low, rsi, sma};
use crate::types::Candle;

/// Impulse tespit aşaması
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpulseStage {
    /// İzlemede – henüz W0 veya CHoCH yok
    Watching,
    /// W1 Aday – CHoCH tetiklendi
    W1Candidate,
    /// W2 validasyonu – geri çekilme
    W2Validating,
    /// Impulse onaylandı – BOS veya W3 başladı
    ImpulseConfirmed,
    /// Geçersiz – W2 <= W0
    Invalidated,
}

/// Impulse tespit sonucu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpulseDetectorState {
    pub stage: ImpulseStage,
    pub is_bullish: bool,
    pub w0_price: Option<f64>,
    pub w0_time: Option<i64>,
    pub w1_high: Option<f64>,
    pub w1_time: Option<i64>,
    pub w2_low: Option<f64>,
    pub w2_time: Option<i64>,
    pub last_swing_high: Option<f64>,
    pub last_swing_low: Option<f64>,
    pub setup_w3: Option<SetupW3>,
    pub setup_w5: Option<SetupW5>,
    pub message: String,
}

impl Default for ImpulseDetectorState {
    fn default() -> Self {
        Self {
            stage: ImpulseStage::Watching,
            is_bullish: true,
            w0_price: None,
            w0_time: None,
            w1_high: None,
            w1_time: None,
            w2_low: None,
            w2_time: None,
            last_swing_high: None,
            last_swing_low: None,
            setup_w3: None,
            setup_w5: None,
            message: "—".to_string(),
        }
    }
}

/// 3 aşamalı Impulse tespiti
pub fn detect_impulse(candles: &[Candle], config: &Config) -> ImpulseDetectorState {
    let pl = config.pivot_length as usize;
    if candles.len() < pl * 2 + 30 {
        return ImpulseDetectorState::default();
    }

    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let volumes: Vec<f64> = candles.iter().map(|c| c.volume).collect();

    let mut state = ImpulseDetectorState::default();
    let mut last_high = f64::NEG_INFINITY;
    let mut last_low = f64::INFINITY;
    let mut prev_pivot_high = f64::NEG_INFINITY;
    let mut prev_pivot_low = f64::INFINITY;

    for i in (pl * 2 + 1)..candles.len() {
        let sub = &candles[..=i];
        let c = &candles[i];
        let prev_c = &candles[i - 1];
        let t = c.time / 1000;

        let ph = pivot_high(sub, pl);
        let pl_val = pivot_low(sub, pl);

        if let Some(ph_val) = ph {
            last_high = ph_val;
        }
        if let Some(plv) = pl_val {
            last_low = plv;
        }

        let prev_last_high = if i >= pl + 2 {
            pivot_high(&candles[..i], pl).unwrap_or(prev_pivot_high)
        } else {
            prev_pivot_high
        };
        let prev_last_low = if i >= pl + 2 {
            pivot_low(&candles[..i], pl).unwrap_or(prev_pivot_low)
        } else {
            prev_pivot_low
        };

        match state.stage {
            ImpulseStage::Watching | ImpulseStage::Invalidated => {
                if state.stage == ImpulseStage::Invalidated {
                    state.stage = ImpulseStage::Watching;
                }
                if let Some(low) = pl_val {
                    state.last_swing_low = Some(low);
                    state.w0_price = Some(low);
                    state.w0_time = Some(t);
                }
                if prev_c.high <= prev_last_low && c.high > prev_last_low && c.is_bullish() {
                    let start = i.saturating_sub(19);
                    let vol_avg = if i >= 19 {
                        sma(&volumes[start..=i], 20).unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    let vol_ok = vol_avg > 0.0 && c.volume >= vol_avg * 1.2;
                    let div_ok = if i >= 14 {
                        let rsi_cur = rsi(&closes[..=i], 14).unwrap_or(50.0);
                        rsi_cur < 45.0
                    } else {
                        true
                    };
                    if vol_ok || div_ok {
                        state.stage = ImpulseStage::W1Candidate;
                        state.is_bullish = true;
                        state.last_swing_low = Some(prev_last_low);
                        state.w0_price = Some(prev_last_low);
                        state.w0_time = Some(t);
                        state.w1_high = Some(c.high);
                        state.w1_time = Some(t);
                        state.message = "W1 Aday (CHoCH)".to_string();
                    }
                }
                if prev_c.low >= prev_last_high && c.low < prev_last_high && c.is_bearish() {
                    state.stage = ImpulseStage::W1Candidate;
                    state.is_bullish = false;
                    state.last_swing_high = Some(prev_last_high);
                    state.w0_price = Some(prev_last_high);
                    state.w0_time = Some(t);
                    state.w1_high = Some(c.low);
                    state.w1_time = Some(t);
                    state.message = "W1 Aday (CHoCH bearish)".to_string();
                }
            }
            ImpulseStage::W1Candidate => {
                if state.is_bullish {
                    state.w1_high = state.w1_high.map(|h| h.max(c.high)).or(Some(c.high));
                    if ph.is_some() {
                        state.w1_high = Some(last_high);
                        state.w1_time = Some(t);
                    }
                    if c.close < state.w1_high.unwrap_or(0.0) && c.is_bearish() {
                        state.stage = ImpulseStage::W2Validating;
                        state.w2_low = Some(c.low);
                        state.w2_time = Some(t);
                        state.message = "W2 Validasyon".to_string();
                    }
                } else {
                    state.w1_high = state.w1_high.map(|h| h.min(c.low)).or(Some(c.low));
                    if pl_val.is_some() {
                        state.w1_high = Some(last_low);
                        state.w1_time = Some(t);
                    }
                    if c.close > state.w1_high.unwrap_or(f64::INFINITY) && c.is_bullish() {
                        state.stage = ImpulseStage::W2Validating;
                        state.w2_low = Some(c.high);
                        state.w2_time = Some(t);
                        state.message = "W2 Validasyon".to_string();
                    }
                }
            }
            ImpulseStage::W2Validating => {
                let w0 = state.w0_price.unwrap_or(0.0);
                let w1 = state.w1_high.unwrap_or(0.0);

                if state.is_bullish {
                    state.w2_low = state.w2_low.map(|l| l.min(c.low)).or(Some(c.low));
                    let w2_low = state.w2_low.unwrap_or(c.low);
                    if w2_low <= w0 {
                        state.stage = ImpulseStage::Invalidated;
                        state.message = "İptal: W2 <= W0".to_string();
                        continue;
                    }
                    if prev_c.high <= w1 && c.high > w1 && c.is_bullish() {
                        state.stage = ImpulseStage::ImpulseConfirmed;
                        state.setup_w3 = Some(compute_setup_w3(w0, w1, w0, w2_low, true));
                        state.message = "Impulse Onay (BOS)".to_string();
                    }
                } else {
                    state.w2_low = state.w2_low.map(|l| l.max(c.high)).or(Some(c.high));
                    let w2_high = state.w2_low.unwrap_or(c.high);
                    if w2_high >= w0 {
                        state.stage = ImpulseStage::Invalidated;
                        state.message = "İptal: W2 >= W0".to_string();
                        continue;
                    }
                    if prev_c.low >= w1 && c.low < w1 && c.is_bearish() {
                        state.stage = ImpulseStage::ImpulseConfirmed;
                        state.setup_w3 = Some(compute_setup_w3(w0, w0, w1, w2_high, false));
                        state.message = "Impulse Onay (BOS bearish)".to_string();
                    }
                }
            }
            ImpulseStage::ImpulseConfirmed => {
                if state.setup_w5.is_none() && state.w1_high.is_some() && state.w2_low.is_some() {
                    let w1 = state.w1_high.unwrap();
                    let w0 = state.w0_price.unwrap_or(0.0);
                    let w2 = state.w2_low.unwrap();
                    let w1_lo = if state.is_bullish { w0 } else { w1 };
                    let w1_hi = if state.is_bullish { w1 } else { w0 };
                    let w3_est = if state.is_bullish {
                        w2 + (w1 - w0).abs() * 1.618
                    } else {
                        w2 - (w0 - w1).abs() * 1.618
                    };
                    let w4_est = if state.is_bullish {
                        w3_est - (w3_est - w2) * 0.382
                    } else {
                        w3_est + (w2 - w3_est) * 0.382
                    };
                    state.setup_w5 = Some(compute_setup_w5(
                        w1_hi,
                        w1_lo,
                        w3_est,
                        w2,
                        w4_est,
                        state.is_bullish,
                    ));
                }
            }
        }

        prev_pivot_high = last_high;
        prev_pivot_low = last_low;
    }

    state
}
