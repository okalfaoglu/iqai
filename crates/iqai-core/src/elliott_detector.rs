//! Merkezi Elliott Wave tespit modülü – Web GUI ve Robot tarafından ortak kullanılır.
//!
//! Swing toplama, formasyon tespiti, hedef/projeksiyon hesapları tek kaynaktan yapılır.

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::elliott::{
    check_alternation_depth, check_alternation_structural, classify_corrective_type,
    classify_diagonal_sub_structure, compute_impulse_channel, compute_impulse_channel_alt,
    compute_impulse_channel_semilog, compute_setup_triangle_e, compute_setup_zigzag_c,
    depth_of_corrective_target_from_subwaves,
    detect_extended_wave, detect_nested_extension,
    detect_throw_over, detect_truncation, flat_valid_detailed, time_projection_w5,
    validate_corrective_subwaves, validate_diagonal, validate_impulse,
    validate_impulse_with_w5, validate_subwave_deep, validate_subwave_structure,
    validate_triangle_abcde,
    validate_zigzag_abc, w1_w5_equality, AlternationResult, DiagonalSubStructure, FlatType,
    ImpulseChannel, WaveDegree,
};
use crate::impulse_detector::{detect_impulse, W5Confirmation};
use crate::indicators::{pivot_high, pivot_low, rsi};
use crate::types::Candle;

/// Dalga noktası (time ms, price, label)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElliottWavePointCore {
    pub time: i64,
    pub price: f64,
    pub label: String,
}

/// Dalga bacak (time1, price1, time2, price2, label, dotted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElliottWaveLegCore {
    pub time1: i64,
    pub price1: f64,
    pub time2: i64,
    pub price2: f64,
    pub label: String,
    pub dotted: bool,
}

/// Fibonacci seviye
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiboLevelCore {
    pub time1: i64,
    pub time2: i64,
    pub price: f64,
    pub label: String,
}

/// Projeksiyon hedefi
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElliottProjectionCore {
    pub price: f64,
    pub label: String,
}

/// Impulse tespit durumu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpulseStateCore {
    pub stage: String,
    pub message: String,
    pub is_bullish: bool,
    pub setup_w3: Option<serde_json::Value>,
    pub setup_w5: Option<serde_json::Value>,
}

/// Merkezi Elliott tespit sonucu – Web ve Robot aynı veriyi kullanır
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElliottDetectorResult {
    pub wave_points: Vec<ElliottWavePointCore>,
    pub wave_legs: Vec<ElliottWaveLegCore>,
    pub fibo_levels: Vec<FiboLevelCore>,
    pub formation: String,
    pub formation_type: String,
    /// Impulse W5 hedefleri: (W1=W5, 61.8%×(0-3), W4 inv 123.6%) – Robot TP için
    pub w5_targets: Option<(f64, f64, f64)>,
    pub impulse_state: Option<ImpulseStateCore>,
    pub validation_ok: Option<bool>,
    pub validation_msg: Option<String>,
    pub in_progress: Option<bool>,
    pub projections: Option<Vec<ElliottProjectionCore>>,
    /// Dalga derecesi (heuristic; TF ve bar sayısına göre)
    pub degree: Option<WaveDegree>,
    /// Truncation: W5 W3'ü aşamadıysa true (trend zayıflama sinyali)
    pub truncation: Option<bool>,
    /// W2-W4 alternation durumu
    pub alternation: Option<AlternationResult>,
    /// Impulse kanal (W2-W4 baz + W3 paralel) – W5 hedefi için
    pub channel: Option<ImpulseChannel>,
    /// W5 giriş teyidi: W4 sonrası mini ChoCh/BOS
    pub w5_confirmation: Option<W5Confirmation>,
    /// W3 hacmi en yüksek mi (PDF: "wave 3 usually has the heaviest volume")
    pub w3_volume_ok: Option<bool>,
    /// W5 süre tahmini: (W1 süresiyle eşit, %61.8, %161.8) – timestamp/1000 olarak bitiş zamanları
    pub w5_time_targets: Option<(i64, i64, i64)>,
    /// W5 throw-over: kanal çizgisini aştıysa true (sert dönüş sinyali)
    pub throw_over: Option<bool>,
    /// Extended dalga: (hangi dalga: 1/3/5, oran)
    pub extended_wave: Option<(u8, f64)>,
    /// W1≈W5 eşitlik oranı (1.0'a yakınsa eşit)
    pub w1_w5_eq: Option<f64>,
    /// W3-W5 RSI bearish/bullish divergence (W5 zayıflama sinyali)
    pub w5_divergence: Option<bool>,
    /// Yapısal alternation (W2 pattern tipi vs W4 pattern tipi)
    pub alternation_structural: Option<AlternationResult>,
    /// W2 dalga tipi (Sharp/Sideways)
    pub w2_corr_type: Option<crate::elliott::CorrWaveType>,
    /// W4 dalga tipi (Sharp/Sideways)
    pub w4_corr_type: Option<crate::elliott::CorrWaveType>,
    /// Diagonal iç yapı: LD(5-3-5-3-5) veya ED(3-3-3-3-3)
    pub diagonal_sub: Option<DiagonalSubStructure>,
    /// Diagonal her dalga iç swing sayıları [W1,W2,W3,W4,W5]
    pub diagonal_inner_counts: Option<[usize; 5]>,
    /// Corrective trade setup (Zigzag C veya Triangle E breakout)
    pub corr_setup: Option<crate::elliott::CorrSetup>,
    /// Alternatif kanal (W3 güçlü ise W1 tepesinden paralel)
    pub channel_alt: Option<ImpulseChannel>,
    /// Semi-log kanal W5 hedefi
    pub channel_semilog_target: Option<f64>,
    /// W5 extension sinyali: W5 vol >= W3 vol ise true
    pub w5_vol_extension: Option<bool>,
    /// W4 Golden Section: W4'ün impulse toplam aralık içindeki oranı
    pub w4_golden_section: Option<f64>,
    /// W2 depth target: W1'in iç W4 seviyesi (beklenen W2 bitiş)
    pub w2_depth_target: Option<f64>,
    /// W4 depth target: W3'ün iç W4 seviyesi (beklenen W4 bitiş)
    pub w4_depth_target: Option<f64>,
    /// Alt-dalga yapısı doğrulaması (W1-W5 iç swing sayıları)
    pub subwave_validation: Option<crate::elliott::SubWaveValidation>,
    /// Nested extension tespiti (W3 iç extension)
    pub nested_extension: Option<(bool, f64)>,
    /// Corrective alt-dalga doğrulaması (Zigzag/Flat A,B,C iç yapı)
    pub corr_subwave_validation: Option<crate::elliott::CorrSubWaveValidation>,
    /// Dalgalardan sonra oluşacak/oluşan formasyonlar için referans seviyeleri (hesaplama referansı)
    pub next_formation_ref: Option<crate::elliott::NextFormationRefLevels>,
}

impl Default for ElliottDetectorResult {
    fn default() -> Self {
        Self {
            wave_points: vec![],
            wave_legs: vec![],
            fibo_levels: vec![],
            formation: "—".to_string(),
            formation_type: "—".to_string(),
            w5_targets: None,
            impulse_state: None,
            validation_ok: None,
            validation_msg: None,
            in_progress: None,
            projections: None,
            degree: None,
            truncation: None,
            alternation: None,
            channel: None,
            w5_confirmation: None,
            w3_volume_ok: None,
            w5_time_targets: None,
            throw_over: None,
            extended_wave: None,
            w1_w5_eq: None,
            w5_divergence: None,
            alternation_structural: None,
            w2_corr_type: None,
            w4_corr_type: None,
            diagonal_sub: None,
            diagonal_inner_counts: None,
            corr_setup: None,
            channel_alt: None,
            channel_semilog_target: None,
            w5_vol_extension: None,
            w4_golden_section: None,
            w2_depth_target: None,
            w4_depth_target: None,
            subwave_validation: None,
            nested_extension: None,
            corr_subwave_validation: None,
            next_formation_ref: None,
        }
    }
}

/// Elliott Wave için min. fiyat hareketi (deviation) – gürültülü pivot'ları filtreler
const ELLIOTT_SWING_DEVIATION_PCT: f64 = 0.005; // %0.5 – EWM/Zigzag ile uyumlu, gürültü filtresi

