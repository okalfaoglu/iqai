//! Elliott Wave formasyonları, kurallar, dalga dereceleri, dalga bacak hesapları, giriş/çıkış/iptal seviyeleri.
//!
//! Bkz: docs/ELLIOTT_WAVE_SPEC.md ve THE_BASICS_OF_THE_ELLIOTT_WAVE_PRINCIPLE.pdf

use serde::{Deserialize, Serialize};

/// Wave personality – PDF: "Each wave has a personality"
pub fn wave_personality(wave_label: &str) -> &'static str {
    match wave_label {
        "1" => "Başlangıç: Şüphe fazı, hacim düşük",
        "2" => "Geri çekilme: Korku, kazanç geri verilir",
        "3" => "Güç: En uzun/güçlü, yüksek hacim",
        "4" => "Konsolidasyon: Kâr real., W2'den farklı yapı",
        "5" => "Son hamle: Momentum azalır, hacim düşer",
        "A" => "Düzeltme başı: Alım fırsatı sanılır",
        "B" => "Tuzak rallisi: Son iyimserlik",
        "C" => "Çöküş: Panik, W3 kadar güçlü olabilir",
        _ => "",
    }
}

/// Dalga derecesi – Elliott’un çoklu derece kavramına göre sadeleştirilmiş sınıflar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaveDegree {
    Grand,
    Primary,
    Intermediate,
    Minor,
    Minute,
    Minuette,
    SubMinuette,
}

impl WaveDegree {
    /// İngilizce kısa etiket (JSON / log).
    pub const fn label_en(self) -> &'static str {
        match self {
            Self::Grand => "Grand",
            Self::Primary => "Primary",
            Self::Intermediate => "Intermediate",
            Self::Minor => "Minor",
            Self::Minute => "Minute",
            Self::Minuette => "Minuette",
            Self::SubMinuette => "SubMinuette",
        }
    }

    /// Türkçe açıklamalı etiket (GUI).
    pub const fn label_tr(self) -> &'static str {
        match self {
            Self::Grand => "Grand (en büyük döngü)",
            Self::Primary => "Primary (birincil)",
            Self::Intermediate => "Intermediate (ara)",
            Self::Minor => "Minor (minör)",
            Self::Minute => "Minute",
            Self::Minuette => "Minuette",
            Self::SubMinuette => "Subminuette (en ince)",
        }
    }

    /// Bir üst derece (daha geniş perspektif; klasik tabloda yukarı).
    pub const fn one_larger(self) -> Option<Self> {
        match self {
            Self::SubMinuette => Some(Self::Minuette),
            Self::Minuette => Some(Self::Minute),
            Self::Minute => Some(Self::Minor),
            Self::Minor => Some(Self::Intermediate),
            Self::Intermediate => Some(Self::Primary),
            Self::Primary => Some(Self::Grand),
            Self::Grand => None,
        }
    }

    /// İç dalga / alt derece (W1–W5’nin içindeki sayım için klasik “bir alt seviye”).
    pub const fn inner_degree(self) -> Option<Self> {
        match self {
            Self::Grand => Some(Self::Primary),
            Self::Primary => Some(Self::Intermediate),
            Self::Intermediate => Some(Self::Minor),
            Self::Minor => Some(Self::Minute),
            Self::Minute => Some(Self::Minuette),
            Self::Minuette => Some(Self::SubMinuette),
            Self::SubMinuette => None,
        }
    }
}

/// Elliott Wave International / klasik grafiklerde dereceye göre dalga gösterimi:
/// Grand → Roma büyük (I–V), Primary → (1), Intermediate → ①, Minor → [1], Minute → alt simge ₁,
/// Minuette → (i)–(v), Subminuette → üst simge ¹.
///
/// Ham `label` (`0`…`5`, `A`…`C`) mantık ve pivot konumu için aynı kalır; bu fonksiyon sadece **görünür metin** üretir.
pub fn format_wave_label_for_degree(degree: Option<WaveDegree>, raw: &str) -> String {
    let d = degree.unwrap_or(WaveDegree::Minute);
    let t = raw.trim();
    if t.is_empty() {
        return raw.to_string();
    }
    let up = t.to_ascii_uppercase();
    match up.as_str() {
        "0" => format_degree_zero(d),
        "1" | "2" | "3" | "4" | "5" => {
            if let Ok(n) = t.parse::<u8>() {
                if (1..=5).contains(&n) {
                    return format_impulse_by_degree(d, n);
                }
            }
            raw.to_string()
        }
        "A" | "B" | "C" => {
            let c = t.chars().next().unwrap_or('A');
            format_corrective_by_degree(d, c)
        }
        _ => raw.to_string(),
    }
}

fn format_degree_zero(d: WaveDegree) -> String {
    match d {
        WaveDegree::Grand => "0".to_string(),
        WaveDegree::Primary => "(0)".to_string(),
        WaveDegree::Intermediate => "⓪".to_string(),
        WaveDegree::Minor => "[0]".to_string(),
        WaveDegree::Minute => "₀".to_string(),
        WaveDegree::Minuette => "(0)".to_string(),
        WaveDegree::SubMinuette => "⁰".to_string(),
    }
}

fn format_impulse_by_degree(d: WaveDegree, n: u8) -> String {
    match d {
        WaveDegree::Grand => roman_upper(n),
        WaveDegree::Primary => format!("({n})"),
        WaveDegree::Intermediate => circled_digit(n),
        WaveDegree::Minor => format!("[{n}]"),
        WaveDegree::Minute => subscript_digit(n),
        WaveDegree::Minuette => format!("({})", roman_lower(n)),
        WaveDegree::SubMinuette => superscript_digit(n),
    }
}

fn roman_upper(n: u8) -> String {
    match n {
        1 => "I".into(),
        2 => "II".into(),
        3 => "III".into(),
        4 => "IV".into(),
        5 => "V".into(),
        _ => n.to_string(),
    }
}

fn roman_lower(n: u8) -> String {
    match n {
        1 => "i".into(),
        2 => "ii".into(),
        3 => "iii".into(),
        4 => "iv".into(),
        5 => "v".into(),
        _ => n.to_string(),
    }
}

fn circled_digit(n: u8) -> String {
    match n {
        0 => "⓪".to_string(),
        1..=9 => char::from_u32(0x2460 + (n as u32) - 1)
            .map(|c| c.to_string())
            .unwrap_or_else(|| n.to_string()),
        _ => n.to_string(),
    }
}

fn subscript_digit(n: u8) -> String {
    const DIG: [char; 6] = ['\u{2080}', '\u{2081}', '\u{2082}', '\u{2083}', '\u{2084}', '\u{2085}'];
    if (n as usize) < DIG.len() {
        DIG[n as usize].to_string()
    } else {
        n.to_string()
    }
}

