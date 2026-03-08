//! Elliott Wave formasyonları, kurallar, dalga bacak hesapları, giriş/çıkış/iptal seviyeleri.
//!
//! Bkz: docs/ELLIOTT_WAVE_SPEC.md

use serde::{Deserialize, Serialize};

/// Elliott Wave formasyon türü (EWM Cheat Sheet + Studocu interchange uyumlu)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElliottFormation {
    /// Impulse (5-3-5-3-5)
    Impulse,
    /// Leading Diagonal (Wave 1 veya A'da)
    LeadingDiagonal,
    /// Ending Diagonal (Wave 5 veya C'de)
    EndingDiagonal,
    /// Contracting Diagonal (daralan wedge)
    ContractingDiagonal,
    /// Expanding Diagonal (genişleyen wedge)
    ExpandingDiagonal,
    /// Zigzag (5-3-5)
    Zigzag,
    /// Double Zigzag (ZZ-X-ZZ)
    DoubleZigzag,
    /// Triple Zigzag (ZZ-X-ZZ-X-ZZ)
    TripleZigzag,
    /// Flat (3-3-5)
    Flat,
    /// Double Three (W-X-Y)
    DoubleThree,
    /// Triple Three (W-X-Y-X-Z)
    TripleThree,
    /// Contracting Triangle (daralan)
    ContractingTriangle,
    /// Expanding Triangle (genişleyen)
    ExpandingTriangle,
    /// Triangle – genel (CT/ET için geriye uyumluluk)
    Triangle,
}

/// Formasyon tamamlandıktan sonra gelebilecek formasyon (Studocu interchange)
impl ElliottFormation {
    pub fn next_formation_after_completion(self, in_trend_direction: bool) -> Vec<ElliottFormation> {
        match self {
            // Motive: Impulse, LD, ED, Contracting/Expanding Diagonal tamamlanınca → düzeltme
            ElliottFormation::Impulse
            | ElliottFormation::LeadingDiagonal
            | ElliottFormation::EndingDiagonal
            | ElliottFormation::ContractingDiagonal
            | ElliottFormation::ExpandingDiagonal => {
                if in_trend_direction {
                    vec![]
                } else {
                    vec![
                        ElliottFormation::Zigzag,
                        ElliottFormation::Flat,
                        ElliottFormation::DoubleZigzag,
                        ElliottFormation::TripleZigzag,
                        ElliottFormation::DoubleThree,
                        ElliottFormation::TripleThree,
                        ElliottFormation::ContractingTriangle,
                        ElliottFormation::ExpandingTriangle,
                    ]
                }
            }
            // Düzeltmeler (ZZ, DZ, TZ, FL, D3, T3, CT, ET) tamamlanınca → motive veya başka düzeltme
            ElliottFormation::Zigzag
            | ElliottFormation::DoubleZigzag
            | ElliottFormation::TripleZigzag
            | ElliottFormation::Flat
            | ElliottFormation::DoubleThree
            | ElliottFormation::TripleThree
            | ElliottFormation::ContractingTriangle
            | ElliottFormation::ExpandingTriangle
            | ElliottFormation::Triangle => {
                vec![
                    ElliottFormation::Impulse,
                    ElliottFormation::LeadingDiagonal,
                    ElliottFormation::Zigzag,
                    ElliottFormation::Flat,
                    ElliottFormation::DoubleZigzag,
                    ElliottFormation::TripleZigzag,
                    ElliottFormation::DoubleThree,
                    ElliottFormation::TripleThree,
                    ElliottFormation::ContractingTriangle,
                    ElliottFormation::ExpandingTriangle,
                ]
            }
        }
    }

    /// W2 konumunda kullanılabilir mi? (EWM: Triangle W2'de olamaz)
    pub fn is_valid_for_w2(self) -> bool {
        matches!(
            self,
            ElliottFormation::Zigzag
                | ElliottFormation::Flat
                | ElliottFormation::DoubleThree
                | ElliottFormation::TripleThree
                | ElliottFormation::DoubleZigzag
                | ElliottFormation::TripleZigzag
        )
    }

