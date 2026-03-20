//! IQAI CLI - Trading bot command-line interface

use anyhow::Result;
use chrono::{Datelike, NaiveDate, TimeZone, Timelike, Utc};
use clap::{Parser, Subcommand};
use iqai_binance::{BinanceFuturesClient, BinanceSpotClient};
use iqai_core::exchange::ExchangeConnector;
use iqai_tv::TvConnectorClient;
use iqai_web::chart_data::scan_elliott_formations;
use iqai_web::ai;
use iqai_core::elliott_detector::compute_elliott;
use iqai_core::{
    build_analysis_snapshot,
    compute_q_radar_opportunity,
    run_backtest,
    build_scenarios_for_series,
    build_smart_money_context_for_series,
    detect_fake_breakout_signal, FakeBreakoutConfig,
    Config, CandleBuffer, SignalEngine, SignalType, Timeframe,
    PositionSide, TradeAction, TradeManager,
};
use iqai_core::auto_trader::{
    AutoTrader, AutoTraderConfig, SignalSource,
    signal_from_q_setup, signal_from_elliott, TradingMode,
};
use iqai_core::trade_db::TradeDb;
use iqai_web::notify::Notifier;
use std::path::PathBuf;
use tokio::process::Command;

/// Watchlist girişi: borsa/piyasa/sembol (watchlist.json)
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WatchlistEntry {
    /// Sembol (örn. ETHUSDT, XU100, AAPL)
    pub symbol: String,
    /// Piyasa: spot | futures | tv
    #[serde(default = "default_market")]
    pub market: String,
    /// Borsa kodu (market=tv iken: BINANCE, BIST, NASDAQ)
    #[serde(default)]
    pub exchange: Option<String>,
    /// Zaman dilimi (1M, 5M, 15M, 1H, 4H, D)
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
}

fn default_market() -> String {
    "futures".to_string()
}
fn default_timeframe() -> String {
    "5M".to_string()
}

/// BIST ve NASDAQ 7/24 değil; piyasa saatleri dışında tarama atlanır.
/// BIST: 10:00–18:00 İstanbul (UTC+3) = 07:00–15:00 UTC, Pazartesi–Cuma.
/// NASDAQ: 09:30–16:00 Doğu (ET) ≈ 13:30–21:00 UTC, Pazartesi–Cuma.
fn is_market_open_tv(exchange: &str) -> bool {
    let now = chrono::Utc::now();
    let day = now.weekday().num_days_from_monday(); // Mon=0 .. Sun=6
    if day >= 5 {
        return false; // Cumartesi, Pazar kapalı
    }
    let hour = now.hour();
    match exchange.to_uppercase().as_str() {
        "BIST" => hour >= 7 && hour < 15,   // 07:00–15:00 UTC
        "NASDAQ" | "NYSE" => hour >= 13 && hour < 21, // ~09:30–16:00 ET
        _ => true, // BINANCE vb. 7/24
    }
}

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

        /// Açık pozisyon varsa: giriş fiyatı (analiz raporunda tut/kapat önerisi için)
        #[arg(long)]
        entry: Option<f64>,

        /// Açık pozisyon stop loss fiyatı (--entry ile birlikte kullanın)
        #[arg(long)]
        sl: Option<f64>,

        /// Açık pozisyon yönü: long veya short (--entry ile birlikte kullanın)
        #[arg(long)]
        side: Option<String>,
    },

    /// Birden fazla borsa/piyasa/sembolü watchlist dosyasına göre tara
    ScanBatch {
        /// Watchlist JSON dosyası (her satır: {"symbol":"ETHUSDT","market":"futures","exchange":null,"timeframe":"5M"})
        #[arg(short, long, default_value = "watchlist.json")]
        watchlist: PathBuf,

        /// Her sembol için 1M mum limiti
        #[arg(short, long, default_value = "500")]
        limit: u32,

        /// Sürekli çalıştır (Q-ANALİZ daemon): her turdan sonra bekleyip tekrar tara
        #[arg(long)]
        daemon: bool,

        /// Daemon modunda turlar arası bekleme süresi (saniye)
        #[arg(long, default_value = "300")]
        interval: u64,
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

    /// Otomatik alım/satım robotu (config.json "trading" bölümünden ayarlar okunur)
    Robot {
        /// Mod: live | dry | paper (config.json'daki değeri override eder)
        #[arg(short, long)]
        mode: Option<String>,

        /// Tarama aralığı (saniye, varsayılan 60)
        #[arg(short, long, default_value = "60")]
        interval: u64,
    },

    /// Q-Analiz daemon: sürekli tarama, tespitleri DB'ye yazar ve Telegram'a gönderir (web kapalıyken çalışır).
    QAnalizDaemon {
        /// Tarama aralığı (saniye, varsayılan 300)
        #[arg(short, long, default_value = "300")]
        interval: u64,
        /// Tüm sembol+timeframe sonuçlarını yazdır (tespit olmasa bile)
        #[arg(long)]
        verbose: bool,
    },

    /// Load config from JSON file
    Config {
        /// Path to config file
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// Geçmiş verilerde geçerli Elliott formasyonlarını tara
    Formations {
        /// Symbol (e.g. ETHUSDT, BTCUSDT)
        #[arg(short, long)]
        symbol: String,

        /// Market: spot veya futures
        #[arg(short, long, default_value = "futures")]
        market: String,

        /// Zaman dilimi (1M, 5M, 15M, 30M, 1H, 4H, D)
        #[arg(short, long, default_value = "15M")]
        timeframe: String,

        /// Bar limiti
        #[arg(short, long, default_value = "500")]
        limit: u32,
    },

    /// Q-Analiz backtest: geçmiş veriyle Q-Setup simülasyonu, başlangıç/son sermaye ve getiri
    Backtest {
        /// Symbol (e.g. ETHUSDT, BTCUSDT)
        #[arg(short, long)]
        symbol: String,

        /// Market: futures veya spot
        #[arg(short, long, default_value = "futures")]
        market: String,

        /// Zaman dilimi (5M, 15M, 1H, 4H, D)
        #[arg(short, long, default_value = "5M")]
        timeframe: String,

        /// Çekilecek mum sayısı (--from/--to yoksa kullanılır)
        #[arg(short, long, default_value = "1000")]
        limit: u32,

        /// Tarih aralığı başlangıcı (YYYY-MM-DD). --to ile veya tek başına (bugüne kadar)
        #[arg(long)]
        from: Option<String>,

        /// Tarih aralığı bitişi (YYYY-MM-DD). --from ile kullanın
        #[arg(long)]
        to: Option<String>,

        /// Başlangıç sermayesi (USDT)
        #[arg(long, default_value = "10000")]
        capital: f64,

        /// İşlem başına risk (%)
        #[arg(long, default_value = "1.0")]
        risk_pct: f64,

        /// Maksimum kaldıraç
        #[arg(long, default_value = "10")]
        leverage: f64,

        /// Q-Setup minimum skor (0–100). Düşürürseniz daha çok sinyal çıkar (varsayılan 70).
        #[arg(long, default_value = "70")]
        q_score_min: f64,
    },

    /// Ollama kurulumunu kontrol et (config.json ai.ollama_base_url veya http://localhost:11434)
    OllamaCheck,

    /// Run q-analiz-daemon + robot + web together
    Stack {
        /// Q-Analiz daemon interval (seconds)
        #[arg(long, default_value = "300")]
        q_interval: u64,

        /// Robot interval (seconds)
        #[arg(long, default_value = "60")]
        robot_interval: u64,

        /// Robot mode: dry | paper | live
        #[arg(long, default_value = "dry")]
        robot_mode: String,

        /// Web port (iqai-web, default 8080)
        #[arg(long, default_value = "8080")]
        web_port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let app_cfg = iqai_core::AppConfig::load();
    iqai_core::init_from_config(app_cfg.as_ref().and_then(|c| c.logging.as_ref()))
        .expect("Loglama başlatılamadı");
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan {
            symbol,
            market,
            timeframe,
            limit,
            entry,
            sl,
            side,
        } => run_scan(&symbol, &market, None, &timeframe, limit, entry, sl, side.as_deref()).await?,
        Commands::ScanBatch {
            watchlist,
            limit,
            daemon,
            interval,
        } => run_scan_batch(&watchlist, limit, daemon, interval).await?,
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
        Commands::Robot { mode, interval } => run_robot(mode.as_deref(), interval).await?,
        Commands::QAnalizDaemon { interval, verbose } => run_q_analiz_daemon(interval, verbose).await?,
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
        Commands::Formations {
            symbol,
            market,
            timeframe,
            limit,
        } => run_formations(&symbol, &market, &timeframe, limit).await?,
        Commands::Backtest {
            symbol,
            market,
            timeframe,
            limit,
            from,
            to,
            capital,
            risk_pct,
            leverage,
            q_score_min,
        } => run_backtest_cmd(&symbol, &market, &timeframe, limit, from.as_deref(), to.as_deref(), capital, risk_pct, leverage, q_score_min).await?,
        Commands::OllamaCheck => run_ollama_check().await?,
        Commands::Stack { q_interval, robot_interval, robot_mode, web_port } => {
            run_stack(q_interval, robot_interval, &robot_mode, web_port).await?
        }
    }
    Ok(())
}

async fn run_stack(q_interval: u64, robot_interval: u64, robot_mode: &str, web_port: u16) -> Result<()> {
    let exe = std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("current_exe bulunamadı: {}", e))?;

    // `iqai` aynı binary: q-analiz-daemon + robot
    let mut q_child = Command::new(&exe)
        .arg("q-analiz-daemon")
        .arg("-i")
        .arg(q_interval.to_string())
        .kill_on_drop(true)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow::anyhow!("q-analiz-daemon başlatılamadı: {}", e))?;

    let mut robot_child = Command::new(&exe)
        .arg("robot")
        .arg("--mode")
        .arg(robot_mode)
        .arg("--interval")
        .arg(robot_interval.to_string())
        .kill_on_drop(true)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow::anyhow!("robot başlatılamadı: {}", e))?;

    // `iqai-web` ayrı binary; aynı target dizininde sibling olarak var.
    let web_exe = exe
        .parent()
        .map(|p| p.join("iqai-web"))
        .ok_or_else(|| anyhow::anyhow!("iqai-web path çözümlenemedi"))?;

    // Web port override: IQAI_WEB_PORT env
    let mut web_child = Command::new(&web_exe)
        .env("IQAI_WEB_PORT", web_port.to_string())
        .kill_on_drop(true)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow::anyhow!("iqai-web başlatılamadı ({}): {}", web_exe.display(), e))?;

    println!(
        "✅ Stack başladı: q-analiz-daemon(i={}) + robot(mode={}, i={}) + web(port={})",
        q_interval, robot_mode, robot_interval, web_port
    );
    println!("   Durdurmak için Ctrl+C.");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("⏹ Ctrl+C alındı, süreçler kapatılıyor...");
        }
        status = q_child.wait() => {
            println!("⏹ q-analiz-daemon durdu: {:?}", status);
        }
        status = robot_child.wait() => {
            println!("⏹ robot durdu: {:?}", status);
        }
        status = web_child.wait() => {
            println!("⏹ web durdu: {:?}", status);
        }
    }

    // Best-effort kill all.
    let _ = q_child.kill().await;
    let _ = robot_child.kill().await;
    let _ = web_child.kill().await;

    Ok(())
}