fn superscript_digit(n: u8) -> String {
    match n {
        0 => "⁰".to_string(),
        1 => "¹".to_string(),
        2 => "²".to_string(),
        3 => "³".to_string(),
        4 => "⁴".to_string(),
        5 => "⁵".to_string(),
        _ => n.to_string(),
    }
}

fn circled_upper_letter(c: char) -> String {
    let u = c.to_ascii_uppercase() as u32;
    if (b'A'..=b'Z').contains(&(u as u8)) {
        char::from_u32(0x24B6 + (u - b'A' as u32))
            .map(|ch| ch.to_string())
            .unwrap_or_else(|| c.to_string())
    } else {
        c.to_string()
    }
}

fn format_corrective_by_degree(d: WaveDegree, c: char) -> String {
    let u = c.to_ascii_uppercase();
    match d {
        WaveDegree::Grand => u.to_string(),
        WaveDegree::Primary => format!("({u})"),
        WaveDegree::Intermediate => circled_upper_letter(u),
        WaveDegree::Minor => format!("[{u}]"),
        WaveDegree::Minute => u.to_string(),
        WaveDegree::Minuette => format!("({})", u.to_ascii_lowercase()),
        WaveDegree::SubMinuette => format!("({})", u), // ince derece: tek harf
    }
}

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

    /// Formation adından tahmini enum (sonraki formasyon referansı için)
    pub fn from_formation_name(name: &str) -> Option<Self> {
        let n = name.to_lowercase();
        if n.contains("impulse") || n.contains("itki") {
            return Some(ElliottFormation::Impulse);
        }
        if n.contains("zigzag") {
            return Some(ElliottFormation::Zigzag);
        }
        if n.contains("flat") {
            return Some(ElliottFormation::Flat);
        }
        if n.contains("triangle") || n.contains("üçgen") {
            return Some(ElliottFormation::ContractingTriangle);
        }
        if n.contains("leading") || n.contains("diagonal") {
            return Some(ElliottFormation::LeadingDiagonal);
        }
        if n.contains("ending") {
            return Some(ElliottFormation::EndingDiagonal);
        }
        None
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

/// Impulse tamamlandıktan sonra beklenen düzeltme (Zigzag/Flat) için referans seviyeleri.
/// Hesaplamalarda referans olarak kullanılabilir (A/B/C hedefleri).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostImpulseCorrectionRef {
    /// Düzeltme başlangıç fiyatı (W5 ucu)
    pub start_price: f64,
    /// Impulse tabanı (W0 – bullish’da dip, bearish’da zirve)
    pub end_price: f64,
    pub impulse_range: f64,
    pub is_bullish_impulse: bool,
    /// Düzeltme A dalgası hedefleri (W5’ten itibaren impulse range retrace: 0.382, 0.5, 0.618)
    pub correction_a_targets: Vec<f64>,
    /// B bölgesi (A bitişinden sonra B retrace – Zigzag tipik 0.382–0.786)
    pub correction_b_zone: (f64, f64),
    /// C hedefleri (A uzunluğunun 1.0, 1.382, 1.618 – B bitişinden; referans)
    pub correction_c_targets: Vec<f64>,
}

/// Mevcut formasyon tamamlandığında sonraki dalga formasyonları için referans seviyeleri.
/// Elliott dalgalarından sonra oluşacak/oluşan formasyonlar hesaplama referansı olarak kullanılabilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextFormationRefLevels {
    /// Olası sonraki formasyon adları (Zigzag, Flat, Triangle, …)
    pub expected_formations: Vec<String>,
    /// Impulse/Diagonal tamamlandıysa: düzeltme A/B/C referans seviyeleri
    pub post_impulse_correction: Option<PostImpulseCorrectionRef>,
}

/// Impulse tamamlandıktan sonra (W5 bitti) beklenen düzeltme için referans seviyelerini hesaplar.
/// w0_price: dalga 0 fiyatı, w5_price: dalga 5 ucu, is_bullish: yükseliş itkisi mi.
pub fn compute_post_impulse_correction_ref(
    w0_price: f64,
    w5_price: f64,
    is_bullish: bool,
) -> PostImpulseCorrectionRef {
    let impulse_range = (w5_price - w0_price).abs();
    let (start_price, end_price) = (w5_price, w0_price);

    let correction_a_targets: Vec<f64> = [0.382, 0.5, 0.618]
        .iter()
        .map(|&r| {
            if is_bullish {
                w5_price - impulse_range * r
            } else {
                w5_price + impulse_range * r
            }
        })
        .collect();

    let a_length = impulse_range * 0.618_f64;
    let a_end = correction_a_targets.get(1).copied().unwrap_or(if is_bullish {
        w5_price - impulse_range * 0.5
    } else {
        w5_price + impulse_range * 0.5
    });
    let correction_b_zone = if is_bullish {
        (a_end - a_length * 0.786, a_end - a_length * 0.382)
    } else {
        (a_end + a_length * 0.382, a_end + a_length * 0.786)
    };

    let correction_c_targets: Vec<f64> = [1.0, 1.236, 1.382, 1.618]
        .iter()
        .map(|&ext| {
            let b_mid = (correction_b_zone.0 + correction_b_zone.1) / 2.0;
            if is_bullish {
                b_mid - a_length * ext
            } else {
                b_mid + a_length * ext
            }
        })
        .collect();

    PostImpulseCorrectionRef {
        start_price,
        end_price,
        impulse_range,
        is_bullish_impulse: is_bullish,
        correction_a_targets,
        correction_b_zone,
        correction_c_targets,
    }
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
    /// Impulse: W4 düzeltmesi W1 tepe/dibinin ötesinde olmamalı (klasik: W4, W1 bölgesine girmez).
    pub w4_vs_w1_valid: bool,
    /// Impulse: W4 ekstremi W3 ekstremunun «geri» tarafında olmalı (bull: W4 dibi < W3 zirvesi; bear: tersi).
    pub w4_vs_w3_valid: bool,
    /// `w4_vs_w1_valid && w4_vs_w3_valid`
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
    // W2 düzeltme: bullish'ta W2 low > W0 ve W2 low < W1 high; bearish'ta simetrik
    let w2_valid = if is_bullish {
        w2_extreme > w0 && w2_extreme < w1_high
    } else {
        w2_extreme < w0 && w2_extreme > w1_low
    };
    let w1_len = (w1_high - w1_low).abs();
    let w3_len = (w3_extreme - w2_extreme).abs();
    // Spec: W3 asla en kısa olamaz. W5 varsa: w3 > min(w1,w5); yoksa: w3 >= w1
    let w3_valid = match w5_extreme {
        Some(w5) => {
            let w5_len = (w5 - w4_extreme).abs();
            w1_len > 0.0 && w3_len > w1_len.min(w5_len)
        }
        None => w1_len > 0.0 && w3_len >= w1_len,
    };
    let w1_extreme = if is_bullish { w1_high } else { w1_low };
    let w4_vs_w1_valid = if is_bullish {
        w4_extreme > w1_extreme
    } else {
        w4_extreme < w1_extreme
    };
    // W4, W3’ün motive ucu kadar ileri gitmemeli (bull’da W4 dibi < W3 zirvesi).
    let w4_vs_w3_valid = if is_bullish {
        w4_extreme < w3_extreme
    } else {
        w4_extreme > w3_extreme
    };
    let w4_valid = w4_vs_w1_valid && w4_vs_w3_valid;
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
        w4_vs_w1_valid,
        w4_vs_w3_valid,
        w4_valid,
        no_triple_extension_valid,
        formation_valid,
    }
}

