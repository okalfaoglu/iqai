# Trade failure / TFAI — uygulama ilerlemesi

Bu dosya `docs/TRADE_FAILURE_ANALYSIS.md`, `TRADE_FAILURE_AI_RESPONSES_SYNTHESIS.md` ve TFAI referanslarına göre **ne yapıldı / ne sırada** özetini tutar. Tamamlanan maddeler `[x]`, kalanlar `[ ]`.

---

## `TRADE_FAILURE_ANALYSIS.md` — yol haritası özeti

| ID | Madde | Durum |
|----|--------|--------|
| P0 | `close_reason` / `reason` sözlüğü; PnL/raporlarda canonical filtre | [x] `close_reason_registry`, `v_positions_canonical`, `close_reason_to_canonical` (`trade_db`, `position_rca`) |
| P1 | Yapılandırılmış log + korelasyon (`trace_id`, …) | [x] `tracing` + `process_signal`; [x] web `X-Request-Id` (`iqai-web` `tower-http` request-id); [x] hata JSON gövdesinde `request_id` (`api_json` + `http_app`, `X-Request-Id` ile aynı değer) |
| P2 | Sembol × nedeni paneli; basit anomali uyarıları | [ ] panel/Grafana; [x] örnek Prometheus uyarı kuralları — `docs/PROMETHEUS_ALERT_EXAMPLES.md` (SLI + Q04 + `iqai_db_reachable`) |
| P3 | Tam replay ortamı; deploy sürümü loglarda | [ ] replay; [x] `iqai_web` başlangıç logu: crate sürümü + `debug`/`release` |
| P4 | Harici APM / metrics backend | [ ] (isteğe bağlı) |

---

## TFAI soru seti (Q01–Q14) — repo özeti

| Key | Tema | Durum (özet) |
|-----|------|----------------|
| Q01 | Kapalı pozisyon RCA alanları | [x] **DONE (enterprise):** çekirdek + MAE/MFE, `trace_id`; ek alanlar: `entry_price_avg`/`exit_price_avg`, `position_notional_usd`, `leverage`, `rr_at_open`, `signal_to_entry_ms`, `pnl_gross_usd`/`pnl_net_usd`/`pnl_bps`, `lifecycle_duration_ms`, `close_order_id`, `exit_orders_json`; SQLite `q01_enterprise` mig.; `volatility_at_open` / `spread_at_open_bps` / `funding_rate_at_open` kolonları hazır, feed sonraki adım — `position_rca`, `trade_db`, `auto_trader` |
| Q02 | `close_reason` taksonomi | [x] registry + canonical view |
| Q03 | Event sourcing | [x] `position_events` |
| Q04 | Borsa hata normalizasyonu | [x] `binance_error`; [x] `AlertTier` + `alert_tier()`; [x] kalıcı sayaç: `q04_norm:v1:*` → SQLite `sli_counters` (Prometheus scrape öncesi bellekten flush); `iqai_exchange_normalized_errors_total` — `docs/ALERT_TIERS.md` (harici uyarı bağlantısı sonraki adım) |
| Q05 | İz / trace | [x] `trace_id`, W3C `traceparent` Binance; [x] HTTP `X-Request-Id`; [x] API hata yanıtlarında JSON `request_id` (başlıkla eşleşir); [ ] tam OTel OTLP |
| Q06 | SLI | [x] Prometheus endpoint + sayaçlar |
| Q07 | Log örnekleme | [x] `verbose_chart_poll`, doküman |
| Q08–Q09 | Rejim vs bug; FDR | [ ] istatistiksel katman |
| Q10 | Güvenli log | [ ] ayrı scrub / rol — politika |
| Q11 | Denetlenebilir AI | [x] `ai_explanations` + hash alanları |
| Q12 | Otomatik kontroller | [ ] proptest/shadow genişletme |
| Q13 | Postmortem | [x] şablon: `docs/POSTMORTEM_TEMPLATE.md` |
| Q14 | Sahiplik / onay | [x] özet: `docs/OPERATIONS_GOVERNANCE.md` |

---

## Son güncelleme

- **Q01 (2026-03-20):** Kapalı pozisyon RCA için enterprise alan seti + DB migrasyonu + `auto_trader` doldurma; `cargo test -p iqai-core` yeşil.
- **Web:** `X-Request-Id` (istemci yoksa UUID), yanıtta aynı başlık; başlangıçta `iqai_web` sürüm logu.
- **Web (hata JSON):** `ok: false` cevaplarda `request_id` alanı — `X-Request-Id` ile aynı (`crates/iqai-web/src/api_json.rs`, `http_app.rs`); `POST /api/config` için `Extension<RequestId>` + `Json` sırası (Axum).
- **Test:** `crates/iqai-web/tests/http_smoke.rs` — `x-request-id` + `api_error_json_request_id_matches_x_request_id_header`.
- **Q04:** `alert_tier()` + sınıflandırma; hot path’te bellek slotları, **`GET /metrics/prometheus` scrape’inde** `persist_q04_normalized_errors_from_memory` ile `sli_counters` (`q04_norm:v1:{exchange}:{category}:{tier}`) içine atomik yazım; exporter `iqai_exchange_normalized_errors_total` üretir; `iqai_exchange` log; `docs/ALERT_TIERS.md`, `docs/SLI_METRICS.md`.
- **Q14:** `docs/OPERATIONS_GOVERNANCE.md` (sahiplik / acil / deploy özeti).

---

## Nasıl güncellenir?

1. Bir TFAI / `TRADE_FAILURE_ANALYSIS` maddesini **kod veya dokümanla** tamamladığında bu dosyada ilgili satırdaki **`[ ]` → `[x]`** yap.
2. İstersen aynı hücreye veya **Son güncelleme** altına kısa not ekle: örn. `tarih: 2026-03-20`, PR/commit, veya dosya yolu.
3. Yeni bir madde eklendiyse tabloya satır aç; eski tamamlananları silmek zorunda değilsin (geçmiş özeti olarak kalabilir).
