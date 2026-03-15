//! Trade Management - Breakeven, kısmi TP, Chandelier/ATR trailing stop

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::indicators::{atr, highest, lowest};
use crate::types::Candle;

/// Açık pozisyon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub side: PositionSide,
    pub entry_price: f64,
    pub quantity: f64,
    pub initial_sl: f64,
    pub initial_tp: f64,
    pub current_sl: f64,
    pub risk_r: f64,           // Risk miktarı (points) = 1R
    pub remaining_pct: f64,    // Kalan pozisyon % (1.0 = %100, 0.67 = %67 vb.)
    pub breakeven_done: bool,
    pub tp1_done: bool,
    pub tp2_done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
}

/// Trade Manager'dan dönen aksiyon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeAction {
    /// Stop'u girişe taşı (breakeven)
    MoveSlToBreakeven,
    /// Kısmi kapat (TP1 veya TP2)
    PartialClose { pct: f64, reason: String },
    /// Trailing stop güncelle
    UpdateTrailingStop { new_sl: f64 },
    /// Tam kapat (SL veya TP tetiklendi)
    FullClose { reason: String },
    /// Aksiyon yok
    None,
}

/// Risk bazlı pozisyon boyutu hesaplama.
/// `account_balance * risk_pct / 100` = riske edilecek tutar;
/// SL mesafesi ile bölünerek miktar (qty) bulunur.
pub fn calculate_position_size(
    account_balance: f64,
    risk_pct: f64,
    entry: f64,
    stop_loss: f64,
    leverage: f64,
) -> f64 {
    let sl_distance = (entry - stop_loss).abs();
    if sl_distance < 1e-10 || entry < 1e-10 {
        return 0.0;
    }
    let risk_amount = account_balance * risk_pct / 100.0;
    let raw_qty = risk_amount / sl_distance;
    let notional = raw_qty * entry;
    let max_notional = account_balance * leverage;
    if notional > max_notional {
        max_notional / entry
    } else {
        raw_qty
    }
}

/// Kar koruma ve trailing stop yönetimi
pub struct TradeManager {
    config: Config,
}

impl TradeManager {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Sinyalden pozisyon oluştur
    pub fn create_position(
        &self,
        side: PositionSide,
        entry: f64,
        quantity: f64,
        sl: f64,
        tp: f64,
    ) -> Position {
        let risk_r = (entry - sl).abs();
        Position {
            side,
            entry_price: entry,
            quantity,
            initial_sl: sl,
            initial_tp: tp,
            current_sl: sl,
            risk_r: risk_r.max(0.0001),
            remaining_pct: 1.0,
            breakeven_done: false,
            tp1_done: false,
            tp2_done: false,
        }
    }

