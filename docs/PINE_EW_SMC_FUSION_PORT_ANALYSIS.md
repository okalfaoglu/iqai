# Pine Script: “Elliott Wave + SMC Fusion” — IQAI’ye taşınabilir özellik analizi

**Kaynak:** Kullanıcı tarafından paylaşılan TradingView Pine v5 göstergesi (özet: ZigZag pivotları, W1–W2 kilidi, W3–5 ilerleme, Fib toleransı, EWO, basit OB/FVG, confluence notu, trade seviyeleri).

Bu doküman **mantıksal fikirleri** IQAI mimarisiyle eşleştirir; Pine’daki hatalı/naif kısımları ayrı işaret eder.

---

## 1. Pine’da ne iyi (ürün fikri olarak)

| Özellik | Pine davranışı | Neden değerli |
|--------|----------------|----------------|
| **Durum makinesi** | W2 kilitlendikten sonra pivotlarla W3→W4→W5 güncellenir; `currentWave`, `isLocked` | Kullanıcıya “neredeyiz?” hissi; sinyal flip-flop azaltma potansiyeli |
| **Geçersiz kılış kuralları** | W0 kırılımı, W2 kırılımı, süre (`autoInvalidateAfter`), tamamlanınca reset | “Hayalet sayım”ı düşürmek için net yaşam döngüsü |
| **Sinyal stabilitesi** | Cooldown, yön değişimi şartı, min bar mesafesi, W2 sonrası N bar onayı | Aynı formasyon için tekrarlayan uyarıları keser |
| **Görsel dil** | W1–3–5 düz, W2–4 kesik çizgi; projeksiyonlar noktalı | IQAI’de kısmen `wave_legs.dotted` ile var; tutarlı legend iyi |
| **Ayarlanabilir projeksiyon** | `wave3_fib`, `wave4_retrace`, `wave5_fib` kullanıcı girdisi | Eğitim / farklı senaryo (agresif/konservatif) için |
| **Fib toleransı** | `strictFibCheck` + `%` bantları veya geniş min–max W2/W1 | “Guideline” yakınlığını skorlamak için taşınabilir fikir |
| **EWO (Elliott Wave Oscillator)** | EMA hızlı/yavaş oranı, sinyal çizgisi, güç eşiği, isteğe bağlı zorunlu onay | IQAI’de **`elliott_fusion`** + `elliott_ewo_*` config; panelde gösterim var |
| **Confluence + not** | W2 oranı, OB/FVG, hacim, EMA50, EWO → skor + harf notu | **`confluence_score` / `wave_grade`** EW fusion’da; Q-RADAR ile tamamen birleşik tek skor değil (isteğe bağlı) |
| **SMC birleşimi** | W2 anında OB/FVG aranıp kutulanması | **`smc_w2_zone_overlap`** + skor katkısı var; **grafikte kutu** (Pine’daki gibi) henüz yok → §7-B |
| **Dashboard / R:R** | Tablo, W3 için basit R:R | Web panelde özet metrik olarak uygun |

---

## 2. Pine’daki zayıf / hatalı taraflar (taşırken düzelt)

1. **Elliott kuralları eksik:** W4–W1 örtüşmesi (impulse), W3 en kısa olmama, diyagonal ayrımı vb. Pine çoğunlukla **W1–W2 geometrisi + pivot takibi** ile gidiyor; IQAI zaten `validate_impulse` vb. ile daha kurallı — Pine mantığını **ham doğrulama** olarak kopyalamak geri adım olur.
2. **W2 “geçerli” tanımı:** `_p2 > _p0 and _p2 < _p1` (bull) klasik W2 sınırına yakın ama tüm edge-case’ler yok; Fib sadece W2/W1 oranı.
3. **Geçersiz kılış bug riski:** `shouldInvalidate` sonrası `isActive`/`isLocked` sıfırlanmıyor gibi görünen akışlar (yorum satırı: “Keep graphics…”) — state tutarlılığı taşınırken sıfırdan tasarlanmalı.
4. **OB/FVG:** Çok basitleştirilmiş kurallar; gerçek ICT/SMC ile uyum garanti değil. IQAI’de var olan OB/FVG mantığı **öncelikli** olmalı, Pine sadece “fikir” kaynağı.
5. **Giriş/SL/TP:** `entry = w2 + 0.3*w2Size` gibi sabitler eğitim amaçlı; prod stratejide `strategy` / risk modülü ile değiştirilmeli.

---

## 3. IQAI’de şu an ne var (özet eşleştirme)