async fn run_scan(
    symbol: &str,
    market: &str,
    exchange: Option<&str>,
    tf_str: &str,
    limit: u32,
    position_entry: Option<f64>,
    position_sl: Option<f64>,
    position_side: Option<&str>,
) -> Result<()> {
    if market.eq_ignore_ascii_case("tv") {
        let ex = exchange.unwrap_or("BINANCE");
        if !is_market_open_tv(ex) {
            println!("{} piyasa kapalı (7/24 değil), atlanıyor.", ex);
            return Ok(());
        }
    }

    let chart_tf = Timeframe::from_str(tf_str).unwrap_or(Timeframe::M5);
    let config = Config::default();

    let client: Box<dyn ExchangeConnector> = if market.eq_ignore_ascii_case("tv") {
        let ex = exchange.unwrap_or("BINANCE");
        let tv_script = std::env::var("TV_CONNECTOR_SCRIPT").ok().filter(|s| !s.is_empty());
        let tv_url = std::env::var("TV_CONNECTOR_URL").ok().filter(|s| !s.is_empty());
        if let Some(script) = tv_script {
            let python = std::env::var("TV_CONNECTOR_PYTHON").unwrap_or_else(|_| "python3".to_string());
            Box::new(TvConnectorClient::subprocess(python, script, ex))
        } else if let Some(base) = tv_url {
            Box::new(TvConnectorClient::with_exchange(base, ex))
        } else {
            Box::new(TvConnectorClient::auto(ex))
        }
    } else if market.eq_ignore_ascii_case("spot") {
        Box::new(BinanceSpotClient::new())
    } else {
        Box::new(BinanceFuturesClient::new())
    };

    if market.eq_ignore_ascii_case("tv") {
        println!("Fetching candles for {} (TV / {})...", symbol, exchange.unwrap_or("BINANCE"));
    } else {
        println!(
            "Fetching native 1M, 5M, 15M, 30M, 1H, 4H, 1D for {} ({} / {})...",
            symbol, market, exchange.unwrap_or("-")
        );
    }
    let buffer = fetch_into_buffer(&*client, symbol, market, exchange, limit, chart_tf).await?;

    run_scan_from_buffer(
        &buffer,
        symbol,
        market,
        tf_str,
        chart_tf,
        &config,
        position_entry,
        position_sl,
        position_side,
    )
    .await
}

async fn fetch_into_buffer(
    client: &dyn ExchangeConnector,
    symbol: &str,
    market: &str,
    exchange: Option<&str>,
    limit: u32,
    chart_tf: Timeframe,
) -> Result<CandleBuffer> {
    let mut buffer = CandleBuffer::new();
    let timeframes = [
        Timeframe::M1,
        Timeframe::M5,
        Timeframe::M15,
        Timeframe::M30,
        Timeframe::H1,
        Timeframe::H4,
        Timeframe::D1,
    ];

    if market.eq_ignore_ascii_case("tv") {
        let ex = exchange.unwrap_or("BINANCE");
        let mut total_received = 0;
        for tf in timeframes {
            match client.fetch_klines(symbol, tf, limit).await {
                Ok(candles) if !candles.is_empty() => {
                    total_received += candles.len();
                    buffer.update(tf, candles);
                }
                Ok(_) => {}
                Err(e) => eprintln!("  TV {} hatası: {}", match tf {
                    Timeframe::M1 => "1M", Timeframe::M5 => "5M", Timeframe::M15 => "15M",
                    Timeframe::M30 => "30M", Timeframe::H1 => "1H", Timeframe::H4 => "4H", Timeframe::D1 => "1D",
                }, e),
            }
        }
        if buffer.get(chart_tf).map(|c| c.is_empty()).unwrap_or(true) {
            anyhow::bail!("TV'den veri alınamadı ({} için {} bar yok).", symbol, ex);
        }
        println!("  TV toplam: {} bar alındı.", total_received);
    } else {
        // Binance/Spot: her TF native (TV gibi)
        let mut total_received = 0;
        for tf in timeframes {
            match client.fetch_klines(symbol, tf, limit).await {
                Ok(candles) if !candles.is_empty() => {
                    total_received += candles.len();
                    buffer.update(tf, candles);
                }
                Ok(_) => {}
                Err(e) => eprintln!("  {} {:?} hatası: {}", symbol, tf, e),
            }
        }
        if buffer.get(chart_tf).map(|c| c.is_empty()).unwrap_or(true) {
            anyhow::bail!("Veri alınamadı ({} için {:?} bar yok).", symbol, chart_tf);
        }
        println!("  Toplam: {} bar alındı.", total_received);
    }
    Ok(buffer)
}

