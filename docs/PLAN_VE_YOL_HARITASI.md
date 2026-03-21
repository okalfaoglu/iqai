# IQAI – Plan ve Yol Haritası

Bu doküman, proje incelemesi ve Q-ANALİZ tartışmalarından çıkan planı özetler. Öncelik sırasına göre gruplanmıştır.

---

## Öncelik 1: Kritik (Hemen)

| # | Durum | Yapılacak | Kaynak |
|---|-------|-----------|--------|
| 1 | **DONE** | **config.json’da gerçek token/şifre kaldır** – Repo’daki config temizlendi (placeholder/null). **Not:** Eski token/şifreler rotate edilmeli. | EKSIK_HATA_IYILESTIRME_OZETI §Kritik |
| 2 | **DONE** | **API anahtarlarını env’den oku** – Live modda `BINANCE_API_KEY/BINANCE_SECRET_KEY` env fallback eklendi. | Aynı |

---

## Öncelik 2: Düzeltmeler ve Netleştirmeler (Kısa vadede)

| # | Durum | Yapılacak | Kaynak |
|---|-------|-----------|--------|
| 3 | **DONE** | **`SymbolPnlStats` / `get_symbol_pnl_stats`** – `total_positions` + `open_count` (`status='open'` ile); `pnl.html` geriye dönük `opened_count` fallback. | `trade_db.rs`, `pnl.html` |
| 4 | **DONE** | **`TradingMode::from_str`** – `live` / `dry` / `paper` açık dallar; bilinmeyen → `paper`. | `auto_trader.rs` |
| 5 | **DONE** | **`GET /api/q-analysis`** – Boş `symbols` → varsayılan ETHUSDT/BTCUSDT; `docs/API_Q_ANALYSIS.md`. | `http_app.rs` |
| 6 | **DONE** | **`TradingConfig::validate`** – `risk_per_trade_pct`, `min_q_score`, `min_rr`, `max_leverage`, … aralıkları. | `app_config.rs` |

---

## Öncelik 2A: Tutarlılık / Spam / Modelleme

| # | Durum | Yapılacak | Kaynak |
|---|-------|-----------|--------|
| 18 | **DONE** | `trade_manager.rs` içinde `remaining_pct` clamp + `pct` guard | EKSIK_HATA §Kritik (Fonksiyonel / Analitik Tutarlılık) |
| 19 | **DONE** | `/api/chart` poll başına `notify` dedup/throttle (spam engeli) | Aynı (crates/iqai-web/src/notify.rs throttling). |
| 20 | **TODO** | Backtest vs live/daemon trade yönetimi uyumu (partial TP/breakeven/trailing + komisyon/slippage + `entry_zone`) | Bu turda `run_backtest` fee (commission) + `slippage_bps` ile effective_exit benzeri PnL hesaplamaya güncellendi. Kalan uyumsuzluk: intrabar sıralama + TP2/TP3 state eşleşmesi. |
| 21 | **DONE** | `auto_trader.rs` PnL/`TradeLog.exit` tutarlılığı (effective_exit vs current_price) | `TradeLog.exit` ve `TradeEvent` fiyatları `effective_exit` ile tutarlı hale getirildi. |
| 22 | **DONE** | Partial close’lar için outcome/raporlama kapsamı netleştirildi: `analysis_outcomes` yalnızca `AutoTrader::close_position()` (tam kapanış) sırasında yazılır; `partial_close()` yazmaz. | Aynı |
| 23 | **DONE** | `candlestick_patterns.rs` için ATR tabanlı noise filter eklendi | EKSIK_HATA §Yüksek (Sinyal Kalitesi / False Positive Riski) |

---

## Öncelik 3: Q-ANALİZ Geliştirmeleri (Özellik)

