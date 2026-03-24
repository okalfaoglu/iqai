//! Uygulama genelinde kullanılan config.json yapısı.
//! Bildirim, TV connector, watchlist yolu vb. ileride burada tutulabilir.
//!
//! **`app_kv` (SQLite)**: `trading.db_path` içinde `app_kv` tablosu ile aynı şemayı
//! dot-path anahtarlarla override edebilirsiniz (örn. `notification.throttle_q_setup_ms`).

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

use crate::trade_db::TradeDb;

/// Loglama hedefi: console (stderr), file (dosya), both (ikisi birden).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogTarget {
    #[default]
    Console,
    File,
    Both,
}

/// Loglama ayarları – config.json "logging" bölümü.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LoggingConfig {
    /// Log seviyesi: trace, debug, info, warn, error (varsayılan: info).
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Nereye yazılacak: console, file, both (varsayılan: console).
    #[serde(default)]
    pub target: LogTarget,
    /// Dosya yolu (target=file veya both iken kullanılır; yoksa iqai.log).
    pub file_path: Option<String>,
    /// TFAI-O07: `true` ise `/api/chart` içindeki TV/Binance TF döngüsü uyarıları **info** hedefinde loglanır (çok satır).
    /// `false` veya yok: aynı mesajlar yalnızca **`iqai_chart`** hedefinde **debug** (varsayılan `info` seviyesinde görünmez).
    #[serde(default)]
    pub verbose_chart_poll: Option<bool>,
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Bildirim kanalları için config.json içindeki "notification" bölümü.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct NotificationConfig {
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub whatsapp_webhook_url: Option<String>,
    pub whatsapp_webhook_token: Option<String>,
    pub instagram_webhook_url: Option<String>,
    pub instagram_webhook_token: Option<String>,
    pub facebook_webhook_url: Option<String>,
    pub facebook_webhook_token: Option<String>,
    pub x_webhook_url: Option<String>,
    pub x_webhook_token: Option<String>,
    pub email_webhook_url: Option<String>,
    pub email_webhook_token: Option<String>,
    /// Q-Setup bildirimi: aynı anahtar için minimum aralık (ms). Yoksa 30_000.
    pub throttle_q_setup_ms: Option<u64>,
    /// Q-Analiz panel bildirimi throttle (ms). Yoksa 30_000.
    pub throttle_q_analysis_ms: Option<u64>,
    /// Q-RADAR bildirimi throttle (ms). Yoksa 10_000.
    pub throttle_q_radar_ms: Option<u64>,
    /// Poz koruma bildirimi throttle (ms). Yoksa 30_000.
    pub throttle_protect_ms: Option<u64>,
}

/// Veri alma ayarları – bar derinliği, native timeframe modu vb.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DataConfig {
    /// Maksimum 1M bar sayısı (ör. 10_000). Yoksa varsayılan kullanılır.
    pub max_bars: Option<u32>,
    /// Artık kullanılmıyor: her zaman native TF (TV gibi). Eski config uyumluluğu için alan duruyor.
    pub native_tf_mode: Option<bool>,
}

