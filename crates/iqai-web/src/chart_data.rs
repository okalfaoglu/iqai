//! Chart annotations - CHoCH, BOS, liquidity, support/resistance, CVD, Elliott, Impulse

use iqai_core::{
    config::Config,
    elliott_detector::compute_elliott,
    indicators::{pivot_high, pivot_low, rsi, sma},
    types::Candle,
};

#[derive(serde::Serialize)]
pub struct ChartAnnotations {
    pub choch: Vec<ChochEvent>,
    pub bos: Vec<BosEvent>,
    pub liquidity: Vec<LiquidityEvent>,
    pub market_profile: Vec<MarketProfileEvent>,
    pub divergence: Vec<DivergenceEvent>,
    pub cvd: f64,
    pub support_line: Option<Line>,
    pub resistance_line: Option<Line>,
    pub elliott: ElliottAnnotations,
    /// Zigzag çizgisi – swing noktaları (time, price)
    pub zigzag: Vec<ZigzagPoint>,
}

#[derive(serde::Serialize)]
pub struct ZigzagPoint {
    pub time: i64,
    pub price: f64,
}

/// Elliott projeksiyon hedefi
#[derive(serde::Serialize)]
pub struct ElliottProjection {
    pub price: f64,
    pub label: String,
}

/// Elliott Wave çizim verileri – swing bazlı dalga bacakları ve seviyeler
#[derive(serde::Serialize, Default)]
pub struct ElliottAnnotations {
    /// Impulse/zigzag dalga bacakları: (time, price) çizgiler
    pub wave_legs: Vec<ElliottWaveLeg>,
    /// Fibonacci seviye çizgileri (yatay)
    pub fibo_levels: Vec<FiboLevel>,
    /// Mevcut formasyon adı (Impulse, Zigzag, vb.)
    pub formation: String,
    /// Formasyon türü: "Motif (İtki)" veya "Düzeltme"
    pub formation_type: String,
    /// Dalga noktaları: zaman, fiyat, label (0,1,2,3,4,5 veya A,B,C)
    pub wave_points: Vec<ElliottWavePoint>,
    /// W5 hedefleri (Impulse): (W1=W5, 0.618×(0-3), W4 inverse 123.6%)
    pub w5_targets: Option<(f64, f64, f64)>,
    /// Impulse tespit (3 aşama: CHoCH, W2 validasyon, BOS onay)
    pub impulse_state: Option<ImpulseState>,
    /// Kurallar geçerli mi (W2<=W0 iptal, W3 en kısa değil, W4-W1 örtüşmez)
    pub validation_ok: Option<bool>,
    pub validation_msg: Option<String>,
    /// Devam eden formasyon (3 veya 4 swing ile henüz tamamlanmamış)
    pub in_progress: Option<bool>,
    /// Projeksiyon hedefleri (W3/W5 veya C) – in_progress iken dolu
    pub projections: Option<Vec<ElliottProjection>>,
    /// EWM tarzı sarı noktalı kanal çizgileri (üst: 1-3-5, alt: 2-4)
    pub channel_upper: Option<Line>,
    pub channel_lower: Option<Line>,
    /// Dalga derecesi
    pub degree: Option<iqai_core::elliott::WaveDegree>,
    /// Truncation
    pub truncation: Option<bool>,
    /// Alternation
    pub alternation: Option<iqai_core::elliott::AlternationResult>,
    /// Impulse kanal (W2-W4 baz + W3 paralel)
    pub channel: Option<iqai_core::elliott::ImpulseChannel>,
    /// W5 giriş teyidi
    pub w5_confirmation: Option<iqai_core::impulse_detector::W5Confirmation>,
    /// W3 hacim kontrolü
    pub w3_volume_ok: Option<bool>,
    /// W5 süre hedefleri
    pub w5_time_targets: Option<(i64, i64, i64)>,
    /// W5 throw-over
    pub throw_over: Option<bool>,
    /// Extended dalga (1/3/5, oran)
    pub extended_wave: Option<(u8, f64)>,
    /// W1≈W5 eşitlik oranı
    pub w1_w5_eq: Option<f64>,
    /// W5 RSI divergence (W5 zayıflama sinyali)
    pub w5_divergence: Option<bool>,
    /// Yapısal alternation (W2 formasyon tipi vs W4 formasyon tipi)
    pub alternation_structural: Option<iqai_core::elliott::AlternationResult>,
    /// W2 düzeltme tipi (Sharp/Sideways)
    pub w2_corr_type: Option<iqai_core::elliott::CorrWaveType>,
    /// W4 düzeltme tipi (Sharp/Sideways)
    pub w4_corr_type: Option<iqai_core::elliott::CorrWaveType>,
    /// Diagonal iç yapı (LD: 5-3-5-3-5, ED: 3-3-3-3-3)
    pub diagonal_sub: Option<iqai_core::elliott::DiagonalSubStructure>,
    /// Diagonal iç swing sayıları [W1,W2,W3,W4,W5]
    pub diagonal_inner_counts: Option<[usize; 5]>,
    /// Corrective trade setup (Zigzag C veya Triangle E breakout)
    pub corr_setup: Option<iqai_core::elliott::CorrSetup>,
    /// Alternatif kanal (W3 güçlü ise W1 tepesinden paralel)
    pub channel_alt: Option<iqai_core::elliott::ImpulseChannel>,
    /// Semi-log kanal W5 hedefi
    pub channel_semilog_target: Option<f64>,
    /// W5 extension sinyali (vol W5 >= vol W3)
    pub w5_vol_extension: Option<bool>,
    /// W4 Golden Section
    pub w4_golden_section: Option<f64>,
    /// W2 depth target (W1 iç W4 seviyesi)
    pub w2_depth_target: Option<f64>,
    /// W4 depth target (W3 iç W4 seviyesi)
    pub w4_depth_target: Option<f64>,
    /// Alt-dalga yapısı doğrulaması (Impulse W1-W5)
    pub subwave_validation: Option<iqai_core::elliott::SubWaveValidation>,
    /// Nested extension (W3 iç ext)
    pub nested_extension: Option<(bool, f64)>,
    /// Corrective alt-dalga doğrulaması (Zigzag/Flat A,B,C)
    pub corr_subwave_validation: Option<iqai_core::elliott::CorrSubWaveValidation>,
}

