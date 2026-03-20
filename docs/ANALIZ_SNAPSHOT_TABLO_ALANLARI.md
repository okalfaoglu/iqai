# Analiz Snapshot Tablosu – Kodda Toplanan Bilgi ile Eşleşme

Bu doküman, **her sembol × her timeframe için tek satır** tutacağımız `analysis_snapshots` tablosunun, kodda gerçekten üretilen verilerle karşılaştırmasını ve **eksik kalan alanları** netleştirir.

---

## 1) Kodda Üretilen Veriler (kaynak modüller)

### 1.1 QRadarOpportunityAnalysis (`q_radar_analysis.rs`)

| Alan | Tip | Tabloda? (önceki öneri) |
|------|-----|-------------------------|
| symbol | String | ✓ symbol |
| timeframe | Timeframe | ✓ timeframe |
| radar | Option<QRadarSignal> | kısmen (radar güven → confidence zaten var) |
| dip | Option<DipAnalysis> | **yok** – dip_price, reversal_strength, spring_detected vb. |
| peak | Option<PeakAnalysis> | **yok** – peak_price, decline_strength, upthrust_detected vb. |
| detection | String | ✓ detection |
| confidence_score | f64 | ✓ confidence_score |
| early_warning_score | f64 | ✓ early_warning_score |
| recommendation | String | ✓ recommendation |
| confirmation_layers | Option<String> | ✓ (confluence_layers sayı olarak) |
| direction | String | ✓ direction |
| reference_price | f64 | ✓ reference_price |
| discrete_score | Option<DipTepeScore> | ✓ discrete_score (total) – **ama sinyal detayı yok** |
| smart_money_score | Option<SmartMoneyRadarScore> | ✓ sm_score (total) – **ama sinyal detayı yok** |

### 1.2 QRadarSignal (opp.radar içinde)

| Alan | Tip | Tabloda? |
|------|-----|----------|
| side | SignalType | direction ile örtüşüyor |
| confidence | f64 | confidence_score ile örtüşüyor |
| expected_window_bars | (u32, u32) | **yok** |
| reference_price | f64 | ✓ reference_price |
| suggested_sl | Option<f64> | **yok** |

### 1.3 DipAnalysis (opp.dip içinde)

| Alan | Tip | Tabloda? |
|------|-----|----------|
| dip_price | f64 | **yok** |
| dip_time | i64 | **yok** |
| bars_since_dip | usize | **yok** |
| reversal_detected | bool | **yok** |
| reversal_strength | f64 | **yok** |
| bounce_from_dip | f64 | **yok** |
| bounce_r | f64 | **yok** |
| spring_detected | bool | **yok** |

### 1.4 PeakAnalysis (opp.peak içinde)

| Alan | Tip | Tabloda? |
|------|-----|----------|
| peak_price | f64 | **yok** |
| peak_time | i64 | **yok** |
| bars_since_peak | usize | **yok** |
| reversal_detected | bool | **yok** |
| decline_strength | f64 | **yok** |
| decline_from_peak | f64 | **yok** |
| decline_r | f64 | **yok** |
| upthrust_detected | bool | **yok** |

### 1.5 DipConfluenceResult (`dip_confluence.rs` – Q-Analiz içinde hesaplanıyor, opp’ta dışa açılmıyor)

| Alan | Tip | Tabloda? |
|------|-----|----------|
| mtf_support_near | bool | **yok** |
| ltf_structure_ok | bool | **yok** |
| fib_elliott_zone | bool | **yok** |
| divergence_ok | bool | **yok** |
| spring_ok | bool | **yok** |
| rsi_zone_ok | bool | **yok** |
| bos_ok | bool | **yok** |
| absorption_ok | bool | **yok** |
| layers_passed | u8 | ✓ confluence_layers |

### 1.6 DipTepeScore (opp.discrete_score – sadece total/recommendation değil)

| Alan | Tip | Tabloda? |
|------|-----|----------|
| signals | Vec<SignalScore> | **yok** (name, points, active – hangi sinyal kaç puan) |
| total | u8 | ✓ discrete_score |
| recommendation | String | **yok** (opsiyonel) |
| early_warning_momentum | bool | **yok** |

### 1.7 SmartMoneyRadarScore (opp.smart_money_score)

