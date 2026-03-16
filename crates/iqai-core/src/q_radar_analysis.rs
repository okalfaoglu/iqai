//! Q-RADAR fırsat analizi – merkezi modül.
//!
//! Fırsat tespiti için detaylı analiz: RADAR sinyali, dip/tepe dönüş gücü,
//! tespit etiketi (DİP BÖLGESİ / TEPE BÖLGESİ), güven skoru, erken uyarı, tavsiye.
//! Çoklu doğrulama (MTF destek, yapı kırılımı, Elliott/Fib cluster, divergence)
//! ile daha doğru ve erken sinyaller. Hem robot hem web bu modülü çağırır.

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::dip_confluence::compute_dip_confluence;
use crate::reversal::{compute_reversal_analysis, DipAnalysis, PeakAnalysis};
use crate::signal::{CandleBuffer, SignalEngine};
use crate::types::{QRadarSignal, SignalType, Timeframe};

/// Confluence ile güven/erken uyarı artışı (katman başına)
const CONFLUENCE_BOOST_PER_LAYER: f64 = 0.6;
/// Maksimum confluence artışı (0–10 skorları taşmasın)
const CONFLUENCE_BOOST_CAP: f64 = 2.5;

/// Q-RADAR fırsat analizi çıktısı – robot ve web aynı yapıyı kullanır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QRadarOpportunityAnalysis {
    /// Sembol
    pub symbol: String,
    /// Zaman dilimi
    pub timeframe: Timeframe,
    /// Ham RADAR sinyali (varsa)
    pub radar: Option<QRadarSignal>,
    /// Dip analizi (pivot low, dipten dönüş, güç)
    pub dip: Option<DipAnalysis>,
    /// Tepe analizi (pivot high, tepeden dönüş, düşüş gücü)
    pub peak: Option<PeakAnalysis>,
    /// Tespit etiketi: "DİP BÖLGESİ (TEPKİ DİBİ)", "TEPE BÖLGESİ", "—"
    pub detection: String,
    /// Güven skoru 0–10 (gösterim için; RADAR confidence + yapı)
    pub confidence_score: f64,
    /// Erken uyarı skoru 0–10 (dip/tepe gücü veya RADAR)
    pub early_warning_score: f64,
    /// Tavsiye: "ZAYIF DİP – İzle", "GÜÇLÜ DİP – İzle", "TEPE – İzle", "—"
    pub recommendation: String,
    /// Onay katmanı sayısı (örn. 3/5 – isteğe bağlı)
    pub confirmation_layers: Option<String>,
    /// Fırsat yönü: "LONG", "SHORT" veya "—"
    pub direction: String,
    /// Referans fiyat (son kapanış veya RADAR reference)
    pub reference_price: f64,
}

/// Merkezi Q-RADAR fırsat analizi. CLI ve Web bu fonksiyonu çağırır.
pub fn compute_q_radar_opportunity(
    buffer: &CandleBuffer,
    chart_tf: Timeframe,
    symbol: &str,
    config: &Config,
) -> QRadarOpportunityAnalysis {
    let engine = SignalEngine::new(config.clone());
    let radar = engine.compute_q_radar(buffer, chart_tf, symbol);

    let candles = buffer.get(chart_tf);
    let (dip, peak) = match candles {
        Some(c) => {
            let rev = compute_reversal_analysis(c, Some(config.pivot_length as usize));
            (rev.dip, rev.peak)
        }
        None => (None, None),
    };

    let reference_price = radar
        .as_ref()
        .map(|r| r.reference_price)
        .or_else(|| {
            candles
                .and_then(|c| c.last())
                .map(|c| c.close)
        })
        .unwrap_or(0.0);

    let (detection, mut confidence_score, mut early_warning_score, recommendation, direction, mut confirmation_layers) =
        build_detection_and_recommendation(&radar, &dip, &peak);

    let is_long = direction == "LONG";
    let mut final_recommendation = recommendation.clone();
    if detection != "—" && (is_long || direction == "SHORT") {
        let confluence = compute_dip_confluence(
            buffer,
            chart_tf,
            config,
            reference_price,
            is_long,
            dip.as_ref(),
            peak.as_ref(),
        );
        let boost = (confluence.layers_passed as f64 * CONFLUENCE_BOOST_PER_LAYER).min(CONFLUENCE_BOOST_CAP);
        confidence_score = (confidence_score + boost).min(10.0);
        early_warning_score = (early_warning_score + boost).min(10.0);
        confirmation_layers = Some(format!("{}/8 katman", confluence.layers_passed));
        if config.q_require_mtf_for_dip_zone && !confluence.mtf_support_near {
            // Opsiyonel: MTF destek yoksa dip/tepe tespitini gösterme
            return QRadarOpportunityAnalysis {
                symbol: symbol.to_string(),
                timeframe: chart_tf,
                radar,
                dip,
                peak,
                detection: "—".to_string(),
                confidence_score: 0.0,
                early_warning_score: 0.0,
                recommendation: "—".to_string(),
                confirmation_layers: Some(format!("{}/8 katman (MTF yok)", confluence.layers_passed)),
                direction: "—".to_string(),
                reference_price,
            };
        }
        if confidence_score >= 7.0 && early_warning_score >= 7.0 {
            final_recommendation = if is_long {
                "GÜÇLÜ DİP – İzle".to_string()
            } else {
                "GÜÇLÜ TEPE – İzle".to_string()
            };
        } else if confidence_score >= 5.0 || early_warning_score >= 5.0 {
            final_recommendation = if is_long {
                "ZAYIF DİP – İzle".to_string()
            } else {
                "ZAYIF TEPE – İzle".to_string()
            };
        }
    }

    QRadarOpportunityAnalysis {
        symbol: symbol.to_string(),
        timeframe: chart_tf,
        radar,
        dip,
        peak,
        detection,
        confidence_score,
        early_warning_score,
        recommendation: final_recommendation,
        confirmation_layers,
        direction,
        reference_price,
    }
}

