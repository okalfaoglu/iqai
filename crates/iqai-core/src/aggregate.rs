//! Candle aggregation - convert 1M to 5M, 15M, etc.

use crate::types::{Candle, Timeframe};

/// Aggregate smaller timeframe candles into larger timeframe
/// e.g. 5 x 1M candles -> 1 x 5M candle
pub fn aggregate_candles(candles: &[Candle], target_tf: Timeframe) -> Vec<Candle> {
    if candles.is_empty() {
        return vec![];
    }
    let target_mins = target_tf.minutes() as i64;
    let mut result = Vec::new();
    let mut i = 0usize;

    while i < candles.len() {
        let start_ts = candles[i].time;
        let bucket_start = (start_ts / (target_mins * 60_000)) * (target_mins * 60_000);
        let mut agg = Candle {
            time: bucket_start,
            open: candles[i].open,
            high: candles[i].high,
            low: candles[i].low,
            close: candles[i].close,
            volume: candles[i].volume,
        };
        i += 1;
        while i < candles.len() {
            let ts = candles[i].time;
            let bucket = (ts / (target_mins * 60_000)) * (target_mins * 60_000);
            if bucket != bucket_start {
                break;
            }
            agg.high = agg.high.max(candles[i].high);
            agg.low = agg.low.min(candles[i].low);
            agg.close = candles[i].close;
            agg.volume += candles[i].volume;
            i += 1;
        }
        result.push(agg);
    }
    result
}