/// Diagonal şekli – EWM Spec: Contracting (daralan) veya Expanding (genişleyen)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagonalShape {
    /// İki trend çizgisi birbirine yaklaşır. Hem Leading hem Ending olabilir.
    Contracting,
    /// İki trend çizgisi birbirinden uzaklaşır. Daha nadir.
    Expanding,
}

/// Diagonal iç yapı tipi – PDF kuralı
/// Leading Diagonal: 5-3-5-3-5 (motive dalgalar impulse yapıda)
/// Ending Diagonal:  3-3-3-3-3 (tüm dalgalar zigzag yapıda)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagonalSubStructure {
    /// LD: 5-3-5-3-5
    LeadingMotive,
    /// ED: 3-3-3-3-3
    EndingCorrective,
    /// Karışık veya belirsiz
    Mixed,
}

/// Leading/Ending Diagonal validasyonu – EWM Cheat Sheet kuralları 1:1
/// Kurallar: W4-W1 örtüşebilir. W3 en kısa olamaz. İki trend çizgisi daralan veya genişleyen.
#[derive(Debug, Clone)]
pub struct DiagonalValidation {
    pub w2_valid: bool,
    pub w3_valid: bool,
    pub formation_valid: bool,
    /// Contracting (daralan) veya Expanding (genişleyen) – kanal şekli
    pub shape: Option<DiagonalShape>,
    /// İç yapı kontrolü: LD(5-3-5-3-5) veya ED(3-3-3-3-3)
    pub sub_structure: Option<DiagonalSubStructure>,
    /// Her dalganın iç swing sayıları [W1, W2, W3, W4, W5]
    pub inner_counts: Option<[usize; 5]>,
}

/// Diagonal iç yapı tipi belirle: her dalganın iç swing sayısına göre
/// 5-dalgalı (impulse): >=4 iç swing → motive
/// 3-dalgalı (zigzag): 2-3 iç swing → corrective
pub fn classify_diagonal_sub_structure(inner_counts: &[usize; 5]) -> DiagonalSubStructure {
    let motive_waves = [0usize, 2, 4]; // W1, W3, W5
    let corrective_waves = [1usize, 3]; // W2, W4

    let motive_5wave = motive_waves.iter().all(|&i| inner_counts[i] >= 4);
    let motive_3wave = corrective_waves.iter().all(|&i| inner_counts[i] >= 2 && inner_counts[i] <= 3);
    let all_3wave = inner_counts.iter().all(|&c| c >= 2 && c <= 3);

    if motive_5wave && motive_3wave {
        DiagonalSubStructure::LeadingMotive
    } else if all_3wave {
        DiagonalSubStructure::EndingCorrective
    } else {
        DiagonalSubStructure::Mixed
    }
}

/// Diagonal kanal şekli: Contracting = zirveler düşer/dipler yükselir, Expanding = tersi
fn diagonal_shape(
    w0: f64,
    w1_high: f64,
    w1_low: f64,
    w2_extreme: f64,
    w3_extreme: f64,
    w4_extreme: f64,
    is_bullish: bool,
) -> Option<DiagonalShape> {
    if is_bullish {
        let highs_narrow = w3_extreme < w1_high;
        let lows_rise = w2_extreme > w0 && w4_extreme > w2_extreme;
        let highs_widen = w3_extreme > w1_high;
        let lows_fall = w2_extreme < w0 && w4_extreme < w2_extreme;
        if highs_narrow && lows_rise {
            Some(DiagonalShape::Contracting)
        } else if highs_widen && lows_fall {
            Some(DiagonalShape::Expanding)
        } else {
            None
        }
    } else {
        // Bearish: highs = w0,w2,w4; lows = w1_low,w3. Contracting = üst çizgi iner, alt çizgi yükselir
        let highs_fall = w2_extreme < w0 && w4_extreme < w2_extreme;
        let lows_rise = w3_extreme > w1_low;
        let highs_rise = w2_extreme > w0 && w4_extreme > w2_extreme;
        let lows_fall = w3_extreme < w1_low;
        if highs_fall && lows_rise {
            Some(DiagonalShape::Contracting)
        } else if highs_rise && lows_fall {
            Some(DiagonalShape::Expanding)
        } else {
            None
        }
    }
}

pub fn validate_diagonal(
    w0: f64,
    w1_high: f64,
    w1_low: f64,
    w2_extreme: f64,
    w3_extreme: f64,
    w4_extreme: f64,
    is_bullish: bool,
) -> DiagonalValidation {
    // Kural 1: W2 düzeltme – bullish'ta W2 low > W0 ve W2 low < W1 high
    let w2_valid = if is_bullish {
        w2_extreme > w0 && w2_extreme < w1_high
    } else {
        w2_extreme < w0 && w2_extreme > w1_low
    };
    let w1_len = (w1_high - w1_low).abs();
    let w3_len = (w3_extreme - w2_extreme).abs();
    // Kural 2: W3 en kısa olamaz (Impulse ile aynı)
    let w3_valid = w1_len > 0.0 && w3_len >= w1_len;
    // Kural 3: W4-W0 kırmamalı (motive diagonal yapıda trend kökünü bozmamak için)
    let w4_above_w0 = if is_bullish {
        w4_extreme > w0
    } else {
        w4_extreme < w0
    };
    // W4 düzeltmesi W3 ekstremunun gerisinde kalmalı (impulse ile aynı geometri).
    let w4_vs_w3 = if is_bullish {
        w4_extreme < w3_extreme
    } else {
        w4_extreme > w3_extreme
    };
    let w4_valid = w4_above_w0 && w4_vs_w3;
    // Kural 4: W4-W1 örtüşebilir (Diagonal'a özgü – validate_diagonal sadece W4 overlap durumunda çağrılır)
    let shape = diagonal_shape(w0, w1_high, w1_low, w2_extreme, w3_extreme, w4_extreme, is_bullish);
    DiagonalValidation {
        w2_valid,
        w3_valid,
        formation_valid: w2_valid && w3_valid && w4_valid,
        shape,
        sub_structure: None,
        inner_counts: None,
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
    /// TP1 bazlı R/R oranı
    pub rr1: f64,
    /// TP2 bazlı R/R oranı
    pub rr2: f64,
}

/// W5 setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupW5 {
    pub entry: f64,
    pub stop_loss: f64,
    pub tp: f64,
    pub tp_alternate: f64,
    pub is_long: bool,
    /// Ana TP bazlı R/R oranı
    pub rr: f64,
}