/// Smart Money / Signal engine ayarları – Pine Script input'larına karşılık gelir.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SmartMoneyConfig {
    // General
    pub pivot_length: Option<u32>,
    pub momentum_threshold_base: Option<f64>,
    pub tp_points: Option<i32>,
    pub sl_points: Option<i32>,
    pub min_signal_distance: Option<u32>,
    pub tp_box_height_pct: Option<f64>,
    pub pre_momentum_factor_base: Option<f64>,
    pub short_trend_period: Option<u32>,
    pub long_trend_period: Option<u32>,

    // Signal Filters
    pub use_momentum_filter: Option<bool>,
    pub use_trend_filter: Option<bool>,
    pub higher_tf: Option<crate::types::Timeframe>,
    pub use_lower_tf_filter: Option<bool>,
    pub lower_tf: Option<crate::types::Timeframe>,
    pub use_volume_filter: Option<bool>,
    pub use_breakout_filter: Option<bool>,
    pub show_get_ready: Option<bool>,
    pub restrict_repeated_signals: Option<bool>,
    pub restrict_trend_tf: Option<crate::types::Timeframe>,

    // Advanced Analysis
    pub enable_liquidity_zones: Option<bool>,
    pub enable_market_profile: Option<bool>,
    pub enable_divergence_scanner: Option<bool>,
    pub enable_trend_analysis: Option<bool>,

    // Volume & Breakout
    pub volume_long_period: Option<u32>,
    pub volume_short_period: Option<u32>,
    pub breakout_period: Option<u32>,

    // Q-Analiz RSI eşikleri (Madde 4: klasik 30/70 veya yumuşak 35/65)
    /// Dip: RSI < bu değer ise aşırı satım (varsayılan 35; klasik 30)
    pub q_rsi_oversold: Option<f64>,
    /// Tepe: RSI > bu değer ise aşırı alım (varsayılan 65; klasik 70)
    pub q_rsi_overbought: Option<f64>,

    // ---- Candlestick pattern noise filter (Madde 9) ----
    /// Mum pattern gürültü filtresi: ATR periyodu (varsayılan 14).
    pub candlestick_noise_atr_period: Option<u32>,
    /// Son mum range / ATR minimum oranı (varsayılan 0.15).
    pub candlestick_noise_min_range_atr_ratio: Option<f64>,

    // ---- Q-RADAR confluence boost (q_radar_analysis) ----
    /// Confluence katmanı başına güven/erken uyarı artışı (varsayılan 0.6).
    pub q_confluence_boost_per_layer: Option<f64>,
    /// Maksimum toplam boost (varsayılan 2.5).
    pub q_confluence_boost_cap: Option<f64>,
    /// Q-RADAR `QRadarOpportunityAnalysis` içine Q-Setup + Elliott (varsayılan true).
    pub q_enrich_opportunity_with_setup_elliott: Option<bool>,

    // ---- Dip/tepe confluence (`dip_confluence.rs`) ----
    pub dip_confluence_mtf_atr_band: Option<f64>,
    pub dip_confluence_fib_price_band_pct: Option<f64>,
    pub dip_confluence_structure_score_min: Option<f64>,
    pub dip_confluence_absorption_atr_margin: Option<f64>,
    pub dip_confluence_absorption_bars: Option<u32>,
    pub dip_confluence_absorption_volume_ratio: Option<f64>,
    pub dip_confluence_absorption_vol_avg_bars: Option<u32>,
    pub dip_confluence_atr_period: Option<u32>,

    // ---- Dip/tepe discrete skor (`dip_tepe_scoring.rs`, Madde 15) ----
    pub dip_tepe_pts_rsi: Option<u8>,
    pub dip_tepe_pts_rsi_divergence: Option<u8>,
    pub dip_tepe_pts_macd_div: Option<u8>,
    pub dip_tepe_pts_support_zone: Option<u8>,
    pub dip_tepe_pts_volume_spike: Option<u8>,
    pub dip_tepe_pts_liquidity_sweep: Option<u8>,
    pub dip_tepe_pts_atr_filter: Option<u8>,
    pub dip_tepe_pts_vwap_mean_reversion: Option<u8>,
    pub dip_tepe_pts_bullish_candle: Option<u8>,
    pub dip_tepe_pts_fib_level: Option<u8>,
    pub dip_tepe_pts_ema200_near: Option<u8>,
    pub dip_tepe_pts_market_structure: Option<u8>,
    pub dip_tepe_pts_bollinger: Option<u8>,
    pub dip_tepe_pts_mean_reversion: Option<u8>,
    pub dip_tepe_score_cap: Option<u8>,

    pub dip_tepe_ma_period: Option<u32>,
    pub dip_tepe_vol_spike_mult: Option<f64>,
    pub dip_tepe_liquidity_lookback: Option<u32>,
    pub dip_tepe_swing_lookback: Option<u32>,
    pub dip_tepe_fib_band_pct: Option<f64>,
    pub dip_tepe_ema_near_dist_pct: Option<f64>,
    pub dip_tepe_structure_score_min: Option<f64>,
    pub dip_tepe_bollinger_period: Option<u32>,
    pub dip_tepe_bollinger_std: Option<f64>,
    pub dip_tepe_mean_rev_dist: Option<f64>,
    pub dip_tepe_vwap_mean_rev_dist: Option<f64>,
    pub dip_tepe_atr_vol_norm_min: Option<f64>,
    pub dip_tepe_atr_vol_norm_max: Option<f64>,
    pub dip_tepe_rsi_div_min_bars: Option<u32>,
    pub dip_tepe_macd_fast: Option<u32>,
    pub dip_tepe_macd_slow: Option<u32>,
    pub dip_tepe_macd_signal: Option<u32>,
    pub dip_tepe_rsi_period: Option<u32>,
    pub dip_tepe_ema_period: Option<u32>,
    pub dip_tepe_rec_strong_min: Option<u8>,
    pub dip_tepe_rec_buy_zone_min: Option<u8>,
    pub dip_tepe_rec_watch_min: Option<u8>,

    // ---- Dip/tepe reversal (`reversal.rs`, Doc §10/§14) ----
    pub reversal_atr_period: Option<u32>,
    pub reversal_margin_atr_up: Option<f64>,
    pub reversal_margin_atr_down: Option<f64>,
    pub reversal_strength_atr_full: Option<f64>,
    pub reversal_spring_recovery_bars: Option<u32>,
    pub reversal_weight_strength_atr: Option<f64>,
    pub reversal_weight_vol_ratio: Option<f64>,
    pub reversal_weight_body_ratio: Option<f64>,
    pub reversal_volume_ma_period: Option<u32>,

    // ---- Elliott Wave (görsel + fusion + potansiyel yol; `config.rs` / `elliott_fusion.rs`) ----
    pub elliott_fibo_gap_bars: Option<u32>,
    pub elliott_fibo_length_bars: Option<u32>,
    pub elliott_min_rr: Option<f64>,
    pub elliott_wave3_extension: Option<f64>,
    pub elliott_wave5_w1_multiple: Option<f64>,
    pub elliott_fib_tolerance_pct: Option<f64>,
    pub elliott_wave4_retrace_path: Option<f64>,
    pub elliott_projection_horizon_bars: Option<u32>,
    pub elliott_projection_segment_gap_bars: Option<u32>,
    pub elliott_ewo_fast: Option<u32>,
    pub elliott_ewo_slow: Option<u32>,
    pub elliott_ewo_signal: Option<u32>,
    pub elliott_ewo_strong_threshold: Option<f64>,
    pub elliott_require_ewo_alignment: Option<bool>,
    pub elliott_stability_min_wave_bars: Option<u32>,
    pub elliott_stability_confirm_bars: Option<u32>,
    pub elliott_stability_auto_invalidate_bars: Option<u32>,
    /// İtki/düzeltme iç-dalga 1:1 doğrulama — `config.elliott_subwave_strict`
    pub elliott_subwave_strict: Option<bool>,
    /// Tez `content.txt` §2.5.3–2.5.4 sayısal kuralları — `config.elliott_thesis_te_y_rules`
    #[serde(alias = "elliott_thesis_teY_rules")]
    pub elliott_thesis_te_y_rules: Option<bool>,
}

