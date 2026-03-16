//! Strategy layer for Q-Analiz.
//!
//! This module does **not** perform heavy mathematical calculations itself.
//! Instead it composes the existing deterministic engines (Q-Setup, Elliott,
//! liquidity / Wyckoff style context) into higher level trade plans that can
//! be consumed by:
//! - backtest engine,
//! - auto trader daemons,
//! - web/API layers (for visualization and AI narration).

use serde::{Deserialize, Serialize};

use crate::classic_patterns::{detect_classic_patterns, ClassicPatternDetection, ClassicPatternKind};
use crate::config::Config;
use crate::elliott_detector::compute_elliott;
use crate::types::{Candle, QSetup, SignalType, Timeframe};
use crate::backtest::scan_historical_q_setups;
use crate::elliott_detector::ElliottDetectorResult;

/// Direction of a strategy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyDirection {
    Long,
    Short,
}

impl From<SignalType> for StrategyDirection {
    fn from(side: SignalType) -> Self {
        match side {
            SignalType::Buy => StrategyDirection::Long,
            SignalType::Sell => StrategyDirection::Short,
            _ => StrategyDirection::Long,
        }
    }
}

/// High level scenario label for a strategy (used by UI / AI narration).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyScenarioKind {
    /// Generic Q-Setup based plan (no specific pattern context).
    GenericQSetup,
    /// Corrective running triangle E-break based short/long.
    TriangleEBreak,
    /// Impulse wave 3 / 5 extension trade.
    ImpulseWave,
    /// Higher timeframe cup & handle style breakout.
    CupAndHandle,
    /// Any other custom / experimental scenario.
    Custom(String),
}

/// Role of a scenario within the global plan for a symbol/timeframe.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyRole {
    /// Main working hypothesis (ör. ana düşüş senaryosu).
    Primary,
    /// Karşıt / ikinci olası senaryo (ör. yukarı kırılım).
    Alternative,
    /// Daha yüksek zaman dilimi veya cycle perspektifli senaryo.
    Macro,
}

/// One concrete take-profit or intermediate objective within a strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyTarget {
    /// Target price level.
    pub price: f64,
    /// Optional textual label (e.g. "Short TP1", "W3 TP2").
    pub label: String,
    /// Target importance or priority (1 = primary).
    pub priority: u8,
}

/// High level trade plan built from Q-Analiz components.
///
/// This is the main object that will be:
/// - executed in backtests,
/// - followed by auto_trader,
/// - visualised in the web UI,
/// - narrated by AI layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPlan {
    /// Underlying symbol (e.g. ETHUSDT).
    pub symbol: String,
    /// Chart timeframe for this plan.
    pub timeframe: Timeframe,
    /// Long / Short.
    pub direction: StrategyDirection,
    /// Recommended entry price (single point).
    pub entry: f64,
    /// Entry zone suggested by Q-Setup, if any.
    pub entry_zone: Option<(f64, f64)>,
    /// Hard stop level (technical invalidation for the position).
    pub stop_loss: f64,
    /// Optional price level where the entire scenario is considered invalid,
    /// even if position has not been opened.
    pub invalidation: Option<f64>,
    /// List of profit targets (short term, wave targets, macro targets, ...).
    pub targets: Vec<StrategyTarget>,
    /// Q-Setup score that produced this plan (0-100).
    pub q_score: f64,
    /// Optional classic pattern context (e.g. Double Top, Cup&Handle).
    pub classic_pattern_kind: Option<ClassicPatternKind>,
    pub classic_pattern_label: Option<String>,
    /// Optional Elliott formation label (e.g. "Running Triangle").
    pub elliott_formation: Option<String>,
    /// Optional Elliott projections (e.g. W3/W5 targets condensed into text).
    pub elliott_summary: Option<String>,
    /// Scenario kind for grouping / narration.
    pub scenario_kind: StrategyScenarioKind,
    /// True if the plan is built with a preceding Q-RADAR context.
    pub has_radar_context: bool,
}

/// Group of one or more plans that describe a coherent scenario
/// (ana / alternatif / macro) for a given symbol + timeframe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyScenario {
    pub symbol: String,
    pub timeframe: Timeframe,
    pub role: StrategyRole,
    /// Short label for UI/AI ("Triangle E SHORT", "Upside Break", ...).
    pub label: String,
    /// Heuristic probability in [0, 1].
    pub probability: f64,
    /// Plans associated with this scenario (usually 1, ama gerekirse fazla).
    pub plans: Vec<StrategyPlan>,
}