/// Pivot bazlı swing noktalarını topla (alternating high/low).
/// Deviation filtresi ile W2>W1 gibi yapısal hataların önüne geçilir.
pub fn collect_swings(candles: &[Candle], pivot_len: usize) -> Vec<(i64, f64, bool)> {
    let mut swings = Vec::new();
    let mut last_was_high = Option::<bool>::None;
    let mut last_price: Option<f64> = None;

    for i in (pivot_len * 2 + 1)..candles.len().saturating_sub(pivot_len) {
        let sub = &candles[..=i + pivot_len];
        let pivot_idx = sub.len() - 1 - pivot_len;
        let t = candles[pivot_idx].time;

        if let Some(ph) = pivot_high(sub, pivot_len) {
            if last_was_high != Some(true) {
                let ok = last_price
                    .map(|lp| (ph - lp).abs() / lp.max(1e-10) >= ELLIOTT_SWING_DEVIATION_PCT)
                    .unwrap_or(true);
                if ok {
                    swings.push((t, ph, true));
                    last_was_high = Some(true);
                    last_price = Some(ph);
                }
            }
        }
        if let Some(pl_val) = pivot_low(sub, pivot_len) {
            if last_was_high != Some(false) {
                let ok = last_price
                    .map(|lp| (pl_val - lp).abs() / lp.max(1e-10) >= ELLIOTT_SWING_DEVIATION_PCT)
                    .unwrap_or(true);
                if ok {
                    swings.push((t, pl_val, false));
                    last_was_high = Some(false);
                    last_price = Some(pl_val);
                }
            }
        }
    }
    swings
}

