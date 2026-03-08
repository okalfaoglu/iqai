//! Chart annotations - CHoCH, BOS, liquidity, support/resistance, CVD, Elliott, Impulse

use iqai_core::{
    config::Config,
    elliott::{validate_diagonal, validate_impulse},
    impulse_detector::detect_impulse,
    indicators::{pivot_high, pivot_low, rsi, sma},
    types::Candle,
};

#[derive(serde::Serialize)]
pub struct ChartAnnotations {
    pub choch: Vec<ChochEvent>,
    pub bos: Vec<BosEvent>,
    pub liquidity: Vec<LiquidityEvent>,
    pub market_profile: Vec<MarketProfileEvent>,
    pub divergence: Vec<DivergenceEvent>,
    pub cvd: f64,
    pub support_line: Option<Line>,
    pub resistance_line: Option<Line>,
    pub elliott: ElliottAnnotations,
}

/// Elliott Wave çizim verileri – swing bazlı dalga bacakları ve seviyeler
#[derive(serde::Serialize, Default)]
pub struct ElliottAnnotations {
    /// Impulse/zigzag dalga bacakları: (time, price) çizgiler
    pub wave_legs: Vec<ElliottWaveLeg>,
    /// Fibonacci seviye çizgileri (yatay)
    pub fibo_levels: Vec<FiboLevel>,
    /// Mevcut formasyon adı (Impulse, Zigzag, vb.)
    pub formation: String,
    /// Formasyon türü: "Motif (İtki)" veya "Düzeltme"
    pub formation_type: String,
    /// Dalga noktaları: zaman, fiyat, label (0,1,2,3,4,5 veya A,B,C)
    pub wave_points: Vec<ElliottWavePoint>,
    /// W5 hedefleri (Impulse): (W1=W5, 0.618×(0-3), W4 inverse 123.6%)
    pub w5_targets: Option<(f64, f64, f64)>,
    /// Impulse tespit (3 aşama: CHoCH, W2 validasyon, BOS onay)
    pub impulse_state: Option<ImpulseState>,
    /// Kurallar geçerli mi (W2<=W0 iptal, W3 en kısa değil, W4-W1 örtüşmez)
    pub validation_ok: Option<bool>,
    pub validation_msg: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ImpulseState {
    pub stage: String,
    pub message: String,
    pub is_bullish: bool,
    pub setup_w3: Option<serde_json::Value>,
    pub setup_w5: Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
pub struct ElliottWavePoint {
    pub time: i64,
    pub price: f64,
    pub label: String,
}

#[derive(serde::Serialize)]
pub struct ElliottWaveLeg {
    pub time1: i64,
    pub price1: f64,
    pub time2: i64,
    pub price2: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct FiboLevel {
    pub time1: i64,
    pub time2: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct ChochEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct BosEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct LiquidityEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
}

#[derive(serde::Serialize)]
pub struct MarketProfileEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct DivergenceEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct Line {
    pub time1: i64,
    pub price1: f64,
    pub time2: i64,
    pub price2: f64,
    pub time3: i64, // extend to right (last candle)
    pub price3: f64,
    pub color: String,
}

pub fn compute_annotations(candles: &[Candle], config: &Config) -> ChartAnnotations {
    let pl = config.pivot_length as usize;
    let mut choch = Vec::new();
    let mut bos = Vec::new();
    let mut liquidity = Vec::new();
    let mut market_profile = Vec::new();
    let mut divergence = Vec::new();
    let mut raw_cvd = 0.0f64;

    if candles.len() < pl * 2 + 20 {
        return ChartAnnotations {
            choch,
            bos,
            liquidity,
            market_profile,
            divergence,
            cvd: 0.0,
            support_line: None,
            resistance_line: None,
            elliott: ElliottAnnotations::default(),
        };
    }

    let mut last_high = f64::NEG_INFINITY;
    let mut last_low = f64::INFINITY;
    let mut recent_buy_vol = 0.0f64;
    let mut recent_sell_vol = 0.0f64;
    let vol_avg = sma(
        &candles.iter().map(|c| c.volume).collect::<Vec<_>>(),
        config.volume_long_period as usize,
    )
    .unwrap_or(0.0);

    let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
    let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

    for i in (pl * 2 + 1)..candles.len() {
        let sub = &candles[..=i];
        let c = &candles[i];
        let prev_c = &candles[i - 1];

        // Pivot update
        if let Some(ph) = pivot_high(sub, pl) {
            last_high = ph;
        }
        if let Some(pl_val) = pivot_low(sub, pl) {
            last_low = pl_val;
        }

        // CVD
        let delta = if c.close > prev_c.close {
            c.volume
        } else if c.close < prev_c.close {
            -c.volume
        } else {
            0.0
        };
        raw_cvd += delta;

        if c.is_bullish() {
            recent_buy_vol = sma(
                &candles[i.saturating_sub(20)..=i]
                    .iter()
                    .map(|x| x.volume)
                    .collect::<Vec<_>>(),
                20,
            )
            .unwrap_or(c.volume);
        } else if c.is_bearish() {
            recent_sell_vol = sma(
                &candles[i.saturating_sub(20)..=i]
                    .iter()
                    .map(|x| x.volume)
                    .collect::<Vec<_>>(),
                20,
            )
            .unwrap_or(c.volume);
        }

        // CHoCH
        if prev_c.low >= last_high && c.low < last_high && c.is_bearish() {
            choch.push(ChochEvent {
                time: c.time / 1000,
                price: last_high,
                label: "CHoCH".to_string(),
                color: "#00E5FF".to_string(),
            });
        }
        if prev_c.high <= last_low && c.high > last_low && c.is_bullish() {
            choch.push(ChochEvent {
                time: c.time / 1000,
                price: last_low,
                label: "CHoCH".to_string(),
                color: "#76FF03".to_string(),
            });
        }

        // BOS - need prev pivot
        let prev_last_low = if i >= 3 {
            pivot_low(&candles[..i], pl).unwrap_or(last_low)
        } else {
            last_low
        };
        let prev_last_high = if i >= 3 {
            pivot_high(&candles[..i], pl).unwrap_or(last_high)
        } else {
            last_high
        };
        if prev_c.low >= prev_last_low && c.low < prev_last_low && c.is_bearish() {
            bos.push(BosEvent {
                time: c.time / 1000,
                price: prev_last_low,
                label: "BOS".to_string(),
                color: "#E040FB".to_string(),
            });
        }
        if prev_c.high <= prev_last_high && c.high > prev_last_high && c.is_bullish() {
            bos.push(BosEvent {
                time: c.time / 1000,
                price: prev_last_high,
                label: "BOS".to_string(),
                color: "#00BFA5".to_string(),
            });
        }

        // Liquidity zones
        if config.enable_liquidity_zones && i >= 20 {
            let lookback = 20;
            let recent_high = highs[i.saturating_sub(lookback)..=i]
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            let recent_low = lows[i.saturating_sub(lookback)..=i]
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min);
            if (c.high - recent_high).abs() / recent_high < 0.0005 {
                liquidity.push(LiquidityEvent {
                    time: c.time / 1000,
                    price: c.high,
                    label: "💧 LIQ".to_string(),
                });
            }
            if (c.low - recent_low).abs() / recent_low < 0.0005 {
                liquidity.push(LiquidityEvent {
                    time: c.time / 1000,
                    price: c.low,
                    label: "💧 LIQ".to_string(),
                });
            }
        }

        // Market profile
        if config.enable_market_profile && i >= 20 {
            let vol_ratio = if recent_buy_vol + recent_sell_vol > 0.0 {
                recent_buy_vol / (recent_buy_vol + recent_sell_vol)
            } else {
                0.5
            };
            if vol_ratio > 0.65 && c.volume > vol_avg * 1.5 {
                market_profile.push(MarketProfileEvent {
                    time: c.time / 1000,
                    price: c.low,
                    label: "🔥 BUY".to_string(),
                    color: "#00D9FF".to_string(),
                });
            }
            if vol_ratio < 0.35 && c.volume > vol_avg * 1.5 {
                market_profile.push(MarketProfileEvent {
                    time: c.time / 1000,
                    price: c.high,
                    label: "🔥 SELL".to_string(),
                    color: "#FF006E".to_string(),
                });
            }
        }

        // Divergence
        if config.enable_divergence_scanner && i >= 14 {
            let rsi_cur = rsi(&closes[..=i], 14).unwrap_or(50.0);
            if i >= 11 {
                let rsi_5 = rsi(&closes[..=(i - 5)], 14).unwrap_or(50.0);
                let rsi_10 = rsi(&closes[..=(i - 10)], 14).unwrap_or(50.0);
                let price_ll = c.low < lows[i - 5] && lows[i - 5] < lows[i - 10];
                let rsi_hl = rsi_cur > rsi_5 && rsi_5 > rsi_10;
                if price_ll && rsi_hl && rsi_cur < 40.0 {
                    divergence.push(DivergenceEvent {
                        time: c.time / 1000,
                        price: c.low,
                        label: "⚡ BULL".to_string(),
                        color: "#00F5FF".to_string(),
                    });
                }
                let price_hh = c.high > highs[i - 5] && highs[i - 5] > highs[i - 10];
                let rsi_lh = rsi_cur < rsi_5 && rsi_5 < rsi_10;
                if price_hh && rsi_lh && rsi_cur > 60.0 {
                    divergence.push(DivergenceEvent {
                        time: c.time / 1000,
                        price: c.high,
                        label: "⚡ BEAR".to_string(),
                        color: "#C77DFF".to_string(),
                    });
                }
            }
        }
    }

    // Support/Resistance lines (Pine script logic)
    let short_p = config.short_trend_period as usize;
    let long_p = config.long_trend_period as usize;
    let n = candles.len();
    let mut support_line = None;
    let mut resistance_line = None;

    if n >= long_p {
        let mut lowest_y2 = f64::INFINITY;
        let mut lowest_x2 = 0usize;
        let mut highest_y2 = f64::NEG_INFINITY;
        let mut highest_x2 = 0usize;
        for i in 1..short_p.min(n) {
            if lows[n - 1 - i] < lowest_y2 {
                lowest_y2 = lows[n - 1 - i];
                lowest_x2 = i;
            }
            if highs[n - 1 - i] > highest_y2 {
                highest_y2 = highs[n - 1 - i];
                highest_x2 = i;
            }
        }
        let mut lowest_y1 = f64::INFINITY;
        let mut lowest_x1 = 0usize;
        let mut highest_y1 = f64::NEG_INFINITY;
        let mut highest_x1 = 0usize;
        for j in (short_p + 1)..long_p.min(n) {
            if lows[n - 1 - j] < lowest_y1 {
                lowest_y1 = lows[n - 1 - j];
                lowest_x1 = j;
            }
            if highs[n - 1 - j] > highest_y1 {
                highest_y1 = highs[n - 1 - j];
                highest_x1 = j;
            }
        }
        let last_time = candles[n - 1].time / 1000;
        if lowest_x1 > 0 && lowest_x2 > 0 {
            let dt = candles[n - 1 - lowest_x2].time - candles[n - 1 - lowest_x1].time;
            let slope = if dt != 0 {
                (lowest_y2 - lowest_y1) as f64 / dt as f64 * 1000.0
            } else {
                0.0
            };
            let price3 = lowest_y2 + slope * (last_time - candles[n - 1 - lowest_x2].time / 1000) as f64;
            support_line = Some(Line {
                time1: candles[n - 1 - lowest_x1].time / 1000,
                price1: lowest_y1,
                time2: candles[n - 1 - lowest_x2].time / 1000,
                price2: lowest_y2,
                time3: last_time,
                price3,
                color: "#00E676".to_string(),
            });
        }
        if highest_x1 > 0 && highest_x2 > 0 {
            let dt = candles[n - 1 - highest_x2].time - candles[n - 1 - highest_x1].time;
            let slope = if dt != 0 {
                (highest_y2 - highest_y1) as f64 / dt as f64 * 1000.0
            } else {
                0.0
            };
            let price3 = highest_y2 + slope * (last_time - candles[n - 1 - highest_x2].time / 1000) as f64;
            resistance_line = Some(Line {
                time1: candles[n - 1 - highest_x1].time / 1000,
                price1: highest_y1,
                time2: candles[n - 1 - highest_x2].time / 1000,
                price2: highest_y2,
                time3: last_time,
                price3,
                color: "#FF1744".to_string(),
            });
        }
    }

    let elliott = compute_elliott_annotations(candles, config);

    ChartAnnotations {
        choch,
        bos,
        liquidity,
        market_profile,
        divergence,
        cvd: raw_cvd,
        support_line,
        resistance_line,
        elliott,
    }
}

/// Pivot bazlı swing noktalarından Elliott dalga bacaklarını türet
/// EWT kuralları: W2<=W0 iptal, W3 en kısa olamaz, W4-W1 örtüşmez
fn compute_elliott_annotations(candles: &[Candle], config: &Config) -> ElliottAnnotations {
    let pivot_len = config.pivot_length as usize;
    let mut wave_legs = Vec::new();
    let mut fibo_levels = Vec::new();
    let mut formation = "—".to_string();
    let mut formation_type = "—".to_string();
    let mut wave_points = Vec::new();
    let mut w5_targets = None;
    let mut validation_ok = None;
    let mut validation_msg = None;

    if candles.len() < pivot_len * 4 + 2 {
        return ElliottAnnotations {
            wave_legs,
            fibo_levels,
            formation,
            formation_type,
            wave_points,
            w5_targets: None,
            impulse_state: None,
            validation_ok: None,
            validation_msg: None,
        };
    }

    let imp = detect_impulse(candles, config);
    let impulse_state = Some(ImpulseState {
        stage: format!("{:?}", imp.stage),
        message: imp.message.clone(),
        is_bullish: imp.is_bullish,
        setup_w3: imp.setup_w3.as_ref().map(|s| {
            serde_json::json!({
                "entry": s.entry,
                "sl": s.stop_loss,
                "tp1": s.tp1,
                "tp2": s.tp2,
                "is_long": s.is_long
            })
        }),
        setup_w5: imp.setup_w5.as_ref().map(|s| {
            serde_json::json!({
                "entry": s.entry,
                "sl": s.stop_loss,
                "tp": s.tp,
                "tp_alt": s.tp_alternate,
                "is_long": s.is_long
            })
        }),
    });

    // Swing high/low noktalarını sırayla topla (alternating)
    let mut swings: Vec<(i64, f64, bool)> = Vec::new();
    let mut last_was_high = Option::<bool>::None;
    for i in (pivot_len * 2 + 1)..(candles.len().saturating_sub(pivot_len)) {
        let sub = &candles[..=i + pivot_len];
        let pivot_idx = sub.len() - 1 - pivot_len;
        let t = candles[pivot_idx].time;

        if let Some(ph) = pivot_high(sub, pivot_len) {
            if last_was_high != Some(true) {
                swings.push((t, ph, true));
                last_was_high = Some(true);
            }
        }
        if let Some(pl_val) = pivot_low(sub, pivot_len) {
            if last_was_high != Some(false) {
                swings.push((t, pl_val, false));
                last_was_high = Some(false);
            }
        }
    }

    // Elliott kuralları: Bullish W0=low,W1=high,W2=low,W3=high,W4=low
    // Bearish W0=high,W1=low,W2=high,W3=low,W4=high
    // Hem bullish hem bearish deneyerek geçerli impulse veren pencere seç
    let (recent, is_bullish): (Vec<_>, bool) = {
        let take = swings.len().min(9);
        let base_start = swings.len().saturating_sub(take);
        let base: Vec<_> = swings[base_start..].to_vec();
        if base.len() < 5 {
            (base, imp.is_bullish)
        } else {
            let mut valid_window: Option<(Vec<_>, bool)> = None;
            let mut fallback: Option<(Vec<_>, bool)> = None;
            for is_bull in [imp.is_bullish, !imp.is_bullish] {
                let need_first_high = !is_bull;
                for s in (0..=base.len().saturating_sub(5)).rev() {
                    let w = &base[s..s + 5];
                    let first_high = w[0].2;
                    if first_high != need_first_high || w[1].2 == first_high || w[2].2 != first_high || w[3].2 == first_high || w[4].2 != first_high {
                        continue;
                    }
                    let (p0, p1, p2, p3, p4) = (w[0].1, w[1].1, w[2].1, w[3].1, w[4].1);
                    let (w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext) = if is_bull {
                        (p0, p1, p0, p2, p3, p4)
                    } else {
                        (p0, p0, p1, p2, p3, p4)
                    };
                    let val = validate_impulse(w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext, is_bull);
                    if fallback.is_none() {
                        fallback = Some((w.to_vec(), is_bull));
                    }
                    if val.formation_valid {
                        valid_window = Some((w.to_vec(), is_bull));
                        break;
                    }
                }
                if valid_window.is_some() {
                    break;
                }
            }
            valid_window
                .or(fallback)
                .unwrap_or_else(|| (base[base.len().saturating_sub(5)..].to_vec(), imp.is_bullish))
        }
    };

    if recent.len() >= 3 {
        let (t0, p0, _) = recent[0];
        let (t1, p1, _) = recent[1];
        let (t2, p2, _) = recent[2];

        wave_points.push(ElliottWavePoint { time: t0 / 1000, price: p0, label: "0".to_string() });
        wave_points.push(ElliottWavePoint { time: t1 / 1000, price: p1, label: "1".to_string() });
        wave_points.push(ElliottWavePoint { time: t2 / 1000, price: p2, label: "2".to_string() });
        if recent.len() < 5 {
            formation = "Impulse (1-2)".to_string();
            formation_type = "Motif (İtki)".to_string();
        }

        wave_legs.push(ElliottWaveLeg {
            time1: t0 / 1000,
            price1: p0,
            time2: t1 / 1000,
            price2: p1,
            label: "1".to_string(),
            color: "#00E5FF".to_string(),
        });
        wave_legs.push(ElliottWaveLeg {
            time1: t1 / 1000,
            price1: p1,
            time2: t2 / 1000,
            price2: p2,
            label: "2".to_string(),
            color: "#00BFA5".to_string(),
        });

        if recent.len() >= 5 {
            let (t3, p3, _) = recent[3];
            let (t4, p4, _) = recent[4];
            wave_points.push(ElliottWavePoint { time: t3 / 1000, price: p3, label: "3".to_string() });
            wave_points.push(ElliottWavePoint { time: t4 / 1000, price: p4, label: "4".to_string() });

            wave_legs.push(ElliottWaveLeg {
                time1: t2 / 1000,
                price1: p2,
                time2: t3 / 1000,
                price2: p3,
                label: "3".to_string(),
                color: "#00E5FF".to_string(),
            });
            wave_legs.push(ElliottWaveLeg {
                time1: t3 / 1000,
                price1: p3,
                time2: t4 / 1000,
                price2: p4,
                label: "4".to_string(),
                color: "#00BFA5".to_string(),
            });

            // EWT kuralları: W2<=W0 iptal, W3 en kısa olamaz, W4-W1 örtüşmez
            let bullish = is_bullish;
            let (w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext) = if bullish {
                (p0, p1, p0, p2, p3, p4)
            } else {
                (p0, p0, p1, p2, p3, p4)
            };
            let val = validate_impulse(w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext, bullish);
            let diag = validate_diagonal(w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext, bullish);
            // W4-W1 örtüşmesi varsa Impulse geçersiz; Diagonal (ED/CD) alternatifi değerlendir
            let (validation_ok_val, validation_msg_val, formation_label, formation_type_label) =
                if val.formation_valid {
                    (Some(true), Some("Kurallar geçerli".to_string()), "Impulse".to_string(), "Motif (İtki)".to_string())
                } else if !val.w4_valid && val.w2_valid && val.w3_valid && diag.formation_valid {
                    // Sadece W4-W1 örtüşme ihlali; Diagonal geçerli
                    (Some(true), Some("Diagonal: W4-W1 örtüşmesi kabul (ED/CD)".to_string()), "Diagonal".to_string(), "Motif (Bitiş Diyagonal)".to_string())
                } else {
                    let mut parts = vec![];
                    if !val.w2_valid {
                        parts.push("W2<=W0");
                    }
                    if !val.w3_valid {
                        parts.push("W3 en kısa");
                    }
                    if !val.w4_valid {
                        parts.push("W4-W1 örtüşme");
                    }
                    if !val.no_triple_extension_valid {
                        parts.push("Triple extension");
                    }
                    let msg = format!("İhlal: {}", parts.join(", "));
                    (Some(false), Some(msg), "Impulse".to_string(), "Motif (İtki)".to_string())
                };
            validation_ok = validation_ok_val;
            validation_msg = validation_msg_val;
            formation = formation_label;
            formation_type = formation_type_label;

            // W5 tahminleri: W1=W5, 0.618×(W0→W3), W4 inverse 123.6% (EWF)
            let w1_len = (p1 - p0).abs();
            let w1_3_len = (p3 - p0).abs();
            let w4_len = (p3 - p4).abs();
            let w5_eq = if bullish { p4 + w1_len } else { p4 - w1_len };
            let w5_618 = if bullish { p4 + 0.618 * w1_3_len } else { p4 - 0.618 * w1_3_len };
            let w5_inv = if bullish { p4 + 1.236 * w4_len } else { p4 - 1.236 * w4_len };
            w5_targets = Some((w5_eq, w5_618, w5_inv));

            let low = [p0, p1, p2].into_iter().fold(f64::INFINITY, f64::min);
            let high = [p0, p1, p2].into_iter().fold(f64::NEG_INFINITY, f64::max);
            let range = high - low;
            if range > 0.0 {
                let last_time = candles.last().map(|c| c.time / 1000).unwrap_or(t4 / 1000);
                for (ratio, label, color) in [
                    (0.146, "14.6%", "#66BB6A"),
                    (0.236, "23.6%", "#4CAF50"),
                    (0.382, "38.2%", "#8BC34A"),
                    (0.5, "50%", "#FFEB3B"),
                    (0.618, "61.8%", "#FF9800"),
                ] {
                    let price = low + range * ratio;
                    fibo_levels.push(FiboLevel {
                        time1: t0 / 1000,
                        time2: last_time,
                        price,
                        label: label.to_string(),
                        color: color.to_string(),
                    });
                }
            }
        }
    }

    ElliottAnnotations {
        wave_legs,
        fibo_levels,
        formation,
        formation_type,
        wave_points,
        w5_targets,
        impulse_state,
        validation_ok,
        validation_msg,
    }
}
