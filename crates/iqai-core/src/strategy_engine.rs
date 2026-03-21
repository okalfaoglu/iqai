//! Strategy execution engine built on top of `StrategyPlan`.
//!
//! Bu modül, StrategyPlan nesnelerini bar bar yürütmek için kullanılır.
//! Amaç:
//! - Backtest modunda belirli bir planın PnL ve R istatistiklerini çıkarmak,
//! - Canlı/daemon modunda aynı mantığı TradeManager / exchange katmanına bağlamak.

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::indicators::{atr, highest, lowest};
use crate::trade_manager::calculate_position_size;
use crate::types::Candle;
use crate::{StrategyDirection, StrategyPlan};

/// Tek bir planın backtest sonucu (plan odaklı, tüm seri için).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPlanBacktestResult {
    pub plan: StrategyPlan,
    pub initial_capital: f64,
    pub final_capital: f64,
    pub total_pnl: f64,
    pub total_return_pct: f64,
    pub trades: usize,
    pub win_count: usize,
    pub loss_count: usize,
    pub win_rate_pct: f64,
}

/// Dahili açık pozisyon temsili.
struct EnginePosition {
    entry_price: f64,
    /// Current stop level (breakeven / trailing ile güncellenir)
    current_sl: f64,
    /// Scenario ana TP seviyesi (kalan pozisyonu kapatır)
    initial_tp: f64,
    qty: f64,
    is_long: bool,
    /// 1R = entry_price - initial_stop (mutlak değer)
    r_per_unit: f64,
    /// 0..1 kalan pozisyon (partial close sonrası düşer)
    remaining_pct: f64,
    breakeven_done: bool,
    tp1_done: bool,
    tp2_done: bool,
}

fn chandelier_long(candles: &[Candle], cfg: &Config) -> f64 {
    let period = cfg.atr_trailing_period as usize;
    let atr_val = atr(candles, period).unwrap_or(0.0);
    let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
    let p = period.min(highs.len()).max(1);
    let hh = highest(&highs, p).unwrap_or_else(|| candles.last().map(|c| c.high).unwrap_or(0.0));
    hh - atr_val * cfg.atr_trailing_mult
}

fn chandelier_short(candles: &[Candle], cfg: &Config) -> f64 {
    let period = cfg.atr_trailing_period as usize;
    let atr_val = atr(candles, period).unwrap_or(0.0);
    let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
    let p = period.min(lows.len()).max(1);
    let ll = lowest(&lows, p).unwrap_or_else(|| candles.last().map(|c| c.low).unwrap_or(f64::MAX));
    ll + atr_val * cfg.atr_trailing_mult
}