/// Merkezi Elliott Wave tespiti – tek kaynak, Web GUI ve Robot bu fonksiyonu kullanır
pub fn compute_elliott(candles: &[Candle], config: &Config, invert: bool) -> ElliottDetectorResult {
    let pivot_len = config.pivot_length as usize;

    if candles.len() < pivot_len * 4 + 2 {
        return ElliottDetectorResult::default();
    }

    let imp = detect_impulse(candles, config);
    let impulse_state = Some(ImpulseStateCore {
        stage: format!("{:?}", imp.stage),
        message: imp.message.clone(),
        is_bullish: imp.is_bullish,
        setup_w3: imp.setup_w3.as_ref().map(|s| {
            serde_json::json!({
                "entry": s.entry,
                "sl": s.stop_loss,
                "tp1": s.tp1,
                "tp2": s.tp2,
                "is_long": s.is_long,
                "rr1": s.rr1,
                "rr2": s.rr2
            })
        }),
        setup_w5: imp.setup_w5.as_ref().map(|s| {
            serde_json::json!({
                "entry": s.entry,
                "sl": s.stop_loss,
                "tp": s.tp,
                "tp_alt": s.tp_alternate,
                "is_long": s.is_long,
                "rr": s.rr
            })
        }),
    });

    let swings = collect_swings(candles, pivot_len);

    let (recent, is_bullish, impulse_complete) =
        find_impulse_window(&swings, imp.is_bullish, invert);

    let mut result = build_impulse_result(
        candles,
        &recent,
        is_bullish,
        impulse_complete,
        &imp,
        pivot_len,
        &swings,
    );

    let last4: Vec<_> = if swings.len() >= 4 {
        swings[swings.len() - 4..].to_vec()
    } else {
        vec![]
    };

    let zigzag_ok = check_zigzag(&last4);

    let (zigzag_valid, _, _) = zigzag_ok;
    if zigzag_valid && (result.validation_ok != Some(true) || result.formation == "—") {
        result = build_zigzag_result(&last4, &swings);
    } else if let Some(flat_typ) = check_flat(&last4) {
        result = build_flat_result(&last4, flat_typ, &swings);
    } else if swings.len() >= 6 && (result.validation_ok != Some(true) || result.formation == "—") {
        if let Some(tri) = try_triangle(&swings) {
            result = tri;
        } else if let Some(dzz) = try_double_zigzag(&swings) {
            result = dzz;
        } else if let Some(dt) = try_double_three(&swings) {
            result = dt;
        } else if let Some(tzz) = try_triple_zigzag(&swings) {
            result = tzz;
        } else if let Some(tt) = try_triple_three(&swings) {
            result = tt;
        }
    }

    result.in_progress = if recent.len() == 3 || recent.len() == 4 {
        Some(result.validation_ok != Some(true))
    } else {
        None
    };

    result.projections = compute_projections(
        candles,
        &recent,
        &last4,
        &result.formation,
        &result.formation_type,
        is_bullish,
        &imp,
    );
    // Dalga derecesini bar sayısına göre yaklaşıkla
    result.degree = Some(infer_wave_degree(candles.len()));
    result.impulse_state = impulse_state;

    // Sonraki formasyon referans seviyeleri (Impulse tamamlandıysa düzeltme A/B/C)
    if result.validation_ok == Some(true)
        && (result.formation.starts_with("Impulse") || result.formation_type.contains("İtki"))
    {
        let p0 = result.wave_points.iter().find(|p| p.label == "0").map(|p| p.price);
        let p5 = result.wave_points.iter().find(|p| p.label == "5").map(|p| p.price);
        if let (Some(w0), Some(w5)) = (p0, p5) {
            let post = crate::elliott::compute_post_impulse_correction_ref(w0, w5, is_bullish);
            let expected: Vec<String> = crate::elliott::ElliottFormation::from_formation_name(&result.formation)
                .map(|f| {
                    f.next_formation_after_completion(false)
                        .iter()
                        .map(|e| format!("{:?}", e))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec!["Zigzag".into(), "Flat".into(), "Triangle".into()]);
            result.next_formation_ref = Some(crate::elliott::NextFormationRefLevels {
                expected_formations: expected,
                post_impulse_correction: Some(post),
            });
        }
    }

    result
}

fn infer_wave_degree(bar_count: usize) -> WaveDegree {
    match bar_count {
        n if n >= 5000 => WaveDegree::Grand,
        n if n >= 2000 => WaveDegree::Primary,
        n if n >= 800 => WaveDegree::Intermediate,
        n if n >= 300 => WaveDegree::Minor,
        n if n >= 120 => WaveDegree::Minute,
        n if n >= 40 => WaveDegree::Minuette,
        _ => WaveDegree::SubMinuette,
    }
}

fn find_impulse_window(
    swings: &[(i64, f64, bool)],
    default_bull: bool,
    invert: bool,
) -> (Vec<(i64, f64, bool)>, bool, bool) {
    let take = swings.len().min(9);
    let base_start = swings.len().saturating_sub(take);
    let base: Vec<_> = swings[base_start..].to_vec();

    if base.len() < 5 {
        return (base, default_bull, false);
    }

    let mut valid_6: Option<(Vec<_>, bool)> = None;
    let mut valid_5: Option<(Vec<_>, bool)> = None;
    let mut fallback: Option<(Vec<_>, bool)> = None;
    let order = if invert {
        [!default_bull, default_bull]
    } else {
        [default_bull, !default_bull]
    };

    for is_bull in order {
        let need_first_high = !is_bull;

        if base.len() >= 6 {
            for s in (0..=base.len() - 6).rev() {
                let w = &base[s..s + 6];
                let first_high = w[0].2;
                if first_high != need_first_high
                    || w[1].2 == first_high
                    || w[2].2 != first_high
                    || w[3].2 == first_high
                    || w[4].2 != first_high
                    || w[5].2 != first_high
                {
                    continue;
                }
                let (p0, p1, p2, p3, p4, p5) =
                    (w[0].1, w[1].1, w[2].1, w[3].1, w[4].1, w[5].1);
                let (w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext) = if is_bull {
                    (p0, p1, p0, p2, p3, p4)
                } else {
                    (p0, p0, p1, p2, p3, p4)
                };
                let val = validate_impulse(w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext, is_bull);
                let w5_ok = if is_bull { p5 > p3 } else { p5 < p3 };
                if val.formation_valid && w5_ok {
                    valid_6 = Some((w.to_vec(), is_bull));
                    break;
                }
            }
        }
        if valid_6.is_some() {
            break;
        }

        for s in (0..=base.len().saturating_sub(5)).rev() {
            let w = &base[s..s + 5];
            let first_high = w[0].2;
            if first_high != need_first_high
                || w[1].2 == first_high
                || w[2].2 != first_high
                || w[3].2 == first_high
                || w[4].2 != first_high
            {
                continue;
            }
            let (p0, p1, p2, p3, p4) = (w[0].1, w[1].1, w[2].1, w[3].1, w[4].1);
            let (w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext) = if is_bull {
                (p0, p1, p0, p2, p3, p4)
            } else {
                (p0, p0, p1, p2, p3, p4)
            };
            let val = validate_impulse(w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext, is_bull);
            if fallback.is_none() {
                fallback = Some((w.to_vec(), is_bull));
            }
            if val.formation_valid {
                valid_5 = Some((w.to_vec(), is_bull));
                break;
            }
        }
        if valid_6.is_some() || valid_5.is_some() {
            break;
        }
    }

    if let Some((w, b)) = valid_6 {
        (w, b, true)
    } else if let Some((w, b)) = valid_5 {
        (w, b, false)
    } else {
        fallback
            .map(|(w, b)| (w, b, false))
            .unwrap_or_else(|| {
                (
                    base[base.len().saturating_sub(5)..].to_vec(),
                    default_bull,
                    false,
                )
            })
    }
}

fn leg(t1: i64, p1: f64, t2: i64, p2: f64, label: &str, dotted: bool) -> ElliottWaveLegCore {
    ElliottWaveLegCore {
        time1: t1,
        price1: p1,
        time2: t2,
        price2: p2,
        label: label.to_string(),
        dotted,
    }
}

fn pt(time: i64, price: f64, label: &str) -> ElliottWavePointCore {
    ElliottWavePointCore {
        time,
        price,
        label: label.to_string(),
    }
}

/// W4 sonrası mumları analiz ederek W5 giriş teyidi ara.
///
/// Yaklaşım: W4 noktasından itibaren mumlardaki mini swing'lerde yapısal kırılım aranır:
/// 1. **Mini BOS**: W4 sonrası oluşan ilk mini swing high/low'dan sonra fiyat bu seviyeyi kırarsa
/// 2. **Momentum**: W4'ten itibaren ardışık N mum trend yönünde kapanırsa (basit filtre)
///
/// Alt TF'ye gerek kalmadan aynı TF'nin W4-sonrası bölgesinde kırılım aranır.
fn detect_w5_confirmation(
    candles: &[Candle],
    w4_time: i64,
    w4_price: f64,
    w3_price: f64,
    is_bullish: bool,
) -> W5Confirmation {
    let w4_idx = candles.iter().position(|c| c.time >= w4_time);
    let start = match w4_idx {
        Some(idx) => idx + 1,
        None => {
            return W5Confirmation {
                confirmed: false,
                signal_type: "pending".to_string(),
                price: None,
                time: None,
            }
        }
    };

    if start >= candles.len() {
        return W5Confirmation {
            confirmed: false,
            signal_type: "pending".to_string(),
            price: None,
            time: None,
        };
    }

    let post_w4 = &candles[start..];
    if post_w4.is_empty() {
        return W5Confirmation {
            confirmed: false,
            signal_type: "pending".to_string(),
            price: None,
            time: None,
        };
    }

    // Mini swing high/low'ları bul (3 bar lookback)
    let mini_pl = 2usize;
    let mut mini_swing_high: Option<(f64, i64)> = None;
    let mut mini_swing_low: Option<(f64, i64)> = None;

    for i in mini_pl..post_w4.len().saturating_sub(mini_pl) {
        let c = &post_w4[i];
        let is_ph = (0..mini_pl).all(|j| post_w4[i - j - 1].high <= c.high)
            && (1..=mini_pl.min(post_w4.len() - i - 1)).all(|j| post_w4[i + j].high <= c.high);
        let is_pl = (0..mini_pl).all(|j| post_w4[i - j - 1].low >= c.low)
            && (1..=mini_pl.min(post_w4.len() - i - 1)).all(|j| post_w4[i + j].low >= c.low);

        if is_ph && mini_swing_high.map_or(true, |(h, _)| c.high > h) {
            mini_swing_high = Some((c.high, c.time));
        }
        if is_pl && mini_swing_low.map_or(true, |(l, _)| c.low < l) {
            mini_swing_low = Some((c.low, c.time));
        }
    }

    // Mini BOS kontrolü: son mumlar mini swing'i kırdı mı?
    if is_bullish {
        // Bullish: W4 dip sonrası mini swing high oluştu, sonra fiyat o high'ı kırıyorsa → BOS
        if let Some((sh, _sh_t)) = mini_swing_high {
            if let Some(last) = post_w4.last() {
                if last.close > sh && sh > w4_price {
                    return W5Confirmation {
                        confirmed: true,
                        signal_type: "mini_bos".to_string(),
                        price: Some(last.close),
                        time: Some(last.time / 1000),
                    };
                }
            }
        }
        // Momentum: W4'ten 3+ mum üst üste bullish kapanış
        let consec = post_w4.iter().rev().take_while(|c| c.close > c.open).count();
        if consec >= 3 {
            let last = post_w4.last().unwrap();
            return W5Confirmation {
                confirmed: true,
                signal_type: "momentum".to_string(),
                price: Some(last.close),
                time: Some(last.time / 1000),
            };
        }
    } else {
        // Bearish: W4 zirve sonrası mini swing low kırılırsa → BOS
        if let Some((sl, _sl_t)) = mini_swing_low {
            if let Some(last) = post_w4.last() {
                if last.close < sl && sl < w4_price {
                    return W5Confirmation {
                        confirmed: true,
                        signal_type: "mini_bos".to_string(),
                        price: Some(last.close),
                        time: Some(last.time / 1000),
                    };
                }
            }
        }
        let consec = post_w4.iter().rev().take_while(|c| c.close < c.open).count();
        if consec >= 3 {
            let last = post_w4.last().unwrap();
            return W5Confirmation {
                confirmed: true,
                signal_type: "momentum".to_string(),
                price: Some(last.close),
                time: Some(last.time / 1000),
            };
        }
    }

    // ChoCh kontrolü: W4 sonrası fiyat W3 yönünde belirgin hareket etti mi
    let progress = if is_bullish {
        let max_after_w4 = post_w4.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
        (max_after_w4 - w4_price) / (w3_price - w4_price).abs().max(1e-10)
    } else {
        let min_after_w4 = post_w4.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        (w4_price - min_after_w4) / (w4_price - w3_price).abs().max(1e-10)
    };

    if progress > 0.382 {
        let last = post_w4.last().unwrap();
        return W5Confirmation {
            confirmed: true,
            signal_type: "choch".to_string(),
            price: Some(last.close),
            time: Some(last.time / 1000),
        };
    }

    W5Confirmation {
        confirmed: false,
        signal_type: "pending".to_string(),
        price: None,
        time: None,
    }
}

fn build_impulse_result(
    candles: &[Candle],
    recent: &[(i64, f64, bool)],
    is_bullish: bool,
    impulse_complete: bool,
    imp: &crate::impulse_detector::ImpulseDetectorState,
    _pivot_len: usize,
    all_swings: &[(i64, f64, bool)],
) -> ElliottDetectorResult {
    let mut result = ElliottDetectorResult::default();

    if recent.len() < 3 {
        return result;
    }

    let (t0, p0, _) = recent[0];
    let (t1, p1, _) = recent[1];
    let (t2, p2, _) = recent[2];
    let t0s = t0 / 1000;
    let t1s = t1 / 1000;
    let t2s = t2 / 1000;

    result.wave_points.push(pt(t0s, p0, "0"));
    result.wave_points.push(pt(t1s, p1, "1"));
    result.wave_points.push(pt(t2s, p2, "2"));

    if recent.len() == 3 {
        result.formation = "Impulse (1-2)".to_string();
        result.formation_type = "Motif (İtki)".to_string();
    } else if recent.len() == 4 {
        result.formation = "Impulse (1-2-3)".to_string();
        result.formation_type = "Motif (İtki)".to_string();
    }

    result.wave_legs.push(leg(t0s, p0, t1s, p1, "1", false));
    result.wave_legs.push(leg(t1s, p1, t2s, p2, "2", false));

    let last_t = candles.last().map(|c| c.time / 1000).unwrap_or(t2s);

    if recent.len() == 3 {
        let w1_len = (p1 - p0).abs();
        let w3_tgt = if imp.is_bullish {
            p2 + w1_len * 1.382
        } else {
            p2 - w1_len * 1.382
        };
        result.wave_legs.push(leg(t2s, p2, last_t, w3_tgt, "3", true));
    } else if recent.len() == 4 {
        let (t3, p3, _) = recent[3];
        let t3s = t3 / 1000;
        result.wave_points.push(pt(t3s, p3, "3"));
        result.wave_legs.push(leg(t2s, p2, t3s, p3, "3", false));
        let w4_est = if imp.is_bullish {
            p3 - 0.382 * (p3 - p2).abs()
        } else {
            p3 + 0.382 * (p3 - p2).abs()
        };
        let w1_len = (p1 - p0).abs();
        let w5_eq = if imp.is_bullish {
            w4_est + w1_len
        } else {
            w4_est - w1_len
        };
        result.wave_legs.push(leg(t3s, p3, last_t, w4_est, "4", true));
        result.wave_legs.push(leg(last_t, w4_est, last_t, w5_eq, "5", true));
    }

    if recent.len() >= 5 {
        let (t3, p3, _) = recent[3];
        let (t4, p4, _) = recent[4];
        let t3s = t3 / 1000;
        let t4s = t4 / 1000;

        result.wave_points.push(pt(t3s, p3, "3"));
        result.wave_points.push(pt(t4s, p4, "4"));
        result.wave_legs.push(leg(t2s, p2, t3s, p3, "3", false));
        result.wave_legs.push(leg(t3s, p3, t4s, p4, "4", false));

        if impulse_complete && recent.len() >= 6 {
            let (t5, p5, _) = recent[5];
            result.wave_points.push(pt(t5 / 1000, p5, "5"));
            result.wave_legs.push(leg(t4s, p4, t5 / 1000, p5, "5", false));
        }

        let bullish = is_bullish;
        let (w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext) = if bullish {
            (p0, p1, p0, p2, p3, p4)
        } else {
            (p0, p0, p1, p2, p3, p4)
        };
        let w5_opt = if impulse_complete && recent.len() >= 6 {
            Some(recent[5].1)
        } else {
            None
        };
        let val = validate_impulse_with_w5(
            w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext, w5_opt, bullish,
        );
        let diag = validate_diagonal(w0, w1_h, w1_l, w2_ext, w3_ext, w4_ext, bullish);

        let (validation_ok_val, validation_msg_val, formation_label, formation_type_label) =
            if val.formation_valid {
                (
                    Some(true),
                    Some("Kurallar geçerli".to_string()),
                    "Impulse".to_string(),
                    "Motif (İtki)".to_string(),
                )
            } else if !val.w4_valid && val.w2_valid && val.w3_valid && diag.formation_valid {
                // EWM Spec: 4 Diagonal türü – Leading/Ending × Contracting/Expanding
                let is_leading = !impulse_complete && recent.len() <= 5;

                // Diagonal iç yapı kontrolü: her dalga segmentinin iç swing sayısını say
                let inner_counts = if recent.len() >= 5 {
                    let mut counts = [0usize; 5];
                    for w in 0..5 {
                        if w + 1 < recent.len() {
                            let inner = collect_inner_swings_between(
                                all_swings, recent[w].0, recent[w + 1].0,
                            );
                            counts[w] = inner.len();
                        }
                    }
                    Some(counts)
                } else {
                    None
                };

                let sub_struct = inner_counts.as_ref().map(classify_diagonal_sub_structure);
                result.diagonal_sub = sub_struct;
                result.diagonal_inner_counts = inner_counts;

                let sub_str = match sub_struct {
                    Some(DiagonalSubStructure::LeadingMotive) => " [5-3-5-3-5]",
                    Some(DiagonalSubStructure::EndingCorrective) => " [3-3-3-3-3]",
                    Some(DiagonalSubStructure::Mixed) => " [karışık]",
                    None => "",
                };

                let shape_str = match diag.shape {
                    Some(crate::elliott::DiagonalShape::Contracting) => "Daralan",
                    Some(crate::elliott::DiagonalShape::Expanding) => "Genişleyen",
                    None => "Diyagonal",
                };
                let (position, position_en) = if is_leading {
                    ("Başlangıç Diyagonal", "Leading Diagonal")
                } else {
                    ("Bitiş Diyagonal", "Ending Diagonal")
                };
                let formation_type_label = if diag.shape.is_some() {
                    format!("Motif ({} {})", position, shape_str)
                } else {
                    format!("Motif ({})", position)
                };
                let formation_label = if diag.shape.is_some() {
                    format!("{} ({})", position_en, shape_str)
                } else {
                    "Diagonal".to_string()
                };
                (
                    Some(true),
                    Some(format!("Diagonal: W4-W1 örtüşmesi kabul (EWM){}", sub_str)),
                    formation_label,
                    formation_type_label,
                )
            } else {
                let mut parts = vec![];
                if !val.w2_valid {
                    parts.push("W2<=W0");
                }
                if !val.w3_valid {
                    parts.push("W3 en kısa");
                }
                if !val.w4_valid {
                    parts.push("W4-W1 örtüşme");
                }
                if !val.no_triple_extension_valid {
                    parts.push("Triple extension");
                }
                let msg = format!("İhlal: {}", parts.join(", "));
                (
                    Some(false),
                    Some(msg),
                    "Impulse".to_string(),
                    "Motif (İtki)".to_string(),
                )
            };

        result.validation_ok = validation_ok_val;
        result.validation_msg = validation_msg_val;
        result.formation = formation_label;
        result.formation_type = formation_type_label;

        if result.validation_ok == Some(false) {
            result.wave_legs.clear();
            result.wave_points.clear();
            result.fibo_levels.clear();
            result.w5_targets = None;
            result.formation = "—".to_string();
            result.formation_type = "—".to_string();
        } else {
            let w1_len = (p1 - p0).abs();
            let w1_3_len = (p3 - p0).abs();
            let w4_len = (p3 - p4).abs();
            result.w5_targets = Some((
                if bullish { p4 + w1_len } else { p4 - w1_len },
                if bullish { p4 + 0.618 * w1_3_len } else { p4 - 0.618 * w1_3_len },
                if bullish { p4 + 1.236 * w4_len } else { p4 - 1.236 * w4_len },
            ));

            if !impulse_complete {
                result
                    .wave_legs
                    .push(leg(t4s, p4, last_t, result.w5_targets.unwrap().0, "5", true));
            }

            // Truncation: W5 tamamlandıysa W5 < W3 kontrolü
            if impulse_complete && recent.len() >= 6 {
                let p5 = recent[5].1;
                let is_truncated = detect_truncation(p3, p5, bullish);
                result.truncation = Some(is_truncated);
                if is_truncated {
                    let existing = result.validation_msg.take().unwrap_or_default();
                    result.validation_msg = Some(format!(
                        "{}; Truncation: W5 W3'ü aşamadı (trend zayıflama)",
                        existing
                    ));
                }
            }

            // Throw-over: W5 kanal çizgisini aştı mı
            if impulse_complete && recent.len() >= 6 {
                let p5 = recent[5].1;
                if let Some(ref ch) = result.channel {
                    let is_to = detect_throw_over(p5, ch.w5_channel_target, bullish);
                    result.throw_over = Some(is_to);
                    if is_to {
                        let existing = result.validation_msg.take().unwrap_or_default();
                        result.validation_msg = Some(format!(
                            "{}; Throw-over: W5 kanalı aştı (sert dönüş riski)",
                            existing
                        ));
                    }
                }
            }

            // Extended dalga + W1≈W5 eşitliği
            if impulse_complete && recent.len() >= 6 {
                let p5 = recent[5].1;
                let w3_len_abs = (p3 - p2).abs();
                let w5_len_abs = (p5 - p4).abs();
                let (ext_wave, ext_ratio) = detect_extended_wave(w1_len, w3_len_abs, w5_len_abs);
                result.extended_wave = Some((ext_wave, ext_ratio));
                if ext_wave == 3 {
                    result.w1_w5_eq = Some(w1_w5_equality(w1_len, w5_len_abs));
                }
            }

            // Alternation: W2 vs W4 derinlik kontrolü
            if w1_len > 1e-10 {
                let w3_len = (p3 - p2).abs();
                let w2_retrace = (p1 - p2).abs() / w1_len;
                let w4_retrace = if w3_len > 1e-10 {
                    (p3 - p4).abs() / w3_len
                } else {
                    0.0
                };
                let alt = check_alternation_depth(w2_retrace, w4_retrace);
                result.alternation = Some(alt);
                if alt == AlternationResult::Violation {
                    let existing = result.validation_msg.take().unwrap_or_default();
                    result.validation_msg =
                        Some(format!("{}; Alternation ihlali: W2 ve W4 aynı derinlikte", existing));
                }

                // Yapısal alternation: W2 ve W4 iç swing sayısı + retrace ile formasyon tipi
                let w2_inner = collect_inner_swings_between(all_swings, recent[1].0, recent[2].0);
                let w4_inner = collect_inner_swings_between(all_swings, recent[3].0, recent[4].0);
                let w2_type = classify_corrective_type(w2_inner.len(), w2_retrace);
                let w4_type = classify_corrective_type(w4_inner.len(), w4_retrace);
                result.w2_corr_type = Some(w2_type);
                result.w4_corr_type = Some(w4_type);
                let struct_alt = check_alternation_structural(w2_type, w4_type);
                result.alternation_structural = Some(struct_alt);
                if struct_alt == AlternationResult::Violation {
                    let existing = result.validation_msg.take().unwrap_or_default();
                    result.validation_msg =
                        Some(format!("{}; Yapısal alt. ihlali: W2 ve W4 aynı tipte", existing));
                }
            }

            // Channeling: W2-W4 baz çizgisi + W3 paraleli → W5 kanal hedefi
            {
                let t5_est = if impulse_complete && recent.len() >= 6 {
                    recent[5].0 / 1000
                } else {
                    let w4_bars = (t4s - t3s).max(1);
                    t4s + w4_bars
                };
                if let Some(ch) = compute_impulse_channel(t2s, p2, t3s, p3, t4s, p4, t5_est, bullish) {
                    // PDF: W3 anormal güçlüyse (W3 > 2.618 × W1) W1 tepesinden paralel daha isabetli
                    let w3_len = (p3 - p2).abs();
                    let w3_strong = w1_len > 1e-10 && w3_len / w1_len > 2.618;
                    if w3_strong {
                        if let Some(alt) = compute_impulse_channel_alt(t1s, p1, t2s, p2, t4s, p4, t5_est) {
                            result.channel_alt = Some(alt);
                        }
                    }
                    result.channel = Some(ch);
                }
                // Semi-log kanal hedefi
                result.channel_semilog_target =
                    compute_impulse_channel_semilog(t2s, p2, t3s, p3, t4s, p4, t5_est);
            }

            // W5 giriş teyidi: W4 sonrası mumlardan mini ChoCh/BOS ara
            if !impulse_complete {
                result.w5_confirmation = Some(detect_w5_confirmation(candles, t4, p4, p3, bullish));
            }

            // Fibonacci zaman hedefleri: W5 bitiş zamanı tahmini
            {
                let w1_duration = t1s - t0s;
                if w1_duration > 0 {
                    let (d100, d618, d1618) = time_projection_w5(w1_duration);
                    result.w5_time_targets = Some((
                        t4s + d100,
                        t4s + d618,
                        t4s + d1618,
                    ));
                }
            }

            // Volume kuralı: W3 aralığındaki ortalama hacim, W1 ve W5 (varsa) aralığından yüksek olmalı
            {
                let avg_vol = |t_start: i64, t_end: i64| -> f64 {
                    let vols: Vec<f64> = candles
                        .iter()
                        .filter(|c| c.time >= t_start && c.time <= t_end)
                        .map(|c| c.volume)
                        .collect();
                    if vols.is_empty() { 0.0 } else { vols.iter().sum::<f64>() / vols.len() as f64 }
                };
                let vol_w1 = avg_vol(t0, t1);
                let vol_w3 = avg_vol(t2, t3);
                let vol_w5 = if impulse_complete && recent.len() >= 6 {
                    avg_vol(t4, recent[5].0)
                } else {
                    0.0
                };
                result.w3_volume_ok = Some(vol_w3 > vol_w1 && vol_w3 > vol_w5);

                // PDF p.28: "If volume in an advancing fifth wave is equal to or greater
                // than that in the third wave, an extension of the fifth is in force."
                if impulse_complete && vol_w5 > 0.0 {
                    result.w5_vol_extension = Some(vol_w5 >= vol_w3);
                }
            }

            // W5 RSI divergence: fiyat yeni extreme yaparken RSI zayıflıyorsa W5 tükenme sinyali
            if impulse_complete && recent.len() >= 6 {
                let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
                let w3_idx = candles.iter().position(|c| c.time == t3);
                let w5_time = recent[5].0;
                let w5_idx = candles.iter().position(|c| c.time == w5_time);
                if let (Some(i3), Some(i5)) = (w3_idx, w5_idx) {
                    let rsi_w3 = if i3 >= 14 { rsi(&closes[..=i3], 14) } else { None };
                    let rsi_w5 = if i5 >= 14 { rsi(&closes[..=i5], 14) } else { None };
                    if let (Some(r3), Some(r5)) = (rsi_w3, rsi_w5) {
                        let p5_price = recent[5].1;
                        let div = if bullish {
                            p5_price > p3 && r5 < r3
                        } else {
                            p5_price < p3 && r5 > r3
                        };
                        result.w5_divergence = Some(div);
                    }
                }
            }

            // PDF p.37: "wave 4 often divides the price range of an impulse wave
            // into the Golden Section (.382 when W5 not extended, .618 when extended)"
            if recent.len() >= 5 {
                let total_range = if bullish { p3 - p0 } else { p0 - p3 };
                if total_range.abs() > 1e-10 {
                    let w4_from_top = if bullish { p3 - p4 } else { p4 - p3 };
                    let golden = w4_from_top / total_range.abs();
                    result.w4_golden_section = Some(golden);
                }
            }

            // PDF p.26-27: Depth of Corrective Waves (multi-degree)
            // W2 → W1'in gerçek iç W4 seviyesinde bitmeli, W4 → W3'ün gerçek iç W4 seviyesinde bitmeli
            {
                let w1_inner = collect_inner_swings_between(all_swings, recent[0].0, recent[1].0);
                result.w2_depth_target = Some(depth_of_corrective_target_from_subwaves(
                    p0, p1, bullish, &w1_inner,
                ));
                let w3_inner = collect_inner_swings_between(all_swings, recent[2].0, recent[3].0);
                result.w4_depth_target = Some(depth_of_corrective_target_from_subwaves(
                    p2, p3, bullish, &w3_inner,
                ));
            }

            // PDF: Alt-dalga yapısı doğrulaması — W1,W3,W5 = 5-dalgalı, W2,W4 = 3-dalgalı
            // Level-1 + Level-2 recursive doğrulama
            {
                let mut sw_counts = [0usize; 5];
                let mut wave_inner_swings: [Vec<(i64, f64, bool)>; 5] = Default::default();
                for w in 0..5 {
                    if w + 1 < recent.len() {
                        let inner = collect_inner_swings_between(
                            all_swings, recent[w].0, recent[w + 1].0,
                        );
                        sw_counts[w] = inner.len();
                        wave_inner_swings[w] = inner;
                    }
                }
                let mut swv = validate_subwave_structure(sw_counts);

                // Level-2: her dalganın iç alt-dalgaları arasındaki iç-iç swing sayıları
                let mut level2_counts: [Vec<usize>; 5] = Default::default();
                for w in 0..5 {
                    let inner = &wave_inner_swings[w];
                    if inner.len() < 2 {
                        continue;
                    }
                    let wave_start_t = recent[w].0;
                    let wave_end_t = if w + 1 < recent.len() { recent[w + 1].0 } else { continue };
                    let mut sub_counts = Vec::new();
                    let boundaries: Vec<i64> = std::iter::once(wave_start_t)
                        .chain(inner.iter().map(|(t, _, _)| *t))
                        .chain(std::iter::once(wave_end_t))
                        .collect();
                    for pair in boundaries.windows(2) {
                        let sub_inner = collect_inner_swings_between(all_swings, pair[0], pair[1]);
                        sub_counts.push(sub_inner.len());
                    }
                    level2_counts[w] = sub_counts;
                }
                validate_subwave_deep(&mut swv, &level2_counts);

                // Truncated W5: iç yapısının 5-dalgalı olduğunu doğrula (≥4 iç swing)
                if result.truncation == Some(true) {
                    swv.truncated_w5_inner_ok = Some(sw_counts[4] >= 4);
                }

                if !swv.valid {
                    let existing = result.validation_msg.take().unwrap_or_default();
                    result.validation_msg = Some(format!(
                        "{}; Alt-dalga: {}/5 uyumlu [{}]",
                        existing, swv.conforming_count,
                        sw_counts.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")
                    ));
                }
                if swv.deep_valid == Some(false) {
                    let existing = result.validation_msg.take().unwrap_or_default();
                    result.validation_msg = Some(format!(
                        "{}; L2 deep: {}/{} uyumlu",
                        existing,
                        swv.deep_conforming.unwrap_or(0),
                        swv.deep_total_checked.unwrap_or(0),
                    ));
                }
                if swv.truncated_w5_inner_ok == Some(false) {
                    let existing = result.validation_msg.take().unwrap_or_default();
                    result.validation_msg = Some(format!(
                        "{}; Truncated W5 iç yapı geçersiz ({} swing, beklenen ≥4)",
                        existing, sw_counts[4],
                    ));
                }
                result.subwave_validation = Some(swv);
            }

            // PDF p.16: Nested extension — extended W3 içinde de extension olabilir
            if result.extended_wave.map_or(false, |(w, _)| w == 3) {
                let w3_inner = collect_inner_swings_between(all_swings, recent[2].0, recent[3].0);
                let (nested, ratio) = detect_nested_extension(&w3_inner);
                result.nested_extension = Some((nested, ratio));
                if nested {
                    let existing = result.validation_msg.take().unwrap_or_default();
                    result.validation_msg = Some(format!(
                        "{}; W3 nested ext ({:.2}x)", existing, ratio
                    ));
                }
            }

            let low = [p0, p1, p2].into_iter().fold(f64::INFINITY, f64::min);
            let high = [p0, p1, p2].into_iter().fold(f64::NEG_INFINITY, f64::max);
            let range = high - low;
            if range > 0.0 {
                let last_time = candles.last().map(|c| c.time / 1000).unwrap_or(t4s);
                for (ratio, label) in [
                    (0.146, "14.6%"),
                    (0.236, "23.6%"),
                    (0.382, "38.2%"),
                    (0.5, "50%"),
                    (0.618, "61.8%"),
                ] {
                    result.fibo_levels.push(FiboLevelCore {
                        time1: t0s,
                        time2: last_time,
                        price: low + range * ratio,
                        label: label.to_string(),
                    });
                }
            }
        }
    }

    result
}

fn check_zigzag(last4: &[(i64, f64, bool)]) -> (bool, Vec<f64>, bool) {
    if last4.len() != 4 {
        return (false, vec![], false);
    }
    let (p0, p1, p2, p3) = (last4[0].1, last4[1].1, last4[2].1, last4[3].1);
    let (h0, h1, h2, h3) = (last4[0].2, last4[1].2, last4[2].2, last4[3].2);
    if h0 && !h1 && h2 && !h3 {
        let (valid, c_targets) = validate_zigzag_abc(p0, p1, p2, p3, true);
        (valid, c_targets, true)
    } else if !h0 && h1 && !h2 && h3 {
        let (valid, c_targets) = validate_zigzag_abc(p0, p1, p2, p3, false);
        (valid, c_targets, false)
    } else {
        (false, vec![], false)
    }
}

/// Flat geçerli mi ve hangi tip (Regular / Expanded / Running)?
fn check_flat(last4: &[(i64, f64, bool)]) -> Option<FlatType> {
    if last4.len() != 4 {
        return None;
    }
    let (p0, p1, p2, p3) = (last4[0].1, last4[1].1, last4[2].1, last4[3].1);
    let (h0, h1, h2, h3) = (last4[0].2, last4[1].2, last4[2].2, last4[3].2);
    let (valid, typ) = if h0 && !h1 && h2 && !h3 {
        flat_valid_detailed(p0, p1, p2, p3, true)
    } else if !h0 && h1 && !h2 && h3 {
        flat_valid_detailed(p0, p1, p2, p3, false)
    } else {
        return None;
    };
    if valid {
        typ
    } else {
        None
    }
}

fn build_zigzag_result(last4: &[(i64, f64, bool)], all_swings: &[(i64, f64, bool)]) -> ElliottDetectorResult {
    let (t0, p0, _) = last4[0];
    let (t1, p1, _) = last4[1];
    let (t2, p2, _) = last4[2];
    let (t3, p3, _) = last4[3];
    let t0s = t0 / 1000;
    let t1s = t1 / 1000;
    let t2s = t2 / 1000;
    let t3s = t3 / 1000;

    let is_bearish_zz = last4[0].2;
    let setup = compute_setup_zigzag_c(p0, p1, p2, is_bearish_zz);

    // PDF: Zigzag iç yapı doğrulaması — A=5dalga, B=3dalga, C=5dalga
    let a_inner = collect_inner_swings_between(all_swings, t0, t1).len();
    let b_inner = collect_inner_swings_between(all_swings, t1, t2).len();
    let c_inner = collect_inner_swings_between(all_swings, t2, t3).len();
    let csv = validate_corrective_subwaves([a_inner, b_inner, c_inner], true);
    let msg = if csv.valid {
        format!("Zigzag kuralları geçerli (iç: {}-{}-{})", a_inner, b_inner, c_inner)
    } else {
        format!("Zigzag dış yapı geçerli (iç yapı kısmen: {}-{}-{})", a_inner, b_inner, c_inner)
    };

    ElliottDetectorResult {
        wave_points: vec![pt(t0s, p0, "A"), pt(t1s, p1, "A'"), pt(t2s, p2, "B"), pt(t3s, p3, "C")],
        wave_legs: vec![
            leg(t0s, p0, t1s, p1, "A", false),
            leg(t1s, p1, t2s, p2, "B", false),
            leg(t2s, p2, t3s, p3, "C", false),
        ],
        formation: "Zigzag".to_string(),
        formation_type: "Düzeltme (Zigzag)".to_string(),
        validation_ok: Some(true),
        validation_msg: Some(msg),
        corr_setup: Some(setup),
        corr_subwave_validation: Some(csv),
        ..Default::default()
    }
}

fn build_flat_result(last4: &[(i64, f64, bool)], flat_type: FlatType, all_swings: &[(i64, f64, bool)]) -> ElliottDetectorResult {
    let (t0, p0, _) = last4[0];
    let (t1, p1, _) = last4[1];
    let (t2, p2, _) = last4[2];
    let (t3, p3, _) = last4[3];
    let t0s = t0 / 1000;
    let t1s = t1 / 1000;
    let t2s = t2 / 1000;
    let t3s = t3 / 1000;

    let (formation_name, type_label) = match flat_type {
        FlatType::Regular => ("Flat (Regular)", "Düzeltme (Flat – Regular)"),
        FlatType::Expanded => ("Flat (Expanded)", "Düzeltme (Flat – Expanded/Irregular)"),
        FlatType::Running => ("Flat (Running)", "Düzeltme (Flat – Running)"),
    };

    // PDF p.38: Expanded flat → C = 1.618 × A; Regular flat → C ≈ A
    let a_len = (p1 - p0).abs();
    let a_down = last4[0].2;
    let projections = match flat_type {
        FlatType::Expanded => {
            let c_target_1618 = if a_down { p2 - a_len * 1.618 } else { p2 + a_len * 1.618 };
            let c_target_618_beyond = if a_down {
                p1 - a_len * 0.618
            } else {
                p1 + a_len * 0.618
            };
            Some(vec![
                ElliottProjectionCore { price: c_target_1618, label: "C 161.8%×A".to_string() },
                ElliottProjectionCore { price: c_target_618_beyond, label: "C A+61.8%".to_string() },
            ])
        }
        FlatType::Regular => {
            let c_target = if a_down { p2 - a_len } else { p2 + a_len };
            Some(vec![
                ElliottProjectionCore { price: c_target, label: "C ≈ A".to_string() },
            ])
        }
        FlatType::Running => None,
    };

    // PDF: Flat iç yapı doğrulaması — A=3dalga, B=3dalga, C=5dalga
    let a_inner = collect_inner_swings_between(all_swings, t0, t1).len();
    let b_inner = collect_inner_swings_between(all_swings, t1, t2).len();
    let c_inner = collect_inner_swings_between(all_swings, t2, t3).len();
    let csv = validate_corrective_subwaves([a_inner, b_inner, c_inner], false);
    let msg = if csv.valid {
        format!("Flat kuralları geçerli ({}, iç: {}-{}-{})", formation_name, a_inner, b_inner, c_inner)
    } else {
        format!("Flat dış yapı geçerli ({}, iç kısmen: {}-{}-{})", formation_name, a_inner, b_inner, c_inner)
    };

    ElliottDetectorResult {
        wave_points: vec![pt(t0s, p0, "0"), pt(t1s, p1, "A"), pt(t2s, p2, "B"), pt(t3s, p3, "C")],
        wave_legs: vec![
            leg(t0s, p0, t1s, p1, "A", false),
            leg(t1s, p1, t2s, p2, "B", false),
            leg(t2s, p2, t3s, p3, "C", false),
        ],
        formation: formation_name.to_string(),
        formation_type: type_label.to_string(),
        validation_ok: Some(true),
        validation_msg: Some(msg),
        projections,
        corr_subwave_validation: Some(csv),
        ..Default::default()
    }
}

/// Bir zaman aralığında (t_start..t_end) tüm swing'lerden alt‑swing'leri filtrele.
/// Triangle iç abc sayımı için: her leg aralığında en az 3 nokta (a,b,c) olmalı.
fn collect_inner_swings_between(
    all_swings: &[(i64, f64, bool)],
    t_start: i64,
    t_end: i64,
) -> Vec<(i64, f64, bool)> {
    all_swings
        .iter()
        .filter(|(t, _, _)| *t > t_start && *t < t_end)
        .cloned()
        .collect()
}

/// Triangle her bacağının (ABCDE) iç yapısının 3‑dalgalı (abc) olup olmadığını kontrol eder.
/// En az 3/5 leg'de iç abc yapısı varsa geçerli sayılır (toleranslı).
fn validate_triangle_inner_abc(
    last6: &[(i64, f64, bool)],
    all_swings: &[(i64, f64, bool)],
) -> bool {
    let mut abc_count = 0u32;
    for i in 0..5 {
        let t_start = last6[i].0;
        let t_end = last6[i + 1].0;
        let inner = collect_inner_swings_between(all_swings, t_start, t_end);
        if inner.len() >= 2 {
            abc_count += 1;
        }
    }
    abc_count >= 3
}

fn try_triangle(swings: &[(i64, f64, bool)]) -> Option<ElliottDetectorResult> {
    if swings.len() < 6 {
        return None;
    }
    let last6 = &swings[swings.len() - 6..];
    let (p0, p1, p2, p3, p4, p5) = (
        last6[0].1, last6[1].1, last6[2].1, last6[3].1, last6[4].1, last6[5].1,
    );
    let h0 = last6[0].2;

    // Running triangle: B dalgası A başlangıcını aşar → validasyonu gevşet
    let b_exceeds_a_start = if h0 { p2 > p0 } else { p2 < p0 };

    let ok = (h0 && !last6[1].2 && last6[2].2 && !last6[3].2 && last6[4].2 && !last6[5].2
        && (validate_triangle_abcde(p0, p1, p2, p3, p4, p5, true) || b_exceeds_a_start))
        || (!h0 && last6[1].2 && !last6[2].2 && last6[3].2 && !last6[4].2 && last6[5].2
            && (validate_triangle_abcde(p0, p1, p2, p3, p4, p5, false) || b_exceeds_a_start));

    if !ok {
        return None;
    }

    // Running triangle ek validasyonu: daralan yapı hâlâ gerekli (B sonrası)
    if b_exceeds_a_start {
        let lens = [
            (p1 - p0).abs(), (p2 - p1).abs(), (p3 - p2).abs(),
            (p4 - p3).abs(), (p5 - p4).abs(),
        ];
        let any_shrink = (2..5).any(|i| lens[i - 1] > 1e-10 && lens[i] / lens[i - 1] < 0.95);
        if !any_shrink { return None; }
    }

    let has_inner_abc = validate_triangle_inner_abc(last6, swings);

    let labels = ["0", "A", "B", "C", "D", "E"];
    let leg_labels = ["A", "B", "C", "D", "E"];
    let mut wave_points = vec![];
    let mut wave_legs = vec![];

    for (i, (t, p, _)) in last6.iter().enumerate() {
        wave_points.push(pt(t / 1000, *p, labels.get(i).copied().unwrap_or("?")));
    }
    for i in 0..5 {
        let (t1, p1, _) = last6[i];
        let (t2, p2, _) = last6[i + 1];
        wave_legs.push(leg(
            t1 / 1000, p1, t2 / 1000, p2,
            leg_labels.get(i).copied().unwrap_or("?"), false,
        ));
    }

    let highs = if h0 { [p0, p2, p4] } else { [p1, p3, p5] };
    let lows = if h0 { [p1, p3, p5] } else { [p0, p2, p4] };
    let is_contracting = highs[0] > highs[1] && highs[1] > highs[2]
        && lows[0] < lows[1] && lows[1] < lows[2];
    let is_expanding = highs[0] < highs[1] && highs[1] < highs[2]
        && lows[0] > lows[1] && lows[1] > lows[2];

    let subtype = crate::elliott::classify_triangle_subtype(highs, lows, b_exceeds_a_start);
    let subtype_str = match subtype {
        crate::elliott::TriangleSubtype::Ascending => "Ascending",
        crate::elliott::TriangleSubtype::Descending => "Descending",
        crate::elliott::TriangleSubtype::Symmetrical => "Symmetrical",
        crate::elliott::TriangleSubtype::Running => "Running",
    };

    let shape_label = if b_exceeds_a_start {
        "Running"
    } else if is_contracting {
        "Contracting"
    } else if is_expanding {
        "Expanding"
    } else {
        ""
    };

    let msg = if has_inner_abc {
        format!("Triangle kuralları geçerli ({} {}, iç abc doğrulandı)", shape_label, subtype_str)
    } else {
        format!("Triangle dış yapı geçerli ({} {}; iç abc yeterli değil)", shape_label, subtype_str)
    };

    let a_len = (p1 - p0).abs();
    let is_bull_breakout = !h0;
    let thrust = crate::elliott::triangle_thrust_target(a_len, p5, is_bull_breakout);
    let projections = vec![ElliottProjectionCore {
        price: thrust,
        label: format!("Thrust: {:.2}", thrust),
    }];

    let tri_setup = compute_setup_triangle_e(a_len, p5, p4, is_bull_breakout);

    let formation_name = match (is_contracting || b_exceeds_a_start, is_expanding) {
        (true, _) => format!("{} Triangle ({})", shape_label, subtype_str),
        (_, true) => format!("Expanding Triangle ({})", subtype_str),
        _ => format!("Triangle ({})", subtype_str),
    };

    Some(ElliottDetectorResult {
        wave_points,
        wave_legs,
        formation: formation_name,
        formation_type: format!("Düzeltme (Üçgen – {})", subtype_str),
        validation_ok: Some(true),
        validation_msg: Some(msg),
        projections: Some(projections),
        corr_setup: Some(tri_setup),
        ..Default::default()
    })
}

/// Double Zigzag (W-X-Y) tespiti: iki zigzag + ara X dalgası = 7 swing noktası
/// PDF: "Two zigzags connected by an intervening corrective wave labeled X"
fn try_double_zigzag(swings: &[(i64, f64, bool)]) -> Option<ElliottDetectorResult> {
    if swings.len() < 8 {
        return None;
    }
    let s = &swings[swings.len() - 8..];
    let (p0, _p1, _p2, p3, p4, _p5, _p6, _p7) = (
        s[0].1, s[1].1, s[2].1, s[3].1, s[4].1, s[5].1, s[6].1, s[7].1,
    );
    let _h0 = s[0].2;
    let (zz1_ok, _, _) = {
        let sub = &s[0..4];
        let (pp0, pp1, pp2, pp3) = (sub[0].1, sub[1].1, sub[2].1, sub[3].1);
        let (hh0, hh1, hh2, hh3) = (sub[0].2, sub[1].2, sub[2].2, sub[3].2);
        if hh0 && !hh1 && hh2 && !hh3 {
            let (v, ct) = validate_zigzag_abc(pp0, pp1, pp2, pp3, true);
            (v, ct, true)
        } else if !hh0 && hh1 && !hh2 && hh3 {
            let (v, ct) = validate_zigzag_abc(pp0, pp1, pp2, pp3, false);
            (v, ct, false)
        } else {
            (false, vec![], false)
        }
    };
    let (zz2_ok, _, _) = {
        let sub = &s[4..8];
        let (pp0, pp1, pp2, pp3) = (sub[0].1, sub[1].1, sub[2].1, sub[3].1);
        let (hh0, hh1, hh2, hh3) = (sub[0].2, sub[1].2, sub[2].2, sub[3].2);
        if hh0 && !hh1 && hh2 && !hh3 {
            let (v, ct) = validate_zigzag_abc(pp0, pp1, pp2, pp3, true);
            (v, ct, true)
        } else if !hh0 && hh1 && !hh2 && hh3 {
            let (v, ct) = validate_zigzag_abc(pp0, pp1, pp2, pp3, false);
            (v, ct, false)
        } else {
            (false, vec![], false)
        }
    };

    if !zz1_ok || !zz2_ok {
        return None;
    }

    // X dalgası: p3 ile p4 arasında kısa geri çekilme (zigzag1'in %38-78 arası)
    let zz1_len = (p3 - p0).abs();
    let x_retrace = (p4 - p3).abs() / zz1_len.max(1e-10);
    if x_retrace < 0.20 || x_retrace > 0.85 {
        return None;
    }

    let labels = ["W-a", "W-b", "W-c", "X", "Y-a", "Y-b", "Y-c", "Y-end"];
    let mut wave_points = Vec::new();
    let mut wave_legs = Vec::new();
    for (i, (t, p, _)) in s.iter().enumerate() {
        wave_points.push(pt(t / 1000, *p, labels[i]));
    }
    for i in 0..7 {
        wave_legs.push(leg(
            s[i].0 / 1000, s[i].1, s[i + 1].0 / 1000, s[i + 1].1,
            labels[i + 1], false,
        ));
    }

    Some(ElliottDetectorResult {
        wave_points,
        wave_legs,
        formation: "Double Zigzag".to_string(),
        formation_type: "Düzeltme (WXY)".to_string(),
        validation_ok: Some(true),
        validation_msg: Some("Double Zigzag kuralları geçerli".to_string()),
        ..Default::default()
    })
}

/// Segment tipi: zigzag, flat veya triangle
fn classify_segment(sl: &[(i64, f64, bool)], all_swings: &[(i64, f64, bool)]) -> Option<&'static str> {
    if sl.len() == 4 {
        if is_valid_zigzag_slice(sl) { return Some("ZZ"); }
        if check_flat(sl).is_some() { return Some("FL"); }
    }
    if sl.len() >= 6 {
        let (p0, p1, p2, p3, p4, p5) = (sl[0].1, sl[1].1, sl[2].1, sl[3].1, sl[4].1, sl[5].1);
        let h0 = sl[0].2;
        let ok = (h0 && !sl[1].2 && sl[2].2 && !sl[3].2 && sl[4].2 && !sl[5].2
            && validate_triangle_abcde(p0, p1, p2, p3, p4, p5, true))
            || (!h0 && sl[1].2 && !sl[2].2 && sl[3].2 && !sl[4].2 && sl[5].2
                && validate_triangle_abcde(p0, p1, p2, p3, p4, p5, false));
        if ok { return Some("TRI"); }
    }
    // 4 swing slice'da kontrol
    if sl.len() >= 4 {
        let sub = &sl[sl.len()-4..];
        if is_valid_zigzag_slice(sub) { return Some("ZZ"); }
        if check_flat(sub).is_some() { return Some("FL"); }
    }
    let _ = all_swings;
    None
}