    /// W4 veya B konumunda kullanılabilir mi? (EWM: Triangle W4 veya B'de olur)
    pub fn is_valid_for_w4_or_b(self) -> bool {
        matches!(
            self,
            ElliottFormation::Zigzag
                | ElliottFormation::Flat
                | ElliottFormation::ContractingTriangle
                | ElliottFormation::ExpandingTriangle
                | ElliottFormation::Triangle
                | ElliottFormation::DoubleThree
                | ElliottFormation::TripleThree
                | ElliottFormation::DoubleZigzag
                | ElliottFormation::TripleZigzag
        )
    }

    /// W2 veya W4/B için geçerli (geriye uyumluluk)
    pub fn is_valid_for_w2_w4(self) -> bool {
        self.is_valid_for_w2() || self.is_valid_for_w4_or_b()
    }

    /// Diagonal ailesi (LD, ED, Contracting, Expanding)
    pub fn is_diagonal_family(self) -> bool {
        matches!(
            self,
            ElliottFormation::LeadingDiagonal
                | ElliottFormation::EndingDiagonal
                | ElliottFormation::ContractingDiagonal
                | ElliottFormation::ExpandingDiagonal
        )
    }

    /// Zigzag ailesi (ZZ, DZ, TZ)
    pub fn is_zigzag_family(self) -> bool {
        matches!(
            self,
            ElliottFormation::Zigzag
                | ElliottFormation::DoubleZigzag
                | ElliottFormation::TripleZigzag
        )
    }
}

/// Dalga bacak (swing) – yüksek/dip noktası
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WaveLeg {
    pub start_time: i64,
    pub start_price: f64,
    pub end_time: i64,
    pub end_price: f64,
    pub is_impulse: bool, // trend yönünde mi
}

impl WaveLeg {
    pub fn length(&self) -> f64 {
        (self.end_price - self.start_price).abs()
    }

    /// Lineer retracement: W2, W4, Zigzag B hedefleri
    pub fn retrace(&self, ratio: f64) -> f64 {
        if self.is_impulse {
            self.start_price + (self.end_price - self.start_price) * (1.0 - ratio)
        } else {
            self.start_price - (self.start_price - self.end_price) * (1.0 - ratio)
        }
    }

    /// Lineer extension: W3, W5, Zigzag C hedefleri
    pub fn extend(&self, ratio: f64) -> f64 {
        if self.is_impulse {
            self.start_price + (self.end_price - self.start_price) * ratio
        } else {
            self.start_price - (self.start_price - self.end_price) * ratio
        }
    }
}

/// Yarı-logaritmik (semi-log) golden ratio projeksiyonu.
/// Geniş fiyat aralıklarında lineer yerine yüzdesel değişim daha anlamlıdır.
/// p = ph^ratio * pl^(1-ratio)
#[inline]
pub fn semi_log_level(high: f64, low: f64, ratio: f64) -> f64 {
    if high <= 0.0 || low <= 0.0 {
        return (high + low) / 2.0;
    }
    high.powf(ratio) * low.powf(1.0 - ratio)
}

/// Yarı-log düzeltme seviyeleri: 0.125, 0.236, 0.382, 0.5, 0.618, 0.764, 0.875
pub fn semi_log_retrace_levels(high: f64, low: f64) -> [f64; 7] {
    let ratios = [0.125, 0.236, 0.382, 0.5, 0.618, 0.764, 0.875];
    ratios.map(|r| semi_log_level(high, low, r))
}

