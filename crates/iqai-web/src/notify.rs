use anyhow::Result;
use iqai_core::auto_trader::TradeEvent;
use iqai_core::{AppConfig, ProtectSignal, QRadarOpportunityAnalysis, QRadarSignal, QSetup};

use crate::q_analiz_card;
use crate::q_setup_card;
use crate::trade_open_card;
use crate::trade_close_card;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Bildirim dedup anahtarında kullanılacak fiyat: anlık fiyat her tick değiştiği için
/// throttle sürekli sıfırlanmasın diye ~0.5% bant içinde yuvarlanır.
fn dedup_price_bucket(p: f64) -> f64 {
    if !p.is_finite() || p <= 0.0 {
        return 0.0;
    }
    let band = (p * 0.005).max(p * 1e-6).max(0.01);
    (p / band).round() * band
}

/// In-memory throttle/dedup cache.
///
/// `/api/chart` canlı modda sık poll yaptığı için (örn. 2sn),
/// aynı tespit sürekli tekrar tekrar bildirim üretebilir.
/// Bu cache aynı event-key için kısa aralıkta tekrar gönderimi engeller.

fn throttle_cache() -> &'static Mutex<HashMap<String, u128>> {
    static CACHE: OnceLock<Mutex<HashMap<String, u128>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `true` = pencere içinde, gönderimi **atla** (zaman damgası değişmez).
pub fn throttle_should_skip(key: &str, min_interval_ms: u128) -> bool {
    let now = now_ms();
    let map = throttle_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&last) = map.get(key) {
        if now.saturating_sub(last) < min_interval_ms {
            return true;
        }
    }
    false
}

fn throttle_mark(key: &str) {
    let mut map = throttle_cache().lock().unwrap_or_else(|e| e.into_inner());
    map.insert(key.to_string(), now_ms());
}

/// Q-Analiz paneli için dedup anahtarı: **güven/erken uyarı** hariç (2/10 ↔ 3/10 titreşimi
/// throttle’ı delmesin). Sembol + TF + tespit + yön + tavsiye özeti + dip/tepe & SM skor toplamları.
pub fn q_analysis_throttle_key(opp: &QRadarOpportunityAnalysis) -> String {
    let tf = format!("{:?}", opp.timeframe);
    let rec_short = opp.recommendation.chars().take(48).collect::<String>();
    let ds = opp
        .discrete_score
        .as_ref()
        .map(|d| i32::from(d.total))
        .unwrap_or(-1);
    let sm = opp
        .smart_money_score
        .as_ref()
        .map(|s| i32::from(s.total))
        .unwrap_or(-1);
    format!(
        "QANALIZ:{}:{}:{}:{}:{}:{}:{}",
        opp.symbol, tf, opp.direction, opp.detection, rec_short, ds, sm
    )
}

fn throttled(key: &str, min_interval_ms: u128) -> bool {
    if throttle_should_skip(key, min_interval_ms) {
        return true;
    }
    throttle_mark(key);
    false
}

/// Bildirim olay tipi; routing_rules ile hangi kanallara gideceği belirlenir.
#[derive(Debug, Clone, Copy)]
pub enum NotificationEventType {
    QSetup,
    QRadar,
    /// Q-Analiz tam panel (Fiyat, YÖN, Tespit, Güven, Erken Uyarı, Tavsiye) – ekranla 1-1 aynı düzen.
    QAnalysis,
    Protect,
    Info,
    TradeSignal,
    TradeOpen,
    TradeClose,
    TradePartial,
    TradeSlUpdate,
    TradeDailySummary,
    ElliottSetup,
}