#[derive(serde::Serialize)]
pub struct ImpulseState {
    pub stage: String,
    pub message: String,
    pub is_bullish: bool,
    pub setup_w3: Option<serde_json::Value>,
    pub setup_w5: Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
pub struct ElliottWavePoint {
    pub time: i64,
    pub price: f64,
    pub label: String,
}

#[derive(serde::Serialize)]
pub struct ElliottWaveLeg {
    pub time1: i64,
    pub price1: f64,
    pub time2: i64,
    pub price2: f64,
    pub label: String,
    pub color: String,
    /// Tamamlanmamış dalga (projeksiyon) ise true – noktalı çizilir
    #[serde(default)]
    pub dotted: bool,
}

#[derive(serde::Serialize)]
pub struct FiboLevel {
    pub time1: i64,
    pub time2: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct ChochEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct BosEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct LiquidityEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
}

#[derive(serde::Serialize)]
pub struct MarketProfileEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

#[derive(serde::Serialize)]
pub struct DivergenceEvent {
    pub time: i64,
    pub price: f64,
    pub label: String,
    pub color: String,
}

/// Geçmiş veride bulunan geçerli Elliott formasyonu
#[derive(serde::Serialize)]
pub struct HistoricalFormation {
    pub end_time: i64,
    pub formation: String,
    pub formation_type: String,
    pub is_bullish: bool,
    pub wave_points: Vec<ElliottWavePoint>,
    pub wave_legs: Vec<ElliottWaveLeg>,
    pub w5_targets: Option<(f64, f64, f64)>,
}

#[derive(serde::Serialize)]
pub struct Line {
    pub time1: i64,
    pub price1: f64,
    pub time2: i64,
    pub price2: f64,
    pub time3: i64, // extend to right (last candle)
    pub price3: f64,
    pub color: String,
}

/// Elliott seçenekleri (API'den gelen)
pub struct ElliottOptions {
    pub invert: bool,
}

pub fn compute_annotations(candles: &[Candle], config: &Config, opts: Option<&ElliottOptions>) -> ChartAnnotations {
    let pl = config.pivot_length as usize;
    let mut choch = Vec::new();
    let mut bos = Vec::new();
    let mut liquidity = Vec::new();
    let mut market_profile = Vec::new();
    let mut divergence = Vec::new();
    let mut raw_cvd = 0.0f64;

    if candles.len() < pl * 2 + 20 {
        return ChartAnnotations {
            choch,
            bos,
            liquidity,
            market_profile,
            divergence,
            cvd: 0.0,
            support_line: None,
            resistance_line: None,
            elliott: ElliottAnnotations::default(),
            zigzag: vec![],
        };
    }

    let mut last_high = f64::NEG_INFINITY;
    let mut last_low = f64::INFINITY;
    let mut recent_buy_vol = 0.0f64;
    let mut recent_sell_vol = 0.0f64;
    let vol_avg = sma(
        &candles.iter().map(|c| c.volume).collect::<Vec<_>>(),
        config.volume_long_period as usize,
    )
    .unwrap_or(0.0);

    let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
    let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

    for i in (pl * 2 + 1)..candles.len() {
        let sub = &candles[..=i];
        let c = &candles[i];
        let prev_c = &candles[i - 1];

        // Pivot update
        if let Some(ph) = pivot_high(sub, pl) {
            last_high = ph;
        }
        if let Some(pl_val) = pivot_low(sub, pl) {
            last_low = pl_val;
        }

        // CVD
        let delta = if c.close > prev_c.close {
            c.volume
        } else if c.close < prev_c.close {
            -c.volume
        } else {
            0.0
        };
        raw_cvd += delta;

        if c.is_bullish() {
            recent_buy_vol = sma(
                &candles[i.saturating_sub(20)..=i]
                    .iter()
                    .map(|x| x.volume)
                    .collect::<Vec<_>>(),
                20,
            )
            .unwrap_or(c.volume);
        } else if c.is_bearish() {
            recent_sell_vol = sma(
                &candles[i.saturating_sub(20)..=i]
                    .iter()
                    .map(|x| x.volume)
                    .collect::<Vec<_>>(),
                20,
            )
            .unwrap_or(c.volume);
        }

        // CHoCH
        if prev_c.low >= last_high && c.low < last_high && c.is_bearish() {
            choch.push(ChochEvent {
                time: c.time / 1000,
                price: last_high,
                label: "CHoCH".to_string(),
                color: "#00E5FF".to_string(),
            });
        }
        if prev_c.high <= last_low && c.high > last_low && c.is_bullish() {
            choch.push(ChochEvent {
                time: c.time / 1000,
                price: last_low,
                label: "CHoCH".to_string(),
                color: "#76FF03".to_string(),
            });
        }

        // BOS - need prev pivot
        let prev_last_low = if i >= 3 {
            pivot_low(&candles[..i], pl).unwrap_or(last_low)
        } else {
            last_low
        };
        let prev_last_high = if i >= 3 {
            pivot_high(&candles[..i], pl).unwrap_or(last_high)
        } else {
            last_high
        };
        if prev_c.low >= prev_last_low && c.low < prev_last_low && c.is_bearish() {
            bos.push(BosEvent {
                time: c.time / 1000,
                price: prev_last_low,
                label: "BOS".to_string(),
                color: "#E040FB".to_string(),
            });
        }
        if prev_c.high <= prev_last_high && c.high > prev_last_high && c.is_bullish() {
            bos.push(BosEvent {
                time: c.time / 1000,
                price: prev_last_high,
                label: "BOS".to_string(),
                color: "#00BFA5".to_string(),
            });
        }

        // Liquidity zones
        if config.enable_liquidity_zones && i >= 20 {
            let lookback = 20;
            let recent_high = highs[i.saturating_sub(lookback)..=i]
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            let recent_low = lows[i.saturating_sub(lookback)..=i]
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min);
            if (c.high - recent_high).abs() / recent_high < 0.0005 {
                liquidity.push(LiquidityEvent {
                    time: c.time / 1000,
                    price: c.high,
                    label: "💧 LIQ".to_string(),
                });
            }
            if (c.low - recent_low).abs() / recent_low < 0.0005 {
                liquidity.push(LiquidityEvent {
                    time: c.time / 1000,
                    price: c.low,
                    label: "💧 LIQ".to_string(),
                });
            }
        }

        // Market profile
        if config.enable_market_profile && i >= 20 {
            let vol_ratio = if recent_buy_vol + recent_sell_vol > 0.0 {
                recent_buy_vol / (recent_buy_vol + recent_sell_vol)
            } else {
                0.5
            };
            if vol_ratio > 0.65 && c.volume > vol_avg * 1.5 {
                market_profile.push(MarketProfileEvent {
                    time: c.time / 1000,
                    price: c.low,
                    label: "🔥 BUY".to_string(),
                    color: "#00D9FF".to_string(),
                });
            }
            if vol_ratio < 0.35 && c.volume > vol_avg * 1.5 {
                market_profile.push(MarketProfileEvent {
                    time: c.time / 1000,
                    price: c.high,
                    label: "🔥 SELL".to_string(),
                    color: "#FF006E".to_string(),
                });
            }
        }

        // Divergence
        if config.enable_divergence_scanner && i >= 14 {
            let rsi_cur = rsi(&closes[..=i], 14).unwrap_or(50.0);
            if i >= 11 {
                let rsi_5 = rsi(&closes[..=(i - 5)], 14).unwrap_or(50.0);
                let rsi_10 = rsi(&closes[..=(i - 10)], 14).unwrap_or(50.0);
                let price_ll = c.low < lows[i - 5] && lows[i - 5] < lows[i - 10];
                let rsi_hl = rsi_cur > rsi_5 && rsi_5 > rsi_10;
                if price_ll && rsi_hl && rsi_cur < 40.0 {
                    divergence.push(DivergenceEvent {
                        time: c.time / 1000,
                        price: c.low,
                        label: "⚡ BULL".to_string(),
                        color: "#00F5FF".to_string(),
                    });
                }
                let price_hh = c.high > highs[i - 5] && highs[i - 5] > highs[i - 10];
                let rsi_lh = rsi_cur < rsi_5 && rsi_5 < rsi_10;
                if price_hh && rsi_lh && rsi_cur > 60.0 {
                    divergence.push(DivergenceEvent {
                        time: c.time / 1000,
                        price: c.high,
                        label: "⚡ BEAR".to_string(),
                        color: "#C77DFF".to_string(),
                    });
                }
            }
        }
    }

