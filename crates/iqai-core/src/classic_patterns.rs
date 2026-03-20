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
use crate::elliott_detector::collect_swings;
use crate::indicators::{atr, bollinger, rsi, pivot_high, pivot_low};
use crate::types::{Candle, Timeframe};

/// One point used for frontend drawing / hit-testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassicDrawPoint {
    /// Unix epoch timestamp in milliseconds.
    pub time: i64,
    pub price: f64,
    /// True if this pivot is a high pivot, otherwise a low pivot.
    pub is_high: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassicDrawLine {
    pub t1: i64,
    pub p1: f64,
    pub t2: i64,
    pub p2: f64,
}

/// Drawing data for "Trendoscope-like" visualization (pivot zigzag + boundary lines).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassicDrawData {
    /// Zigzag/pivot points in chronological order.
    pub zigzag_points: Vec<ClassicDrawPoint>,
    /// Upper boundary trendline segment (trendline1/centered variant).
    pub upper_line: Option<ClassicDrawLine>,
    /// Lower boundary trendline segment (trendline2 variant).
    pub lower_line: Option<ClassicDrawLine>,
    /// Optional middle/neck line (usually reference_level).
    pub center_line: Option<ClassicDrawLine>,
}

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
    /// Confidence score in [0, 1] (pattern içsel kalitesi).
    pub confidence: f64,
    /// Composite quality score in [0, 10] (7 maddelik engine sonucu).
    pub quality_score: f64,
    /// Pattern invalidation (kill switch) price.
    pub invalidation_level: Option<f64>,
    /// Derived target levels.
    pub targets: Vec<ClassicPatternTarget>,

    /// Optional drawing geometry for the Web GUI (zigzag pivots + boundary lines).
    pub draw: Option<ClassicDrawData>,
}

impl ClassicPatternDetection {
    pub fn simple_label(&self) -> String {
        format!("{:?} ({:?})", self.kind, self.direction)
    }
}

fn compute_quality_score(
    has_pattern_geometry: bool,
    fibo_ok: bool,
    rsi_div: bool,
    volume_breakout: bool,
    trend_aligned: bool,
    atr_expansion: bool,
) -> f64 {
    let mut score: f64 = 0.0;
    if has_pattern_geometry {
        score += 2.0;
    }
    if fibo_ok {
        score += 2.0;
    }
    if rsi_div {
        score += 2.0;
    }
    if volume_breakout {
        score += 2.0;
    }
    if trend_aligned {
        score += 1.0;
    }
    if atr_expansion {
        score += 1.0;
    }
    score.clamp(0.0, 10.0)
}

fn collect_pivot_candidates(candles: &[Candle], pivot_len: usize) -> Vec<(usize, f64, bool)> {
    // bool == true => pivot high, false => pivot low
    if candles.len() < pivot_len * 2 + 1 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in pivot_len..(candles.len() - pivot_len) {
        let sub = &candles[i - pivot_len..=i + pivot_len];
        if let Some(h) = pivot_high(sub, pivot_len) {
            out.push((i, h, true));
            continue;
        }
        if let Some(l) = pivot_low(sub, pivot_len) {
            out.push((i, l, false));
        }
    }
    out
}

fn build_alternating_zigzag_from_candidates(
    candidates: &[(usize, f64, bool)],
    desired: usize,
) -> Vec<(usize, f64, bool)> {
    if candidates.len() < 3 {
        return Vec::new();
    }
    let mut res_rev: Vec<(usize, f64, bool)> = Vec::new();
    let mut last_type: Option<bool> = None;
    for &(idx, price, is_high) in candidates.iter().rev() {
        if res_rev.is_empty() {
            res_rev.push((idx, price, is_high));
            last_type = Some(is_high);
        } else if last_type.unwrap() != is_high {
            res_rev.push((idx, price, is_high));
            last_type = Some(is_high);
        }
        if res_rev.len() >= desired {
            break;
        }
    }
    res_rev.reverse();
    if res_rev.len() >= 3 { res_rev } else { Vec::new() }
}

fn interpolate_price(t1: i64, p1: f64, t2: i64, p2: f64, target_t: i64) -> f64 {
    if t1 == t2 {
        return p1;
    }
    let slope = (p2 - p1) / (t2 - t1) as f64;
    p1 + slope * (target_t - t1) as f64
}

fn build_draw_data(
    candles_range: &[Candle],
    pivot_len: usize,
    desired_points: usize,
    reference_level: f64,
    global_start_time: i64,
    global_end_time: i64,
) -> Option<ClassicDrawData> {
    let cands = collect_pivot_candidates(candles_range, pivot_len);
    if cands.is_empty() {
        return None;
    }

    // Trendoscope'ta numberOfPivots=5 halinde points array'i p2..p6 kullanır (yani ilk pivot atlanır).
    // Bizde desired_points=5 için bu "skip first pivot" hissini birebir yakalamak adına
    // önce bir pivot fazla üretip ilkini drop ediyoruz.
    let build_points = if desired_points == 5 { desired_points + 1 } else { desired_points };
    let mut zig = build_alternating_zigzag_from_candidates(&cands, build_points);
    if zig.len() < 3 {
        return None;
    }

    if desired_points == 5 && zig.len() >= 6 {
        zig = zig.into_iter().skip(1).take(5).collect();
    }

    if zig.len() < desired_points {
        // Yeterli pivot bulunamadıysa çizimi basitleştirip yine de ilerleyelim.
        // (front-end'de zaten length>=3 kontrolü var.)
    }

    let zigzag_points: Vec<ClassicDrawPoint> = zig
        .iter()
        .map(|(idx, price, is_high)| ClassicDrawPoint {
            time: candles_range[*idx].time,
            price: *price,
            is_high: *is_high,
        })
        .collect();

    let t_start = global_start_time;
    let t_end = global_end_time;
    if t_start >= t_end {
        return None;
    }

    // Classify upper/lower from reference level.
    let mut upper_pts: Vec<&ClassicDrawPoint> = zigzag_points
        .iter()
        .filter(|p| p.price >= reference_level)
        .collect();
    let mut lower_pts: Vec<&ClassicDrawPoint> = zigzag_points
        .iter()
        .filter(|p| p.price < reference_level)
        .collect();

    // Fallback if classification is degenerate.
    if upper_pts.len() < 2 {
        if let (Some(maxp), Some(minp)) = (
            zigzag_points.iter().max_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal)),
            zigzag_points.iter().min_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal)),
        ) {
            upper_pts = vec![maxp, minp];
        }
    }
    if lower_pts.len() < 2 {
        if let (Some(minp), Some(maxp)) = (
            zigzag_points.iter().min_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal)),
            zigzag_points.iter().max_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal)),
        ) {
            lower_pts = vec![minp, maxp];
        }
    }

    let first_upper = upper_pts.first().unwrap();
    let last_upper = upper_pts.last().unwrap();
    let upper_p_start = interpolate_price(first_upper.time, first_upper.price, last_upper.time, last_upper.price, t_start);
    let upper_p_end = interpolate_price(first_upper.time, first_upper.price, last_upper.time, last_upper.price, t_end);

    let first_lower = lower_pts.first().unwrap();
    let last_lower = lower_pts.last().unwrap();
    let lower_p_start = interpolate_price(first_lower.time, first_lower.price, last_lower.time, last_lower.price, t_start);
    let lower_p_end = interpolate_price(first_lower.time, first_lower.price, last_lower.time, last_lower.price, t_end);

    let upper_line = Some(ClassicDrawLine { t1: t_start, p1: upper_p_start, t2: t_end, p2: upper_p_end });
    let lower_line = Some(ClassicDrawLine { t1: t_start, p1: lower_p_start, t2: t_end, p2: lower_p_end });
    let center_line = Some(ClassicDrawLine { t1: t_start, p1: reference_level, t2: t_end, p2: reference_level });

    Some(ClassicDrawData { zigzag_points, upper_line, lower_line, center_line })
}