/// Fibonacci oranları (EWT spesifikasyonu + Elliott Wave Forecast genişletmesi)
pub mod fibo {
    // Geri çekilme (retracement)
    pub const RETRACE_125: f64 = 0.125;
    pub const RETRACE_146: f64 = 0.146; // W4 min (14.6%)
    pub const RETRACE_236: f64 = 0.236;
    pub const RETRACE_382: f64 = 0.382;
    pub const RETRACE_500: f64 = 0.5;
    pub const RETRACE_618: f64 = 0.618;
    pub const RETRACE_764: f64 = 0.764;
    pub const RETRACE_786: f64 = 0.786;
    pub const RETRACE_854: f64 = 0.854; // W2/B max (85.4%)
    pub const RETRACE_875: f64 = 0.875;
    // Uzantı (extension)
    pub const EXT_100: f64 = 1.0;
    pub const EXT_1236: f64 = 1.236; // W5 inverse retrace min
    pub const EXT_1382: f64 = 1.382;
    pub const EXT_150: f64 = 1.5;
    pub const EXT_1618: f64 = 1.618; // W5 inverse retrace max
    pub const EXT_2618: f64 = 2.618;
    pub const EXT_3236: f64 = 3.236; // W3 (323.6%)
    pub const EXT_4236: f64 = 4.236;
    // W2 hedefleri: 0.5, 0.618, 0.786, 0.854 (EWF)
    pub const W2_RETRACES: [f64; 4] = [0.5, 0.618, 0.786, 0.854];
    // W3 hedefleri: 1.382, 1.618, 2.618, 3.236, 4.236 (EWF)
    pub const W3_EXTENSIONS: [f64; 5] = [1.382, 1.618, 2.618, 3.236, 4.236];
    // W4 hedefleri: 0.146, 0.236, 0.382, 0.5 (14.6% EWF)
    pub const W4_RETRACES: [f64; 4] = [0.146, 0.236, 0.382, 0.5];
    // ABC Zigzag: B = 0.382, 0.5, 0.618, 0.764, 0.854; C = 1.0, 1.236, 1.382, 1.618
    pub const ZIGZAG_B_RETRACES: [f64; 5] = [0.382, 0.5, 0.618, 0.764, 0.854];
    pub const ZIGZAG_C_EXTENSIONS: [f64; 4] = [1.0, 1.236, 1.382, 1.618];
    // Triangle (EWM): her dalga öncekinin ~%61.8 veya %78.6
    pub const TRIANGLE_RETRACES: [f64; 2] = [0.618, 0.786];
}

/// Impulse dalga hesapları
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpulseLevels {
    pub w1_high: f64,
    pub w1_low: f64,
    pub w2_targets: (f64, f64, f64, f64),  // 0.5, 0.618, 0.786, 0.854
    pub w3_targets: (f64, f64, f64, f64),  // 1.382, 1.618, 2.618, 3.236
    pub w4_targets: (f64, f64, f64, f64),  // 0.146, 0.236, 0.382, 0.5
    /// W5: (W1=W5, 0.618×(0-3), W4 inverse 1.236-1.618, W3<W1 ise 0.382×W3)
    pub w5_targets: (f64, f64, f64, f64),
}