/// Belirli bir `StrategyPlan` için tek-seri backtest çalıştır.
///
/// Notlar:
/// - Sadece planın kendi entry/SL/TP'si kullanılır (ek hedefler istatistik amaçlıdır).
/// - Aynı anda yalnızca bir pozisyon açık tutulur.
pub fn run_strategy_plan_backtest(
    candles: &[Candle],
    cfg: &Config,
    plan: &StrategyPlan,
    initial_capital: f64,
    risk_pct_per_trade: f64,
    leverage: f64,
    commission_bps: u32,
    slippage_bps: u32,
) -> StrategyPlanBacktestResult {
    if candles.len() < 10 {
        return StrategyPlanBacktestResult {
            plan: plan.clone(),
            initial_capital,
            final_capital: initial_capital,
            total_pnl: 0.0,
            total_return_pct: 0.0,
            trades: 0,
            win_count: 0,
            loss_count: 0,
            win_rate_pct: 0.0,
        };
    }

    let mut balance = initial_capital;
    let mut pos: Option<EnginePosition> = None;
    let mut trades = 0usize;
    let mut win_count = 0usize;
    let mut loss_count = 0usize;
    let commission_rate = commission_bps as f64 / 10_000.0;

    // Match AutoTrader's slippage direction: long exit price decreases, short exit price increases.
    let apply_slippage = |price: f64, is_long: bool| -> f64 {
        if slippage_bps == 0 || !price.is_finite() {
            return price;
        }
        let pct = slippage_bps as f64 / 10_000.0;
        if is_long {
            price * (1.0 - pct)
        } else {
            price * (1.0 + pct)
        }
    };

    for (i, c) in candles.iter().enumerate() {
        // Eğer pozisyon yoksa, entry bölgesini test et.
        if pos.is_none() {
            // Entry tetikleme:
            // - Önceki sürümde sadece `plan.entry` tek noktası kullanılıyordu.
            // - Live/plan tarafında önerilen `entry_zone` (min..max) olduğu için
            //   mum aralığı entry_zone ile kesişiyorsa pozisyon açalım.
            let (is_long, entry_price) = match plan.direction {
                StrategyDirection::Long => {
                    let target_price = plan.entry;
                    let (z_min, z_max) = plan.entry_zone.unwrap_or((target_price, target_price));
                    // Candle (low..high) ile zone (z_min..z_max) kesişimi
                    if c.low <= z_max && c.high >= z_min {
                        (true, target_price)
                    } else {
                        continue;
                    }
                }
                StrategyDirection::Short => {
                    let target_price = plan.entry;
                    let (z_min, z_max) = plan.entry_zone.unwrap_or((target_price, target_price));
                    if c.low <= z_max && c.high >= z_min {
                        (false, target_price)
                    } else {
                        continue;
                    }
                }
            };

            let risk = (plan.stop_loss - entry_price).abs().max(1e-6);
            let qty = calculate_position_size(
                balance,
                risk_pct_per_trade,
                entry_price,
                plan.stop_loss,
                leverage,
            )
            .max(0.0);
            if qty <= 0.0 {
                continue;
            }

            let tp_price = plan
                .targets
                .first()
                .map(|t| t.price)
                .unwrap_or(plan.stop_loss + if is_long { risk } else { -risk });

            pos = Some(EnginePosition {
                entry_price,
                current_sl: plan.stop_loss,
                initial_tp: tp_price,
                qty,
                is_long,
                r_per_unit: risk,
                remaining_pct: 1.0,
                breakeven_done: false,
                tp1_done: false,
                tp2_done: false,
            });
            continue;
        }

        // Pozisyon açıksa SL/TP kontrol et.
        // T-05: `trade_manager::TradeManager::evaluate` ile aynı öncelik ve `current_price`≈`close`
        // varsayımı (R-tabanlı seviyeler mum kapanışıyla; SL için low/high).
        if let Some(p) = &mut pos {
            let mut closed = false;
            let mut exit_price = p.entry_price;

            let current_sl = p.current_sl;
            let profit_r = if p.is_long {
                (c.close - p.entry_price) / p.r_per_unit
            } else {
                (p.entry_price - c.close) / p.r_per_unit
            };

            if p.is_long {
                // 1) SL  2) tam TP  3) TP1  4) breakeven  5) TP2  6) trailing
                if c.low <= current_sl {
                    exit_price = current_sl;
                    closed = true;
                } else if c.close >= p.initial_tp {
                    exit_price = p.initial_tp;
                    closed = true;
                } else if !p.tp1_done && profit_r >= cfg.tp1_r {
                    let tp1_price = p.entry_price + cfg.tp1_r * p.r_per_unit;
                    let pct = cfg.partial_tp1_pct.clamp(0.0, 1.0);
                    let applied_pct = pct.min(p.remaining_pct.max(0.0));
                    let close_qty = p.qty * applied_pct;
                    if close_qty > 0.0 {
                        let effective_exit = apply_slippage(tp1_price, p.is_long);
                        let pnl_gross = (effective_exit - p.entry_price) * close_qty;
                        let fee = (p.entry_price * close_qty + effective_exit * close_qty)
                            * commission_rate;
                        let pnl = pnl_gross - fee;
                        balance += pnl;
                        p.remaining_pct = (p.remaining_pct - applied_pct).clamp(0.0, 1.0);
                    }
                    p.tp1_done = true;
                } else if !p.breakeven_done && cfg.breakeven_r > 0.0 && profit_r >= cfg.breakeven_r {
                    p.current_sl = p.entry_price;
                    p.breakeven_done = true;
                } else if !p.tp2_done && profit_r >= cfg.tp2_r {
                    let tp2_price = p.entry_price + cfg.tp2_r * p.r_per_unit;
                    let pct = cfg.partial_tp2_pct.clamp(0.0, 1.0);
                    let applied_pct = pct.min(p.remaining_pct.max(0.0));
                    let close_qty = p.qty * applied_pct;
                    if close_qty > 0.0 {
                        let effective_exit = apply_slippage(tp2_price, p.is_long);
                        let pnl_gross = (effective_exit - p.entry_price) * close_qty;
                        let fee = (p.entry_price * close_qty + effective_exit * close_qty)
                            * commission_rate;
                        let pnl = pnl_gross - fee;
                        balance += pnl;
                        p.remaining_pct = (p.remaining_pct - applied_pct).clamp(0.0, 1.0);
                    }
                    p.tp2_done = true;
                } else if p.breakeven_done && i + 1 >= cfg.atr_trailing_period as usize {
                    let new_sl = chandelier_long(&candles[..=i], cfg);
                    if new_sl > p.current_sl && new_sl < c.close {
                        p.current_sl = new_sl;
                    }
                }
            } else {
                // Short
                if c.high >= current_sl {
                    exit_price = current_sl;
                    closed = true;
                } else if c.close <= p.initial_tp {
                    exit_price = p.initial_tp;
                    closed = true;
                } else if !p.tp1_done && profit_r >= cfg.tp1_r {
                    let tp1_price = p.entry_price - cfg.tp1_r * p.r_per_unit;
                    let pct = cfg.partial_tp1_pct.clamp(0.0, 1.0);
                    let applied_pct = pct.min(p.remaining_pct.max(0.0));
                    let close_qty = p.qty * applied_pct;
                    if close_qty > 0.0 {
                        let effective_exit = apply_slippage(tp1_price, p.is_long);
                        let pnl_gross = (p.entry_price - effective_exit) * close_qty;
                        let fee = (p.entry_price * close_qty + effective_exit * close_qty)
                            * commission_rate;
                        let pnl = pnl_gross - fee;
                        balance += pnl;
                        p.remaining_pct = (p.remaining_pct - applied_pct).clamp(0.0, 1.0);
                    }
                    p.tp1_done = true;
                } else if !p.breakeven_done && cfg.breakeven_r > 0.0 && profit_r >= cfg.breakeven_r {
                    p.current_sl = p.entry_price;
                    p.breakeven_done = true;
                } else if !p.tp2_done && profit_r >= cfg.tp2_r {
                    let tp2_price = p.entry_price - cfg.tp2_r * p.r_per_unit;
                    let pct = cfg.partial_tp2_pct.clamp(0.0, 1.0);
                    let applied_pct = pct.min(p.remaining_pct.max(0.0));
                    let close_qty = p.qty * applied_pct;
                    if close_qty > 0.0 {
                        let effective_exit = apply_slippage(tp2_price, p.is_long);
                        let pnl_gross = (p.entry_price - effective_exit) * close_qty;
                        let fee = (p.entry_price * close_qty + effective_exit * close_qty)
                            * commission_rate;
                        let pnl = pnl_gross - fee;
                        balance += pnl;
                        p.remaining_pct = (p.remaining_pct - applied_pct).clamp(0.0, 1.0);
                    }
                    p.tp2_done = true;
                } else if p.breakeven_done && i + 1 >= cfg.atr_trailing_period as usize {
                    let new_sl = chandelier_short(&candles[..=i], cfg);
                    if new_sl < p.current_sl && new_sl > c.close {
                        p.current_sl = new_sl;
                    }
                }
            }

            if closed {
                let close_qty = p.qty * p.remaining_pct.max(0.0);
                let effective_exit = apply_slippage(exit_price, p.is_long);
                let pnl_gross = if p.is_long {
                    (effective_exit - p.entry_price) * close_qty
                } else {
                    (p.entry_price - effective_exit) * close_qty
                };
                let fee = (p.entry_price * close_qty + effective_exit * close_qty) * commission_rate;
                let pnl = pnl_gross - fee;
                balance += pnl;
                trades += 1;
                if pnl >= 0.0 {
                    win_count += 1;
                } else {
                    loss_count += 1;
                }
                pos = None;
            }
        }
    }

    let total_pnl = balance - initial_capital;
    let total_return_pct = if initial_capital > 0.0 {
        total_pnl / initial_capital * 100.0
    } else {
        0.0
    };
    let win_rate_pct = if trades > 0 {
        win_count as f64 / trades as f64 * 100.0
    } else {
        0.0
    };

    StrategyPlanBacktestResult {
        plan: plan.clone(),
        initial_capital,
        final_capital: balance,
        total_pnl,
        total_return_pct,
        trades,
        win_count,
        loss_count,
        win_rate_pct,
    }
}

