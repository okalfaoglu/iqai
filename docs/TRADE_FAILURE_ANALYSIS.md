# Başarısız işlem tespiti, kök neden analizi ve üretim altyapısı

Bu doküman, **düşük kazanma oranı**, **beklenmedik zarar**, **tekrarlayan hatalar** ve **kod kaynaklı mı yoksa piyasa/yürütme kaynaklı mı** ayrımı için uygulanabilir bir çerçeve sunar. IQAI (`trade_db`, `auto_trader`, snapshot’lar) ile uyumlu olacak şekilde yazılmıştır.

---

## 0. Kapsam ve tanımlar

| Terim | Anlam |
|--------|--------|
| **Kayıp (loss)** | Negatif PnL ile kapanan işlem — *tek başına* “başarısızlık” değildir (strateji SL’i bilinçli kabul edebilir). |
| **Başarısızlık (failure)** | *Beklenen davranıştan sapma*: emir gitmedi, veri bozuk, mantık hatası, çifte emir, yanlış fiyat. |
| **İstatistiksel kötü performans** | Uzun vadede win rate / expectancy düşük — çoğunlukla strateji/piyasa rejimi; kök neden “tek bug” olmayabilir. |
| **Sistematik hata** | Aynı koşulda tekrarlayan yanlış sonuç — kod, parametre veya borsa entegrasyonu şüphesi. |

**Önemli:** “Başarı düşük” ile “sistem bozuk” aynı şey değildir. Önce **ölç**, sonra **sınıflandır**, sonra **derinleş**.

---

## 1. Başarısızlık sınıfları (detaylı taksonomi)

### 1.1 Piyasa / strateji (normal operasyon)

- Stop-loss veya trailing ile kapanış; negatif PnL ama kurallara uygun.
- **Sinyal:** `close_reason` / `TradeLog.reason` “Stop Loss”, “Take Profit”, trailing metinleri; backtest ile tutarlı davranış.

### 1.2 Yürütme (execution)

- Kısmi dolma, reddedilen emir, post-only çakışması, reduce-only hatası.
- **Sinyal:** borsa `code` / `msg`, `executed_qty` ≠ beklenen, beklenmeyen ortalama fiyat.

### 1.3 Operasyon / altyapı

- API anahtarı, IP kısıtı, rate limit (429), ağ kesintisi, saat senkronu (NTP drift), disk dolu.
- **Sinyal:** HTTP durum kodları, timeout, art arda yeniden deneme logları.

### 1.4 Veri kalitesi

- Eksik mum, yanlış sıralama, boş OHLC, farklı timeframe karışması.
- **Sinyal:** `CandleBuffer` uzunluğu ani düşüş, spike detection, son fiyat vs borsa uyuşmazlığı.

### 1.5 Kod / mantık

- Yanlış dal (if/else), birim hatası (USDT vs kontrat), `remaining_pct` tutarsızlığı, backtest ≠ canlı (T-5).
- **Sinyal:** `ERROR` log, assert, aynı fixture’da replay farkı.

### 1.6 Güvenlik / yetkisiz işlem

- Beklenmeyen sembol, beklenmeyen boyut — çoğunlukla konfig veya API sızıntısı.
- **Sinyal:** audit log, beklenmeyen `order_id` deseni.

### 1.7 İnsan / süreç

- Manuel kapatma, parametre yanlış deploy, yanlış `config.json` ortamı.

---

## 2. IQAI veri modeli (mevcut ve hedef)

### 2.1 Şu an kullanılan yapılar

