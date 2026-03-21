# Web API: Q-Analiz toplu uç noktası

## `GET /api/q-analysis`

Tüm izlenen sembol + timeframe çiftleri için Q-RADAR / Q-Analiz sonuçlarını döndürür (Binance Futures mum verisi).

### Sembol listesi (`trading.symbols`)

| Durum | Davranış |
|--------|-----------|
| `trading.symbols` **tanımlı ve boş değil** | Config’teki liste kullanılır. |
| `trading` yok, `symbols` yok veya **`symbols: []`** (boş dizi) | Varsayılan semboller kullanılır: **`ETHUSDT`**, **`BTCUSDT`**. |

Boş liste “tüm piyasayı tarama” anlamına gelmez; güvenli varsayılan olarak iki likit sembol seçilir.

### Timeframe listesi (`trading.timeframes`)

| Durum | Davranış |
|--------|-----------|
| Dolu liste | Parse edilebilen timeframe’ler kullanılır. |
| Yok / boş | Varsayılan: **`5m`**, **`15m`**, **`1h`**, **`4h`**. |

Hiç geçerli timeframe yoksa yanıt: `{ "error": "Geçerli timeframe yok", "results": [] }`.

### Yanıt alanları (G05 — Q-Setup + Elliott zenginleştirme)

`config.json` → `smart_money.q_enrich_opportunity_with_setup_elliott` (**varsayılan `true`**) açıkken her `results[]` öğesi, çekirdek Q-RADAR alanlarına ek olarak şunları içerebilir:

| Alan | Anlam |
|------|--------|
| `q_setup` | Aynı TF için Q-Setup (giriş, SL, TP, `q_score`, …) veya yok |
| `radar_setup_alignment` | RADAR yönü ile Q-Setup yönü: `1.0` uyum, `0.0` çelişki, `0.5` setup üretilmedi |
| `elliott_secondary_tp` | Elliott W5 veya düzeltme hedefi (ikinci TP fiyatı) |
| `elliott_summary` | Kısa metin: `formation` / `formation_type` |
| `abc_correction_hint` | Zigzag/Flat ABC ipucu (ör. LONG bias POC) |

Çelişki (`radar_setup_alignment === 0`) durumunda güven skoru düşürülür ve tavsiye metnine `ÇELİŞKİ (Q-Setup) –` öneki eklenir. Zenginleştirmeyi kapatmak için `q_enrich_opportunity_with_setup_elliott: false`.

`analysis_snapshots.extra_json` içine de `radar_setup_alignment`, `q_setup`, `elliott_secondary_tp`, `elliott_summary`, `abc_correction_hint` özetleri yazılır.

### İlgili kod

- `crates/iqai-web/src/http_app.rs` — `api_q_analysis_all`
- `crates/iqai-core/src/q_radar_analysis.rs` — `compute_q_radar_opportunity`, `radar_setup_alignment_score`
