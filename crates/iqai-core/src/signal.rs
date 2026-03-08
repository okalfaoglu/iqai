//! Smart Money Structure signal engine - 1:1 logic from Pine Script

use std::collections::HashMap;

use crate::config::Config;
use crate::indicators::*;
use crate::types::{Candle, Signal, SignalType, Timeframe};

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

    pub fn trend_for_tf(&self, buffer: &CandleBuffer, tf: Timeframe) -> i32 {
        let candles = match buffer.get(tf) {
            Some(c) if c.len() >= 20 => c,
            _ => return 0,
        };
        let closes: Vec<f64> = candles.iter().map(|x| x.close).collect();
        let ema_val = ema(&closes, 20).unwrap_or(0.0);
        let vwap_val = vwap(candles).unwrap_or(0.0);
        let last_close = candles.last().map(|c| c.close).unwrap_or(0.0);
        if last_close > ema_val && last_close > vwap_val {
            1
        } else if last_close < ema_val && last_close < vwap_val {
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
}
