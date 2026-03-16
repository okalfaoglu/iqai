//! Geçmiş mum verisi üzerinde Q-Setup tarama ve sermaye simülasyonu (backtest).

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::signal::{CandleBuffer, SignalEngine};
use crate::trade_manager::calculate_position_size;
use crate::types::{Candle, QSetup, SignalType, Timeframe};

/// Geçmiş `candles` (tek zaman dilimi) üzerinde bar bar Q-Setup tara.
/// Her bar'da buffer = candles[..=i] ile engine çalıştırılır; setup bulunursa (bar_index, setup) döner.
///
/// Örnek: İstatistik veya grafik için tüm geçmiş Q-setup'ları almak.
pub fn scan_historical_q_setups(
    candles: &[Candle],
    config: &Config,
    chart_tf: Timeframe,
    symbol: &str,
) -> Vec<(usize, QSetup)> {
    let engine = SignalEngine::new(config.clone());
    let min_len = (config.pivot_length * 4 + 50) as usize;
    if candles.len() < min_len {
        return vec![];
    }

    let mut out = Vec::new();
    let mut buffer = CandleBuffer::new();

    for i in (min_len - 1)..candles.len() {
        let slice: Vec<Candle> = candles[..=i].to_vec();
        buffer.update(chart_tf, slice);
        if let Some(setup) = engine.compute_q_setup(&buffer, chart_tf, symbol, None) {
            out.push((i, setup));
        }
    }

    out
}

/// Tek işlem kaydı (backtest çıktısı).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    pub entry_bar: usize,
    pub exit_bar: usize,
    pub side: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub qty: f64,
    pub pnl: f64,
    pub pnl_r: f64,
    pub exit_reason: String,
    pub q_score: f64,
}

/// Backtest sonucu: başlangıç sermayesi, gün sonu (dönem sonu) sermayesi, getiri, işlemler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub initial_capital: f64,
    pub final_capital: f64,
    pub total_return_pct: f64,
    pub trades: Vec<BacktestTrade>,
    pub win_count: usize,
    pub loss_count: usize,
    pub win_rate_pct: f64,
    pub total_pnl: f64,
    pub symbol: String,
    pub timeframe: String,
    pub bar_count: usize,
}

/// Açık pozisyon (backtest içinde).
struct OpenPosition {
    entry_bar: usize,
    entry_price: f64,
    stop_loss: f64,
    take_profit: f64,
    qty: f64,
    is_long: bool,
    risk_r: f64,
    q_score: f64,
}