    /// Anlık fiyat ve mumlarla pozisyonu değerlendir, aksiyon öner
    pub fn evaluate(
        &self,
        position: &mut Position,
        current_price: f64,
        candles: &[Candle],
    ) -> TradeAction {
        if !self.config.enable_trade_management {
            return TradeAction::None;
        }

        let cfg = &self.config;
        let r = position.risk_r;

        match position.side {
            PositionSide::Long => {
                let profit = current_price - position.entry_price;
                let profit_r = profit / r;

                // SL tetiklendi mi?
                if current_price <= position.current_sl {
                    return TradeAction::FullClose {
                        reason: "Stop Loss".to_string(),
                    };
                }

                // TP tetiklendi mi? (kalan için)
                if current_price >= position.initial_tp {
                    return TradeAction::FullClose {
                        reason: "Take Profit".to_string(),
                    };
                }

                // TP1 kısmi kapat
                if !position.tp1_done && profit_r >= cfg.tp1_r {
                    return TradeAction::PartialClose {
                        pct: cfg.partial_tp1_pct,
                        reason: "TP1 (1R)".to_string(),
                    };
                }

                // Breakeven: 1R kârda SL'i girişe taşı
                if !position.breakeven_done && profit_r >= cfg.breakeven_r {
                    return TradeAction::MoveSlToBreakeven;
                }

                // TP2 kısmi kapat
                if !position.tp2_done && profit_r >= cfg.tp2_r {
                    return TradeAction::PartialClose {
                        pct: cfg.partial_tp2_pct,
                        reason: "TP2 (2R)".to_string(),
                    };
                }

                // Trailing stop (Chandelier veya ATR)
                if position.breakeven_done && candles.len() >= cfg.atr_trailing_period as usize {
                    let new_sl = self.chandelier_long(candles, cfg);
                    if new_sl > position.current_sl && new_sl < current_price {
                        return TradeAction::UpdateTrailingStop { new_sl };
                    }
                }
            }

            PositionSide::Short => {
                let profit = position.entry_price - current_price;
                let profit_r = profit / r;

                // SL tetiklendi mi?
                if current_price >= position.current_sl {
                    return TradeAction::FullClose {
                        reason: "Stop Loss".to_string(),
                    };
                }

                // TP tetiklendi mi?
                if current_price <= position.initial_tp {
                    return TradeAction::FullClose {
                        reason: "Take Profit".to_string(),
                    };
                }

                // TP1
                if !position.tp1_done && profit_r >= cfg.tp1_r {
                    return TradeAction::PartialClose {
                        pct: cfg.partial_tp1_pct,
                        reason: "TP1 (1R)".to_string(),
                    };
                }

                // Breakeven
                if !position.breakeven_done && profit_r >= cfg.breakeven_r {
                    return TradeAction::MoveSlToBreakeven;
                }

                // TP2
                if !position.tp2_done && profit_r >= cfg.tp2_r {
                    return TradeAction::PartialClose {
                        pct: cfg.partial_tp2_pct,
                        reason: "TP2 (2R)".to_string(),
                    };
                }

                // Trailing stop short
                if position.breakeven_done && candles.len() >= cfg.atr_trailing_period as usize {
                    let new_sl = self.chandelier_short(candles, cfg);
                    if new_sl < position.current_sl && new_sl > current_price {
                        return TradeAction::UpdateTrailingStop { new_sl };
                    }
                }
            }
        }

        TradeAction::None
    }

    /// Chandelier Exit - Long: Stop = Highest(high, N) - ATR(N) * mult
    fn chandelier_long(&self, candles: &[Candle], cfg: &Config) -> f64 {
        let period = cfg.atr_trailing_period as usize;
        let atr_val = atr(candles, period).unwrap_or(0.0);
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let p = period.min(highs.len()).max(1);
        let hh = highest(&highs, p).unwrap_or_else(|| candles.last().map(|c| c.high).unwrap_or(0.0));
        hh - atr_val * cfg.atr_trailing_mult
    }

    /// Chandelier Exit - Short: Stop = Lowest(low, N) + ATR(N) * mult
    fn chandelier_short(&self, candles: &[Candle], cfg: &Config) -> f64 {
        let period = cfg.atr_trailing_period as usize;
        let atr_val = atr(candles, period).unwrap_or(0.0);
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let p = period.min(lows.len()).max(1);
        let ll = lowest(&lows, p).unwrap_or_else(|| candles.last().map(|c| c.low).unwrap_or(f64::MAX));
        ll + atr_val * cfg.atr_trailing_mult
    }

    /// Aksiyonu uygula (pozisyon state güncelle)
    pub fn apply_action(&self, position: &mut Position, action: &TradeAction) {
        match action {
            TradeAction::MoveSlToBreakeven => {
                position.current_sl = position.entry_price;
                position.breakeven_done = true;
            }
            TradeAction::PartialClose { pct, reason: _ } => {
                position.remaining_pct -= pct;
                if !position.tp1_done {
                    position.tp1_done = true;
                } else {
                    position.tp2_done = true;
                }
            }
            TradeAction::UpdateTrailingStop { new_sl } => {
                position.current_sl = *new_sl;
            }
            TradeAction::FullClose { .. } | TradeAction::None => {}
        }
    }
}