/// Otomatik trading ayarları – config.json "trading" bölümü.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TradingConfig {
    /// Otomatik trading açık/kapalı (güvenlik: default false)
    pub enabled: Option<bool>,
    /// Mod: "live" | "dry" | "paper"
    ///   live  – gerçek veri + gerçek emir + DB kaydı
    ///   dry   – gerçek veri + simüle emir + DB kaydı
    ///   paper – gerçek veri + simüle emir, DB kaydı yok (geçici test)
    pub mode: Option<String>,
    /// Piyasa: "futures" veya "spot"
    pub market: Option<String>,
    /// RCA / TFAI: strateji kimliği (örn. `iqai_default`). Kapalı pozisyon satırına yazılır.
    pub strategy_id: Option<String>,
    /// RCA: borsa kimliği (örn. `binance_futures`). Yoksa `market` alanından türetilir.
    pub exchange_id: Option<String>,
    /// Binance API key (live mod için zorunlu)
    pub api_key: Option<String>,
    /// Binance Secret key (live mod için zorunlu)
    pub secret_key: Option<String>,
    /// Trade veritabanı dosya yolu (varsayılan: data/trades.db)
    pub db_path: Option<String>,

    // ---- Risk yönetimi ----
    /// İşlem başına risk (hesap bakiyesinin yüzdesi, ör. 1.0 = %1)
    pub risk_per_trade_pct: Option<f64>,
    /// Aynı anda açık pozisyon limiti
    pub max_positions: Option<u32>,
    /// Futures maks kaldıraç (emir gönderirken kullanılır)
    pub max_leverage: Option<u32>,
    /// Günlük maks kayıp limiti (hesap bakiyesinin yüzdesi)
    pub daily_loss_limit_pct: Option<f64>,
    /// Min. Q-Score eşiği (bu altında sinyal işleme alınmaz)
    pub min_q_score: Option<f64>,
    /// Min. R:R oranı (bu altında sinyal işleme alınmaz)
    pub min_rr: Option<f64>,
    /// Q-RADAR ile sinyal filtrele: sadece RADAR yönü ile uyumlu sinyalleri al (default false)
    pub use_radar_filter: Option<bool>,
    /// RADAR filtre için min. güven skoru 0–10 (default 4.0)
    pub min_radar_confidence: Option<f64>,
    /// Q-Analiz dip/tepe discrete skor filtresi (0–10). 4 = WATCH ve üstü.
    pub min_qanaliz_discrete_score: Option<u8>,
    /// Smart Money Radar skor filtresi (0–10). 4 = SM WATCH ve üstü.
    pub min_qanaliz_sm_score: Option<u8>,

    // ---- Fake Breakout (liq sweep) setup ----
    /// Fake breakout: liquidity lookback (bar). Varsayılan: 40
    pub fake_breakout_lookback: Option<u32>,
    /// Fake breakout: BOS lookback (bar). Varsayılan: 6
    pub fake_breakout_bos_lookback: Option<u32>,
    /// Fake breakout: min wick ratio (0..1). Varsayılan: 0.35
    pub fake_breakout_min_wick_ratio: Option<f64>,
    /// Fake breakout: SL buffer ATR multiple. Varsayılan: 0.2
    pub fake_breakout_sl_atr_mult: Option<f64>,
    /// Fake breakout: fallback TP RR multiple. Varsayılan: 2.0
    pub fake_breakout_tp_rr: Option<f64>,
    /// İzlenen sembol listesi (ör. ["ETHUSDT", "BTCUSDT"])
    pub symbols: Option<Vec<String>>,
    /// İzlenen timeframe listesi (ör. ["5m", "15m", "1h"])
    pub timeframes: Option<Vec<String>>,
    /// Komisyon oranı (basis points, örn. 4 = %0.04). Yoksa Binance API'den çekilir; yine yoksa 4 kullanılır.
    pub commission_bps: Option<u32>,
    /// Binance komisyon oranını cache'lemek için TTL (ms).
    ///
    /// `0` ise her çağrıda anlık fetch edilir (signed USER_DATA endpoint).
    /// Varsayılan: 600_000 ms (~10 dk).
    pub commission_bps_cache_ttl_ms: Option<u64>,
    /// Kapanışta kayma (slippage) basis points (örn. 5 = %0.05). Simülasyonda daha gerçekçi fill.
    pub slippage_bps: Option<u32>,
    /// true ise piyasa emri yerine limit IOC kullanılır (maks kayma limit_slippage_bps ile sınırlı).
    pub use_limit_order: Option<bool>,
    /// Limit emirde izin verilen maks kayma (basis points, örn. 50 = %0.5). Sadece use_limit_order=true iken.
    pub limit_slippage_bps: Option<u32>,
}