/// Geçmiş mumlar üzerinde Q-Setup ile sermaye simülasyonu.
///
/// - Her bar'da sadece **chart_tf** kullanılır (çoklu TF verisi yok; trend tek TF'den).
/// - Sinyal barında giriş = o barın **kapanışı** (look-ahead yok).
/// - Aynı anda en fazla **bir** pozisyon; kapanınca yeni sinyal aranır.
/// - SL/TP: bar içinde önce **SL** kontrol edilir (long: low <= sl → çıkış, short: high >= sl → çıkış), sonra TP.
///
/// # Parametreler
/// - `candles`: Tek timeframe OHLCV.
/// - `initial_capital`: Başlangıç sermayesi (örn. 10_000).
/// - `risk_pct_per_trade`: İşlem başına risk (%); örn. 1.0 = %1.
/// - `leverage`: Maksimum kaldıraç (pozisyon büyüklüğü sınırı); örn. 10.
pub fn run_backtest(
    candles: &[Candle],
    config: &Config,
    chart_tf: Timeframe,
    symbol: &str,
    initial_capital: f64,
    risk_pct_per_trade: f64,
    leverage: f64,
) -> BacktestResult {
    let min_len = (config.pivot_length * 4 + 50) as usize;
    let mut trades = Vec::new();
    let mut balance = initial_capital;
    let mut position: Option<OpenPosition> = None;
    let engine = SignalEngine::new(config.clone());

    if candles.len() < min_len {
        return BacktestResult {
            initial_capital,
            final_capital: balance,
            total_return_pct: 0.0,
            trades,
            win_count: 0,
            loss_count: 0,
            win_rate_pct: 0.0,
            total_pnl: 0.0,
            symbol: symbol.to_string(),
            timeframe: chart_tf.to_binance_interval().to_string(),
            bar_count: candles.len(),
        };
    }

    let mut buffer = CandleBuffer::new();

    for i in (min_len - 1)..candles.len() {
        let slice: Vec<Candle> = candles[..=i].to_vec();
        buffer.update(chart_tf, slice);
        let c = &candles[i];

        // 1) Açık pozisyon varsa bu bar'da SL/TP kontrol et
        if let Some(pos) = position.take() {
            let (exit_price, reason) = if pos.is_long {
                if c.low <= pos.stop_loss {
                    (pos.stop_loss, "SL")
                } else if c.high >= pos.take_profit {
                    (pos.take_profit, "TP")
                } else {
                    position = Some(pos);
                    continue;
                }
            } else {
                if c.high >= pos.stop_loss {
                    (pos.stop_loss, "SL")
                } else if c.low <= pos.take_profit {
                    (pos.take_profit, "TP")
                } else {
                    position = Some(pos);
                    continue;
                }
            };

            let pnl = if pos.is_long {
                (exit_price - pos.entry_price) * pos.qty
            } else {
                (pos.entry_price - exit_price) * pos.qty
            };
            let pnl_r = if pos.risk_r > 1e-10 {
                pnl / (pos.risk_r * pos.qty)
            } else {
                0.0
            };
            balance += pnl;
            trades.push(BacktestTrade {
                entry_bar: pos.entry_bar,
                exit_bar: i,
                side: if pos.is_long { "LONG".into() } else { "SHORT".into() },
                entry_price: pos.entry_price,
                exit_price,
                qty: pos.qty,
                pnl,
                pnl_r,
                exit_reason: reason.to_string(),
                q_score: pos.q_score,
            });
            continue;
        }

        // 2) Pozisyon yoksa yeni Q-Setup ara
        if let Some(setup) = engine.compute_q_setup(&buffer, chart_tf, symbol, None) {
            let is_long = matches!(
                setup.side,
                SignalType::Buy | SignalType::ChochBuy | SignalType::BosBuy
            );
            let entry_price = c.close;
            let sl = setup.stop_loss;
            let tp = setup.take_profit;
            let risk_r = (entry_price - sl).abs().max(1e-10);
            let qty = calculate_position_size(balance, risk_pct_per_trade, entry_price, sl, leverage);
            if qty >= 1e-10 {
                position = Some(OpenPosition {
                    entry_bar: i,
                    entry_price,
                    stop_loss: sl,
                    take_profit: tp,
                    qty,
                    is_long,
                    risk_r,
                    q_score: setup.q_score,
                });
            }
        }
    }

    let win_count = trades.iter().filter(|t| t.pnl > 0.0).count();
    let loss_count = trades.iter().filter(|t| t.pnl <= 0.0).count();
    let total_pnl = balance - initial_capital;
    let total_return_pct = if initial_capital > 1e-10 {
        (total_pnl / initial_capital) * 100.0
    } else {
        0.0
    };
    let win_rate_pct = if trades.is_empty() {
        0.0
    } else {
        (win_count as f64 / trades.len() as f64) * 100.0
    };

    BacktestResult {
        initial_capital,
        final_capital: balance,
        total_return_pct,
        trades,
        win_count,
        loss_count,
        win_rate_pct,
        total_pnl,
        symbol: symbol.to_string(),
        timeframe: chart_tf.to_binance_interval().to_string(),
        bar_count: candles.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candle;

    fn make_candle(t: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Candle {
        Candle { time: t, open: o, high: h, low: l, close: c, volume: v }
    }

    /// Az veri: min_len altında boş döner.
    #[test]
    fn backtest_returns_empty_for_insufficient_candles() {
        let config = Config::default();
        let min_len = (config.pivot_length * 4 + 50) as usize;
        let candles: Vec<Candle> = (0..min_len.saturating_sub(1))
            .map(|i| make_candle(i as i64 * 60_000, 100.0, 101.0, 99.0, 100.5, 1000.0))
            .collect();
        let out = scan_historical_q_setups(&candles, &config, Timeframe::M5, "TEST");
        assert!(out.is_empty());
    }

    /// Yeterli bar sayısında fonksiyon çalışır (setup bulunmasa da panic yok).
    #[test]
    fn backtest_runs_without_panic_for_minimum_bars() {
        let config = Config::default();
        let min_len = (config.pivot_length * 4 + 50) as usize;
        let candles: Vec<Candle> = (0..min_len + 10)
            .map(|i| make_candle(i as i64 * 60_000, 100.0, 101.0, 99.0, 100.5, 1000.0))
            .collect();
        let out = scan_historical_q_setups(&candles, &config, Timeframe::M5, "TEST");
        assert!(out.len() <= candles.len());
    }

    /// run_backtest yeterli bar ile çalışır; sentetik veride işlem çıkmayabilir.
    #[test]
    fn run_backtest_executes_without_panic() {
        let config = Config::default();
        let min_len = (config.pivot_length * 4 + 50) as usize;
        let candles: Vec<Candle> = (0..min_len + 100)
            .map(|i| make_candle(i as i64 * 60_000, 100.0, 101.0, 99.0, 100.5, 1000.0))
            .collect();
        let result = run_backtest(&candles, &config, Timeframe::M5, "TEST", 10_000.0, 1.0, 10.0);
        assert_eq!(result.bar_count, candles.len());
        assert!(result.initial_capital == 10_000.0);
        assert!(result.final_capital >= 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candle;

    fn make_candle(t: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Candle {
        Candle { time: t, open: o, high: h, low: l, close: c, volume: v }
    }

    /// Az veri: min_len altında boş döner.
    #[test]
    fn backtest_returns_empty_for_insufficient_candles() {
        let config = Config::default();
        let min_len = (config.pivot_length * 4 + 50) as usize;
        let candles: Vec<Candle> = (0..min_len.saturating_sub(1))
            .map(|i| make_candle(i as i64 * 60_000, 100.0, 101.0, 99.0, 100.5, 1000.0))
            .collect();
        let out = scan_historical_q_setups(&candles, &config, Timeframe::M5, "TEST");
        assert!(out.is_empty());
    }

    /// Yeterli bar sayısında fonksiyon çalışır (setup bulunmasa da panic yok).
    #[test]
    fn backtest_runs_without_panic_for_minimum_bars() {
        let config = Config::default();
        let min_len = (config.pivot_length * 4 + 50) as usize;
        let candles: Vec<Candle> = (0..min_len + 10)
            .map(|i| make_candle(i as i64 * 60_000, 100.0, 101.0, 99.0, 100.5, 1000.0))
            .collect();
        let out = scan_historical_q_setups(&candles, &config, Timeframe::M5, "TEST");
        // Sentetik düz veride setup çıkmayabilir; sadece çalıştığını doğrula
        assert!(out.len() <= candles.len());
    }
}