fn build_draw_data_from_swings(
    swings: &[(i64, f64, bool)],
    desired_points: usize,
    reference_level: f64,
    start_time: i64,
    end_time: i64,
) -> Option<ClassicDrawData> {
    if desired_points < 3 {
        return None;
    }
    if start_time >= end_time {
        return None;
    }
    let window: Vec<_> = swings
        .iter()
        .filter(|(t, _, _)| *t >= start_time && *t <= end_time)
        .cloned()
        .collect();
    if window.len() < desired_points {
        return None;
    }

    // Trendoscope-like: numberOfPivots=5 => points array p2..p6 (first pivot dropped).
    let mut sel: Vec<(i64, f64, bool)> = if desired_points == 5 && window.len() >= desired_points + 1 {
        window[window.len() - (desired_points + 1)..].to_vec()
    } else {
        window[window.len() - desired_points..].to_vec()
    };
    if desired_points == 5 && sel.len() >= desired_points + 1 {
        sel = sel.into_iter().skip(1).take(desired_points).collect();
    } else if sel.len() > desired_points {
        sel = sel.into_iter().take(desired_points).collect();
    }

    if sel.len() < 3 {
        return None;
    }

    let zigzag_points: Vec<ClassicDrawPoint> = sel
        .iter()
        .map(|(t, p, is_high)| ClassicDrawPoint { time: *t, price: *p, is_high: *is_high })
        .collect();

    if zigzag_points.len() != desired_points {
        return None;
    }

    // Trendoscope-like:
    // - For numberOfPivots=5 => points array behaves like p2..p6 (size 5)
    // - Two trendlines are approximated using endpoints:
    //     lineA: pt0 -> pt4
    //     lineB: pt1 -> pt3
    //   Then choose which one is "upper" at the left edge.
    let t_left = zigzag_points.first()?.time;
    let t_right = zigzag_points.last()?.time;
    if t_left >= t_right {
        return None;
    }

    let p0 = &zigzag_points[0];
    let p1 = &zigzag_points[1];
    let p3 = &zigzag_points[3];
    let p4 = &zigzag_points[4];

    let y_on = |a_t: i64, a_p: f64, b_t: i64, b_p: f64, t: i64| -> f64 {
        if a_t == b_t {
            a_p
        } else {
            let slope = (b_p - a_p) / (b_t - a_t) as f64;
            a_p + slope * (t - a_t) as f64
        }
    };

    let lineA_p_left = y_on(p0.time, p0.price, p4.time, p4.price, t_left);
    let lineB_p_left = y_on(p1.time, p1.price, p3.time, p3.price, t_left);

    let (upper_line, lower_line) = if lineA_p_left >= lineB_p_left {
        (
            Some(ClassicDrawLine { t1: t_left, p1: lineA_p_left, t2: t_right, p2: y_on(p0.time, p0.price, p4.time, p4.price, t_right) }),
            Some(ClassicDrawLine { t1: t_left, p1: lineB_p_left, t2: t_right, p2: y_on(p1.time, p1.price, p3.time, p3.price, t_right) }),
        )
    } else {
        (
            Some(ClassicDrawLine { t1: t_left, p1: lineB_p_left, t2: t_right, p2: y_on(p1.time, p1.price, p3.time, p3.price, t_right) }),
            Some(ClassicDrawLine { t1: t_left, p1: lineA_p_left, t2: t_right, p2: y_on(p0.time, p0.price, p4.time, p4.price, t_right) }),
        )
    };

    let center_line = Some(ClassicDrawLine { t1: t_left, p1: reference_level, t2: t_right, p2: reference_level });

    Some(ClassicDrawData { zigzag_points, upper_line, lower_line, center_line })
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
    if candles.len() < 80 {
        return Vec::new();
    }

    let mut out = Vec::new();

    // Build Elliott swing structure once, then align classical draw geometry on top of it.
    // This makes the 1..N pivot numbering visually closer to Trendoscope-like drawings.
    let pivot_len = cfg.pivot_length as usize;
    let swings = collect_swings(candles, pivot_len);

    if let Some(triangle) = detect_triangle(symbol, timeframe, candles) {
        out.push(triangle);
    }

    if let Some(double) = detect_double_top_bottom(symbol, timeframe, candles, cfg) {
        out.push(double);
    }

    if let Some(cup) = detect_cup_and_handle(symbol, timeframe, candles) {
        out.push(cup);
    }

    if let Some(hs) = detect_head_and_shoulders(symbol, timeframe, candles) {
        out.push(hs);
    }

    if let Some(flag) = detect_flag(symbol, timeframe, candles) {
        out.push(flag);
    }

    if let Some(range) = detect_range(symbol, timeframe, candles) {
        out.push(range);
    }

    if let Some(channel) = detect_channel(symbol, timeframe, candles) {
        out.push(channel);
    }

    // Hiçbiri tutmazsa (çok sıkı heuristikler) volatilite sıkışması ile zayıf Range adayı üret;
    // GUI/API boş kalmasın, kalite skoru düşük olur.
    if out.is_empty() {
        if let Some(fallback) = detect_bollinger_compression_range(symbol, timeframe, candles) {
            out.push(fallback);
        }
    }

    // Override/align draw data using Elliott swings (Option A).
    for p in out.iter_mut() {
        if let Some(draw) = build_draw_data_from_swings(
            &swings,
            5,
            p.reference_level,
            p.start_time,
            p.end_time,
        ) {
            p.draw = Some(draw);
        }
    }

    out
}

