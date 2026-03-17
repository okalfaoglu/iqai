# Q-Analiz: Dip / Tepe Tespiti ve Ekran Verileri

Bu doküman, IQAI’de bir sembol için **dip (bottom)** ve **tepe (top)** tespitinin nasıl yapıldığını, ekrandaki **Tespit**, **Güven**, **Erken Uyarı** ve **Tavsiye** alanlarının hangi yöntem, indikatör, pattern ve verilerle hesaplandığını tek tek açıklar.

---

## 1. Kullanılan temel veri (Raw Data)

| Veri | Kullanım |
|------|----------|
| **OHLCV** | Tüm hesaplamalar sadece **Open, High, Low, Close, Volume** ile yapılır. |
| Order book, funding, open interest, on-chain | **Kullanılmıyor.** |

Kaynak: `Candle` (`types.rs`), `CandleBuffer` ile çoklu timeframe (M1, M5, M15, M30, H1, H4, D1).

---

## 2. Dip / tepe matematiksel tespiti (Pivot)

**Yöntem:** Fractal tarzı **Pivot Low / Pivot High** (TradingView `ta.pivotlow` / `ta.pivothigh` ile uyumlu).

- **Dip:** Merkez barın `low` değeri, sol ve sağdaki `length` barın `low` değerlerinden **kesinlikle düşük** olmalı.
- **Tepe:** Merkez barın `high` değeri, sol ve sağdaki `length` barın `high` değerlerinden **kesinlikle yüksek** olmalı.

**Parametre:** `config.pivot_length` (varsayılan **5**) → 5 bar sol, 1 merkez, 5 bar sağ (toplam 11 bar penceresi).

**Kod:** `indicators.rs` → `pivot_low()`, `pivot_high()`; `reversal.rs` → `get_dip_price_and_index()`, `get_peak_price_and_index()`.

---

## 3. Destek / direnç bölgesi (MTF)

- **Üst timeframe destek:** Her üst TF’de (M5→M15→…→D1) pivot low (long) veya pivot high (short) alınır.
- **Bölge:** `support ± 0.5 * ATR(14)`.
- **Kontrol:** Referans fiyat bu bandın içindeyse `mtf_support_near = true` (confluence katmanı).

**Kod:** `dip_confluence.rs` → `compute_dip_confluence()`, `mtf_support_near`.

---

## 4. RSI

- **Hesaplama:** Klasik RSI, period = **14** (`indicators.rs` → `rsi()`).
- **Kullanım:**
  - **Bölge filtresi:** Long için `RSI < q_rsi_oversold` (varsayılan 35), short için `RSI > q_rsi_overbought` (varsayılan 65) → `rsi_zone_ok`.
  - **Divergence:** Bullish: son iki pivot low’da fiyat **LL** (lower low), RSI **HL** (higher low). Bearish: son iki pivot high’da fiyat **HH**, RSI **LH**.

**Kod:** `dip_confluence.rs` → `bullish_divergence()`, `bearish_divergence()`, `rsi_zone_ok`.

**Ek:** MACD divergence ve Bollinger, Madde 15 skorlamasında kullanılır (`dip_tepe_scoring.rs`, `indicators.rs` → `macd()`, `bollinger()`). RSI eşikleri klasik 30/70 için config: `smart_money.q_rsi_oversold: 30`, `q_rsi_overbought: 70`.

---

## 5. Fibonacci / Elliott

- **Elliott + Fib cluster:** `elliott_detector.rs` → impulse (W1–W5), correction (A–B–C), Fib seviyeleri (0.382, 0.5, 0.618, 0.786 vb.).
- **Confluence:** Fiyat, Elliott entry (W3/W5) veya `fibo_levels` içindeki bir seviyeye **%0.3** bandında yakınsa `fib_elliott_zone = true`.

**Kod:** `dip_confluence.rs` → `compute_elliott()`, `ref_levels`, `FIB_PRICE_BAND_PCT = 0.003`.

---

## 6. Hacim (Volume)

- **Dönüş gücü:** Son 20 bar ortalama hacimle son mum hacmi oranı → `vol_ratio` (max 1.0), `reversal_strength` içinde **%30** ağırlık.
- **Absorption:** Son 5 bar ortalama hacim ≥ son 20 bar ort. hacmin **1.5** katı ve fiyat destek/tepe bandında kalıyorsa `absorption_ok = true`.
- **RADAR / Q-Setup:** Hacim filtreleri (vol_condition, vol_ratio) sinyal ve momentum skorunda kullanılır.

**Kod:** `reversal.rs` → `reversal_strength_from_dip` / `decline_strength_from_peak`; `dip_confluence.rs` → `absorption_ok`; `signal.rs` → volume SMA, vol_condition.

---

## 7. Mum yapısı (Candlestick)