| Bileşen | Dosya / tablo | Not |
|---------|----------------|-----|
| `positions` | `trade_db` | `status`, `closed_at`, `pnl`, `close_reason`, `exit_price`, `mode`, `trace_id`, `position_uuid`, RCA alanları. |
| `signals` | `trade_db` | Her değerlendirme satırı; `trace_id` ile pozisyon zinciri (O-05). |
| Korelasyon logları | `auto_trader` | `format_trade_correlation(trace_id, signal_id, position_uuid, order_id)` — grep / Loki. |
| `TradeLog` | `auto_trader` | `reason` — kapanış/kısmi sebebi (serbest metin). |
| `TradeEvent` | `auto_trader` | `PositionClosed`, `PartialClose`, `SlUpdated` — olay akışı. |
| `AnalysisSnapshot` / snapshot tablosu | `analysis_snapshot`, `trade_db` | İşlem anındaki Q-Analiz durumu (geri dönük “ne biliyorduk?”). |
| `TradeAnalysisLink` | `trade_db` | Pozisyon ↔ sinyal/snapshot bağlantısı. |
| `AnalysisOutcomeRecord` | `trade_db` | Tespit sonrası horizon performansı (MFE/MAE, TP/SL isabet). |

### 2.2 Önerilen sıkılaştırma (P0–P1)

1. **`close_reason` / `TradeLog.reason` sözlüğü:** Serbest metin yerine sabit etiketler: `SL`, `TP`, `TP_PARTIAL`, `TRAIL`, `MANUAL`, `LIQ`, `ERROR_EXCHANGE`, `ERROR_INTERNAL`, `CANCELLED` … SQL gruplama için şart.
2. **Opsiyonel sayısal alanlar:** `exit_slippage_bps`, `intended_price` vs `fill_price`, `exchange_error_code`.
3. **Korelasyon:** `position_id`, `run_id` / `daemon_instance_id`, isteğe bağlı `trace_id` (HTTP veya dahili).

---

## 3. Gözlemlenebilirlik katmanı

### 3.1 Loglama

- **Yapılandırılmış log** (JSON veya `key=value`): `timestamp`, `level`, `symbol`, `position_id`, `mode`, `event`, `reason`, `pnl`, `latency_ms`.
- **Ayrım:** Strateji kayıpları `INFO` veya `WARN`; geri kazanılamayan durumlar `ERROR`.
- **PII:** API anahtarı, tam IP — loga yazma; hata kodu yeter.

### 3.2 Metrikler (örnek isimler)

- `trades_closed_total{reason, symbol, mode}`
- `order_reject_total{code}`
- `signal_to_fill_latency_ms` (histogram)
- `position_count_open{mode}`
- **TFAI-Q04 (IQAI):** Normalize Binance (ve benzeri) hataları için `iqai_exchange_normalized_errors_total{exchange,category,tier}` — sınıflandırma sırasında bellekte birikir, Prometheus scrape’inde `sli_counters` içindeki `q04_norm:v1:*` anahtarlarına flush edilir (süreç içi kalıcı toplamlar). Ayrıntı: `docs/SLI_METRICS.md`.

### 3.3 İzlenebilirlik (trace)

- Tek bir pozisyon ömrü: sinyal → emir → kısmi → kapanış zincirinde **aynı `correlation_id`**.
- Web istekleri için `request_id` (Axum middleware).

---

## 4. Kök neden analizi yöntemleri

1. **Beş Neden:** “Neden zarar?” → “Neden SL?” → … kök: veri mi, parametre mi, kod mu?
2. **Zaman çizelgesi:** İşlem ID’si ile log + DB satırlarını kronolojik birleştir.
3. **Karşılaştırma:** Aynı `symbol`, `timeframe`, mum aralığı ile `run_strategy_plan_backtest` / `run_backtest` — sapma varsa sürüm veya T-5 farkı.
4. **Kontrol listesi — kod mu?**
   - Aynı veriyle replay edilebiliyor mu?
   - Panic / `unwrap` yolu var mı?
   - Son deploy commit’i biliniyor mu?

---

## 5. Sorgular ve paneller (örnek SQL fikirleri)