/// R/R hesapla: |TP - Entry| / |Entry - SL|
#[inline]
pub fn calc_rr(entry: f64, stop_loss: f64, take_profit: f64) -> f64 {
    let risk = (entry - stop_loss).abs();
    if risk < 1e-10 {
        return 0.0;
    }
    (take_profit - entry).abs() / risk
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
        let tp1 = w1_high;
        let tp2 = w1_low + w1_len * fibo::EXT_1618;
        SetupW3 {
            entry: w2_extreme,
            stop_loss: w0,
            tp1,
            tp2,
            is_long: true,
            rr1: calc_rr(w2_extreme, w0, tp1),
            rr2: calc_rr(w2_extreme, w0, tp2),
        }
    } else {
        let tp1 = w1_low;
        let tp2 = w1_high - w1_len * fibo::EXT_1618;
        SetupW3 {
            entry: w2_extreme,
            stop_loss: w0,
            tp1,
            tp2,
            is_long: false,
            rr1: calc_rr(w2_extreme, w0, tp1),
            rr2: calc_rr(w2_extreme, w0, tp2),
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
            rr: calc_rr(entry, w1_high, tp),
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
            rr: calc_rr(entry, w1_low, tp),
        }
    }
}

/// Corrective trade setup – Zigzag C dalgası veya Triangle E breakout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrSetup {
    pub entry: f64,
    pub stop_loss: f64,
    pub tp: f64,
    pub is_long: bool,
    pub rr: f64,
    pub setup_type: String,
}

/// Zigzag C dalgası setup: B noktasından giriş, A noktası SL, C hedefi TP
/// C hedefi: A dalgasının %100 veya %161.8'i (B'den itibaren)
pub fn compute_setup_zigzag_c(
    a_start: f64, a_end: f64, b_end: f64, is_bullish: bool,
) -> CorrSetup {
    let a_len = (a_end - a_start).abs();
    let (entry, stop_loss, tp) = if is_bullish {
        // Bearish zigzag: A aşağı, B yukarı düzeltme, C aşağı → short
        (b_end, a_start, b_end - a_len)
    } else {
        // Bullish zigzag: A yukarı, B aşağı düzeltme, C yukarı → long
        (b_end, a_start, b_end + a_len)
    };
    let is_long = !is_bullish;
    CorrSetup {
        entry,
        stop_loss,
        tp,
        is_long,
        rr: calc_rr(entry, stop_loss, tp),
        setup_type: "Zigzag C".to_string(),
    }
}

/// Triangle E breakout setup: E noktasından giriş, D noktası SL, thrust hedefi TP
pub fn compute_setup_triangle_e(
    a_len: f64, e_price: f64, d_price: f64, is_bullish_breakout: bool,
) -> CorrSetup {
    let tp = triangle_thrust_target(a_len, e_price, is_bullish_breakout);
    let (entry, stop_loss) = (e_price, d_price);
    CorrSetup {
        entry,
        stop_loss,
        tp,
        is_long: is_bullish_breakout,
        rr: calc_rr(entry, stop_loss, tp),
        setup_type: "Triangle E".to_string(),
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

/// Zigzag kuralı (EWM): B, A'nın başlangıç noktasını aşamaz.
/// Ek: B geri çekilme olmalı – A start ile A end arasında kalmalı (devam değil).
/// a_down: A düşüş dalgası ise true (A high→low)
pub fn zigzag_valid(a_start: f64, a_end: f64, b_extreme: f64, a_down: bool) -> bool {
    if a_down {
        b_extreme <= a_start && b_extreme >= a_end
    } else {
        b_extreme >= a_start && b_extreme <= a_end
    }
}

/// Zigzag ABC validasyonu: B retrace 38.2–85.4%, C extension 100–161.8%, C exceeds A end.
/// p0=A start, p1=A end, p2=B extreme, p3=C extreme.
/// is_bearish_zz: true = bearish zigzag (A up, B down, C up).
/// Returns (valid, c_targets) where c_targets are projection levels for C (100%, 123.6%, 138.2%, 161.8%).
pub fn validate_zigzag_abc(
    p0: f64,
    p1: f64,
    p2: f64,
    p3: f64,
    is_bearish_zz: bool,
) -> (bool, Vec<f64>) {
    let a_len = (p1 - p0).abs();
    if a_len < 1e-12 {
        return (false, vec![]);
    }
    let a_down = is_bearish_zz;
    if !zigzag_valid(p0, p1, p2, a_down) {
        return (false, vec![]);
    }
    let c_exceeds_a_end = if a_down { p3 < p1 } else { p3 > p1 };
    if !c_exceeds_a_end {
        return (false, vec![]);
    }
    let b_retrace = if a_down {
        (p2 - p1) / a_len
    } else {
        (p1 - p2) / a_len
    };
    let b_ok = b_retrace >= 0.382 && b_retrace <= 0.854;
    let c_len = (p3 - p2).abs();
    let c_ratio = c_len / a_len;
    let c_ok = c_ratio >= 0.99 && c_ratio <= 1.65;
    let valid = b_ok && c_ok;

    let c_targets: Vec<f64> = fibo::ZIGZAG_C_EXTENSIONS
        .iter()
        .map(|&ext| {
            if a_down {
                p2 - a_len * ext
            } else {
                p2 + a_len * ext
            }
        })
        .collect();
    (valid, c_targets)
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
        // 1.05 < B < 1.236: B, A başlangıcını aştı ama extended değil → Running
        Some(FlatType::Running)
    }
}

/// Flat validasyonu: Regular B≈%90–100, Expanded B>%123.6, Running (B aştığında C kısa)
///
/// `b_retrace` işaretli tutulur: B, A’nın düzeltme yönünde değilse (ör. bull A sonrası B daha da yukarı)
/// oran negatif olur → `flat_type` eşiğinin altında kalır ve flat reddedilir (`.abs()` ile maskelenmez).
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
    let b_ratio = b_retrace;
    let typ = flat_type(b_ratio);
    if typ.is_none() {
        return (false, None);
    }
    let c_retrace = if a_down {
        (a_start - c_extreme).abs() / a_len
    } else {
        (c_extreme - a_start).abs() / a_len
    };
    // Spec: Regular C ≈ A %100; Expanded B/C üst sınır (aşırı oranlar “flat” değil); Running: flat_type’taki (1.05,1.236) bant + C kısa
    const EXPANDED_B_MAX: f64 = 2.0;
    const EXPANDED_C_MAX: f64 = fibo::EXT_2618;
    let valid = match typ.unwrap() {
        FlatType::Regular => b_ratio >= 0.9 && b_ratio <= 1.05 && c_retrace >= 0.85 && c_retrace <= 1.15,
        FlatType::Expanded => {
            b_ratio >= 1.236
                && b_ratio <= EXPANDED_B_MAX
                && c_retrace >= 1.0
                && c_retrace <= EXPANDED_C_MAX
        }
        FlatType::Running => {
            b_ratio > 1.05 && b_ratio < 1.236 && c_retrace < 1.0 && c_retrace >= 0.5
        }
    };
    (valid, typ)
}