/// Son bant genişliği daraldığında (Bollinger bandwidth düşük) yatay sıkışma adayı.
fn detect_bollinger_compression_range(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
) -> Option<ClassicPatternDetection> {
    let take = 100usize.min(candles.len());
    let slice = &candles[candles.len() - take..];
    if slice.len() < 40 {
        return None;
    }
    let closes: Vec<f64> = slice.iter().map(|c| c.close).collect();
    let (lower, middle, upper) = bollinger(&closes, 20, 2.0)?;
    if middle.abs() < 1e-12 {
        return None;
    }
    let bandwidth = (upper - lower) / middle;
    // Dar bant = sıkışma (kripto 5m için ~%6'ya kadar gevşek)
    if bandwidth > 0.06 {
        return None;
    }
    let height = upper - lower;
    if height <= 0.0 {
        return None;
    }
    let start_time = slice.first()?.time;
    let end_time = slice.last()?.time;
    let mut targets = Vec::new();
    targets.push(ClassicPatternTarget {
        price: upper,
        label: "Sıkışma üst (BB)".to_string(),
        priority: 1,
    });
    targets.push(ClassicPatternTarget {
        price: lower,
        label: "Sıkışma alt (BB)".to_string(),
        priority: 1,
    });
    let quality_score = compute_quality_score(true, true, false, false, true, false);
    Some(ClassicPatternDetection {
        symbol: symbol.to_string(),
        timeframe,
        kind: ClassicPatternKind::Range,
        direction: PatternDirection::Neutral,
        start_time,
        end_time,
        reference_level: middle,
        secondary_level: Some(upper),
        height,
        confidence: 0.35,
        quality_score,
        invalidation_level: Some(lower),
        targets,
        draw: build_draw_data(&slice, 2, 5, middle, start_time, end_time),
    })
}

fn avg_volume(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < period || period == 0 {
        return None;
    }
    let s: f64 = candles[candles.len() - period..].iter().map(|c| c.volume).sum();
    Some(s / period as f64)
}

fn has_breakout_volume(candles: &[Candle], period: usize, min_mult: f64) -> bool {
    if candles.len() < period + 1 {
        return false;
    }
    let last_vol = candles.last().map(|c| c.volume).unwrap_or(0.0);
    let avg = avg_volume(&candles[..candles.len() - 1], period).unwrap_or(0.0);
    avg > 0.0 && last_vol >= avg * min_mult
}

fn rsi_at(prices: &[f64], end_idx: usize, period: usize) -> Option<f64> {
    if end_idx + 1 < period + 1 {
        return None;
    }
    rsi(&prices[..=end_idx], period)
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

    // Check for compression and classify triangle type.
    let (first_hi_idx, first_hi) = highs.first().copied()?;
    let (last_hi_idx, last_hi) = highs.last().copied()?;
    let (first_lo_idx, first_lo) = lows.first().copied()?;
    let (last_lo_idx, last_lo) = lows.last().copied()?;

    if first_lo >= last_lo {
        // Lows must be flat or rising for valid triangle in our heuristic.
        return None;
    }

    let highs_down = last_hi < first_hi;
    let lows_up = last_lo > first_lo;
    if !highs_down && !lows_up {
        return None;
    }

    let kind = if (first_hi - last_hi).abs() / first_hi.max(1.0) < 0.01 && lows_up {
        ClassicPatternKind::AscendingTriangle
    } else if (first_lo - last_lo).abs() / first_lo.max(1.0) < 0.01 && highs_down {
        ClassicPatternKind::DescendingTriangle
    } else {
        ClassicPatternKind::SymmetricalTriangle
    };
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

    // Measured move targets (TP1/TP2/TP3).
    let mut targets = Vec::new();
    match dir {
        PatternDirection::Bearish => {
            targets.push(ClassicPatternTarget {
                price: ref_level - 0.618 * height,
                label: "Triangle TP1 (0.618H)".to_string(),
                priority: 1,
            });
            targets.push(ClassicPatternTarget {
                price: ref_level - height,
                label: "Triangle TP2 (1.0H)".to_string(),
                priority: 2,
            });
            targets.push(ClassicPatternTarget {
                price: ref_level - 1.272 * height,
                label: "Triangle TP3 (1.272H)".to_string(),
                priority: 3,
            });
        }
        PatternDirection::Bullish => {
            targets.push(ClassicPatternTarget {
                price: ref_level + 0.618 * height,
                label: "Triangle TP1 (0.618H)".to_string(),
                priority: 1,
            });
            targets.push(ClassicPatternTarget {
                price: ref_level + height,
                label: "Triangle TP2 (1.0H)".to_string(),
                priority: 2,
            });
            targets.push(ClassicPatternTarget {
                price: ref_level + 1.272 * height,
                label: "Triangle TP3 (1.272H)".to_string(),
                priority: 3,
            });
        }
        PatternDirection::Neutral => {}
    }

    let close_series: Vec<f64> = slice.iter().map(|c| c.close).collect();
    let rsi_val = rsi(&close_series, 14).unwrap_or(50.0);
    let vol_ok = has_breakout_volume(slice, 20, 1.2);
    let rsi_ok = match dir {
        PatternDirection::Bullish => rsi_val > 50.0,
        PatternDirection::Bearish => rsi_val < 50.0,
        PatternDirection::Neutral => false,
    };
    let base = ((height / atr_val).min(5.0) / 5.0).clamp(0.0, 1.0);
    let mut confidence = base * 0.6;
    if vol_ok {
        confidence += 0.2;
    }
    if rsi_ok {
        confidence += 0.2;
    }
    let confidence = confidence.clamp(0.0, 1.0);
    let atr_now = atr(slice, 14).unwrap_or(atr_val);
    let atr_expansion = atr_now > atr_val;
    let trend_aligned = match dir {
        PatternDirection::Bullish => slice.last().unwrap().close >= ref_level,
        PatternDirection::Bearish => slice.last().unwrap().close <= ref_level,
        PatternDirection::Neutral => false,
    };
    let quality_score = compute_quality_score(
        true,
        true,       // triangle height + breakout zaten fibo-friendly kabul
        rsi_ok,
        vol_ok,
        trend_aligned,
        atr_expansion,
    );

    let draw = build_draw_data(
        &slice[start_idx..=end_idx],
        3,
        5,
        ref_level,
        start_time,
        end_time,
    );

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
        quality_score,
        invalidation_level: Some(ref_level),
        targets,
        draw,
    })
}

