# IQAI – Geliştirme TODO Planı

Bu dosya **aktif backlog** için kullanılır. Eski tamamlanan maddeler burada tutulmaz; geçmiş kararlar için `docs/EKSIK_HATA_IYILESTIRME_OZETI.md`, `docs/PLAN_VE_YOL_HARITASI.md` ve ilgili özellik dokümanlarına bakın.

**Kod taraması:** `rg -n "TODO|FIXME|todo!|unimplemented!" crates` (periyodik kontrol).

---

## Şu anki odak: Observability & RCA (TFAI)

Teknik referans (şema, SQL, span, SLI, postmortem): **`docs/TRADE_FAILURE_TFAI_CLAUDE_FULL.md`**  
IQAI bağlamı ve sentez: `docs/TRADE_FAILURE_ANALYSIS.md`, `docs/TRADE_FAILURE_AI_RESPONSES_SYNTHESIS.md`.

| ID | Durum | Konu | Not |
|----|--------|------|-----|
| O-01 | [x] DONE | Kapalı pozisyon RCA alanları | `positions`: `position_uuid`, `strategy_id`, `exchange`, `opened_at_us`/`closed_at_us`, slippage, MAE/MFE, `trace_id`; `ManagedPosition` MAE/MFE tick — TFAI-Q01 |
| O-02 | [x] DONE | `close_reason` taksonomisi + sürüm | `close_reason_v`, `v_positions_canonical`, `close_reason_registry`, `close_reason_to_canonical` + testler — TFAI-Q02 |
| O-03 | [x] DONE | `position_events` (event sourcing) | `position_events` tablosu; `opened` / `tp_partial` / `sl_moved` / `closed` + testler — TFAI-Q03 (tam replay read-model sonraki iterasyon) |
| O-04 | [x] DONE | Borsa hata normalizasyonu | `iqai-core::binance_error`: `ExchangeErrorCategory`, `NormalizedExchangeError`, `classify_binance_json`; `ExchangeError::Normalized`; Futures/Spot emir+bakiye hataları — TFAI-Q04 (alert katmanı / Retry-After ayrı) |
| O-05 | [x] DONE | İz sürme | `trace_id` kök UUID her `process_signal`; `signals.trace_id` + `positions.trace_id`; `ManagedPosition.trace_id` + `signal_db_id`; loglarda `format_trade_correlation`; `position_events.payload` içinde trace/signal. **OTel / W3C span ağacı** sonraki adım (TFAI-Q05 uzun ömürlü span). |
| O-06 | [x] DONE | SLI başlangıç seti | `GET /metrics/prometheus` (`iqai-web`), `render_prometheus_sli` + `sli_counters` + canlı `auto_trader` sayaçları — `docs/SLI_METRICS.md` — TFAI-Q06 |
| O-07 | [x] DONE | Örnekleme + log hacmi | `logging.verbose_chart_poll`; `/api/chart` TF/Binance mesajları varsayılan `debug` (`iqai_chart`) — `docs/LOG_SAMPLING.md` — TFAI-Q07 |
| O-08 | [x] DONE | `ai_explanations` izlenebilirlik | `ai_explanations` tablosu; `prompt_hash`, `context_hash`, `query_fingerprint`, `source_refs_json`; web büyük resim + q-analiz daemon; `GET /api/ai-explanations`; `ai.persist_explanations` — `docs/AI_EXPLANATIONS.md` — TFAI-Q11 |
| O-09 | [x] DONE | Tracing kök span (OTel öncesi) | `tracing` + `#[instrument]` `iqai.process_signal` (`trace_id`, `symbol`, `mode`) — `docs/TRACING.md`; OTLP exporter ayrı PR |
| O-10 | [x] DONE | W3C traceparent + GET Retry-After | `traceparent_from_uuid` (`trace_context`); Binance `send_get_retry` `Retry-After` saniye — `docs/TRACING.md`, `docs/HTTP_RETRY_AFTER.md` |

**Tamamlanınca:** İlgili satırı `[x] DONE` yapın; yeni maddeler tabloya eklenir.

---

## Genel backlog (düşük öncelik / ayrı sprint)

İhtiyaç oldukça buraya veya ayrı issue’lara taşınır.

| ID | Durum | Konu |
|----|--------|------|
| G-01 | [x] DONE | Birim testleri: `SignalEngine` + `CandleBuffer` (`signal.rs`); `AppConfig` logging/AI varsayılanları — RCA/Binance testleri önceden vardı |
| G-02 | [x] DONE | `openapi.yaml` + `GET /api/openapi.yaml` + `GET /api/docs` (Swagger UI) — `docs/OPENAPI.md` |
| G-03 | [x] DONE | `tower-http` CORS; `config.web.cors_allow_origins` (boş = permissive) |
| G-04 | [x] DONE | `iqai-gui` stub mesajı + `crates/iqai-gui/README.md` + `docs/GUI_ROADMAP.md` |
| G-05 | [x] DONE | Q-RADAR + Q-Setup + Elliott: `q_radar_analysis` zenginleştirme, `config.q_enrich_opportunity_with_setup_elliott`, `radar_setup_alignment_score` — `docs/API_Q_ANALYSIS.md` |
| T-01 | [x] DONE | `iqai-web` router lib’de: `http_app::build_router`, `run_server`; ince `main.rs`; `tests/http_smoke` — `/`, `/metrics/prometheus`, OpenAPI uçları |
| G-06 | [x] DONE | XMSTradeX / TradingView Elliott PDF × kod matrisi (TODO/DONE): `docs/XMST_TRADINGVIEW_EW_PDF_VS_KOD.md` — PDF sayfa eşlemesi §4 kullanıcı tarafından doldurulacak |

---

## Sıradaki adaylar (backlog boş / yeni sprint)

O ve G tabloları tamamlandı; öncelik ürün ve operasyon ihtiyacına göre seçilir:

| Öncelik | Konu | Not |
|--------|------|-----|
| **Observability** | **OTel OTLP exporter** | `tracing` + `traceparent_from_uuid` hazır; Collector / `traceparent` header otomatik bağlama — sonraki PR |
| **Operasyon** | **Alert katmanı** (normalize hata → uyarı) | Örnek Prometheus kuralları: `docs/PROMETHEUS_ALERT_EXAMPLES.md`; tam Alertmanager/Grafana pipeline ayrı |
| **Ürün** | **Elliott ABC derinleştirme** | İsteğe bağlı: ayrı TF ABC doğrulama, backtest bayrakları — `docs/PLAN_VE_YOL_HARITASI.md` Öncelik 3 |
| **Masaüstü** | **iqai-gui** (Tauri veya egui) | `docs/GUI_ROADMAP.md` |
| **Test** | Daha fazla `/api/*` senaryosu (mock DB / ağ) | `build_router()` hazır (`T-01`); kritik uçlar için ek `http_smoke` veya ayrı fixture testleri |

---

## Kullanım

- Madde kodlandığında tabloda durumu güncelleyin.
- Büyük değişikliklerde ilgili `docs/*.md` dosyasını (ör. snapshot, API) güncelleyin.