/// event_type'a göre hangi kanallara gönderileceği (ör. Q-RADAR sadece Telegram + X).
pub fn routing_rules(event_type: NotificationEventType) -> Vec<Channel> {
    match event_type {
        NotificationEventType::QSetup => vec![Channel::Telegram, Channel::WhatsApp, Channel::Instagram, Channel::Facebook, Channel::X, Channel::Email],
        NotificationEventType::QRadar => vec![Channel::Telegram, Channel::X],
        NotificationEventType::QAnalysis => vec![Channel::Telegram, Channel::WhatsApp, Channel::Instagram, Channel::Facebook, Channel::X, Channel::Email],
        NotificationEventType::Protect => vec![Channel::Telegram, Channel::WhatsApp, Channel::X, Channel::Email],
        NotificationEventType::Info => vec![Channel::Telegram, Channel::Email],
        NotificationEventType::TradeSignal => vec![Channel::Telegram],
        NotificationEventType::TradeOpen => vec![Channel::Telegram],
        NotificationEventType::TradeClose => vec![Channel::Telegram, Channel::Email],
        NotificationEventType::TradePartial => vec![Channel::Telegram],
        NotificationEventType::TradeSlUpdate => vec![Channel::Telegram],
        NotificationEventType::TradeDailySummary => vec![Channel::Telegram, Channel::Email],
        NotificationEventType::ElliottSetup => vec![Channel::Telegram],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q_analysis_throttle_key_ignores_confidence_jitter() {
        use iqai_core::QRadarOpportunityAnalysis;
        use iqai_core::Timeframe;
        let a = QRadarOpportunityAnalysis {
            symbol: "BTCUSDT".into(),
            timeframe: Timeframe::H1,
            radar: None,
            dip: None,
            peak: None,
            detection: "DİP".into(),
            confidence_score: 0.2,
            early_warning_score: 0.6,
            recommendation: "ZAYIF DİP – İzle".into(),
            confirmation_layers: None,
            direction: "LONG".into(),
            reference_price: 70_000.0,
            discrete_score: None,
            smart_money_score: None,
            confluence: None,
            q_setup: None,
            radar_setup_alignment: None,
            elliott_secondary_tp: None,
            elliott_summary: None,
            abc_correction_hint: None,
        };
        let mut b = a.clone();
        b.confidence_score = 0.3;
        b.early_warning_score = 0.7;
        assert_eq!(
            q_analysis_throttle_key(&a),
            q_analysis_throttle_key(&b),
            "güven/erken uyarı titreşimi anahtarı değiştirmemeli"
        );
    }

    #[test]
    fn routing_q_setup_includes_all_channels() {
        let ch = routing_rules(NotificationEventType::QSetup);
        assert!(ch.contains(&Channel::Telegram));
        assert!(ch.contains(&Channel::X));
        assert!(ch.contains(&Channel::Email));
        assert_eq!(ch.len(), 6);
    }

    #[test]
    fn routing_q_radar_only_telegram_and_x() {
        let ch = routing_rules(NotificationEventType::QRadar);
        assert_eq!(ch.len(), 2);
        assert!(ch.contains(&Channel::Telegram));
        assert!(ch.contains(&Channel::X));
    }

    #[test]
    fn routing_q_analysis_all_channels() {
        let ch = routing_rules(NotificationEventType::QAnalysis);
        assert_eq!(ch.len(), 6);
        assert!(ch.contains(&Channel::Telegram));
        assert!(ch.contains(&Channel::WhatsApp));
        assert!(ch.contains(&Channel::X));
        assert!(ch.contains(&Channel::Email));
    }

    #[test]
    fn routing_protect_has_telegram_whatsapp_x_email() {
        let ch = routing_rules(NotificationEventType::Protect);
        assert!(ch.contains(&Channel::Telegram));
        assert!(ch.contains(&Channel::WhatsApp));
        assert!(ch.contains(&Channel::X));
        assert!(ch.contains(&Channel::Email));
        assert_eq!(ch.len(), 4);
    }

    #[test]
    fn routing_info_telegram_and_email() {
        let ch = routing_rules(NotificationEventType::Info);
        assert_eq!(ch.len(), 2);
        assert!(ch.contains(&Channel::Telegram));
        assert!(ch.contains(&Channel::Email));
    }

    #[test]
    fn routing_trade_events_include_telegram() {
        for evt in [
            NotificationEventType::TradeSignal,
            NotificationEventType::TradeOpen,
            NotificationEventType::TradeClose,
            NotificationEventType::TradePartial,
            NotificationEventType::TradeSlUpdate,
            NotificationEventType::TradeDailySummary,
            NotificationEventType::ElliottSetup,
        ] {
            let ch = routing_rules(evt);
            assert!(ch.contains(&Channel::Telegram), "{:?} should include Telegram", evt);
        }
    }
}

/// Desteklenen bildirim kanalları.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Telegram,
    WhatsApp,
    Instagram,
    Facebook,
    X,
    Email,
}

/// Basit webhook tabanlı kanal yapılandırması (WhatsApp, Instagram, Facebook, X, Email).
#[derive(Clone)]
struct WebhookConfig {
    name: &'static str,
    url: String,
    token: Option<String>,
}

#[derive(Clone)]
struct TelegramConfig {
    bot_token: String,
    chat_id: String,
}

/// Bildirim kanalları için ortak arayüz.
///
/// - Telegram doğrudan Telegram Bot API ile konuşur.
/// - Diğer kanallar (WhatsApp, Instagram, Facebook, X, Email) için
///   ortam değişkenleri ile yapılandırılabilen webhook endpoint'leri kullanılır.
///   Örn. kendi backend'iniz bu webhook'u alıp ilgili platforma iletebilir.
#[derive(Clone)]
pub struct Notifier {
    telegram: Option<TelegramConfig>,
    whatsapp: Option<WebhookConfig>,
    instagram: Option<WebhookConfig>,
    facebook: Option<WebhookConfig>,
    x: Option<WebhookConfig>,
    email: Option<WebhookConfig>,
    throttle_q_setup_ms: u128,
    throttle_q_analysis_ms: u128,
    throttle_q_radar_ms: u128,
    throttle_protect_ms: u128,
}

impl Default for Notifier {
    fn default() -> Self {
        Self {
            telegram: None,
            whatsapp: None,
            instagram: None,
            facebook: None,
            x: None,
            email: None,
            throttle_q_setup_ms: 30_000,
            // Q-Analiz: varsayılan 5 dk (Telegram tekrarları)
            throttle_q_analysis_ms: 300_000,
            throttle_q_radar_ms: 10_000,
            throttle_protect_ms: 30_000,
        }
    }
}