/// T-05: `run_strategy_plan_backtest` ile birebir karşılaştırma için referans simülasyon —
/// `TradeManager::evaluate` + `apply_action` + bar uçlarında SL (engine ile aynı sıra).
#[cfg(test)]
fn run_plan_backtest_via_trade_manager(
    candles: &[Candle],
    cfg: &Config,
    plan: &StrategyPlan,
    initial_capital: f64,
    risk_pct_per_trade: f64,
    leverage: f64,
    commission_bps: u32,
    slippage_bps: u32,
) -> StrategyPlanBacktestResult {
    use crate::trade_manager::{calculate_position_size, PositionSide, TradeAction, TradeManager};

    if candles.len() < 10 {
        return StrategyPlanBacktestResult {
            plan: plan.clone(),
            initial_capital,
            final_capital: initial_capital,
            total_pnl: 0.0,
            total_return_pct: 0.0,
            trades: 0,
            win_count: 0,
            loss_count: 0,
            win_rate_pct: 0.0,
        };
    }

    let tm = TradeManager::new(cfg.clone());
    let mut balance = initial_capital;
    let mut pos: Option<crate::trade_manager::Position> = None;
    let mut trades = 0usize;
    let mut win_count = 0usize;
    let mut loss_count = 0usize;
    let commission_rate = commission_bps as f64 / 10_000.0;

    let apply_slippage = |price: f64, is_long: bool| -> f64 {
        if slippage_bps == 0 || !price.is_finite() {
            return price;
        }
        let pct = slippage_bps as f64 / 10_000.0;
        if is_long {
            price * (1.0 - pct)
        } else {
            price * (1.0 + pct)
        }
    };

    for (i, c) in candles.iter().enumerate() {
        if pos.is_none() {
            let (is_long, entry_price) = match plan.direction {
                StrategyDirection::Long => {
                    let target_price = plan.entry;
                    let (z_min, z_max) = plan.entry_zone.unwrap_or((target_price, target_price));
                    if c.low <= z_max && c.high >= z_min {
                        (true, target_price)
                    } else {
                        continue;
                    }
                }
                StrategyDirection::Short => {
                    let target_price = plan.entry;
                    let (z_min, z_max) = plan.entry_zone.unwrap_or((target_price, target_price));
                    if c.low <= z_max && c.high >= z_min {
                        (false, target_price)
                    } else {
                        continue;
                    }
                }
            };

            let risk = (plan.stop_loss - entry_price).abs().max(1e-6);
            let qty = calculate_position_size(
                balance,
                risk_pct_per_trade,
                entry_price,
                plan.stop_loss,
                leverage,
            )
            .max(0.0);
            if qty <= 0.0 {
                continue;
            }

            let tp_price = plan
                .targets
                .first()
                .map(|t| t.price)
                .unwrap_or(plan.stop_loss + if is_long { risk } else { -risk });

            let side = if is_long {
                PositionSide::Long
            } else {
                PositionSide::Short
            };
            pos = Some(tm.create_position(side, entry_price, qty, plan.stop_loss, tp_price));
            continue;
        }

        let mut p = pos.take().expect("position");
        let mut closed = false;
        let mut exit_price = p.entry_price;

        if matches!(p.side, PositionSide::Long) {
            if c.low <= p.current_sl {
                exit_price = p.current_sl;
                closed = true;
            } else {
                let action = tm.evaluate(&mut p, c.close, &candles[..=i]);
                match &action {
                    TradeAction::FullClose { reason } => {
                        exit_price = if reason == "Stop Loss" {
                            p.current_sl
                        } else {
                            p.initial_tp
                        };
                        closed = true;
                    }
                    TradeAction::PartialClose { pct, .. } => {
                        let tp_exec = if !p.tp1_done {
                            p.entry_price + cfg.tp1_r * p.risk_r
                        } else {
                            p.entry_price + cfg.tp2_r * p.risk_r
                        };
                        let pct_val = (*pct).clamp(0.0, 1.0);
                        let applied_pct = pct_val.min(p.remaining_pct.max(0.0));
                        let close_qty = p.quantity * applied_pct;
                        if close_qty > 0.0 {
                            let effective_exit = apply_slippage(tp_exec, true);
                            let pnl_gross = (effective_exit - p.entry_price) * close_qty;
                            let fee = (p.entry_price * close_qty + effective_exit * close_qty)
                                * commission_rate;
                            balance += pnl_gross - fee;
                        }
                        tm.apply_action(&mut p, &action);
                    }
                    TradeAction::MoveSlToBreakeven => {
                        tm.apply_action(&mut p, &action);
                    }
                    TradeAction::UpdateTrailingStop { .. } => {
                        tm.apply_action(&mut p, &action);
                    }
                    TradeAction::None => {}
                }
            }
        } else {
            // Short
            if c.high >= p.current_sl {
                exit_price = p.current_sl;
                closed = true;
            } else {
                let action = tm.evaluate(&mut p, c.close, &candles[..=i]);
                match &action {
                    TradeAction::FullClose { reason } => {
                        exit_price = if reason == "Stop Loss" {
                            p.current_sl
                        } else {
                            p.initial_tp
                        };
                        closed = true;
                    }
                    TradeAction::PartialClose { pct, .. } => {
                        let tp_exec = if !p.tp1_done {
                            p.entry_price - cfg.tp1_r * p.risk_r
                        } else {
                            p.entry_price - cfg.tp2_r * p.risk_r
                        };
                        let pct_val = (*pct).clamp(0.0, 1.0);
                        let applied_pct = pct_val.min(p.remaining_pct.max(0.0));
                        let close_qty = p.quantity * applied_pct;
                        if close_qty > 0.0 {
                            let effective_exit = apply_slippage(tp_exec, false);
                            let pnl_gross = (p.entry_price - effective_exit) * close_qty;
                            let fee = (p.entry_price * close_qty + effective_exit * close_qty)
                                * commission_rate;
                            balance += pnl_gross - fee;
                        }
                        tm.apply_action(&mut p, &action);
                    }
                    TradeAction::MoveSlToBreakeven => {
                        tm.apply_action(&mut p, &action);
                    }
                    TradeAction::UpdateTrailingStop { .. } => {
                        tm.apply_action(&mut p, &action);
                    }
                    TradeAction::None => {}
                }
            }
        }

        if closed {
            let close_qty = p.quantity * p.remaining_pct.max(0.0);
            let effective_exit = apply_slippage(exit_price, matches!(p.side, PositionSide::Long));
            let pnl_gross = if matches!(p.side, PositionSide::Long) {
                (effective_exit - p.entry_price) * close_qty
            } else {
                (p.entry_price - effective_exit) * close_qty
            };
            let fee =
                (p.entry_price * close_qty + effective_exit * close_qty) * commission_rate;
            let pnl = pnl_gross - fee;
            balance += pnl;
            trades += 1;
            if pnl >= 0.0 {
                win_count += 1;
            } else {
                loss_count += 1;
            }
            pos = None;
        } else {
            pos = Some(p);
        }
    }

    let total_pnl = balance - initial_capital;
    let total_return_pct = if initial_capital > 0.0 {
        total_pnl / initial_capital * 100.0
    } else {
        0.0
    };
    let win_rate_pct = if trades > 0 {
        win_count as f64 / trades as f64 * 100.0
    } else {
        0.0
    };

    StrategyPlanBacktestResult {
        plan: plan.clone(),
        initial_capital,
        final_capital: balance,
        total_pnl,
        total_return_pct,
        trades,
        win_count,
        loss_count,
        win_rate_pct,
    }
}