impl StrategyPlan {
    /// Build a basic plan from a single `QSetup` without Elliott context.
    ///
    /// This is the minimal building block – higher level constructors can
    /// enrich it with Elliott / liquidity / Wyckoff annotations.
    pub fn from_q_setup(setup: &QSetup) -> Self {
        let direction = StrategyDirection::from(setup.side);
        let primary_target = StrategyTarget {
            price: setup.take_profit,
            label: "Q-Setup TP".to_string(),
            priority: 1,
        };

        StrategyPlan {
            symbol: setup.symbol.clone(),
            timeframe: setup.timeframe,
            direction,
            entry: setup.entry,
            entry_zone: Some(setup.entry_zone),
            stop_loss: setup.stop_loss,
            invalidation: None,
            targets: vec![primary_target],
            q_score: setup.q_score,
            classic_pattern_kind: None,
            classic_pattern_label: None,
            elliott_formation: None,
            elliott_summary: None,
            scenario_kind: StrategyScenarioKind::GenericQSetup,
            has_radar_context: setup.radar_early,
        }
    }

    /// Enrich an existing plan with Elliott detector information.
    ///
    /// This keeps all numeric levels computed by Q-Analiz core and only adds
    /// human-friendly metadata for UI / AI.
    pub fn with_elliott(mut self, ed: &ElliottDetectorResult) -> Self {
        if !ed.formation.is_empty() {
            self.elliott_formation = Some(ed.formation.clone());
        }

        // Use available W5 targets (if any) as additional long-term targets.
        if let Some((t1, t2, t3)) = ed.w5_targets {
            // Avoid duplicating very close levels to the existing TP.
            let mut extra = Vec::new();
            for (idx, level) in [t1, t2, t3].iter().enumerate() {
                if !level.is_finite() {
                    continue;
                }
                if (self.entry - *level).abs() < f64::EPSILON {
                    continue;
                }
                let label = format!("W5 TP{}", idx + 1);
                extra.push(StrategyTarget {
                    price: *level,
                    label,
                    priority: (idx + 2) as u8,
                });
            }
            self.targets.extend(extra);
        }

        // Compact textual summary for AI / UI consumption.
        let mut parts = Vec::new();
        if let Some((t1, t2, t3)) = ed.w5_targets {
            parts.push(format!("W5 targets: {:.2}, {:.2}, {:.2}", t1, t2, t3));
        }
        if let Some(projs) = &ed.projections {
            if !projs.is_empty() {
                let labels: Vec<String> = projs
                    .iter()
                    .take(3)
                    .map(|p| format!("{} @ {:.2}", p.label, p.price))
                    .collect();
                parts.push(format!("Projections: {}", labels.join(", ")));
            }
        }
        if !parts.is_empty() {
            self.elliott_summary = Some(parts.join(" | "));
        }

        // Map some common corrective setups to scenario kinds.
        if let Some(_corr) = &ed.corr_setup {
            // We don't depend on the exact variant names here; only the
            // high level fact that a corrective setup exists.
            self.scenario_kind = StrategyScenarioKind::TriangleEBreak;
        }

        self
    }

    /// Enrich an existing plan with a detected classic chart pattern.
    pub fn with_classic_pattern(mut self, pat: &ClassicPatternDetection) -> Self {
        self.classic_pattern_kind = Some(pat.kind.clone());
        self.classic_pattern_label = Some(pat.simple_label());
        // Merge pattern targets as lower-priority objectives, without
        // overriding Q-Setup / Elliott targets.
        let base_priority = self
            .targets
            .iter()
            .map(|t| t.priority)
            .max()
            .unwrap_or(1);
        for (idx, t) in pat.targets.iter().enumerate() {
            self.targets.push(StrategyTarget {
                price: t.price,
                label: t.label.clone(),
                priority: base_priority.saturating_add((idx + 1) as u8),
            });
        }
        self
    }
}