impl Notifier {
    /// Telegram yapılandırıldı mı? (TELEGRAM_BOT_TOKEN + TELEGRAM_CHAT_ID veya config.json)
    pub fn has_telegram(&self) -> bool {
        self.telegram.is_some()
    }

    /// Q-Analiz bildirimi throttle penceresinde mi? (`true` = Telegram/Ollama için atla; AI çağrısını da atlamak için kullanın.)
    pub fn q_analysis_would_skip(&self, opp: &QRadarOpportunityAnalysis) -> bool {
        throttle_should_skip(&q_analysis_throttle_key(opp), self.throttle_q_analysis_ms)
    }

    /// Bildirim ayarlarını config.json + ortam değişkenlerinden oku.
    ///
    /// Önce `config.json` (IQAI_CONFIG env, ./config.json veya ~/.config/iqai/config.json)
    /// içindeki `notification` bölümüne bakılır; eksik alanlar env ile tamamlanır.
    /// Config dosyası yoksa sadece env kullanılır.
    ///
    /// Config örneği: `{ "notification": { "telegram_bot_token": "...", "telegram_chat_id": "..." } }`
    /// Env: `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`, `WHATSAPP_WEBHOOK_URL`, vb.
    pub fn from_env() -> Self {
        let cfg = AppConfig::load().and_then(|c| c.notification);

        let telegram = {
            let bot_token = cfg
                .as_ref()
                .and_then(|n| n.telegram_bot_token.clone())
                .filter(|s| !s.is_empty())
                .or_else(|| std::env::var("TELEGRAM_BOT_TOKEN").ok());
            let chat_id = cfg
                .as_ref()
                .and_then(|n| n.telegram_chat_id.clone())
                .filter(|s| !s.is_empty())
                .or_else(|| std::env::var("TELEGRAM_CHAT_ID").ok());
            match (bot_token, chat_id) {
                (Some(bot_token), Some(chat_id)) if !bot_token.is_empty() && !chat_id.is_empty() => {
                    Some(TelegramConfig { bot_token, chat_id })
                }
                _ => None,
            }
        };

        let mk_webhook = |name: &'static str,
                          url_key: &str,
                          token_key: &str,
                          cfg_url: Option<&String>,
                          cfg_token: Option<&String>|
         -> Option<WebhookConfig> {
            let url = cfg_url
                .cloned()
                .filter(|s| !s.is_empty())
                .or_else(|| std::env::var(url_key).ok());
            let token = cfg_token
                .cloned()
                .or_else(|| std::env::var(token_key).ok());
            url.filter(|u| !u.is_empty()).map(|u| WebhookConfig {
                name,
                url: u,
                token: token.filter(|t| !t.is_empty()),
            })
        };

        let n = cfg.as_ref();
        let throttle_q_setup_ms = n
            .and_then(|x| x.throttle_q_setup_ms)
            .unwrap_or(30_000) as u128;
        let throttle_q_analysis_ms = n
            .and_then(|x| x.throttle_q_analysis_ms)
            .unwrap_or(300_000) as u128;
        let throttle_q_radar_ms = n
            .and_then(|x| x.throttle_q_radar_ms)
            .unwrap_or(10_000) as u128;
        let throttle_protect_ms = n
            .and_then(|x| x.throttle_protect_ms)
            .unwrap_or(30_000) as u128;
        let whatsapp = mk_webhook(
            "WhatsApp",
            "WHATSAPP_WEBHOOK_URL",
            "WHATSAPP_WEBHOOK_TOKEN",
            n.and_then(|x| x.whatsapp_webhook_url.as_ref()),
            n.and_then(|x| x.whatsapp_webhook_token.as_ref()),
        );
        let instagram = mk_webhook(
            "Instagram",
            "INSTAGRAM_WEBHOOK_URL",
            "INSTAGRAM_WEBHOOK_TOKEN",
            n.and_then(|x| x.instagram_webhook_url.as_ref()),
            n.and_then(|x| x.instagram_webhook_token.as_ref()),
        );
        let facebook = mk_webhook(
            "Facebook",
            "FACEBOOK_WEBHOOK_URL",
            "FACEBOOK_WEBHOOK_TOKEN",
            n.and_then(|x| x.facebook_webhook_url.as_ref()),
            n.and_then(|x| x.facebook_webhook_token.as_ref()),
        );
        let x = mk_webhook(
            "X",
            "X_WEBHOOK_URL",
            "X_WEBHOOK_TOKEN",
            n.and_then(|x| x.x_webhook_url.as_ref()),
            n.and_then(|x| x.x_webhook_token.as_ref()),
        );
        let email = mk_webhook(
            "Email",
            "EMAIL_WEBHOOK_URL",
            "EMAIL_WEBHOOK_TOKEN",
            n.and_then(|x| x.email_webhook_url.as_ref()),
            n.and_then(|x| x.email_webhook_token.as_ref()),
        );

        Self {
            telegram,
            whatsapp,
            instagram,
            facebook,
            x,
            email,
            throttle_q_setup_ms,
            throttle_q_analysis_ms,
            throttle_q_radar_ms,
            throttle_protect_ms,
        }
    }

