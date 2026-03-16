# Q-ANALİZ – Detaylı Teknik Dokümantasyon

Bu doküman Q-ANALİZ yönteminin bileşenlerini, formüllerini, veri yapılarını ve akışını kodla uyumlu biçimde tanımlar. Referans: `iqai-core` (signal.rs, q_radar_analysis.rs, dip_confluence.rs, reversal.rs, types.rs, config.rs).

---

## 1. Genel Tanım ve Bileşenler

**Q-ANALİZ**, Smart Money yapısına dayalı dip/tepe bölgesi tespiti ve işlem fırsatı üretme yönteminin genel adıdır. Üç ana çıktıyı kapsar:

| Bileşen | Açıklama | Çıktı tipi |
|--------|----------|------------|
| **Q-RADAR** | Setup’tan önce tetiklenen erken uyarı; zaman penceresi odaklı. | `QRadarSignal` |
| **Q-Setup** | Somut işlem fırsatı: yön, giriş bölgesi, entry, SL, TP, Q-skor, zaman penceresi. | `QSetup` |
| **Poz Koruma** | Kar içindeyken zorunlu koruma/çıkış uyarısı (trailing veya geç faz). | `ProtectSignal` |

**Not:** Q-Setup hesaplaması Elliott Wave kullanmaz; girdiler çok zaman dilimli trend (EMA/VWAP), pivot, ATR, Fibo-zaman fazı, yapı skoru (HH/HL) ve momentum/hacimdir. Elliott Wave ayrı modülde (grafik/formasyon listesi) kullanılır.

---

## 2. Temel Kavramlar

### 2.1 Fibo-zaman fazı (phase)

Zaman ekseninde “döngü içinde neredeyiz?” tahminidir. Tam döngü tespiti yok; son pivot (low veya high) döngü başlangıcı kabul edilir, Fibo bar uzunlukları (1,2,3,5,8,13 → toplam 32 bar) ile normalize edilir.

- **Kaynak:** `signal.rs` → `fibo_time_phase(candles, pivot_len)`  
- **Çıktı:** `[0, 1.5]` aralığında (1.5’e taşma kırpılır).  
- **Kullanım:**  
  - Q-RADAR sadece **erken faz**da tetiklenir: `q_radar_phase_min ≤ phase ≤ q_radar_phase_max` (varsayılan 0.1–0.3).  
  - Q-Setup **giriş fazı** için ideal: `q_entry_phase_min ≤ phase ≤ q_entry_phase_max` (varsayılan 0.2–0.6).  
  - **Geç faz** (`phase ≥ q_late_phase`, varsayılan 0.7): Poz Koruma “LATE_PHASE” veya time_score=0.

### 2.2 Pivot (L_pivot / H_pivot)

TradingView tarzı pivot high/low: merkez barın sol ve sağında `pivot_length` bar hiç daha yüksek/düşük değil.

- **Kaynak:** `indicators.rs` → `pivot_high(candles, length)`, `pivot_low(candles, length)`.  
- **Kullanım:** Giriş bölgesi, SL, yapı skoru (HH/HL), dip/tepe fiyatı.

### 2.3 Yapı skoru (structure_score)

Son iki pivot karşılaştırması: Long için HL (higher low), Short için LH (lower high) varsa 1.0; yoksa 0.3.

- **Kaynak:** `signal.rs` → `structure_score(candles, side, pivot_len)`.

### 2.4 Trend ve güven

- **trend_strength:** 7 zaman dilimindeki trend yönünün toplamı (−7..+7) × 100/7.  
- **system_confidence:** Aynı 7 TF’ye göre 50 / 60 / 75 / 90 bandında bir skor.  
- **trend_for_tf:** İlgili TF’de close ≥ EMA(20) ve close ≥ VWAP → 1; close ≤ ikisi → −1; diğer 0.

---

## 3. Q-RADAR (Erken Uyarı)

Q-Setup’tan önce, erken zaman fazında üretilen uyarı sinyali.

### 3.1 Koşullar (compute_q_radar)

1. **Yön:**  
   - Long: `trend_strength > 0` ve son mum bullish ve `close ≥ prev.close`.  
   - Short: `trend_strength < 0` ve son mum bearish ve `close ≤ prev.close`.  
   Aksi halde RADAR üretilmez.

