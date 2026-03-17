# Q-Analiz: Şu Anki Yapı İncelemesi

Bu doküman, dip/tepe ve skorlama sisteminin **mevcut kod akışını** ve **ana karar noktalarını** özetler.

---

## 1. Giriş noktası

Tüm Q-Analiz tek fonksiyondan tetiklenir:

```
compute_q_radar_opportunity(buffer, chart_tf, symbol, config)
  → QRadarOpportunityAnalysis
```

**Çağıranlar:** `iqai-cli` (daemon, tek sembol tarama), `iqai-web` (API, kart, bildirim).  
**Girdi:** Çoklu TF mumları (`CandleBuffer`), seçili TF, sembol, `Config`.

---

## 2. Ana akış (sırayla)

```
1. RADAR sinyali
   engine.compute_q_radar(buffer, chart_tf, symbol)
   → Trend + son mum yönü + fibo_time_phase bandı + system_confidence
   → Option<QRadarSignal> (yön: Buy/Sell, confidence 0–1)

2. Dip / tepe analizi (tek TF)
   compute_reversal_analysis(candles, Some(config.pivot_length))
   → ReversalAnalysis { dip: Option<DipAnalysis>, peak: Option<PeakAnalysis> }
   - Pivot low/high (indicators) → dip_price, peak_price
   - Dönüş tespiti (fiyat margin dışında + mum yönü)
   - reversal_strength / decline_strength (bounce/ATR + hacim + gövde)
   - Spring / Upthrust (reversal.rs)

3. Tespit + ilk güven + tavsiye taslağı
   build_detection_and_recommendation(&radar, &dip, &peak)
   - RADAR varsa: conf_10 = radar.confidence*10, early_10 = reversal_strength*10 (veya conf_10)
   - RADAR yoksa: sadece dip/peak path → conf = early*0.4, early = reversal_strength*10
   - Tavsiye: conf≥7 ve early≥7 → GÜÇLÜ; conf≥4 veya early≥5 → ZAYIF; else BÖLGESİ
   → detection, confidence_score, early_warning_score, recommendation, direction

4. Sadece tespit varken (detection != "—"):
   a) Confluence (8 katman)
      compute_dip_confluence(buffer, chart_tf, config, reference_price, is_long, dip, peak)
      → mtf_support_near, ltf_structure_ok, fib_elliott_zone, divergence_ok, spring_ok, rsi_zone_ok, bos_ok, absorption_ok
      → layers_passed (0–8)
   b) Boost
      confidence_score += min(layers_passed * 0.6, 2.5)
      early_warning_score += min(layers_passed * 0.6, 2.5)
   c) Opsiyonel: q_require_mtf_for_dip_zone && !mtf_support_near → tespiti iptal (detection="—", skorlar 0)
   d) Son tavsiye güncellemesi (confluence sonrası skorlara göre)
      conf≥7 ve early≥7 → GÜÇLÜ DİP/TEPE – İzle
      conf≥5 veya early≥5 (ama yukarıdaki değil) → ZAYIF DİP/TEPE – İzle
   e) Sinyal bazlı skorlama (Madde 15)
      compute_dip_tepe_score(candles, config, is_long, dip, peak, structure_score, mtf_support_near)
      → DipTepeScore { signals[], total 0–10, recommendation (STRONG/BUY ZONE/WATCH/NO SIGNAL), early_warning_momentum }
      → discrete_score = Some(...)

5. Çıktı
   QRadarOpportunityAnalysis { symbol, timeframe, radar, dip, peak, detection, confidence_score, early_warning_score, recommendation, confirmation_layers, direction, reference_price, discrete_score }
```

---

## 3. Modül rolleri