    /// Q-Analiz sonucunu routing_rules(QSetup) kanallarına gönder.
    pub async fn notify_q_setup(&self, setup: &QSetup) -> Result<()> {
        self.notify_q_setup_with_price(setup, None).await
    }

    /// Q-Setup bildirimini Telegram’da kart olarak gönder (opsiyonel current_price ile).
    pub async fn notify_q_setup_with_price(&self, setup: &QSetup, current_price: Option<f64>) -> Result<()> {
        // Q-Setup: aynı signal birkaç refresh boyunca tekrar gelebilir.
        let cp = current_price.unwrap_or(setup.entry);
        let tf = format!("{:?}", setup.timeframe);
        let side = format!("{:?}", setup.side);
        // Fiyatı anahtara ham koyma: her poll'da throttle bypass olur.
        let key = format!(
            "QSETUP:{}:{}:{}:{:.4}:{:.4}:{:.4}",
            setup.symbol,
            tf,
            side,
            dedup_price_bucket(cp),
            setup.stop_loss,
            setup.take_profit
        );
        if throttled(&key, self.throttle_q_setup_ms) {
            return Ok(());
        }

        let text = self.format_q_setup(setup);
        let channels = routing_rules(NotificationEventType::QSetup);
        let caption = format!("{} (Q) · Q-SETUP · {:.0}/100", setup.symbol, setup.q_score);
        match q_setup_card::render_q_setup_card(setup, current_price) {
            Ok(png_bytes) => {
                for ch in &channels {
                    match ch {
                        Channel::Telegram => {
                            if let Some(ref tg) = self.telegram {
                                if let Err(e) = self.notify_telegram_photo(tg, &png_bytes, &caption).await {
                                    let _ = self.notify_telegram(tg, &text).await;
                                    log::warn!("Telegram Q-Setup foto gönderilemedi, metin gönderildi: {:?}", e);
                                }
                            }
                        }
                        _ => {
                            self.send_to_channels(&text, vec![*ch], Some(setup)).await?;
                        }
                    }
                }
            }
            Err(_) => {
                self.send_to_channels(&text, channels, Some(setup)).await?;
            }
        }
        Ok(())
    }

    /// Q-Analiz tam panelini bildirim kanallarına gönder.
    /// Telegram’a ekteki gibi görsel kart (PNG) gönderilir; font yoksa veya hata olursa metin gider.
    pub async fn notify_q_analysis(
        &self,
        opp: &QRadarOpportunityAnalysis,
        elliott_summary: Option<&str>,
    ) -> Result<()> {
        let key = q_analysis_throttle_key(opp);
        if throttle_should_skip(&key, self.throttle_q_analysis_ms) {
            return Ok(());
        }
        throttle_mark(&key);

        let text = Self::format_q_analysis_panel(opp, elliott_summary);
        let channels = routing_rules(NotificationEventType::QAnalysis);
        let caption = format!("{} (Q) · {}", opp.symbol, opp.recommendation.as_str());
        match q_analiz_card::render_q_analiz_card(opp) {
            Ok(png_bytes) => {
                for ch in &channels {
                    match ch {
                        Channel::Telegram => {
                            if let Some(ref tg) = self.telegram {
                                if let Err(e) = self.notify_telegram_photo(tg, &png_bytes, &caption).await {
                                    let _ = self.notify_telegram(tg, &text).await;
                                    log::warn!("Telegram Q-Analiz foto gönderilemedi, metin gönderildi: {:?}", e);
                                }
                            }
                        }
                        _ => {
                            self.send_to_channels(&text, vec![*ch], None).await?;
                        }
                    }
                }
            }
            Err(_) => {
                self.send_to_channels(&text, channels, None).await?;
            }
        }
        Ok(())
    }

    fn format_q_analysis_panel(opp: &QRadarOpportunityAnalysis, elliott_summary: Option<&str>) -> String {
        let tf_str = format!("{:?}", opp.timeframe);
        let price = opp.reference_price;
        let direction = if opp.direction.is_empty() { "—" } else { opp.direction.as_str() };
        let detection = if opp.detection.is_empty() { "—" } else { opp.detection.as_str() };
        let conf_10 = (opp.confidence_score.round() as i32).clamp(0, 10) as u32;
        let early_10 = (opp.early_warning_score.round() as i32).clamp(0, 10) as u32;
        let early_label = if opp.direction == "LONG" {
            format!("DİP {}/10", early_10)
        } else if opp.direction == "SHORT" {
            format!("TEPE {}/10", early_10)
        } else {
            format!("{}/10", early_10)
        };
        let recommendation = if opp.recommendation.is_empty() { "—" } else { opp.recommendation.as_str() };
        let bar: String = (0..10).map(|i| if i < conf_10 { "█" } else { "░" }).collect();
        let mut s = format!(
            "📊 <b>{} (Q) · RADAR</b>\n\
             TF: {}\n\n\
             <b>Fiyat</b> {:.4}\n\
             <b>YÖN</b> {}\n\
             <b>Tespit</b> {}\n\
             <b>Güven (Radar)</b> {} {}/10\n\
             <b>Erken Uyarı</b> {}\n\
             <b>Tavsiye</b> {}",
            opp.symbol,
            tf_str,
            price,
            direction,
            detection,
            bar,
            conf_10,
            early_label,
            recommendation,
        );
        if let Some(ref ds) = opp.discrete_score {
            s.push_str("\n\n<b>Skor (Dip/Tepe · sinyal)</b> ");
            s.push_str(&ds.total.to_string());
            s.push_str("/10 · ");
            s.push_str(&ds.recommendation);
            if ds.early_warning_momentum {
                s.push_str(" · ⚡ Momentum dönüşü");
            }
            let active: Vec<&str> = ds.signals.iter().filter(|x| x.active).map(|x| x.name.as_str()).collect();
            if !active.is_empty() {
                s.push_str("\n• ");
                s.push_str(&active.join(", "));
            }
        }
        if let Some(ref sm) = opp.smart_money_score {
            s.push_str("\n\n<b>Skor (Smart Money Radar)</b> ");
            s.push_str(&sm.total.to_string());
            s.push_str("/10 · ");
            s.push_str(&sm.recommendation);
            let active: Vec<&str> = sm
                .signals
                .iter()
                .filter(|x| x.active)
                .map(|x| x.name.as_str())
                .collect();
            if !active.is_empty() {
                s.push_str("\n• ");
                s.push_str(&active.join(", "));
            }
        }
        if let Some(ew) = elliott_summary {
            if !ew.is_empty() {
                s.push_str("\n\n<b>Elliott</b>\n");
                s.push_str(ew);
            }
        }
        s
    }

