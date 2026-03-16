# IQAI – Eksik / Hata / İyileştirme Özeti

Proje satır satır incelenerek tespit edilen maddelerin kısa listesi. Detay için `PROJE_DOKUMANTASYONU.md` §10’a bakın.

---

## Kritik (Güvenlik)

| # | Konu | Öneri |
|---|------|--------|
| 1 | **config.json’da gerçek token/şifre** | Telegram token, TV şifresi repo/ortamda düz metin olmamalı. Değerler değiştirilsin; hassas config sadece örnek (config.json.example) ile paylaşılsın. |
| 2 | **API anahtarları** | Canlı modda api_key/secret_key mümkünse env’den okunsun; örnek config’te asla gerçek değer kullanılmasın. |

---

## Olası Hata / Tutarsızlık

| # | Konu | Öneri |
|---|------|--------|
| 3 | **api_q_analysis_all – boş symbols** | Config’te `symbols: []` iken varsayılan semboller kullanılıyor; davranış dokümante edilsin. |
| 4 | **trade_db opened_count** | `get_symbol_pnl_stats` içinde “opened_count” aslında toplam pozisyon sayısı (açık+kapalı). Sorgu `status='open'` ile kısıtlansın veya alan adı (örn. total_positions) netleştirilsin. |
| 5 | **TradingMode::from_str** | "paper" için açık branch eklenmesi (şu an `_ => Paper` ile dolaylı); okunabilirlik için iyileştirilebilir. |

---

## Eksikler

| # | Konu | Öneri |
|---|------|--------|
| 6 | **Test kapsamı** | Sinyal motoru, Q-Setup, confluence, Elliott için birim testleri; exchange mock ile entegrasyon testleri eklenebilir. |
| 7 | **Hata cevabı standardı** | API/DB hatalarında tutarlı JSON `error` alanı ve (dev modunda) request id düşünülebilir. |
| 8 | **Rate limit / retry** | Binance ve TV çağrılarında retry/backoff politikası eklenebilir. |
| 9 | **Config validasyonu** | Yükleme sonrası min_q_score, min_rr, risk_per_trade_pct vb. için aralık/format validasyonu eklenebilir. |

---

## İyileştirmeler

| # | Konu | Öneri |
|---|------|--------|
| 10 | **API dokümantasyonu** | OpenAPI/Swagger tanımı eklenebilir. |
| 11 | **iqai-gui** | Stub durumu README’de “experimental/stub” olarak belirtilsin veya tamamlanıp dokümante edilsin. |
| 12 | **Piyasa saatleri** | BIST/NASDAQ saatleri config veya dil ayarından okunabilir. |
| 13 | **CORS** | Web API farklı origin’den kullanılacaksa Axum’da CORS middleware eklenmeli. |
| 14 | **Log önceliği** | RUST_LOG vs config “logging.level” önceliği dokümante edilsin. |
| 15 | **Pozisyon limiti** | Sembol bazlı max notional/pozisyon limiti (exchangeInfo/config) desteklenebilir. |

---

*Son güncelleme: Proje incelemesi çıktısı.*
