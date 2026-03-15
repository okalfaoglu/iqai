//! IQAI CLI - Trading bot command-line interface

use anyhow::Result;
use chrono::{Datelike, Timelike};
use clap::{Parser, Subcommand};
use iqai_binance::{BinanceFuturesClient, BinanceSpotClient};
use iqai_core::exchange::ExchangeConnector;
use iqai_tv::TvConnectorClient;
use iqai_web::chart_data::scan_elliott_formations;
use iqai_core::{
    compute_elliott,
    compute_q_radar_opportunity,
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
        Commands::QAnalizDaemon { interval } => run_q_analiz_daemon(interval).await?,
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
    }
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
        if let Err(e) = notifier.notify_q_setup(q).await {
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
        let api_key = trading_cfg.api_key.clone()
            .ok_or_else(|| anyhow::anyhow!("Live mod için api_key gerekli"))?;
        let secret = trading_cfg.secret_key.clone()
            .ok_or_else(|| anyhow::anyhow!("Live mod için secret_key gerekli"))?;
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
                        let radar_ok = |is_long: bool| {
                            if !at_cfg.use_radar_filter || opportunity.direction == "—" {
                                return true;
                            }
                            let dir_ok = (is_long && opportunity.direction == "LONG")
                                || (!is_long && opportunity.direction == "SHORT");
                            dir_ok && opportunity.confidence_score >= at_cfg.min_radar_confidence
                        };
                        if let Some(ref setup) = engine.compute_q_setup(&buffer, tf, symbol, q_radar) {
                            let is_long = matches!(setup.side, SignalType::Buy | SignalType::ChochBuy | SignalType::BosBuy);
                            if radar_ok(is_long) {
                                all_signals.push(signal_from_q_setup(setup));
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
                                        if radar_ok(is_long) {
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
                                        if radar_ok(is_long) {
                                            all_signals.push(signal_from_elliott(
                                                SignalSource::ElliottW5, symbol, tf, is_long, entry, sl, tp, 75.0,
                                            ));
                                        }
                                    }
                                }
                            }
                            if let Some(ref corr) = elliott.corr_setup {
                                if radar_ok(corr.is_long) {
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
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
    }
}

async fn run_q_analiz_daemon(interval_secs: u64) -> Result<()> {
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
    println!("   DB: {} | Tespitler kaydedilip Telegram'a gönderilecek.\n", db_path.unwrap_or("data/trades.db"));

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
            for &tf in &timeframes {
                let opp = compute_q_radar_opportunity(&buffer, tf, symbol, &sm_config);
                if !opp.detection.is_empty() && opp.detection != "—" {
                    if let Err(e) = db.insert_q_analiz_detection(&opp) {
                        eprintln!("   DB yazma hatası {} {}: {}", symbol, tf.to_binance_interval(), e);
                    } else {
                        println!("   ✓ {} {} | {} | {}", symbol, tf.to_binance_interval(), opp.detection, opp.recommendation);
                        if let Err(e) = notifier.notify_q_analysis(&opp).await {
                            eprintln!("   Bildirim hatası: {:?}", e);
                        }
                    }
                }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
    }
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