    // Support/Resistance lines (Pine script logic)
    let short_p = config.short_trend_period as usize;
    let long_p = config.long_trend_period as usize;
    let n = candles.len();
    let mut support_line = None;
    let mut resistance_line = None;

    if n >= long_p {
        let mut lowest_y2 = f64::INFINITY;
        let mut lowest_x2 = 0usize;
        let mut highest_y2 = f64::NEG_INFINITY;
        let mut highest_x2 = 0usize;
        for i in 1..short_p.min(n) {
            if lows[n - 1 - i] < lowest_y2 {
                lowest_y2 = lows[n - 1 - i];
                lowest_x2 = i;
            }
            if highs[n - 1 - i] > highest_y2 {
                highest_y2 = highs[n - 1 - i];
                highest_x2 = i;
            }
        }
        let mut lowest_y1 = f64::INFINITY;
        let mut lowest_x1 = 0usize;
        let mut highest_y1 = f64::NEG_INFINITY;
        let mut highest_x1 = 0usize;
        for j in (short_p + 1)..long_p.min(n) {
            if lows[n - 1 - j] < lowest_y1 {
                lowest_y1 = lows[n - 1 - j];
                lowest_x1 = j;
            }
            if highs[n - 1 - j] > highest_y1 {
                highest_y1 = highs[n - 1 - j];
                highest_x1 = j;
            }
        }
        // Work consistently in seconds for all time-based calculations to avoid
        // unit mismatches (milliseconds vs seconds) that flatten the slope.
        let last_time_sec = candles[n - 1].time / 1000;
        if lowest_x1 > 0 && lowest_x2 > 0 {
            let t1_sec = candles[n - 1 - lowest_x1].time / 1000;
            let t2_sec = candles[n - 1 - lowest_x2].time / 1000;
            let dt_sec = t2_sec - t1_sec;
            let slope = if dt_sec != 0 {
                (lowest_y2 - lowest_y1) as f64 / dt_sec as f64
            } else {
                0.0
            };
            let price3 = lowest_y2 + slope * (last_time_sec - t2_sec) as f64;
            support_line = Some(Line {
                time1: t1_sec,
                price1: lowest_y1,
                time2: t2_sec,
                price2: lowest_y2,
                time3: last_time_sec,
                price3,
                color: "#00E676".to_string(),
            });
        }
        if highest_x1 > 0 && highest_x2 > 0 {
            let t1_sec = candles[n - 1 - highest_x1].time / 1000;
            let t2_sec = candles[n - 1 - highest_x2].time / 1000;
            let dt_sec = t2_sec - t1_sec;
            let slope = if dt_sec != 0 {
                (highest_y2 - highest_y1) as f64 / dt_sec as f64
            } else {
                0.0
            };
            let price3 = highest_y2 + slope * (last_time_sec - t2_sec) as f64;
            resistance_line = Some(Line {
                time1: t1_sec,
                price1: highest_y1,
                time2: t2_sec,
                price2: highest_y2,
                time3: last_time_sec,
                price3,
                color: "#FF1744".to_string(),
            });
        }
    }