    /// Q-RADAR erken uyarısını bildirim kanallarına gönder (routing: Telegram + X).
    pub async fn notify_q_radar(&self, radar: &QRadarSignal) -> Result<()> {
        let tf = format!("{:?}", radar.timeframe);
        let side = format!("{:?}", radar.side);
        let key = format!(
            "QRADAR:{}:{}:{}:{:.1}:{:.4}:{:?}",
            radar.symbol,
            tf,
            side,
            radar.confidence,
            dedup_price_bucket(radar.reference_price),
            radar.expected_window_bars
        );
        if throttled(&key, self.throttle_q_radar_ms) {
            return Ok(());
        }

        let side = match radar.side {
            iqai_core::SignalType::Buy => "LONG",
            iqai_core::SignalType::Sell => "SHORT",
            _ => "N/A",
        };
        let sl_line = radar.suggested_sl
            .map(|s| format!("Tahmini SL: {:.4}\n", s))
            .unwrap_or_else(|| String::new());
        let text = format!(
            "Q-RADAR (Erken uyarı)\n\
Sembol: {}\n\
Zaman Dilimi: {:?}\n\
Yön: {}\n\
Güven: {:.0}%\n\
Pencere: {}-{} bar\n\
Referans (izlenecek) fiyat: {:.4}\n\
{}Hedef: Q-Setup bekleniyor (giriş/SL/TP Q-ANALİZ'de belirlenir)",
            radar.symbol,
            radar.timeframe,
            side,
            radar.confidence * 100.0,
            radar.expected_window_bars.0,
            radar.expected_window_bars.1,
            radar.reference_price,
            sl_line,
        );
        self.send_to_channels(&text, routing_rules(NotificationEventType::QRadar), None).await
    }

    /// Poz Koruma uyarısını bildirim kanallarına gönder.
    pub async fn notify_protect(&self, protect: &ProtectSignal) -> Result<()> {
        let key = format!(
            "PROTECT:{}:{:.4}:{:.4}:{:.2}",
            protect.symbol,
            protect.entry_price,
            protect.trigger_price,
            protect.locked_r
        );
        if throttled(&key, self.throttle_protect_ms) {
            return Ok(());
        }

        let text = format!(
            "Q-ANALİZ POZ KORUMA\n\
Sembol: {}\n\
Giriş: {:.4}\n\
Kilitlenecek seviye: {:.4}\n\
Sebep: {}\n\
Kilitlenen R: {:.2}\n\n\
Kural: Poz Koruma geldiğinde çıkış zorunlu.",
            protect.symbol,
            protect.entry_price,
            protect.trigger_price,
            protect.reason,
            protect.locked_r,
        );
        self.send_to_channels(&text, routing_rules(NotificationEventType::Protect), None).await
    }

    /// Sadece belirtilen kanallara gönder (yapılandırılmış olanlar).
    pub async fn send_to_channels(
        &self,
        text: &str,
        channels: Vec<Channel>,
        setup: Option<&QSetup>,
    ) -> Result<()> {
        for ch in channels {
            match ch {
                Channel::Telegram => {
                    if let Some(ref tg) = self.telegram {
                        if let Err(e) = self.notify_telegram(tg, text).await {
                            return Err(e.into());
                        }
                    }
                }
                Channel::WhatsApp => {
                    if let Some(ref w) = self.whatsapp {
                        let _ = self.notify_webhook(w, text, setup).await;
                    }
                }
                Channel::Instagram => {
                    if let Some(ref ig) = self.instagram {
                        let _ = self.notify_webhook(ig, text, setup).await;
                    }
                }
                Channel::Facebook => {
                    if let Some(ref fb) = self.facebook {
                        let _ = self.notify_webhook(fb, text, setup).await;
                    }
                }
                Channel::X => {
                    if let Some(ref x) = self.x {
                        let _ = self.notify_webhook(x, text, setup).await;
                    }
                }
                Channel::Email => {
                    if let Some(ref mail) = self.email {
                        let _ = self.notify_webhook(mail, text, setup).await;
                    }
                }
            }
        }
        Ok(())
    }