#[cfg(test)]
mod strategy_plan_backtest_tests {
    use super::*;
    use crate::strategy::{StrategyScenarioKind, StrategyTarget};
    use crate::types::Timeframe;

    fn candle(t: i64, o: f64, h: f64, l: f64, c: f64) -> Candle {
        Candle {
            time: t,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: 1.0,
        }
    }

    fn base_plan_long() -> StrategyPlan {
        StrategyPlan {
            symbol: "TEST".into(),
            timeframe: Timeframe::M5,
            direction: StrategyDirection::Long,
            entry: 100.0,
            entry_zone: Some((99.0, 101.0)),
            stop_loss: 90.0,
            invalidation: None,
            targets: vec![StrategyTarget {
                price: 120.0,
                label: "TP".into(),
                priority: 1,
            }],
            q_score: 80.0,
            classic_pattern_kind: None,
            classic_pattern_label: None,
            elliott_formation: None,
            elliott_summary: None,
            scenario_kind: StrategyScenarioKind::GenericQSetup,
            has_radar_context: false,
        }
    }

    fn base_plan_short() -> StrategyPlan {
        let mut p = base_plan_long();
        p.direction = StrategyDirection::Short;
        p.entry = 100.0;
        p.entry_zone = Some((99.0, 101.0));
        p.stop_loss = 110.0;
        p.targets = vec![StrategyTarget {
            price: 80.0,
            label: "TP".into(),
            priority: 1,
        }];
        p
    }

