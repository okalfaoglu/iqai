//! Smart Money / liquidity / PO3 / Wyckoff context.
//!
//! Bu modül, StrategyScenario ve AI anlatımı için kullanılacak yüksek seviye
//! "Smart Money" bağlamını üretir. Matematiksel hesaplar basit ve muhafazakâr
//! tutulmuştur; temel amaç grafik üstüne ve AI'ye verilebilecek anlamlı
//! etiketler sağlamaktır.

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::indicators::{pivot_high, pivot_low};
use crate::types::{Candle, Timeframe};

/// Likidite seviyesinin tipi.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LiquidityKind {
    EqualHighs,
    EqualLows,
    PreviousHigh,
    PreviousLow,
    Psychological,
    OrderBlockHigh,
    OrderBlockLow,
}

/// Basit likidite seviyesi tanımı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityLevel {
    pub price: f64,
    pub kind: LiquidityKind,
    /// Kısa açıklama (ör. "1900 likidite havuzu").
    pub label: String,
    /// 0–1 arası önem skoru.
    pub strength: f64,
}

/// Order block yönü.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrderBlockSide {
    Bullish,
    Bearish,
}

/// Basit order block bölgesi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBlockZone {
    pub side: OrderBlockSide,
    pub high: f64,
    pub low: f64,
    pub label: String,
}

/// PO3 (Power of 3) fazı.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Po3Phase {
    Accumulation,
    Manipulation,
    Expansion,
}

/// Wyckoff stilinde genel etiketler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WyckoffTag {
    pub label: String,
    pub price: f64,
}

/// Wyckoff event dizisi (BC→AR→ST→UT→SOW→Spring/Test).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WyckoffEvent {
    Bc,
    Ar,
    St,
    Ut,
    Sow,
    Spring,
    Test,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WyckoffState {
    pub events: Vec<(WyckoffEvent, usize, f64)>, // (event, bar_index, price)
    pub complete: bool,
}

/// Smart Money bağlamı: likidite, order block, PO3 fazı, Wyckoff etiketleri.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartMoneyContext {
    pub symbol: String,
    pub timeframe: Timeframe,
    pub po3_phase: Po3Phase,
    pub liquidity_levels: Vec<LiquidityLevel>,
    pub order_blocks: Vec<OrderBlockZone>,
    pub wyckoff_tags: Vec<WyckoffTag>,
    pub wyckoff_state: Option<WyckoffState>,
}

/// Verilen seri için basit bir Smart Money bağlamı oluştur.
///
/// Heuristikler:
/// - Son X bar içindeki equal highs / equal lows → likidite seviyeleri
/// - Son güçlü "impulse" mumlarının öncesindeki karşı renkli mumlar → order block
/// - Son pivot yapısına göre kabaca Accumulation / Manipulation / Expansion etiketi.
pub fn build_smart_money_context_for_series(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
    _config: &Config,
) -> Option<SmartMoneyContext> {
    if candles.len() < 50 {
        return None;
    }

    let window = candles.len().saturating_sub(300);
    let slice = &candles[window..];

    let liquidity_levels = detect_liquidity_levels(slice);
    let order_blocks = detect_order_blocks(slice);
    let po3_phase = infer_po3_phase(slice);
    let (wyckoff_tags, wyckoff_state) = infer_wyckoff_tags_and_state(slice);

    Some(SmartMoneyContext {
        symbol: symbol.to_string(),
        timeframe,
        po3_phase,
        liquidity_levels,
        order_blocks,
        wyckoff_tags,
        wyckoff_state,
    })
}