fn detect_double_top_bottom(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
    cfg: &Config,
) -> Option<ClassicPatternDetection> {
    if let Some(family) = detect_extremum_family_pattern(symbol, timeframe, candles, cfg) {
        return Some(family);
    }

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

    let close_series: Vec<f64> = slice.iter().map(|c| c.close).collect();

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
            let depth_pct = (h1.max(h2) - mid_low) / h1.max(h2);
            let start_time = slice[i1].time;
            let end_time = slice[i2].time;
            let height = h1 - mid_low;
            if height > 0.0 && (0.03..=0.40).contains(&depth_pct) {
                let rsi1 = rsi_at(&close_series, i1, 14).unwrap_or(50.0);
                let rsi2 = rsi_at(&close_series, i2, 14).unwrap_or(50.0);
                let bearish_div = rsi2 < rsi1;
                let vol_ok = has_breakout_volume(slice, 20, 1.2);
                let mut targets = Vec::new();
                targets.push(ClassicPatternTarget {
                    price: mid_low - 0.618 * height,
                    label: "Double Top TP1 (0.618H)".to_string(),
                    priority: 1,
                });
                targets.push(ClassicPatternTarget {
                    price: mid_low - height,
                    label: "Double Top TP2 (1.0H)".to_string(),
                    priority: 2,
                });
                targets.push(ClassicPatternTarget {
                    price: mid_low - 1.272 * height,
                    label: "Double Top TP3 (1.272H)".to_string(),
                    priority: 3,
                });
                let mut confidence: f64 = 0.55;
                if bearish_div {
                    confidence += 0.2;
                }
                if vol_ok {
                    confidence += 0.15;
                }
                let fibo_ok = (0.38..=0.65).contains(&depth_pct);
                if (0.08..=0.25).contains(&depth_pct) {
                    confidence += 0.1;
                }
                let volume_breakout = vol_ok;
                let trend_aligned = slice.last().unwrap().close < mid_low;
                let atr_before = atr(slice, 14).unwrap_or(height / 4.0);
                let atr_after = atr(&slice[i2..], 14).unwrap_or(atr_before);
                let atr_expansion = atr_after > atr_before;
                let quality_score = compute_quality_score(
                    true,
                    fibo_ok,
                    bearish_div,
                    volume_breakout,
                    trend_aligned,
                    atr_expansion,
                );
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
                    confidence: confidence.clamp(0.0, 1.0),
                    quality_score,
                    invalidation_level: Some((h1 + h2) / 2.0),
                    targets,
                    draw: build_draw_data(
                        &slice[i1..=i2],
                        2,
                        5,
                        mid_low,
                        start_time,
                        end_time,
                    ),
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
            let depth_pct = (mid_high - l1.min(l2)) / mid_high;
            if height > 0.0 && (0.03..=0.40).contains(&depth_pct) {
                let rsi1 = rsi_at(&close_series, i1, 14).unwrap_or(50.0);
                let rsi2 = rsi_at(&close_series, i2, 14).unwrap_or(50.0);
                let bullish_div = rsi2 > rsi1;
                let vol_ok = has_breakout_volume(slice, 20, 1.2);
                let mut targets = Vec::new();
                targets.push(ClassicPatternTarget {
                    price: mid_high + 0.618 * height,
                    label: "Double Bottom TP1 (0.618H)".to_string(),
                    priority: 1,
                });
                targets.push(ClassicPatternTarget {
                    price: mid_high + height,
                    label: "Double Bottom TP2 (1.0H)".to_string(),
                    priority: 2,
                });
                targets.push(ClassicPatternTarget {
                    price: mid_high + 1.272 * height,
                    label: "Double Bottom TP3 (1.272H)".to_string(),
                    priority: 3,
                });
                let mut confidence: f64 = 0.55;
                if bullish_div {
                    confidence += 0.2;
                }
                if vol_ok {
                    confidence += 0.15;
                }
                let fibo_ok = (0.38..=0.65).contains(&depth_pct);
                if (0.08..=0.25).contains(&depth_pct) {
                    confidence += 0.1;
                }
                let volume_breakout = vol_ok;
                let trend_aligned = slice.last().unwrap().close > mid_high;
                let atr_before = atr(slice, 14).unwrap_or(height / 4.0);
                let atr_after = atr(&slice[i2..], 14).unwrap_or(atr_before);
                let atr_expansion = atr_after > atr_before;
                let quality_score = compute_quality_score(
                    true,
                    fibo_ok,
                    bullish_div,
                    volume_breakout,
                    trend_aligned,
                    atr_expansion,
                );
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
                    confidence: confidence.clamp(0.0, 1.0),
                    quality_score,
                    invalidation_level: Some((l1 + l2) / 2.0),
                    targets,
                    draw: build_draw_data(
                        &slice[i1..=i2],
                        2,
                        5,
                        mid_high,
                        start_time,
                        end_time,
                    ),
                });
            }
        }
    }

    None
}

