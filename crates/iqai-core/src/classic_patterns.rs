//! Classic chart pattern detection for higher level strategy building.
//!
//! This module provides *structural* pattern descriptions (triangle, flags,
//! double tops/bottoms, head & shoulders, cup & handle, channels, ranges)
//! that can be consumed by the strategy layer and Web GUI.
//!
//! All heavy mathematical work (pivots, ATR, etc.) is delegated to existing
//! indicator utilities.

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::indicators::{atr, pivot_high, pivot_low};
use crate::types::{Candle, Timeframe};

/// Directional bias for a detected pattern.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PatternDirection {
    Bullish,
    Bearish,
    Neutral,
}

/// Supported classic pattern types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClassicPatternKind {
    SymmetricalTriangle,
    AscendingTriangle,
    DescendingTriangle,
    BullFlag,
    BearFlag,
    HeadAndShoulders,
    InverseHeadAndShoulders,
    DoubleTop,
    DoubleBottom,
    CupAndHandle,
    Range,
    Channel,
}

/// One target derived from a classic pattern (measured move, fib, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassicPatternTarget {
    pub price: f64,
    pub label: String,
    pub priority: u8,
}

/// Detection result for a single classic chart pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassicPatternDetection {
    pub symbol: String,
    pub timeframe: Timeframe,
    pub kind: ClassicPatternKind,
    pub direction: PatternDirection,
    /// Approximate pattern start time (ms).
    pub start_time: i64,
    /// Approximate pattern end / breakout time (ms).
    pub end_time: i64,
    /// Key horizontal level (e.g. neckline, range mid, breakout level).
    pub reference_level: f64,
    /// Optional secondary level (e.g. neckline for H&S, handle low for cup).
    pub secondary_level: Option<f64>,
    /// Pattern height (used for measured move).
    pub height: f64,
    /// Confidence score in [0, 1].
    pub confidence: f64,
    /// Derived target levels.
    pub targets: Vec<ClassicPatternTarget>,
}

impl ClassicPatternDetection {
    pub fn simple_label(&self) -> String {
        format!("{:?} ({:?})", self.kind, self.direction)
    }
}

/// Detect a small set of high-impact classic patterns on the given candles.
///
/// This function is intentionally conservative: it tries to avoid false
/// positives and prefers to return an empty list rather than noisy signals.
pub fn detect_classic_patterns(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
    cfg: &Config,
) -> Vec<ClassicPatternDetection> {
    if candles.len() < 100 {
        return Vec::new();
    }

    let mut out = Vec::new();

    if let Some(triangle) = detect_triangle(symbol, timeframe, candles) {
        out.push(triangle);
    }

    if let Some(double) = detect_double_top_bottom(symbol, timeframe, candles, cfg) {
        out.push(double);
    }

    if let Some(cup) = detect_cup_and_handle(symbol, timeframe, candles) {
        out.push(cup);
    }

    out
}