| Alan | Tip | Tabloda? |
|------|-----|----------|
| signals | Vec<SmartMoneyRadarSignal> | **yok** |
| total | u8 | ✓ sm_score |
| recommendation | String | **yok** |

### 1.8 ElliottDetectorResult (`elliott_detector.rs` – daemon’da compute_elliott + scenarios)

| Alan | Tip | Tabloda? |
|------|-----|----------|
| formation | String | ✓ elliott_formation |
| formation_type | String | ✓ elliott_type |
| in_progress | Option<bool> | ✓ elliott_in_progress |
| validation_ok | Option<bool> | **yok** |
| validation_msg | Option<String> | **yok** |
| impulse_state (setup_w3, setup_w5) | Option<…> | scenario_entry/stop/tp ile kısmen |
| w5_targets | Option<(f64,f64,f64)> | **yok** (hedefler) |
| projections | Option<Vec<…>> | **yok** |
| truncation, throw_over, w5_divergence | Option<bool> | **yok** |
| corr_setup (Zigzag C, Triangle E) | Option<CorrSetup> | **yok** (entry/sl/tp ayrı) |
| next_formation_ref | … | **yok** |

### 1.9 StrategyPlan / StrategyScenario (build_scenarios_for_series – “en iyi” senaryo)

| Alan | Tip | Tabloda? |
|------|-----|----------|
| direction, entry, stop_loss, targets | … | ✓ scenario_* |
| q_score | f64 | ✓ scenario_qscore |
| classic_pattern_label | Option<String> | ✓ classic_pattern |
| elliott_formation | Option<String> | ✓ elliott_formation ile çakışıyor |
| scenario_kind | StrategyScenarioKind | **yok** (GenericQSetup, TriangleEBreak, ImpulseWave, CupAndHandle) |
| has_radar_context | bool | **yok** |
| role (Primary/Alternative/Macro) | StrategyRole | ✓ scenario_role |
| probability | f64 | **yok** |
| targets (birden fazla TP) | Vec<StrategyTarget> | sadece tp1 – **tp2, tp3 yok** |

### 1.10 SmartMoneyContext (build_smart_money_context – daemon’da mevcut)

| Alan | Tip | Tabloda? |
|------|-----|----------|
| po3_phase | Po3Phase | **yok** (Accumulation/Manipulation/Expansion) |
| liquidity_levels | Vec<LiquidityLevel> | **yok** (fiyat + label + strength) |
| order_blocks | Vec<OrderBlockZone> | **yok** |
| fair_value_gaps | Vec<FairValueGap> | **yok** |
| wyckoff_tags | Vec<WyckoffTag> | **yok** |
| wyckoff_state | Option<WyckoffState> | **yok** |

### 1.11 İndikatörler (son bar – şu an tabloda önerilen)

| Hesaplanan | Tabloda? |
|------------|----------|
| rsi_14 | ✓ rsi_14 |
| atr_14 | ✓ atr_14 |
| macd (line, signal, hist) | ✓ macd_* |
| Bollinger (lower, middle, upper) | **yok** |
| EMA (örn. 20, 50, 200) | **yok** |
| VWAP | **yok** |

---

## 2) Sonuç: Tablo Gerçekten “Az” Kalıyor

Evet. Önceki şemada:

- **Dip/tepe ham verisi** (dip_price, peak_price, reversal_strength, spring_detected, bars_since_dip vb.) yok.
- **Confluence bayrakları** (mtf_support_near, divergence_ok, rsi_zone_ok, bos_ok vb.) yok; sadece `layers_passed` vardı.
- **Q-RADAR detayı** (expected_window_bars, suggested_sl) yok.
- **Discrete / SM skor detayı** (hangi sinyal kaç puan) yok.
- **Elliott ek alanları** (validation_ok/msg, w5_targets, projections, corr_setup) yok.
- **Strateji detayı** (scenario_kind, probability, tp2/tp3, has_radar_context) yok.
- **Smart Money ham verisi** (po3_phase, likidite/OB/FVG/Wyckoff listeleri) yok.
- **Ek indikatörler** (Bollinger, EMA, VWAP) yok.

Bu verilerin hepsi ya da büyük kısmı AI raporu ve filtreleme için işe yarar; tabloda sadece “özet” tutarsak tablo kodda toplanan bilgiye göre **az** kalır.

