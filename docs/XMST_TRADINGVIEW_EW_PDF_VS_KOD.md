# XMSTradeX / TradingView Elliott PDF’leri × IQAI kod — TODO / DONE

**Tarih:** 2026-03-20  
**Amaç:** *Elliott Wave Konuları* tarzı eğitim materyali ile `iqai-core` / `iqai-web` Elliott uygulamasını karşılaştırmak; **eksik, hatalı, problemli veya bizde olmayan** özellikleri tek yerde toplamak.

---

## 1. Kaynak PDF’ler ve okuma notu

| # | Kaynak | Repoda |
|---|--------|--------|
| 1 | *The Basics of the Elliott Wave Principle* (metin tabanlı eğitim PDF’i) | `docs/THE_BASICS_OF_THE_ELLIOTT_WAVE_PRINCIPLE.pdf` |
| 2 | İsteğe bağlı metin çıktısı (`poppler-utils` → `pdftotext`) | `docs/THE_BASICS_OF_THE_ELLIOTT_WAVE_PRINCIPLE.txt` — **yoksa** yerelde üretin; repoya commit isteğe bağlı |
| 3 | Eski Cursor storage yolları (`46249.pdf`, `content.pdf`) | Repoda yok; içerik bilinmiyor |
| 4 | XMSTradeX *Elliott Wave Konuları* (TradingView) | PDF repoda yoksa **kısmi eşdeğer:** `docs/ELLIOTT_CODE_REVIEW_AND_PLAN.md` **§1.1** |

**Sayfa / bölüm eşlemesi:** İçindekileri **§4** tablosuna işleyin; TXT’den kopyalamak hızlıdır, **görselli sayfalar** için aşağıdaki nota bakın.

**Teknik sözleşme (kurallar):** `docs/ELLIOTT_WAVE_SPEC.md` + Frost/Prechter uyumlu cheat sheet çevirisi aynı repoda.

### 1.1 PDF → TXT (`pdftotext`)

- Paket: `sudo apt install -y poppler-utils`
- Örnek: `pdftotext -layout "docs/THE_BASICS_OF_THE_ELLIOTT_WAVE_PRINCIPLE.pdf" "docs/THE_BASICS_OF_THE_ELLIOTT_WAVE_PRINCIPLE.txt"`
- Amaç: içindekiler ve başlıkları aramak; **§4** doldurmak.

### 1.2 Resim, şema ve taranmış sayfalar

`pdftotext` yalnızca PDF’deki **metin katmanını** okur:

| Durum | TXT’de ne olur | Ne yapılmalı |
|--------|----------------|--------------|
| Normal metin + gömülü yazı | Başlıklar/metin gelir | §3–§4 ile eşle |
| Sayfa çoğunlukla **görsel / şema** | Boş, çok kısa veya anlamsız | PDF’de **sayfa numarasını** §4’e yaz; kuralı `ELLIOTT_WAVE_SPEC.md` veya elle özetle |
| **Taranmış** PDF (tarayıcı görüntüsü) | Genelde metin yok | Gerekirse OCR (`ocrmypdf`, `tesseract`) veya PDF’den elle not |

**Özet:** “İçinde resim olabilir” = TXT tek başına yeterli olmayabilir; **§4’te PDF sayfa referansı** tutmak şart.

---

## 2. Durum anahtarı

| Etiket | Anlamı |
|--------|--------|
| **DONE** | Üretim kodunda anlamlı karşılık var (kural veya sezgisel tespit). |
| **KISMEN** | Kısmi / heuristic / skor; tam EWM/PDF uyumu yok. |
| **TODO** | Yok veya sadece dokümanda planlı. |
| **RİSK** | Bilinen zayıf nokta / audit gerekir. |

---

## 3. XMSTradeX / TradingView “Elliott Wave Konuları” × kod (özet matris)

> Aşağıdaki satırlar **`ELLIOTT_CODE_REVIEW_AND_PLAN.md` §1.1** ve **`ELLIOTT_WAVE_SPEC.md`** ile hizalanmıştır. PDF’deki sıra farklı olabilir.

### 3.1 İtki (motive) ve düzeltme ayrımı