    fn assert_parity(a: &StrategyPlanBacktestResult, b: &StrategyPlanBacktestResult) {
        const EPS: f64 = 1e-6;
        assert_eq!(a.trades, b.trades, "trades");
        assert_eq!(a.win_count, b.win_count, "win_count");
        assert_eq!(a.loss_count, b.loss_count, "loss_count");
        assert!(
            (a.total_pnl - b.total_pnl).abs() < EPS,
            "total_pnl: {} vs {}",
            a.total_pnl,
            b.total_pnl
        );
        assert!(
            (a.final_capital - b.final_capital).abs() < EPS,
            "final_capital: {} vs {}",
            a.final_capital,
            b.final_capital
        );
    }

    #[test]
    fn parity_long_stop_loss() {
        let mut cfg = Config::default();
        cfg.enable_trade_management = true;
        let plan = base_plan_long();
        let mut v: Vec<Candle> = (0..5)
            .map(|i| candle(i, 95.0, 96.0, 94.0, 95.0))
            .collect();
        v.push(candle(5, 99.0, 102.0, 99.0, 101.0));
        v.push(candle(6, 101.0, 101.0, 88.0, 89.0));
        for i in 7..12 {
            v.push(candle(i as i64, 89.0, 90.0, 88.0, 89.0));
        }

        let a = run_strategy_plan_backtest(
            &v, &cfg, &plan, 10_000.0, 1.0, 5.0, 5, 0,
        );
        let b = run_plan_backtest_via_trade_manager(
            &v, &cfg, &plan, 10_000.0, 1.0, 5.0, 5, 0,
        );
        assert_parity(&a, &b);
        assert_eq!(a.trades, 1);
        assert_eq!(a.loss_count, 1);
    }