---

## 3) Genişletilmiş Tablo Önerisi (kodla uyumlu)

İki katman öneriyoruz:

### A) Sütun olarak tutulacaklar (sorgu / filtre / AI için kolay kullanım)

Aşağıdaki alanlar **ayrı kolon** olsun; böylece SQL ve raporlama “kodda toplanan bilgi”yle uyumlu olur.

```text
-- Kimlik & zaman
symbol, timeframe, updated_at

-- Q-Analiz özet (mevcut)
detection, direction, recommendation, confidence_score, early_warning_score,
reference_price, confirmation_layers (string), discrete_score, sm_score, confluence_layers

-- Q-RADAR detay
radar_confidence, radar_window_min, radar_window_max, radar_suggested_sl

-- Dip analizi (opp.dip)
dip_price, dip_time, bars_since_dip, reversal_detected, reversal_strength,
bounce_from_dip, bounce_r, spring_detected

-- Tepe analizi (opp.peak)
peak_price, peak_time, bars_since_peak, peak_reversal_detected, decline_strength,
decline_from_peak, decline_r, upthrust_detected

-- Confluence bayrakları (DipConfluenceResult)
mtf_support_near, ltf_structure_ok, fib_elliott_zone, divergence_ok,
confluence_spring_ok, rsi_zone_ok, bos_ok, absorption_ok

-- Osilatör / indikatör (son bar)
rsi_14, atr_14, macd_line, macd_signal, macd_hist,
bb_lower, bb_middle, bb_upper, ema_20, ema_50, ema_200, vwap

-- Elliott özet
elliott_formation, elliott_type, elliott_in_progress, elliott_validation_ok, elliott_validation_msg,
elliott_w5_t1, elliott_w5_t2, elliott_w5_t3, elliott_truncation, elliott_throw_over

-- Strateji (en iyi senaryo)
classic_pattern, scenario_role, scenario_direction, scenario_kind, scenario_probability,
scenario_entry, scenario_stop, scenario_tp1, scenario_tp2, scenario_tp3, scenario_qscore,
scenario_has_radar, scenario_invalidation

-- Smart Money özet (tek değerler / enum)
po3_phase, sm_discrete_recommendation, sm_early_warning_momentum
```

### B) JSON ile saklanacaklar (şema şişmesin, ama hiçbir bilgi kaybolmasın)

Aşağıdakiler **tek TEXT kolonda JSON** (ör. `extra_json` veya ayrı `signals_json` / `context_json`) olarak saklanabilir:

- **Discrete skor detayı**: `DipTepeScore.signals` (name, points, active)
- **Smart Money skor detayı**: `SmartMoneyRadarScore.signals`
- **Elliott ek**: projections, next_formation_ref, corr_setup (entry/sl/tp), impulse_state tam
- **Smart Money context**: liquidity_levels, order_blocks, fair_value_gaps, wyckoff_tags, wyckoff_state (kısaltılmış liste)
- **Strateji targets**: targets dizisinin tamamı (label + price + priority)

Böylece:

- Tablo **kodda toplanan bilgiye göre az kalmaz**: hem sayısal/sorgulanabilir alanlar hem de detaylar (JSON) tutulur.
- AI ve raporlama: önce kolonlardan okuyup, gerektiğinde JSON’dan detay çeker.

---

## 4) Özet

| Soru | Cevap |
|------|--------|
| Önceki tablo kodda toplanan bilgiye göre az mı? | **Evet.** Dip/tepe, confluence bayrakları, radar detayı, Elliott ek alanları, strateji detayı, SMC listeleri ve bazı indikatörler eksikti. |
| Ne yapmalı? | Yukarıdaki **genişletilmiş sütun listesi** ile tabloyu büyüt; liste/detay alanları için **extra_json** (ve gerekirse signals_json) kullan. |
| Sonuç | Tablo, kodda üretilen veriyle uyumlu hale gelir; hem filtreleme hem AI raporu için yeterli bilgi tek yerde toplanmış olur. |

Bu genişletilmiş şemayı `trade_db.rs` ve daemon tarafına uygulayacak adımları istersen bir sonraki adımda kod seviyesinde yazabilirim.