    /// Genel amaçlı metin bildirimi – alım/satım emri, info mesajı vb.
    ///
    /// `setup` sağlanırsa, webhook payload'ına sembol/zaman dilimi gibi alanlar eklenir.
    pub async fn notify_text(&self, text: &str, setup: Option<&QSetup>) -> Result<()> {
        if let Some(ref tg) = self.telegram {
            if let Err(e) = self.notify_telegram(tg, text).await {
                eprintln!("Telegram notify failed: {:?}", e);
            }
        }
        if let Some(ref w) = self.whatsapp {
            if let Err(e) = self.notify_webhook(w, text, setup).await {
                eprintln!("WhatsApp notify failed: {:?}", e);
            }
        }
        if let Some(ref ig) = self.instagram {
            if let Err(e) = self.notify_webhook(ig, text, setup).await {
                eprintln!("Instagram notify failed: {:?}", e);
            }
        }
        if let Some(ref fb) = self.facebook {
            if let Err(e) = self.notify_webhook(fb, text, setup).await {
                eprintln!("Facebook notify failed: {:?}", e);
            }
        }
        if let Some(ref x) = self.x {
            if let Err(e) = self.notify_webhook(x, text, setup).await {
                eprintln!("X notify failed: {:?}", e);
            }
        }
        if let Some(ref mail) = self.email {
            if let Err(e) = self.notify_webhook(mail, text, setup).await {
                eprintln!("Email notify failed: {:?}", e);
            }
        }

        Ok(())
    }

    /// AutoTrader olaylarını Telegram'a (ve diğer kanallara) ilet.
    /// Pozisyon açıldı/kapandı için Telegram'a görsel kart gönderilir (font varsa).
    pub async fn notify_trade_event(&self, event: &TradeEvent) -> Result<()> {
        let (text, event_type) = Self::format_trade_event(event);
        let channels = routing_rules(event_type);

        let telegram_sent_photo = match event {
            TradeEvent::PositionOpened { signal, quantity: _, avg_price, mode } => {
                let side = if signal.is_long { "LONG" } else { "SHORT" };
                if let Some(ref tg) = self.telegram {
                    match trade_open_card::render_trade_open_card(
                        &signal.symbol,
                        side,
                        &mode.to_string(),
                        signal.entry,
                        *avg_price,
                        signal.stop_loss,
                        signal.take_profit,
                        signal.stop_loss,
                        signal.score,
                        signal.rr,
                    ) {
                        Ok(png_bytes) => {
                            let caption = format!("{} · {} @ {:.4}", signal.symbol, side, avg_price);
                            if self.notify_telegram_photo(tg, &png_bytes, &caption).await.is_ok() {
                                true
                            } else {
                                let _ = self.notify_telegram(tg, &text).await;
                                false
                            }
                        }
                        Err(_) => {
                            let _ = self.notify_telegram(tg, &text).await;
                            false
                        }
                    }
                } else {
                    false
                }
            }
            TradeEvent::PositionClosed { symbol, side, entry, exit, pnl, .. } => {
                let pnl_pct = if entry.abs() >= 1e-12 {
                    (exit - entry) / entry * 100.0
                } else {
                    0.0
                };
                if let Some(ref tg) = self.telegram {
                    match trade_close_card::render_trade_close_card(
                        symbol,
                        side,
                        *entry,
                        *exit,
                        pnl_pct,
                    ) {
                        Ok(png_bytes) => {
                            let caption = format!("{} · {} | PnL: {:+.2} ({:+.2}%)", symbol, side, pnl, pnl_pct);
                            if self.notify_telegram_photo(tg, &png_bytes, &caption).await.is_ok() {
                                true
                            } else {
                                let _ = self.notify_telegram(tg, &text).await;
                                false
                            }
                        }
                        Err(_) => {
                            let _ = self.notify_telegram(tg, &text).await;
                            false
                        }
                    }
                } else {
                    false
                }
            }
            _ => false,
        };

        let channels_to_send: Vec<Channel> = if telegram_sent_photo {
            channels.iter().filter(|c| **c != Channel::Telegram).copied().collect()
        } else {
            channels
        };
        self.send_to_channels(&text, channels_to_send, None).await
    }