    let invert = opts.map(|o| o.invert).unwrap_or(false);
    let mut elliott = elliott_result_to_annotations(compute_elliott(candles, config, invert));
    let last_sec = candles.last().map(|c| c.time / 1000).unwrap_or(0);
    if let Some((upper, lower)) = compute_elliott_channel_lines(&elliott, last_sec) {
        elliott.channel_upper = Some(upper);
        elliott.channel_lower = Some(lower);
    }

    // Elliott Fibo seviyelerini dalga çiziminden sonra, sağ tarafta kısa yatay çizgiler
    // olarak göster: önce küçük bir boşluk (gap), sonra sabit uzunluk.
    if !elliott.fibo_levels.is_empty() && candles.len() >= 2 {
        let last = candles.last().unwrap();
        let prev = &candles[candles.len() - 2];
        let bar_sec = ((last.time - prev.time) / 1000).max(1);
        let gap_bars = config.elliott_fibo_gap_bars.max(1) as i64;
        let len_bars = config.elliott_fibo_length_bars.max(1) as i64;

        // Son dalga noktası varsa onu referans al, yoksa son mum zamanı
        let anchor_time = elliott
            .wave_points
            .last()
            .map(|p| p.time)
            .unwrap_or(last_sec);

        let start = anchor_time + gap_bars * bar_sec as i64;
        let end = start + len_bars * bar_sec as i64;

        for f in elliott.fibo_levels.iter_mut() {
            f.time1 = start;
            f.time2 = end;
        }
    }
    let zigzag = collect_zigzag_swings(candles, pl);