2. **Faz:**  
   `q_radar_phase_min ≤ phase ≤ q_radar_phase_max` (örn. 0.1–0.3). Bu aralık dışında RADAR yok.

3. **Confidence:**  
   `confidence = (dir_score_norm×0.5 + conf_norm×0.3 + phase_score×0.2)` kırpılmış 0–1.  
   - `dir_score_norm`: trend yön skoru 0–100’den 0–1.  
   - `conf_norm`: (system_conf − 50) / 40 → 0–1.  
   - `phase_score`: Fazın erken bölge ortasına uzaklığından türetilen 0–1 skor.  
   `confidence < 0.4` ise RADAR üretilmez.

### 3.2 Çıktı (QRadarSignal)

| Alan | Açıklama |
|------|----------|
| symbol, timeframe, side | Sembol, TF, Buy/Sell. |
| confidence | 0–1. |
| expected_window_bars | (5, 13) sabit – beklenen tamamlanma penceresi (bar). |
| reference_price | Son mum kapanışı. |
| suggested_sl | Pivot ± ATR ile tahmini SL; Q-Setup gelince kesinleşir. |

---

## 4. Q-Setup (İşlem Fırsatı)

Tek sembol/TF için somut giriş/çıkış seviyeleri ve Q-skor.

### 4.1 Yön ve giriş koşulu

- **Yön:** Long: `trend_strength > 0` ve son mum bullish ve `close ≥ prev.close`. Short: trend < 0, bearish, `close ≤ prev.close`. Aksi halde setup üretilmez.

### 4.2 Giriş bölgesi ve SL (pivot + ATR)

Katsayılar: `q_entry_atr_alpha`, `q_entry_atr_beta`, `q_sl_atr_gamma` (varsayılan 0.2, 0.8, 1.5).

| Çıktı | Long | Short |
|--------|------|--------|
| Giriş bölgesi (entry_zone) | [L_pivot + α·ATR, L_pivot + β·ATR] | [H_pivot − β·ATR, H_pivot − α·ATR] |
| Entry | Son mum close, bölgeye clamp | Aynı mantık |
| SL | L_pivot − γ·ATR | H_pivot + γ·ATR |

`risk = |entry − SL|`.

### 4.3 Take Profit

1. **Minimum RR tabanı:**  
   - Long: `entry + max(q_min_rr × risk, 2×ATR)`  
   - Short: `entry − max(q_min_rr × risk, 2×ATR)`  

2. **Yapıya uygun TP (structure_based_tp):**  
   - Long: `entry + q_tp_structure_ext × (recent_high − L_pivot)`, cap: `entry + q_tp_max_r × risk`.  
   - Short: `entry − q_tp_structure_ext × (H_pivot − recent_low)`, cap: `entry − q_tp_max_r × risk`.  
   - Son TP = max(long’ta structure_tp, rr_tp) / min(short’ta structure_tp, rr_tp); structure yoksa sadece rr_tp.

### 4.4 Q-Skor (0–100)

Beş bileşen (0–1 normalize), ağırlıklı toplam × 100:

| Bileşen | Ağırlık (varsayılan) | Hesaplama |
|---------|----------------------|-----------|
| trend_score | q_weight_trend (0.35) | dir_score/100 kırpılmış. |
| structure_score | q_weight_structure (0.20) | HH/HL veya LH/LL (0.3 veya 1.0). |
| time_score | q_weight_time (0.25) | Giriş fazındaysa 1.0, geç fazdaysa 0.0, ara 0.5. |
| rr_score | q_weight_rr (0.10) | (RR−1)/2 kırpılmış; 1R→0, 3R→1. |
| momentum_score | q_weight_momentum (0.10) | Body/ATR ve volume ratio (son 20 bar ort.) ile 0–1. |

`q_score < q_score_threshold` (varsayılan 70) ise setup döndürülmez.

### 4.5 Zaman penceresi ve radar_early

- **time_window_bars:** (13, 21) sabit – setup’ın tamamlanması için bar penceresi.  
- **expected_bars:** 13.  
- **radar_early:** Verilen RADAR sinyali aynı sembol/TF ve aynı yöndeyse true.

