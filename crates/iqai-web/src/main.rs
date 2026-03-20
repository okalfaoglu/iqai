//! IQAI Web GUI - Smart Money Structure chart & panels

use anyhow::Result;
use axum::{
    extract::{Json, Query},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use iqai_binance::BinanceFuturesClient;
use iqai_tv::TvConnectorClient;
use iqai_web::{
    chart_data::{compute_annotations, scan_elliott_formations, ElliottOptions},
    notify::Notifier,
};
use iqai_core::{
    compute_q_radar_opportunity,
    detect_classic_patterns,
    exchange::ExchangeConnector,
    trade_db::{TradeDb, AnalysisSnapshotRecord},
    build_scenarios_for_series,
    build_smart_money_context_for_series,
    Config, CandleBuffer, SignalEngine, SignalType, Timeframe, TradingMode,
};
use serde::Deserialize;
use std::fs;
use std::net::SocketAddr;

#[derive(Debug, Deserialize)]
struct ChartParams {
    symbol: Option<String>,
    market: Option<String>,
    /// TradingView borsa kodu (market=tv iken: BINANCE, BIST, NASDAQ vb.)
    exchange: Option<String>,
    #[serde(rename = "tf")]
    timeframe: Option<String>,
    /// Invert Pattern: downtrend'de Impulse, uptrend'de Zigzag ara (?invert=1)
    invert: Option<String>,
    /// Poz Koruma için giriş fiyatı (?entry=...)
    entry: Option<String>,
    /// Poz Koruma için stop loss (?sl=...)
    sl: Option<String>,
    /// ?debug=1 → chart_bars ve klasik pattern sayısı (prod teşhisi)
    debug: Option<String>,
    /// Aynı amaç (?chart_debug=1) — bazı proxy'ler `debug` parametresini keser
    #[serde(rename = "chart_debug")]
    chart_debug: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MetricsParams {
    symbol: Option<String>,
    #[serde(rename = "tf")]
    timeframe: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FormationsParams {
    symbol: Option<String>,
    market: Option<String>,
    exchange: Option<String>,
    #[serde(rename = "tf")]
    timeframe: Option<String>,
    limit: Option<u32>,
}

/// Tüm config.json içeriğini okuyan API çıktısı için tip ipucu.
type AppConfig = iqai_core::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let app_cfg = iqai_core::AppConfig::load();
    iqai_core::init_from_config(app_cfg.as_ref().and_then(|c| c.logging.as_ref()))
        .expect("Loglama başlatılamadı");
    let app = Router::new()
        .route("/", get(index))
        .route("/settings", get(settings_page))
        .route("/metrics", get(metrics_page))
        .route("/pnl", get(pnl_page))
        .route("/q-analiz", get(q_analiz_page))
        .route("/snapshots", get(snapshots_page))
        .route("/api/chart", get(api_chart))
        .route("/api/metrics", get(api_metrics))
        .route("/api/formations", get(api_formations))
        .route("/api/pnl/symbols", get(api_pnl_symbols))
        .route("/api/q-analysis", get(api_q_analysis_all))
        .route("/api/q-analiz/detections", get(api_q_analiz_detections))
        .route("/api/q-analiz/snapshot", get(api_q_analiz_snapshot))
        .route("/api/analysis-snapshots", get(api_analysis_snapshots))
        .route("/api/analysis-snapshots/report", get(api_analysis_snapshots_report))
        .route(
            "/api/config",
            get(api_get_config).post(api_update_config),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("IQAI Web GUI: http://localhost:8080");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

async fn index() -> impl IntoResponse {
    Html(include_str!("index.html"))
}

async fn metrics_page() -> impl IntoResponse {
    Html(include_str!("metrics.html"))
}

async fn settings_page() -> impl IntoResponse {
    Html(include_str!("settings.html"))
}

async fn pnl_page() -> impl IntoResponse {
    Html(include_str!("pnl.html"))
}

async fn q_analiz_page() -> impl IntoResponse {
    Html(include_str!("q_analiz.html"))
}

async fn snapshots_page() -> impl IntoResponse {
    Html(include_str!("snapshots.html"))
}

#[derive(Debug, Deserialize)]
struct PnlQuery {
    mode: Option<String>,
}

async fn api_pnl_symbols(Query(params): Query<PnlQuery>) -> impl IntoResponse {
    let mode = params
        .mode
        .as_deref()
        .map(TradingMode::from_str)
        .unwrap_or(TradingMode::Paper);
    let path = match iqai_core::AppConfig::load()
        .and_then(|c| c.trading.and_then(|t| t.db_path))
    {
        Some(p) => p,
        None => return axum::Json(serde_json::json!({ "error": "trading.db_path not set", "symbols": [] })),
    };
    let db = match TradeDb::open(Some(path.as_str())) {
        Ok(d) => d,
        Err(e) => {
            return axum::Json(
                serde_json::json!({ "error": e.to_string(), "symbols": [] }),
            );
        }
    };
    match db.get_symbol_pnl_stats(mode) {
        Ok(stats) => axum::Json(serde_json::json!({ "symbols": stats, "mode": mode.to_string() })),
        Err(e) => axum::Json(
            serde_json::json!({ "error": e.to_string(), "symbols": [] }),
        ),
    }
}

/// İzlenen tüm semboller için Q-Analiz sonuçları (config.json trading.symbols + timeframes).
/// Piyasa: Binance Futures. GET /api/q-analysis
async fn api_q_analysis_all() -> impl IntoResponse {
    let app_cfg = iqai_core::AppConfig::load().unwrap_or_default();
    let symbols: Vec<String> = app_cfg
        .trading
        .as_ref()
        .and_then(|t| t.symbols.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| vec!["ETHUSDT".into(), "BTCUSDT".into()]);
    let tf_strings: Vec<String> = app_cfg
        .trading
        .as_ref()
        .and_then(|t| t.timeframes.clone())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| vec!["5m".into(), "15m".into(), "1h".into(), "4h".into()]);
    let timeframes: Vec<Timeframe> = tf_strings
        .iter()
        .filter_map(|s| Timeframe::from_str(s))
        .collect::<Vec<_>>();
    if timeframes.is_empty() {
        return axum::Json(serde_json::json!({
            "error": "Geçerli timeframe yok",
            "results": []
        }));
    }
    let sm_cfg = app_cfg.smart_money.as_ref();
    let config = Config::from_smart_money(sm_cfg);
    let client = BinanceFuturesClient::new();
    let max_bars = app_cfg.data.and_then(|d| d.max_bars).unwrap_or(500);
    let mut results = Vec::new();
    for symbol in &symbols {
        let mut buffer = CandleBuffer::new();
        for &tf in &timeframes {
            match client.fetch_klines(symbol, tf, max_bars).await {
                Ok(candles) if !candles.is_empty() => buffer.update(tf, candles),
                _ => {}
            }
        }
        for &tf in &timeframes {
            let opportunity = compute_q_radar_opportunity(&buffer, tf, symbol, &config);
            results.push(serde_json::json!({
                "symbol": opportunity.symbol,
                "timeframe": opportunity.timeframe,
                "detection": opportunity.detection,
                "direction": opportunity.direction,
                "confidence_score": opportunity.confidence_score,
                "early_warning_score": opportunity.early_warning_score,
                "recommendation": opportunity.recommendation,
                "reference_price": opportunity.reference_price,
                "confirmation_layers": opportunity.confirmation_layers,
                "radar": opportunity.radar,
            }));
        }
    }
    axum::Json(serde_json::json!({ "results": results }))
}

#[derive(Debug, Deserialize)]
struct DetectionsQuery {
    limit: Option<u32>,
    symbol: Option<String>,
}

/// DB'deki Q-Analiz tespit kayıtları (daemon tarafından yazılan). GET /api/q-analiz/detections?limit=50&symbol=ETHUSDT
async fn api_q_analiz_detections(Query(params): Query<DetectionsQuery>) -> impl IntoResponse {
    let path = match iqai_core::AppConfig::load().and_then(|c| c.trading.and_then(|t| t.db_path.clone())) {
        Some(p) => p,
        None => return axum::Json(serde_json::json!({ "error": "trading.db_path not set", "detections": [] })),
    };
    let db = match TradeDb::open(Some(path.as_str())) {
        Ok(d) => d,
        Err(e) => return axum::Json(serde_json::json!({ "error": e.to_string(), "detections": [] })),
    };
    let limit = params.limit.unwrap_or(100);
    let symbol = params.symbol.as_deref();
    match db.get_q_analiz_detections(limit, symbol) {
        Ok(detections) => axum::Json(serde_json::json!({ "detections": detections })),
        Err(e) => axum::Json(serde_json::json!({ "error": e.to_string(), "detections": [] })),
    }
}

#[derive(Debug, Deserialize)]
struct SnapshotQuery {
    symbol: Option<String>,
    #[serde(rename = "tf")]
    timeframe: Option<String>,
}

/// Tek sembol + timeframe için Q-Analiz snapshot'ı:
/// - Q-Radar opportunity
/// - Strategy scenarios (primary/alternative/macro)
/// - Smart Money context (likidite, OB, Wyckoff, PO3)
/// GET /api/q-analiz/snapshot?symbol=ETHUSDT&tf=5m
async fn api_q_analiz_snapshot(Query(params): Query<SnapshotQuery>) -> impl IntoResponse {
    let app_cfg = iqai_core::AppConfig::load().unwrap_or_default();
    let symbol = params
        .symbol
        .or_else(|| {
            app_cfg
                .trading
                .as_ref()
                .and_then(|t| t.symbols.as_ref())
                .and_then(|v| v.first().cloned())
        })
        .unwrap_or_else(|| "ETHUSDT".to_string());
    let tf_str = params
        .timeframe
        .or_else(|| {
            app_cfg
                .trading
                .as_ref()
                .and_then(|t| t.timeframes.as_ref())
                .and_then(|v| v.first().cloned())
        })
        .unwrap_or_else(|| "5m".to_string());
    let tf = match Timeframe::from_str(&tf_str) {
        Some(t) => t,
        None => {
            return axum::Json(serde_json::json!({
                "error": format!("Geçersiz timeframe: {}", tf_str),
            }));
        }
    };

    let sm_cfg = app_cfg.smart_money.as_ref();
    let config = Config::from_smart_money(sm_cfg);
    let client = BinanceFuturesClient::new();
    let max_bars = app_cfg.data.and_then(|d| d.max_bars).unwrap_or(500);

    let candles = match client.fetch_klines(&symbol, tf, max_bars).await {
        Ok(c) if !c.is_empty() => c,
        _ => {
            return axum::Json(serde_json::json!({
                "error": "Veri alınamadı",
                "symbol": symbol,
                "timeframe": tf.to_binance_interval(),
            }));
        }
    };

    let mut buffer = CandleBuffer::new();
    buffer.update(tf, candles.clone());
    let opportunity = compute_q_radar_opportunity(&buffer, tf, &symbol, &config);
    let scenarios = build_scenarios_for_series(&symbol, tf, &candles, &config);
    let sm_ctx = build_smart_money_context_for_series(&symbol, tf, &candles, &config);

    axum::Json(serde_json::json!({
        "symbol": symbol,
        "timeframe": tf.to_binance_interval(),
        "opportunity": opportunity,
        "scenarios": scenarios,
        "smart_money": sm_ctx,
    }))
}

#[derive(Debug, Deserialize)]
struct AnalysisSnapshotsQuery {
    symbol: Option<String>,
}

/// DB'deki analiz snapshot'ları (daemon'un yazdığı). GET /api/analysis-snapshots?symbol=BTCUSDT
async fn api_analysis_snapshots(Query(params): Query<AnalysisSnapshotsQuery>) -> impl IntoResponse {
    let path = match iqai_core::AppConfig::load().and_then(|c| c.trading.and_then(|t| t.db_path.clone())) {
        Some(p) => p,
        None => return axum::Json(serde_json::json!({ "error": "trading.db_path not set", "snapshots": [] })),
    };
    let db = match TradeDb::open(Some(path.as_str())) {
        Ok(d) => d,
        Err(e) => return axum::Json(serde_json::json!({ "error": e.to_string(), "snapshots": [] })),
    };
    let symbol = params.symbol.as_deref();
    match db.get_analysis_snapshots(symbol) {
        Ok(snapshots) => axum::Json(serde_json::json!({ "snapshots": snapshots })),
        Err(e) => axum::Json(serde_json::json!({ "error": e.to_string(), "snapshots": [] })),
    }
}

/// Bir sembol için tüm TF snapshot'larından AI büyük resim raporu. GET /api/analysis-snapshots/report?symbol=BTCUSDT
async fn api_analysis_snapshots_report(Query(params): Query<AnalysisSnapshotsQuery>) -> impl IntoResponse {
    let path = match iqai_core::AppConfig::load().and_then(|c| c.trading.and_then(|t| t.db_path.clone())) {
        Some(p) => p,
        None => return axum::Json(serde_json::json!({ "error": "trading.db_path not set", "report": null })),
    };
    let symbol = params.symbol.as_deref().unwrap_or("ETHUSDT");
    let db = match TradeDb::open(Some(path.as_str())) {
        Ok(d) => d,
        Err(e) => return axum::Json(serde_json::json!({ "error": e.to_string(), "report": null })),
    };
    let snapshots: Vec<AnalysisSnapshotRecord> = match db.get_analysis_snapshots(Some(symbol)) {
        Ok(s) => s,
        Err(e) => return axum::Json(serde_json::json!({ "error": e.to_string(), "report": null })),
    };
    if snapshots.is_empty() {
        return axum::Json(serde_json::json!({
            "symbol": symbol,
            "report": "Bu sembol için henüz snapshot yok. Q-Analiz daemon çalıştırılıyor olmalı.",
            "ai": null,
        }));
    }
    let context = format_snapshots_for_ai(symbol, &snapshots);
    let app_cfg = iqai_core::AppConfig::load().unwrap_or_default();
    let ai_text = if let Some(ref ai) = app_cfg.ai {
        if ai.enabled == Some(true) {
            let base = ai.ollama_base_url.as_deref().unwrap_or("http://localhost:11434");
            let model = ai.model.as_deref().unwrap_or("llama2");
            iqai_web::ai::interpret_big_picture(base, model, symbol, &context).await
        } else {
            None
        }
    } else {
        None
    };
    axum::Json(serde_json::json!({
        "symbol": symbol,
        "snapshot_count": snapshots.len(),
        "report": context,
        "ai": ai_text,
    }))
}

fn format_snapshots_for_ai(symbol: &str, snapshots: &[AnalysisSnapshotRecord]) -> String {
    let mut lines = vec![format!("Sembol: {} | {} timeframe özeti:\n", symbol, snapshots.len())];
    for s in snapshots {
        lines.push(format!(
            "[{}] Tespit: {} | Yön: {} | Tavsiye: {} | Güven: {:.1}/10 | Erken: {:.1}/10 | Fiyat: {:.4}",
            s.timeframe,
            s.detection,
            s.direction,
            s.recommendation,
            s.confidence_score,
            s.early_warning_score,
            s.reference_price,
        ));

        // Pozisyon metrikleri (tek tablo: analysis_snapshots)
        let strength = s.tmr_strength_points.unwrap_or(0);
        let trend_pts = s.tmr_trend_points.unwrap_or(0);
        let mom_pts = s.tmr_momentum_points.unwrap_or(0);
        let rr_pts = s.tmr_rr_points.unwrap_or(0);
        if s.position_state.is_some()
            || s.market_mode.is_some()
            || s.local_trend.is_some()
            || s.global_trend.is_some()
            || s.volatility_pct.is_some()
            || s.rr.is_some()
            || strength > 0
        {
            lines.push(format!(
                "  Metrikler: Poz={} | Rejim={} | Trend L/G={}/{} | Güç {}/10 (T/M/RR={}/{}/{}) | Vol={:.2}% | Mom S/L={:.2}%/{:.2}% | RR={:.2}",
                s.position_state.as_deref().unwrap_or("—"),
                s.market_mode.as_deref().unwrap_or("—"),
                s.local_trend.unwrap_or(0),
                s.global_trend.unwrap_or(0),
                strength,
                trend_pts,
                mom_pts,
                rr_pts,
                s.volatility_pct.unwrap_or(0.0),
                s.momentum_short.unwrap_or(0.0) * 100.0,
                s.momentum_long.unwrap_or(0.0) * 100.0,
                s.rr.unwrap_or(0.0),
            ));
            let mut flags = Vec::new();
            if s.trend_exhaustion.unwrap_or(false) {
                flags.push("trend_tükenmesi");
            }
            if s.structure_shift.unwrap_or(false) {
                flags.push("structure_shift");
            }
            if !flags.is_empty() {
                lines.push(format!("  Uyarılar: {}", flags.join(", ")));
            }
        }

        if let Some(ew) = s.elliott_formation.as_deref().filter(|x| !x.is_empty()) {
            lines.push(format!("  Elliott: {} ({})", ew, s.elliott_type.as_deref().unwrap_or("—")));
        }
        if let Some(cp) = s.classic_pattern.as_deref().filter(|x| !x.is_empty()) {
            lines.push(format!("  Klasik formasyon: {}", cp));
        }
        if let (Some(entry), Some(sl), Some(tp)) = (s.scenario_entry, s.scenario_stop, s.scenario_tp1) {
            lines.push(format!("  Strateji: Entry {:.2} | SL {:.2} | TP1 {:.2}", entry, sl, tp));
        }
        if let Some(po3) = s.po3_phase.as_deref().filter(|x| !x.is_empty()) {
            lines.push(format!("  PO3: {}", po3));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

async fn api_get_config() -> impl IntoResponse {
    let cfg = iqai_core::AppConfig::load().unwrap_or_default();
    axum::Json(cfg)
}

async fn api_update_config(Json(new_cfg): Json<AppConfig>) -> impl IntoResponse {
    let path_opt = iqai_core::AppConfig::config_path();
    if let Some(path) = path_opt {
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Config dir create error: {}", e);
            }
        }
        match serde_json::to_string_pretty(&new_cfg) {
            Ok(contents) => {
                if let Err(e) = fs::write(&path, contents) {
                    eprintln!("Config write error: {}", e);
                    return axum::Json(serde_json::json!({ "ok": false, "error": e.to_string() }));
                }
                axum::Json(serde_json::json!({ "ok": true }))
            }
            Err(e) => {
                eprintln!("Config serialize error: {}", e);
                axum::Json(serde_json::json!({ "ok": false, "error": e.to_string() }))
            }
        }
    } else {
        axum::Json(serde_json::json!({ "ok": false, "error": "Config path not found" }))
    }
}

/// TV connector varsayılan semboller: borsaya göre (BIST, NASDAQ, BINANCE).
fn tv_default_symbol(exchange: &str) -> &'static str {
    match exchange.to_uppercase().as_str() {
        "BIST" => "XU100",       // Borsa İstanbul 100 endeksi
        "NASDAQ" => "AAPL",      // Örnek hisse; kullanıcı NDX, MSFT vb. yazabilir
        _ => "ETHUSDT27H2026",   // BINANCE / kripto
    }
}

/// Chart JSON şeması / teşhis alanları değişince artır (curl ile binary doğrulama).
const CHART_API_REV: u32 = 2;

async fn api_chart(Query(params): Query<ChartParams>) -> impl IntoResponse {
    let market = params.market.as_deref().unwrap_or("futures");
    let tv_exchange_param = params.exchange.as_deref().unwrap_or("BINANCE");
    let symbol = params.symbol.as_deref().unwrap_or_else(|| {
        if market.eq_ignore_ascii_case("tv") {
            tv_default_symbol(tv_exchange_param)
        } else {
            "ETHUSDT"
        }
    });
    let tf_str = params.timeframe.as_deref().unwrap_or("5M");
    let chart_tf = Timeframe::from_str(tf_str).unwrap_or(Timeframe::M5);

    // Her TF için çekilecek max bar (config.json "data.max_bars" veya 10_000).
    let max_bars = iqai_core::AppConfig::load()
        .and_then(|c| c.data.and_then(|d| d.max_bars))
        .unwrap_or(10_000);
    let mut buffer = CandleBuffer::new();

    let use_tv = market.eq_ignore_ascii_case("tv");
    let tv_exchange = params.exchange.as_deref().unwrap_or("BINANCE");
    let tv_script = std::env::var("TV_CONNECTOR_SCRIPT").ok().filter(|s| !s.is_empty());
    let tv_url = std::env::var("TV_CONNECTOR_URL").ok().filter(|s| !s.is_empty());

    let timeframes = [
        Timeframe::M1,
        Timeframe::M5,
        Timeframe::M15,
        Timeframe::M30,
        Timeframe::H1,
        Timeframe::H4,
        Timeframe::D1,
    ];

    if use_tv {
        let client = if let Some(script) = tv_script {
            eprintln!("[TV] Backend: Subprocess (Python) → {}", script);
            let python = std::env::var("TV_CONNECTOR_PYTHON").unwrap_or_else(|_| "python3".to_string());
            TvConnectorClient::subprocess(python, script, tv_exchange)
        } else if let Some(base) = tv_url {
            eprintln!("[TV] Backend: HTTP → {}", base);
            TvConnectorClient::with_exchange(base, tv_exchange)
        } else {
            eprintln!("[TV] Backend: Auto (tradingview-rs varsa onu, yoksa Native Rust WebSocket)");
            TvConnectorClient::auto(tv_exchange)
        };
        for tf in timeframes {
            match client.fetch_klines(symbol, tf, max_bars).await {
                Ok(candles) if !candles.is_empty() => {
                    buffer.update(tf, candles);
                }
                Ok(_) => eprintln!("[TV] {} {:?}: 0 bar (boş)", symbol, tf),
                Err(e) => eprintln!("[TV] {} {:?} hatası: {}", symbol, tf, e),
            }
        }
    } else {
        // Binance: her TF native çekilir (TV gibi)
        let client = BinanceFuturesClient::new();
        for tf in timeframes {
            match client.fetch_klines(symbol, tf, max_bars).await {
                Ok(candles) if !candles.is_empty() => buffer.update(tf, candles),
                Ok(_) => eprintln!("[Binance] {} {:?}: 0 bar (boş)", symbol, tf),
                Err(e) => eprintln!("[Binance] {} {:?} hatası: {}", symbol, tf, e),
            }
        }
    }

    let app_cfg = iqai_core::AppConfig::load();
    let sm_cfg = app_cfg.as_ref().and_then(|c| c.smart_money.as_ref());
    let config = Config::from_smart_money(sm_cfg);
    let mut engine = SignalEngine::new(config.clone());
    let signals = engine.process(&buffer, chart_tf);
    let trend_strength = engine.trend_strength(&buffer);
    let confidence = engine.system_confidence(&buffer);

    // Multi-TF trends
    let timeframes = ["1M", "5M", "15M", "30M", "1H", "4H", "1D"];
    let mut trends = vec![];
    for tf in [Timeframe::M1, Timeframe::M5, Timeframe::M15, Timeframe::M30, Timeframe::H1, Timeframe::H4, Timeframe::D1] {
        let t = engine.trend_for_tf(&buffer, tf);
        trends.push(match t {
            1 => "▲",
            -1 => "▼",
            _ => "━",
        });
    }

    let chart_candles: Vec<_> = buffer
        .get(chart_tf)
        .map(|s| s.to_vec())
        .unwrap_or_default();
    let n = chart_candles.len();
    let candle_data: Vec<_> = chart_candles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let is_last = i == n.saturating_sub(1);
            let is_current = !use_tv && is_last && n > 0;
            serde_json::json!({
                "time": c.time / 1000,
                "open": c.open,
                "high": c.high,
                "low": c.low,
                "close": c.close,
                "volume": c.volume,
                "is_current": is_current
            })
        })
        .collect();

    // Anlık fiyat (Binance); TV veya hata durumunda None
    let last_price: Option<f64> = if !use_tv {
        let client = BinanceFuturesClient::new();
        client.fetch_ticker_price(symbol).await.ok()
    } else {
        None
    };

    let signal_data: Vec<_> = signals
        .iter()
        .map(|s| {
            let label = match s.signal_type {
                SignalType::Buy => "🟢 BUY",
                SignalType::Sell => "🔴 SELL",
                SignalType::GetReadyBuy => "⚠ READY",
                SignalType::GetReadySell => "⚠ READY",
                _ => "",
            };
            serde_json::json!({
                "time": s.timestamp / 1000,
                "price": s.price,
                "label": label,
                "type": format!("{:?}", s.signal_type),
                "tp": s.take_profit,
                "sl": s.stop_loss
            })
        })
        .collect();

    let chart_volume: f64 = chart_candles.iter().map(|c| c.volume).sum();

    let invert = params
        .invert
        .as_deref()
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let elliott_opts = ElliottOptions { invert };
    let annotations = compute_annotations(&chart_candles, &config, Some(&elliott_opts));
    let formations = scan_elliott_formations(&chart_candles, &config);
    let classic_patterns = detect_classic_patterns(symbol, chart_tf, &chart_candles, &config);
    let classic_patterns_count = classic_patterns.len();
    let truthy = |s: &str| s == "1" || s.eq_ignore_ascii_case("true");
    let chart_debug = params
        .debug
        .as_deref()
        .map(truthy)
        .unwrap_or(false)
        || params
            .chart_debug
            .as_deref()
            .map(truthy)
            .unwrap_or(false);

    // Merkezi Q-RADAR fırsat analizi (robot + web aynı modül)
    let q_radar_opportunity = compute_q_radar_opportunity(&buffer, chart_tf, symbol, &config);
    let q_setup = engine.compute_q_setup(&buffer, chart_tf, symbol, q_radar_opportunity.radar.as_ref());

    let notifier = Notifier::from_env();
    if let Some(ref setup) = q_setup {
        let setup = setup.clone();
        let current_price = chart_candles.last().map(|c| c.close);
        tokio::spawn(async move {
            if let Err(e) = notifier.notify_q_setup_with_price(&setup, current_price).await {
                eprintln!("Q-setup notification error: {:?}", e);
            }
        });
    }
    if let Some(radar) = q_radar_opportunity.radar.clone() {
        let notifier = Notifier::from_env();
        tokio::spawn(async move {
            if let Err(e) = notifier.notify_q_radar(&radar).await {
                eprintln!("Q-radar notification error: {:?}", e);
            }
        });
    }
    // Q-Analiz tam panel (ekranla 1-1 aynı düzen) – Tespit varsa tüm kanallara gönder
    if q_radar_opportunity.detection != "—" && !q_radar_opportunity.detection.is_empty() {
        let opp = q_radar_opportunity.clone();
        let notifier = Notifier::from_env();
        tokio::spawn(async move {
            if let Err(e) = notifier.notify_q_analysis(&opp, None).await {
                eprintln!("Q-analysis notification error: {:?}", e);
            }
        });
    }

    // Poz Koruma: ?entry=...&sl=... verilmişse hesapla, JSON'a ekle ve bildir
    let protect_signal = params
        .entry
        .as_ref()
        .and_then(|e| e.parse::<f64>().ok())
        .zip(params.sl.as_ref().and_then(|s| s.parse::<f64>().ok()))
        .and_then(|(entry, sl)| {
            engine.compute_protect_signal(&buffer, chart_tf, symbol, entry, sl)
        });
    if let Some(ref protect) = protect_signal {
        let protect = protect.clone();
        let notifier = Notifier::from_env();
        tokio::spawn(async move {
            if let Err(e) = notifier.notify_protect(&protect).await {
                eprintln!("Protect notification error: {:?}", e);
            }
        });
    }

    // Position-level metrics (shared T/D/Q view) – uses optional Q-Setup / external position info.
    let side_enum = params
        .entry
        .as_ref()
        .and(params.sl.as_ref())
        .map(|_| {
            // If user passes ?entry&sl but no explicit side, infer from entry vs SL.
            // Long: entry > sl, Short: entry < sl.
            if let (Ok(entry), Ok(sl)) =
                (params.entry.as_deref().unwrap_or("").parse::<f64>(), params.sl.as_deref().unwrap_or("").parse::<f64>())
            {
                if entry > sl {
                    Some(SignalType::Buy)
                } else if entry < sl {
                    Some(SignalType::Sell)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .flatten();
    let metrics = engine.compute_position_metrics(
        &buffer,
        chart_tf,
        symbol,
        side_enum,
        params
            .entry
            .as_ref()
            .and_then(|e| e.parse::<f64>().ok())
            .or_else(|| q_setup.as_ref().map(|q| q.entry)),
        params
            .sl
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| q_setup.as_ref().map(|q| q.stop_loss)),
        q_setup.as_ref().map(|q| q.take_profit),
    );

    let mut response = serde_json::json!({
        "symbol": symbol,
        "market": market,
        "server_meta": {
            "iqai_web_version": env!("CARGO_PKG_VERSION"),
            "chart_api_rev": CHART_API_REV,
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" }
        },
        "exchange": if use_tv { Some(tv_exchange) } else { None::<&str> },
        "timeframe": tf_str,
        "last_price": last_price,
        "candles": candle_data,
        "signals": signal_data,
        "trend": {
            "strength": trend_strength,
            "confidence": confidence,
            "timeframes": timeframes,
            "trends": trends,
            "volume": chart_volume,
            "cvd": annotations.cvd
        },
        "q_setup": q_setup,
        "q_radar": q_radar_opportunity.radar,
        "q_radar_opportunity": q_radar_opportunity,
        "protect_signal": protect_signal,
        "position_metrics": metrics,
        "annotations": {
            "choch": annotations.choch,
            "bos": annotations.bos,
            "liquidity": annotations.liquidity,
            "market_profile": annotations.market_profile,
            "divergence": annotations.divergence,
            "support_line": annotations.support_line,
            "resistance_line": annotations.resistance_line,
            "elliott": annotations.elliott,
            "zigzag": annotations.zigzag,
            "classic_patterns": classic_patterns
        },
        "formations": formations
    });
    if chart_debug {
        if let Some(obj) = response.as_object_mut() {
            obj.insert(
                "debug".to_string(),
                serde_json::json!({
                    "chart_bars": chart_candles.len(),
                    "classic_patterns_count": classic_patterns_count,
                }),
            );
        }
    }
    axum::Json(response)
}

/// Metrikleri analysis_snapshots tablosundan döndürür (tek tablo: analiz + pozisyon metrikleri).
async fn api_metrics(Query(params): Query<MetricsParams>) -> impl IntoResponse {
    let symbol = params.symbol.as_deref().unwrap_or("ETHUSDT").to_string();
    let tf_param = params.timeframe.as_deref().unwrap_or("5m");
    let tf_lower = tf_param.to_lowercase();
    let path = iqai_core::AppConfig::load()
        .and_then(|c| c.trading.and_then(|t| t.db_path))
        .unwrap_or_else(|| "data/trades.db".to_string());
    let db = match TradeDb::open(Some(path.as_str())) {
        Ok(d) => d,
        Err(_) => {
            return axum::Json(serde_json::json!({
                "symbol": symbol,
                "timeframe": tf_param,
                "position_metrics": serde_json::Value::Null
            }));
        }
    };
    let snapshots = match db.get_analysis_snapshots(Some(&symbol)) {
        Ok(s) => s,
        Err(_) => {
            return axum::Json(serde_json::json!({
                "symbol": symbol,
                "timeframe": tf_param,
                "position_metrics": serde_json::Value::Null
            }));
        }
    };
    let record = snapshots
        .into_iter()
        .find(|r| r.timeframe.to_lowercase() == tf_lower);
    let position_metrics = record.map(|r| {
        serde_json::json!({
            "symbol": r.symbol,
            "timeframe": r.timeframe,
            "side": r.position_side,
            "position_state": r.position_state.unwrap_or_else(|| "Flat".to_string()),
            "market_mode": r.market_mode.unwrap_or_else(|| "—".to_string()),
            "local_trend": r.local_trend.unwrap_or(0),
            "global_trend": r.global_trend.unwrap_or(0),
            "volatility_pct": r.volatility_pct.unwrap_or(0.0),
            "momentum_short": r.momentum_short.unwrap_or(0.0),
            "momentum_long": r.momentum_long.unwrap_or(0.0),
            "entry_price": r.scenario_entry,
            "stop_loss_initial": r.scenario_stop,
            "take_profit_initial": r.scenario_tp1,
            "rr": r.rr.unwrap_or(0.0),
            "tmr_scores": {
                "trend_points": r.tmr_trend_points.unwrap_or(0),
                "momentum_points": r.tmr_momentum_points.unwrap_or(0),
                "rr_points": r.tmr_rr_points.unwrap_or(0),
                "strength_points": r.tmr_strength_points.unwrap_or(0)
            },
            "trend_exhaustion": r.trend_exhaustion.unwrap_or(false),
            "structure_shift": r.structure_shift.unwrap_or(false)
        })
    });
    axum::Json(serde_json::json!({
        "symbol": symbol,
        "timeframe": tf_param,
        "position_metrics": position_metrics
    }))
}

async fn api_formations(Query(params): Query<FormationsParams>) -> impl IntoResponse {
    let market = params.market.as_deref().unwrap_or("futures");
    let tv_exchange_param = params.exchange.as_deref().unwrap_or("BINANCE");
    let symbol = params.symbol.as_deref().unwrap_or_else(|| {
        if market.eq_ignore_ascii_case("tv") {
            tv_default_symbol(tv_exchange_param)
        } else {
            "ETHUSDT"
        }
    });
    let tf_str = params.timeframe.as_deref().unwrap_or("15M");
    let limit = params.limit.unwrap_or(500);
    let chart_tf = Timeframe::from_str(tf_str).unwrap_or(Timeframe::M15);

    let use_tv = market.eq_ignore_ascii_case("tv");
    let tv_exchange = params.exchange.as_deref().unwrap_or("BINANCE");
    let tv_script = std::env::var("TV_CONNECTOR_SCRIPT").ok().filter(|s| !s.is_empty());
    let tv_url = std::env::var("TV_CONNECTOR_URL").ok().filter(|s| !s.is_empty());
    let candles = if use_tv {
        let client = if let Some(script) = tv_script {
            let python = std::env::var("TV_CONNECTOR_PYTHON").unwrap_or_else(|_| "python3".to_string());
            TvConnectorClient::subprocess(python, script, tv_exchange)
        } else if let Some(base) = tv_url {
            TvConnectorClient::with_exchange(base, tv_exchange)
        } else {
            TvConnectorClient::auto(tv_exchange)
        };
        client
            .fetch_klines(symbol, chart_tf, limit)
            .await
            .unwrap_or_default()
    } else {
        BinanceFuturesClient::new()
            .fetch_klines(symbol, chart_tf, limit)
            .await
            .unwrap_or_default()
    };

    let config = Config::default();
    let formations = scan_elliott_formations(&candles, &config);

    axum::Json(serde_json::json!({
        "symbol": symbol,
        "timeframe": tf_str,
        "count": formations.len(),
        "formations": formations
    }))
}