    ChartAnnotations {
        choch,
        bos,
        liquidity,
        market_profile,
        divergence,
        cvd: raw_cvd,
        support_line,
        resistance_line,
        elliott,
        zigzag,
    }
}

/// Zigzag için minimum fiyat değişimi (deviation) – gürültüyü filtreler (yön değişiminde)
const ZIGZAG_DEVIATION_PCT: f64 = 0.005; // %0.5 – TradingView benzeri

/// Pivot bazlı swing noktalarını topla – zigzag çizgisi için
/// LuxAlgo mantığı: aynı yönde daha ekstrem pivot geldiğinde son noktayı güncelle (extend)
/// Deviation: yön değişiminde yeni nokta eklemek için min % hareket gerekir
fn collect_zigzag_swings(candles: &[Candle], pivot_len: usize) -> Vec<ZigzagPoint> {
    let mut result = Vec::new();
    if candles.len() < pivot_len * 4 + 2 {
        return result;
    }
    let mut last_was_high = Option::<bool>::None;
    let mut last_price: Option<f64> = None;

    for i in (pivot_len * 2 + 1)..(candles.len().saturating_sub(pivot_len)) {
        let sub = &candles[..=i + pivot_len];
        let pivot_idx = sub.len() - 1 - pivot_len;
        let t = candles[pivot_idx].time / 1000;

        if let Some(ph) = pivot_high(sub, pivot_len) {
            if last_was_high != Some(true) {
                // Yön değişimi: önceki low veya ilk nokta – deviation kontrolü ile ekle
                let ok = last_price
                    .map(|lp| (ph - lp).abs() / lp.max(1e-10) >= ZIGZAG_DEVIATION_PCT)
                    .unwrap_or(true);
                if ok {
                    result.push(ZigzagPoint { time: t, price: ph });
                    last_was_high = Some(true);
                    last_price = Some(ph);
                }
            } else if ph > last_price.unwrap_or(0.0) {
                // LuxAlgo extend: aynı yönde daha yüksek high – son noktayı güncelle
                if let Some(last) = result.last_mut() {
                    last.time = t;
                    last.price = ph;
                }
                last_price = Some(ph);
            }
        }
        if let Some(pl_val) = pivot_low(sub, pivot_len) {
            if last_was_high != Some(false) {
                // Yön değişimi: önceki high veya ilk nokta
                let ok = last_price
                    .map(|lp| (pl_val - lp).abs() / lp.max(1e-10) >= ZIGZAG_DEVIATION_PCT)
                    .unwrap_or(true);
                if ok {
                    result.push(ZigzagPoint { time: t, price: pl_val });
                    last_was_high = Some(false);
                    last_price = Some(pl_val);
                }
            } else if pl_val < last_price.unwrap_or(f64::INFINITY) {
                // LuxAlgo extend: aynı yönde daha düşük low – son noktayı güncelle
                if let Some(last) = result.last_mut() {
                    last.time = t;
                    last.price = pl_val;
                }
                last_price = Some(pl_val);
            }
        }
    }
    result
}