- **Yön:** `is_bullish()` (close > open), `is_bearish()` (close < open).
- **Güç:** Dönüş gücünde son mum gövdesi / ATR → `body_ratio` (**%20** ağırlık). Q-Setup’ta `body_ratio` + `vol_ratio` → momentum_score.

**Pattern (Madde 9):** Dip: Hammer, Bullish Engulfing, Morning Star, Piercing. Tepe: Shooting Star, Bearish Engulfing, Evening Star, Dark Cloud Cover. Skorlama: `candlestick_patterns.rs` → `detect_candle_patterns()`.

**Kod:** `reversal.rs` → `reversal_strength_from_dip`; `signal.rs` → `c.is_bullish()`, momentum_score.

---

## 8. Trend analizi

- **Per TF:** `close` vs **EMA(20)** ve **VWAP(hlc3)**.  
  - close ≥ EMA ve ≥ VWAP → trend = **1** (yukarı).  
  - close ≤ EMA ve ≤ VWAP → trend = **-1** (aşağı).  
  - Diğer → **0** (nötr).
- **Trend gücü:** Tüm 7 TF’nin trend değerleri toplanıp `(toplam/7)*100` → -100 ile +100 arası.

**Kod:** `signal.rs` → `trend_for_tf()`, `trend_strength()`.

---

## 9. Market structure (yapı)

- **Yapı skoru:** Long için son pivot low > önceki pivot low (**higher low**) → 1.0; değilse 0.3. Short için son pivot high < önceki pivot high (**lower high**) → 1.0; değilse 0.3.
- **BOS (Break of structure):** Long için son kapanış > son pivot high; short için son kapanış < son pivot low → `bos_ok`.

**Kod:** `signal.rs` → `structure_score()`; `dip_confluence.rs` → `bos_ok`.

---

## 10. Wyckoff (Spring / Upthrust)

- **Spring (dip):** Dip barından sonra fiyat dip altına inip, en fazla **4 bar** içinde tekrar dip üstüne dönmüş mü → `spring_detected`.
- **Upthrust (tepe):** Tepe barından sonra fiyat tepe üstüne çıkıp, en fazla 4 bar içinde tekrar tepe altına dönmüş mü → `upthrust_detected`.

**Kod:** `reversal.rs` → `detect_spring()`, `detect_upthrust()`; `dip_confluence.rs` → `spring_ok`, `absorption_ok` ile birlikte kullanım.

---

## 11. Zaman fazı (Fibo time phase)

- Son pivot low/high barı “döngü başlangıcı” kabul edilir.
- **bars_since_start / 32** (Fibo 1,2,3,5,8,13 toplamı) → **[0–1.5]** arası faz.
- RADAR sadece `q_radar_phase_min`–`q_radar_phase_max` aralığında tetiklenir; Q-Setup’ta giriş fazı ve “late phase” için kullanılır.

**Kod:** `signal.rs` → `fibo_time_phase()`.

---

## 12. Confluence katmanları (8 katman)

Her biri “geçti” ise güven ve erken uyarı skorları artar:

| # | Katman | Açıklama |
|---|--------|----------|
| 1 | **mtf_support_near** | Üst TF pivot ± 0.5×ATR bandında fiyat |
| 2 | **ltf_structure_ok** | structure_score ≥ 0.55 (HL long / LH short) |
| 3 | **fib_elliott_zone** | Fiyat Elliott/Fib seviyesine %0.3 bandında |
| 4 | **divergence_ok** | RSI divergence (bullish long / bearish short) |
| 5 | **spring_ok** / **upthrust_ok** | Wyckoff spring (dip) veya upthrust (tepe) |
| 6 | **rsi_zone_ok** | Long: RSI < 35; short: RSI > 65 |
| 7 | **bos_ok** | Break of structure (son tepe/dip kırıldı) |
| 8 | **absorption_ok** | Band içinde hacim artışı + fiyat bandında |

**Artış:** Katman başı **+0.6** (confidence ve early_warning’e), toplam cap **+2.5**.

**Kod:** `dip_confluence.rs` → `layers_passed`; `q_radar_analysis.rs` → `CONFLUENCE_BOOST_PER_LAYER`, `CONFLUENCE_BOOST_CAP`.

---

## 13. RADAR sinyali (Q-RADAR)

RADAR, “erken uyarı” bandında dip/tepe yönünde sinyal üretir.

- **Yön:** `trend_strength` + son mum: long için trend > 0 ve mum yükseliş; short için trend < 0 ve mum düşüş.
- **Faz:** `fibo_time_phase` `q_radar_phase_min`–`q_radar_phase_max` içinde olmalı.
- **Güven (0–1):**  
  `confidence = dir_score_norm*0.5 + conf_norm*0.3 + phase_score*0.2`  
  - `dir_score_norm`: trend gücü (0–100) → 0–1.  
  - `conf_norm`: system_confidence (50–90) → 0–1.  
  - `phase_score`: fazın band ortasına yakınlığı.  