fn build_detection_and_recommendation(
    radar: &Option<QRadarSignal>,
    dip: &Option<DipAnalysis>,
    peak: &Option<PeakAnalysis>,
) -> (String, f64, f64, String, String, Option<String>) {
    if let Some(ref r) = radar {
        let conf_10 = (r.confidence * 10.0).min(10.0);
        let side_long = matches!(r.side, SignalType::Buy | SignalType::ChochBuy | SignalType::BosBuy);
        let dir = if side_long { "LONG" } else { "SHORT" };
        let (detection, early_10, rec) = if side_long {
            let early = dip
                .as_ref()
                .map(|d| d.reversal_strength * 10.0)
                .unwrap_or(conf_10);
            let rec = if conf_10 >= 7.0 && early >= 7.0 {
                "GÜÇLÜ DİP – İzle"
            } else if conf_10 >= 4.0 || early >= 5.0 {
                "ZAYIF DİP – İzle"
            } else {
                "DİP BÖLGESİ – İzle"
            };
            ("DİP BÖLGESİ (TEPKİ DİBİ)".to_string(), early.min(10.0), rec)
        } else {
            let early = peak
                .as_ref()
                .map(|p| p.decline_strength * 10.0)
                .unwrap_or(conf_10);
            let rec = if conf_10 >= 7.0 && early >= 7.0 {
                "GÜÇLÜ TEPE – İzle"
            } else if conf_10 >= 4.0 || early >= 5.0 {
                "ZAYIF TEPE – İzle"
            } else {
                "TEPE BÖLGESİ – İzle"
            };
            ("TEPE BÖLGESİ (TEPKİ TEPESİ)".to_string(), early.min(10.0), rec)
        };
        let layers = if conf_10 >= 4.0 && early_10 >= 5.0 {
            Some(format!("{}/5 katman", (conf_10 as u32).min(5)))
        } else {
            None
        };
        return (detection, conf_10, early_10, rec.to_string(), dir.to_string(), layers);
    }

    if let Some(ref d) = dip {
        if d.reversal_detected && d.reversal_strength >= 0.5 {
            let early = d.reversal_strength * 10.0;
            let conf = (early * 0.4).min(10.0);
            let rec = if d.reversal_strength >= 0.7 {
                "GÜÇLÜ DİP – İzle"
            } else {
                "ZAYIF DİP – İzle"
            };
            return (
                "DİP BÖLGESİ (TEPKİ DİBİ)".to_string(),
                conf,
                early.min(10.0),
                rec.to_string(),
                "LONG".to_string(),
                None,
            );
        }
    }
    if let Some(ref p) = peak {
        if p.reversal_detected && p.decline_strength >= 0.5 {
            let early = p.decline_strength * 10.0;
            let conf = (early * 0.4).min(10.0);
            let rec = if p.decline_strength >= 0.7 {
                "GÜÇLÜ TEPE – İzle"
            } else {
                "ZAYIF TEPE – İzle"
            };
            return (
                "TEPE BÖLGESİ (TEPKİ TEPESİ)".to_string(),
                conf,
                early.min(10.0),
                rec.to_string(),
                "SHORT".to_string(),
                None,
            );
        }
    }
    (
        "—".to_string(),
        0.0,
        0.0,
        "—".to_string(),
        "—".to_string(),
        None,
    )
}