/// Merkezi Elliott sonucunu Web GUI için görsel formata dönüştür (renk ekle)
fn leg_color(label: &str) -> &'static str {
    match label {
        "1" | "3" | "5" | "A" | "C" | "E" => "#00E5FF",
        _ => "#00BFA5",
    }
}

/// EWM tarzı sarı noktalı kanal çizgileri. Impulse/Diagonal: üst (1-3-5), alt (2-4). Triangle: üst (B-D), alt (A-C). Zigzag/Flat: üst (A'-C veya A-B), alt (A-B veya A'-C).
fn compute_elliott_channel_lines(
    ann: &ElliottAnnotations,
    last_candle_time_sec: i64,
) -> Option<(Line, Line)> {
    if ann.validation_ok != Some(true) {
        return None;
    }
    let yellow = "#FFD700".to_string();
    let t_sec = |p: &ElliottWavePoint| if p.time > 1_000_000_000_000 { p.time / 1000 } else { p.time };
    let pr = |p: &ElliottWavePoint| p.price;

    if ann.formation == "Triangle" {
        let pts = &ann.wave_points;
        let by_label = |l: &str| pts.iter().find(|p| p.label == l);
        let pa = by_label("A")?;
        let pb = by_label("B")?;
        let pc = by_label("C")?;
        let pd = by_label("D")?;
        // EWM: üst çizgi B-D, alt çizgi A-C (daralan üçgen)
        let (ta, pra) = (t_sec(pa), pr(pa));
        let (tb, prb) = (t_sec(pb), pr(pb));
        let (tc, prc) = (t_sec(pc), pr(pc));
        let (td, prd) = (t_sec(pd), pr(pd));
        let dt_upper = td - tb;
        let slope_upper = if dt_upper != 0 {
            (prd - prb) as f64 / dt_upper as f64
        } else {
            0.0
        };
        let price3_upper = prd + slope_upper * (last_candle_time_sec - td) as f64;
        let dt_lower = tc - ta;
        let slope_lower = if dt_lower != 0 {
            (prc - pra) as f64 / dt_lower as f64
        } else {
            0.0
        };
        let price3_lower = prc + slope_lower * (last_candle_time_sec - tc) as f64;
        let upper = Line {
            time1: tb,
            price1: prb,
            time2: td,
            price2: prd,
            time3: last_candle_time_sec,
            price3: price3_upper,
            color: yellow.clone(),
        };
        let lower = Line {
            time1: ta,
            price1: pra,
            time2: tc,
            price2: prc,
            time3: last_candle_time_sec,
            price3: price3_lower,
            color: yellow,
        };
        return Some((upper, lower));
    }

    if ann.formation == "Zigzag" || ann.formation.starts_with("Flat") {
        let pts = &ann.wave_points;
        let by_label = |l: &str| pts.iter().find(|p| p.label == l);
        // Flat: 0, A, B, C (Regular/Expanded/Running). Zigzag: A, A', B, C.
        let (pa, pa2) = if ann.formation.starts_with("Flat") {
            (by_label("0")?, by_label("A")?)
        } else {
            (by_label("A")?, by_label("A'")?)
        };
        let pb = by_label("B")?;
        let pc = by_label("C")?;
        let (ta, pra) = (t_sec(pa), pr(pa));
        let (ta2, pra2) = (t_sec(pa2), pr(pa2));
        let (tb, prb) = (t_sec(pb), pr(pb));
        let (tc, prc) = (t_sec(pc), pr(pc));
        let (upper_t1, upper_p1, upper_t2, upper_p2, lower_t1, lower_p1, lower_t2, lower_p2) =
            if pra < pra2 && prc > prb {
                (ta2, pra2, tc, prc, ta, pra, tb, prb)
            } else {
                (ta, pra, tb, prb, ta2, pra2, tc, prc)
            };
        let dt_u = upper_t2 - upper_t1;
        let slope_u = if dt_u != 0 {
            (upper_p2 - upper_p1) as f64 / dt_u as f64
        } else {
            0.0
        };
        let price3_u = upper_p2 + slope_u * (last_candle_time_sec - upper_t2) as f64;
        let dt_l = lower_t2 - lower_t1;
        let slope_l = if dt_l != 0 {
            (lower_p2 - lower_p1) as f64 / dt_l as f64
        } else {
            0.0
        };
        let price3_l = lower_p2 + slope_l * (last_candle_time_sec - lower_t2) as f64;
        let upper = Line {
            time1: upper_t1,
            price1: upper_p1,
            time2: upper_t2,
            price2: upper_p2,
            time3: last_candle_time_sec,
            price3: price3_u,
            color: yellow.clone(),
        };
        let lower = Line {
            time1: lower_t1,
            price1: lower_p1,
            time2: lower_t2,
            price2: lower_p2,
            time3: last_candle_time_sec,
            price3: price3_l,
            color: yellow,
        };
        return Some((upper, lower));
    }

    if ann.formation != "Impulse" && ann.formation != "Diagonal" {
        return None;
    }
    let pts = &ann.wave_points;
    let by_label = |l: &str| pts.iter().find(|p| p.label == l);
    let p1 = by_label("1")?;
    let p2 = by_label("2")?;
    let p3 = by_label("3")?;
    let p4 = by_label("4")?;
    let p5 = by_label("5");
    let (t1, pr1) = (t_sec(p1), pr(p1));
    let (t2, pr2) = (t_sec(p2), pr(p2));
    let (t3, pr3) = (t_sec(p3), pr(p3));
    let (t4, pr4) = (t_sec(p4), pr(p4));
    let upper_t2 = p5.map(|p| t_sec(p)).unwrap_or(t3);
    let upper_pr2 = p5.map(|p| pr(p)).unwrap_or(pr3);
    let dt_u = upper_t2 - t1;
    let slope_u = if dt_u != 0 {
        (upper_pr2 - pr1) as f64 / dt_u as f64
    } else {
        0.0
    };
    let price3_u = upper_pr2 + slope_u * (last_candle_time_sec - upper_t2) as f64;
    let dt_l = t4 - t2;
    let slope_l = if dt_l != 0 {
        (pr4 - pr2) as f64 / dt_l as f64
    } else {
        0.0
    };
    let price3_l = pr4 + slope_l * (last_candle_time_sec - t4) as f64;
    let upper = Line {
        time1: t1,
        price1: pr1,
        time2: upper_t2,
        price2: upper_pr2,
        time3: last_candle_time_sec,
        price3: price3_u,
        color: yellow.clone(),
    };
    let lower = Line {
        time1: t2,
        price1: pr2,
        time2: t4,
        price2: pr4,
        time3: last_candle_time_sec,
        price3: price3_l,
        color: yellow,
    };
    Some((upper, lower))
}