- **system_confidence:** Tüm TF’lerde trend aynı yönde (7 veya -7) → 90; 4+ veya -4+ → 75; 2+ veya -2+ → 60; diğer → 50.

**Kod:** `signal.rs` → `compute_q_radar()`, `system_confidence()`.

---

## 14. Dönüş gücü (reversal_strength / decline_strength)

**Dip (0–1):**

- Bounce (son close − dip fiyatı) ATR ile normalize; **2 ATR = 1.0** → strength_atr.
- Son 20 bar ortalama hacme göre son mum hacmi → vol_ratio (max 1.0).
- Son mum gövdesi / ATR → body_ratio (max 1.0).
- **reversal_strength = 0.5×strength_atr + 0.3×vol_ratio + 0.2×body_ratio.**

**Tepe:** Aynı mantık, decline ve bearish body ile **decline_strength**.

**Kod:** `reversal.rs` → `reversal_strength_from_dip()`, `decline_strength_from_peak()`.

---

## 15. Ekrandaki alanların üretimi

### Fiyat

- **reference_price:** RADAR varsa RADAR’ın reference_price’ı, yoksa chart TF’deki son mumun **close** değeri.
- Değişim yüzdesi uygulama tarafında (örn. önceki kapanışa göre) hesaplanır.

### Tespit (Detection)

- **"DİP BÖLGESİ (TEPKİ DİBİ)":** RADAR long veya dip analizi (reversal_detected + reversal_strength ≥ 0.5) ile long tespit.
- **"TEPE BÖLGESİ (TEPKİ TEPESİ)":** RADAR short veya tepe analizi ile short tespit.
- **"—":** Hiçbiri yoksa.

**Kod:** `q_radar_analysis.rs` → `build_detection_and_recommendation()`.

### Güven (Confidence, 0–10)

- **RADAR varsa:** `conf_10 = RADAR.confidence * 10` (RADAR güveni 0–1’den).
- **RADAR yok, sadece dip/tepe varsa:** `conf = (early * 0.4).min(10)` (early = reversal_strength*10).
- **Confluence sonrası:** `confidence_score = (conf_10 + confluence_boost).min(10)`.

Yani güven: RADAR’ın yön + system_confidence + faz skorundan ve/veya dönüş gücünden gelir; confluence ile yükselir.

### Erken Uyarı (Early Warning, 0–10)

- **RADAR long:** `early_10 = min(dip.reversal_strength * 10, 10)` (dip yoksa RADAR confidence*10).
- **RADAR short:** `early_10 = min(peak.decline_strength * 10, 10)`.
- **Confluence sonrası:** `early_warning_score = (early_10 + confluence_boost).min(10)`.

Yani erken uyarı: dönüş gücü (bounce/ATR + hacim + gövde) veya RADAR güveni; confluence ile yükselir.

### Tavsiye (Recommendation)

| Koşul | Tavsiye |
|-------|---------|
| confidence_score ≥ 7 **ve** early_warning_score ≥ 7 | **GÜÇLÜ DİP – İzle** / **GÜÇLÜ TEPE – İzle** |
| confidence_score ≥ 5 **veya** early_warning_score ≥ 5 (ama yukarıdaki değil) | **ZAYIF DİP – İzle** / **ZAYIF TEPE – İzle** |
| Diğer (RADAR/dip path’te) | DİP BÖLGESİ – İzle / TEPE BÖLGESİ – İzle |
| Tespit yok | — |

**Kod:** `q_radar_analysis.rs` → `build_detection_and_recommendation()`, sonra confluence sonrası `final_recommendation` güncellemesi.

### Skor (sinyal bazlı, 0–10) – Madde 15

Tespit varken ek olarak **sinyal → puan** tablosu hesaplanır; çıktı `QRadarOpportunityAnalysis.discrete_score` içinde gelir (Telegram/Web kartında “Skor” satırı).

| Sinyal | Puan |
|--------|------|
| RSI aşırı satım/alım | +1 |
| MACD divergence | +2 |
| Destek/direnç bölgesi (MTF dahil) | +2 |
| Hacim spike (vol > vol_MA×1.5) | +1 |
| Yükseliş/düşüş mumu veya pattern | +1 |
| Fibonacci seviyesi (0.382–0.786) | +1 |
| Fiyat EMA200 yakın | +1 |
| Piyasa yapısı (HL/LH) | +1 |
| Bollinger reversion (dip: close < lower, tepe: close > upper) | +1 |
| Ortalamadan sapma (mean reversion) | +1 |