fn detect_liquidity_levels(candles: &[Candle]) -> Vec<LiquidityLevel> {
    let mut levels = Vec::new();
    let len = candles.len();
    if len < 20 {
        return levels;
    }

    // Equal highs / lows cluster'ları (son ~150 bar).
    let start = len.saturating_sub(100);
    let mut high_clusters: Vec<(f64, usize)> = Vec::new();
    let mut low_clusters: Vec<(f64, usize)> = Vec::new();

    for i in start + 2..len {
        let c0 = &candles[i - 2];
        let c1 = &candles[i - 1];
        let c2 = &candles[i];
        let tol_high = (c1.high * 0.0015).max(1.0);
        let tol_low = (c1.low * 0.0015).max(1.0);

        if (c0.high - c1.high).abs() <= tol_high && (c2.high - c1.high).abs() <= tol_high {
            high_clusters.push((c1.high, i));
        }
        if (c0.low - c1.low).abs() <= tol_low && (c2.low - c1.low).abs() <= tol_low {
            low_clusters.push((c1.low, i));
        }
    }

    // Cluster'ları seviyelere indir (aynı fiyata yakın birden çok eşit seviye varsa strength yükselir).
    fn collapse_clusters(src: &[(f64, usize)], kind: LiquidityKind, label: &str, out: &mut Vec<LiquidityLevel>) {
        if src.is_empty() {
            return;
        }
        let mut tmp = src.to_vec();
        tmp.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut i = 0;
        while i < tmp.len() {
            let (base_price, _) = tmp[i];
            let mut sum_price = base_price;
            let mut count = 1usize;
            let mut j = i + 1;
            while j < tmp.len() && (tmp[j].0 - base_price).abs() <= base_price * 0.0015 {
                sum_price += tmp[j].0;
                count += 1;
                j += 1;
            }
            let avg_price = sum_price / count as f64;
            let strength = (0.5 + 0.1 * count as f64).min(1.0);
            out.push(LiquidityLevel {
                price: avg_price,
                kind,
                label: format!("{} (x{})", label, count),
                strength,
            });
            i = j;
        }
    }

    collapse_clusters(&high_clusters, LiquidityKind::EqualHighs, "Equal highs likidite", &mut levels);
    collapse_clusters(&low_clusters, LiquidityKind::EqualLows, "Equal lows likidite", &mut levels);

    // Önceki büyük swing high/low'lara dayalı likidite.
    if len >= 40 {
        let swing_window = len.saturating_sub(60);
        let recent = &candles[swing_window..];
        if let Some(max_h) = recent.iter().map(|c| c.high).max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)) {
            levels.push(LiquidityLevel {
                price: max_h,
                kind: LiquidityKind::PreviousHigh,
                label: "Önceki swing high".to_string(),
                strength: 0.6,
            });
        }
        if let Some(min_l) = recent.iter().map(|c| c.low).min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)) {
            levels.push(LiquidityLevel {
                price: min_l,
                kind: LiquidityKind::PreviousLow,
                label: "Önceki swing low".to_string(),
                strength: 0.6,
            });
        }
    }

    // Psychological levels: 50 / 100 adımlı yuvarlak seviyeler (kaba).
    if let Some(last) = candles.last() {
        let base = last.close;
        let step = 50.0;
        for k in -3..=3 {
            let lvl = (base / step).round() * step + k as f64 * step;
            if lvl > 0.0 && (lvl - base).abs() / base < 0.15 {
                levels.push(LiquidityLevel {
                    price: lvl,
                    kind: LiquidityKind::Psychological,
                    label: format!("Psikolojik seviye {:.0}", lvl),
                    strength: 0.4,
                });
            }
        }
    }

    levels
}

fn detect_order_blocks(candles: &[Candle]) -> Vec<OrderBlockZone> {
    let mut out = Vec::new();
    let len = candles.len();
    if len < 20 {
        return out;
    }

    // Gelişmiş heuristic:
    // - büyük gövdeli impulse mumları,
    // - ortalamanın üzeri hacim,
    // - hemen önceki karşı renkli mum blok olarak işaretlenir.
    let window = len.saturating_sub(120);
    let avg_vol: f64 = candles[window..]
        .iter()
        .map(|c| c.volume)
        .sum::<f64>()
        / (len - window) as f64;

    for i in window + 1..len {
        let prev = &candles[i - 1];
        let curr = &candles[i];
        let body = (curr.close - curr.open).abs();
        let range = curr.high - curr.low;
        if range <= 0.0 {
            continue;
        }
        let body_ratio = body / range;
        if body_ratio < 0.6 {
            continue;
        }

        if curr.volume < avg_vol * 1.2 {
            continue;
        }

        if curr.close > curr.open && prev.close < prev.open {
            // Bullish impulse, önceki kırmızı mum = bullish order block.
            out.push(OrderBlockZone {
                side: OrderBlockSide::Bullish,
                high: prev.high,
                low: prev.low,
                label: "Bullish OB".to_string(),
            });
        } else if curr.close < curr.open && prev.close > prev.open {
            out.push(OrderBlockZone {
                side: OrderBlockSide::Bearish,
                high: prev.high,
                low: prev.low,
                label: "Bearish OB".to_string(),
            });
        }
    }

    out
}

fn infer_po3_phase(candles: &[Candle]) -> Po3Phase {
    let len = candles.len();
    if len < 30 {
        return Po3Phase::Accumulation;
    }
    let recent = &candles[len.saturating_sub(60)..];
    let first = &recent[0];
    let last = recent.last().unwrap();
    let mid_idx = recent.len() / 2;
    let mid = &recent[mid_idx];

    let total_move = last.close - first.close;
    let mid_range = (mid.close - first.close).abs();

    if total_move.abs() < first.close * 0.01 {
        Po3Phase::Accumulation
    } else if mid_range < total_move.abs() * 0.4 {
        Po3Phase::Manipulation
    } else {
        Po3Phase::Expansion
    }
}