/// Zaman Fibonacci: W3 süresi W1'in %100, %161.8 veya %261.8'i
pub fn time_projection_w3(w1_bars: u32, ratio: f64) -> u32 {
    ((w1_bars as f64) * ratio) as u32
}

/// W5 süre tahmini: W1 süresinin Fibonacci katları
/// PDF: "Time relationships between waves exist but are less reliable"
/// Dönüş: (bars_100, bars_618, bars_1618) – W5'in W1'e göre olası süresi
pub fn time_projection_w5(w1_duration: i64) -> (i64, i64, i64) {
    let d = w1_duration as f64;
    (
        d as i64,                    // %100 – W5 = W1
        (d * 0.618) as i64,          // %61.8 – sıkıştırılmış W5
        (d * 1.618) as i64,          // %161.8 – uzatılmış W5
    )
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

/// Triangle E-dalgası sonrası thrust hedefi.
/// PDF: Triangle tamamlandığında, A dalgasının uzunluğu kadar trend yönünde kırılım beklenir.
/// a_len: A dalgasının uzunluğu, e_price: E noktası fiyatı
pub fn triangle_thrust_target(a_len: f64, e_price: f64, is_bullish_breakout: bool) -> f64 {
    if is_bullish_breakout {
        e_price + a_len
    } else {
        e_price - a_len
    }
}

/// Truncation (kesilmiş W5) tespiti.
/// PDF: "Rarely, the fifth wave will not have enough momentum to exceed the end of wave 3."
/// W5 ucu W3 ucunu aşamadıysa truncation var demektir. Formasyon yine geçerli kalır ama
/// trendin zayıfladığı sinyali verir.
pub fn detect_truncation(
    w3_extreme: f64,
    w5_extreme: f64,
    is_bullish: bool,
) -> bool {
    if is_bullish {
        w5_extreme < w3_extreme
    } else {
        w5_extreme > w3_extreme
    }
}

/// Throw-over tespiti: W5 kanal çizgisini aşar (overshoot).
/// PDF: W5 bazen kanalı aşar → ardından sert geri dönüş gelir.
/// channel_target: kanal paralel çizgisinin W5 bitiş zamanındaki fiyatı.
pub fn detect_throw_over(
    w5_extreme: f64,
    channel_target: f64,
    is_bullish: bool,
) -> bool {
    if is_bullish {
        w5_extreme > channel_target
    } else {
        w5_extreme < channel_target
    }
}

/// Extended dalga tespiti: W1, W3, W5 arasında hangisi "extended" (en uzun)?
/// PDF: "Most commonly wave 3 is extended"
/// Dönüş: (extended_wave: 1/3/5, ratio: extended/next_longest)
pub fn detect_extended_wave(w1_len: f64, w3_len: f64, w5_len: f64) -> (u8, f64) {
    let max = w1_len.max(w3_len).max(w5_len);
    let (wave, second) = if max == w3_len {
        (3u8, w1_len.max(w5_len))
    } else if max == w1_len {
        (1u8, w3_len.max(w5_len))
    } else {
        (5u8, w1_len.max(w3_len))
    };
    let ratio = if second > 1e-10 { max / second } else { 0.0 };
    (wave, ratio)
}

/// W1 ≈ W5 eşitlik kontrolü (W3 extended olduğunda)
/// PDF: "When wave 3 is extended, waves 1 and 5 tend toward equality"
pub fn w1_w5_equality(w1_len: f64, w5_len: f64) -> f64 {
    let max = w1_len.max(w5_len).max(1e-10);
    let min = w1_len.min(w5_len);
    min / max
}

/// Alternation kuralı (PDF): "Wave 2 usually corrects in a different pattern than wave 4."
/// Derinlik alternasyonu: W2 derin (>%50) ise W4 sığ (<%50) beklenir veya tam tersi.
/// Yapısal alternasyon: W2 sharp (zigzag) ise W4 flat/triangle beklenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlternationResult {
    /// W2 ve W4 iyi bir alternasyon gösteriyor
    Good,
    /// Alternasyon zayıf (ikisi de yakın derinlikte)
    Weak,
    /// Alternasyon ihlali (ikisi de çok derin veya ikisi de çok sığ)
    Violation,
}

/// W2 ve W4 derinlik alternasyonu: biri >%50, diğeri <%50 olmalı.
/// w2_retrace_ratio: W2'nin W1'e geri çekilme oranı (0..1)
/// w4_retrace_ratio: W4'ün W3'e geri çekilme oranı (0..1)
pub fn check_alternation_depth(
    w2_retrace_ratio: f64,
    w4_retrace_ratio: f64,
) -> AlternationResult {
    let w2_deep = w2_retrace_ratio > 0.5;
    let w4_deep = w4_retrace_ratio > 0.5;
    if w2_deep != w4_deep {
        AlternationResult::Good
    } else {
        let diff = (w2_retrace_ratio - w4_retrace_ratio).abs();
        if diff > 0.15 {
            AlternationResult::Weak
        } else {
            AlternationResult::Violation
        }
    }
}

/// Düzeltme dalga tipi – yapısal alternation kontrolü için
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrWaveType {
    /// Sharp (zigzag): hızlı, derin düzeltme
    Sharp,
    /// Sideways (flat/triangle): yatay, zaman bazlı düzeltme
    Sideways,
    /// Belirsiz
    Unknown,
}

/// İç swing sayısına ve retrace oranına göre düzeltme dalga tipini belirle.
/// sharp (zigzag): genellikle 2 iç swing, derin (>%50)
/// sideways (flat/triangle): genellikle 4+ iç swing, sığ (<%50)
pub fn classify_corrective_type(inner_swing_count: usize, retrace_ratio: f64) -> CorrWaveType {
    if retrace_ratio > 0.5 && inner_swing_count <= 3 {
        CorrWaveType::Sharp
    } else if retrace_ratio <= 0.5 || inner_swing_count >= 4 {
        CorrWaveType::Sideways
    } else {
        CorrWaveType::Unknown
    }
}

/// Yapısal alternation kontrolü (formasyon tipi bazlı):
/// W2 sharp (zigzag) ise W4 sideways (flat/triangle) olmalı, veya tersi.
pub fn check_alternation_structural(
    w2_type: CorrWaveType,
    w4_type: CorrWaveType,
) -> AlternationResult {
    match (w2_type, w4_type) {
        (CorrWaveType::Sharp, CorrWaveType::Sideways)
        | (CorrWaveType::Sideways, CorrWaveType::Sharp) => AlternationResult::Good,
        (CorrWaveType::Unknown, _) | (_, CorrWaveType::Unknown) => AlternationResult::Weak,
        (CorrWaveType::Sharp, CorrWaveType::Sharp)
        | (CorrWaveType::Sideways, CorrWaveType::Sideways) => AlternationResult::Violation,
    }
}