fn detect_extremum_family_pattern(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
    cfg: &Config,
) -> Option<ClassicPatternDetection> {
    let window = candles.len().saturating_sub(320);
    let slice = &candles[window..];
    let len = slice.len();
    if len < 80 {
        return None;
    }

    let min_series = cfg.pivot_length.clamp(2, 5) as usize;
    let highs = collect_family_extrema(slice, min_series, true);
    let lows = collect_family_extrema(slice, min_series, false);
    if highs.len() < 2 || lows.len() < 2 {
        return None;
    }

    let tops_n = if highs.len() >= 3 && lows.len() >= 3 { 3usize } else { 2usize };
    let sel_highs = select_recent_extrema(&highs, tops_n);
    let sel_lows = select_recent_extrema(&lows, tops_n);
    if sel_highs.len() < 2 || sel_lows.len() < 2 {
        return None;
    }

    let latest_high_idx = sel_highs.last()?.0;
    let latest_low_idx = sel_lows.last()?.0;
    let is_bottom_family = latest_low_idx > latest_high_idx;
    let chosen = if is_bottom_family { &sel_lows } else { &sel_highs };
    if chosen.len() < 2 {
        return None;
    }

    let start_idx = chosen.first()?.0;
    let end_idx = chosen.last()?.0;
    if end_idx <= start_idx || end_idx >= len {
        return None;
    }

    let neckline = if is_bottom_family {
        slice[start_idx..=end_idx]
            .iter()
            .map(|c| c.high)
            .fold(f64::NEG_INFINITY, f64::max)
    } else {
        slice[start_idx..=end_idx]
            .iter()
            .map(|c| c.low)
            .fold(f64::INFINITY, f64::min)
    };
    if !neckline.is_finite() {
        return None;
    }

    let farthest = if is_bottom_family {
        chosen
            .iter()
            .map(|(i, p)| (*i, neckline - *p))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?
    } else {
        chosen
            .iter()
            .map(|(i, p)| (*i, *p - neckline))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?
    };
    let height = farthest.1;
    if height <= 0.0 {
        return None;
    }

    // Vertical/horizontal balancing from MQL prototype.
    let shoulder_distances: Vec<f64> = chosen
        .iter()
        .map(|(_, p)| {
            if is_bottom_family {
                neckline - *p
            } else {
                *p - neckline
            }
        })
        .collect();
    let min_dist = shoulder_distances
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let max_dist = shoulder_distances
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    if min_dist <= 0.0 || max_dist <= 0.0 {
        return None;
    }
    let rel_unstability_max = 0.8;
    if (max_dist - min_dist) / min_dist >= rel_unstability_max {
        return None;
    }

    if chosen.len() >= 3 {
        let mut gaps = Vec::new();
        for i in 1..chosen.len() {
            gaps.push((chosen[i].0 as f64 - chosen[i - 1].0 as f64).abs());
        }
        let gmin = gaps.iter().cloned().fold(f64::INFINITY, f64::min);
        let gmax = gaps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if gmin <= 0.0 || (gmax - gmin) / gmin > 0.8 {
            return None;
        }
    }

    let close_now = slice.last()?.close;
    let direction = if is_bottom_family {
        PatternDirection::Bullish
    } else {
        PatternDirection::Bearish
    };

    let kind = if chosen.len() >= 3 {
        if is_bottom_family {
            ClassicPatternKind::InverseHeadAndShoulders
        } else {
            ClassicPatternKind::HeadAndShoulders
        }
    } else if is_bottom_family {
        ClassicPatternKind::DoubleBottom
    } else {
        ClassicPatternKind::DoubleTop
    };

    let mut targets = Vec::new();
    match direction {
        PatternDirection::Bullish => {
            targets.push(ClassicPatternTarget {
                price: neckline + 0.618 * height,
                label: "Family TP1 (0.618H)".to_string(),
                priority: 1,
            });
            targets.push(ClassicPatternTarget {
                price: neckline + height,
                label: "Family TP2 (1.0H)".to_string(),
                priority: 2,
            });
            targets.push(ClassicPatternTarget {
                price: neckline + 1.272 * height,
                label: "Family TP3 (1.272H)".to_string(),
                priority: 3,
            });
        }
        PatternDirection::Bearish => {
            targets.push(ClassicPatternTarget {
                price: neckline - 0.618 * height,
                label: "Family TP1 (0.618H)".to_string(),
                priority: 1,
            });
            targets.push(ClassicPatternTarget {
                price: neckline - height,
                label: "Family TP2 (1.0H)".to_string(),
                priority: 2,
            });
            targets.push(ClassicPatternTarget {
                price: neckline - 1.272 * height,
                label: "Family TP3 (1.272H)".to_string(),
                priority: 3,
            });
        }
        PatternDirection::Neutral => {}
    }

    let close_series: Vec<f64> = slice.iter().map(|c| c.close).collect();
    let rsi_end = rsi(&close_series, 14).unwrap_or(50.0);
    let rsi_div = if direction == PatternDirection::Bearish {
        rsi_end < 50.0
    } else {
        rsi_end > 50.0
    };
    let vol_ok = has_breakout_volume(slice, 20, 1.1);
    let trend_aligned = if direction == PatternDirection::Bearish {
        close_now <= neckline
    } else {
        close_now >= neckline
    };
    let atr_before = atr(slice, 14).unwrap_or(height / 4.0);
    let atr_after = atr(&slice[end_idx..], 14).unwrap_or(atr_before);
    let atr_expansion = atr_after >= atr_before;
    let quality_score = compute_quality_score(
        true,
        true,
        rsi_div,
        vol_ok,
        trend_aligned,
        atr_expansion,
    );
    let mut confidence: f64 = 0.5;
    if rsi_div {
        confidence += 0.2;
    }
    if vol_ok {
        confidence += 0.15;
    }
    if trend_aligned {
        confidence += 0.15;
    }

    Some(ClassicPatternDetection {
        symbol: symbol.to_string(),
        timeframe,
        kind,
        direction,
        start_time: slice[start_idx].time,
        end_time: slice[end_idx].time,
        reference_level: neckline,
        secondary_level: Some(chosen.iter().map(|(_, p)| *p).sum::<f64>() / chosen.len() as f64),
        height,
        confidence: confidence.clamp(0.0, 1.0),
        quality_score,
        invalidation_level: Some(if direction == PatternDirection::Bearish {
            chosen.iter().map(|(_, p)| *p).fold(f64::NEG_INFINITY, f64::max)
        } else {
            chosen.iter().map(|(_, p)| *p).fold(f64::INFINITY, f64::min)
        }),
        targets,
        draw: build_draw_data(
            &slice[start_idx..=end_idx],
            2,
            5,
            neckline,
            slice[start_idx].time,
            slice[end_idx].time,
        ),
    })
}

fn collect_family_extrema(slice: &[Candle], min_series: usize, top: bool) -> Vec<(usize, f64)> {
    let len = slice.len();
    if len < min_series * 2 + 1 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..=(len - min_series * 2) {
        let mut first_ok = true;
        let mut second_ok = true;
        for j in i..(i + min_series) {
            let bearish = slice[j].open > slice[j].close;
            if top {
                first_ok &= bearish;
            } else {
                first_ok &= !bearish;
            }
        }
        for j in (i + min_series)..(i + min_series * 2) {
            let bullish = slice[j].close > slice[j].open;
            if top {
                second_ok &= bullish;
            } else {
                second_ok &= !bullish;
            }
        }
        if !(first_ok && second_ok) {
            continue;
        }
        let mut extremum_idx = i;
        let mut extremum_price = if top { f64::NEG_INFINITY } else { f64::INFINITY };
        for (k, c) in slice
            .iter()
            .enumerate()
            .skip(i)
            .take(min_series * 2)
        {
            if top {
                if c.high > extremum_price {
                    extremum_price = c.high;
                    extremum_idx = k;
                }
            } else if c.low < extremum_price {
                extremum_price = c.low;
                extremum_idx = k;
            }
        }
        if out.last().map(|(idx, _)| *idx) != Some(extremum_idx) {
            out.push((extremum_idx, extremum_price));
        }
    }
    out
}