| # | Yapılacak | Açıklama |
|---|-----------|-----------|
| 7 | **DONE** | **Q-RADAR + Q-Setup + Elliott (G05):** `compute_q_radar_opportunity` → `q_setup`, `radar_setup_alignment`, `elliott_secondary_tp`, `elliott_summary`, `abc_correction_hint`; `config.q_enrich_opportunity_with_setup_elliott`. Kalan: TP birleştirme kuralları / çoklu TF — `docs/API_Q_ANALYSIS.md`, `Q_ANALIZ_WYCKOFF_POZ_KORUMA_CEVAP.md` §3. |
| 8 | **Wyckoff faz etiketleri (opsiyonel)** | Pivot + RSI at pivot ile SC, AR, ST, BC, DAR, DST etiketlemesi; referans doküman ve Pine Script ile uyum. DIP_TEPE_VE_WYCKOFF_REFERANS.md, Q_ANALIZ_WYCKOFF_POZ_KORUMA_CEVAP.md §4. |

---

## Öncelik 4: Kalite ve Altyapı

| # | Durum | Yapılacak | Kaynak |
|---|-------|-----------|--------|
| 9 | **TODO** | **Test kapsamı** – Sinyal motoru, Q-Setup, confluence, Elliott birim testleri; exchange mock ile entegrasyon. | EKSIK_HATA §Eksikler |
| 10 | **TODO** | **Hata cevabı standardı** – API/DB hatalarında tutarlı JSON `error` alanı (ve isteğe bağlı request id). | Aynı |
| 11 | **TODO** | **Rate limit / retry** – Binance ve TV çağrılarında retry/backoff. | Aynı |

---

## Öncelik 5: İyileştirmeler (Orta vadede)

| # | Durum | Yapılacak | Kaynak |
|---|-------|-----------|--------|
| 12 | **DONE** | **OpenAPI/Swagger** – `openapi.yaml`, `GET /api/openapi.yaml`, `GET /api/docs`. | `docs/OPENAPI.md`, `TODO_PLAN` G-02 |
| 13 | **DONE** | **CORS** – `tower-http` + `config.web.cors_allow_origins`. | `TODO_PLAN` G-03 |
| 14 | **TODO** | **iqai-gui** – README’de “experimental/stub” veya tamamlama. | Aynı |
| 15 | **TODO** | **Piyasa saatleri** – BIST/NASDAQ config veya dil ayarından. | Aynı |
| 16 | **TODO** | **Log önceliği** – RUST_LOG vs config “logging.level” dokümante. | Aynı |
| 17 | **TODO** | **Pozisyon limiti** – Sembol bazlı max notional (exchangeInfo/config). | Aynı |

---

## Referans Dokümanlar

| Doküman | İçerik |
|---------|--------|
| **EKSIK_HATA_IYILESTIRME_OZETI.md** | Eksik/hata/iyileştirme listesi (kısa). |
| **PROJE_DOKUMANTASYONU.md** | Proje mimarisi ve §10’da aynı maddelerin detayı. |
| **Q_ANALIZ_DETAYLI_DOKUMANTASYON.md** | Q-ANALİZ (Q-RADAR, Q-Setup, Poz Koruma) formüller ve akış. |
| **Q_ANALIZ_WYCKOFF_POZ_KORUMA_CEVAP.md** | Wyckoff kullanımı, Poz Koruma hesabı, Q-RADAR→Q-Setup+Elliott tasarımı. |
| **DIP_TEPE_VE_WYCKOFF_REFERANS.md** | Dip/tepe yöntemleri ve Wyckoff referansı. |
| **DIP_TESPITI_KATMANLAR.md** | Katmanlı dip tespiti ile kod eşlemesi, geliştirme önerileri. |

---

## Özet: Planımız

1. **Hemen:** Güvenlik (config token/şifre, API key env).  
2. **Kısa vade:** DB/tutarlılık düzeltmeleri, config validasyonu, dokümantasyon netleştirmeleri.  
3. **Özellik:** Q-RADAR tespitine Q-Setup (skor, giriş, stop, TP) + Elliott Wave hedefleri; isteğe bağlı Wyckoff faz etiketleri.  
4. **Kalite:** Test kapsamı, hata standardı, rate limit/retry.  
5. **Orta vade:** API dokümantasyonu, CORS, iqai-gui, piyasa saatleri, log önceliği, pozisyon limiti.

Bu sırayla ilerlenebilir; öncelikler ihtiyaca göre kaydırılabilir.
