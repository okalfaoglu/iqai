//! Binance REST hata cevaplarını TFAI-Q04 ile hizalı kategorilere ayırır (alert / retry mantığı).

use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

/// Borsa hatasının operasyonel sınıfı (TFAI-Q04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExchangeErrorCategory {
    RateLimit,
    AuthFailure,
    InsufficientFunds,
    InvalidOrder,
    MarketClosed,
    PositionRisk,
    ExchangeInternal,
    NetworkTransient,
    Unknown,
}

impl ExchangeErrorCategory {
    /// Prometheus etiketi (`snake_case`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RateLimit => "rate_limit",
            Self::AuthFailure => "auth_failure",
            Self::InsufficientFunds => "insufficient_funds",
            Self::InvalidOrder => "invalid_order",
            Self::MarketClosed => "market_closed",
            Self::PositionRisk => "position_risk",
            Self::ExchangeInternal => "exchange_internal",
            Self::NetworkTransient => "network_transient",
            Self::Unknown => "unknown",
        }
    }
}

/// Operasyonel uyarı önceliği — dış uyarı sistemi (PagerDuty, e-posta) eşlemesi için (TFAI-Q04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AlertTier {
    /// Hemen müdahale (ör. yetki / API anahtarı).
    Critical,
    /// Kısa sürede triage (oran limiti, borsa içi, ağ, bilinmeyen kod).
    Elevated,
    /// İzleme; çoğunlukla emir parametresi / piyasa durumu.
    Low,
}

impl AlertTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Elevated => "elevated",
            Self::Low => "low",
        }
    }
}

/// Normalize edilmiş Binance API hatası (log / metrik / alert için).
#[derive(Debug, Clone, thiserror::Error)]
#[error("[{exchange}] {category:?} code={raw_code} http={http_status} retryable={retryable} — {raw_message}")]
pub struct NormalizedExchangeError {
    pub exchange: String,
    pub raw_code: i32,
    pub raw_message: String,
    pub category: ExchangeErrorCategory,
    pub retryable: bool,
    pub retry_after_ms: Option<u64>,
    pub http_status: u16,
}

// --- TFAI-Q04: Prometheus sayaçları (process içi; `sli` scrape’e ekler) ---

const N_EXCH: usize = 3;
const N_CAT: usize = 9;
const N_TIER: usize = 3;
const N_SLOTS: usize = N_EXCH * N_CAT * N_TIER;

static NORMALIZED_ERR_COUNTS: OnceLock<Mutex<[u64; N_SLOTS]>> = OnceLock::new();

fn metrics_buf() -> &'static Mutex<[u64; N_SLOTS]> {
    NORMALIZED_ERR_COUNTS.get_or_init(|| Mutex::new([0u64; N_SLOTS]))
}

fn exchange_label_idx(s: &str) -> usize {
    match s {
        "binance_futures" => 0,
        "binance_spot" => 1,
        _ => 2,
    }
}

fn category_index(c: ExchangeErrorCategory) -> usize {
    match c {
        ExchangeErrorCategory::RateLimit => 0,
        ExchangeErrorCategory::AuthFailure => 1,
        ExchangeErrorCategory::InsufficientFunds => 2,
        ExchangeErrorCategory::InvalidOrder => 3,
        ExchangeErrorCategory::MarketClosed => 4,
        ExchangeErrorCategory::PositionRisk => 5,
        ExchangeErrorCategory::ExchangeInternal => 6,
        ExchangeErrorCategory::NetworkTransient => 7,
        ExchangeErrorCategory::Unknown => 8,
    }
}

fn tier_index(t: AlertTier) -> usize {
    match t {
        AlertTier::Critical => 0,
        AlertTier::Elevated => 1,
        AlertTier::Low => 2,
    }
}

fn slot_index(e: &NormalizedExchangeError) -> usize {
    let xi = exchange_label_idx(&e.exchange);
    let ci = category_index(e.category);
    let ti = tier_index(e.alert_tier());
    xi * (N_CAT * N_TIER) + ci * N_TIER + ti
}