impl ImpulseLevels {
    pub fn from_w1(w1: &WaveLeg, w2_low: f64, w3_high: f64, w4_low: f64) -> Self {
        let w1_len = w1.length();
        let w0_w3_len = (w3_high - w1.start_price).abs();
        let w3_len = (w3_high - w2_low).abs();

        let w2_50 = w1.retrace(fibo::RETRACE_500);
        let w2_618 = w1.retrace(fibo::RETRACE_618);
        let w2_786 = w1.retrace(fibo::RETRACE_786);
        let w2_854 = w1.retrace(fibo::RETRACE_854);

        let w3_1382 = if w1.is_impulse {
            w2_low + w1_len * fibo::EXT_1382
        } else {
            w2_low - w1_len * fibo::EXT_1382
        };
        let w3_1618 = if w1.is_impulse {
            w2_low + w1_len * fibo::EXT_1618
        } else {
            w2_low - w1_len * fibo::EXT_1618
        };
        let w3_2618 = if w1.is_impulse {
            w2_low + w1_len * fibo::EXT_2618
        } else {
            w2_low - w1_len * fibo::EXT_2618
        };
        let w3_3236 = if w1.is_impulse {
            w2_low + w1_len * fibo::EXT_3236
        } else {
            w2_low - w1_len * fibo::EXT_3236
        };

        let w4_146 = if w1.is_impulse {
            w3_high - w3_len * fibo::RETRACE_146
        } else {
            w3_high + w3_len * fibo::RETRACE_146
        };
        let w4_236 = if w1.is_impulse {
            w3_high - w3_len * fibo::RETRACE_236
        } else {
            w3_high + w3_len * fibo::RETRACE_236
        };
        let w4_382 = if w1.is_impulse {
            w3_high - w3_len * fibo::RETRACE_382
        } else {
            w3_high + w3_len * fibo::RETRACE_382
        };
        let w4_50 = if w1.is_impulse {
            w3_high - w3_len * fibo::RETRACE_500
        } else {
            w3_high + w3_len * fibo::RETRACE_500
        };

        let w5_eq = if w1.is_impulse {
            w4_low + w1_len
        } else {
            w4_low - w1_len
        };
        let w5_618 = if w1.is_impulse {
            w4_low + w0_w3_len * fibo::RETRACE_618
        } else {
            w4_low - w0_w3_len * fibo::RETRACE_618
        };
        // W3 < W1 ise W5: 0.382 × W3 (Stock-market kuralı)
        let w5_382_w3 = if w3_len < w1_len {
            if w1.is_impulse {
                w4_low + w3_len * fibo::RETRACE_382
            } else {
                w4_low - w3_len * fibo::RETRACE_382
            }
        } else {
            w5_eq
        };
        // W5 inverse retrace: W4'ün 123.6% uzantısı (EWF: inverse 123.6-161.8% of W4)
        let w4_len = (w3_high - w4_low).abs();
        let w5_inv = if w1.is_impulse {
            w4_low + w4_len * fibo::EXT_1236
        } else {
            w4_low - w4_len * fibo::EXT_1236
        };

        ImpulseLevels {
            w1_high: w1.end_price,
            w1_low: w1.start_price,
            w2_targets: (w2_50, w2_618, w2_786, w2_854),
            w3_targets: (w3_1382, w3_1618, w3_2618, w3_3236),
            w4_targets: (w4_146, w4_236, w4_382, w4_50),
            w5_targets: (w5_eq, w5_618, w5_inv, w5_382_w3),
        }
    }
}

/// Impulse geçersizlik (invalidation) kontrolleri
#[derive(Debug, Clone)]
pub struct ImpulseValidation {
    pub w2_valid: bool,
    pub w3_valid: bool,
    pub w4_valid: bool,
    /// N°11: W1, W3, W5 aynı anda extended olamaz (en fazla 1 extended)
    pub no_triple_extension_valid: bool,
    pub formation_valid: bool,
}

/// W0,W1,W2,W3,W4 fiyatları ile impulse kurallarını kontrol et
/// w5_extreme: opsiyonel, varsa N°11 (triple extension) kontrolü yapılır
pub fn validate_impulse(
    w0: f64,
    w1_high: f64,
    w1_low: f64,
    w2_extreme: f64,
    w3_extreme: f64,
    w4_extreme: f64,
    is_bullish: bool,
) -> ImpulseValidation {
    validate_impulse_with_w5(w0, w1_high, w1_low, w2_extreme, w3_extreme, w4_extreme, None, is_bullish)
}

/// W5 dahil tam validasyon (N°11 için)
pub fn validate_impulse_with_w5(
    w0: f64,
    w1_high: f64,
    w1_low: f64,
    w2_extreme: f64,
    w3_extreme: f64,
    w4_extreme: f64,
    w5_extreme: Option<f64>,
    is_bullish: bool,
) -> ImpulseValidation {
    let w2_valid = if is_bullish {
        w2_extreme > w0
    } else {
        w2_extreme < w0
    };
    let w1_len = (w1_high - w1_low).abs();
    let w3_len = (w3_extreme - w2_extreme).abs();
    let w3_valid = w1_len > 0.0 && w3_len >= w1_len * 0.9;
    let w1_extreme = if is_bullish { w1_high } else { w1_low };
    let w4_valid = if is_bullish {
        w4_extreme > w1_extreme
    } else {
        w4_extreme < w1_extreme
    };
    // N°11: W1, W3, W5 asla üçü birden extended olamaz (extended = >= 1.618 × min)
    let no_triple_extension_valid = match w5_extreme {
        Some(w5) => {
            let w5_len = (w5 - w4_extreme).abs();
            let min_len = w1_len.min(w3_len).min(w5_len).max(1e-10);
            let extended_count = [w1_len, w3_len, w5_len]
                .iter()
                .filter(|&&len| len >= min_len * fibo::EXT_1618)
                .count();
            extended_count <= 1
        }
        None => true,
    };
    let formation_valid = w2_valid && w3_valid && w4_valid && no_triple_extension_valid;
    ImpulseValidation {
        w2_valid,
        w3_valid,
        w4_valid,
        no_triple_extension_valid,
        formation_valid,
    }
}