| Modül | Sorumluluk |
|-------|------------|
| **indicators.rs** | pivot_low, pivot_high, atr, rsi, ema, sma, macd, bollinger, vwap |
| **reversal.rs** | Dip/tepe fiyatı (pivot), dönüş var mı, reversal_strength, spring/upthrust |
| **signal.rs** | SignalEngine: compute_q_radar, trend_for_tf, structure_score, fibo_time_phase |
| **dip_confluence.rs** | 8 katman (MTF destek, yapı, Fib/Elliott, divergence, spring, RSI, BOS, absorption) |
| **dip_tepe_scoring.rs** | Sinyal→puan (RSI, MACD div, destek, hacim, mum/pattern, Fib, EMA200, yapı, Bollinger, mean reversion), toplam 0–10, tavsiye, early_warning_momentum |
| **candlestick_patterns.rs** | Hammer, Engulfing, Morning Star, Piercing; Shooting Star, Bearish Engulfing, Evening Star, Dark Cloud |
| **q_radar_analysis.rs** | Hepsinin orkestrasyonu: RADAR + reversal → detection → confluence → boost → discrete_score → QRadarOpportunityAnalysis |

---

## 4. İki skor sistemi (yan yana)

| | **Güven / Erken uyarı (0–10)** | **Discrete skor (0–10)** |
|---|--------------------------------|---------------------------|
| Kaynak | RADAR confidence + reversal_strength; confluence boost | Sinyal bazlı puan (RSI +1, destek +2, …) |
| Tavsiye | GÜÇLÜ / ZAYIF / BÖLGESİ – İzle | STRONG BUY / BUY ZONE / WATCH / NO SIGNAL |
| Kullanım | Ana ekran (Güven çubuğu, Erken uyarı, Tavsiye) | Ek bilgi (Skor X/10, hangi sinyaller aktif) |
| Yer | confidence_score, early_warning_score, recommendation | discrete_score.total, .signals, .recommendation |

İkisi de aynı anda dolu; biri kurallı formül + confluence, diğeri açık puan tablosu.

---

## 5. Önemli eşikler (şu anki hali)

- **Pivot:** `config.pivot_length` (varsayılan 5).
- **RADAR:** confidence < 0.4 → RADAR yok; `q_radar_phase_min`–`q_radar_phase_max` dışında RADAR yok.
- **Tespit (RADAR yok):** dip.reversal_detected && reversal_strength ≥ 0.5 (veya peak tarafı).
- **GÜÇLÜ:** conf ≥ 7 ve early ≥ 7 (confluence sonrası).
- **ZAYIF:** (conf ≥ 5 veya early ≥ 5) ve GÜÇLÜ değil.
- **Confluence:** Katman başı +0.6, toplam cap +2.5.
- **MTF zorunlu:** `q_require_mtf_for_dip_zone` true ise, mtf_support_near yoksa tespit iptal.
- **RSI:** Long RSI < q_rsi_oversold (35 veya 30), short RSI > q_rsi_overbought (65 veya 70).
- **Discrete tavsiye:** total ≥ 8 STRONG, ≥ 6 BUY ZONE, ≥ 4 WATCH, < 4 NO SIGNAL.

---

## 6. Veri akışı özeti

```
CandleBuffer (OHLCV, çoklu TF)
    ↓
┌─────────────────────────────────────────────────────────┐
│ compute_q_radar_opportunity                              │
│   → RADAR (trend, faz, confidence)                      │
│   → reversal (dip/peak, strength, spring/upthrust)      │
│   → build_detection_and_recommendation                   │
│   → [tespit varken] confluence → boost                  │
│   → [tespit varken] compute_dip_tepe_score → discrete_score │
└─────────────────────────────────────────────────────────┘
    ↓
QRadarOpportunityAnalysis
  (detection, confidence_score, early_warning_score, recommendation, discrete_score, …)
    ↓
CLI / Web / Telegram kartı / DB kaydı
```

---

## 7. Kısa değerlendirme

- **Güçlü taraflar:** Tek giriş noktası, net modül ayrımı, hem formül tabanlı (güven/erken uyarı) hem sinyal tabanlı (discrete) skor, confluence ile filtre, dokümantasyonla uyumlu.
- **Dikkat edilebilecekler:** `dip`/`peak` discrete_score’a sadece yön (is_long) ve confluence (mtf_support_near) için kullanılıyor; ileride dip_price/peak_price skorlama içinde de kullanılabilir. RADAR yokken sadece reversal path’i çalışıyor; RADAR’ın faz bandı dar ise çoğu zaman “RADAR yok, dip/tepe var” path’i ağırlıklı olur.

Bu inceleme, `docs/Q_ANALIZ_DIP_TEPE_YONTEM.md` ile birlikte mevcut davranışı tarif eder.