fn fibo_color(label: &str) -> &'static str {
    match label {
        "14.6%" => "#66BB6A",
        "23.6%" => "#4CAF50",
        "38.2%" => "#8BC34A",
        "50%" => "#FFEB3B",
        "61.8%" => "#FF9800",
        _ => "#8BC34A",
    }
}

fn elliott_result_to_annotations(
    r: iqai_core::elliott_detector::ElliottDetectorResult,
) -> ElliottAnnotations {
    let wave_points: Vec<_> = r
        .wave_points
        .into_iter()
        .map(|p| ElliottWavePoint {
            time: p.time,
            price: p.price,
            label: p.label,
        })
        .collect();

    let wave_legs: Vec<_> = r
        .wave_legs
        .into_iter()
        .map(|l| {
            let color = leg_color(&l.label).to_string();
            ElliottWaveLeg {
                time1: l.time1,
                price1: l.price1,
                time2: l.time2,
                price2: l.price2,
                label: l.label,
                color,
                dotted: l.dotted,
            }
        })
        .collect();

    let fibo_levels: Vec<_> = r
        .fibo_levels
        .into_iter()
        .map(|f| FiboLevel {
            time1: f.time1,
            time2: f.time2,
            price: f.price,
            label: f.label.clone(),
            color: fibo_color(&f.label).to_string(),
        })
        .collect();

    let impulse_state = r.impulse_state.map(|s| ImpulseState {
        stage: s.stage,
        message: s.message,
        is_bullish: s.is_bullish,
        setup_w3: s.setup_w3,
        setup_w5: s.setup_w5,
    });

    let projections = r.projections.map(|v| {
        v.into_iter()
            .map(|p| ElliottProjection {
                price: p.price,
                label: p.label,
            })
            .collect()
    });

    ElliottAnnotations {
        wave_legs,
        fibo_levels,
        formation: r.formation,
        formation_type: r.formation_type,
        wave_points,
        w5_targets: r.w5_targets,
        impulse_state,
        validation_ok: r.validation_ok,
        validation_msg: r.validation_msg,
        in_progress: r.in_progress,
        projections,
        channel_upper: None,
        channel_lower: None,
        degree: r.degree,
        truncation: r.truncation,
        alternation: r.alternation,
        channel: r.channel,
        w5_confirmation: r.w5_confirmation,
        w3_volume_ok: r.w3_volume_ok,
        w5_time_targets: r.w5_time_targets,
        throw_over: r.throw_over,
        extended_wave: r.extended_wave,
        w1_w5_eq: r.w1_w5_eq,
        w5_divergence: r.w5_divergence,
        alternation_structural: r.alternation_structural,
        w2_corr_type: r.w2_corr_type,
        w4_corr_type: r.w4_corr_type,
        diagonal_sub: r.diagonal_sub,
        diagonal_inner_counts: r.diagonal_inner_counts,
        corr_setup: r.corr_setup,
        channel_alt: r.channel_alt,
        channel_semilog_target: r.channel_semilog_target,
        w5_vol_extension: r.w5_vol_extension,
        w4_golden_section: r.w4_golden_section,
        w2_depth_target: r.w2_depth_target,
        w4_depth_target: r.w4_depth_target,
        subwave_validation: r.subwave_validation,
        nested_extension: r.nested_extension,
        corr_subwave_validation: r.corr_subwave_validation,
    }
}