fn emit_normalized_exchange_observability(e: &NormalizedExchangeError) {
    if let Ok(mut g) = metrics_buf().lock() {
        let i = slot_index(e);
        g[i] = g[i].saturating_add(1);
    }
    log_by_tier(e);
}

fn log_by_tier(e: &NormalizedExchangeError) {
    let line = format!(
        "exchange={} category={} tier={} code={} http={} retryable={} msg={}",
        e.exchange,
        e.category.as_str(),
        e.alert_tier().as_str(),
        e.raw_code,
        e.http_status,
        e.retryable,
        e.raw_message.replace('"', "'")
    );
    match e.alert_tier() {
        AlertTier::Critical => log::warn!(target: "iqai_exchange", "{}", line),
        AlertTier::Elevated => log::debug!(target: "iqai_exchange", "{}", line),
        AlertTier::Low => log::trace!(target: "iqai_exchange", "{}", line),
    }
}

/// Prometheus `counter` satırları (`iqai_exchange_normalized_errors_total`).
pub fn prometheus_exchange_normalized_errors() -> String {
    let g = match metrics_buf().lock() {
        Ok(x) => x,
        Err(_) => return String::new(),
    };
    let mut w = String::new();
    let mut any = false;
    for i in 0..N_SLOTS {
        let v = g[i];
        if v == 0 {
            continue;
        }
        any = true;
        let xi = i / (N_CAT * N_TIER);
        let rem = i % (N_CAT * N_TIER);
        let ci = rem / N_TIER;
        let ti = rem % N_TIER;
        let ex = match xi {
            0 => "binance_futures",
            1 => "binance_spot",
            _ => "other",
        };
        let cat = CAT_LABELS[ci];
        let tier = TIER_LABELS[ti];
        use std::fmt::Write;
        let _ = writeln!(
            &mut w,
            r#"iqai_exchange_normalized_errors_total{{exchange="{}",category="{}",tier="{}"}} {}"#,
            ex,
            cat,
            tier,
            v
        );
    }
    if !any {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("# HELP iqai_exchange_normalized_errors_total TFAI-Q04 normalize edilmiş borsa API hataları (sayaç).\n");
    out.push_str("# TYPE iqai_exchange_normalized_errors_total counter\n");
    out.push_str(&w);
    out
}

const CAT_LABELS: [&str; 9] = [
    "rate_limit",
    "auth_failure",
    "insufficient_funds",
    "invalid_order",
    "market_closed",
    "position_risk",
    "exchange_internal",
    "network_transient",
    "unknown",
];

const TIER_LABELS: [&str; 3] = ["critical", "elevated", "low"];

/// `sli_counters` içinde kalıcı Q04 satırları — `render_prometheus_sli` scrape sırasında bellekten buraya aktarır.
pub const Q04_SLI_KEY_PREFIX: &str = "q04_norm:v1:";

fn index_to_sli_key(i: usize) -> String {
    let xi = i / (N_CAT * N_TIER);
    let rem = i % (N_CAT * N_TIER);
    let ci = rem / N_TIER;
    let ti = rem % N_TIER;
    let ex = match xi {
        0 => "binance_futures",
        1 => "binance_spot",
        _ => "other",
    };
    format!(
        "{}{}:{}:{}",
        Q04_SLI_KEY_PREFIX,
        ex,
        CAT_LABELS[ci],
        TIER_LABELS[ti]
    )
}

/// Bellekteki Q04 sayaçlarını boşaltır `(sli_counters` anahtarı, adet)`; `TradeDb::persist_q04_*` yazar.
pub fn drain_q04_memory_to_vec() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut g = match metrics_buf().lock() {
        Ok(x) => x,
        Err(_) => return out,
    };
    for i in 0..N_SLOTS {
        let v = g[i];
        if v == 0 {
            continue;
        }
        out.push((index_to_sli_key(i), v));
        g[i] = 0;
    }
    out
}