/// Double Three (W-X-Y) tespiti: iki farklı düzeltme yapısı X ile bağlanmış
/// PDF: "combination of simpler types of corrections, including zigzags, flats, and triangles"
/// PDF kuralları:
///   - Bir kombinasyonda en fazla 1 zigzag olabilir
///   - En fazla 1 triangle olabilir, ve yalnızca son dalga (Y) olarak
fn try_double_three(swings: &[(i64, f64, bool)]) -> Option<ElliottDetectorResult> {
    if swings.len() < 8 {
        return None;
    }
    let s = &swings[swings.len() - 8..];

    let w_last4 = &s[0..4];
    let y_last4 = &s[4..8];

    let w_label = classify_segment(w_last4, swings)?;
    let y_label = classify_segment(y_last4, swings)?;

    // PDF: Triangle yalnızca son dalga (Y) olabilir, W olarak olamaz
    if w_label == "TRI" {
        return None;
    }

    // PDF: Bir kombinasyonda en fazla 1 zigzag
    if w_label == "ZZ" && y_label == "ZZ" {
        return None;
    }

    let w_len = (s[3].1 - s[0].1).abs();
    let x_retrace = (s[4].1 - s[3].1).abs() / w_len.max(1e-10);
    if x_retrace < 0.20 || x_retrace > 0.85 {
        return None;
    }

    let labels = ["W-a", "W-b", "W-c", "X", "Y-a", "Y-b", "Y-c", "Y-end"];
    let mut wave_points = Vec::new();
    let mut wave_legs = Vec::new();
    for (i, (t, p, _)) in s.iter().enumerate() {
        wave_points.push(pt(t / 1000, *p, labels[i]));
    }
    for i in 0..7 {
        wave_legs.push(leg(
            s[i].0 / 1000, s[i].1, s[i + 1].0 / 1000, s[i + 1].1,
            labels[i + 1], false,
        ));
    }

    Some(ElliottDetectorResult {
        wave_points,
        wave_legs,
        formation: "Double Three".to_string(),
        formation_type: format!("Düzeltme (WXY: {}-X-{})", w_label, y_label),
        validation_ok: Some(true),
        validation_msg: Some("Double Three kuralları geçerli".to_string()),
        ..Default::default()
    })
}