- Sembol bazında: `COUNT(*)`, `SUM(pnl)`, `AVG(pnl)`, `win_rate`, `GROUP BY close_reason`.
- Zaman penceresi: son 7 gün `paper` vs `dry` (aynı kod, farklı DB).
- **Anomali:** `close_reason = 'ERROR_%'` oranı ani yükseldiyse uyarı.
- **Snapshot karşılaştırması:** Kayıplı işlemlerde `confidence_score` / `recommendation` dağılımı (sinyal kalitesi analizi).

---

## 6. Uyarı ve otomasyon

| Tetikleyici | Örnek eşik |
|-------------|------------|
| Red oranı | `order_reject_total` 5 dk’da N’den fazla |
| Veri | Mum sayısı &lt; beklenen minimum |
| PnL | Günlük drawdown &gt; X% (parametreli) |
| Kod | `ERROR` log oranı ani artış |

**Not:** Finansal “alarm” ile teknik “alarm”ı ayırın; ikincisi on-call, birincisi risk yönetimi.

---

## 7. Yapay zekâ kullanımı (sınırlar ve fayda)

**Uygun kullanım**

- Log + metrik özetinden **doğal dil özeti** (insan okuması için).
- Anomali **açıklaması** (“bu saatte red oranı yükseldi”) — karar değil.

**Uygun olmayan / riskli kullanım**

- “Bu işlem neden kaybetti?” için **tek başına LLM cevabına güvenmek** — halüsinasyon ve uyumluluk riski.
- Otomatik **parametre değişikliği** veya **canlı emir** LLM’e bağlamak.

**Prompt ilkesi (IQAI’de benzeri):** Yatırım tavsiyesi, sabit hedef fiyat, al/sat emri üretmemesi; sadece açıklayıcı özet.

---

## 8. Herhangi bir AI asistanına sorulacak derinlemesine soru seti

Aşağıdaki liste, **başka bir sohbet AI’sına** veya **dış danışmana** aynı problemi anlatırken kullanılabilir; cevaplar tasarım kararlarını netleştirir.

**İngilizce prompt paketi + cevap anahtarları (`TFAI-Q01` … `TFAI-Q14`):** `docs/TRADE_FAILURE_AI_PROMPTS.md` — Cursor’a cevap yapıştırırken bu anahtarları kullanın.  
Dış AI cevaplarının IQAI ile eşlemesi: `docs/TRADE_FAILURE_AI_RESPONSES_SYNTHESIS.md`.  
Claude TFAI tam metin (şema/örnekler): `docs/TRADE_FAILURE_TFAI_CLAUDE_FULL.md`.

### 8.1 Veri ve olay modeli

1. Kapalı bir pozisyon için hangi **minimum alan seti** olmadan kök neden analizi yapılamaz?
2. `close_reason` için **kapalı bir taksonomi** nasıl tasarlanır; genişleme nasıl sürümlenir?
3. Kısmi TP ve tam kapanış aynı `position_id` altında nasıl **event sourcing** ile modellenir?
4. Borsa hata kodları (Binance vb.) nasıl **normalize** edilir?  
   *IQAI uygulaması:* `binance_error` / `classify_binance_json`; uyarı katmanı `alert_tier()` — `docs/ALERT_TIERS.md`. Sayaçlar `iqai_exchange_normalized_errors_total` (Prometheus) ve kalıcı toplamlar `sli_counters` (`q04_norm:v1:*`) — `docs/SLI_METRICS.md`.

### 8.2 Gözlemlenebilirlik

5. Bir işlem ömrü için **OpenTelemetry** benzeri tek `trace_id` yeterli mi; hangi span’lar gerekir?
6. Hangi metrikler **SLI** olarak tanımlanır (ör. “emir başarılı gönderim oranı”)?
7. Log hacmi patlamasını önlemek için **örnekleme** (sampling) stratejisi?

### 8.3 İstatistik ve yanlış pozitif

8. Düşük win rate’i **rejim değişimi** ile **bug** ayırt etmek için hangi istatistiksel testler kullanılır?
9. Çoklu karşılaştırma (çok sembol) için **FDR** (false discovery rate) düşünülmeli mi?