/// Build high level strategy plans for a given symbol/timeframe from raw candles.
///
/// This function:
/// - scans historical Q-Setups on the given candles,
/// - computes Elliott context on the same series,
/// - detects classic chart patterns,
/// - and merges all into a set of `StrategyPlan` objects suitable for
///   backtesting, auto trading or AI narration.
pub fn build_strategies_for_series(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
    config: &Config,
) -> Vec<StrategyPlan> {
    if candles.len() < 200 {
        return Vec::new();
    }

    // 1) Pre-compute Elliott and classic patterns once per series.
    let elliott = compute_elliott(candles, config, false);
    let classic_patterns = detect_classic_patterns(symbol, timeframe, candles, config);

    // For quick lookup: we use the detector result directly as context.
    let elliott_ctx = &elliott;
    let classic_ctx = classic_patterns.last();

    // 2) Scan Q-Setups over the whole history.
    let setups = scan_historical_q_setups(candles, config, timeframe, symbol);
    let mut plans = Vec::new();

    for (_bar_idx, setup) in setups {
        let mut plan = StrategyPlan::from_q_setup(&setup);
        plan = plan.with_elliott(elliott_ctx);
        if let Some(pat) = classic_ctx {
            plan = plan.with_classic_pattern(pat);
        }
        plans.push(plan);
    }

    plans
}

/// Build higher level scenarios (primary/alternative/macro) from raw candles.
///
/// This is a thin layer on top of `build_strategies_for_series` that:
/// - splits plans by direction (long/short),
/// - picks the strongest plan per direction as candidate scenarios,
/// - heuristically adds a macro scenario if suitable patterns are present.
pub fn build_scenarios_for_series(
    symbol: &str,
    timeframe: Timeframe,
    candles: &[Candle],
    config: &Config,
) -> Vec<StrategyScenario> {
    let plans = build_strategies_for_series(symbol, timeframe, candles, config);
    if plans.is_empty() {
        return Vec::new();
    }

    let mut longs: Vec<StrategyPlan> = plans
        .iter()
        .cloned()
        .filter(|p| matches!(p.direction, StrategyDirection::Long))
        .collect();
    let mut shorts: Vec<StrategyPlan> = plans
        .iter()
        .cloned()
        .filter(|p| matches!(p.direction, StrategyDirection::Short))
        .collect();

    longs.sort_by(|a, b| b.q_score.partial_cmp(&a.q_score).unwrap_or(std::cmp::Ordering::Equal));
    shorts.sort_by(|a, b| b.q_score.partial_cmp(&a.q_score).unwrap_or(std::cmp::Ordering::Equal));

    let mut scenarios = Vec::new();

    // 1) Primary scenario: tercih sırası short > long (düşüş ana senaryosu yaygın).
    if let Some(primary_plan) = shorts.first().cloned().or_else(|| longs.first().cloned()) {
        let label = match (&primary_plan.classic_pattern_label, &primary_plan.elliott_formation) {
            (Some(cp), _) => cp.clone(),
            (None, Some(ef)) => ef.clone(),
            _ => "Primary scenario".to_string(),
        };
        scenarios.push(StrategyScenario {
            symbol: symbol.to_string(),
            timeframe,
            role: StrategyRole::Primary,
            label,
            probability: 0.6,
            plans: vec![primary_plan.clone()],
        });

        // 2) Alternative: karşıt yöndeki en iyi plan.
        let alt_source = match primary_plan.direction {
            StrategyDirection::Long => &shorts,
            StrategyDirection::Short => &longs,
        };
        if let Some(alt_plan) = alt_source.first().cloned() {
            let label = match (&alt_plan.classic_pattern_label, &alt_plan.elliott_formation) {
                (Some(cp), _) => cp.clone(),
                (None, Some(ef)) => ef.clone(),
                _ => "Alternative scenario".to_string(),
            };
            scenarios.push(StrategyScenario {
                symbol: symbol.to_string(),
                timeframe,
                role: StrategyRole::Alternative,
                label,
                probability: 0.3,
                plans: vec![alt_plan],
            });
        }
    }

    // 3) Macro: Cup&Handle veya benzeri büyük yapı içeren planlardan biri.
    let macro_candidate = plans.iter().cloned().find(|p| {
        matches!(p.classic_pattern_kind, Some(ClassicPatternKind::CupAndHandle))
            || p
                .elliott_formation
                .as_deref()
                .map(|s| s.to_lowercase().contains("flat") || s.to_lowercase().contains("triangle"))
                .unwrap_or(false)
            || matches!(p.timeframe, Timeframe::H4 | Timeframe::D1)
    });

    if let Some(macro_plan) = macro_candidate {
        let label = macro_plan
            .classic_pattern_label
            .clone()
            .or(macro_plan.elliott_formation.clone())
            .unwrap_or_else(|| "Macro scenario".to_string());
        scenarios.push(StrategyScenario {
            symbol: symbol.to_string(),
            timeframe,
            role: StrategyRole::Macro,
            label,
            probability: 0.1,
            plans: vec![macro_plan],
        });
    }

    scenarios
}