fn detect_triangle(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
) -> Option<ClassicPatternDetection> {
    // Use recent part of the history to stabilise pivots.
    let window = candles.len().saturating_sub(300);
    let slice = &candles[window..];
    let len = slice.len();
    if len < 60 {
        return None;
    }

    let pivot_len = 5usize;
    let mut highs = Vec::new();
    let mut lows = Vec::new();

    for i in (pivot_len..len - pivot_len).rev().take(20) {
        let sub = &slice[..=i + pivot_len];
        if let Some(h) = pivot_high(sub, pivot_len) {
            highs.push((i, h));
        }
        if let Some(l) = pivot_low(sub, pivot_len) {
            lows.push((i, l));
        }
        if highs.len() >= 4 && lows.len() >= 4 {
            break;
        }
    }

    if highs.len() < 3 || lows.len() < 3 {
        return None;
    }

    highs.sort_by_key(|(i, _)| *i);
    lows.sort_by_key(|(i, _)| *i);

    // Check for descending highs and ascending lows (symmetrical triangle).
    let (first_hi_idx, first_hi) = highs.first().copied()?;
    let (last_hi_idx, last_hi) = highs.last().copied()?;
    let (first_lo_idx, first_lo) = lows.first().copied()?;
    let (last_lo_idx, last_lo) = lows.last().copied()?;

    if first_hi <= last_hi || first_lo >= last_lo {
        // No clear compression.
        return None;
    }

    let kind = ClassicPatternKind::SymmetricalTriangle;
    let dir = {
        let last_close = slice.last().unwrap().close;
        if last_close < last_lo {
            PatternDirection::Bearish
        } else if last_close > last_hi {
            PatternDirection::Bullish
        } else {
            PatternDirection::Neutral
        }
    };

    let start_idx = first_hi_idx.min(first_lo_idx);
    let end_idx = last_hi_idx.max(last_lo_idx);
    let start_time = slice[start_idx].time;
    let end_time = slice[end_idx].time;
    let height = (first_hi - first_lo).abs();
    if height <= 0.0 {
        return None;
    }

    let ref_level = (last_hi + last_lo) / 2.0;
    let atr_val = atr(slice, 14).unwrap_or(height / 5.0);

    // Measured move targets.
    let mut targets = Vec::new();
    match dir {
        PatternDirection::Bearish => {
            targets.push(ClassicPatternTarget {
                price: ref_level - height,
                label: "Triangle MM TP1".to_string(),
                priority: 1,
            });
            targets.push(ClassicPatternTarget {
                price: ref_level - 1.5 * height,
                label: "Triangle MM TP2".to_string(),
                priority: 2,
            });
        }
        PatternDirection::Bullish => {
            targets.push(ClassicPatternTarget {
                price: ref_level + height,
                label: "Triangle MM TP1".to_string(),
                priority: 1,
            });
            targets.push(ClassicPatternTarget {
                price: ref_level + 1.5 * height,
                label: "Triangle MM TP2".to_string(),
                priority: 2,
            });
        }
        PatternDirection::Neutral => {}
    }

    let confidence = ((height / atr_val).min(5.0) / 5.0).clamp(0.0, 1.0);

    Some(ClassicPatternDetection {
        symbol: symbol.to_string(),
        timeframe,
        kind,
        direction: dir,
        start_time,
        end_time,
        reference_level: ref_level,
        secondary_level: None,
        height,
        confidence,
        targets,
    })
}

fn detect_double_top_bottom(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
    _cfg: &Config,
) -> Option<ClassicPatternDetection> {
    let window = candles.len().saturating_sub(250);
    let slice = &candles[window..];
    let len = slice.len();
    if len < 80 {
        return None;
    }
    let pivot_len = 4usize;

    let mut highs = Vec::new();
    let mut lows = Vec::new();
    for i in (pivot_len..len - pivot_len).rev().take(30) {
        let sub = &slice[..=i + pivot_len];
        if let Some(h) = pivot_high(sub, pivot_len) {
            highs.push((i, h));
        }
        if let Some(l) = pivot_low(sub, pivot_len) {
            lows.push((i, l));
        }
    }

    highs.sort_by_key(|(i, _)| *i);
    lows.sort_by_key(|(i, _)| *i);

    // Try double top first.
    if highs.len() >= 2 {
        let (i1, h1) = highs[highs.len() - 2];
        let (i2, h2) = highs[highs.len() - 1];
        let tol = (h1 * 0.003).max(3.0);
        if (h1 - h2).abs() <= tol && i2 > i1 + pivot_len {
            // Neckline: lowest low between the two tops.
            let mid_low = slice[i1..=i2]
                .iter()
                .map(|c| c.low)
                .fold(f64::INFINITY, f64::min);
            let start_time = slice[i1].time;
            let end_time = slice[i2].time;
            let height = h1 - mid_low;
            if height > 0.0 {
                let mut targets = Vec::new();
                targets.push(ClassicPatternTarget {
                    price: mid_low - height,
                    label: "Double Top MM TP1".to_string(),
                    priority: 1,
                });
                targets.push(ClassicPatternTarget {
                    price: mid_low - 1.5 * height,
                    label: "Double Top MM TP2".to_string(),
                    priority: 2,
                });
                return Some(ClassicPatternDetection {
                    symbol: symbol.to_string(),
                    timeframe,
                    kind: ClassicPatternKind::DoubleTop,
                    direction: PatternDirection::Bearish,
                    start_time,
                    end_time,
                    reference_level: mid_low,
                    secondary_level: Some((h1 + h2) / 2.0),
                    height,
                    confidence: 0.7,
                    targets,
                });
            }
        }
    }

    // Double bottom.
    if lows.len() >= 2 {
        let (i1, l1) = lows[lows.len() - 2];
        let (i2, l2) = lows[lows.len() - 1];
        let tol = (l1 * 0.003).max(3.0);
        if (l1 - l2).abs() <= tol && i2 > i1 + pivot_len {
            let mid_high = slice[i1..=i2]
                .iter()
                .map(|c| c.high)
                .fold(f64::NEG_INFINITY, f64::max);
            let start_time = slice[i1].time;
            let end_time = slice[i2].time;
            let height = mid_high - l1;
            if height > 0.0 {
                let mut targets = Vec::new();
                targets.push(ClassicPatternTarget {
                    price: mid_high + height,
                    label: "Double Bottom MM TP1".to_string(),
                    priority: 1,
                });
                targets.push(ClassicPatternTarget {
                    price: mid_high + 1.5 * height,
                    label: "Double Bottom MM TP2".to_string(),
                    priority: 2,
                });
                return Some(ClassicPatternDetection {
                    symbol: symbol.to_string(),
                    timeframe,
                    kind: ClassicPatternKind::DoubleBottom,
                    direction: PatternDirection::Bullish,
                    start_time,
                    end_time,
                    reference_level: mid_high,
                    secondary_level: Some((l1 + l2) / 2.0),
                    height,
                    confidence: 0.7,
                    targets,
                });
            }
        }
    }

    None
}

