//! Q-RADAR fırsat analizi – merkezi modül.
//!
//! Fırsat tespiti için detaylı analiz: RADAR sinyali, dip/tepe dönüş gücü,
//! tespit etiketi (DİP BÖLGESİ / TEPE BÖLGESİ), güven skoru, erken uyarı, tavsiye.
//! Çoklu doğrulama (MTF destek, yapı kırılımı, Elliott/Fib cluster, divergence)
//! ile daha doğru ve erken sinyaller. Hem robot hem web bu modülü çağırır.

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::dip_confluence::{compute_dip_confluence, DipConfluenceResult};
use crate::dip_tepe_scoring::compute_dip_tepe_score;
use crate::elliott_detector::compute_elliott;
use crate::reversal::{compute_reversal_analysis, DipAnalysis, PeakAnalysis};
use crate::signal::{CandleBuffer, SignalEngine};
use crate::types::{QRadarSignal, QSetup, SignalType, Timeframe};

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
    /// Madde 15: Sinyal bazlı skorlama (RSI +1, Support +2, … toplam 0–10). Tespit varken dolu.
    pub discrete_score: Option<crate::dip_tepe_scoring::DipTepeScore>,
    /// Smart Money Radar skoru (likidite, OB, FVG, Wyckoff, PO3). Tespit varken dolu olabilir.
    pub smart_money_score: Option<crate::smart_money::SmartMoneyRadarScore>,
    /// Confluence katmanları (MTF, divergence, RSI zone vb.). Tespit varken dolu.
    pub confluence: Option<DipConfluenceResult>,
    /// Aynı TF için Q-Setup (G05: RADAR ile hizalama).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q_setup: Option<QSetup>,
    /// RADAR/opp yönü ile Q-Setup: `1.0` uyum, `0.0` çelişki, `0.5` setup yok, `None` yön yok.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radar_setup_alignment: Option<f64>,
    /// Elliott W5 veya düzeltme hedefi (ikinci TP fiyatı).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elliott_secondary_tp: Option<f64>,
    /// Kısa Elliott özeti (`formation` / `formation_type`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elliott_summary: Option<String>,
    /// ABC / Zigzag düzeltme ipucu (LONG bias POC metni).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abc_correction_hint: Option<String>,
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
            let rev = compute_reversal_analysis(c, Some(config.pivot_length as usize), config);
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
    let mut discrete_score: Option<crate::dip_tepe_scoring::DipTepeScore> = None;
    let mut smart_money_score: Option<crate::smart_money::SmartMoneyRadarScore> = None;
    let mut confluence_out: Option<DipConfluenceResult> = None;
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
        confluence_out = Some(confluence.clone());
        let boost = (confluence.layers_passed as f64 * config.q_confluence_boost_per_layer)
            .min(config.q_confluence_boost_cap);
        confidence_score = (confidence_score + boost).min(10.0);
        early_warning_score = (early_warning_score + boost).min(10.0);
        confirmation_layers = Some(format!("{}/8 katman", confluence.layers_passed));
        if config.q_require_mtf_for_dip_zone && !confluence.mtf_support_near {
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
                discrete_score: None,
                smart_money_score: None,
                confluence: Some(confluence),
                q_setup: None,
                radar_setup_alignment: None,
                elliott_secondary_tp: None,
                elliott_summary: None,
                abc_correction_hint: None,
            };
        }
        // Nihai tavsiye aşağıda, tüm skorlar hesaplandıktan sonra belirlenir.
        // Madde 15: Sinyal bazlı skorlama (0–10)
        if let Some(candles_slice) = candles {
            let side = if is_long {
                SignalType::Buy
            } else {
                SignalType::Sell
            };
            let structure_score = engine.structure_score(
                candles_slice,
                side,
                config.pivot_length as usize,
            );
            discrete_score = Some(compute_dip_tepe_score(
                candles_slice,
                config,
                is_long,
                dip.as_ref(),
                peak.as_ref(),
                structure_score,
                confluence.mtf_support_near,
            ));
            // Smart Money Radar skoru – varsa SmartMoneyContext üzerinden hesaplanır.
            if let Some(ctx) = crate::smart_money::build_smart_money_context_for_series(
                symbol,
                chart_tf,
                candles_slice,
                config,
            ) {
                smart_money_score =
                    Some(crate::smart_money::compute_smart_money_radar_score(
                        candles_slice,
                        &ctx,
                        is_long,
                    ));
            }
        }
        final_recommendation = build_final_recommendation(
            is_long,
            confidence_score,
            early_warning_score,
            discrete_score.as_ref().map(|d| d.total),
            smart_money_score.as_ref().map(|s| s.total),
        );
    }

    let mut analysis = QRadarOpportunityAnalysis {
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
        discrete_score,
        smart_money_score,
        confluence: confluence_out,
        q_setup: None,
        radar_setup_alignment: None,
        elliott_secondary_tp: None,
        elliott_summary: None,
        abc_correction_hint: None,
    };
    enrich_opportunity_with_setup_elliott(
        &mut analysis,
        buffer,
        chart_tf,
        symbol,
        config,
        &engine,
    );
    analysis
}

