//! Geçmiş mum verisi üzerinde Q-Setup tarama (backtest stub).

use crate::config::Config;
use crate::signal::{CandleBuffer, SignalEngine};
use crate::types::{Candle, QSetup, Timeframe};

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