/// Triangle alt-tipleri – PDF p.22-24
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriangleSubtype {
    /// Symmetrical (simetrik): hem üst hem alt çizgi eşit daralar
    Symmetrical,
    /// Ascending (yükselen): üst çizgi düz, alt çizgi yükseliyor
    Ascending,
    /// Descending (alçalan): alt çizgi düz, üst çizgi düşüyor
    Descending,
    /// Running: B dalgası A başlangıcını aşar
    Running,
}

/// Triangle alt-tipini belirle: high/low noktalarının davranışına göre.
/// highs: [h0, h1, h2] – zirve noktaları (sırasıyla)
/// lows: [l0, l1, l2] – dip noktaları
/// b_exceeds_a_start: B dalgası A başlangıcını aşıyor mu
pub fn classify_triangle_subtype(
    highs: [f64; 3],
    lows: [f64; 3],
    b_exceeds_a_start: bool,
) -> TriangleSubtype {
    if b_exceeds_a_start {
        return TriangleSubtype::Running;
    }
    let top_flat = (highs[0] - highs[2]).abs() / highs[0].max(1e-10) < 0.02;
    let bot_flat = (lows[0] - lows[2]).abs() / lows[0].max(1e-10) < 0.02;
    let top_falling = highs[0] > highs[1] && highs[1] > highs[2];
    let bot_rising = lows[0] < lows[1] && lows[1] < lows[2];

    if top_flat && bot_rising {
        TriangleSubtype::Ascending
    } else if bot_flat && top_falling {
        TriangleSubtype::Descending
    } else {
        TriangleSubtype::Symmetrical
    }
}

/// Triangle ABCDE validasyonu (3-3-3-3-3) – EWM Spec
/// 6 nokta: p0..p5 → A=(p0,p1), B=(p1,p2), C=(p2,p3), D=(p3,p4), E=(p4,p5)
/// Her dalga öncekinin ~%61.8 veya %78.6 (tolerance: 0.55–0.90)
pub fn validate_triangle_abcde(
    p0: f64,
    p1: f64,
    p2: f64,
    p3: f64,
    p4: f64,
    p5: f64,
    first_is_high: bool,
) -> bool {
    let lens = [
        (p1 - p0).abs(),
        (p2 - p1).abs(),
        (p3 - p2).abs(),
        (p4 - p3).abs(),
        (p5 - p4).abs(),
    ];
    let highs = if first_is_high {
        [p0, p2, p4]
    } else {
        [p1, p3, p5]
    };
    let lows = if first_is_high {
        [p1, p3, p5]
    } else {
        [p0, p2, p4]
    };
    let contracting = highs[0] > highs[1] && highs[1] > highs[2] && lows[0] < lows[1] && lows[1] < lows[2];
    let expanding = highs[0] < highs[1] && highs[1] < highs[2] && lows[0] > lows[1] && lows[1] > lows[2];

    if contracting {
        // Contracting: her ardışık dalga öncekinin ~.618'i civarında (toleranslı .55-.90)
        for i in 1..5 {
            if lens[i - 1] < 1e-10 { return false; }
            let ratio = lens[i] / lens[i - 1];
            if ratio < 0.50 || ratio > 0.95 { return false; }
        }
        return true;
    }

    if expanding {
        // PDF p.39: Expanding triangle → her dalga öncekinin 1.618 katı civarında
        // Toleranslı aralık: 1.05 .. 2.00
        for i in 1..5 {
            if lens[i - 1] < 1e-10 { return false; }
            let ratio = lens[i] / lens[i - 1];
            if ratio < 1.05 || ratio > 2.00 { return false; }
        }
        return true;
    }

    false
}

/// Impulse kanal (channeling) hesaplama – PDF: "The Elliott Wave Channel"
///
/// İlk kanal (0-2 çizgisi + 1 paraleli): W1 bittikten, W2 oluştuktan sonra çizilebilir.
/// Son kanal (2-4 çizgisi + 3 paraleli): W4 bittikten sonra çizilebilir – W5 hedefini verir.
///
/// Zaman ekseni olarak bar indeksi (i64 timestamp) kullanılır.
/// Dönüş: W5 için kanal üst/alt sınırı tahmini (fiyat).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpulseChannel {
    /// 2-4 baz çizgisi: (time1,price1) – (time2,price2)
    pub base_t1: i64,
    pub base_p1: f64,
    pub base_t2: i64,
    pub base_p2: f64,
    /// 1-3 (veya 3) paralel çizgi
    pub parallel_t: i64,
    pub parallel_p: f64,
    /// W5 için kanal hedefi: paralel çizginin t5 anındaki fiyatı
    pub w5_channel_target: f64,
}

/// W2-W4 baz çizgisi + W3 paraleli ile W5 kanal hedefini hesapla.
/// t0..t4: zaman (timestamp/1000), p0..p4: fiyatlar, t5_est: W5'in tahmini bitiş zamanı.
/// Bullish: kanal üst sınırı W5 hedefi; bearish: kanal alt sınırı.
///
/// PDF p.27-28: "If wave 3 is abnormally strong, almost vertical, then a parallel
/// drawn from its top may be too high. A parallel to the baseline that touches
/// the top of wave 1 is then more useful."
pub fn compute_impulse_channel(
    t2: i64,
    p2: f64,
    t3: i64,
    p3: f64,
    t4: i64,
    p4: f64,
    t5_est: i64,
    _is_bullish: bool,
) -> Option<ImpulseChannel> {
    let dt_base = (t4 - t2) as f64;
    if dt_base.abs() < 1e-10 {
        return None;
    }
    let slope = (p4 - p2) / dt_base;

    let parallel_p_at_t3 = p3;
    let intercept_parallel = parallel_p_at_t3 - slope * (t3 - t2) as f64;

    let w5_target = slope * (t5_est - t2) as f64 + intercept_parallel;

    Some(ImpulseChannel {
        base_t1: t2,
        base_p1: p2,
        base_t2: t4,
        base_p2: p4,
        parallel_t: t3,
        parallel_p: p3,
        w5_channel_target: w5_target,
    })
}