/// Zigzag validasyonu helper – 4 swing'lik dilim üzerinde
fn is_valid_zigzag_slice(s: &[(i64, f64, bool)]) -> bool {
    if s.len() != 4 { return false; }
    let (v, _, _) = check_zigzag(s);
    v
}

/// X dalgası geri çekilme kontrolü
fn is_valid_x_wave(prev_end: f64, x_end: f64, prev_start: f64) -> bool {
    let seg_len = (prev_end - prev_start).abs();
    if seg_len < 1e-10 { return false; }
    let x_retrace = (x_end - prev_end).abs() / seg_len;
    x_retrace >= 0.20 && x_retrace <= 0.85
}

/// Triple Zigzag (W-X-Y-X-Z) tespiti: 12 swing noktası
fn try_triple_zigzag(swings: &[(i64, f64, bool)]) -> Option<ElliottDetectorResult> {
    if swings.len() < 12 {
        return None;
    }
    let s = &swings[swings.len() - 12..];

    let zz1_ok = is_valid_zigzag_slice(&s[0..4]);
    let zz2_ok = is_valid_zigzag_slice(&s[4..8]);
    let zz3_ok = is_valid_zigzag_slice(&s[8..12]);

    if !zz1_ok || !zz2_ok || !zz3_ok {
        return None;
    }

    let x1_ok = is_valid_x_wave(s[3].1, s[4].1, s[0].1);
    let x2_ok = is_valid_x_wave(s[7].1, s[8].1, s[4].1);
    if !x1_ok || !x2_ok {
        return None;
    }

    let labels = ["W-a","W-b","W-c","X1","Y-a","Y-b","Y-c","X2","Z-a","Z-b","Z-c","Z-end"];
    let mut wave_points = Vec::new();
    let mut wave_legs = Vec::new();
    for (i, (t, p, _)) in s.iter().enumerate() {
        wave_points.push(pt(t / 1000, *p, labels[i]));
    }
    for i in 0..11 {
        wave_legs.push(leg(s[i].0/1000, s[i].1, s[i+1].0/1000, s[i+1].1, labels[i+1], false));
    }

    Some(ElliottDetectorResult {
        wave_points,
        wave_legs,
        formation: "Triple Zigzag".to_string(),
        formation_type: "Düzeltme (WXYXZ)".to_string(),
        validation_ok: Some(true),
        validation_msg: Some("Triple Zigzag kuralları geçerli".to_string()),
        ..Default::default()
    })
}

