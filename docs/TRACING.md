# Tracing / OTel hazırlığı (O-05+)

## Şu an (IQAI)

- `AutoTrader::process_signal` üzerinde **`tracing::instrument`** ile span adı: **`iqai.process_signal`**.
- Alanlar: **`trace_id`** (DB/log ile aynı UUID), **`symbol`**, **`mode`** (`live` / `dry` / `paper`).
- `flexi_logger` + `log::` çıktısı aynen durur; `tracing` abonesi yoksa span oluşturma **düşük maliyetlidir**.

## Abone (isteğe bağlı)

Tam span çıktısı için süreç içinde **`tracing-subscriber`** (ör. `fmt` veya OTLP) tekilleştirilmeli; `flexi_logger` ile **çift stderr** olmaması için ya:

- yalnızca birini global logger olarak kullanın, veya  
- `tracing-log` köprüsü ile `log` → `tracing` (ayrı kurulum).

Öneri: üretimde **OpenTelemetry Collector** hedefi için `tracing-opentelemetry` + `opentelemetry-otlp` ayrı bir PR’da.

## W3C traceparent

`iqai_core::traceparent_from_uuid(&trace_id)` — `trace_id` (standart UUID metni) için W3C `traceparent` değeri üretir:

- Örnek: `00-550e8400e29b41d4a716446655440000-0000000000000001-01`
- **Auto-trader:** `process_signal` içinde `ExchangeTraceScopeGuard` (`iqai_core`) `trace_id`’yi borsa bağlayıcısına iletir; `BinanceFuturesClient` / `BinanceSpotClient` GET (`send_get_retry`) ve POST emirlerine `traceparent` başlığı ekler.

## HTTP `X-Request-Id` (`iqai-web`)

Tarayıcı / API istemcisi korelasyonu: `iqai_web::build_router()` üzerinde `tower-http` **`SetRequestIdLayer`** + **`PropagateRequestIdLayer`** — istekte `X-Request-Id` yoksa UUID üretilir; yanıtta aynı başlık döner (`docs/TRADE_FAILURE_ANALYSIS.md` §3.3).

### JSON hata gövdesinde `request_id`

Hata cevaplarında (`ok: false`, `error: "…"`) isteğe bağlı **`request_id`** alanı eklenir; değer, yanıt başlığındaki **`X-Request-Id`** ile aynıdır (destek / log eşlemesi).

- **Yardımcılar:** `iqai_web::api_json` — `api_error_with_request_id`, `api_error_with_extras_and_request_id` (boş/eksik id ise alan yazılmaz).
- **Handler’lar:** `crates/iqai-web/src/http_app.rs` — GET uçları `Request` extractor + `x_request_id_from_extensions`; **`POST /api/config`** gövde `Json` ile okunduğu için `Request` kullanılamaz → **`Extension<RequestId>` önce**, `Json<AppConfig>` sonra (Axum: `FromRequestParts` body’den önce).
- **Test:** `crates/iqai-web/tests/http_smoke.rs` — `api_error_json_request_id_matches_x_request_id_header`.

## Kod

- `crates/iqai-core/src/trace_context.rs` — `traceparent_from_uuid`
- `crates/iqai-core/src/exchange.rs` — `ExchangeTraceScopeGuard`, `ExchangeConnector::set_trace_id_for_request`
- `crates/iqai-core/src/auto_trader.rs` — `process_signal`
- `crates/iqai-binance` — `futures.rs` / `spot.rs` `traceparent` + `http_retry::send_get_retry`
- `crates/iqai-web/src/http_app.rs` — `X-Request-Id`, hata JSON `request_id`
- `crates/iqai-web/src/api_json.rs` — `request_id` alanı şeması