impl TradingConfig {
    /// Risk ve eşik alanları için aralık kontrolü. `None` alanlar atlanır.
    /// Otomatik trader / CLI başlangıcında uyarı veya hata için kullanılabilir.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errs = Vec::new();
        if let Some(m) = &self.mode {
            let m = m.to_lowercase();
            if m != "live" && m != "dry" && m != "paper" {
                errs.push(format!(
                    "trading.mode: '{}' geçerli değil (live|dry|paper)",
                    m
                ));
            }
        }
        if let Some(v) = self.risk_per_trade_pct {
            if !v.is_finite() || v <= 0.0 || v > 100.0 {
                errs.push(format!(
                    "trading.risk_per_trade_pct: {} (0, 100] aralığında olmalı",
                    v
                ));
            }
        }
        if let Some(v) = self.max_positions {
            if v < 1 {
                errs.push(format!("trading.max_positions: {} >= 1 olmalı", v));
            }
        }
        if let Some(v) = self.max_leverage {
            if v < 1 || v > 125 {
                errs.push(format!(
                    "trading.max_leverage: {} 1–125 aralığında olmalı",
                    v
                ));
            }
        }
        if let Some(v) = self.daily_loss_limit_pct {
            if !v.is_finite() || v <= 0.0 || v > 100.0 {
                errs.push(format!(
                    "trading.daily_loss_limit_pct: {} (0, 100] aralığında olmalı",
                    v
                ));
            }
        }
        if let Some(v) = self.min_q_score {
            if !v.is_finite() || v < 0.0 || v > 100.0 {
                errs.push(format!(
                    "trading.min_q_score: {} [0, 100] aralığında olmalı",
                    v
                ));
            }
        }
        if let Some(v) = self.min_rr {
            if !v.is_finite() || v <= 0.0 {
                errs.push(format!("trading.min_rr: {} pozitif olmalı", v));
            }
        }
        if let Some(v) = self.min_radar_confidence {
            if !v.is_finite() || v < 0.0 || v > 10.0 {
                errs.push(format!(
                    "trading.min_radar_confidence: {} [0, 10] aralığında olmalı",
                    v
                ));
            }
        }
        if let Some(v) = self.min_qanaliz_discrete_score {
            if v > 10 {
                errs.push(format!(
                    "trading.min_qanaliz_discrete_score: {} 0–10 olmalı",
                    v
                ));
            }
        }
        if let Some(v) = self.min_qanaliz_sm_score {
            if v > 10 {
                errs.push(format!("trading.min_qanaliz_sm_score: {} 0–10 olmalı", v));
            }
        }
        if let Some(v) = self.fake_breakout_min_wick_ratio {
            if !v.is_finite() || v < 0.0 || v > 1.0 {
                errs.push(format!(
                    "trading.fake_breakout_min_wick_ratio: {} [0, 1] aralığında olmalı",
                    v
                ));
            }
        }
        if let Some(v) = self.fake_breakout_sl_atr_mult {
            if !v.is_finite() || v <= 0.0 {
                errs.push(format!(
                    "trading.fake_breakout_sl_atr_mult: {} pozitif olmalı",
                    v
                ));
            }
        }
        if let Some(v) = self.fake_breakout_tp_rr {
            if !v.is_finite() || v <= 0.0 {
                errs.push(format!(
                    "trading.fake_breakout_tp_rr: {} pozitif olmalı",
                    v
                ));
            }
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }
}

