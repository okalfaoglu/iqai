# Uyarı katmanları (TFAI-Q04)

Normalize edilmiş Binance hatası: `iqai_core::NormalizedExchangeError::alert_tier()` → `AlertTier`.

| `AlertTier` | Anlam | Örnek kategori |
|-------------|--------|----------------|
| **Critical** | Hemen müdahale | `AuthFailure` |
| **Elevated** | Kısa sürede triage | `RateLimit`, `ExchangeInternal`, `NetworkTransient`, `InsufficientFunds`, `PositionRisk`, `Unknown` |
| **Low** | İzleme / düşük öncelik | `InvalidOrder`, `MarketClosed` |

## Metrik (Prometheus)

`classify_binance_json` her çağrıldığında sayaç artar; `GET /metrics/prometheus` çıktısına eklenir:

- `iqai_exchange_normalized_errors_total{exchange="binance_futures|binance_spot|other",category="...",tier="critical|elevated|low"}`

Sıfır olan seriler scrape’te yazılmaz (çıktı boyutu).

## Log

`target: "iqai_exchange"`: **Critical** → `warn`, **Elevated** → `debug`, **Low** → `trace`.

**Not:** PagerDuty / Telegram otomasyonu ayrı bağlanır; metrik + log Q04 gözlemi için yeterlidir.

Kod: `crates/iqai-core/src/binance_error.rs` — `AlertTier`, `alert_tier`, `prometheus_exchange_normalized_errors`.