/// RADAR/opp yönü (`LONG`/`SHORT`) ile Q-Setup yönü uyumu: `1.0` / `0.0` / `0.5` / `None`.
pub fn radar_setup_alignment_score(opp_dir: &str, setup: Option<&QSetup>) -> Option<f64> {
    if opp_dir == "—" {
        return None;
    }
    let Some(qs) = setup else {
        return Some(0.5);
    };
    let setup_long = matches!(qs.side, SignalType::Buy);
    let opp_long = opp_dir == "LONG";
    if (opp_long && setup_long) || (!opp_long && !setup_long) {
        Some(1.0)
    } else {
        Some(0.0)
    }
}

fn enrich_opportunity_with_setup_elliott(
    analysis: &mut QRadarOpportunityAnalysis,
    buffer: &CandleBuffer,
    chart_tf: Timeframe,
    symbol: &str,
    config: &Config,
    engine: &SignalEngine,
) {
    if !config.q_enrich_opportunity_with_setup_elliott {
        return;
    }
    let Some(candles) = buffer.get(chart_tf) else {
        return;
    };
    if candles.len() < (config.pivot_length as usize) * 4 + 50 {
        return;
    }
    let q_setup = engine.compute_q_setup(buffer, chart_tf, symbol, analysis.radar.as_ref());
    analysis.radar_setup_alignment = radar_setup_alignment_score(&analysis.direction, q_setup.as_ref());
    analysis.q_setup = q_setup;

    let elliott = compute_elliott(candles, config, false, Some(chart_tf), Some(symbol));
    if !elliott.formation.is_empty() && elliott.formation != "—" {
        analysis.elliott_summary = Some(format!("{} / {}", elliott.formation, elliott.formation_type));
    }
    if let Some((t1, _, _)) = elliott.w5_targets {
        analysis.elliott_secondary_tp = Some(t1);
    } else if let Some(ref cs) = elliott.corr_setup {
        analysis.elliott_secondary_tp = Some(cs.tp);
    }

    if analysis.direction == "LONG" {
        let ft = elliott.formation_type.to_lowercase();
        let fm = elliott.formation.to_lowercase();
        let zigzag = ft.contains("zigzag") || fm.contains("zigzag");
        if zigzag && elliott.corr_setup.is_some() {
            analysis.abc_correction_hint = Some("Zigzag ABC (LONG bias POC)".to_string());
        } else if ft.contains("flat") {
            analysis.abc_correction_hint = Some("Flat ABC düzeltme".to_string());
        }
    }

    if analysis.radar_setup_alignment == Some(0.0) && analysis.direction != "—" {
        analysis.confidence_score = (analysis.confidence_score - 2.0).max(0.0);
        analysis.recommendation = format!("ÇELİŞKİ (Q-Setup) – {}", analysis.recommendation);
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

fn build_final_recommendation(
    is_long: bool,
    confidence_score: f64,
    early_warning_score: f64,
    discrete_total: Option<u8>,
    sm_total: Option<u8>,
) -> String {
    let disc = discrete_total.unwrap_or(0);
    let sm = sm_total.unwrap_or(0);

    let strong_radar = confidence_score >= 7.0 && early_warning_score >= 7.0;
    let weak_radar = confidence_score >= 5.0 || early_warning_score >= 5.0;
    let strong_confirm = disc >= 6 && sm >= 4;
    let moderate_confirm = disc >= 4 || sm >= 4;

    if strong_radar && strong_confirm {
        if is_long {
            "GÜÇLÜ DİP – İzle".to_string()
        } else {
            "GÜÇLÜ TEPE – İzle".to_string()
        }
    } else if weak_radar && moderate_confirm {
        if is_long {
            "ZAYIF DİP – İzle".to_string()
        } else {
            "ZAYIF TEPE – İzle".to_string()
        }
    } else if strong_radar {
        if is_long {
            "TEYİTSİZ DİP ADAYI – İzle".to_string()
        } else {
            "TEYİTSİZ TEPE ADAYI – İzle".to_string()
        }
    } else if weak_radar {
        if is_long {
            "ZAYIF DİP – İzle".to_string()
        } else {
            "ZAYIF TEPE – İzle".to_string()
        }
    } else if is_long {
        "DİP BÖLGESİ – İzle".to_string()
    } else {
        "TEPE BÖLGESİ – İzle".to_string()
    }
}

#[cfg(test)]
mod radar_alignment_tests {
    use super::radar_setup_alignment_score;
    use crate::types::{QSetup, SignalType, Timeframe};

    fn dummy_setup(side: SignalType) -> QSetup {
        QSetup {
            symbol: "X".into(),
            timeframe: Timeframe::M5,
            side,
            entry: 1.0,
            entry_zone: (0.9, 1.1),
            stop_loss: 0.8,
            take_profit: 1.2,
            q_score: 50.0,
            time_window_bars: (5, 20),
            expected_bars: 10,
            radar_early: false,
        }
    }

    #[test]
    fn alignment_none_for_neutral_direction() {
        assert_eq!(radar_setup_alignment_score("—", None), None);
    }

    #[test]
    fn alignment_half_without_setup() {
        assert_eq!(radar_setup_alignment_score("LONG", None), Some(0.5));
    }

    #[test]
    fn alignment_match_long_buy() {
        assert_eq!(
            radar_setup_alignment_score("LONG", Some(&dummy_setup(SignalType::Buy))),
            Some(1.0)
        );
    }

    #[test]
    fn alignment_mismatch_long_sell() {
        assert_eq!(
            radar_setup_alignment_score("LONG", Some(&dummy_setup(SignalType::Sell))),
            Some(0.0)
        );
    }
}