    #[test]
    fn parity_long_full_take_profit() {
        let mut cfg = Config::default();
        cfg.enable_trade_management = true;
        let plan = base_plan_long();
        let mut v: Vec<Candle> = (0..5)
            .map(|i| candle(i, 95.0, 96.0, 94.0, 95.0))
            .collect();
        v.push(candle(5, 99.0, 102.0, 99.0, 101.0));
        v.push(candle(6, 101.0, 125.0, 100.0, 122.0));
        for i in 7..12 {
            v.push(candle(i as i64, 122.0, 123.0, 121.0, 122.0));
        }

        let a = run_strategy_plan_backtest(
            &v, &cfg, &plan, 10_000.0, 1.0, 5.0, 5, 0,
        );
        let b = run_plan_backtest_via_trade_manager(
            &v, &cfg, &plan, 10_000.0, 1.0, 5.0, 5, 0,
        );
        assert_parity(&a, &b);
        assert_eq!(a.trades, 1);
        assert_eq!(a.win_count, 1);
    }

    #[test]
    fn parity_long_tp1_then_full_tp() {
        let mut cfg = Config::default();
        cfg.enable_trade_management = true;
        cfg.tp1_r = 1.0;
        cfg.tp2_r = 2.0;
        cfg.partial_tp1_pct = 0.33;
        cfg.partial_tp2_pct = 0.33;
        let plan = base_plan_long();
        let mut v: Vec<Candle> = (0..5)
            .map(|i| candle(i, 95.0, 96.0, 94.0, 95.0))
            .collect();
        v.push(candle(5, 99.0, 102.0, 99.0, 101.0));
        v.push(candle(6, 101.0, 115.0, 100.0, 112.0));
        v.push(candle(7, 112.0, 130.0, 111.0, 125.0));
        for i in 8..12 {
            v.push(candle(i as i64, 125.0, 126.0, 124.0, 125.0));
        }

        let a = run_strategy_plan_backtest(
            &v, &cfg, &plan, 10_000.0, 1.0, 5.0, 5, 0,
        );
        let b = run_plan_backtest_via_trade_manager(
            &v, &cfg, &plan, 10_000.0, 1.0, 5.0, 5, 0,
        );
        assert_parity(&a, &b);
        assert_eq!(a.trades, 1);
        assert_eq!(a.win_count, 1);
    }

    #[test]
    fn parity_short_stop_loss() {
        let mut cfg = Config::default();
        cfg.enable_trade_management = true;
        let plan = base_plan_short();
        let mut v: Vec<Candle> = (0..5)
            .map(|i| candle(i, 105.0, 106.0, 104.0, 105.0))
            .collect();
        v.push(candle(5, 100.0, 101.0, 99.0, 100.0));
        v.push(candle(6, 100.0, 112.0, 100.0, 111.0));
        for i in 7..12 {
            v.push(candle(i as i64, 111.0, 112.0, 110.0, 111.0));
        }

        let a = run_strategy_plan_backtest(
            &v, &cfg, &plan, 10_000.0, 1.0, 5.0, 5, 0,
        );
        let b = run_plan_backtest_via_trade_manager(
            &v, &cfg, &plan, 10_000.0, 1.0, 5.0, 5, 0,
        );
        assert_parity(&a, &b);
        assert_eq!(a.trades, 1);
        assert_eq!(a.loss_count, 1);
    }
}

