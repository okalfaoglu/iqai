//! Uygulama genelinde kullanılan config.json yapısı.
//! Bildirim, TV connector, watchlist yolu vb. ileride burada tutulabilir.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    /// İzlenen sembol listesi (ör. ["ETHUSDT", "BTCUSDT"])
    pub symbols: Option<Vec<String>>,
    /// İzlenen timeframe listesi (ör. ["5m", "15m", "1h"])
    pub timeframes: Option<Vec<String>>,
    /// Komisyon oranı (basis points, örn. 4 = %0.04). Yoksa Binance API'den çekilir; yine yoksa 4 kullanılır.
    pub commission_bps: Option<u32>,
    /// Kapanışta kayma (slippage) basis points (örn. 5 = %0.05). Simülasyonda daha gerçekçi fill.
    pub slippage_bps: Option<u32>,
    /// true ise piyasa emri yerine limit IOC kullanılır (maks kayma limit_slippage_bps ile sınırlı).
    pub use_limit_order: Option<bool>,
    /// Limit emirde izin verilen maks kayma (basis points, örn. 50 = %0.5). Sadece use_limit_order=true iken.
    pub limit_slippage_bps: Option<u32>,
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
}

/// Tek config.json dosyası: notification, logging, trading vb.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AppConfig {
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

    /// Dosyadan yükle; dosya yoksa veya parse hatası varsa None.
    pub fn load() -> Option<Self> {
        let path = Self::config_path()?;
        let contents = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&contents).ok()
    }
}