/// Alternatif kanal: W3 anormal güçlüyse W1 tepesinden paralel çizgi kullan.
/// PDF: "a parallel to the baseline that touches the top of wave one is then more useful."
pub fn compute_impulse_channel_alt(
    t1: i64,
    p1: f64,
    t2: i64,
    p2: f64,
    t4: i64,
    p4: f64,
    t5_est: i64,
) -> Option<ImpulseChannel> {
    let dt_base = (t4 - t2) as f64;
    if dt_base.abs() < 1e-10 {
        return None;
    }
    let slope = (p4 - p2) / dt_base;
    let intercept_parallel = p1 - slope * (t1 - t2) as f64;
    let w5_target = slope * (t5_est - t2) as f64 + intercept_parallel;

    Some(ImpulseChannel {
        base_t1: t2,
        base_p1: p2,
        base_t2: t4,
        base_p2: p4,
        parallel_t: t1,
        parallel_p: p1,
        w5_channel_target: w5_target,
    })
}

/// PDF p.26-27: "Depth of Corrective Waves"
/// W2 genellikle W1'in iç W4 seviyesinde biter.
/// W4 genellikle W3'ün iç W4 seviyesinde biter.
/// İç W4 seviyesi: ana dalganın (W1/W3) .382 retrace noktası.
pub fn depth_of_corrective_target(
    wave_start: f64,
    wave_end: f64,
    bullish: bool,
) -> f64 {
    let range = (wave_end - wave_start).abs();
    if bullish {
        wave_end - range * 0.382
    } else {
        wave_end + range * 0.382
    }
}

/// PDF p.26-27: Gerçek iç W4 seviyesini alt-dalga yapısından tespit et.
/// Motive dalga içinde sub-W4, son corrective dönüş noktasıdır.
/// Bullish'te son trough (is_high=false), bearish'te son peak (is_high=true).
/// Yeterli iç swing yoksa .382 heuristic'e fallback yapar.
pub fn depth_of_corrective_target_from_subwaves(
    wave_start: f64,
    wave_end: f64,
    bullish: bool,
    inner_swings: &[(i64, f64, bool)],
) -> f64 {
    let corrective_points: Vec<f64> = inner_swings
        .iter()
        .filter(|(_, _, is_high)| if bullish { !*is_high } else { *is_high })
        .map(|(_, p, _)| *p)
        .collect();

    if corrective_points.len() >= 2 {
        return *corrective_points.last().unwrap();
    }

    depth_of_corrective_target(wave_start, wave_end, bullish)
}

/// Semi-log kanal hedefi: fiyatları logaritmik ölçekte hesaplayıp geri dönüştür.
/// PDF p.28: "switch to the other scale (semilog) in order to observe the channel"
pub fn compute_impulse_channel_semilog(
    t2: i64,
    p2: f64,
    t3: i64,
    p3: f64,
    t4: i64,
    p4: f64,
    t5_est: i64,
) -> Option<f64> {
    if p2 <= 0.0 || p3 <= 0.0 || p4 <= 0.0 {
        return None;
    }
    let dt_base = (t4 - t2) as f64;
    if dt_base.abs() < 1e-10 {
        return None;
    }
    let lp2 = p2.ln();
    let lp3 = p3.ln();
    let lp4 = p4.ln();
    let slope = (lp4 - lp2) / dt_base;
    let intercept = lp3 - slope * (t3 - t2) as f64;
    let log_target = slope * (t5_est - t2) as f64 + intercept;
    Some(log_target.exp())
}

/// Alt-dalga yapısı doğrulama sonucu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubWaveValidation {
    /// W1 iç swing sayısı (beklenen: ≥4 → 5-dalgalı)
    pub w1_inner: usize,
    /// W2 iç swing sayısı (beklenen: ≥2 → 3-dalgalı)
    pub w2_inner: usize,
    /// W3 iç swing sayısı (beklenen: ≥4 → 5-dalgalı)
    pub w3_inner: usize,
    /// W4 iç swing sayısı (beklenen: ≥2 → 3-dalgalı)
    pub w4_inner: usize,
    /// W5 iç swing sayısı (beklenen: ≥4 → 5-dalgalı)
    pub w5_inner: usize,
    /// Genel geçerlilik: motive dalgalar 5-dalgalı, corrective dalgalar 3-dalgalı mı
    pub valid: bool,
    /// Kaç dalga beklenen yapıyla uyumlu (0-5)
    pub conforming_count: u8,
    /// Level-2 recursive: kaç alt-dalganın kendi iç yapısı da doğrulandı
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deep_conforming: Option<u8>,
    /// Level-2 recursive: kaç alt-dalga kontrol edilebildi
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deep_total_checked: Option<u8>,
    /// Level-2 recursive genel geçerlilik
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deep_valid: Option<bool>,
    /// Truncated W5 iç yapısı: 5-dalgalı yapıya sahip mi (≥4 iç swing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated_w5_inner_ok: Option<bool>,
}

/// İç swing sayısına göre alt-dalga yapısını doğrula.
/// PDF: "Waves 1, 3, and 5 are themselves motive (5-wave), waves 2 and 4 are corrective (3-wave)."
/// inner_counts: [W1, W2, W3, W4, W5] iç swing sayıları
pub fn validate_subwave_structure(inner_counts: [usize; 5]) -> SubWaveValidation {
    // 5-dalgalı motive yapı → en az 4 iç swing (5 nokta arası 4 bacak)
    // 3-dalgalı corrective yapı → en az 2 iç swing
    let w1_ok = inner_counts[0] >= 4;
    let w2_ok = inner_counts[1] >= 2 && inner_counts[1] <= 5;
    let w3_ok = inner_counts[2] >= 4;
    let w4_ok = inner_counts[3] >= 2 && inner_counts[3] <= 5;
    let w5_ok = inner_counts[4] >= 4;

    let conforming = [w1_ok, w2_ok, w3_ok, w4_ok, w5_ok]
        .iter()
        .filter(|&&x| x)
        .count() as u8;

    // En az 3/5 uyum → geçerli (toleranslı — gerçek piyasada alt swing'ler her zaman ideal sayıda olmayabilir)
    let valid = conforming >= 3;

    SubWaveValidation {
        w1_inner: inner_counts[0],
        w2_inner: inner_counts[1],
        w3_inner: inner_counts[2],
        w4_inner: inner_counts[3],
        w5_inner: inner_counts[4],
        valid,
        conforming_count: conforming,
        deep_conforming: None,
        deep_total_checked: None,
        deep_valid: None,
        truncated_w5_inner_ok: None,
    }
}

