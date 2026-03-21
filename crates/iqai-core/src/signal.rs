//! Smart Money Structure signal engine - 1:1 logic from Pine Script

use std::collections::HashMap;

use crate::config::Config;
use crate::indicators::*;
use crate::types::{
    Candle, PositionMetrics, ProtectSignal, QSetup, QRadarSignal, Signal, SignalType, Timeframe,
};

/// Multi-timeframe candle buffer for signal calculation
pub struct CandleBuffer {
    pub candles: HashMap<Timeframe, Vec<Candle>>,
}

impl CandleBuffer {
    pub fn new() -> Self {
        Self {
            candles: HashMap::new(),
        }
    }

    pub fn update(&mut self, tf: Timeframe, candles: Vec<Candle>) {
        self.candles.insert(tf, candles);
    }

    pub fn get(&self, tf: Timeframe) -> Option<&[Candle]> {
        self.candles.get(&tf).map(|v| v.as_slice())
    }
}

impl Default for CandleBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// State carried between bars (Pine Script var)
#[derive(Debug, Clone)]
struct SignalState {
    last_high: f64,
    last_low: f64,
    last_signal_bar: i64,
    last_signal: String,
    last_trend: i32,
    raw_cvd: f64,
    recent_buy_vol: f64,
    recent_sell_vol: f64,
    bar_index: i64,
}

impl Default for SignalState {
    fn default() -> Self {
        Self {
            last_high: f64::NEG_INFINITY,
            last_low: f64::INFINITY,
            last_signal_bar: -100,
            last_signal: "Neutral".to_string(),
            last_trend: 0,
            raw_cvd: 0.0,
            recent_buy_vol: 0.0,
            recent_sell_vol: 0.0,
            bar_index: 0,
        }
    }
}

/// Smart Money Structure signal engine
pub struct SignalEngine {
    config: Config,
    state: SignalState,
}

