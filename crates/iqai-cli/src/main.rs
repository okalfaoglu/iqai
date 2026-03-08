//! IQAI CLI - Trading bot command-line interface

use anyhow::Result;
use clap::{Parser, Subcommand};
use iqai_binance::{BinanceFuturesClient, BinanceSpotClient};
use iqai_core::exchange::ExchangeConnector;
use iqai_core::{
    aggregate::aggregate_candles,
    Config, CandleBuffer, SignalEngine, SignalType, Timeframe,
    PositionSide, TradeAction, TradeManager,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "iqai")]
#[command(about = "Smart Money Structure Trading Bot", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run signal scanner (no trading)
    Scan {
        /// Symbol (e.g. ETHUSDT, BTCUSDT)
        #[arg(short, long)]
        symbol: String,

        /// Market: spot or futures
        #[arg(short, long, default_value = "futures")]
        market: String,

        /// Chart timeframe (1M, 5M, 15M, 30M, 1H, 4H, D)
        #[arg(short, long, default_value = "5M")]
        timeframe: String,

        /// Limit of 1M candles to fetch (more = more TF data)
        #[arg(short, long, default_value = "500")]
        limit: u32,
    },

    /// Execute trade on signal (requires API keys)
    Trade {
        /// Symbol
        #[arg(short, long)]
        symbol: String,

        /// Side: buy or sell
        #[arg(short, long)]
        side: String,

        /// Quantity (in base asset)
        #[arg(short, long)]
        quantity: f64,

        /// Market: spot or futures
        #[arg(short, long, default_value = "futures")]
        market: String,
    },

    /// Watch position - anlık fiyat ile kar koruma önerileri
    Watch {
        /// Symbol (e.g. ETHUSDT)
        #[arg(short, long)]
        symbol: String,

        /// Side: long veya short
        #[arg(long)]
        side: String,

        /// Giriş fiyatı
        #[arg(long)]
        entry: f64,

        /// Stop Loss fiyatı
        #[arg(long)]
        sl: f64,

        /// Take Profit fiyatı
        #[arg(long)]
        tp: f64,

        /// Miktar (log için)
        #[arg(short, long, default_value = "1.0")]
        quantity: f64,

        /// Market: spot veya futures
        #[arg(short, long, default_value = "futures")]
        market: String,

        /// Kontrol aralığı (saniye)
        #[arg(long, default_value = "10")]
        interval: u64,
    },

    /// Load config from JSON file
    Config {
        /// Path to config file
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan {
            symbol,
            market,
            timeframe,
            limit,
        } => run_scan(&symbol, &market, &timeframe, limit).await?,
        Commands::Watch {
            symbol,
            side,
            entry,
            sl,
            tp,
            quantity,
            market,
            interval,
        } => run_watch(&symbol, &side, entry, sl, tp, quantity, &market, interval).await?,
        Commands::Trade {
            symbol,
            side,
            quantity,
            market,
        } => run_trade(&symbol, &side, quantity, &market).await?,
        Commands::Config { file } => {
            if let Some(path) = file {
                let cfg = Config::default();
                let json = serde_json::to_string_pretty(&cfg)?;
                std::fs::write(&path, json)?;
                println!("Config written to {:?}", path);
            } else {
                println!("{}", serde_json::to_string_pretty(&Config::default())?);
            }
        }
    }
    Ok(())
}

async fn run_scan(symbol: &str, market: &str, tf_str: &str, limit: u32) -> Result<()> {
    let chart_tf = Timeframe::from_str(tf_str).unwrap_or(Timeframe::M5);
    let config = Config::default();
    let mut engine = SignalEngine::new(config.clone());
    let mut buffer = CandleBuffer::new();

    // Fetch 1M candles and aggregate to all timeframes
    let client: Box<dyn ExchangeConnector> = if market.eq_ignore_ascii_case("spot") {
        Box::new(BinanceSpotClient::new())
    } else {
        Box::new(BinanceFuturesClient::new())
    };

    println!("Fetching 1M candles for {}...", symbol);
    let candles_1m = client
        .fetch_klines(symbol, Timeframe::M1, limit)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if candles_1m.is_empty() {
        anyhow::bail!("No candles received");
    }

    // Aggregate to all timeframes
    for tf in [
        Timeframe::M1,
        Timeframe::M5,
        Timeframe::M15,
        Timeframe::M30,
        Timeframe::H1,
        Timeframe::H4,
        Timeframe::D1,
    ] {
        let agg = aggregate_candles(&candles_1m, tf);
        buffer.update(tf, agg);
    }

    let signals = engine.process(&buffer, chart_tf);
    let trend_strength = engine.trend_strength(&buffer);
    let confidence = engine.system_confidence(&buffer);

    println!("\n=== Smart Money Structure | GainzAlgo v3.0 ===");
    println!("Symbol: {} | Market: {} | TF: {}", symbol, market, tf_str);
    println!("Strength: {:.0} | Confidence: {:.0}%", trend_strength, confidence);
    println!();

    if signals.is_empty() {
        println!("No new signals.");
    } else {
        let cfg = &config;
        if cfg.enable_trade_management {
            println!("📋 Trade Management: Breakeven @ {:.0}R | TP1: {:.0}R ({:.0}%) | TP2: {:.0}R ({:.0}%) | Chandelier ATR({})×{:.1}",
                cfg.breakeven_r, cfg.tp1_r, cfg.partial_tp1_pct * 100.0,
                cfg.tp2_r, cfg.partial_tp2_pct * 100.0,
                cfg.atr_trailing_period, cfg.atr_trailing_mult);
            println!();
        }
        for s in &signals {
            let side = match s.signal_type {
                SignalType::Buy => "🟢 BUY",
                SignalType::Sell => "🔴 SELL",
                SignalType::GetReadyBuy => "⚠ READY BUY",
                SignalType::GetReadySell => "⚠ READY SELL",
                _ => continue,
            };
            println!(
                "{} @ {:.4} | TP: {:?} | SL: {:?} | Conf: {:.0}%",
                side,
                s.price,
                s.take_profit,
                s.stop_loss,
                s.confidence
            );
        }
    }

    Ok(())
}

