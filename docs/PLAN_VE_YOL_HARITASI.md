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
| 3 | **TODO** | **trade_db opened_count** – Sorguyu `status='open'` ile kısıtla veya alan adını (örn. total_positions) netleştir. | EKSIK_HATA §Olası Hata |
| 4 | **TODO** | **TradingMode::from_str** – "paper" için açık branch ekle. | Aynı |
| 5 | **TODO** | **api_q_analysis_all** – Boş symbols davranışını dokümante et. | Aynı |
| 6 | **TODO** | **Config validasyonu** – Yükleme sonrası min_q_score, min_rr, risk_per_trade_pct vb. aralık kontrolü. | EKSIK_HATA §Eksikler |

---

## Öncelik 3: Q-ANALİZ Geliştirmeleri (Özellik)

| # | Yapılacak | Açıklama |
|---|-----------|-----------|
| 7 | **Q-RADAR tespitine Q-Setup + Elliott hedefleri ekle** | Q-RADAR tespiti olduğunda: (a) Aynı buffer/TF ile `compute_q_setup` çağrılsın; (b) `compute_elliott` ile w5_targets ve corr_setup (Zigzag C / Triangle E) alınsın; (c) TP, Elliott hedefleriyle birleştirilsin veya “ikinci hedef” olarak sunulsun; (d) Çıktıya q_setup + elliott_targets alanları eklenebilir. Detay: Q_ANALIZ_WYCKOFF_POZ_KORUMA_CEVAP.md §3. |
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
| 12 | **TODO** | **OpenAPI/Swagger** – Web API dokümantasyonu. | EKSIK_HATA §İyileştirmeler |
| 13 | **TODO** | **CORS** – Web API farklı origin’den kullanılacaksa Axum’da CORS. | Aynı |
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