/// Triple Three (W-X-Y-X-Z) tespiti: 12 swing, her bölge zigzag/flat/triangle
/// PDF kuralları:
///   - Bir kombinasyonda en fazla 1 zigzag
///   - En fazla 1 triangle, yalnızca son dalga (Z) olarak
fn try_triple_three(swings: &[(i64, f64, bool)]) -> Option<ElliottDetectorResult> {
    if swings.len() < 12 {
        return None;
    }
    let s = &swings[swings.len() - 12..];

    let w = classify_segment(&s[0..4], swings)?;
    let y = classify_segment(&s[4..8], swings)?;
    let z = classify_segment(&s[8..12], swings)?;

    // PDF: Triangle yalnızca son dalga (Z) olabilir
    if w == "TRI" || y == "TRI" {
        return None;
    }

    // PDF: En fazla 1 zigzag
    let zz_count = [w, y, z].iter().filter(|&&t| t == "ZZ").count();
    if zz_count > 1 {
        return None;
    }

    if !is_valid_x_wave(s[3].1, s[4].1, s[0].1) { return None; }
    if !is_valid_x_wave(s[7].1, s[8].1, s[4].1) { return None; }

    let labels = ["W-a","W-b","W-c","X1","Y-a","Y-b","Y-c","X2","Z-a","Z-b","Z-c","Z-end"];
    let mut wave_points = Vec::new();
    let mut wave_legs = Vec::new();
    for (i, (t, p, _)) in s.iter().enumerate() {
        wave_points.push(pt(t / 1000, *p, labels[i]));
    }
    for i in 0..11 {
        wave_legs.push(leg(s[i].0/1000, s[i].1, s[i+1].0/1000, s[i+1].1, labels[i+1], false));
    }

    Some(ElliottDetectorResult {
        wave_points,
        wave_legs,
        formation: "Triple Three".to_string(),
        formation_type: format!("Düzeltme (WXYXZ: {}-{}-{})", w, y, z),
        validation_ok: Some(true),
        validation_msg: Some("Triple Three kuralları geçerli".to_string()),
        ..Default::default()
    })
}