fn infer_wyckoff_tags_and_state(candles: &[Candle]) -> (Vec<WyckoffTag>, Option<WyckoffState>) {
    let mut tags = Vec::new();
    let len = candles.len();
    if len < 40 {
        return (tags, None);
    }

    // Kullanılabilir en son pivot high/low'ları BC / AR / ST olarak etiketle.
    let pivot_len = 5usize;
    let last_segment = &candles[len.saturating_sub(120)..];
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    for i in pivot_len..last_segment.len() - pivot_len {
        let sub = &last_segment[..=i + pivot_len];
        if let Some(h) = pivot_high(sub, pivot_len) {
            highs.push((i, h));
        }
        if let Some(l) = pivot_low(sub, pivot_len) {
            lows.push((i, l));
        }
    }

    highs.sort_by_key(|(i, _)| *i);
    lows.sort_by_key(|(i, _)| *i);

    let mut events = Vec::new();

    if let Some((i, h)) = highs.last().copied() {
        tags.push(WyckoffTag {
            label: "BC/UT candidate".to_string(),
            price: last_segment[i].high.max(h),
        });
        events.push((WyckoffEvent::Bc, i, last_segment[i].high.max(h)));
    }
    if let Some((i, l)) = lows.first().copied() {
        tags.push(WyckoffTag {
            label: "AR/Spring candidate".to_string(),
            price: last_segment[i].low.min(l),
        });
        events.push((WyckoffEvent::Ar, i, last_segment[i].low.min(l)));
    }

    // Çok basit bir state machine: BC ve AR varsa, sonrasında daha düşük bir low → Spring,
    // BC üstünde bir fake breakout → UT, AR altındaki kırılım → SOW gibi yorumlanır.
    let mut state: Option<WyckoffState> = None;
    if events.len() >= 2 {
        let (_, bc_idx, bc_price) = events[0];
        let (_, ar_idx, ar_price) = events[1];
        let seg_from_ar = &last_segment[ar_idx..];

        let mut seq = vec![(WyckoffEvent::Bc, bc_idx, bc_price), (WyckoffEvent::Ar, ar_idx, ar_price)];

        // UT: BC'den sonra, BC fiyatının bir miktar üzerinde spike.
        if let Some((i_ut, ut_high)) = seg_from_ar
            .iter()
            .enumerate()
            .filter(|(_, c)| c.high > bc_price * 1.005)
            .max_by(|a, b| a.1.high.partial_cmp(&b.1.high).unwrap_or(std::cmp::Ordering::Equal))
        {
            let idx = ar_idx + i_ut;
            seq.push((WyckoffEvent::Ut, idx, ut_high.high));
        }

        // SOW: AR'den sonra AR low'un belirgin altına kırılım.
        if let Some((i_sow, sow_low)) = seg_from_ar
            .iter()
            .enumerate()
            .filter(|(_, c)| c.low < ar_price * 0.995)
            .min_by(|a, b| a.1.low.partial_cmp(&b.1.low).unwrap_or(std::cmp::Ordering::Equal))
        {
            let idx = ar_idx + i_sow;
            seq.push((WyckoffEvent::Sow, idx, sow_low.low));
        }

        // Spring: Eğer SOW sonrası daha da derin bir low ve hızlı toparlanma varsa.
        if let Some((_, _, sow_price)) = seq.iter().rev().find(|(e, _, _)| *e == WyckoffEvent::Sow) {
            if let Some((i_spring, spring_low)) = seg_from_ar
                .iter()
                .enumerate()
                .filter(|(_, c)| c.low < *sow_price * 0.997)
                .min_by(|a, b| a.1.low.partial_cmp(&b.1.low).unwrap_or(std::cmp::Ordering::Equal))
            {
                let idx = ar_idx + i_spring;
                seq.push((WyckoffEvent::Spring, idx, spring_low.low));
            }
        }

        if seq.len() >= 2 {
            state = Some(WyckoffState {
                events: seq,
                complete: state_is_complete(&events),
            });
        }
    }

    (tags, state)
}

fn state_is_complete(events: &[(WyckoffEvent, usize, f64)]) -> bool {
    let mut has_bc = false;
    let mut has_ar = false;
    let mut has_sow_or_spring = false;
    for (e, _, _) in events {
        match e {
            WyckoffEvent::Bc => has_bc = true,
            WyckoffEvent::Ar => has_ar = true,
            WyckoffEvent::Sow | WyckoffEvent::Spring => has_sow_or_spring = true,
            _ => {}
        }
    }
    has_bc && has_ar && has_sow_or_spring
}