/// Leading/Ending Diagonal validasyonu.
/// Impulse'tan fark: W4, W1 ile örtüşebilir (trend çizgileri arasında wedge).
/// W2 ve W3 kuralları aynı: W2<=W0 iptal, W3 en kısa olamaz.
#[derive(Debug, Clone)]
pub struct DiagonalValidation {
    pub w2_valid: bool,
    pub w3_valid: bool,
    pub formation_valid: bool,
}

pub fn validate_diagonal(
    w0: f64,
    w1_high: f64,
    w1_low: f64,
    w2_extreme: f64,
    w3_extreme: f64,
    _w4_extreme: f64,
    is_bullish: bool,
) -> DiagonalValidation {
    let w2_valid = if is_bullish {
        w2_extreme > w0
    } else {
        w2_extreme < w0
    };
    let w1_len = (w1_high - w1_low).abs();
    let w3_len = (w3_extreme - w2_extreme).abs();
    let w3_valid = w1_len > 0.0 && w3_len >= w1_len * 0.9;
    DiagonalValidation {
        w2_valid,
        w3_valid,
        formation_valid: w2_valid && w3_valid,
    }
}

/// W3 setup – en güvenli ve kârlı işlem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupW3 {
    pub entry: f64,
    pub stop_loss: f64,
    pub tp1: f64,
    pub tp2: f64,
    pub is_long: bool,
}

/// W5 setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupW5 {
    pub entry: f64,
    pub stop_loss: f64,
    pub tp: f64,
    pub tp_alternate: f64,
    pub is_long: bool,
}

/// W3 setup hesapla: W1 bitti, W2 %50–%61.8 geri çekildi
/// w2_extreme: bullish'da W2 dip, bearish'da W2 zirve
pub fn compute_setup_w3(
    w0: f64,
    w1_high: f64,
    w1_low: f64,
    w2_extreme: f64,
    is_bullish: bool,
) -> SetupW3 {
    let w1_len = (w1_high - w1_low).abs();
    if is_bullish {
        SetupW3 {
            entry: w2_extreme,
            stop_loss: w0,
            tp1: w1_high,
            tp2: w1_low + w1_len * fibo::EXT_1618,
            is_long: true,
        }
    } else {
        SetupW3 {
            entry: w2_extreme,
            stop_loss: w0,
            tp1: w1_low,
            tp2: w1_high - w1_len * fibo::EXT_1618,
            is_long: false,
        }
    }
}

/// W5 setup hesapla: W4 %38.2, SL=W1 tepe
pub fn compute_setup_w5(
    w1_high: f64,
    w1_low: f64,
    w3_high: f64,
    w3_low: f64,
    w4_low: f64,
    is_bullish: bool,
) -> SetupW5 {
    let w1_len = (w1_high - w1_low).abs();
    let w0_w3 = if is_bullish {
        w3_high - w1_low
    } else {
        w1_high - w3_low
    };
    if is_bullish {
        let entry = w4_low;
        let tp = w4_low + w1_len;
        let tp_alt = w4_low + w0_w3 * fibo::RETRACE_618;
        SetupW5 {
            entry,
            stop_loss: w1_high,
            tp,
            tp_alternate: tp_alt,
            is_long: true,
        }
    } else {
        let entry = w4_low;
        let tp = w4_low - w1_len;
        let tp_alt = w4_low - w0_w3 * fibo::RETRACE_618;
        SetupW5 {
            entry,
            stop_loss: w1_low,
            tp,
            tp_alternate: tp_alt,
            is_long: false,
        }
    }
}

/// Giriş / çıkış / iptal seviyeleri – tek dalga için
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveTradeLevels {
    pub entry: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub cancel_level: f64,
    pub is_long: bool,
}