fn compute_projections(
    _candles: &[Candle],
    recent: &[(i64, f64, bool)],
    last4: &[(i64, f64, bool)],
    formation: &str,
    formation_type: &str,
    is_bullish: bool,
    imp: &crate::impulse_detector::ImpulseDetectorState,
) -> Option<Vec<ElliottProjectionCore>> {
    let mut proj = Vec::new();

    if formation == "Impulse (1-2)" && recent.len() == 3 {
        let (_, p0, _) = recent[0];
        let (_, p1, _) = recent[1];
        let (_, p2, _) = recent[2];
        let w1_len = (p1 - p0).abs();
        for (ext, lbl) in [(1.382, "W3 138.2%"), (1.618, "W3 161.8%"), (2.618, "W3 261.8%")] {
            let price = if imp.is_bullish {
                p2 + w1_len * ext
            } else {
                p2 - w1_len * ext
            };
            proj.push(ElliottProjectionCore {
                price,
                label: lbl.to_string(),
            });
        }
    } else if (formation == "Impulse" || formation_type == "Motif (İtki)") && recent.len() == 4 {
        let (_, p0, _) = recent[0];
        let (_, p1, _) = recent[1];
        let (_, p4, _) = recent[3];
        let w1_len = (p1 - p0).abs();
        let w1_3 = (recent[2].1 - p0).abs();
        let w4_len = (recent[2].1 - p4).abs();
        proj.push(ElliottProjectionCore {
            price: if is_bullish { p4 + w1_len } else { p4 - w1_len },
            label: "W5=W1".to_string(),
        });
        proj.push(ElliottProjectionCore {
            price: if is_bullish { p4 + 0.618 * w1_3 } else { p4 - 0.618 * w1_3 },
            label: "W5 61.8%".to_string(),
        });
        proj.push(ElliottProjectionCore {
            price: if is_bullish {
                p4 + 1.236 * w4_len
            } else {
                p4 - 1.236 * w4_len
            },
            label: "W5 inv123.6%".to_string(),
        });
    } else if last4.len() == 4 {
        let (_, p0, _) = last4[0];
        let (_, p1, _) = last4[1];
        let (_, p2, _) = last4[2];
        let a_len = (p1 - p0).abs();
        if a_len > 1e-12 {
            let a_down = last4[0].2;
            for (ext, lbl) in [
                (1.0, "C 100%"),
                (1.236, "C 123.6%"),
                (1.382, "C 138.2%"),
                (1.618, "C 161.8%"),
            ] {
                let price = if a_down {
                    p2 - a_len * ext
                } else {
                    p2 + a_len * ext
                };
                proj.push(ElliottProjectionCore {
                    price,
                    label: lbl.to_string(),
                });
            }
        }
    }

    if proj.is_empty() {
        None
    } else {
        Some(proj)
    }
}
