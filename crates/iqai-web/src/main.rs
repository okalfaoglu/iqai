//! IQAI Web GUI - Smart Money Structure chart & panels

mod chart_data;

use anyhow::Result;
use axum::{
    extract::Query,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use iqai_binance::BinanceFuturesClient;
use chart_data::compute_annotations;
use iqai_core::{
    aggregate::aggregate_candles,
    exchange::ExchangeConnector,
    Config, CandleBuffer, SignalEngine, SignalType, Timeframe,
};
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Debug, Deserialize)]
struct ChartParams {
    symbol: Option<String>,
    market: Option<String>,
    #[serde(rename = "tf")]
    timeframe: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = Router::new()
        .route("/", get(index))
        .route("/api/chart", get(api_chart));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("IQAI Web GUI: http://localhost:8080");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

async fn index() -> impl IntoResponse {
    Html(include_str!("index.html"))
}

async fn api_chart(Query(params): Query<ChartParams>) -> impl IntoResponse {
    let symbol = params.symbol.as_deref().unwrap_or("ETHUSDT");
    let market = params.market.as_deref().unwrap_or("futures");
    let tf_str = params.timeframe.as_deref().unwrap_or("5M");
    let chart_tf = Timeframe::from_str(tf_str).unwrap_or(Timeframe::M5);

    let client = BinanceFuturesClient::new();
    const BARS: u32 = 500;

    // Grafik periyodunu Binance'ten doğrudan çek – Elliott dalga sayımı için yeterli veri
    let chart_candles_direct = client
        .fetch_klines(symbol, chart_tf, BARS)
        .await
        .unwrap_or_default();

    // M1 – alt periyotlar için
    let candles_1m = client
        .fetch_klines(symbol, Timeframe::M1, BARS)
        .await
        .unwrap_or_default();

    let mut buffer = CandleBuffer::new();

    // Önce grafik periyodunu 500 bar ile doldur
    if !chart_candles_direct.is_empty() {
        buffer.update(chart_tf, chart_candles_direct);
    }

    // Alt periyotlar: M1'den türet (chart_tf zaten dolu ise üzerine yazma)
    for tf in [Timeframe::M1, Timeframe::M5, Timeframe::M15, Timeframe::M30] {
        if tf == chart_tf {
            continue;
        }
        let agg = aggregate_candles(&candles_1m, tf);
        if !agg.is_empty() {
            buffer.update(tf, agg);
        }
    }

    // Üst periyotlar: doğrudan çek
    for tf in [Timeframe::H1, Timeframe::H4, Timeframe::D1] {
        if tf == chart_tf {
            continue;
        }
        if let Ok(candles) = client.fetch_klines(symbol, tf, BARS).await {
            if !candles.is_empty() {
                buffer.update(tf, candles);
            }
        } else {
            let agg = aggregate_candles(&candles_1m, tf);
            if !agg.is_empty() {
                buffer.update(tf, agg);
            }
        }
    }

    let config = Config::default();
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
    let candle_data: Vec<_> = chart_candles
        .iter()
        .map(|c| {
            serde_json::json!({
                "time": c.time / 1000,
                "open": c.open,
                "high": c.high,
                "low": c.low,
                "close": c.close,
                "volume": c.volume
            })
        })
        .collect();

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

    let annotations = compute_annotations(&chart_candles, &config);

    axum::Json(serde_json::json!({
        "symbol": symbol,
        "market": market,
        "timeframe": tf_str,
        "candles": candle_data,
        "signals": signal_data,
        "trend": {
            "strength": trend_strength,
            "confidence": confidence,
            "timeframes": timeframes,
            "trends": trends,
            "cvd": annotations.cvd
        },
        "annotations": {
            "choch": annotations.choch,
            "bos": annotations.bos,
            "liquidity": annotations.liquidity,
            "market_profile": annotations.market_profile,
            "divergence": annotations.divergence,
            "support_line": annotations.support_line,
            "resistance_line": annotations.resistance_line,
            "elliott": annotations.elliott
        }
    }))
}