async fn run_scan_from_buffer(
    buffer: &CandleBuffer,
    symbol: &str,
    market: &str,
    tf_str: &str,
    chart_tf: Timeframe,
    config: &Config,
    position_entry: Option<f64>,
    position_sl: Option<f64>,
    position_side: Option<&str>,
) -> Result<()> {
    let app_cfg = iqai_core::AppConfig::load();
    let sm_cfg = app_cfg.as_ref().and_then(|c| c.smart_money.as_ref());
    let sm_config = iqai_core::Config::from_smart_money(sm_cfg);
    let mut engine = SignalEngine::new(sm_config.clone());

    let signals = engine.process(buffer, chart_tf);
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

    // Q-ANALİZ / Q-RADAR (radar önce hesaplanır; setup'ta radar_early set edilir)
    let q_radar = engine.compute_q_radar(&buffer, chart_tf, symbol);
    let q_setup = engine.compute_q_setup(&buffer, chart_tf, symbol, q_radar.as_ref());

    if let Some(ref q) = q_setup {
        let side = match q.side {
            SignalType::Buy => "LONG",
            SignalType::Sell => "SHORT",
            _ => "—",
        };
        println!("\n=== Q-ANALİZ (Tek Mal – Tek Hedef – Tek Stop) ===");
        println!("  Yön: {} | Giriş: {:.4} | TP: {:.4} | SL: {:.4}", side, q.entry, q.take_profit, q.stop_loss);
        println!("  Q Skoru: {:.1} | Beklenen süre: ~{} bar", q.q_score, q.expected_bars);
        // Telegram ve diğer kanallara bildir (TELEGRAM_BOT_TOKEN, TELEGRAM_CHAT_ID gerekir)
        let notifier = Notifier::from_env();
        let current_price = buffer.get(chart_tf).and_then(|c| c.last()).map(|c| c.close);
        if let Err(e) = notifier.notify_q_setup_with_price(q, current_price).await {
            eprintln!("  [Bildirim hatası: {}]", e);
        } else {
            println!("  📤 Bildirim gönderildi (Telegram vb.).");
            if !notifier.has_telegram() {
                println!("  (Telegram atlandı: TELEGRAM_BOT_TOKEN + TELEGRAM_CHAT_ID veya config.json \"notification\" gerekir.)");
            }
        }
    } else {
        println!("\n=== Q-ANALİZ ===");
        println!("  Aktif setup yok.");
    }

    if let Some(ref r) = q_radar {
        let side = match r.side {
            SignalType::Buy => "LONG",
            SignalType::Sell => "SHORT",
            _ => "—",
        };
        println!("\n=== Q-RADAR (Erken uyarı) ===");
        println!("  Yön: {} | Güven: {:.0}% | Pencere: {}-{} bar", side, r.confidence * 100.0, r.expected_window_bars.0, r.expected_window_bars.1);
        println!("  Referans (izlenecek) fiyat: {:.4}", r.reference_price);
        if let Some(sl) = r.suggested_sl {
            println!("  Tahmini SL: {:.4} (Q-Setup çıkınca kesinleşir)", sl);
        }
        println!("  Hedef: Q-Setup bekleniyor (giriş/SL/TP Q-ANALİZ'de belirlenir)");
        println!("  (Q-Setup ne zaman: zaman fazı 0.2–0.6 ve Q skoru ≥70; Q-RADAR erken faz 0.1–0.3.)");
        let notifier = Notifier::from_env();
        if let Err(e) = notifier.notify_q_radar(r).await {
            eprintln!("  [Bildirim hatası: {}]", e);
        } else {
            println!("  📤 Q-RADAR bildirimi gönderildi.");
            if !notifier.has_telegram() {
                println!("  (Telegram atlandı: TELEGRAM_BOT_TOKEN + TELEGRAM_CHAT_ID veya config.json \"notification\" gerekir.)");
            }
        }
    } else {
        println!("\n=== Q-RADAR ===");
        println!("  Erken uyarı yok.");
    }

    // === POSITION METRICS (shared T/D/Q-style view) ===
    let side_enum = position_side.and_then(|s| {
        if s.eq_ignore_ascii_case("long") || s.eq_ignore_ascii_case("buy") {
            Some(SignalType::Buy)
        } else if s.eq_ignore_ascii_case("short") || s.eq_ignore_ascii_case("sell") {
            Some(SignalType::Sell)
        } else {
            None
        }
    });
    let tp_for_metrics = q_setup.as_ref().and_then(|q| Some(q.take_profit));
    if let Some(metrics) = engine.compute_position_metrics(
        buffer,
        chart_tf,
        symbol,
        side_enum,
        position_entry,
        position_sl,
        tp_for_metrics,
    ) {
        let tmr = &metrics.tmr_scores;
        println!("\n=== POZİSYON METRİKLERİ (T/D/Q ortak) ===");
        println!(
            "  Yerel trend: {} | Global trend: {} | Oynaklık: {:.2}%",
            metrics.local_trend, metrics.global_trend, metrics.volatility_pct
        );
        println!(
            "  Momentum (kısa/uzun): {:.2}% / {:.2}%",
            metrics.momentum_short * 100.0,
            metrics.momentum_long * 100.0
        );
        println!(
            "  Risk/Ödül (RR): {:.2} | Trend/Momentum/RR puanı: {}/{}/{} | Pozisyon Gücü: {}/10",
            metrics.rr,
            tmr.trend_points,
            tmr.momentum_points,
            tmr.rr_points,
            tmr.strength_points
        );
        if metrics.trend_exhaustion {
            println!("  ⚠ Trend tükenmesi tespit edildi.");
        }
        if metrics.structure_shift {
            println!("  ⚠ Yapı kayması (structure shift) tespit edildi.");
        }
    }

    // === ANALİZ RAPORU (Web GUI ile aynı veri: aktif dalga, tip, Elliott vs kriterler) ===
    let min_bars_elliott = (config.pivot_length as usize) * 4 + 20;
    if let Some(candles) = buffer.get(chart_tf) {
        if candles.len() >= min_bars_elliott {
            let elliott = compute_elliott(candles, &config, false);
            println!("\n=== ANALİZ RAPORU (Elliott + Smart Money) ===");
            if elliott.validation_ok == Some(true) && !elliott.formation.is_empty() && elliott.formation != "—" {
                println!("  Aktif dalga: {} ({})", elliott.formation, elliott.formation_type);
                if let Some(ref msg) = elliott.validation_msg {
                    println!("  Doğrulama: {}", msg);
                }
                if let Some(in_prog) = elliott.in_progress {
                    if in_prog {
                        println!("  Durum: Formasyon henüz tamamlanmadı (dalga türü değişebilir).");
                    }
                }
                // Elliott vs Smart Money: Impulse/Diagonal'da yön; düzeltmede sadece bilgi
                let overlap_msg = if let Some(ref imp) = elliott.impulse_state {
                    let ew_bull = imp.is_bullish;
                    let sm_bull = trend_strength > 0.0;
                    if ew_bull == sm_bull { "Uyumlu (yapı ile Strength aynı yönde)." } else { "Dikkat — yapı ile Strength zıt yönde." }
                } else {
                    "Düzeltme formasyonu — Strength ile birlikte değerlendirin."
                };
                println!("  Elliott ↔ Smart Money: {}", overlap_msg);

                // Açık pozisyon önerisi (--entry --sl --side verilmişse)
                if let (Some(entry), Some(sl), Some(side)) = (position_entry, position_sl, position_side) {
                    let is_long = side.eq_ignore_ascii_case("long");
                    let suggest_hold = if let Some(ref imp) = elliott.impulse_state {
                        imp.is_bullish == is_long
                    } else {
                        (trend_strength > 0.0) == is_long
                    };
                    let msg = if suggest_hold {
                        "Poz önerisi: Tut (yapı pozla aynı yönde). SL ile takip edin."
                    } else {
                        "Poz önerisi: Kapatmayı değerlendir (yapı veya Strength pozun tersinde)."
                    };
                    println!("  Giriş: {} | SL: {} | {}", entry, sl, msg);
                }
            } else {
                println!("  Aktif (geçerli) Elliott formasyonu yok.");
                if let (Some(entry), Some(sl), Some(side)) = (position_entry, position_sl, position_side) {
                    let is_long = side.eq_ignore_ascii_case("long");
                    let suggest_hold = (trend_strength > 0.0) == is_long;
                    let msg = if suggest_hold { "Poz önerisi: Tut (Strength pozla aynı yönde)." } else { "Poz önerisi: Kapatmayı değerlendir." };
                    println!("  Giriş: {} | SL: {} | {}", entry, sl, msg);
                }
            }
            println!("  (Web ile tutarlılık: aynı sembol/TF ve limit ile Web de native TF kullanır.)");
        } else {
            println!(
                "\n=== ANALİZ RAPORU (Elliott + Smart Money) ===\n  Elliott analizi: yetersiz bar (mevcut: {}, en az: {}). 15M/30M için --limit 1000+ önerilir.",
                candles.len(),
                min_bars_elliott
            );
        }
    }

    Ok(())
}

