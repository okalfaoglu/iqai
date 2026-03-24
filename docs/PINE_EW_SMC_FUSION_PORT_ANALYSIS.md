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
| **EWO (Elliott Wave Oscillator)** | EMA hızlı/yavaş oranı, sinyal çizgisi, güç eşiği, isteğe bağlı zorunlu onay | Repoda **yok**; momentum teyidi katmanı olarak eklenmeye değer |
| **Confluence + not** | W2 oranı, OB/FVG, hacim, EMA50, EWO → skor + harf notu | Q-RADAR / setup ile birleştirilebilir tek “EW kalite” metriği |
| **SMC birleşimi** | W2 anında OB/FVG aranıp kutulanması | IQAI `smart_money` zengin; **aynı zaman penceresinde** EW ile kesiştirme eksik olabilir |
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
| EWO | **Yok** |
| Harf notu / tek confluence skoru | Kısmi: `q_radar_analysis`, dip confluence; **EW’ye özel “grade” yok** |
| Lock + cooldown + invalidate yaşam döngüsü | **Yok** (her bar yeniden `compute_elliott` benzeri akış) |

---

## 4. Taşıma önerisi (öncelik sırası)

### P0 — Düşük risk, yüksek fikir değeri
- [ ] **EW kalite / confluence skoru (Pine’daki `calcConfluence` ilhamlı):** W2/W1 bandına yakınlık, hacim spike, isteğe bağlı EMA hizası — mevcut `ElliottDetectorResult` veya Q-RADAR zenginleştirmesine alan ekleme.
- [ ] **Konfigürasyon:** `config` veya `AppConfig` üzerinden Pine’daki gibi `wave3_ext`, `wave4_retrace`, `fib_tolerance` (projeksiyon/“guideline” için) — **kural doğrulamasını gevşetmeden** sadece hedef/projeksiyon tarafında.

### P1 — Orta efor
- [ ] **EWO modülü:** `ewo = (EMA_fast/EMA_slow - 1) * 100`, sinyal EMA; `ElliottDetectorResult` veya ayrı struct’ta `ewo_bull`, `ewo_cross`, `ewo_strong` — Web’de küçük satır; isteğe bağlı “setup filtresi”.
- [ ] **Aktif sayım durumu (state):** `ElliottPatternState { locked, current_leg, start_time, invalidate_reason }` — API’de tek nesne; cooldown / min bar mesafesi burada. *Dikkat:* Mevcut stateless tespitle birleştirmek için net spec gerekir (ör. “son geçerli impulse ID + sonraki barlarda sadece güncelle”).

### P2 — SMC füzyonu
- [ ] W2/W4 teyit bölgesinde `build_smart_money_context` ile **OB/FVG çakışması** bayrağı (Pine’daki kutu yerine skor katkısı).
- [ ] Chart’ta isteğe bağlı “EW + SMC confluence” işareti (mevcut overlay sistemine).

### P3 — UX
- [ ] Panelde Pine dashboard benzeri: **not (A–D)**, **confluence %**, **aktif dalga numarası**, **invalidate nedeni** (string).
- [ ] Alert/notify: Yüksek confluence + geçerli impulse → mevcut `Notifier` ile hizala.

---

## 5. Sonuç

- Pine script **tam Elliott motoru değil**; güçlü tarafı **durum + stabilite + skor + EWO + basit SMC görünürlüğü**.
- IQAI **kurallı sayım** tarafında genelde daha güçlü; taşınacak başlıca değer: **EWO**, **EW’ye özel confluence/grade**, **yaşam döngüsü (lock/invalidate/cooldown)** ve **kullanıcı ayarlı projeksiyon oranları**.
- SMC için Pine’ı kopyalamaktan çok **mevcut `smart_money` ile zaman/price penceresinde kesiştirme** yapılmalı.

---

## 6. İlgili dosyalar (IQAI)

- `crates/iqai-core/src/elliott_detector.rs`, `elliott.rs`, `impulse_detector.rs`
- `crates/iqai-core/src/q_radar_analysis.rs`, `dip_confluence.rs`, `smart_money.rs`
- `crates/iqai-web/src/index.html` (Elliott paneli), `chart_data.rs`
