//! GET istekleri için basit retry (429 / 502 / 503 ve ağ hataları).
//! POST emir vb. idempotent olmayan çağrılar burada **kullanılmamalı**.
//!
//! **Retry-After:** 429/503/502 yanıtında `Retry-After: <saniye>` (tam sayı) varsa önce bu süre beklenir (üst sınır 120 sn).

use std::time::Duration;

use iqai_core::exchange::ExchangeError;
use reqwest::Client;

/// `Retry-After` başlığı — yalnızca saniye cinsinden tam sayı (Binance yaygın kullanım).
fn parse_retry_after_seconds(resp: &reqwest::Response) -> Option<Duration> {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Aynı GET’i en fazla 4 kez dener; exponential backoff (max 5 sn).
pub async fn send_get_retry<F>(
    _client: &Client,
    traceparent: Option<&str>,
    build: F,
) -> Result<reqwest::Response, ExchangeError>
where
    F: Fn() -> reqwest::RequestBuilder,
{
    const MAX_ATTEMPTS: usize = 4;
    const MAX_RETRY_AFTER: Duration = Duration::from_secs(120);
    let mut wait = Duration::from_millis(150);
    for attempt in 0..MAX_ATTEMPTS {
        let mut builder = build();
        if let Some(tp) = traceparent {
            builder = builder.header("traceparent", tp);
        }
        match builder.send().await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                if (code == 429 || code == 503 || code == 502) && attempt + 1 < MAX_ATTEMPTS {
                    let retry_after = parse_retry_after_seconds(&resp)
                        .map(|d| d.min(MAX_RETRY_AFTER))
                        .unwrap_or(wait);
                    let sleep_dur = retry_after.max(Duration::from_millis(1));
                    drop(resp);
                    tokio::time::sleep(sleep_dur).await;
                    wait = wait.saturating_mul(2).min(Duration::from_secs(5));
                    continue;
                }
                return Ok(resp);
            }
            Err(e) => {
                if attempt + 1 < MAX_ATTEMPTS {
                    tokio::time::sleep(wait).await;
                    wait = wait.saturating_mul(2).min(Duration::from_secs(5));
                    continue;
                }
                return Err(ExchangeError::Http(e.to_string()));
            }
        }
    }
    Err(ExchangeError::Http("send_get_retry: exhausted".into()))
}
