# `content.pdf` §2.5.3 / §2.5.4 × IQAI kod eşlemesi

**Not:** `docs/content.pdf` bu repoda **ikili (sıkıştırılmış)** olduğu için metin otomatik çıkarılamadı. Aşağıdaki tablolar, **Elliott Wave Monitor cheat sheet** ile hizalı `docs/ELLIOTT_WAVE_SPEC.md` maddelerine göre IQAI’daki karşılıkları listeler. Tezdeki numaralandırma veya ek cümleler farklıysa, lütfen `pdftotext docs/content.pdf -` çıktısından ilgili paragrafları bu dosyaya ekleyin.

---

## §2.5.3 İtki (Impulse) dalgaları — kurallar → kod

| Kural (EWM / spec özeti) | IQAI uygulaması | Dosya / not |
|--------------------------|-----------------|-------------|
| W2, W0 (W1 başlangıcı) üzerine taşınmaz | `validate_impulse` / `validate_impulse_with_w5` → `w2_valid` | `elliott.rs` |
| W3, W1–W3–W5 içinde **en kısa** olamaz | `w3_valid` (W5 varken `min(W1,W5)` ile karşılaştırma) | `elliott.rs` |
| W4, W1 tepe/dibi ile **örtüşmez** (klasik impulse) | `w4_vs_w1_valid`, `w4_vs_w3_valid` | `elliott.rs` |
| W1–W3–W5 aynı anda “extended” olamaz (N°11) | `no_triple_extension_valid` | `elliott.rs` |
| İç yapı: itki 5-3-5-3-5 (dalga içinde dalga) | `collect_inner_swings_between` + `validate_subwave_structure` / **`validate_subwave_structure_with_mode(..., strict)`** | `elliott_detector.rs`, `elliott.rs` |
| Seviye 2 iç-iç doğrulama | `validate_subwave_deep` | `elliott.rs` |
| Diyagonal: W4–W1 örtüşebilir; impulse’tan fark | `validate_diagonal` | `elliott.rs` |
| Leading / Ending alt yapı | `classify_diagonal_sub_structure`, `diagonal_inner_counts` | `elliott_detector.rs` |

**Yapılandırma:** `smart_money.elliott_subwave_strict` — `true` iken iç-dalga kontrolü **5/5 bacak** uyumu ister (1:1 tez modu); `false` (varsayılan) en az 3/5 uyum (pratik tolerans).

---

## §2.5.4 Düzeltme (Corrective) dalgaları — kurallar → kod

| Kural (EWM / spec özeti) | IQAI uygulaması | Dosya / not |
|--------------------------|-----------------|-------------|
| Zigzag A-B-C: 5-3-5; B, A başlangıcını aşmaz | `validate_zigzag_abc`, `build_zigzag_result` | `elliott_detector.rs` |
| Zigzag/flat **iç** swing: A,B,C sayımı | `validate_corrective_subwaves` / **`validate_corrective_subwaves_with_mode(..., strict)`** | `elliott.rs` |
| Flat tipleri (regular / expanded / running) | `flat_valid_detailed`, `build_flat_result` | `elliott.rs`, `elliott_detector.rs` |
| Üçgen ABCDE; **W2’de üçgen olamaz** | `try_triangle`, `validate_triangle_abcde`, `triangle_wave2_context_blocked` | `elliott_detector.rs`, `elliott.rs` |
| Üçgen bacak içi abc (yönerge) | `validate_triangle_inner_abc` | `elliott_detector.rs` |
| Kombinasyon: W-X-Y kuralları (ör. tek üçgen, W’de üçgen yok) | `try_double_three`, `classify_segment` | `elliott_detector.rs` |
| Düzeltme bütünü “tek başına 5 itki” sayılmaz | Ayrı zigzag/flat/triangle dalları; motive ile karıştırılmaz | `compute_elliott` akışı |

**Dalga içinde dalga (düzeltme):** Zigzag ve flat için A/B/C aralıklarında pivot sayısı; üçgen için her A–E bacığında alt swing.

---

## Eksik kalanlar (tam 1:1 için)

1. **PDF metni doğrulanmadı** — tezde ek numaralı kural varsa buraya satır eklenmeli.
2. **Üst derece / çoklu TF** — “büyük dalga = küçük 5’li” tutarlılığı tek TF pivot penceresinde tam modellenmez (`ELLIOTT_CODE_REVIEW_AND_PLAN.md` Faz 2).
3. **Üçgen her bacakta tam abc** — `ELLIOTT_WAVE_SPEC.md` TODO; pivot yoğunluğu gerekir.

---

## İlgili ayarlar

| Config | Anlam |
|--------|--------|
| `elliott_subwave_strict` | İtki ve zigzag/flat iç-dalga **katı** mod (varsayılan `false`) |