| Pine | IQAI (yaklaşık) |
|------|------------------|
| ZigZag `zzLen` | `pivot_length`, `collect_swings` |
| Impulse + validasyon | `elliott.rs` + `validate_impulse_with_w5`, W4–W3, vb. |
| W3–5 projeksiyon | `compute_projections`, `w5_targets`, kanal, çoklu hedef |
| Impulse aşamaları | `impulse_detector` (CHoCH, W2, BOS…) |
| Dalga derecesi (yaklaşık) | `WaveDegree` + TF + `subwave_degree` |
| SMC OB/FVG | `smart_money` (daha kapsamlı olabilir) |
| EWO | **Var** — `elliott_fusion.rs` (`ewo_value`, EMA sinyal, güç bayrakları); `config`: `elliott_ewo_*` |
| Harf notu / tek confluence skoru | **Var (EW özel)** — `ElliottFusionExtras`: `confluence_score`, `wave_grade`; panel + API |
| Lock + cooldown + invalidate yaşam döngüsü | **Kısmi** — stateless tespit devam; `pattern_stability` + `invalidate_hint` + config bar eşikleri var; **kalıcı `locked/current_leg` state makinesi yok** |

---

## 4. Taşıma önerisi (öncelik sırası) — **durum (kod tabanı kontrolü)**

### P0 — Düşük risk, yüksek fikir değeri
- [x] **EW kalite / confluence skoru:** `crates/iqai-core/src/elliott_fusion.rs` — W2/W1 oranı, Fib bant bonusu, hacim, EMA50, EWO; `confluence_score` + `wave_grade`.
- [x] **Konfigürasyon:** `crates/iqai-core/src/config.rs` — `elliott_wave3_extension`, `elliott_wave4_retrace_path`, `elliott_fib_tolerance_pct`, projeksiyon barları, EWO ve stabilite alanları; `config.json.example` ile uyumlu tutulmalı.

### P1 — Orta efor
- [x] **EWO modülü:** `elliott_fusion.rs` (`compute_ewo_tail`); fusion ekleri `ElliottDetectorResult` / `chart_data` üzerinden web’e; `elliott_require_ewo_alignment` opsiyonel soft fail.
- [ ] **Aktif sayım durumu (tam state makinesi):** `ElliottPatternState { locked, current_leg, … }` — **yapılmadı**. Şu an: bar başına yeniden hesap + `ElliottPatternStability` (min mesafe, onay barı, yaş, timeout uyarısı) + `invalidate_hint`. Kalıcı lock/cooldown için ayrı spec + depolama (ör. snapshot/DB) gerekir.

### P2 — SMC füzyonu
- [x] W2 bölgesinde **OB/FVG çakışması** — `build_smart_money_context_for_series` + `smc_w2_zone_overlap` / `smc_w2_detail`; confluence skoruna katkı.
- [ ] Chart’ta **görsel** “EW + SMC” (OB kutusu / ENTRY-STOP çizgileri) — panelde metin var; **grafik overlay** (Pine’daki kutu çizimi) ayrı iş (bkz. `GUI_ROADMAP` / Elliott backlog).

### P3 — UX
- [x] Panel: **not (A+…D)**, **confluence %**, **EWO**, **stabilite**, **invalidate metni**, **SMC–W2** — `index.html` `ewFusionBlock` + `updateElliottPanel`.
- [ ] **Alert/notify:** Yüksek confluence + geçerli impulse için otomatik bildirim kuralı — `notify.rs` Q/ Elliott özetleriyle kısmen yakın; **eşik tetikli** ayrı kural yok (isteğe bağlı geliştirme).

---

## 5. Sonuç

- Pine script **tam Elliott motoru değil**; güçlü tarafı **durum + stabilite + skor + EWO + basit SMC görünürlüğü**.
- IQAI’de **EWO**, **EW confluence/grade**, **projeksiyon oranları (config)** ve **SMC–W2 skor kesişimi** uygulanmış durumda (§4). Pine’a en yakın **eksikler**: kalıcı **state makinesi**, grafikte **OB/ENTRY/STOP** çizimi, isteğe bağlı **notify eşikleri** (§7).
- SMC için Pine’ı kopyalamaktan çok **mevcut `smart_money` ile zaman/price penceresinde kesiştirme** yapıldı; görsel overlay sonraki adım.

---

## 6. İlgili dosyalar (IQAI)

- `crates/iqai-core/src/elliott_detector.rs`, `elliott.rs`, `impulse_detector.rs`
- `crates/iqai-core/src/elliott_fusion.rs` — EWO, confluence, SMC–W2, stabilite
- `crates/iqai-core/src/config.rs` — `elliott_*` fusion/projeksiyon ayarları
- `crates/iqai-core/src/q_radar_analysis.rs`, `dip_confluence.rs`, `smart_money.rs`
- `crates/iqai-web/src/index.html` (Elliott paneli), `chart_data.rs`, `notify.rs`

---

## 7. Öncelikli eksikler (geliştirme adayları)

| Öncelik | Konu | Not |
|--------|------|-----|
| **A** | Kalıcı **Elliott state** (locked leg, cooldown, tek impulse ID) | Stateless motor ile birleştirme tasarımı şart |
| **B** | Grafikte **OB + ENTRY/STOP** (Pine görünürlüğü) | API’de fiyat kutusu/çizgi; `lightweight-charts` |
| **C** | **Notify eşiği** (örn. confluence ≥ X ve `validation_ok`) | `Notifier` + `config` eşikleri |

Bu üçü tamamlandığında dokümandaki “Pine fikir paketi” ürün tarafında büyük ölçüde kapanmış olur.
