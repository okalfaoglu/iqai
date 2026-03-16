//! Strategy execution engine built on top of `StrategyPlan`.
//!
//! Bu modül, StrategyPlan nesnelerini bar bar yürütmek için kullanılır.
//! Amaç:
//! - Backtest modunda belirli bir planın PnL ve R istatistiklerini çıkarmak,
//! - Canlı/daemon modunda aynı mantığı TradeManager / exchange katmanına bağlamak.

use serde::{Deserialize, Serialize};

use crate::config::Config;
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
    stop_loss: f64,
    take_profit: f64,
    qty: f64,
    is_long: bool,
    /// 1R = entry_price - initial_stop (mutlak değer)
    r_per_unit: f64,
    /// Pozisyon açıldıktan sonra fiyatın lehe gittiği en iyi seviye.
    best_favorable_price: f64,
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

    for c in candles {
        // Eğer pozisyon yoksa, entry bölgesini test et.
        if pos.is_none() {
            // Basit tetikleme: long için low..high aralığı entry/entry_zone ile kesişsin.
            let (is_long, entry_price) = match plan.direction {
                StrategyDirection::Long => {
                    let target = plan.entry;
                    if c.low <= target && c.high >= target {
                        (true, target)
                    } else {
                        continue;
                    }
                }
                StrategyDirection::Short => {
                    let target = plan.entry;
                    if c.low <= target && c.high >= target {
                        (false, target)
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

            let best_price = if is_long { entry_price } else { entry_price };

            pos = Some(EnginePosition {
                entry_price,
                stop_loss: plan.stop_loss,
                take_profit: tp_price,
                qty,
                is_long,
                r_per_unit: risk,
                best_favorable_price: best_price,
            });
            continue;
        }

        // Pozisyon açıksa SL/TP kontrol et.
        if let Some(p) = &mut pos {
            // Pozisyon lehe gittiyse en iyi fiyatı güncelle.
            if p.is_long {
                if c.high > p.best_favorable_price {
                    p.best_favorable_price = c.high;
                }
            } else if c.low < p.best_favorable_price {
                p.best_favorable_price = c.low;
            }

            // R bazlı poz koruma: fiyat en az q_protect_min_r kadar lehe gittiyse,
            // stop'u en az q_protect_lock_r R kâr kilitleyecek seviyeye çek.
            let protect_min_r = cfg.q_protect_min_r.max(0.0);
            let protect_lock_r = cfg.q_protect_lock_r.max(0.0);
            if protect_min_r > 0.0 && protect_lock_r > 0.0 {
                let favorable_move = if p.is_long {
                    p.best_favorable_price - p.entry_price
                } else {
                    p.entry_price - p.best_favorable_price
                };
                let reached_r = favorable_move / p.r_per_unit.max(1e-6);
                if reached_r >= protect_min_r {
                    let lock_price = if p.is_long {
                        p.entry_price + protect_lock_r * p.r_per_unit
                    } else {
                        p.entry_price - protect_lock_r * p.r_per_unit
                    };
                    // Long için stop azalır (aşağıdan yukarı), short için artar (yukarıdan aşağı).
                    if p.is_long {
                        if lock_price > p.stop_loss {
                            p.stop_loss = lock_price.min(p.best_favorable_price);
                        }
                    } else if lock_price < p.stop_loss {
                        p.stop_loss = lock_price.max(p.best_favorable_price);
                    }
                }
            }

            let mut closed = false;
            let mut exit_price = p.entry_price;

            if p.is_long {
                // Önce SL, sonra TP.
                if c.low <= p.stop_loss {
                    exit_price = p.stop_loss;
                    closed = true;
                } else if c.high >= p.take_profit {
                    exit_price = p.take_profit;
                    closed = true;
                }
            } else {
                // Short.
                if c.high >= p.stop_loss {
                    exit_price = p.stop_loss;
                    closed = true;
                } else if c.low <= p.take_profit {
                    exit_price = p.take_profit;
                    closed = true;
                }
            }

            if closed {
                let pnl = if p.is_long {
                    (exit_price - p.entry_price) * p.qty
                } else {
                    (p.entry_price - exit_price) * p.qty
                };
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