| Konu (eğitim) | Durum | Kod / not |
|---------------|-------|-----------|
| Motive vs corrective ayrımı | **KISMEN** | `compute_elliott` önce impulse penceresi, sonra zigzag/flat/triangle denemesi; “motif” etiketi `formation` / `formation_type` ile gelir. |
| Impulse 5-3-5-3-5 kuralları (W2 sınırı, W3 en kısa değil, W4–W1 örtüşmesi impulse’ta yasak) | **KISMEN** | `elliott.rs` — `validate_impulse_with_w5` vb.; edge-case’ler için `ELLIOTT_CODE_REVIEW_AND_PLAN.md` **§3.1 A0–A5**. |
| Diagonal (LD/ED, contracting/expanding), W4–W1 örtüşmesi diagonalda serbest | **KISMEN** | `validate_diagonal`, `impulse_detector` aşamaları; tam alt tür ayrımı her senaryoda doğrulanmayabilir → **RİSK**. |
| Throw-over / truncation (W5, diagonal/C uçları) | **KISMEN** | Alanlar: `throw_over`, `truncation` (`ElliottDetectorResult`); tam görsel/teknik PDF uyumu için detay audit: **`elliott_detector.rs`**, **`elliott.rs`**. |

**Detaya girilmesi gereken yerler:** `crates/iqai-core/src/elliott.rs` (doğrulama fonksiyonları), `crates/iqai-core/src/elliott_detector.rs` (`build_impulse_result`, `compute_elliott` sonu dalları).  
**Referans doküman:** `docs/ELLIOTT_WAVE_SPEC.md` — *Impulse Wave*, *Diagonal Waves* bölümleri (kaynak sayfa: EWM cheat sheet linkleri o dosyada).

---

### 3.2 Düzeltme yapıları (zigzag, flat, üçgen, kompleks)

| Konu | Durum | Kod / not |
|------|-------|-----------|
| Zigzag A-B-C (5-3-5), B sınırı | **KISMEN** | `build_zigzag_result`, `compute_setup_zigzag_c`; iç swing doğrulama `validate_corrective_subwaves`. |
| Flat regular / expanded / running | **KISMEN** | `flat_valid_detailed`, `build_flat_result`; irregular vs “failure” ayrımı **TODO** → `ELLIOTT_CODE_REVIEW_AND_PLAN.md` **A3**. |
| Triangle (contracting/expanding), ABCDE | **KISMEN** | `try_triangle`, `validate_triangle_*`; **W2’de üçgen olamaz** → heuristik: toplam **8** pivot’ta `triangle_wave2_context_blocked` + `validation_ok: false`, `elliott_invalidate_hint` (`elliott.rs` / `elliott_detector.rs`). Tam pozisyon (W4 vs B) için daha geniş audit → **A4** kısmen. |
| Double/triple zigzag ve kombinasyonlar | **KISMEN** | `try_double_zigzag`, `try_triple_zigzag`, `try_double_three` — Post kuralları kenar durumları **B3** (plan). |

**Detaya girilmesi gereken yerler:** `elliott_detector.rs` (zigzag/flat/triangle builder’lar), `elliott.rs` (triangle/flat validasyonları).  
**Referans doküman:** `docs/ELLIOTT_WAVE_SPEC.md` — *Corrective Waves*, *Flat*, *Triangle* bölümleri.

---

### 3.3 Fibonacci: W2 / W4 “sıklık” ve hedefler

| Konu (XM özeti) | Durum | Kod / not |
|-------------------|-------|-----------|
| W2: 0.618 → 0.786 → 0.5 gibi **sıklık** (yönerge) | **KISMEN** | `elliott_fusion.rs` — W2/W1 oranı + `elliott_fib_tolerance_pct` ile confluence; **kural olarak zorunlu değil** → A2 (plan). |
| W4: 0.382 → 0.5 → 0.236 sıklığı | **KISMEN** | Projeksiyon/kanal ile kısmen; guideline skoru **Faz 1** TODO (plan §4). |
| Hedef tabloları (W3/W5 uzantıları) | **KISMEN** | `compute_projections`, `w5_targets`, config `elliott_wave3_extension` vb. |

**Referans:** `ELLIOTT_CODE_REVIEW_AND_PLAN.md` **§1.1**; `docs/ELLIOTT_WAVE_SPEC.md` Fibonacci tabloları.

---

### 3.4 Dalga dereceleri (majör/minör, iç içe sayım)

| Konu | Durum | Kod / not |
|------|-------|-----------|
| Derece etiketi (Grand … Subminuette) | **KISMEN** | `WaveDegree` + `infer_wave_degree` (TF + bar sayısı); **hiyerarşik ağaç / üst-alt TF senkronu** → **TODO** (plan **§1.4**, **Faz 2**). |
| Grafikte dereceye göre etiket biçimi | **KISMEN / DONE** | `format_wave_label_for_degree` (`elliott.rs`) + `label_display` (`chart_data.rs`) — repoya göre; kontrol: `git` güncel mi. |
| Aynı grafikte “üst derece W3 = alt impulse” tutarlılığı | **TODO** | Üst TF özeti / `parent_context` (plan Faz 2). |