async fn run_scan_batch(
    watchlist_path: &PathBuf,
    limit: u32,
    daemon: bool,
    interval_secs: u64,
) -> Result<()> {
    let contents = std::fs::read_to_string(watchlist_path)
        .map_err(|e| anyhow::anyhow!("Watchlist okunamadı {:?}: {}", watchlist_path, e))?;
    let entries: Vec<WatchlistEntry> = serde_json::from_str(&contents)
        .map_err(|e| anyhow::anyhow!("Watchlist JSON hatası: {}", e))?;
    if entries.is_empty() {
        anyhow::bail!("Watchlist boş");
    }

    // Aynı (sembol, piyasa, borsa) için tek veri çekimi: grupla
    let groups: Vec<(String, String, Option<String>, Vec<String>)> = {
        let mut g: Vec<(String, String, Option<String>, Vec<String>)> = Vec::new();
        for e in &entries {
            match g.last_mut() {
                Some((s, m, ex, tfs)) if *s == e.symbol && *m == e.market && ex.as_deref() == e.exchange.as_deref() => {
                    tfs.push(e.timeframe.clone());
                }
                _ => {
                    g.push((e.symbol.clone(), e.market.clone(), e.exchange.clone(), vec![e.timeframe.clone()]));
                }
            }
        }
        g
    };
    let total_entries = entries.len();

    let mut round: u64 = 0;
    loop {
        round += 1;
        if daemon {
            println!(
                "\n🔄 Q-ANALİZ Daemon — Tur {} | Sonraki tur: {}s sonra (Ctrl+C ile çık)\n",
                round, interval_secs
            );
        } else {
            println!("Watchlist: {} giriş ({} sembol grubu), taranacak.\n", total_entries, groups.len());
        }

        let mut entry_index = 0;
        for (symbol, market, exchange, timeframes) in &groups {
            entry_index += 1;
            if market.eq_ignore_ascii_case("tv") {
                let ex = exchange.as_deref().unwrap_or("BINANCE");
                if !is_market_open_tv(ex) {
                    println!(
                        "\n========== [{}/{}] {} | {} | {} ==========",
                        entry_index, total_entries, symbol, market, exchange.as_deref().unwrap_or("-"),
                    );
                    println!("{} piyasa kapalı (7/24 değil), atlanıyor.", ex);
                    entry_index += timeframes.len();
                    continue;
                }
            }

            let client: Box<dyn ExchangeConnector> = if market.eq_ignore_ascii_case("tv") {
                let ex = exchange.as_deref().unwrap_or("BINANCE");
                let tv_script = std::env::var("TV_CONNECTOR_SCRIPT").ok().filter(|s| !s.is_empty());
                let tv_url = std::env::var("TV_CONNECTOR_URL").ok().filter(|s| !s.is_empty());
                if let Some(script) = tv_script {
                    let python = std::env::var("TV_CONNECTOR_PYTHON").unwrap_or_else(|_| "python3".to_string());
                    Box::new(TvConnectorClient::subprocess(python, script, ex))
                } else if let Some(base) = tv_url {
                    Box::new(TvConnectorClient::with_exchange(base, ex))
                } else {
                    Box::new(TvConnectorClient::native(ex))
                }
            } else if market.eq_ignore_ascii_case("spot") {
                Box::new(BinanceSpotClient::new())
            } else {
                Box::new(BinanceFuturesClient::new())
            };

            let chart_tf_first = Timeframe::from_str(timeframes.first().map(String::as_str).unwrap_or("5M")).unwrap_or(Timeframe::M5);
            if market.eq_ignore_ascii_case("tv") {
                println!("\nFetching candles for {} (TV / {})...", symbol, exchange.as_deref().unwrap_or("BINANCE"));
            } else {
                println!(
                    "\nFetching native 1M, 5M, 15M, 30M, 1H, 4H, 1D for {} ({} / {}) — {} TF...",
                    symbol, market, exchange.as_deref().unwrap_or("-"), timeframes.len()
                );
            }
            let buffer = match fetch_into_buffer(&*client, symbol, market, exchange.as_deref(), limit, chart_tf_first).await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Hata ({}): {}", symbol, e);
                    continue;
                }
            };
            let config = Config::default();

            for (tf_idx, tf_str) in timeframes.iter().enumerate() {
                let idx = entry_index + tf_idx;
                println!(
                    "\n========== [{}/{}] {} | {} | {} | {} ==========",
                    idx, total_entries, symbol, market, exchange.as_deref().unwrap_or("-"), tf_str,
                );
                let chart_tf = Timeframe::from_str(tf_str).unwrap_or(Timeframe::M5);
                if let Err(e) = run_scan_from_buffer(
                    &buffer,
                    symbol,
                    market,
                    tf_str,
                    chart_tf,
                    &config,
                    None,
                    None,
                    None,
                )
                .await
                {
                    eprintln!("Hata: {}", e);
                }
            }
            entry_index += timeframes.len() - 1;
        }

        if !daemon {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
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

    let chart_tf = Timeframe::M1;
    let mut protect_notified = false;
    let notifier = Notifier::from_env();

    loop {
        // Güncel mumları al (ATR/trailing için)
        let candles = client
            .fetch_klines(symbol, chart_tf, 50)
            .await
            .unwrap_or_default();
        let current_price = candles
            .last()
            .map(|c| c.close)
            .unwrap_or(entry);

        // Poz Koruma: buffer doldur, sinyal varsa bir kez bildir
        if !candles.is_empty() && !protect_notified {
            let mut buffer = CandleBuffer::new();
            buffer.update(chart_tf, candles.clone());
            let engine = SignalEngine::new(config.clone());
            if let Some(protect) = engine.compute_protect_signal(&buffer, chart_tf, symbol, entry, sl) {
                if let Err(e) = notifier.notify_protect(&protect).await {
                    eprintln!("Poz Koruma bildirimi hatası: {:?}", e);
                }
                protect_notified = true;
            }
        }

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

async fn run_formations(symbol: &str, market: &str, tf_str: &str, limit: u32) -> Result<()> {
    let chart_tf = Timeframe::from_str(tf_str).unwrap_or(Timeframe::M15);
    let config = Config::default();

    let client: Box<dyn ExchangeConnector> = if market.eq_ignore_ascii_case("spot") {
        Box::new(BinanceSpotClient::new())
    } else {
        Box::new(BinanceFuturesClient::new())
    };

    println!("Geçmiş formasyon taranıyor: {} | {} | limit={}...", symbol, tf_str, limit);
    let candles = client
        .fetch_klines(symbol, chart_tf, limit)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if candles.is_empty() {
        anyhow::bail!("Veri alınamadı");
    }

    let formations = scan_elliott_formations(&candles, &config);
    println!("\n=== Geçerli Elliott Formasyonları ({}) ===", formations.len());
    for (i, f) in formations.iter().enumerate() {
        let dir = if f.is_bullish { "▲" } else { "▼" };
        let pts: String = f
            .wave_points
            .iter()
            .map(|p| format!("W{}:{:.2}", p.label, p.price))
            .collect::<Vec<_>>()
            .join(" ");
        let time_str = chrono::DateTime::from_timestamp(f.end_time / 1000, 0)
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| f.end_time.to_string());
        println!(
            "{}. {} {} | {} | {} | {}",
            i + 1, dir, f.formation, f.formation_type, time_str, pts
        );
    }
    Ok(())
}

async fn run_robot(mode_override: Option<&str>, interval_secs: u64) -> Result<()> {
    let app_cfg = iqai_core::AppConfig::load()
        .ok_or_else(|| anyhow::anyhow!("config.json bulunamadı veya okunamadı"))?;

    let trading_cfg = app_cfg.trading.as_ref()
        .ok_or_else(|| anyhow::anyhow!("config.json'da \"trading\" bölümü yok"))?;

    let mut at_cfg = AutoTraderConfig::from_trading_config(trading_cfg);
    at_cfg.enabled = true;
    if let Some(m) = mode_override {
        at_cfg.mode = TradingMode::from_str(m);
    }

    let symbols: Vec<String> = trading_cfg.symbols.clone().unwrap_or_else(|| vec!["ETHUSDT".into()]);
    let timeframes: Vec<Timeframe> = trading_cfg.timeframes.as_ref()
        .map(|tfs| tfs.iter().filter_map(|s| Timeframe::from_str(s)).collect())
        .unwrap_or_else(|| vec![Timeframe::M5, Timeframe::M15, Timeframe::H1]);

    let market = trading_cfg.market.as_deref().unwrap_or("futures");

    let exchange: Box<dyn ExchangeConnector> = if at_cfg.mode == TradingMode::Live {
        let api_key = trading_cfg
            .api_key
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("BINANCE_API_KEY").ok().filter(|s| !s.trim().is_empty()))
            .ok_or_else(|| anyhow::anyhow!("Live mod için api_key gerekli (config.json trading.api_key veya BINANCE_API_KEY env)"))?;
        let secret = trading_cfg
            .secret_key
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("BINANCE_SECRET_KEY").ok().filter(|s| !s.trim().is_empty()))
            .ok_or_else(|| anyhow::anyhow!("Live mod için secret_key gerekli (config.json trading.secret_key veya BINANCE_SECRET_KEY env)"))?;
        if market.eq_ignore_ascii_case("spot") {
            Box::new(BinanceSpotClient::with_credentials(api_key, secret))
        } else {
            Box::new(BinanceFuturesClient::with_credentials(api_key, secret))
        }
    } else {
        if market.eq_ignore_ascii_case("spot") {
            Box::new(BinanceSpotClient::new())
        } else {
            Box::new(BinanceFuturesClient::new())
        }
    };

    let sm_cfg = app_cfg.smart_money.as_ref();
    let sm_config = Config::from_smart_money(sm_cfg);

    let mut trader = AutoTrader::new(at_cfg.clone(), sm_config.clone());
    let notifier = Notifier::from_env();

    println!("🤖 IQAI Robot başlatıldı");
    println!("   Mod: {} | Piyasa: {} | Aralık: {}s", at_cfg.mode, market, interval_secs);
    println!("   Semboller: {:?}", symbols);
    println!("   Timeframe: {:?}", timeframes);
    println!("   Risk: %{} | Maks Poz: {} | Kaldıraç: {}x", at_cfg.risk_per_trade_pct, at_cfg.max_positions, at_cfg.max_leverage);
    println!("   Min RR: {} | Min Q-Score: {}", at_cfg.min_rr, at_cfg.min_q_score);
    println!("   Q-RADAR filtre: {} (min güven: {}/10)", if at_cfg.use_radar_filter { "açık" } else { "kapalı" }, at_cfg.min_radar_confidence);
    println!("   Günlük kayıp limiti: %{}", at_cfg.daily_loss_limit_pct);
    if at_cfg.mode.writes_db() {
        println!("   DB: {}", at_cfg.db_path.as_deref().unwrap_or("data/trades.db"));
    }
    println!();

    if at_cfg.mode.writes_db() {
        let path_str = at_cfg.db_path.as_deref();
        if let Ok(db) = TradeDb::open(path_str) {
            let rows: Vec<(i64, i64, String, String, String, String, f64, f64, f64, f64, f64, String)> =
                match db.load_open_positions(at_cfg.mode) {
                    Ok(r) => r,
                    Err(_) => vec![],
                };
            if !rows.is_empty() {
                trader.restore_open_positions(rows);
                let n = trader.open_positions.len();
                println!("   {} açık pozisyon DB'den yüklendi (recovery).\n", n);
            }
        }
    }

    let balance = if at_cfg.mode == TradingMode::Live {
        match exchange.get_balance("USDT").await {
            Ok(b) => { println!("   Hesap bakiyesi: {:.2} USDT\n", b); b }
            Err(e) => { eprintln!("   Bakiye sorgulanamadı: {} — 1000 USDT varsayılıyor\n", e); 1000.0 }
        }
    } else {
        println!("   Simüle bakiye: 1000.00 USDT (dry/paper)\n");
        1000.0
    };

    let mut round: u64 = 0;
    let mut last_hourly_report: Option<(chrono::NaiveDate, u32)> = None;
    loop {
        round += 1;
        let now_dt = chrono::Utc::now();
        let now = now_dt.format("%H:%M:%S");
        println!("──── Tur {} | {} ────", round, now);

        let mut all_signals = Vec::new();
        let mut current_prices = std::collections::HashMap::new();
        let mut candles_map = std::collections::HashMap::new();

        for symbol in &symbols {
            for &tf in &timeframes {
                match exchange.fetch_klines(symbol, tf, 500).await {
                    Ok(candles) if !candles.is_empty() => {
                        if let Some(last) = candles.last() {
                            current_prices.insert(symbol.clone(), last.close);
                        }

                        // Merkezi Q-RADAR fırsat analizi (robot + web aynı modül)
                        let mut buffer = CandleBuffer::new();
                        buffer.update(tf, candles.clone());
                        let opportunity = compute_q_radar_opportunity(&buffer, tf, symbol, &sm_config);
                        let q_radar = opportunity.radar.as_ref();
                        let engine = SignalEngine::new(sm_config.clone());
                        let qanaliz_ok = |is_long: bool| {
                            if !at_cfg.use_radar_filter || opportunity.direction == "—" {
                                return true;
                            }
                            let dir_ok = (is_long && opportunity.direction == "LONG")
                                || (!is_long && opportunity.direction == "SHORT");
                            let conf_ok = opportunity.confidence_score >= at_cfg.min_radar_confidence;
                            let disc_ok = opportunity
                                .discrete_score
                                .as_ref()
                                .map(|d| d.total >= at_cfg.min_qanaliz_discrete_score)
                                .unwrap_or(false);
                            let sm_ok = opportunity
                                .smart_money_score
                                .as_ref()
                                .map(|s| s.total >= at_cfg.min_qanaliz_sm_score)
                                .unwrap_or(false);
                            dir_ok && conf_ok && disc_ok && sm_ok
                        };
                        if let Some(ref setup) = engine.compute_q_setup(&buffer, tf, symbol, q_radar) {
                            let is_long = matches!(setup.side, SignalType::Buy | SignalType::ChochBuy | SignalType::BosBuy);
                            if qanaliz_ok(is_long) {
                                all_signals.push(signal_from_q_setup(setup));
                            }
                        }

                        // Fake Breakout (liq sweep + reclaim + BOS) – LONG/SHORT
                        // Uses the same TF candles (conservative 2-candle pattern).
                        let fb_cfg = FakeBreakoutConfig {
                            lookback: at_cfg.fake_breakout_lookback,
                            bos_lookback: at_cfg.fake_breakout_bos_lookback,
                            min_wick_ratio: at_cfg.fake_breakout_min_wick_ratio,
                            sl_atr_mult: at_cfg.fake_breakout_sl_atr_mult,
                            tp_rr: at_cfg.fake_breakout_tp_rr,
                        };
                        if let Some(fb) = detect_fake_breakout_signal(&candles, false, fb_cfg) {
                            if qanaliz_ok(false) {
                                all_signals.push(iqai_core::auto_trader::TradeSignal {
                                    source: SignalSource::FakeBreakout,
                                    symbol: symbol.to_string(),
                                    timeframe: tf,
                                    is_long: false,
                                    entry: fb.entry,
                                    stop_loss: fb.stop_loss,
                                    take_profit: fb.take_profit,
                                    score: 75.0,
                                    rr: {
                                        let risk = (fb.entry - fb.stop_loss).abs();
                                        if risk > 1e-10 { (fb.take_profit - fb.entry).abs() / risk } else { 0.0 }
                                    },
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                });
                            }
                        }
                        if let Some(fb) = detect_fake_breakout_signal(&candles, true, fb_cfg) {
                            if qanaliz_ok(true) {
                                all_signals.push(iqai_core::auto_trader::TradeSignal {
                                    source: SignalSource::FakeBreakout,
                                    symbol: symbol.to_string(),
                                    timeframe: tf,
                                    is_long: true,
                                    entry: fb.entry,
                                    stop_loss: fb.stop_loss,
                                    take_profit: fb.take_profit,
                                    score: 75.0,
                                    rr: {
                                        let risk = (fb.entry - fb.stop_loss).abs();
                                        if risk > 1e-10 { (fb.take_profit - fb.entry).abs() / risk } else { 0.0 }
                                    },
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                });
                            }
                        }

                        // Elliott Wave setup sinyalleri
                        let config = &sm_config;
                        let elliott = compute_elliott(&candles, config, false);
                        if elliott.validation_ok == Some(true) {
                            if let Some(ref imp) = elliott.impulse_state {
                                if let Some(ref setup_json) = imp.setup_w3 {
                                    if let (Some(entry), Some(sl), Some(tp1)) = (
                                        setup_json.get("entry").and_then(|v| v.as_f64()),
                                        setup_json.get("stop_loss").and_then(|v| v.as_f64()),
                                        setup_json.get("tp1").and_then(|v| v.as_f64()),
                                    ) {
                                        let is_long = setup_json.get("is_long").and_then(|v| v.as_bool()).unwrap_or(imp.is_bullish);
                                        if qanaliz_ok(is_long) {
                                            all_signals.push(signal_from_elliott(
                                                SignalSource::ElliottW3, symbol, tf, is_long, entry, sl, tp1, 80.0,
                                            ));
                                        }
                                    }
                                }
                                if let Some(ref setup_json) = imp.setup_w5 {
                                    if let (Some(entry), Some(sl), Some(tp)) = (
                                        setup_json.get("entry").and_then(|v| v.as_f64()),
                                        setup_json.get("stop_loss").and_then(|v| v.as_f64()),
                                        setup_json.get("tp").and_then(|v| v.as_f64()),
                                    ) {
                                        let is_long = setup_json.get("is_long").and_then(|v| v.as_bool()).unwrap_or(imp.is_bullish);
                                        if qanaliz_ok(is_long) {
                                            all_signals.push(signal_from_elliott(
                                                SignalSource::ElliottW5, symbol, tf, is_long, entry, sl, tp, 75.0,
                                            ));
                                        }
                                    }
                                }
                            }
                            if let Some(ref corr) = elliott.corr_setup {
                                if qanaliz_ok(corr.is_long) {
                                    let source = if corr.setup_type.contains("Zigzag") {
                                        SignalSource::ZigzagC
                                    } else {
                                        SignalSource::TriangleE
                                    };
                                    all_signals.push(signal_from_elliott(
                                        source, symbol, tf, corr.is_long, corr.entry, corr.stop_loss, corr.tp, 70.0,
                                    ));
                                }
                            }
                        }

                        let map_key = format!("{}_{}", symbol, tf.to_binance_interval());
                        candles_map.insert(map_key, candles);
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("  {} {:?} veri hatası: {}", symbol, tf, e),
                }
            }
        }

        let logs = trader.full_tick(
            &all_signals,
            balance,
            &current_prices,
            &candles_map,
            &*exchange,
        ).await;

        let events = trader.drain_events();

        for log_entry in &logs {
            println!(
                "  📋 {} {} {} PnL={:+.2} ({:+.2}R) — {}",
                log_entry.source, log_entry.symbol, log_entry.side,
                log_entry.pnl, log_entry.pnl_r, log_entry.reason,
            );
        }

        if !events.is_empty() {
            println!("  📤 {} bildirim gönderiliyor...", events.len());
            if let Err(e) = notifier.notify_trade_events(&events).await {
                eprintln!("  Bildirim hatası: {}", e);
            }
        }

        let s = trader.summary();
        if s.total_trades > 0 {
            println!(
                "  📊 Özet: {} kapanan işlem ({} kar, {} zarar) | Win: %{:.0} | Toplam PnL: {:+.2} USDT",
                s.total_trades, s.winners, s.losers, s.win_rate, s.total_pnl,
            );
        }
        if trader.open_position_count() > 0 {
            println!("  📂 Açık pozisyonlar:");
            for (key, mp) in &trader.open_positions {
                let price = current_prices.get(&mp.signal.symbol).copied().unwrap_or(mp.position.entry_price);
                let unrealized = if mp.signal.is_long {
                    (price - mp.position.entry_price) * mp.position.quantity
                } else {
                    (mp.position.entry_price - price) * mp.position.quantity
                };
                let side_str = if mp.signal.is_long { "LONG" } else { "SHORT" };
                println!(
                    "     {} {} {} | TF: {} | Formasyon: {} | Giriş: {:.2} | SL: {:.2} | TP: {:.2} | Miktar: {:.6} | Anlık fiyat: {:.2} | PnL: {:+.2}",
                    key, mp.signal.symbol, side_str,
                    mp.signal.timeframe.to_binance_interval(),
                    mp.signal.source,
                    mp.position.entry_price,
                    mp.position.current_sl,
                    mp.position.initial_tp,
                    mp.position.quantity,
                    price,
                    unrealized,
                );
            }
        }
        if !events.is_empty() || !logs.is_empty() {
            println!(
                "  Durum: {} açık poz | {} işlem | Win: %{:.0} | PnL: {:+.2} | Günlük: {:+.2}",
                trader.open_position_count(), s.total_trades, s.win_rate, s.total_pnl, trader.daily_pnl(),
            );
        } else {
            println!(
                "  ⏳ {} açık poz | Günlük: {:+.2} | Bekleniyor...",
                trader.open_position_count(), trader.daily_pnl(),
            );
        }

        // Saat başı genel durum raporunu bildirim kanallarına gönder (bu tur verisi ile)
        let hour_key = (now_dt.date_naive(), now_dt.hour());
        if last_hourly_report.as_ref() != Some(&hour_key) {
            last_hourly_report = Some(hour_key);
            let s = trader.summary();
            let mode_str = format!("{:?}", trader.mode());
            let report = format!(
                "🕐 IQAI Robot — Saatlik özet ({})\n\
                 Mod: {} | Açık poz: {} | Kapanan işlem: {} ({} kar, {} zarar)\n\
                 Win: %{:.0} | Toplam PnL: {:+.2} USDT | Günlük: {:+.2} USDT",
                now_dt.format("%Y-%m-%d %H:00"),
                mode_str,
                trader.open_position_count(),
                s.total_trades,
                s.winners,
                s.losers,
                s.win_rate,
                s.total_pnl,
                trader.daily_pnl(),
            );
            if let Err(e) = notifier.notify_text(&report, None).await {
                eprintln!("  Saatlik rapor bildirimi atlandı: {:?}", e);
            } else {
                println!("  📤 Saatlik rapor bildirim kanallarına gönderildi.");
            }
            // Saat başı her açık pozisyon için CANLI POZİSYON kartı (PNG) gönder; pozisyon kapanana kadar tekrarlanır.
            for (_, mp) in &trader.open_positions {
                let price = current_prices
                    .get(&mp.signal.symbol)
                    .copied()
                    .unwrap_or(mp.position.entry_price);
                let side = if mp.signal.is_long { "LONG" } else { "SHORT" };
                let mode_str = format!("{}", trader.mode());
                if let Err(e) = notifier
                    .notify_live_position_card(
                        &mp.signal.symbol,
                        side,
                        &mode_str,
                        mp.position.entry_price,
                        price,
                        mp.position.current_sl,
                        mp.position.initial_tp,
                        mp.signal.score,
                        mp.signal.rr,
                    )
                    .await
                {
                    eprintln!("  CANLI POZİSYON kartı gönderilemedi ({}): {:?}", mp.signal.symbol, e);
                } else {
                    println!("  📤 CANLI POZİSYON kartı gönderildi: {} {}", mp.signal.symbol, side);
                }
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
    }
}

async fn run_q_analiz_daemon(interval_secs: u64, verbose: bool) -> Result<()> {
    let app_cfg = iqai_core::AppConfig::load()
        .ok_or_else(|| anyhow::anyhow!("config.json bulunamadı"))?;
    let trading = app_cfg.trading.as_ref()
        .ok_or_else(|| anyhow::anyhow!("config.json'da \"trading\" bölümü yok"))?;
    let symbols: Vec<String> = trading.symbols.clone().unwrap_or_else(|| vec!["ETHUSDT".into(), "BTCUSDT".into()]);
    let tf_strings: Vec<String> = trading.timeframes.clone().unwrap_or_else(|| vec!["5m".into(), "15m".into(), "1h".into(), "4h".into()]);
    let timeframes: Vec<Timeframe> = tf_strings.iter().filter_map(|s| Timeframe::from_str(s)).collect::<Vec<_>>();
    if timeframes.is_empty() {
        anyhow::bail!("En az bir geçerli timeframe gerekli (5m, 15m, 1h, 4h)");
    }
    let db_path = trading.db_path.as_deref();
    let db = TradeDb::open(db_path).map_err(|e| anyhow::anyhow!("DB açılamadı: {}", e))?;
    let client = BinanceFuturesClient::new();
    let sm_config = Config::from_smart_money(app_cfg.smart_money.as_ref());
    let notifier = Notifier::from_env();
    let max_bars = app_cfg.data.and_then(|d| d.max_bars).unwrap_or(500);

    println!("📌 Q-Analiz Daemon başlatıldı");
    println!("   Aralık: {}s | Semboller: {:?} | TF: {:?}", interval_secs, symbols, timeframes);
    println!("   DB: {} | Tespitler kaydedilip Telegram'a gönderilecek.", db_path.unwrap_or("data/trades.db"));
    if let Some(ref ai) = app_cfg.ai {
        if ai.enabled == Some(true) {
            let base = ai.ollama_base_url.as_deref().unwrap_or("http://localhost:11434");
            let (ok, models) = ai::check_ollama(base).await;
            if ok {
                let model = ai.model.as_deref().unwrap_or("llama2");
                let has_model = models.iter().any(|m| m.starts_with(model) || model.starts_with(m.split(':').next().unwrap_or("")));
                println!("   AI (Ollama): {} | Model: {} | Yüklü modeller: {:?}", if has_model { "✓" } else { "?" }, model, models);
                if !has_model && !models.is_empty() {
                    println!("   Uyarı: '{}' modeli listede yok. ollama pull {} ile yükleyin.", model, model);
                } else if !has_model {
                    println!("   Uyarı: Ollama'da model yok. ollama pull {} ile yükleyin.", model);
                }
            } else {
                println!("   AI (Ollama): ✗ Erişilemiyor ({}). AI yorum atlanacak. ollama serve ile başlatın.", base);
            }
        }
    }
    println!();

    let mut round: u64 = 0;
    loop {
        round += 1;
        let now = chrono::Utc::now().format("%H:%M:%S");
        println!("──── Q-Analiz tur {} | {} ────", round, now);
        for symbol in &symbols {
            let mut buffer = CandleBuffer::new();
            for &tf in &timeframes {
                if let Ok(candles) = client.fetch_klines(symbol, tf, max_bars).await {
                    if !candles.is_empty() {
                        buffer.update(tf, candles);
                    }
                }
            }
            let engine = SignalEngine::new(sm_config.clone());
            for &tf in &timeframes {
                let opp = compute_q_radar_opportunity(&buffer, tf, symbol, &sm_config);
                let candles: &[iqai_core::Candle] = buffer.get(tf).map(|v| &v[..]).unwrap_or(&[]);
                let mut snapshot = build_analysis_snapshot(&opp, candles, &sm_config);
                let side = match snapshot.scenario_direction.as_deref() {
                    Some("LONG") => Some(SignalType::Buy),
                    Some("SHORT") => Some(SignalType::Sell),
                    _ => None,
                };
                let entry = snapshot.scenario_entry.or_else(|| if snapshot.reference_price > 0.0 { Some(snapshot.reference_price) } else { None });
                if let Some(m) = engine.compute_position_metrics(&buffer, tf, symbol, side, entry, snapshot.scenario_stop, snapshot.scenario_tp1) {
                    snapshot.position_state = Some(m.position_state.clone());
                    snapshot.market_mode = Some(m.market_mode);
                    snapshot.local_trend = Some(m.local_trend);
                    snapshot.global_trend = Some(m.global_trend);
                    snapshot.volatility_pct = Some(m.volatility_pct);
                    snapshot.momentum_short = Some(m.momentum_short);
                    snapshot.momentum_long = Some(m.momentum_long);
                    snapshot.rr = Some(m.rr);
                    snapshot.tmr_trend_points = Some(m.tmr_scores.trend_points as i32);
                    snapshot.tmr_momentum_points = Some(m.tmr_scores.momentum_points as i32);
                    snapshot.tmr_rr_points = Some(m.tmr_scores.rr_points as i32);
                    snapshot.tmr_strength_points = Some(m.tmr_scores.strength_points as i32);
                    snapshot.trend_exhaustion = Some(m.trend_exhaustion);
                    snapshot.structure_shift = Some(m.structure_shift);
                    snapshot.position_side = Some(m.position_state.clone());
                }
                if let Err(e) = db.upsert_analysis_snapshot(&snapshot) {
                    eprintln!("   Snapshot DB hatası {} {}: {}", symbol, tf.to_binance_interval(), e);
                }
                if verbose {
                    println!(
                        "   · {} {} | {} | {} | Güven {:.1}/10 | Erken {:.1}/10",
                        symbol,
                        tf.to_binance_interval(),
                        if opp.detection.is_empty() { "—" } else { opp.detection.as_str() },
                        if opp.recommendation.is_empty() { "—" } else { opp.recommendation.as_str() },
                        opp.confidence_score,
                        opp.early_warning_score
                    );
                }
                if !opp.detection.is_empty() && opp.detection != "—" {
                    let is_long_ctx = opp.direction == "LONG";
                    let elliott_summary = buffer.get(tf).and_then(|candles| {
                        let min_bars = (sm_config.pivot_length as usize) * 4 + 2;
                        if candles.len() < min_bars {
                            None
                        } else {
                            // Elliott bağlamı, strateji senaryoları ve Smart Money context'ini üret.
                            let elliott = compute_elliott(candles, &sm_config, false);
                            let scenarios = build_scenarios_for_series(symbol, tf, candles, &sm_config);
                            let sm_ctx = build_smart_money_context_for_series(symbol, tf, candles, &sm_config);

                            let mut lines = Vec::new();
                            if !elliott.formation.is_empty() && elliott.formation != "—" {
                                lines.push(format!("Elliott: {} ({})", elliott.formation, elliott.formation_type));
                                if let Some(ref msg) = elliott.validation_msg {
                                    if !msg.is_empty() {
                                        lines.push(format!("  {}", msg));
                                    }
                                }
                                if let Some(ref next) = elliott.next_formation_ref {
                                    if !next.expected_formations.is_empty() {
                                        lines.push(format!("  Sonra beklenen: {}", next.expected_formations.join(", ")));
                                    }
                                }
                            }

                            // Senaryolardan en güçlü primary/alternative'ı seçip özet çıkar (AI ve Telegram için).
                            if let Some(best_scn) = scenarios
                                .iter()
                                .max_by(|a, b| {
                                    let aq = a.plans.first().map(|p| p.q_score).unwrap_or(0.0);
                                    let bq = b.plans.first().map(|p| p.q_score).unwrap_or(0.0);
                                    aq.partial_cmp(&bq).unwrap_or(std::cmp::Ordering::Equal)
                                })
                            {
                                if let Some(best) = best_scn.plans.first() {
                                    let dir = match best.direction {
                                        iqai_core::StrategyDirection::Long => "LONG",
                                        iqai_core::StrategyDirection::Short => "SHORT",
                                    };
                                    lines.push(format!(
                                        "Strateji ({}): {} {} @ {:.2} SL {:.2}",
                                        format!("{:?}", best_scn.role),
                                        dir,
                                        best.timeframe.to_binance_interval(),
                                        best.entry,
                                        best.stop_loss
                                    ));
                                    if !best.targets.is_empty() {
                                        let tps: Vec<String> = best.targets
                                            .iter()
                                            .take(3)
                                            .map(|t| format!("{} {:.2}", t.label, t.price))
                                            .collect();
                                        lines.push(format!("  Hedefler: {}", tps.join(", ")));
                                    }
                                    if let Some(ref lbl) = best.classic_pattern_label {
                                        lines.push(format!("  Klasik formasyon: {}", lbl));
                                    }
                                    if let Some(ref ef) = best.elliott_formation {
                                        lines.push(format!("  Elliott formasyon: {}", ef));
                                    }
                                }
                            }

                            // Smart Money / likidite özeti (yöne göre önceliklendirilir).
                            if let Some(ctx) = sm_ctx {
                                if !ctx.liquidity_levels.is_empty() {
                                    let mut lvls_all: Vec<String> = Vec::new();
                                    // LONG: Equal lows / Previous low; SHORT: Equal highs / Previous high
                                    let preferred = if is_long_ctx {
                                        |k: iqai_core::LiquidityKind| matches!(k, iqai_core::LiquidityKind::EqualLows | iqai_core::LiquidityKind::PreviousLow)
                                    } else {
                                        |k: iqai_core::LiquidityKind| matches!(k, iqai_core::LiquidityKind::EqualHighs | iqai_core::LiquidityKind::PreviousHigh)
                                    };
                                    for l in ctx.liquidity_levels.iter().filter(|l| preferred(l.kind)).take(3) {
                                        lvls_all.push(format!("{} @ {:.2}", l.label, l.price));
                                    }
                                    // Fallback: genel ilk seviyeler
                                    if lvls_all.is_empty() {
                                        for l in ctx.liquidity_levels.iter().take(3) {
                                            lvls_all.push(format!("{} @ {:.2}", l.label, l.price));
                                        }
                                    }
                                    lines.push(format!("Likidite: {}", lvls_all.join(", ")));
                                }
                                if !ctx.order_blocks.is_empty() {
                                    // LONG: Bullish OB; SHORT: Bearish OB. Karşıt OB varsa ayrı satır.
                                    let want_bullish = is_long_ctx;
                                    let mut primary: Vec<String> = ctx
                                        .order_blocks
                                        .iter()
                                        .filter(|z| matches!(z.side, iqai_core::OrderBlockSide::Bullish) == want_bullish)
                                        .take(2)
                                        .map(|z| format!("{} [{:.2}-{:.2}]", z.label, z.low, z.high))
                                        .collect();
                                    if primary.is_empty() {
                                        primary = ctx
                                            .order_blocks
                                            .iter()
                                            .take(2)
                                            .map(|z| format!("{} [{:.2}-{:.2}]", z.label, z.low, z.high))
                                            .collect();
                                    }
                                    lines.push(format!("Order Blocks: {}", primary.join(", ")));

                                    let counter: Vec<String> = ctx
                                        .order_blocks
                                        .iter()
                                        .filter(|z| matches!(z.side, iqai_core::OrderBlockSide::Bullish) != want_bullish)
                                        .take(1)
                                        .map(|z| format!("{} [{:.2}-{:.2}]", z.label, z.low, z.high))
                                        .collect();
                                    if !counter.is_empty() {
                                        lines.push(format!("Karşıt OB: {}", counter.join(", ")));
                                    }
                                }
                                if !ctx.fair_value_gaps.is_empty() {
                                    // LONG: Bullish FVG; SHORT: Bearish FVG
                                    let want_bullish = is_long_ctx;
                                    let mut fvgs: Vec<String> = ctx
                                        .fair_value_gaps
                                        .iter()
                                        .filter(|z| z.bullish == want_bullish)
                                        .take(2)
                                        .map(|z| format!("{} [{:.2}-{:.2}]", z.label, z.lower, z.upper))
                                        .collect();
                                    if fvgs.is_empty() {
                                        fvgs = ctx
                                            .fair_value_gaps
                                            .iter()
                                            .take(2)
                                            .map(|z| format!("{} [{:.2}-{:.2}]", z.label, z.lower, z.upper))
                                            .collect();
                                    }
                                    lines.push(format!("FVG: {}", fvgs.join(", ")));
                                }
                                if !ctx.wyckoff_tags.is_empty() {
                                    let w: Vec<String> = ctx
                                        .wyckoff_tags
                                        .iter()
                                        .take(2)
                                        .map(|t| format!("{} @ {:.2}", t.label, t.price))
                                        .collect();
                                    lines.push(format!("Wyckoff: {}", w.join(", ")));
                                }
                                lines.push(format!("PO3 fazı: {:?}", ctx.po3_phase));
                            }

                            if lines.is_empty() {
                                None
                            } else {
                                Some(lines.join("\n"))
                            }
                        }
                    });
                    if let Err(e) = db.insert_q_analiz_detection(&opp) {
                        eprintln!("   DB yazma hatası {} {}: {}", symbol, tf.to_binance_interval(), e);
                    } else {
                        println!(
                            "   ✓ {} {} | {} | {}",
                            symbol,
                            tf.to_binance_interval(),
                            opp.detection,
                            opp.recommendation
                        );
                        // Skor breakdown (debug için): hangi sinyal kaç puan verdi?
                        if let Some(ref ds) = opp.discrete_score {
                            println!("      Skor: {}/10 | {}", ds.total, ds.recommendation);
                            for s in &ds.signals {
                                if s.active {
                                    println!("        {}: +{}", s.name, s.points);
                                }
                            }
                        }
                        // Smart Money Radar breakdown: OB, FVG, likidite, Wyckoff, PO3
                        if let Some(ref sm) = opp.smart_money_score {
                            println!("      SM Skor: {}/10 | {}", sm.total, sm.recommendation);
                            for s in &sm.signals {
                                if s.active {
                                    println!("        [SM] {}: +{}", s.name, s.points);
                                }
                            }
                        }
                        let mut extra = elliott_summary.clone().unwrap_or_default();
                        if let Some(ref ai_cfg) = app_cfg.ai {
                            if ai_cfg.enabled == Some(true) {
                                let base = ai_cfg.ollama_base_url.as_deref().unwrap_or("http://localhost:11434");
                                let model = ai_cfg.model.as_deref().unwrap_or("llama2");

                                // Q-Analiz bloğu
                                let q_block = format!(
                                    "[Q-Analiz]\nTespit: {}\nYön: {}\nTavsiye: {}\nGüven: {:.1}/10\nErken uyarı: {:.1}/10\nFiyat: {:.4}{}",
                                    opp.detection,
                                    opp.direction,
                                    opp.recommendation,
                                    opp.confidence_score,
                                    opp.early_warning_score,
                                    opp.reference_price,
                                    opp.confirmation_layers
                                        .as_deref()
                                        .map(|c| format!("\nOnay: {}", c))
                                        .unwrap_or_default(),
                                );

                                // Dip/tepe skor bloğu
                                let discrete_part = opp.discrete_score.as_ref().map(|ds| {
                                    let active: Vec<&str> = ds
                                        .signals
                                        .iter()
                                        .filter(|s| s.active)
                                        .map(|s| s.name.as_str())
                                        .collect();
                                    let active_str: String =
                                        if active.is_empty() { "yok".to_string() } else { active.join(", ") };
                                    format!(
                                        "[Q-Analiz Skor]\nToplam: {}/10 | {}\nAktif sinyaller: {}",
                                        ds.total, ds.recommendation, active_str
                                    )
                                }).unwrap_or_default();

                                // Smart Money Radar bloğu
                                let sm_part = opp.smart_money_score.as_ref().map(|sm| {
                                    let active: Vec<&str> = sm
                                        .signals
                                        .iter()
                                        .filter(|s| s.active)
                                        .map(|s| s.name.as_str())
                                        .collect();
                                    let active_str: String =
                                        if active.is_empty() { "yok".to_string() } else { active.join(", ") };
                                    format!(
                                        "[Smart Money Radar]\nToplam: {}/10 | {}\nAktif SM sinyaller: {}",
                                        sm.total, sm.recommendation, active_str
                                    )
                                }).unwrap_or_default();

                                // Q-Setup / Strateji bloğu – en güçlü senaryodan özet (isteğe bağlı)
                                let qsetup_part = buffer.get(tf).map(|candles_for_tf| {
                                    let strategies = iqai_core::build_strategies_for_series(
                                        symbol,
                                        tf,
                                        candles_for_tf,
                                        &sm_config,
                                    );
                                    if strategies.is_empty() {
                                        return String::new();
                                    }
                                    let mut best = strategies[0].clone();
                                    for p in &strategies {
                                        if p.q_score > best.q_score {
                                            best = p.clone();
                                        }
                                    }
                                    let targets: Vec<String> = best
                                        .targets
                                        .iter()
                                        .take(3)
                                        .map(|t| format!("{} @ {:.2}", t.label, t.price))
                                        .collect();
                                    format!(
                                        "[Q-Setup]\nYön: {:?}\nEntry: {:.2}\nSL: {:.2}\nTP'ler: {}\nQ-Score: {:.1}",
                                        best.direction,
                                        best.entry,
                                        best.stop_loss,
                                        if targets.is_empty() {
                                            "yok".to_string()
                                        } else {
                                            targets.join(", ")
                                        },
                                        best.q_score
                                    )
                                }).unwrap_or_default();

                                // Elliott / SMC ek açıklama bloğu
                                let elliott_part = if extra.is_empty() {
                                    String::new()
                                } else {
                                    format!("[Elliott/SMC]\n{}", extra)
                                };

                                let context = format!(
                                    "Sembol: {} | TF: {}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}",
                                    opp.symbol,
                                    tf.to_binance_interval(),
                                    q_block,
                                    discrete_part,
                                    sm_part,
                                    qsetup_part,
                                    elliott_part,
                                );

                                if let Some(ai_text) = ai::interpret_q_analysis(base, model, &context).await {
                                    println!("   🤖 AI: {}", ai_text.trim());
                                    if !extra.is_empty() {
                                        extra.push_str("\n\n");
                                    }
                                    extra.push_str("🤖 ");
                                    extra.push_str(&ai_text);
                                }
                            }
                        }
                        let extra_opt = if extra.is_empty() { None } else { Some(extra.as_str()) };
                        if let Err(e) = notifier.notify_q_analysis(&opp, extra_opt).await {
                            eprintln!("   Bildirim hatası: {:?}", e);
                        }
                    }
                }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
    }
}

/// "YYYY-MM-DD" -> (o gün 00:00:00 UTC ms, o gün 23:59:59.999 UTC ms)
fn parse_date_range(from: Option<&str>, to: Option<&str>) -> Result<(i64, i64)> {
    fn day_start_ms(s: &str) -> Result<i64> {
        let d = NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("Tarih formatı YYYY-MM-DD olmalı ({}): {}", s, e))?;
        let ndt = d.and_hms_opt(0, 0, 0).unwrap();
        Ok(Utc.from_utc_datetime(&ndt).timestamp_millis())
    }
    fn day_end_ms(s: &str) -> Result<i64> {
        let d = NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("Tarih formatı YYYY-MM-DD olmalı ({}): {}", s, e))?;
        let ndt = d.and_hms_opt(23, 59, 59).unwrap();
        Ok(Utc.from_utc_datetime(&ndt).timestamp_millis() + 999)
    }
    let (start_ms, end_ms) = match (from, to) {
        (Some(f), Some(t)) => (day_start_ms(f)?, day_end_ms(t)?),
        (Some(f), None) => (day_start_ms(f)?, Utc::now().timestamp_millis()),
        (None, Some(t)) => {
            let end = day_end_ms(t)?;
            let from_d = NaiveDate::parse_from_str(t, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("Tarih formatı YYYY-MM-DD olmalı ({}): {}", t, e))?;
            let start_d = from_d - chrono::Duration::days(365);
            let ndt = start_d.and_hms_opt(0, 0, 0).unwrap();
            (Utc.from_utc_datetime(&ndt).timestamp_millis(), end)
        }
        (None, None) => return Err(anyhow::anyhow!("Tarih aralığı için --from veya --to gerekir")),
    };
    if start_ms >= end_ms {
        anyhow::bail!("--from tarihi --to tarihinden önce olmalı");
    }
    Ok((start_ms, end_ms))
}

async fn run_backtest_cmd(
    symbol: &str,
    market: &str,
    timeframe: &str,
    limit: u32,
    from: Option<&str>,
    to: Option<&str>,
    capital: f64,
    risk_pct: f64,
    leverage: f64,
    q_score_min: f64,
) -> Result<()> {
    let chart_tf = Timeframe::from_str(timeframe)
        .ok_or_else(|| anyhow::anyhow!("Geçersiz timeframe: {} (5M, 15M, 1H, 4H, D)", timeframe))?;
    let binance_interval = chart_tf.to_binance_interval();

    let candles = if from.is_some() || to.is_some() {
        let (start_ms, end_ms) = parse_date_range(from, to)?;
        println!(
            "📥 Geçmiş veri çekiliyor: {} {} ({} — {} tarih aralığı)...",
            symbol,
            timeframe,
            from.unwrap_or("?"),
            to.unwrap_or("bugün")
        );
        if market.eq_ignore_ascii_case("spot") {
            BinanceSpotClient::new()
                .fetch_klines_range(symbol, &binance_interval, start_ms, end_ms)
                .await
                .map_err(|e| anyhow::anyhow!("Klines hatası: {}", e))?
        } else {
            BinanceFuturesClient::new()
                .fetch_klines_range(symbol, &binance_interval, start_ms, end_ms)
                .await
                .map_err(|e| anyhow::anyhow!("Klines hatası: {}", e))?
        }
    } else {
        let client: Box<dyn ExchangeConnector> = if market.eq_ignore_ascii_case("spot") {
            Box::new(BinanceSpotClient::new())
        } else {
            Box::new(BinanceFuturesClient::new())
        };
        println!("📥 Geçmiş veri çekiliyor: {} {} ({} bar)...", symbol, timeframe, limit);
        client
            .fetch_klines(symbol, chart_tf, limit)
            .await
            .map_err(|e| anyhow::anyhow!("Klines hatası: {}", e))?
    };
    if candles.len() < 100 {
        anyhow::bail!("Yetersiz veri: {} bar. En az ~100 bar gerekir.", candles.len());
    }
    let app_cfg = iqai_core::AppConfig::load();
    let mut config = Config::from_smart_money(app_cfg.as_ref().and_then(|c| c.smart_money.as_ref()));
    config.q_score_threshold = q_score_min;
    println!("🔄 Q-Analiz backtest çalıştırılıyor (sermaye: {:.0}, risk: %{}, kaldıraç: {}, Q-score ≥ {:.0})...", capital, risk_pct, leverage, q_score_min);
    let result = run_backtest(
        &candles,
        &config,
        chart_tf,
        symbol,
        capital,
        risk_pct,
        leverage,
    );
    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Q-ANALİZ BACKTEST SONUCU  |  {}  {}  ({} bar)", result.symbol, result.timeframe, result.bar_count);
    println!("═══════════════════════════════════════════════════════════");
    println!("  Başlangıç sermayesi  : {:>12.2} USDT", result.initial_capital);
    println!("  Dönem sonu sermayesi : {:>12.2} USDT", result.final_capital);
    println!("  Toplam getiri       : {:>12.2} %", result.total_return_pct);
    println!("  Toplam PnL          : {:>+12.2} USDT", result.total_pnl);
    println!("  İşlem sayısı        : {:>12}", result.trades.len());
    println!("  Kazanan / Kaybeden  : {:>6} / {:>6}", result.win_count, result.loss_count);
    println!("  Kazanç oranı        : {:>12.1} %", result.win_rate_pct);
    println!("═══════════════════════════════════════════════════════════");
    if !result.trades.is_empty() {
        println!("  Son 10 işlem:");
        for t in result.trades.iter().rev().take(10) {
            println!(
                "    Bar {}→{} | {} | Giriş: {:.4} Çıkış: {:.4} | PnL: {:+.2} ({:+.2}R) | {}",
                t.entry_bar, t.exit_bar, t.side, t.entry_price, t.exit_price, t.pnl, t.pnl_r, t.exit_reason
            );
        }
    }
    println!();
    Ok(())
}

async fn run_ollama_check() -> Result<()> {
    let base = iqai_core::AppConfig::load()
        .and_then(|c| c.ai.and_then(|a| a.ollama_base_url))
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    println!("Ollama kontrol: {}", base);
    let (ok, models) = ai::check_ollama(&base).await;
    if ok {
        println!("  Durum: ✓ Erişilebilir");
        if models.is_empty() {
            println!("  Modeller: (liste alınamadı veya boş)");
        } else {
            println!("  Yüklü modeller:");
            for m in &models {
                println!("    - {}", m);
            }
        }
        let cfg_model = iqai_core::AppConfig::load()
            .and_then(|c| c.ai.and_then(|a| a.model))
            .unwrap_or_else(|| "llama2".to_string());
        let has = models.iter().any(|m| m == &cfg_model || m.starts_with(&format!("{}:", cfg_model)));
        if has {
            println!("  Config model '{}': ✓ mevcut", cfg_model);
        } else {
            println!("  Config model '{}': ✗ yüklü değil. ollama pull {} çalıştırın.", cfg_model, cfg_model);
        }
    } else {
        println!("  Durum: ✗ Erişilemiyor");
        println!("  Ollama kurulu değilse: https://ollama.com | Kuruluysa: ollama serve");
    }
    Ok(())
}

async fn run_trade(symbol: &str, side: &str, quantity: f64, market: &str) -> Result<()> {
    let app_cfg = iqai_core::AppConfig::load().unwrap_or_default();
    let tcfg = app_cfg.trading.unwrap_or_default();
    let api_key = tcfg
        .api_key
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("BINANCE_API_KEY").ok().filter(|s| !s.trim().is_empty()));
    let secret = tcfg
        .secret_key
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("BINANCE_SECRET_KEY").ok().filter(|s| !s.trim().is_empty()));
    let client: Box<dyn ExchangeConnector> = match (api_key, secret) {
        (Some(k), Some(s)) => {
            if market.eq_ignore_ascii_case("spot") {
                Box::new(BinanceSpotClient::with_credentials(k, s))
            } else {
                Box::new(BinanceFuturesClient::with_credentials(k, s))
            }
        }
        _ => anyhow::bail!(
            "Trade komutu için Binance API key gerekli (config.json trading.api_key/secret_key veya BINANCE_API_KEY/BINANCE_SECRET_KEY env)"
        ),
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