fn select_recent_extrema(src: &[(usize, f64)], n: usize) -> Vec<(usize, f64)> {
    if src.len() <= n {
        return src.to_vec();
    }
    src[src.len() - n..].to_vec()
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

    // Handle: last 15% of bars, shallow pullback (ideally 0.382-0.618 of cup depth).
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

    let handle_retr = ((rim_avg - handle_low) / depth).clamp(0.0, 2.0);
    if !(0.2..=0.7).contains(&handle_retr) {
        return None;
    }

    let close_series: Vec<f64> = slice.iter().map(|c| c.close).collect();
    let rsi_val = rsi(&close_series, 14).unwrap_or(50.0);
    let vol_ok = has_breakout_volume(slice, 20, 1.2);
    let start_time = slice[max_idx].time;
    let end_time = slice[len - 1].time;
    let mut targets = Vec::new();
    targets.push(ClassicPatternTarget {
        price: rim_avg + 0.618 * depth,
        label: "Cup&Handle TP1 (0.618H)".to_string(),
        priority: 1,
    });
    targets.push(ClassicPatternTarget {
        price: rim_avg + depth,
        label: "Cup&Handle TP2 (1.0H)".to_string(),
        priority: 2,
    });
    targets.push(ClassicPatternTarget {
        price: rim_avg + 1.272 * depth,
        label: "Cup&Handle TP3 (1.272H)".to_string(),
        priority: 3,
    });

    let mut confidence: f64 = 0.5;
    let fibo_ok = handle_retr >= 0.382 && handle_retr <= 0.618;
    if fibo_ok {
        confidence += 0.2;
    }
    let volume_breakout = vol_ok;
    if volume_breakout {
        confidence += 0.15;
    }
    let rsi_div = rsi_val >= 50.0;
    if rsi_div {
        confidence += 0.15;
    }
    let atr_before = atr(slice, 14).unwrap_or(depth / 4.0);
    let atr_after = atr(&slice[min_idx..], 14).unwrap_or(atr_before);
    let atr_expansion = atr_after > atr_before;
    let trend_aligned = rim_avg > slice[min_idx].close;
    let quality_score = compute_quality_score(
        true,
        fibo_ok,
        rsi_div,
        volume_breakout,
        trend_aligned,
        atr_expansion,
    );

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
        confidence: confidence.clamp(0.0, 1.0),
        quality_score,
        invalidation_level: Some(handle_low),
        targets,
        draw: build_draw_data(&slice[max_idx..], 2, 5, rim_avg, start_time, end_time),
    })
}