/// Geçmiş verilerde geçerli Elliott formasyonlarını tara (sliding window)
pub fn scan_elliott_formations(candles: &[Candle], config: &Config) -> Vec<HistoricalFormation> {
    let pivot_len = config.pivot_length as usize;
    let min_len = pivot_len * 4 + 2;
    let step = pivot_len.max(10); // Adım: pivot_length veya en az 10 bar

    let mut results = Vec::new();
    let mut last_w4_time: Option<i64> = None;

    for end in (min_len..candles.len()).step_by(step) {
        let slice = &candles[..=end];
        let ew = elliott_result_to_annotations(compute_elliott(slice, config, false));

        if ew.validation_ok != Some(true) || ew.wave_points.len() < 4 {
            continue;
        }

        let last_time = ew
            .wave_points
            .iter()
            .find(|p| matches!(p.label.as_str(), "4" | "C" | "E"))
            .map(|p| p.time)
            .or_else(|| ew.wave_points.last().map(|p| p.time));
        if let Some(t) = last_time {
            if last_w4_time == Some(t) {
                continue;
            }
            last_w4_time = Some(t);
        }

        let is_bullish = ew
            .impulse_state
            .as_ref()
            .map(|s| s.is_bullish)
            .unwrap_or(true);

        results.push(HistoricalFormation {
            end_time: candles[end].time,
            formation: ew.formation.clone(),
            formation_type: ew.formation_type.clone(),
            is_bullish,
            wave_points: ew.wave_points,
            wave_legs: ew.wave_legs,
            w5_targets: ew.w5_targets,
        });
    }

    results
}