/// Q-Analiz tespitleri için AI yorumu (Ollama yerel).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AiConfig {
    /// true ise tespit olduğunda Ollama ile kısa yorum/tahmin istenir (Türkçe).
    pub enabled: Option<bool>,
    /// Ollama model adı (örn. llama2, mistral, qwen2).
    pub model: Option<String>,
    /// Ollama base URL (örn. http://localhost:11434).
    pub ollama_base_url: Option<String>,
    /// TFAI-O08: AI yanıtlarını `ai_explanations` tablosuna yaz. Varsayılan: true (alan yoksa yazar).
    #[serde(default)]
    pub persist_explanations: Option<bool>,
}

/// `iqai-web` HTTP (CORS vb.) — G-03.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct WebConfig {
    /// CORS: izin verilen `Origin` değerleri (örn. `http://localhost:5173`). Boş veya yok: **permissive** (geliştirme).
    pub cors_allow_origins: Option<Vec<String>>,
}

/// Tek config.json dosyası: notification, logging, trading vb.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AppConfig {
    /// Web sunucusu (CORS, ileride başlıklar).
    pub web: Option<WebConfig>,
    pub notification: Option<NotificationConfig>,
    /// Q-Analiz tespitinde AI yorumu (tepe/dipte ne var, sonra ne olabilir).
    pub ai: Option<AiConfig>,
    /// Loglama: seviye, hedef (console/file/both), dosya yolu.
    pub logging: Option<LoggingConfig>,
    /// Veri alma ayarları (bar limiti, native TF modu vb.).
    pub data: Option<DataConfig>,
    /// Smart Money Struktur motoru için ayarlar (opsiyonel).
    pub smart_money: Option<SmartMoneyConfig>,
    /// Otomatik trading ayarları.
    pub trading: Option<TradingConfig>,
    /// TradingView hesabı için kullanıcı adı (opsiyonel).
    pub tv_username: Option<String>,
    /// TradingView hesabı için şifre (opsiyonel).
    pub tv_password: Option<String>,
    /// TradingView 2FA TOTP secret (opsiyonel).
    pub tv_totp_secret: Option<String>,
    /// İsteğe bağlı: Hazır TradingView auth token (varsa direkt kullanılır).
    pub tradingview_auth_token: Option<String>,
}

impl AppConfig {
    /// Config dosyası aranacak sıra: IQAI_CONFIG env → ./config.json → ~/.config/iqai/config.json
    pub fn config_path() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("IQAI_CONFIG") {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
        let current = PathBuf::from("config.json");
        if current.exists() {
            return Some(current);
        }
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()?;
        let fallback = PathBuf::from(home).join(".config").join("iqai").join("config.json");
        if fallback.exists() {
            return Some(fallback);
        }
        None
    }