### 8.4 Güvenlik ve uyumluluk

10. Üretim loglarında hangi veriler **asla** tutulmamalı (regülasyon / GDPR benzeri)?
11. “AI açıkladı” çıktısı **denetlenebilir** nasıl yapılır (kaynak log satırlarına referans)?

### 8.5 Otomasyon ve runbook

12. “Kod kaynaklı” şüphe için **otomatik** hangi kontroller çalıştırılır (test, replay)?
13. Incident sonrası **postmortem** şablonu nasıl olmalı?

### 8.6 Organizasyon

14. Bu altyapının sahibi **teknik ekip mi risk ekibi mi**; onay akışı?

*(Bu sorulara verilen cevapları doğrudan üretime uygulamadan önce kod ve güvenlik incelemesi şarttır.)*

---

## 9. Yol haritası (özet)

| Öncelik | İş |
|--------|-----|
| P0 | `close_reason` / `reason` sözlüğü; PnL/raporlarda filtre |
| P1 | Yapılandırılmış log + `position_id` / korelasyon |
| P2 | Sembol × nedeni dağılımı paneli; basit anomali uyarıları |
| P3 | Tam replay ortamı; deploy sürüm etiketi her log satırında |
| P4 | (İsteğe bağlı) harici APM / metrics backend |

İlgili kod uyumu: `docs/BACKTEST_TRADE_MANAGEMENT.md` (T-05); web duman testleri: `crates/iqai-web/tests/http_smoke.rs`.

---

## 10. Ön yüz ve raporlama hataları

Tarayıcıda görünen hatalar (ör. eski JS değişken adları) **PnL verisiyle karıştırılmamalı**; ayrı issue olarak takip edin. API/JSON hata standardı: `docs/API_ERRORS.md`.

---

## 11. Özet kontrol listesi (günlük kullanım)

- [ ] Kayıplı işlem için DB’de `close_reason` ve zaman damgası var mı?
- [ ] Aynı sembolde tekrarlayan **aynı** hata kodu var mı?
- [ ] O anki **snapshot** veya link kaydı var mı?
- [ ] Son deploy **commit / sürüm** biliniyor mu?
- [ ] Sorun **tek işlem mi** yoksa **toplu** mı?

---

## 12. Kaynaklar (repo içi)

- `docs/TRADE_FAILURE_PROGRESS.md` — **TFAI / P0–P4 / Q01–Q14 uygulama durumu** (işaretli checklist)
- `docs/POSTMORTEM_TEMPLATE.md` — olay sonrası özet şablonu (TFAI-Q13)
- `docs/ALERT_TIERS.md` — normalize borsa hatası → uyarı katmanı (TFAI-Q04)
- `docs/SLI_METRICS.md` — Prometheus / `sli_counters`; TFAI-Q04: `iqai_exchange_normalized_errors_total` ve `q04_norm:v1:*` kalıcı sayaçlar
- `docs/PROMETHEUS_ALERT_EXAMPLES.md` — örnek uyarı kuralları (P2)
- `docs/OPERATIONS_GOVERNANCE.md` — sahiplik / acil / deploy özeti (TFAI-Q14)
- `docs/ANALYSIS_DATA_LAYERS.md` — veri katmanları
- `docs/TRADE_FAILURE_ANALYSIS.md` (bu dosya)
- `crates/iqai-core/src/trade_db.rs` — şema ve kayıt türleri
- `crates/iqai-core/src/auto_trader.rs` — `TradeLog`, `TradeEvent`

---

*Son güncelleme: doküman genişletildi (operasyonel taksonomi, AI sınırları, çok katmanlı soru seti, IQAI eşlemesi). TFAI-Q04 metrikleri `SLI_METRICS.md` ile hizalandı (Prometheus + SQLite `sli_counters`).*