### 4.6 Çıktı (QSetup)

symbol, timeframe, side, entry, entry_zone, stop_loss, take_profit, q_score, time_window_bars, expected_bars, radar_early.

---

## 5. Poz Koruma (ProtectSignal)

Kar içindeyken “koruma moduna geç” uyarısı.

### 5.1 Koşullar (compute_protect_signal)

- **Giriş:** entry, stop_loss, buffer, chart_tf, symbol.  
- **risk_r = |entry − SL|.**  
- **profit_r:** Long: (current_price − entry)/risk_r; Short: (entry − current_price)/risk_r.  
- **Tetik:** `profit_r ≥ q_protect_min_r` (varsayılan 1.5R). Aksi halde None.

### 5.2 Reason

- `phase ≥ q_late_phase` → `"LATE_PHASE"`.  
- Değilse → `"TRAILING_PROFIT"`.

### 5.3 Kilitlenecek kâr ve tetik fiyatı

- **locked_r:** `min(q_protect_lock_r, profit_r)` (varsayılan 0.5R’e kadar kilit).  
- **trigger_price:** Long: entry + locked_r × risk_r; Short: entry − locked_r × risk_r.  
  (SL’i bu seviyeye taşıma önerisi olarak düşünülebilir.)

### 5.4 Çıktı (ProtectSignal)

symbol, timeframe, reason, trigger_price, entry_price, locked_r.

---

## 6. Dip / Tepe Analizi (reversal.rs)

Q-RADAR fırsat analizinde “dip bölgesi” / “tepe bölgesi” ve güç skorları için kullanılır.

### 6.1 Dip (DipAnalysis)

- **dip_price:** Son pivot low.  
- **dip_time, dip_bar_index, bars_since_dip.**  
- **reversal_detected:** Dip barından sonra fiyat ≥ dip + 0.2×ATR ve son mum bullish ve close ≥ prev.close.  
- **reversal_strength (0–1):** Bounce/ATR, hacim oranı (son mum / 20 bar ortalama), gövde/ATR ile 0.5×strength_atr + 0.3×vol_ratio + 0.2×body_ratio.  
- **bounce_from_dip, bounce_r.**  
- **spring_detected:** Wyckoff Spring – dip barından sonra low < dip, ardından 4 bar içinde close > dip.

### 6.2 Tepe (PeakAnalysis)

- **peak_price:** Son pivot high.  
- **reversal_detected:** Fiyat ≤ peak − 0.2×ATR ve son mum bearish ve close ≤ prev.close.  
- **decline_strength (0–1):** Tepe–close, ATR, hacim ve gövde ile benzer formül.  
- **upthrust_detected:** Wyckoff Upthrust – tepe sonrası high > peak, ardından 4 bar içinde close < peak.

---

## 7. Confluence (8 Katman) – dip_confluence.rs

Dip/tepe tespitinin güvenilirliğini artırmak için çoklu doğrulama. LONG için dip, SHORT için tepe bağlamında hesaplanır.

| # | Katman | Açıklama |
|---|--------|----------|
| 1 | mtf_support_near | Üst TF’de pivot low/high ile referans fiyat ±0.5×ATR bandında. |
| 2 | ltf_structure_ok | Chart TF’de structure_score ≥ 0.55 (HL/LH uyumu). |
| 3 | fib_elliott_zone | Elliott/Fib seviyeleri (W2/W4/C, fibo_levels) ile referans fiyat %0.3 bandında. |
| 4 | divergence_ok | Long: fiyat LL + RSI HL (bullish). Short: fiyat HH + RSI LH (bearish). |
| 5 | spring_ok | Long: spring_detected. Short: upthrust_detected. |
| 6 | rsi_zone_ok | Long: RSI < q_rsi_oversold (35). Short: RSI > q_rsi_overbought (65). |
| 7 | bos_ok | Long: son kapanış > son iki pivot high’ın ilki. Short: son kapanış < son iki pivot low’un ilki. |
| 8 | absorption_ok | Destek/tepe bandında (ATR×0.3) son 5 bar hacmi, 20 bar ortalamasının 1.5 katı ve fiyat bandı kırmıyor. |