fn prom_escape_label_q04(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// `sli_counters` anlık görüntüsünden `iqai_exchange_normalized_errors_total` satırları (kalıcı `q04_norm:v1:*` anahtarları).
pub fn prometheus_q04_from_sli_snapshot(snap: &BTreeMap<String, f64>) -> String {
    let mut w = String::new();
    let mut any = false;
    use std::fmt::Write;
    for (k, v) in snap {
        if !k.starts_with(Q04_SLI_KEY_PREFIX) || *v == 0.0 {
            continue;
        }
        let rest = match k.strip_prefix(Q04_SLI_KEY_PREFIX) {
            Some(r) => r,
            None => continue,
        };
        let p: Vec<&str> = rest.split(':').collect();
        if p.len() != 3 {
            continue;
        }
        let ex = p[0];
        let cat = p[1];
        let tier = p[2];
        any = true;
        let _ = writeln!(
            &mut w,
            r#"iqai_exchange_normalized_errors_total{{exchange="{}",category="{}",tier="{}"}} {}"#,
            prom_escape_label_q04(ex),
            prom_escape_label_q04(cat),
            prom_escape_label_q04(tier),
            v
        );
    }
    if !any {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("# HELP iqai_exchange_normalized_errors_total TFAI-Q04 normalize edilmiş borsa API hataları (sayaç; kalıcı + bellek birleşimi scrape başında).\n");
    out.push_str("# TYPE iqai_exchange_normalized_errors_total counter\n");
    out.push_str(&w);
    out
}

/// Yalnızca birim testlerinde sayaçları sıfırlamak için.
#[cfg(test)]
pub fn reset_exchange_normalized_metrics_for_test() {
    if let Ok(mut g) = metrics_buf().lock() {
        g.fill(0);
    }
}

/// JSON gövdeden `code` / `msg` okuyup kategori atar. `exchange` örn. `binance_futures`, `binance_spot`.
pub fn classify_binance_json(exchange: &str, http_status: u16, body: &Value) -> NormalizedExchangeError {
    let raw_code = body["code"].as_i64().map(|c| c as i32).unwrap_or(0);
    let raw_message = body["msg"]
        .as_str()
        .unwrap_or("unknown error")
        .to_string();
    let (category, retryable, retry_after_ms) = map_binance(raw_code, http_status, &raw_message);
    let e = NormalizedExchangeError {
        exchange: exchange.to_string(),
        raw_code,
        raw_message,
        category,
        retryable,
        retry_after_ms,
        http_status,
    };
    emit_normalized_exchange_observability(&e);
    e
}

fn map_binance(raw_code: i32, http: u16, msg: &str) -> (ExchangeErrorCategory, bool, Option<u64>) {
    // HTTP öncelik (gövde code yok veya 0 iken)
    match http {
        429 => return (ExchangeErrorCategory::RateLimit, true, None),
        418 => return (ExchangeErrorCategory::RateLimit, true, None),
        401 | 403 => return (ExchangeErrorCategory::AuthFailure, false, None),
        408 => return (ExchangeErrorCategory::NetworkTransient, true, None),
        500..=504 => {
            return (
                ExchangeErrorCategory::ExchangeInternal,
                true,
                None,
            );
        }
        _ => {}
    }

    let c = match raw_code {
        -1003 | -1006 | -1007 | -1015 => ExchangeErrorCategory::RateLimit,
        -1000 | -1001 | -1016 => ExchangeErrorCategory::ExchangeInternal,
        -1002 | -1022 | -2014 | -2015 => ExchangeErrorCategory::AuthFailure,
        -1021 => ExchangeErrorCategory::NetworkTransient,
        -2010 => ExchangeErrorCategory::InsufficientFunds,
        -2019 => ExchangeErrorCategory::PositionRisk,
        -1013 | -1100 | -1102 | -1111 | -1116 | -2011 | -2013 | -4164 => {
            ExchangeErrorCategory::InvalidOrder
        }
        -1121 => ExchangeErrorCategory::InvalidOrder,
        0 if http >= 400 => {
            // Bazı edge cevaplarda code yok
            if msg.to_lowercase().contains("insufficient") || msg.to_lowercase().contains("balance") {
                ExchangeErrorCategory::InsufficientFunds
            } else if msg.to_lowercase().contains("rate") || msg.to_lowercase().contains("limit") {
                ExchangeErrorCategory::RateLimit
            } else if msg.to_lowercase().contains("invalid") || msg.to_lowercase().contains("precision") {
                ExchangeErrorCategory::InvalidOrder
            } else {
                ExchangeErrorCategory::Unknown
            }
        }
        _ => ExchangeErrorCategory::Unknown,
    };

    let retryable = matches!(
        c,
        ExchangeErrorCategory::RateLimit
            | ExchangeErrorCategory::ExchangeInternal
            | ExchangeErrorCategory::NetworkTransient
    );

    (c, retryable, None)
}

impl NormalizedExchangeError {
    /// `ExchangeErrorCategory` → uyarı katmanı (harici otomasyon bu alanı kullanabilir).
    pub fn alert_tier(&self) -> AlertTier {
        match self.category {
            ExchangeErrorCategory::AuthFailure => AlertTier::Critical,
            ExchangeErrorCategory::InsufficientFunds | ExchangeErrorCategory::PositionRisk => {
                AlertTier::Elevated
            }
            ExchangeErrorCategory::RateLimit
            | ExchangeErrorCategory::ExchangeInternal
            | ExchangeErrorCategory::NetworkTransient
            | ExchangeErrorCategory::Unknown => AlertTier::Elevated,
            ExchangeErrorCategory::InvalidOrder | ExchangeErrorCategory::MarketClosed => AlertTier::Low,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn insufficient_balance_2010() {
        reset_exchange_normalized_metrics_for_test();
        let body = json!({"code": -2010, "msg": "Account has insufficient balance"});
        let e = classify_binance_json("binance_futures", 400, &body);
        assert_eq!(e.category, ExchangeErrorCategory::InsufficientFunds);
        assert_eq!(e.raw_code, -2010);
        assert!(!e.retryable);
    }

    #[test]
    fn rate_limit_1003() {
        reset_exchange_normalized_metrics_for_test();
        let body = json!({"code": -1003, "msg": "Too many requests"});
        let e = classify_binance_json("binance_futures", 418, &body);
        assert_eq!(e.category, ExchangeErrorCategory::RateLimit);
        assert!(e.retryable);
    }

    #[test]
    fn http_429_without_code() {
        reset_exchange_normalized_metrics_for_test();
        let body = json!({"msg": "Too many requests"});
        let e = classify_binance_json("binance_spot", 429, &body);
        assert_eq!(e.category, ExchangeErrorCategory::RateLimit);
        assert!(e.retryable);
    }

    #[test]
    fn timestamp_1021_retryable() {
        reset_exchange_normalized_metrics_for_test();
        let body = json!({"code": -1021, "msg": "Timestamp outside recvWindow"});
        let e = classify_binance_json("binance_futures", 400, &body);
        assert_eq!(e.category, ExchangeErrorCategory::NetworkTransient);
        assert!(e.retryable);
        assert_eq!(e.alert_tier(), AlertTier::Elevated);
    }

    #[test]
    fn alert_tier_auth_is_critical() {
        reset_exchange_normalized_metrics_for_test();
        let body = json!({"code": -2015, "msg": "Invalid API-key"});
        let e = classify_binance_json("binance_futures", 401, &body);
        assert_eq!(e.category, ExchangeErrorCategory::AuthFailure);
        assert_eq!(e.alert_tier(), AlertTier::Critical);
    }

    #[test]
    fn alert_tier_invalid_order_is_low() {
        reset_exchange_normalized_metrics_for_test();
        let body = json!({"code": -1013, "msg": "Invalid quantity"});
        let e = classify_binance_json("binance_futures", 400, &body);
        assert_eq!(e.category, ExchangeErrorCategory::InvalidOrder);
        assert_eq!(e.alert_tier(), AlertTier::Low);
    }

    #[test]
    fn prometheus_export_contains_normalized_counter() {
        reset_exchange_normalized_metrics_for_test();
        let body = json!({"code": -2015, "msg": "Invalid API-key"});
        let _ = classify_binance_json("binance_futures", 401, &body);
        let s = prometheus_exchange_normalized_errors();
        assert!(
            s.contains("iqai_exchange_normalized_errors_total"),
            "{}",
            s
        );
        assert!(s.contains("auth_failure"));
        assert!(s.contains("critical"));
        assert!(s.contains("binance_futures"));
    }
}