/// Impulse için her dalga bacak için trade seviyeleri
pub fn impulse_trade_levels(
    w1: &WaveLeg,
    w2: Option<&WaveLeg>,
    w3: Option<&WaveLeg>,
    w4: Option<&WaveLeg>,
    wave_index: u8,
    is_bullish: bool,
) -> Option<WaveTradeLevels> {
    let cancel_long = w1.start_price;
    let cancel_short = w1.end_price;

    match wave_index {
        2 => {
            // W2 dip girişi
            if is_bullish {
                let entry = w1.retrace(fibo::RETRACE_500);
                let sl = cancel_long;
                let tp = w1.end_price;
                Some(WaveTradeLevels {
                    entry,
                    stop_loss: sl,
                    take_profit: tp,
                    cancel_level: cancel_long,
                    is_long: true,
                })
            } else {
                let entry = w1.retrace(fibo::RETRACE_500);
                let sl = cancel_short;
                let tp = w1.start_price;
                Some(WaveTradeLevels {
                    entry,
                    stop_loss: sl,
                    take_profit: tp,
                    cancel_level: cancel_short,
                    is_long: false,
                })
            }
        }
        3 => {
            let w2_leg = w2?;
            if is_bullish {
                let entry = w2_leg.end_price;
                let sl = w2_leg.start_price.min(w2_leg.end_price);
                let tp = w2_leg.end_price + w1.length() * 1.618;
                Some(WaveTradeLevels {
                    entry,
                    stop_loss: sl,
                    take_profit: tp,
                    cancel_level: cancel_long,
                    is_long: true,
                })
            } else {
                let entry = w2_leg.end_price;
                let sl = w2_leg.start_price.max(w2_leg.end_price);
                let tp = w2_leg.end_price - w1.length() * 1.618;
                Some(WaveTradeLevels {
                    entry,
                    stop_loss: sl,
                    take_profit: tp,
                    cancel_level: cancel_short,
                    is_long: false,
                })
            }
        }
        4 => {
            let w3_leg = w3?;
            let range = (w3_leg.end_price - w3_leg.start_price).abs();
            if is_bullish {
                let entry = w3_leg.end_price - range * 0.382;
                let sl = w3_leg.start_price.min(w3_leg.end_price);
                let tp = w3_leg.end_price;
                Some(WaveTradeLevels {
                    entry,
                    stop_loss: sl,
                    take_profit: tp,
                    cancel_level: w1.end_price,
                    is_long: true,
                })
            } else {
                let entry = w3_leg.end_price + range * 0.382;
                let sl = w3_leg.start_price.max(w3_leg.end_price);
                let tp = w3_leg.end_price;
                Some(WaveTradeLevels {
                    entry,
                    stop_loss: sl,
                    take_profit: tp,
                    cancel_level: w1.start_price,
                    is_long: false,
                })
            }
        }
        5 => {
            let w4_leg = w4?;
            if is_bullish {
                let entry = w4_leg.end_price;
                let sl = w4_leg.start_price.min(w4_leg.end_price);
                let tp = w4_leg.end_price + w1.length();
                Some(WaveTradeLevels {
                    entry,
                    stop_loss: sl,
                    take_profit: tp,
                    cancel_level: w1.end_price,
                    is_long: true,
                })
            } else {
                let entry = w4_leg.end_price;
                let sl = w4_leg.start_price.max(w4_leg.end_price);
                let tp = w4_leg.end_price - w1.length();
                Some(WaveTradeLevels {
                    entry,
                    stop_loss: sl,
                    take_profit: tp,
                    cancel_level: w1.start_price,
                    is_long: false,
                })
            }
        }
        _ => None,
    }
}

/// Zigzag kuralı: B, A'nın başlangıç noktasını aşamaz
/// a_down: A düşüş dalgası ise true (A high→low)
pub fn zigzag_valid(a_start: f64, b_extreme: f64, a_down: bool) -> bool {
    if a_down {
        b_extreme <= a_start
    } else {
        b_extreme >= a_start
    }
}

/// Flat kuralları: B, A'nın %90'ından fazlasını geri alır
/// Flat genel kuralı: B, A'nın %90'ından fazlasını geri alır
pub fn flat_valid(_a_len: f64, b_retrace_ratio: f64) -> bool {
    b_retrace_ratio >= 0.9
}