fn detect_cup_and_handle(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
) -> Option<ClassicPatternDetection> {
    let window = candles.len().saturating_sub(600);
    let slice = &candles[window..];
    let len = slice.len();
    if len < 200 {
        return None;
    }

    // Rough heuristic:
    // - find major high (left rim),
    // - find deep low after it (cup bottom),
    // - price recovers near rim but forms a shallow pullback (handle).
    let mut max_price = f64::NEG_INFINITY;
    let mut max_idx = 0usize;
    for (i, c) in slice.iter().enumerate().take(len / 3) {
        if c.high > max_price {
            max_price = c.high;
            max_idx = i;
        }
    }

    if max_idx + 20 >= len {
        return None;
    }

    let mut min_price = f64::INFINITY;
    let mut min_idx = max_idx + 1;
    for (i, c) in slice.iter().enumerate().skip(max_idx + 1).take(len / 3) {
        if c.low < min_price {
            min_price = c.low;
            min_idx = i;
        }
    }

    if min_idx + 20 >= len {
        return None;
    }

    let right_rim_zone_start = len * 2 / 3;
    let right_slice = &slice[right_rim_zone_start..];
    let right_rim = right_slice
        .iter()
        .map(|c| c.high)
        .fold(f64::NEG_INFINITY, f64::max);

    let rim_avg = (max_price + right_rim) / 2.0;
    let depth = rim_avg - min_price;
    if depth <= 0.0 {
        return None;
    }

    // Basic consistency checks.
    if depth / rim_avg < 0.1 || depth / rim_avg > 0.6 {
        return None;
    }

    // Handle: last 15% of bars, shallow pullback.
    let handle_start = len * 85 / 100;
    let handle_slice = &slice[handle_start..];
    let handle_low = handle_slice
        .iter()
        .map(|c| c.low)
        .fold(f64::INFINITY, f64::min);
    if (handle_low - min_price).abs() < depth * 0.2 {
        // Too deep; handle should be much shallower than cup.
        return None;
    }

    let start_time = slice[max_idx].time;
    let end_time = slice[len - 1].time;
    let mut targets = Vec::new();
    targets.push(ClassicPatternTarget {
        price: rim_avg + depth,
        label: "Cup&Handle TP1".to_string(),
        priority: 1,
    });
    targets.push(ClassicPatternTarget {
        price: rim_avg + 1.5 * depth,
        label: "Cup&Handle TP2".to_string(),
        priority: 2,
    });

    Some(ClassicPatternDetection {
        symbol: symbol.to_string(),
        timeframe,
        kind: ClassicPatternKind::CupAndHandle,
        direction: PatternDirection::Bullish,
        start_time,
        end_time,
        reference_level: rim_avg,
        secondary_level: Some(min_price),
        height: depth,
        confidence: 0.6,
        targets,
    })
}