fn detect_head_and_shoulders(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
) -> Option<ClassicPatternDetection> {
    let window = candles.len().saturating_sub(300);
    let slice = &candles[window..];
    let len = slice.len();
    if len < 120 {
        return None;
    }

    // Find recent pivot highs as candidates for shoulders/head.
    let pivot_len = 4usize;
    let mut highs = Vec::new();
    for i in (pivot_len..len - pivot_len).rev().take(40) {
        let sub = &slice[..=i + pivot_len];
        if let Some(h) = pivot_high(sub, pivot_len) {
            highs.push((i, h));
        }
    }
    if highs.len() < 3 {
        return None;
    }
    highs.sort_by_key(|(i, _)| *i);

    // Take last three highs as [LS, H, RS].
    let (ls_idx, ls) = highs[highs.len().saturating_sub(3)];
    let (h_idx, h) = highs[highs.len().saturating_sub(2)];
    let (rs_idx, rs) = highs[highs.len().saturating_sub(1)];

    // Head must be highest.
    if !(h > ls && h > rs) {
        // Try inverse H&S on lows.
    } else {
        // Bearish Head & Shoulders.
        let shoulder_tol = (h * 0.02).max(3.0);
        if (ls - rs).abs() > shoulder_tol {
            // Shoulders too asymmetric.
        } else {
            // Neckline: lows between LS-H and H-RS.
            let left_min = slice[ls_idx..=h_idx]
                .iter()
                .map(|c| c.low)
                .fold(f64::INFINITY, f64::min);
            let right_min = slice[h_idx..=rs_idx]
                .iter()
                .map(|c| c.low)
                .fold(f64::INFINITY, f64::min);
            let neckline = (left_min + right_min) / 2.0;
            let height = h - neckline;
            if height > 0.0 {
                let start_time = slice[ls_idx].time;
                let end_time = slice[rs_idx].time;
                let mut targets = Vec::new();
                targets.push(ClassicPatternTarget {
                    price: neckline - 0.618 * height,
                    label: "H&S TP1 (0.618H)".to_string(),
                    priority: 1,
                });
                targets.push(ClassicPatternTarget {
                    price: neckline - height,
                    label: "H&S TP2 (1.0H)".to_string(),
                    priority: 2,
                });
                targets.push(ClassicPatternTarget {
                    price: neckline - 1.272 * height,
                    label: "H&S TP3 (1.272H)".to_string(),
                    priority: 3,
                });
                let mut confidence: f64 = 0.6;
                if (ls - rs).abs() <= shoulder_tol / 2.0 {
                    confidence += 0.1;
                }
                let fibo_ok = true;
                let rsi_div = false;
                let volume_breakout = has_breakout_volume(slice, 20, 1.2);
                let trend_aligned = slice.last().unwrap().close < neckline;
                let atr_before = atr(slice, 14).unwrap_or(height / 4.0);
                let atr_after = atr(&slice[rs_idx..], 14).unwrap_or(atr_before);
                let atr_expansion = atr_after > atr_before;
                let quality_score = compute_quality_score(
                    true,
                    fibo_ok,
                    rsi_div,
                    volume_breakout,
                    trend_aligned,
                    atr_expansion,
                );
                return Some(ClassicPatternDetection {
                    symbol: symbol.to_string(),
                    timeframe,
                    kind: ClassicPatternKind::HeadAndShoulders,
                    direction: PatternDirection::Bearish,
                    start_time,
                    end_time,
                    reference_level: neckline,
                    secondary_level: Some(h),
                    height,
                    confidence: confidence.clamp(0.0, 1.0),
                    quality_score,
                    invalidation_level: Some(rs.max(ls)),
                    targets,
                    draw: build_draw_data(&slice[ls_idx..=rs_idx], 2, 5, neckline, start_time, end_time),
                });
            }
        }
    }

    // Inverse Head & Shoulders (bullish) on lows.
    let mut lows = Vec::new();
    for i in (pivot_len..len - pivot_len).rev().take(40) {
        let sub = &slice[..=i + pivot_len];
        if let Some(l) = pivot_low(sub, pivot_len) {
            lows.push((i, l));
        }
    }
    if lows.len() < 3 {
        return None;
    }
    lows.sort_by_key(|(i, _)| *i);
    let (ls_idx, ls) = lows[lows.len().saturating_sub(3)];
    let (h_idx, h) = lows[lows.len().saturating_sub(2)];
    let (rs_idx, rs) = lows[lows.len().saturating_sub(1)];
    if !(h < ls && h < rs) {
        return None;
    }
    let shoulder_tol = (h * 0.02).abs().max(3.0);
    if (ls - rs).abs() > shoulder_tol {
        return None;
    }
    let left_max = slice[ls_idx..=h_idx]
        .iter()
        .map(|c| c.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let right_max = slice[h_idx..=rs_idx]
        .iter()
        .map(|c| c.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let neckline = (left_max + right_max) / 2.0;
    let height = neckline - h;
    if height <= 0.0 {
        return None;
    }
    let start_time = slice[ls_idx].time;
    let end_time = slice[rs_idx].time;
    let mut targets = Vec::new();
    targets.push(ClassicPatternTarget {
        price: neckline + 0.618 * height,
        label: "Inv H&S TP1 (0.618H)".to_string(),
        priority: 1,
    });
    targets.push(ClassicPatternTarget {
        price: neckline + height,
        label: "Inv H&S TP2 (1.0H)".to_string(),
        priority: 2,
    });
    targets.push(ClassicPatternTarget {
        price: neckline + 1.272 * height,
        label: "Inv H&S TP3 (1.272H)".to_string(),
        priority: 3,
    });
    let mut confidence: f64 = 0.6;
    if (ls - rs).abs() <= shoulder_tol / 2.0 {
        confidence += 0.1;
    }
    let fibo_ok = true;
    let rsi_div = false;
    let volume_breakout = has_breakout_volume(slice, 20, 1.2);
    let trend_aligned = slice.last().unwrap().close > neckline;
    let atr_before = atr(slice, 14).unwrap_or(height / 4.0);
    let atr_after = atr(&slice[rs_idx..], 14).unwrap_or(atr_before);
    let atr_expansion = atr_after > atr_before;
    let quality_score = compute_quality_score(
        true,
        fibo_ok,
        rsi_div,
        volume_breakout,
        trend_aligned,
        atr_expansion,
    );
    Some(ClassicPatternDetection {
        symbol: symbol.to_string(),
        timeframe,
        kind: ClassicPatternKind::InverseHeadAndShoulders,
        direction: PatternDirection::Bullish,
        start_time,
        end_time,
        reference_level: neckline,
        secondary_level: Some(h),
        height,
        confidence: confidence.clamp(0.0, 1.0),
        quality_score,
        invalidation_level: Some(ls.min(rs)),
        targets,
        draw: build_draw_data(&slice[ls_idx..=rs_idx], 2, 5, neckline, start_time, end_time),
    })
}

fn detect_flag(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
) -> Option<ClassicPatternDetection> {
    let window = candles.len().saturating_sub(150);
    let slice = &candles[window..];
    let len = slice.len();
    if len < 80 {
        return None;
    }

    // Strong prior move: compare last close vs close 40 bars ago.
    let lookback = 40usize.min(len - 1);
    let base_close = slice[len - 1 - lookback].close;
    let last_close = slice[len - 1].close;
    let atr_val = atr(slice, 14).unwrap_or((slice[len - 1].high - slice[len - 1].low).abs());
    if atr_val <= 0.0 {
        return None;
    }
    let move_size = (last_close - base_close) / atr_val;

    // Flag consolidation: last 25 bars in a small channel.
    let flag_len = 25usize.min(len - 1);
    let flag_slice = &slice[len - flag_len..];
    let max_h = flag_slice
        .iter()
        .map(|c| c.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_l = flag_slice
        .iter()
        .map(|c| c.low)
        .fold(f64::INFINITY, f64::min);
    let range = max_h - min_l;
    if range <= 0.0 {
        return None;
    }

    let dir = if move_size > 3.0 {
        // Strong up move → Bull flag.
        let upper_trend = flag_slice.last().unwrap().high - flag_slice.first().unwrap().high;
        let downsloping = upper_trend < 0.0;
        if !downsloping {
            return None;
        }
        PatternDirection::Bullish
    } else if move_size < -3.0 {
        // Strong down move → Bear flag.
        let lower_trend = flag_slice.last().unwrap().low - flag_slice.first().unwrap().low;
        let upsloping = lower_trend > 0.0;
        if !upsloping {
            return None;
        }
        PatternDirection::Bearish
    } else {
        return None;
    };

    let kind = match dir {
        PatternDirection::Bullish => ClassicPatternKind::BullFlag,
        PatternDirection::Bearish => ClassicPatternKind::BearFlag,
        PatternDirection::Neutral => return None,
    };

    let start_time = flag_slice.first().unwrap().time;
    let end_time = flag_slice.last().unwrap().time;
    let ref_level = (max_h + min_l) / 2.0;
    let height = range;

    let mut targets = Vec::new();
    match dir {
        PatternDirection::Bullish => {
            targets.push(ClassicPatternTarget {
                price: last_close + 0.618 * range,
                label: "Bull Flag TP1".to_string(),
                priority: 1,
            });
            targets.push(ClassicPatternTarget {
                price: last_close + range,
                label: "Bull Flag TP2".to_string(),
                priority: 2,
            });
        }
        PatternDirection::Bearish => {
            targets.push(ClassicPatternTarget {
                price: last_close - 0.618 * range,
                label: "Bear Flag TP1".to_string(),
                priority: 1,
            });
            targets.push(ClassicPatternTarget {
                price: last_close - range,
                label: "Bear Flag TP2".to_string(),
                priority: 2,
            });
        }
        PatternDirection::Neutral => {}
    }

    let mut confidence: f64 = 0.55;
    if move_size.abs() > 4.0 {
        confidence += 0.15;
    }
    if range / base_close < 0.03 {
        confidence += 0.1;
    }
    let fibo_ok = true;
    let rsi_div = false;
    let volume_breakout = has_breakout_volume(slice, 20, 1.2);
    let trend_aligned = matches!(dir, PatternDirection::Bullish) == (last_close > base_close);
    let atr_before = atr(slice, 14).unwrap_or(range / 4.0);
    let atr_after = atr(flag_slice, 14).unwrap_or(atr_before);
    let atr_expansion = atr_after > atr_before;
    let quality_score = compute_quality_score(
        true,
        fibo_ok,
        rsi_div,
        volume_breakout,
        trend_aligned,
        atr_expansion,
    );

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
        confidence: confidence.clamp(0.0, 1.0),
        quality_score,
        invalidation_level: Some(ref_level),
        targets,
        draw: build_draw_data(flag_slice, 2, 5, ref_level, start_time, end_time),
    })
}

fn detect_range(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
) -> Option<ClassicPatternDetection> {
    let window = candles.len().saturating_sub(200);
    let slice = &candles[window..];
    let len = slice.len();
    if len < 80 {
        return None;
    }

    let recent_len = 60usize;
    let segment = &slice[len - recent_len..];
    let max_h = segment
        .iter()
        .map(|c| c.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_l = segment
        .iter()
        .map(|c| c.low)
        .fold(f64::INFINITY, f64::min);
    let range = max_h - min_l;
    if range <= 0.0 {
        return None;
    }

    let mid = (max_h + min_l) / 2.0;
    let first_close = segment.first().unwrap().close;
    let last_close = segment.last().unwrap().close;
    let drift = (last_close - first_close).abs();

    // Flat-ish market: drift küçük, range belli (hafif trendde bile range adayı).
    if drift / range > 0.58 {
        return None;
    }

    let start_time = segment.first().unwrap().time;
    let end_time = segment.last().unwrap().time;
    let mut targets = Vec::new();
    targets.push(ClassicPatternTarget {
        price: max_h,
        label: "Range Üstü".to_string(),
        priority: 1,
    });
    targets.push(ClassicPatternTarget {
        price: min_l,
        label: "Range Altı".to_string(),
        priority: 1,
    });

    let confidence: f64 = (1.0 - drift / range).clamp(0.4, 0.9);
    let fibo_ok = true;
    let rsi_div = false;
    let volume_breakout = has_breakout_volume(slice, 20, 1.2);
    let trend_aligned = (last_close - first_close).abs() < range * 0.3;
    let atr_before = atr(slice, 14).unwrap_or(range / 4.0);
    let atr_after = atr(segment, 14).unwrap_or(atr_before);
    let atr_expansion = atr_after <= atr_before;
    let quality_score = compute_quality_score(
        true,
        fibo_ok,
        rsi_div,
        volume_breakout,
        trend_aligned,
        atr_expansion,
    );

    Some(ClassicPatternDetection {
        symbol: symbol.to_string(),
        timeframe,
        kind: ClassicPatternKind::Range,
        direction: PatternDirection::Neutral,
        start_time,
        end_time,
        reference_level: mid,
        secondary_level: None,
        height: range,
        confidence,
        quality_score,
        invalidation_level: None,
        targets,
        draw: build_draw_data(segment, 2, 5, mid, start_time, end_time),
    })
}

fn detect_channel(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
) -> Option<ClassicPatternDetection> {
    let window = candles.len().saturating_sub(200);
    let slice = &candles[window..];
    let len = slice.len();
    if len < 80 {
        return None;
    }

    let recent_len = 80usize;
    let segment = &slice[len - recent_len..];
    let first = segment.first().unwrap();
    let last = segment.last().unwrap();
    let up_trend = last.close > first.close;
    let down_trend = last.close < first.close;
    if !up_trend && !down_trend {
        return None;
    }

    let max_h = segment
        .iter()
        .map(|c| c.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_l = segment
        .iter()
        .map(|c| c.low)
        .fold(f64::INFINITY, f64::min);
    let range = max_h - min_l;
    if range <= 0.0 {
        return None;
    }

    // Channel: price stays inside parallel-ish band.
    let mid = (max_h + min_l) / 2.0;
    let mut deviations = Vec::new();
    for c in segment {
        deviations.push((c.close - mid).abs());
    }
    let avg_dev = deviations.iter().sum::<f64>() / deviations.len() as f64;
    if avg_dev / range > 0.48 {
        return None;
    }

    let dir = if up_trend {
        PatternDirection::Bullish
    } else {
        PatternDirection::Bearish
    };

    let start_time = segment.first().unwrap().time;
    let end_time = segment.last().unwrap().time;
    let mut targets = Vec::new();
    match dir {
        PatternDirection::Bullish => {
            targets.push(ClassicPatternTarget {
                price: max_h + range * 0.5,
                label: "Channel TP".to_string(),
                priority: 1,
            });
        }
        PatternDirection::Bearish => {
            targets.push(ClassicPatternTarget {
                price: min_l - range * 0.5,
                label: "Channel TP".to_string(),
                priority: 1,
            });
        }
        PatternDirection::Neutral => {}
    }

    let confidence: f64 = (1.0 - avg_dev / range).clamp(0.4, 0.9);
    let fibo_ok = true;
    let rsi_div = false;
    let volume_breakout = has_breakout_volume(slice, 20, 1.2);
    let trend_aligned = up_trend == matches!(dir, PatternDirection::Bullish);
    let atr_before = atr(slice, 14).unwrap_or(range / 4.0);
    let atr_after = atr(segment, 14).unwrap_or(atr_before);
    let atr_expansion = atr_after > atr_before;
    let quality_score = compute_quality_score(
        true,
        fibo_ok,
        rsi_div,
        volume_breakout,
        trend_aligned,
        atr_expansion,
    );

    Some(ClassicPatternDetection {
        symbol: symbol.to_string(),
        timeframe,
        kind: ClassicPatternKind::Channel,
        direction: dir,
        start_time,
        end_time,
        reference_level: mid,
        secondary_level: None,
        height: range,
        confidence,
        quality_score,
        invalidation_level: Some(if matches!(dir, PatternDirection::Bullish) { min_l } else { max_h }),
        targets,
        draw: build_draw_data(segment, 2, 5, mid, start_time, end_time),
    })
}