**Boost:** `layers_passed × 0.6` (maks 2.5) hem confidence_score hem early_warning_score’a eklenir; toplam 10’u geçmez.

**q_require_mtf_for_dip_zone:** true ise ve `mtf_support_near == false` ise tespit gösterilmez (detection "—", skorlar 0).

---

## 8. Q-RADAR Fırsat Analizi (QRadarOpportunityAnalysis)

Merkezi fonksiyon: `compute_q_radar_opportunity(buffer, chart_tf, symbol, config)`.

### 8.1 Akış

1. RADAR hesapla: `engine.compute_q_radar(...)`.  
2. Dip/tepe: `compute_reversal_analysis(candles, pivot_length)`.  
3. reference_price: RADAR varsa onun reference_price, yoksa son mum close.  
4. build_detection_and_recommendation(radar, dip, peak) → detection, confidence_score, early_warning_score, recommendation, direction, confirmation_layers.  
5. Tespit varsa (detection ≠ "—") confluence hesapla; skorlara boost ekle; confirmation_layers = "x/8 katman".  
6. q_require_mtf_for_dip_zone ve MTF yoksa tespiti sıfırla.  
7. Confluence sonrası confidence ≥ 7 ve early ≥ 7 → "GÜÇLÜ DİP/TEPE – İzle"; confidence ≥ 5 veya early ≥ 5 → "ZAYIF DİP/TEPE – İzle".

### 8.2 Tespit ve tavsiye (build_detection_and_recommendation)

- **RADAR varsa:**  
  - LONG: detection = "DİP BÖLGESİ (TEPKİ DİBİ)", early = dip.reversal_strength×10 veya conf×10.  
  - SHORT: detection = "TEPE BÖLGESİ (TEPKİ TEPESİ)", early = peak.decline_strength×10 veya conf×10.  
  - Tavsiye: conf_10≥7 ve early≥7 → GÜÇLÜ; conf_10≥4 veya early≥5 → ZAYIF; else DİP/TEPE BÖLGESİ – İzle.

- **RADAR yok, sadece dip:** reversal_detected ve reversal_strength ≥ 0.5 → "DİP BÖLGESİ (TEPKİ DİBİ)", conf = early×0.4, tavsiye strength ≥ 0.7 → GÜÇLÜ else ZAYIF.

- **RADAR yok, sadece tepe:** reversal_detected ve decline_strength ≥ 0.5 → "TEPE BÖLGESİ (TEPKİ TEPESİ)", aynı mantık.

- Hiçbiri yoksa: detection "—", skorlar 0, recommendation "—", direction "—".

### 8.3 Panel alanları (Web / bildirim)

| Alan | Kaynak |
|------|--------|
| Fiyat | reference_price |
| YÖN | LONG / SHORT / "—" |
| Tespit | detection |
| Güven | confidence_score (0–10) |
| Erken Uyarı | early_warning_score (0–10) |
| Tavsiye | recommendation |
| Onay katmanı | confirmation_layers (x/8 katman) |

Detay: `docs/Q_ANALIZ_ALANLARI.md`.

---

## 9. Config Parametreleri (Q-ANALİZ ile ilgili)

| Parametre | Varsayılan | Açıklama |
|-----------|------------|----------|
| q_score_threshold | 70 | Q-Setup üretmek için min Q-skor (0–100). |
| q_elite_threshold | 85 | “Elit” setup eşiği (istatistiksel güç). |
| q_min_rr | 1.5 | Min risk/ödül oranı. |
| q_radar_phase_min / q_radar_phase_max | 0.1, 0.3 | RADAR’ın tetikleneceği faz aralığı. |
| q_entry_phase_min / q_entry_phase_max | 0.2, 0.6 | Giriş için ideal faz. |
| q_late_phase | 0.7 | Geç faz eşiği (Poz Koruma / time_score=0). |
| q_protect_min_r | 1.5 | Poz Koruma için min kâr (R). |
| q_protect_lock_r | 0.5 | Kilitlenecek min kâr (R). |
| q_entry_atr_alpha / q_entry_atr_beta | 0.2, 0.8 | Giriş bölgesi ATR katsayıları. |
| q_sl_atr_gamma | 1.5 | SL mesafesi ATR katsayısı. |
| q_tp_structure_ext | 1.618 | Yapı TP projeksiyon çarpanı. |
| q_tp_max_r | 5.0 | TP üst sınırı (R cinsinden). |
| q_require_mtf_for_dip_zone | false | true ise MTF destek yoksa tespit gösterilmez. |
| q_rsi_oversold / q_rsi_overbought | 35, 65 | Confluence RSI bandı. |
| q_weight_trend / structure / time / rr / momentum | 0.35, 0.20, 0.25, 0.10, 0.10 | Q-skor ağırlıkları (toplam 1). |