impl SignalEngine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: SignalState::default(),
        }
    }

    /// Process current chart timeframe candles and return signals
    pub fn process(&mut self, buffer: &CandleBuffer, chart_tf: Timeframe) -> Vec<Signal> {
        let candles = match buffer.get(chart_tf) {
            Some(c) if c.len() > (self.config.pivot_length * 2 + 20) as usize => c,
            _ => return vec![],
        };

        let mut signals = Vec::new();
        let cfg = &self.config;
        let len = candles.len();

        // Call these before mutable borrow of state
        let (higher_tf_trend, lower_tf_trend, restrict_tf_trend) = self.compute_trends(buffer);
        let system_conf = self.system_confidence(buffer);
        let trend_str = self.trend_strength(buffer);

        let state = &mut self.state;
        let c = &candles[len - 1];
        let prev_c = match candles.get(len - 2) {
            Some(p) => p,
            None => return signals,
        };

        // ATR & volatility
        let atr_val = atr(candles, 14).unwrap_or(c.high - c.low);
        let volatility_factor = atr_val / c.close;
        let momentum_threshold = cfg.momentum_threshold_base * (1.0 + volatility_factor * 2.0);
        let pre_momentum_factor =
            cfg.pre_momentum_factor_base * (1.0 - volatility_factor * 0.5);
        let _pre_momentum_threshold = momentum_threshold * pre_momentum_factor;

        // CVD
        let delta = if c.close > prev_c.close {
            c.volume
        } else if c.close < prev_c.close {
            -c.volume
        } else {
            0.0
        };
        state.raw_cvd += delta;

        if c.is_bullish() {
            state.recent_buy_vol = sma(
                &candles[len.saturating_sub(20)..]
                    .iter()
                    .map(|x| x.volume)
                    .collect::<Vec<_>>(),
                20,
            )
            .unwrap_or(c.volume);
        } else if c.is_bearish() {
            state.recent_sell_vol = sma(
                &candles[len.saturating_sub(20)..]
                    .iter()
                    .map(|x| x.volume)
                    .collect::<Vec<_>>(),
                20,
            )
            .unwrap_or(c.volume);
        }

        // Price change %
        let price_change = ((c.close - prev_c.close) / prev_c.close) * 100.0;

        // Pivot detection - scan recent bars for pivots
        for i in (cfg.pivot_length * 2 + 1) as usize..len {
            let sub = &candles[..=i];
            if let Some(ph) = pivot_high(sub, cfg.pivot_length as usize) {
                state.last_high = ph;
            }
            if let Some(pl) = pivot_low(sub, cfg.pivot_length as usize) {
                state.last_low = pl;
            }
        }

        let bullish_trend_ok = higher_tf_trend == 1;
        let bearish_trend_ok = higher_tf_trend == -1;
        let lower_tf_bullish = lower_tf_trend == 1;
        let lower_tf_bearish = lower_tf_trend == -1;
        let lower_tf_not_neutral = lower_tf_trend != 0;

        // Volume
        let _closes: Vec<f64> = candles.iter().map(|x| x.close).collect();
        let vol_avg = sma(&candles.iter().map(|x| x.volume).collect::<Vec<_>>(), cfg.volume_long_period as usize).unwrap_or(0.0);
        let vol_short = sma(&candles.iter().map(|x| x.volume).collect::<Vec<_>>(), cfg.volume_short_period as usize).unwrap_or(0.0);
        let vol_prev = candles.get(len - 2).map(|x| x.volume).unwrap_or(0.0);
        let vol_condition = c.volume > vol_avg && (vol_short - vol_prev) > 0.0;

        let highs: Vec<f64> = candles.iter().map(|x| x.high).collect();
        let lows: Vec<f64> = candles.iter().map(|x| x.low).collect();
        let _highest_breakout = highest(&highs, cfg.breakout_period as usize).unwrap_or(c.high);
        let _lowest_breakout = lowest(&lows, cfg.breakout_period as usize).unwrap_or(c.low);
        let prev_highest = highest(&highs[..highs.len().saturating_sub(1)], cfg.breakout_period as usize).unwrap_or(0.0);
        let prev_lowest = lowest(&lows[..lows.len().saturating_sub(1)], cfg.breakout_period as usize).unwrap_or(f64::INFINITY);

        // CHoCH: cross of structure level | BOS: break of prior structure
        let _choch_sell = prev_c.low >= state.last_high && c.low < state.last_high && c.is_bearish();
        let _choch_buy = prev_c.high <= state.last_low && c.high > state.last_low && c.is_bullish();
        let prev_last_low = candles
            .get(len.saturating_sub(3))
            .and_then(|_| {
                let sub = &candles[..len.saturating_sub(1)];
                pivot_low(sub, cfg.pivot_length as usize)
            })
            .unwrap_or(state.last_low);
        let prev_last_high = candles
            .get(len.saturating_sub(3))
            .and_then(|_| {
                let sub = &candles[..len.saturating_sub(1)];
                pivot_high(sub, cfg.pivot_length as usize)
            })
            .unwrap_or(state.last_high);
        let _bos_sell =
            prev_c.low >= prev_last_low && c.low < prev_last_low && c.is_bearish();
        let _bos_buy =
            prev_c.high <= prev_last_high && c.high > prev_last_high && c.is_bullish();

        let early_sell = !cfg.use_momentum_filter || price_change < -momentum_threshold;
        let early_buy = !cfg.use_momentum_filter || price_change > momentum_threshold;
        let sell_trend_ok = !cfg.use_trend_filter || bearish_trend_ok;
        let buy_trend_ok = !cfg.use_trend_filter || bullish_trend_ok;
        let sell_lower_ok = !cfg.use_lower_tf_filter || (!lower_tf_bullish && lower_tf_not_neutral);
        let buy_lower_ok = !cfg.use_lower_tf_filter || (!lower_tf_bearish && lower_tf_not_neutral);
        let sell_vol_ok = !cfg.use_volume_filter || vol_condition;
        let buy_vol_ok = !cfg.use_volume_filter || vol_condition;
        let sell_breakout_ok = !cfg.use_breakout_filter || c.close < prev_lowest;
        let buy_breakout_ok = !cfg.use_breakout_filter || c.close > prev_highest;

        let sell_allowed = !cfg.restrict_repeated_signals
            || (state.last_signal != "Sell"
                || (state.last_signal == "Sell"
                    && restrict_tf_trend != state.last_trend
                    && restrict_tf_trend != -1));
        let buy_allowed = !cfg.restrict_repeated_signals
            || (state.last_signal != "Buy"
                || (state.last_signal == "Buy"
                    && restrict_tf_trend != state.last_trend
                    && restrict_tf_trend != 1));

        let bar_idx = state.bar_index;
        let dist_ok = bar_idx - state.last_signal_bar >= cfg.min_signal_distance as i64;

        let sell_condition = early_sell && dist_ok && sell_trend_ok && sell_lower_ok && sell_vol_ok && sell_breakout_ok && sell_allowed;
        let buy_condition = early_buy && dist_ok && buy_trend_ok && buy_lower_ok && buy_vol_ok && buy_breakout_ok && buy_allowed;

        if sell_condition {
            state.last_signal = "Sell".to_string();
            state.last_signal_bar = bar_idx;
            state.last_trend = restrict_tf_trend;
            signals.push(Signal {
                signal_type: SignalType::Sell,
                price: c.close,
                timestamp: c.time,
                timeframe: chart_tf,
                take_profit: Some(c.low - cfg.tp_points as f64),
                stop_loss: Some(c.high + cfg.sl_points as f64),
                confidence: system_conf,
                trend_strength: trend_str,
                metadata: serde_json::json!({"cvd": state.raw_cvd}),
            });
        }

        if buy_condition {
            state.last_signal = "Buy".to_string();
            state.last_signal_bar = bar_idx;
            state.last_trend = restrict_tf_trend;
            signals.push(Signal {
                signal_type: SignalType::Buy,
                price: c.close,
                timestamp: c.time,
                timeframe: chart_tf,
                take_profit: Some(c.high + cfg.tp_points as f64),
                stop_loss: Some(c.low - cfg.sl_points as f64),
                confidence: system_conf,
                trend_strength: trend_str,
                metadata: serde_json::json!({"cvd": state.raw_cvd}),
            });
        }

        state.bar_index += 1;
        signals
    }

    fn compute_trends(&self, buffer: &CandleBuffer) -> (i32, i32, i32) {
        let mut ht = 0i32;
        let mut lt = 0i32;
        let mut rt = 0i32;
        for tf in [Timeframe::M1, Timeframe::M5, Timeframe::M15, Timeframe::M30, Timeframe::H1, Timeframe::H4, Timeframe::D1] {
            let t = self.trend_for_tf(buffer, tf);
            if tf == self.config.higher_tf {
                ht = t;
            }
            if tf == self.config.lower_tf {
                lt = t;
            }
            if tf == self.config.restrict_trend_tf {
                rt = t;
            }
        }
        (ht, lt, rt)
    }

    /// Pine: request.security(..., "60"/"240"/"D", [ta.ema(close,20), ta.vwap(hlc3)]) — en az 1 bar ile çalışır.
    fn min_bars_for_trend(tf: Timeframe) -> usize {
        match tf {
            Timeframe::D1 => 1,
            Timeframe::H4 => 2,
            Timeframe::H1 => 5,
            _ => 20,
        }
    }

    pub fn trend_for_tf(&self, buffer: &CandleBuffer, tf: Timeframe) -> i32 {
        let min_bars = Self::min_bars_for_trend(tf);
        let candles = match buffer.get(tf) {
            Some(c) if c.len() >= min_bars => c,
            _ => return 0,
        };
        let closes: Vec<f64> = candles.iter().map(|x| x.close).collect();
        let period = (20).max(1).min(closes.len()); // Pine: ta.ema(close, 20); 1 bar ile period=1
        let ema_val = ema(&closes, period).unwrap_or_else(|| closes.last().copied().unwrap_or(0.0));
        let vwap_val = vwap(candles).unwrap_or(ema_val);
        let last_close = candles.last().map(|c| c.close).unwrap_or(0.0);
        // Pine: close > ema and close > vwap ? 1 : ... ; >= ile sınırda da yön ver (1H/4H/1D flat kalmasın)
        if last_close >= ema_val && last_close >= vwap_val {
            1
        } else if last_close <= ema_val && last_close <= vwap_val {
            -1
        } else {
            0
        }
    }

    pub fn trend_strength(&self, buffer: &CandleBuffer) -> f64 {
        let mut raw = 0i32;
        for tf in [Timeframe::M1, Timeframe::M5, Timeframe::M15, Timeframe::M30, Timeframe::H1, Timeframe::H4, Timeframe::D1] {
            raw += self.trend_for_tf(buffer, tf);
        }
        (raw as f64 / 7.0) * 100.0
    }

    pub fn system_confidence(&self, buffer: &CandleBuffer) -> f64 {
        let mut raw = 0i32;
        for tf in [Timeframe::M1, Timeframe::M5, Timeframe::M15, Timeframe::M30, Timeframe::H1, Timeframe::H4, Timeframe::D1] {
            raw += self.trend_for_tf(buffer, tf);
        }
        if raw == 7 || raw == -7 {
            90.0
        } else if raw >= 4 || raw <= -4 {
            75.0
        } else if raw >= 2 || raw <= -2 {
            60.0
        } else {
            50.0
        }
    }

    /// Compute generic position-level metrics that can be reused by D/T/Q style setups.
    ///
    /// - `side`: optional position direction; if `None`, trend direction is inferred.
    /// - `entry` / `stop_loss` / `take_profit`: if missing, fall back to chart close and ATR-based estimates.
    pub fn compute_position_metrics(
        &self,
        buffer: &CandleBuffer,
        chart_tf: Timeframe,
        symbol: &str,
        side: Option<SignalType>,
        entry: Option<f64>,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    ) -> Option<PositionMetrics> {
        let candles = buffer.get(chart_tf)?;
        if candles.len() < 50 {
            return None;
        }
        let c = candles.last()?;
        let price = c.close.max(1e-6);
        let atr_val = atr(candles, 14).unwrap_or((c.high - c.low).max(1e-6));
        let volatility_pct = (atr_val / price) * 100.0;

        // Local/global trends
        let local_trend = self.trend_for_tf(buffer, chart_tf);
        let global_trend = self.trend_for_tf(buffer, self.config.higher_tf);
        let trend_strength = self.trend_strength(buffer);

        // Direction sign for aligning momentum with the position or trend.
        let dir_sign = match side {
            Some(SignalType::Sell) => -1.0,
            Some(SignalType::Buy) => 1.0,
            _ => {
                if trend_strength >= 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
        };

        // Short/long momentum as rate-of-change over two windows.
        let roc = |lookback: usize| -> f64 {
            if candles.len() <= lookback {
                return 0.0;
            }
            let prev = candles[candles.len() - 1 - lookback].close.max(1e-6);
            (price - prev) / prev
        };
        let raw_mom_short = dir_sign * roc(5);
        let raw_mom_long = dir_sign * roc(20);

        // Normalize ROC: map roughly -5%..+5% to 0..1.
        let norm_roc = |v: f64| -> f64 {
            let max = 0.05;
            let x = (v / max).clamp(-1.0, 1.0);
            (x + 1.0) / 2.0
        };
        let mom_short_score = norm_roc(raw_mom_short);
        let mom_long_score = norm_roc(raw_mom_long);
        let momentum_score = 0.5 * mom_short_score + 0.5 * mom_long_score;

        // Risk / reward ratio: fall back to ATR-based estimates if needed.
        let effective_entry = entry.unwrap_or(price);
        let effective_sl = stop_loss.unwrap_or_else(|| {
            if dir_sign >= 0.0 {
                effective_entry - 1.5 * atr_val
            } else {
                effective_entry + 1.5 * atr_val
            }
        });
        let risk = (effective_entry - effective_sl).abs().max(1e-6);
        let effective_tp = take_profit.unwrap_or_else(|| {
            if dir_sign >= 0.0 {
                effective_entry + self.config.q_min_rr * risk
            } else {
                effective_entry - self.config.q_min_rr * risk
            }
        });
        let rr = (effective_tp - effective_entry).abs() / risk;
        let rr_score = ((rr - 1.0) / 2.0).clamp(0.0, 1.0);

        // Trend score (0–1) from trend_strength magnitude.
        let trend_score = (trend_strength.abs() / 100.0).clamp(0.0, 1.0);

        // Combine to overall [0,1] score; reuse config weights for simplicity.
        let w_trend = self.config.q_weight_trend;
        let w_momentum = self.config.q_weight_momentum;
        let w_rr = self.config.q_weight_rr;
        let denom = (w_trend + w_momentum + w_rr).max(1e-6);
        let overall_0_1 =
            (w_trend * trend_score + w_momentum * momentum_score + w_rr * rr_score) / denom;
        let overall_0_1 = overall_0_1.clamp(0.0, 1.0);

        // Points for UI buckets.
        let trend_points = (trend_score * 4.0).round().clamp(0.0, 4.0) as u8;
        let momentum_points = (momentum_score * 3.0).round().clamp(0.0, 3.0) as u8;
        let rr_points = (rr_score * 3.0).round().clamp(0.0, 3.0) as u8;
        let strength_points = (1.0 + 9.0 * overall_0_1).round().clamp(1.0, 10.0) as u8;

        let tmr_scores = crate::types::TrendMomentumRiskScores {
            trend_score,
            momentum_score,
            rr_score,
            overall_score: overall_0_1,
            trend_points,
            momentum_points,
            rr_points,
            strength_points,
        };

        // Trend exhaustion: late fibo phase + weak momentum.
        let phase = self.fibo_time_phase(candles, self.config.pivot_length as usize);
        let trend_exhaustion =
            phase >= self.config.q_late_phase && momentum_score < 0.4 && trend_score > 0.3;

        // Structure shift: structure score degraded.
        let inferred_side = side.unwrap_or_else(|| {
            if trend_strength >= 0.0 {
                SignalType::Buy
            } else {
                SignalType::Sell
            }
        });
        let structure_score =
            self.structure_score(candles, inferred_side, self.config.pivot_length as usize);
        let structure_shift = structure_score < 0.3;

        // Position state and simple market mode classification.
        let position_state = match side {
            Some(SignalType::Buy) => "Long".to_string(),
            Some(SignalType::Sell) => "Short".to_string(),
            _ => "Flat".to_string(),
        };
        let market_mode = {
            // Use volatility and trend magnitude to classify a few basic regimes.
            let vol_norm = (volatility_pct / 1.0).clamp(0.0, 3.0); // roughly 0–3%
            if trend_strength.abs() >= 60.0 && vol_norm > 1.2 {
                "Breakout".to_string()
            } else if trend_strength.abs() >= 40.0 {
                "Trending".to_string()
            } else if vol_norm < 0.6 {
                "Range".to_string()
            } else {
                "Neutral".to_string()
            }
        };

        Some(PositionMetrics {
            symbol: symbol.to_string(),
            timeframe: chart_tf,
            side,
            local_trend,
            global_trend,
            position_state,
            market_mode,
            volatility_pct,
            momentum_short: raw_mom_short,
            momentum_long: raw_mom_long,
            entry_price: effective_entry,
            stop_loss_initial: effective_sl,
            take_profit_initial: effective_tp,
            stop_trail_active: f64::NAN,
            take_profit_dynamic: f64::NAN,
            rr,
            tmr_scores,
            trend_exhaustion,
            structure_shift,
        })
    }

    /// Mevcut mum dizisinden basit bir Fibo-zaman fazı tahmini üret.
    ///
    /// Burada tam döngü tespiti yerine, son güçlü pivot'u döngü başlangıcı varsayarak
    /// Fibo uzunluklarının kümülatif toplamına göre normalize edilmiş bir [0-1] faz değeri döner.
    fn fibo_time_phase(&self, candles: &[Candle], pivot_len: usize) -> f64 {
        if candles.len() < pivot_len * 4 + 10 {
            return 0.0;
        }
        let len = candles.len();
        let sub = &candles[..len];
        // Son pivot low veya high'ı döngü başlangıcı kabul et
        let last_pivot_low = pivot_low(sub, pivot_len).unwrap_or(candles[0].low);
        let last_pivot_high = pivot_high(sub, pivot_len).unwrap_or(candles[0].high);
        let start_idx = candles
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, c)| {
                if (c.low - last_pivot_low).abs() < 1e-6 || (c.high - last_pivot_high).abs() < 1e-6 {
                    Some(i)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let bars_since_start = (len - 1).saturating_sub(start_idx) as u32;
        // Fibo dizisi: 1,2,3,5,8,13 – toplam yaklaşık 32 barlık döngü
        let fibo: [u32; 6] = [1, 2, 3, 5, 8, 13];
        let total: u32 = fibo.iter().sum();
        let phase = bars_since_start as f64 / total.max(1) as f64;
        phase.clamp(0.0, 1.5) // bir miktar taşmaya izin ver, sonra kırp
    }

    /// Son pivot low/high değerlerini döndürür (giriş bölgesi ve yapı skoru için).
    fn last_pivots(&self, candles: &[Candle], pivot_len: usize) -> (f64, f64) {
        let l = pivot_low(candles, pivot_len).unwrap_or_else(|| candles.first().map(|c| c.low).unwrap_or(0.0));
        let h = pivot_high(candles, pivot_len).unwrap_or_else(|| candles.first().map(|c| c.high).unwrap_or(0.0));
        (l, h)
    }

    /// Piyasa yapısına uygun TP: pivot–swing mesafesi extension ile projekte edilir.
    /// Long: entry + ext * (recent_high - L_pivot), Short: entry - ext * (H_pivot - recent_low).
    fn structure_based_tp(
        &self,
        candles: &[Candle],
        side: SignalType,
        l_pivot: f64,
        h_pivot: f64,
        entry: f64,
        risk: f64,
        pivot_len: usize,
    ) -> Option<f64> {
        let len = candles.len();
        let lookback = (pivot_len * 2).min(len.saturating_sub(1));
        if lookback == 0 {
            return None;
        }
        let start = len.saturating_sub(lookback);
        let recent_high = candles[start..len].iter().map(|c| c.high).fold(0.0_f64, f64::max);
        let recent_low = candles[start..len]
            .iter()
            .map(|c| c.low)
            .fold(f64::INFINITY, f64::min);
        let ext = self.config.q_tp_structure_ext;
        let max_r = self.config.q_tp_max_r;

        match side {
            SignalType::Buy => {
                let swing_up = (recent_high - l_pivot).max(0.0);
                if swing_up < 1e-9 {
                    return None;
                }
                let raw = entry + ext * swing_up;
                let cap = entry + max_r * risk;
                Some(raw.min(cap))
            }
            SignalType::Sell => {
                let swing_down = (h_pivot - recent_low).max(0.0);
                if swing_down < 1e-9 {
                    return None;
                }
                let raw = entry - ext * swing_down;
                let cap = entry - max_r * risk;
                Some(raw.max(cap))
            }
            _ => None,
        }
    }

    /// Yapı skoru: HH/HL (long) veya LL/LH (short) – son iki pivot karşılaştırması.
    /// Yapı skoru (HL/LL); dip confluence için pub(crate).
    pub(crate) fn structure_score(&self, candles: &[Candle], side: SignalType, pivot_len: usize) -> f64 {
        if candles.len() < pivot_len * 3 + 2 {
            return 0.5;
        }
        let curr_pl = pivot_low(candles, pivot_len);
        let curr_ph = pivot_high(candles, pivot_len);
        let prev_pl = pivot_low(&candles[..candles.len() - 1], pivot_len);
        let prev_ph = pivot_high(&candles[..candles.len() - 1], pivot_len);
        match side {
            SignalType::Buy => {
                if let (Some(curr), Some(prev)) = (curr_pl, prev_pl) {
                    if curr > prev {
                        return 1.0; // higher low
                    }
                }
            }
            SignalType::Sell => {
                if let (Some(curr), Some(prev)) = (curr_ph, prev_ph) {
                    if curr < prev {
                        return 1.0; // lower high
                    }
                }
            }
            _ => {}
        }
        0.3
    }

    /// Q-Setup / Q-Analiz erken uyarı hesaplama
    /// Pivot + ATR giriş bölgesi, 5 bileşenli Q-skor, radar_early ve time_window_bars ile.
    pub fn compute_q_setup(
        &self,
        buffer: &CandleBuffer,
        chart_tf: Timeframe,
        symbol: &str,
        radar: Option<&QRadarSignal>,
    ) -> Option<QSetup> {
        let candles = buffer.get(chart_tf)?;
        let pl = self.config.pivot_length as usize;
        if candles.len() < (self.config.pivot_length * 4 + 50) as usize {
            return None;
        }
        let len = candles.len();
        let c = candles.last()?;
        let prev_c = candles.get(len - 2)?;

        let trend_strength = self.trend_strength(buffer);
        let _system_conf = self.system_confidence(buffer);
        let atr_val = atr(candles, 14).unwrap_or((c.high - c.low).max(1e-6));

        let side = if trend_strength > 0.0 && c.is_bullish() && c.close >= prev_c.close {
            SignalType::Buy
        } else if trend_strength < 0.0 && c.is_bearish() && c.close <= prev_c.close {
            SignalType::Sell
        } else {
            return None;
        };

        let dir_score = match side {
            SignalType::Buy => trend_strength.max(0.0),
            SignalType::Sell => (-trend_strength).max(0.0),
            _ => 0.0,
        };
        let phase = self.fibo_time_phase(candles, pl);

        // Pivot tabanlı giriş bölgesi ve SL (L_pivot ± α·ATR, γ·ATR)
        let (l_pivot, h_pivot) = self.last_pivots(candles, pl);
        let (entry_zone, stop_loss, entry) = match side {
            SignalType::Buy => {
                let ez_min = l_pivot + self.config.q_entry_atr_alpha * atr_val;
                let ez_max = l_pivot + self.config.q_entry_atr_beta * atr_val;
                let sl = l_pivot - self.config.q_sl_atr_gamma * atr_val;
                let entry = c.close.clamp(ez_min, ez_max.max(ez_min));
                ((ez_min, ez_max), sl, entry)
            }
            SignalType::Sell => {
                let ez_max = h_pivot - self.config.q_entry_atr_alpha * atr_val;
                let ez_min = h_pivot - self.config.q_entry_atr_beta * atr_val;
                let sl = h_pivot + self.config.q_sl_atr_gamma * atr_val;
                let entry = c.close.clamp(ez_min.min(ez_max), ez_max);
                ((ez_min.min(ez_max), ez_max), sl, entry)
            }
            _ => return None,
        };

        let risk = (entry - stop_loss).abs().max(1e-6);
        let rr_tp = match side {
            SignalType::Buy => entry + (self.config.q_min_rr * risk).max(2.0 * atr_val),
            SignalType::Sell => entry - (self.config.q_min_rr * risk).max(2.0 * atr_val),
            _ => return None,
        };
        // Piyasa yapısına uygun TP: pivot–swing projeksiyonu, en az rr_tp kadar (long’ta yukarı, short’ta aşağı)
        let take_profit = self
            .structure_based_tp(candles, side, l_pivot, h_pivot, entry, risk, pl)
            .map(|structure_tp| match side {
                SignalType::Buy => structure_tp.max(rr_tp),
                SignalType::Sell => structure_tp.min(rr_tp),
                _ => rr_tp,
            })
            .unwrap_or(rr_tp);

        // 5 bileşenli Q-skor (0–1 normalize)
        let trend_score = (dir_score / 100.0).clamp(0.0, 1.0);
        let structure_score = self.structure_score(candles, side, pl);
        let time_score = if phase >= self.config.q_entry_phase_min && phase <= self.config.q_entry_phase_max {
            1.0
        } else if phase > self.config.q_late_phase {
            0.0
        } else {
            0.5
        };
        let rr = (take_profit - entry).abs() / risk;
        let rr_score = ((rr - 1.0) / 2.0).clamp(0.0, 1.0); // 1R->0, 3R->1
        let vol_avg = sma(
            &candles.iter().map(|x| x.volume).collect::<Vec<_>>(),
            20,
        ).unwrap_or(c.volume);
        let body_ratio = (c.close - c.open).abs() / atr_val.max(1e-6);
        let vol_ratio = (c.volume / vol_avg.max(1e-6)).min(2.0) / 2.0;
        let momentum_score = (body_ratio.min(1.0) * 0.6 + vol_ratio * 0.4).min(1.0);

        let q_score = 100.0
            * (self.config.q_weight_trend * trend_score
                + self.config.q_weight_structure * structure_score
                + self.config.q_weight_time * time_score
                + self.config.q_weight_rr * rr_score
                + self.config.q_weight_momentum * momentum_score);
        let q_score = q_score.clamp(0.0, 100.0);

        if q_score < self.config.q_score_threshold {
            return None;
        }

        let expected_bars = 13u32;
        let time_window_bars = (expected_bars, expected_bars + 8);
        let radar_early = radar
            .map(|r| r.side == side && r.symbol == symbol)
            .unwrap_or(false);

        Some(QSetup {
            symbol: symbol.to_string(),
            timeframe: chart_tf,
            side,
            entry,
            entry_zone,
            stop_loss,
            take_profit,
            q_score,
            time_window_bars,
            expected_bars,
            radar_early,
        })
    }

    /// Q-RADAR: Erken uyarı sinyali üret (setup'tan önceki bant).
    pub fn compute_q_radar(
        &self,
        buffer: &CandleBuffer,
        chart_tf: Timeframe,
        symbol: &str,
    ) -> Option<QRadarSignal> {
        let candles = buffer.get(chart_tf)?;
        if candles.len() < (self.config.pivot_length * 4 + 30) as usize {
            return None;
        }
        let len = candles.len();
        let c = candles.last()?;
        let prev_c = candles.get(len - 2)?;

        let trend_strength = self.trend_strength(buffer);
        let system_conf = self.system_confidence(buffer);
        let atr_val = atr(candles, 14).unwrap_or((c.high - c.low).max(1e-6));

        // Yön: trend + son mum
        let side = if trend_strength > 0.0 && c.is_bullish() && c.close >= prev_c.close {
            SignalType::Buy
        } else if trend_strength < 0.0 && c.is_bearish() && c.close <= prev_c.close {
            SignalType::Sell
        } else {
            return None;
        };

        let dir_score = match side {
            SignalType::Buy => trend_strength.max(0.0),
            SignalType::Sell => (-trend_strength).max(0.0),
            _ => 0.0,
        };

        let phase = self.fibo_time_phase(candles, self.config.pivot_length as usize);

        // Radar sadece erken fazda tetiklenir
        if phase < self.config.q_radar_phase_min || phase > self.config.q_radar_phase_max {
            return None;
        }

        let dir_score_norm = (dir_score / 100.0).clamp(0.0, 1.0);
        let conf_norm = ((system_conf - 50.0) / 40.0).clamp(0.0, 1.0);
        let phase_score = 1.0 - ((phase - (self.config.q_radar_phase_min + self.config.q_radar_phase_max) / 2.0)
            / ((self.config.q_radar_phase_max - self.config.q_radar_phase_min) / 2.0))
            .abs()
            .clamp(0.0, 1.0);

        let confidence = (dir_score_norm * 0.5 + conf_norm * 0.3 + phase_score * 0.2).clamp(0.0, 1.0);
        if confidence < 0.4 {
            return None;
        }

        let expected_min = 5u32;
        let expected_max = 13u32;

        let reference_price = c.close;
        let pivot_len = self.config.pivot_length as usize;
        let suggested_sl = match side {
            SignalType::Buy => {
                pivot_low(candles, pivot_len)
                    .map(|pl| (pl - atr_val * 0.15).min(reference_price - 1e-6))
                    .or_else(|| Some((c.low - atr_val * 0.5).min(reference_price - 1e-6)))
            }
            SignalType::Sell => {
                pivot_high(candles, pivot_len)
                    .map(|ph| (ph + atr_val * 0.15).max(reference_price + 1e-6))
                    .or_else(|| Some((c.high + atr_val * 0.5).max(reference_price + 1e-6)))
            }
            _ => None,
        };

        Some(QRadarSignal {
            symbol: symbol.to_string(),
            timeframe: chart_tf,
            side,
            confidence,
            expected_window_bars: (expected_min, expected_max),
            reference_price,
            suggested_sl,
        })
    }

    /// Poz Koruma sinyali üret – mevcut fiyat ve giriş/SL bilgisine göre.
    ///
    /// Bu fonksiyon, kar içindeyken zorunlu koruma/çıkış alanına girildiğinde tetiklenmek üzere tasarlanır.
    pub fn compute_protect_signal(
        &self,
        buffer: &CandleBuffer,
        chart_tf: Timeframe,
        symbol: &str,
        entry: f64,
        stop_loss: f64,
    ) -> Option<ProtectSignal> {
        let candles = buffer.get(chart_tf)?;
        if candles.is_empty() {
            return None;
        }
        let c = candles.last()?;
        let current_price = c.close;

        let risk_r = (entry - stop_loss).abs().max(1e-6);
        let profit = match entry.partial_cmp(&stop_loss) {
            Some(std::cmp::Ordering::Greater) => current_price - entry, // long
            Some(std::cmp::Ordering::Less) => entry - current_price,   // short
            _ => 0.0,
        };
        let profit_r = profit / risk_r;
        if profit_r < self.config.q_protect_min_r {
            return None;
        }

        let phase = self.fibo_time_phase(candles, self.config.pivot_length as usize);
        let reason = if phase >= self.config.q_late_phase {
            "LATE_PHASE"
        } else {
            "TRAILING_PROFIT"
        }
        .to_string();

        // En az q_protect_lock_r kadar kârı kilitle
        let locked_r = self.config.q_protect_lock_r.min(profit_r).max(0.0);
        let trigger_price = match entry.partial_cmp(&stop_loss) {
            Some(std::cmp::Ordering::Greater) => entry + locked_r * risk_r,
            Some(std::cmp::Ordering::Less) => entry - locked_r * risk_r,
            _ => current_price,
        };

        Some(ProtectSignal {
            symbol: symbol.to_string(),
            timeframe: chart_tf,
            reason,
            trigger_price,
            entry_price: entry,
            locked_r,
        })
    }
}

#[cfg(test)]
mod signal_engine_tests {
    use super::{CandleBuffer, SignalEngine};
    use crate::config::Config;
    use crate::types::{Candle, Timeframe};

    fn synthetic_uptrend(n: usize) -> Vec<Candle> {
        let mut v = Vec::with_capacity(n);
        let mut price = 100.0_f64;
        for i in 0..n {
            let t = (i as i64) * 60_000;
            price += 0.12;
            v.push(Candle {
                time: t,
                open: price,
                high: price + 0.4,
                low: price - 0.4,
                close: price + 0.08,
                volume: 800.0 + i as f64,
            });
        }
        v
    }

    /// `process` için tüm TF’lerde yeterli mum (trend hesapları için).
    fn buffer_all_tf(n: usize) -> CandleBuffer {
        let c = synthetic_uptrend(n);
        let mut buf = CandleBuffer::new();
        for tf in [
            Timeframe::M1,
            Timeframe::M5,
            Timeframe::M15,
            Timeframe::M30,
            Timeframe::H1,
            Timeframe::H4,
            Timeframe::D1,
        ] {
            buf.update(tf, c.clone());
        }
        buf
    }

    #[test]
    fn candle_buffer_update_and_get() {
        let mut b = CandleBuffer::new();
        let c = synthetic_uptrend(10);
        b.update(Timeframe::M5, c.clone());
        assert_eq!(b.get(Timeframe::M5).unwrap().len(), 10);
        assert!(b.get(Timeframe::H1).is_none());
    }

    #[test]
    fn signal_engine_process_runs_without_panic() {
        let mut engine = SignalEngine::new(Config::default());
        let buf = buffer_all_tf(50);
        let _signals = engine.process(&buf, Timeframe::M5);
    }

    #[test]
    fn trend_for_tf_returns_when_enough_bars() {
        let engine = SignalEngine::new(Config::default());
        let buf = buffer_all_tf(25);
        let t = engine.trend_for_tf(&buf, Timeframe::M5);
        assert!(t >= -1 && t <= 1);
    }
}