/// Flat alt tipleri (Elliott Wave Monitor / EWF)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlatType {
    /// Regular: B ≈ %90–100, C ≈ %100
    Regular,
    /// Expanded/Irregular: B > %123.6, C > %123.6–161.8
    Expanded,
    /// Running: B aşar, C tam mesafeyi tamamlamaz
    Running,
}

/// Flat tipi tespit: B'nin A'ya oranına göre
pub fn flat_type(b_retrace_ratio: f64) -> Option<FlatType> {
    if b_retrace_ratio < 0.9 {
        None
    } else if b_retrace_ratio <= 1.05 {
        Some(FlatType::Regular)
    } else if b_retrace_ratio >= 1.236 {
        Some(FlatType::Expanded)
    } else {
        Some(FlatType::Regular)
    }
}

/// Flat validasyonu: Regular B≈%90–100, Expanded B>%123.6, Running (B aştığında C kısa)
pub fn flat_valid_detailed(
    a_start: f64,
    a_end: f64,
    b_extreme: f64,
    c_extreme: f64,
    a_down: bool,
) -> (bool, Option<FlatType>) {
    let a_len = (a_end - a_start).abs();
    if a_len < 1e-10 {
        return (false, None);
    }
    let b_retrace = if a_down {
        (b_extreme - a_end) / a_len
    } else {
        (a_end - b_extreme) / a_len
    };
    let b_ratio = b_retrace.abs();
    let typ = flat_type(b_ratio);
    if typ.is_none() {
        return (false, None);
    }
    let c_retrace = if a_down {
        (a_start - c_extreme).abs() / a_len
    } else {
        (c_extreme - a_start).abs() / a_len
    };
    let valid = match typ.unwrap() {
        FlatType::Regular => b_ratio >= 0.9 && b_ratio <= 1.05 && c_retrace >= 0.6,
        FlatType::Expanded => b_ratio >= 1.236 && c_retrace >= 1.0,
        FlatType::Running => b_ratio >= 1.236 && c_retrace < 1.0 && c_retrace >= 0.5,
    };
    (valid, typ)
}

/// Zaman Fibonacci: W3 süresi W1'in %100, %161.8 veya %261.8'i
pub fn time_projection_w3(w1_bars: u32, ratio: f64) -> u32 {
    ((w1_bars as f64) * ratio) as u32
}

/// Zigzag (ABC) trade seviyeleri
pub fn zigzag_trade_levels(
    a: &WaveLeg,
    b: &WaveLeg,
    c_target: f64,
    is_long_at_c_end: bool,
) -> WaveTradeLevels {
    let cancel = a.start_price;
    let sl = b.end_price;
    let tp = if is_long_at_c_end {
        b.end_price + a.length() * fibo::EXT_1618
    } else {
        b.end_price - a.length() * fibo::EXT_1618
    };
    WaveTradeLevels {
        entry: c_target,
        stop_loss: sl,
        take_profit: tp,
        cancel_level: cancel,
        is_long: is_long_at_c_end,
    }
}

/// Zigzag ABC hedef seviyeleri: B = A'nın 0.382, 0.5, 0.618; C = A'nın 1.0, 1.382, 1.618
pub fn zigzag_targets(
    a: &WaveLeg,
    _is_a_down: bool,
) -> (Vec<f64>, Vec<f64>) {
    let b_targets: Vec<f64> = fibo::ZIGZAG_B_RETRACES
        .iter()
        .map(|&r| a.retrace(r))
        .collect();
    let c_targets: Vec<f64> = fibo::ZIGZAG_C_EXTENSIONS
        .iter()
        .map(|&r| a.extend(r))
        .collect();
    (b_targets, c_targets)
}

/// Triangle hedef seviyeleri (EWM: her dalga öncekinin ~%61.8 veya %78.6)
/// prev: önceki dalga (A, B, C veya D). Dönüş: bir sonraki dalga için retrace hedefleri.
pub fn triangle_targets(prev: &WaveLeg) -> Vec<f64> {
    fibo::TRIANGLE_RETRACES
        .iter()
        .map(|&r| prev.retrace(r))
        .collect()
}