**Toplam:** 0–10 (cap). **Tavsiye (Madde 17):** ≥8 → STRONG BUY/SELL, ≥6 → BUY/SELL ZONE, ≥4 → WATCH, <4 → NO SIGNAL. **Erken uyarı momentum:** RSI slope yukarı veya MACD histogram dönüşü.

**Kod:** `dip_tepe_scoring.rs` → `compute_dip_tepe_score()`; `indicators.rs` → `macd()`, `bollinger()`; `candlestick_patterns.rs`.

---

## 16. Özet pipeline (ekran çıktısına giden akış)

```
OHLCV (CandleBuffer, çoklu TF)
    ↓
Pivot Low / Pivot High (pivot_length=5)
    ↓
Reversal analizi (dip/tepe fiyatı, reversal_detected, reversal_strength, spring/upthrust)
    ↓
RADAR (trend + mum + faz + system_confidence → confidence, side)
    ↓
build_detection_and_recommendation (Tespit, conf_10, early_10, Tavsiye taslağı)
    ↓
Confluence (8 katman → layers_passed, boost)
    ↓
confidence_score, early_warning_score güncellenir
    ↓
final_recommendation (GÜÇLÜ / ZAYIF / BÖLGESİ)
    ↓
UI: Fiyat, YÖN, Tespit, Güven [■■■■□□□□□□] 4/10, Erken Uyarı DİP 8/10, Tavsiye ZAYIF DİP – İzle
```

---

## 17. Senin ekrandaki örnekle eşleşme

- **Güven 4/10:** RADAR confidence * 10 ≈ 4 veya confluence öncesi conf_10 = 4. Erken uyarı 8 olduğu için “ZAYIF” (7+7 sağlanmıyor, 5+ sağlanıyor).
- **Erken Uyarı DİP 8/10:** `reversal_strength` ≈ 0.8 (güçlü bounce/hacim/gövde).
- **ZAYIF DİP – İzle:** İkisi de ≥ 7 olmadığı için “GÜÇLÜ” değil; biri veya ikisi ≥ 5 olduğu için “ZAYIF”.

Bu dokümandaki tüm formüller ve eşikler, `crates/iqai-core` içindeki ilgili modüllerde tanımlıdır; config değerleri `config.rs` ve `app_config.rs` üzerinden değiştirilebilir.

---

## 18. Hesaplamalar doğrulama (doc ↔ kod)

Aşağıdaki değerler dokümandaki tanımla kodda birebir uyumludur:

| Madde | Doküman | Kod (değer / dosya) |
|-------|---------|---------------------|
| §2 | Pivot length 5 | `config.pivot_length` (default 5), `reversal.rs` DEFAULT_PIVOT_LEN |
| §3 | MTF band 0.5×ATR | `dip_confluence.rs` MTF_ATR_BAND = 0.5 |
| §4 | RSI 35/65 (veya 30/70) | `config.q_rsi_oversold` / `q_rsi_overbought`; confluence + scoring |
| §5–7 | Fib band %0.3 | FIB_PRICE_BAND_PCT = 0.003 |
| §9 | Structure ≥ 0.55 | STRUCTURE_SCORE_MIN = 0.55 |
| §12 | Confluence +0.6, cap +2.5 | CONFLUENCE_BOOST_PER_LAYER = 0.6, CONFLUENCE_BOOST_CAP = 2.5 |
| §14 | reversal_strength 0.5/0.3/0.2, 2 ATR = 1.0 | reversal.rs WEIGHT_*, STRENGTH_ATR_FULL = 2.0 |
| §15 | GÜÇLÜ 7+7, ZAYIF 5+ | q_radar_analysis.rs build_detection + final_recommendation |
| §15 (skor) | Sinyal puanları, 0–10, STRONG/BUY ZONE/WATCH/NO SIGNAL | dip_tepe_scoring.rs compute_dip_tepe_score |

Hesaplamalar dokümana göre tamamlanmış durumdadır.

---

## 19. AI (Ollama) ile kullanım

Q-Analiz daemon (`q-analiz-daemon`) tespit bulduğunda, config’te `ai.enabled: true` ise Ollama’ya şu context gönderilir:

- **Sembol, TF, Tespit, Yön, Tavsiye**
- **Güven (0–10)** ve **Erken uyarı (0–10)**
- **Fiyat**, **Onay katmanları** (örn. 3/8 katman)
- **Skor (sinyal):** toplam 0–10, discrete tavsiye (STRONG BUY / WATCH vb.), **aktif sinyaller listesi** (RSI aşırı satım, Destek bölgesi, …)
- İsteğe bağlı **Elliott özeti**

Böylece AI, hem klasik alanları hem de sinyal bazlı skoru ve hangi sinyallerin tetikli olduğunu görerek yorum yapar. Q-Analiz yeni bilgileri (discrete_score, confidence, early_warning, confirmation_layers) AI ile kullanmaya hazırdır.