---

## 10. Veri Yapıları Özeti

| Tip | Dosya | Ana alanlar |
|-----|--------|-------------|
| QSetup | types.rs | symbol, timeframe, side, entry, entry_zone, stop_loss, take_profit, q_score, time_window_bars, expected_bars, radar_early |
| QRadarSignal | types.rs | symbol, timeframe, side, confidence, expected_window_bars, reference_price, suggested_sl |
| ProtectSignal | types.rs | symbol, timeframe, reason, trigger_price, entry_price, locked_r |
| QRadarOpportunityAnalysis | q_radar_analysis.rs | symbol, timeframe, radar, dip, peak, detection, confidence_score, early_warning_score, recommendation, confirmation_layers, direction, reference_price |
| DipAnalysis | reversal.rs | dip_price, dip_time, reversal_detected, reversal_strength, bounce_from_dip, bounce_r, spring_detected |
| PeakAnalysis | reversal.rs | peak_price, peak_time, reversal_detected, decline_strength, decline_from_peak, decline_r, upthrust_detected |
| DipConfluenceResult | dip_confluence.rs | mtf_support_near, ltf_structure_ok, fib_elliott_zone, divergence_ok, spring_ok, rsi_zone_ok, bos_ok, absorption_ok, layers_passed |

---

## 11. Çağrı Sırası ve Entegrasyon

### 11.1 Tek sembol/TF (ör. grafik isteği)

1. CandleBuffer doldurulur (tüm gerekli TF’ler).  
2. `compute_q_radar_opportunity(buffer, chart_tf, symbol, config)` → QRadarOpportunityAnalysis.  
3. `engine.compute_q_setup(buffer, chart_tf, symbol, opportunity.radar.as_ref())` → Option<QSetup>.  
4. Poz Koruma için (entry, sl verilmişse): `engine.compute_protect_signal(buffer, chart_tf, symbol, entry, sl)` → Option<ProtectSignal>.

### 11.2 Robot (AutoTrader)

- Q-Setup → `signal_from_q_setup(setup)` → TradeSignal.  
- use_radar_filter: true ise sadece RADAR yönü ile uyumlu sinyaller kabul edilir; min_radar_confidence eşiği uygulanır.

### 11.3 Web

- `/api/chart`: Q-RADAR fırsat, Q-Setup, Poz Koruma hesaplanır; panel ve bildirimler aynı alanlarla doldurulur.  
- `/api/q-analysis`: config’teki symbols × timeframes için tüm Q-RADAR fırsat listesi.  
- `/api/q-analiz/detections`: DB’deki Q-Analiz tespit kayıtları (daemon tarafından yazılan).

### 11.4 Q-Analiz daemon

- Belirli aralıklarla trading.symbols × timeframes taranır.  
- `compute_q_radar_opportunity` sonucu tespit varsa DB’ye (`insert_q_analiz_detection`) yazılır ve Telegram (veya routing’e göre diğer kanallar) ile bildirilir.

---

## 12. İlgili Dokümanlar

- **Q_ANALIZ_ALANLARI.md** – Panel alanlarının (Fiyat, YÖN, Tespit, Güven, Erken Uyarı, Tavsiye) hesaplanması.  
- **DIP_TESPITI_KATMANLAR.md** – Katmanlı dip tespiti ile mevcut kod eşlemesi ve geliştirme önerileri.  
- **USAGE.md** – Q-ANALİZ vs Q-Setup, Entry/SL/TP formülleri (özet).

Bu doküman, Q-ANALİZ’in kodla uyumlu teknik referansıdır; güncel davranış için ilgili kaynak dosyaları esas alınmalıdır.