async fn run_watch(
    symbol: &str,
    side: &str,
    entry: f64,
    sl: f64,
    tp: f64,
    quantity: f64,
    market: &str,
    interval_secs: u64,
) -> Result<()> {
    let config = Config::default();
    let manager = TradeManager::new(config.clone());
    let pos_side = if side.eq_ignore_ascii_case("long") {
        PositionSide::Long
    } else {
        PositionSide::Short
    };
    let mut position = manager.create_position(pos_side, entry, quantity, sl, tp);

    let client: Box<dyn ExchangeConnector> = if market.eq_ignore_ascii_case("spot") {
        Box::new(BinanceSpotClient::new())
    } else {
        Box::new(BinanceFuturesClient::new())
    };

    println!("=== Position Watch (kar koruma) ===");
    println!("{} {} @ {:.4} | SL: {:.4} | TP: {:.4}", symbol, side, entry, sl, tp);
    println!("Kontrol aralığı: {}s | Ctrl+C ile çık\n", interval_secs);

    loop {
        // Güncel mumları al (ATR/trailing için)
        let candles = client
            .fetch_klines(symbol, Timeframe::M1, 50)
            .await
            .unwrap_or_default();
        let current_price = candles
            .last()
            .map(|c| c.close)
            .unwrap_or(entry);

        let action = manager.evaluate(&mut position, current_price, &candles);
        manager.apply_action(&mut position, &action);

        match &action {
            TradeAction::MoveSlToBreakeven => {
                println!("[{}] ⚡ BREAKEVEN: SL'i girişe taşı → {:.4}", chrono::Utc::now().format("%H:%M:%S"), entry);
            }
            TradeAction::PartialClose { pct, reason } => {
                println!("[{}] 📤 KISMI KAPAT: %{:.0} - {}", chrono::Utc::now().format("%H:%M:%S"), pct * 100.0, reason);
            }
            TradeAction::UpdateTrailingStop { new_sl } => {
                println!("[{}] 📈 TRAILING: Yeni SL → {:.4}", chrono::Utc::now().format("%H:%M:%S"), new_sl);
            }
            TradeAction::FullClose { reason } => {
                println!("[{}] 🔴 KAPAT: {}", chrono::Utc::now().format("%H:%M:%S"), reason);
                break;
            }
            TradeAction::None => {}
        }

        // Her döngüde durum (sadece aksiyon varsa veya 5 döngüde bir)
        if matches!(action, TradeAction::None) {
            let profit = match position.side {
                PositionSide::Long => current_price - entry,
                PositionSide::Short => entry - current_price,
            };
            let r = position.risk_r;
            let profit_r = profit / r;
            print!("\rFiyat: {:.4} | Kâr: {:.4} ({:.2}R) | SL: {:.4}   ", current_price, profit, profit_r, position.current_sl);
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
    }

    Ok(())
}

async fn run_trade(symbol: &str, side: &str, quantity: f64, market: &str) -> Result<()> {
    let client: Box<dyn ExchangeConnector> = if market.eq_ignore_ascii_case("spot") {
        Box::new(BinanceSpotClient::new())
    } else {
        Box::new(BinanceFuturesClient::new())
    };

    let order_side = if side.eq_ignore_ascii_case("buy") {
        iqai_core::OrderSide::Buy
    } else {
        iqai_core::OrderSide::Sell
    };

    match client.place_market_order(symbol, order_side, quantity).await {
        Ok(res) => {
            println!("Order executed: {:?}", res);
        }
        Err(e) => {
            eprintln!("Order failed: {}", e);
        }
    }
    Ok(())
}
