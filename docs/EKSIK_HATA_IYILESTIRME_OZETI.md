# IQAI – Eksik / Hata / İyileştirme Özeti

Proje satır satır incelenerek tespit edilen maddelerin kısa listesi. Detay için `PROJE_DOKUMANTASYONU.md` §10’a bakın.

---

## Kritik (Güvenlik)

| # | Durum | Konu | Öneri |
|---|-------|------|--------|
| 1 | **DONE** | **config.json’da gerçek token/şifre** | Repo’daki `config.json` içeriği placeholder/null olacak şekilde temizlendi. **Not:** Eski token/şifreler mutlaka rotate edilmeli. |
| 2 | **DONE** | **API anahtarları** | Live modda `BINANCE_API_KEY` / `BINANCE_SECRET_KEY` env fallback eklendi (config.json yoksa da çalışır). |

---

## Olası Hata / Tutarsızlık

| # | Durum | Konu | Öneri |
|---|-------|------|--------|
| 3 | **TODO** | **api_q_analysis_all – boş symbols** | Config’te `symbols: []` iken varsayılan semboller kullanılıyor; davranış dokümante edilsin. |
| 4 | **TODO** | **trade_db opened_count** | `get_symbol_pnl_stats` içinde “opened_count” aslında toplam pozisyon sayısı (açık+kapalı). Sorgu `status='open'` ile kısıtlansın veya alan adı (örn. total_positions) netleştirilsin. |
| 5 | **TODO** | **TradingMode::from_str** | "paper" için açık branch eklenmesi (şu an `_ => Paper` ile dolaylı); okunabilirlik için iyileştirilebilir. |

---

## Eksikler

| # | Durum | Konu | Öneri |
|---|-------|------|--------|
| 6 | **TODO** | **Test kapsamı** | Sinyal motoru, Q-Setup, confluence, Elliott için birim testleri; exchange mock ile entegrasyon testleri eklenebilir. |
| 7 | **TODO** | **Hata cevabı standardı** | API/DB hatalarında tutarlı JSON `error` alanı ve (dev modunda) request id düşünülebilir. |
| 8 | **TODO** | **Rate limit / retry** | Binance ve TV çağrılarında retry/backoff politikası eklenebilir. |
| 9 | **TODO** | **Config validasyonu** | Yükleme sonrası min_q_score, min_rr, risk_per_trade_pct vb. için aralık/format validasyonu eklenebilir. |

---

## İyileştirmeler

| # | Durum | Konu | Öneri |
|---|-------|------|--------|
| 10 | **TODO** | **API dokümantasyonu** | OpenAPI/Swagger tanımı eklenebilir. |
| 11 | **TODO** | **iqai-gui** | Stub durumu README’de “experimental/stub” olarak belirtilsin veya tamamlanıp dokümante edilsin. |
| 12 | **TODO** | **Piyasa saatleri** | BIST/NASDAQ saatleri config veya dil ayarından okunabilir. |
| 13 | **TODO** | **CORS** | Web API farklı origin’den kullanılacaksa Axum’da CORS middleware eklenmeli. |
| 14 | **TODO** | **Log önceliği** | RUST_LOG vs config “logging.level” önceliği dokümante edilsin. |
| 15 | **TODO** | **Pozisyon limiti** | Sembol bazlı max notional/pozisyon limiti (exchangeInfo/config) desteklenebilir. |

---

## Kritik (Fonksiyonel / Analitik Tutarlılık)

| # | Durum | Konu | Öneri |
|---|-------|------|--------|
| 16 | **DONE** | `trade_manager.rs` içinde `remaining_pct` clamp yok | `trade_manager.apply_action()` ve `auto_trader` close/partial_close akışlarında `pct` + `remaining_pct` clamp/guard eklendi. |
| 17 | **DONE** | `/api/chart` her poll’da `notify` spawn = potansiyel spam/tekrarlı bildirim | `crates/iqai-web/src/notify.rs`: in-memory throttle/dedup cache ile Q-Setup/Q-Analiz/Q-RADAR/Protect için aynı event-key tekrarını kısa aralıkta engellendi. |
| 18 | **TODO** | Backtest vs live davranış uyumsuzluğu | `run_strategy_plan_backtest` giriş tetikleme `entry_zone` ile uyumlu hale getirildi; partial TP1/TP2 + breakeven + trailing state’i eklendi. Ayrıca `run_backtest` artık fee (komisyon) ve `slippage_bps` ile effective exit benzeri PnL hesaplıyor. Kalan uyumsuzluk: plan-backtest ile trade-manager intrabar sıralaması ve TP2/TP3 / remaining_pct / trailing state’inin birebir eşleşmesi. |
| 19 | **DONE** | `auto_trader.rs` PnL hesabında `effective_exit` kullanılıyor ama `TradeLog.exit` `current_price` | `TradeLog.exit` ve `TradeEvent::PositionClosed`/`PartialClose` fiyat alanları `effective_exit` ile tutarlı hale getirildi. |
| 20 | **DONE** | Partial close’ların outcome/raporlama kapsamı belirsiz | `docs/ANALYSIS_DATA_LAYERS.md` içine kapsam notu eklendi: `analysis_outcomes` yalnızca `AutoTrader::close_position()` (tam kapanış) sırasında üretilir; `partial_close()` için yazılmaz. |

## Yüksek (Sinyal Kalitesi / False Positive Riski)

| # | Durum | Konu | Öneri |
|---|-------|------|--------|
| 21 | **DONE** | `candlestick_patterns.rs` ATR/volatilite filtresi yok | Son mumun range’i `ATR(14)`’e göre çok küçükse (noise filter) tüm pattern bayrakları `false` döndürülerek false positive azaltıldı. |

---

*Son güncelleme: 2026-03-20 - “Kritiklik sıralı bulgular” eklendi.*