/// Level-2 recursive alt-dalga doğrulaması.
/// `level2_counts[w]` = w. dalganın iç alt-dalgalarının her birinin iç swing sayıları.
///
/// Motive dalga (W1,W3,W5) 5 alt-dalgadan oluşur → sub-W1,3,5 ≥ 2 iç swing, sub-W2,4 ≥ 1.
/// Corrective dalga (W2,W4) 3 alt-dalgadan oluşur → sub-A,C ≥ 2 iç swing, sub-B ≥ 1.
pub fn validate_subwave_deep(
    swv: &mut SubWaveValidation,
    level2_counts: &[Vec<usize>; 5],
) {
    let mut checked: u8 = 0;
    let mut conforming: u8 = 0;

    for (w, sub_counts) in level2_counts.iter().enumerate() {
        let is_motive = w == 0 || w == 2 || w == 4; // W1, W3, W5
        if is_motive {
            if sub_counts.len() >= 5 {
                checked += 1;
                let sub_w1_ok = sub_counts[0] >= 2;
                let sub_w2_ok = sub_counts[1] >= 1;
                let sub_w3_ok = sub_counts[2] >= 2;
                let sub_w4_ok = sub_counts[3] >= 1;
                let sub_w5_ok = sub_counts[4] >= 2;
                let ok_count = [sub_w1_ok, sub_w2_ok, sub_w3_ok, sub_w4_ok, sub_w5_ok]
                    .iter()
                    .filter(|&&x| x)
                    .count();
                if ok_count >= 3 {
                    conforming += 1;
                }
            }
        } else {
            if sub_counts.len() >= 3 {
                checked += 1;
                let sub_a_ok = sub_counts[0] >= 2;
                let sub_b_ok = sub_counts[1] >= 1;
                let sub_c_ok = sub_counts[2] >= 2;
                let ok_count = [sub_a_ok, sub_b_ok, sub_c_ok]
                    .iter()
                    .filter(|&&x| x)
                    .count();
                if ok_count >= 2 {
                    conforming += 1;
                }
            }
        }
    }

    swv.deep_total_checked = Some(checked);
    swv.deep_conforming = Some(conforming);
    swv.deep_valid = if checked >= 2 {
        Some(conforming as f64 / checked as f64 >= 0.5)
    } else {
        None // yetersiz veri
    };
}

/// Nested extension tespiti: bir dalganın kendi içinde de extension olup olmadığını kontrol et.
/// PDF p.16: "the third wave of an extended third wave is itself an extension"
/// inner_swings: ilgili dalga aralığındaki iç swing'ler
/// Dönüş: (nested_extended: bool, nested_ratio: f64)
pub fn detect_nested_extension(inner_swings: &[(i64, f64, bool)]) -> (bool, f64) {
    if inner_swings.len() < 5 {
        return (false, 0.0);
    }
    // İç dalga bacaklarını hesapla
    let mut legs: Vec<f64> = Vec::new();
    for i in 0..inner_swings.len() - 1 {
        legs.push((inner_swings[i + 1].1 - inner_swings[i].1).abs());
    }
    if legs.len() < 3 {
        return (false, 0.0);
    }
    // En uzun bacağı bul
    let max_leg = legs.iter().cloned().fold(0.0_f64, f64::max);
    let avg_leg = legs.iter().sum::<f64>() / legs.len() as f64;
    if avg_leg < 1e-10 {
        return (false, 0.0);
    }
    let ratio = max_leg / avg_leg;
    // 1.618'den büyük oran → iç extension
    (ratio > 1.618, ratio)
}

/// Corrective dalganın iç yapı sayısını doğrula (Zigzag/Flat).
/// PDF: Zigzag A=5dalga, B=3dalga, C=5dalga; Flat A=3, B=3, C=5
/// inner_counts: [A_inner, B_inner, C_inner]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrSubWaveValidation {
    pub a_inner: usize,
    pub b_inner: usize,
    pub c_inner: usize,
    pub pattern: String,
    pub valid: bool,
}

pub fn validate_corrective_subwaves(
    inner_counts: [usize; 3],
    is_zigzag: bool,
) -> CorrSubWaveValidation {
    let (a_ok, b_ok, c_ok, pattern) = if is_zigzag {
        (inner_counts[0] >= 4, inner_counts[1] >= 2, inner_counts[2] >= 4, "5-3-5")
    } else {
        (inner_counts[0] >= 2, inner_counts[1] >= 2, inner_counts[2] >= 4, "3-3-5")
    };
    let valid = [a_ok, b_ok, c_ok].iter().filter(|&&x| x).count() >= 2;
    CorrSubWaveValidation {
        a_inner: inner_counts[0],
        b_inner: inner_counts[1],
        c_inner: inner_counts[2],
        pattern: pattern.to_string(),
        valid,
    }
}

#[cfg(test)]
mod impulse_validation_tests {
    use super::{validate_impulse, validate_impulse_with_w5};

    /// ETHUSDT benzeri: W4 etiketi W3'ten yüksek → impulse geçersiz.
    #[test]
    fn rejects_bullish_w4_extreme_not_below_w3() {
        let v = validate_impulse(
            2060.09, 2123.25, 2060.09, 2085.10, 2288.00, 2305.60, true,
        );
        assert!(v.w4_vs_w1_valid, "W1 bölgesi dışında kalmış örnek");
        assert!(!v.w4_vs_w3_valid);
        assert!(!v.w4_valid);
        assert!(!v.formation_valid);
    }

    #[test]
    fn accepts_bullish_w4_dip_below_w3() {
        let v = validate_impulse(
            100.0, 110.0, 100.0, 105.0, 130.0, 120.0, true,
        );
        assert!(v.w4_vs_w3_valid);
        assert!(v.w4_vs_w1_valid);
        assert!(v.w4_valid);
    }

    #[test]
    fn rejects_bearish_w4_extreme_not_above_w3() {
        let v = validate_impulse(
            200.0, 200.0, 180.0, 195.0, 150.0, 145.0, false,
        );
        assert!(!v.w4_vs_w3_valid);
        assert!(!v.formation_valid);
    }

    #[test]
    fn w5_present_still_requires_w4_vs_w3() {
        let v = validate_impulse_with_w5(
            2060.09, 2123.25, 2060.09, 2085.10, 2288.00, 2305.60, Some(2400.0), true,
        );
        assert!(!v.w4_vs_w3_valid);
        assert!(!v.formation_valid);
    }
}

#[cfg(test)]
mod flat_valid_detailed_tests {
    use super::{flat_valid_detailed, FlatType};

    /// Cyan örnek: A yukarı, B daha da yukarı → b_retrace negatif; abs olmadan flat reddedilir.
    #[test]
    fn rejects_b_when_same_direction_as_bullish_a() {
        let (valid, typ) = flat_valid_detailed(2288.0, 2305.60, 2349.99, 2150.70, false);
        assert!(!valid);
        assert!(typ.is_none());
    }

    /// Turuncu örnek: B/A > 2 → Expanded üst sınırı aşılır.
    #[test]
    fn rejects_expanded_flat_excessive_b_ratio() {
        let (valid, _) = flat_valid_detailed(2148.0, 2088.52, 2209.49, 2077.03, true);
        assert!(!valid);
    }

    /// Mor örnek: genişlemiş flat, B ve C oranları makul bantta.
    #[test]
    fn accepts_typical_expanded_flat() {
        let (valid, typ) = flat_valid_detailed(2155.90, 2120.04, 2175.84, 2114.20, true);
        assert!(valid);
        assert_eq!(typ, Some(FlatType::Expanded));
    }
}