    /// Açık pozisyon için CANLI POZİSYON kartını saat başı hatırlatma olarak gönderir (Telegram PNG).
    /// Pozisyon açıldığında değil, her saat başı pozisyon kapanana kadar tekrarlanacak çağrı için kullanılır.
    pub async fn notify_live_position_card(
        &self,
        symbol: &str,
        side: &str,
        mode: &str,
        entry: f64,
        current_price: f64,
        stop_loss: f64,
        take_profit: f64,
        protection_sl: f64,
        score: f64,
        rr: f64,
    ) -> Result<()> {
        if let Some(ref tg) = self.telegram {
            match trade_open_card::render_trade_open_card(
                symbol,
                side,
                mode,
                entry,
                current_price,
                stop_loss,
                take_profit,
                protection_sl,
                score,
                rr,
            ) {
                Ok(png_bytes) => {
                    let caption = format!("CANLI POZİSYON · {} · {}", symbol, side);
                    self.notify_telegram_photo(tg, &png_bytes, &caption).await
                }
                Err(_font_err) => {
                    // Font yok / PNG üretilemedi: Telegram HTML ile aynı içerik (kart benzeri).
                    let text = trade_open_card::format_live_position_html(
                        symbol,
                        side,
                        entry,
                        current_price,
                        stop_loss,
                        take_profit,
                        protection_sl,
                        score,
                        rr,
                    );
                    self.notify_telegram(tg, &text).await
                }
            }
        } else {
            Ok(())
        }
    }

    /// Birden fazla trade event'i toplu gönder.
    pub async fn notify_trade_events(&self, events: &[TradeEvent]) -> Result<()> {
        for event in events {
            if let Err(e) = self.notify_trade_event(event).await {
                log::error!("Trade event bildirim hatası: {:?}", e);
            }
        }
        Ok(())
    }

    fn format_trade_event(event: &TradeEvent) -> (String, NotificationEventType) {
        match event {
            TradeEvent::SignalReceived { signal, accepted, reason } => {
                let icon = if *accepted { "✅" } else { "❌" };
                let side = if signal.is_long { "LONG" } else { "SHORT" };
                let text = format!(
                    "{icon} <b>SİNYAL {status}</b>\n\
                    Kaynak: {source}\n\
                    Sembol: {sym} | TF: {tf}\n\
                    Yön: {side}\n\
                    Giriş: {entry:.4} | SL: {sl:.4} | TP: {tp:.4}\n\
                    RR: {rr:.2} | Skor: {score:.0}\n\
                    Durum: {reason}",
                    icon = icon,
                    status = if *accepted { "KABUL" } else { "RED" },
                    source = signal.source,
                    sym = html_escape(&signal.symbol),
                    tf = signal.timeframe.to_binance_interval(),
                    side = side,
                    entry = signal.entry,
                    sl = signal.stop_loss,
                    tp = signal.take_profit,
                    rr = signal.rr,
                    score = signal.score,
                    reason = html_escape(reason),
                );
                (text, NotificationEventType::TradeSignal)
            }
            TradeEvent::PositionOpened { signal, quantity, avg_price, mode } => {
                let side = if signal.is_long { "LONG" } else { "SHORT" };
                let text = format!(
                    "📈 <b>POZİSYON AÇILDI</b> [{mode}]\n\
                    Kaynak: {source}\n\
                    Sembol: {sym} | TF: {tf}\n\
                    Yön: {side}\n\
                    Giriş: {price:.4} | Miktar: {qty:.6}\n\
                    SL: {sl:.4} | TP: {tp:.4}\n\
                    RR: {rr:.2} | Skor: {score:.0}",
                    mode = mode,
                    source = signal.source,
                    sym = html_escape(&signal.symbol),
                    tf = signal.timeframe.to_binance_interval(),
                    side = side,
                    price = avg_price,
                    qty = quantity,
                    sl = signal.stop_loss,
                    tp = signal.take_profit,
                    rr = signal.rr,
                    score = signal.score,
                );
                (text, NotificationEventType::TradeOpen)
            }
            TradeEvent::PositionClosed { symbol, side, entry, exit, quantity, pnl, pnl_r, reason, source, mode } => {
                let icon = if *pnl >= 0.0 { "💰" } else { "🔻" };
                let text = format!(
                    "{icon} <b>POZİSYON KAPANDI</b> [{mode}]\n\
                    Kaynak: {source}\n\
                    Sembol: {sym} | Yön: {side}\n\
                    Giriş: {entry:.4} → Çıkış: {exit:.4}\n\
                    Miktar: {qty:.6}\n\
                    <b>PnL: {pnl:+.2} ({pnl_r:+.2}R)</b>\n\
                    Sebep: {reason}",
                    icon = icon,
                    mode = mode,
                    source = source,
                    sym = html_escape(symbol),
                    side = side,
                    entry = entry,
                    exit = exit,
                    qty = quantity,
                    pnl = pnl,
                    pnl_r = pnl_r,
                    reason = html_escape(reason),
                );
                (text, NotificationEventType::TradeClose)
            }
            TradeEvent::PartialClose { symbol, side, pct, price, reason, mode } => {
                let text = format!(
                    "✂️ <b>KISMİ KAPANIŞ</b> [{mode}]\n\
                    Sembol: {sym} | Yön: {side}\n\
                    Kapatılan: %{pct:.0} @ {price:.4}\n\
                    Sebep: {reason}",
                    mode = mode,
                    sym = html_escape(symbol),
                    side = side,
                    pct = pct * 100.0,
                    price = price,
                    reason = html_escape(reason),
                );
                (text, NotificationEventType::TradePartial)
            }
            TradeEvent::SlUpdated { symbol, old_sl, new_sl, reason } => {
                let text = format!(
                    "🛡️ <b>SL GÜNCELLENDİ</b>\n\
                    Sembol: {sym}\n\
                    Eski SL: {old:.4} → Yeni SL: {new:.4}\n\
                    Sebep: {reason}",
                    sym = html_escape(symbol),
                    old = old_sl,
                    new = new_sl,
                    reason = html_escape(reason),
                );
                (text, NotificationEventType::TradeSlUpdate)
            }
            TradeEvent::DailySummary { date, summary, mode } => {
                let text = format!(
                    "📊 <b>GÜNLÜK ÖZET</b> [{mode}] {date}\n\
                    İşlem: {total} | Kazanan: {win} | Kaybeden: {lose}\n\
                    Win Rate: %{wr:.1}\n\
                    <b>Toplam PnL: {pnl:+.2}</b>\n\
                    Ortalama R: {avg_r:+.2}\n\
                    Açık Pozisyon: {open}",
                    mode = mode,
                    date = date,
                    total = summary.total_trades,
                    win = summary.winners,
                    lose = summary.losers,
                    wr = summary.win_rate,
                    pnl = summary.total_pnl,
                    avg_r = summary.avg_r,
                    open = summary.open_positions,
                );
                (text, NotificationEventType::TradeDailySummary)
            }
            TradeEvent::ElliottSetup { symbol, timeframe, source, side, entry, stop_loss, take_profit, rr } => {
                let text = format!(
                    "🌊 <b>ELLİOTT SETUP</b>\n\
                    Kaynak: {source}\n\
                    Sembol: {sym} | TF: {tf}\n\
                    Yön: {side}\n\
                    Giriş: {entry:.4} | SL: {sl:.4} | TP: {tp:.4}\n\
                    RR: {rr:.2}",
                    source = source,
                    sym = html_escape(symbol),
                    tf = timeframe.to_binance_interval(),
                    side = side,
                    entry = entry,
                    sl = stop_loss,
                    tp = take_profit,
                    rr = rr,
                );
                (text, NotificationEventType::ElliottSetup)
            }
        }
    }