---

### 3.5 Gösterge / TradingView tarzı yardımcılar

| Konu | Durum | Kod / not |
|------|-------|-----------|
| EWO (Elliott Wave Oscillator) | **DONE** | `elliott_fusion.rs`, `elliott_ewo_*` config; panel. |
| Confluence + harf notu | **DONE** | `confluence_score`, `wave_grade`. |
| SMC–W2 (OB/FVG) skor katkısı | **DONE** | `smc_w2_zone_overlap`, `smart_money`. |
| Grafikte OB kutusu + ENTRY/STOP | **KISMEN** | `ElliottFusionChartOverlay` + `showEwFusionOverlay` (`index.html`) — **branch’te varsa DONE**; yoksa **TODO** → `PINE_EW_SMC_FUSION_PORT_ANALYSIS.md` §7-B. |
| Kalıcı sayım state (locked, cooldown) | **TODO** | Stateless motor + isteğe bağlı snapshot (aynı Pine analiz dokümanı §4 P1). |

---

## 4. Elle PDF sayfa eşlemesi (şablon — doldurun)

*Kaynak: `THE_BASICS_… .txt` içindekiler / başlıklar + gerektiğinde PDF’de sayfa numarası (görseller için).*

| PDF sayfa / bölüm | Konu başlığı | Bu dosyada § | Kod modülü | Not |
|-------------------|--------------|--------------|------------|-----|
| *örn. s. 3* | *örn. Impulse kuralları* | §3.1 | `elliott.rs` | |
| | | | | *şema sayfasıysa: “diagram only”* |
| | | | | |

---

## 5. Özet: “PDF’de var, bizde yok / zayıf” (öncelik)

1. **Fraktal / çoklu derece hiyerarşisi** (üst TF alt TF ilişkisi) — **TODO**  
2. **Guideline vs rule** ayrımının kullanıcıya net skor olarak yansıması — **KISMEN / TODO** (`confidence` birleşik)  
3. **Failure correction vs irregular flat** ayrı teşhis etiketi — **TODO**  
4. **Üçgen W2 yasağı** — **KISMEN**: 8-pivot heuristik + testler; tam W4/B ayrımı hâlâ **RİSK**  
5. **ABCDE iç ABC** granülaritesi — **TODO** (`ELLIOTT_WAVE_SPEC.md` notu)  
6. **TradingView tarzı tam state makinesi** (lock/cooldown) — **TODO**  
7. **Notify eşiği** (yüksek confluence) — **TODO** (`notify.rs` ile bağlama)

---

## 6. İlgili dosyalar (IQAI)

| Dosya | İçerik |
|-------|--------|
| `docs/ELLIOTT_CODE_REVIEW_AND_PLAN.md` | XM özet + kod audit maddeleri (A0–C2, Faz 0–3) |
| `docs/ELLIOTT_WAVE_SPEC.md` | Kural referansı (EWM cheat sheet çevirisi) |
| `docs/THE_BASICS_OF_THE_ELLIOTT_WAVE_PRINCIPLE.pdf` (ve isteğe bağlı `.txt`) | Dış eğitim PDF’i; TXT = `pdftotext` çıktısı |
| `docs/PINE_EW_SMC_FUSION_PORT_ANALYSIS.md` | Pine/fusion özellik eşlemesi |
| `crates/iqai-core/src/elliott.rs` | Kurallar, setup, dalga derecesi formatı |
| `crates/iqai-core/src/elliott_detector.rs` | Ana tespit ve sonuç üretimi |
| `crates/iqai-core/src/impulse_detector.rs` | CHoCH / W2 / BOS aşamaları |
| `crates/iqai-core/src/elliott_fusion.rs` | EWO, confluence, SMC–W2, overlay (varsa) |
| `crates/iqai-web/src/chart_data.rs`, `index.html` | API + grafik |

---

## 7. Son not

Bu dosya **görsel şemaları otomatik anlamaz**; metin katmanı `pdftotext` ile TXT’ye alınabilir (**§1.1**). XMSTradeX *Elliott Wave Konuları* PDF’i repoda yoksa uyum, **`ELLIOTT_CODE_REVIEW_AND_PLAN.md`** özeti ve kod taramasına dayanır. **§4** dolduruldukça eğitim–kod izlenebilirliği netleşir.

**Sonraki adım:** `THE_BASICS_… .txt` içinden içindekileri **§4**’e satır satır işleyin; şema ağırlıklı sayfalarda **PDF sayfa no** + “diagram” notu kullanın.