    /// Sadece config dosyasından yükle (DB override yok).
    pub fn load_file_only() -> Option<Self> {
        let path = Self::config_path()?;
        let contents = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&contents).ok()
    }

    /// `trades.db` içindeki `app_kv` ile birleştirilmiş config.
    /// Önce JSON, sonra `IQAI_TRADING_DB` env veya `trading.db_path` veya `data/trades.db` üzerinden DB.
    pub fn load_with_db_overrides() -> Option<Self> {
        let path = Self::config_path()?;
        let contents = std::fs::read_to_string(&path).ok()?;
        let mut v: serde_json::Value = serde_json::from_str(&contents).ok()?;

        let db_path = std::env::var("IQAI_TRADING_DB").ok().or_else(|| {
            v.get("trading")
                .and_then(|t| t.get("db_path"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        }).unwrap_or_else(|| "data/trades.db".to_string());

        if std::path::Path::new(&db_path).exists() {
            if let Ok(db) = TradeDb::open(Some(&db_path)) {
                if let Ok(rows) = db.load_app_kv() {
                    for (k, val_str) in rows {
                        let _ = merge_kv_into_config_value(&mut v, &k, &val_str);
                    }
                }
            }
        }
        serde_json::from_value(v).ok()
    }

    /// Varsayılan: `load_with_db_overrides()`; dosya yoksa None.
    pub fn load() -> Option<Self> {
        Self::load_with_db_overrides()
    }

    /// `trading` bölümü varsa `TradingConfig::validate` çalıştırır.
    pub fn validate_trading(&self) -> Result<(), Vec<String>> {
        match &self.trading {
            Some(t) => t.validate(),
            None => Ok(()),
        }
    }

    /// TFAI-O08: AI yanıtları `ai_explanations` tablosuna yazılsın mı (varsayılan: evet).
    pub fn ai_persist_explanations(&self) -> bool {
        self.ai
            .as_ref()
            .and_then(|a| a.persist_explanations)
            .unwrap_or(true)
    }

    /// TFAI-O07: `/api/chart` TF başına uyarıları info’da göster (varsayılan: hayır → yalnızca debug).
    pub fn logging_verbose_chart_poll(&self) -> bool {
        self.logging
            .as_ref()
            .and_then(|l| l.verbose_chart_poll)
            .unwrap_or(false)
    }
}

/// Dot-path ile JSON köküne değer yazar (`notification.throttle_q_setup_ms` gibi).
fn merge_kv_into_config_value(
    root: &mut serde_json::Value,
    path: &str,
    value_str: &str,
) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(value_str)
        .unwrap_or_else(|_| json!(value_str));
    let segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Err("app_kv: boş key".into());
    }
    let mut cur = root;
    for (i, seg) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            let obj = cur.as_object_mut().ok_or_else(|| {
                format!("app_kv: '{}' yolunda nesne değil", path)
            })?;
            obj.insert(seg.to_string(), value);
            return Ok(());
        }
        if !cur.is_object() {
            *cur = json!({});
        }
        let obj = cur
            .as_object_mut()
            .ok_or_else(|| format!("app_kv: '{}' kökü nesne değil", path))?;
        let next = obj
            .entry(seg.to_string())
            .or_insert_with(|| json!({}));
        cur = next;
    }
    Ok(())
}

#[cfg(test)]
mod app_config_helpers_tests {
    use super::AppConfig;

    #[test]
    fn logging_verbose_chart_poll_defaults_false() {
        let c = AppConfig::default();
        assert!(!c.logging_verbose_chart_poll());
    }

    #[test]
    fn ai_persist_explanations_defaults_true() {
        let c = AppConfig::default();
        assert!(c.ai_persist_explanations());
    }
}

#[cfg(test)]
mod trading_config_validate_tests {
    use super::TradingConfig;

    #[test]
    fn validate_accepts_defaults_empty() {
        let c = TradingConfig::default();
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_rejects_bad_risk_pct() {
        let c = TradingConfig {
            risk_per_trade_pct: Some(0.0),
            ..Default::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn validate_accepts_explicit_paper_mode() {
        let c = TradingConfig {
            mode: Some("paper".into()),
            ..Default::default()
        };
        assert!(c.validate().is_ok());
    }
}