    fn format_q_setup(&self, setup: &QSetup) -> String {
        let side = match setup.side {
            iqai_core::SignalType::Buy => "LONG",
            iqai_core::SignalType::Sell => "SHORT",
            _ => "N/A",
        };

        format!(
            "Q-ANALİZ SETUP\n\
Sembol: {}\n\
Zaman Dilimi: {:?}\n\
Yön: {}\n\
Giriş: {:.4}\n\
TP: {:.4}\n\
SL: {:.4}\n\
Q Skoru: {:.1}\n\
Beklenen Süre: ~{} bar",
            setup.symbol,
            setup.timeframe,
            side,
            setup.entry,
            setup.take_profit,
            setup.stop_loss,
            setup.q_score,
            setup.expected_bars,
        )
    }

    async fn notify_telegram(&self, cfg: &TelegramConfig, text: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", cfg.bot_token);
        let body = serde_json::json!({
            "chat_id": cfg.chat_id,
            "text": text,
            "parse_mode": "HTML",
        });

        let client = reqwest::Client::new();
        let resp = client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Telegram API {}: {}", status, body_text));
        }
        Ok(())
    }

    /// Telegram’a foto gönder (sendPhoto). PNG bytes + isteğe bağlı caption.
    async fn notify_telegram_photo(
        &self,
        cfg: &TelegramConfig,
        png_bytes: &[u8],
        caption: &str,
    ) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendPhoto", cfg.bot_token);
        let part = reqwest::multipart::Part::bytes(png_bytes.to_vec())
            .file_name("q-analiz.png")
            .mime_str("image/png")?;
        let form = reqwest::multipart::Form::new()
            .text("chat_id", cfg.chat_id.clone())
            .part("photo", part)
            .text("caption", caption.to_string());
        let client = reqwest::Client::new();
        let resp = client.post(&url).multipart(form).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Telegram sendPhoto {}: {}", status, body_text));
        }
        Ok(())
    }

    /// Ortak webhook formatı:
    /// { "text": "...", "symbol": "...", "timeframe": "...", "side": "...", "q_score": ..., "expected_bars": ... }
    async fn notify_webhook(
        &self,
        cfg: &WebhookConfig,
        text: &str,
        setup: Option<&QSetup>,
    ) -> Result<()> {
        let mut body = serde_json::json!({
            "channel": cfg.name,
            "text": text,
        });

        if let Some(setup) = setup {
            let side = match setup.side {
                iqai_core::SignalType::Buy => "LONG",
                iqai_core::SignalType::Sell => "SHORT",
                _ => "N/A",
            };
            body["symbol"] = serde_json::Value::String(setup.symbol.clone());
            body["timeframe"] = serde_json::Value::String(format!("{:?}", setup.timeframe));
            body["side"] = serde_json::Value::String(side.to_string());
            body["q_score"] = serde_json::Value::from(setup.q_score);
            body["expected_bars"] = serde_json::Value::from(setup.expected_bars);
        }

        if let Some(token) = &cfg.token {
            body["token"] = serde_json::Value::String(token.clone());
        }

        let client = reqwest::Client::new();
        let resp = client.post(&cfg.url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            eprintln!("{} webhook notify failed: {} - {}", cfg.name, status, txt);
        }
        Ok(())
    }
}

