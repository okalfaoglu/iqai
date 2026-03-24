# XMSTradeX / TradingView Elliott PDF’leri × IQAI kod — TODO / DONE

**Tarih:** 2026-03-24  
**Amaç:** *Elliott Wave Konuları* tarzı eğitim materyali ile `iqai-core` / `iqai-web` Elliott uygulamasını karşılaştırmak; **eksik, hatalı, problemli veya bizde olmayan** özellikleri tek yerde toplamak.

---

## 1. Kaynak PDF’ler ve okuma notu

| # | Sizin paylaştığınız dosya (Cursor workspace storage) | Repoda doğrudan kopyası |
|---|------------------------------------------------------|-------------------------|
| 1 | `…\pdfs\83c23f9f-…\46249.pdf` | **Yok** — içerik bu makinede açılmadı; adından tez/rapor olabileceği **varsayılır**. |
| 2 | `…\pdfs\c2643919-…\content.pdf` | **Yok** — aynı. |
| 3 | `…\BINANCE_BTCUSDT için XMSTradeX tarafından ELLİOT WAVE KONULARI — TradingView.pdf` | **Kısmi eşdeğer:** `docs/ELLIOTT_CODE_REVIEW_AND_PLAN.md` **§1.1** (XMSTradeX notları özeti). |

**Önemli:** Bu ortamda PDF metni taranmadı. **Sayfa numarası** ve **bölüm başlığı** eşlemesi için:

- PDF’leri `docs/` altına kopyalayıp (ör. `docs/ref/XMST_EW_KONULARI.pdf`) repoda tutabilirsiniz, **veya**
- İçindekiler tablosunu buraya yapıştırarak **§4 Elle sayfa eşlemesi** tablosunu doldurabilirsiniz.

**Teknik sözleşme (kurallar):** `docs/ELLIOTT_WAVE_SPEC.md` + Frost/Prechter uyumlu cheat sheet çevirisi aynı repoda.

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
| Triangle (contracting/expanding), ABCDE | **KISMEN** | `try_triangle`, `validate_triangle_*`; **W2’de üçgen olamaz** kuralı → **RİSK** (audit) → aynı plan **A4**. |
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

*XMSTradeX PDF içindekiler veya bölüm başlıkları buraya yapıştırıldıkça güncellenir.*

| PDF sayfa / bölüm | Konu başlığı | Bu dosyada § | Kod modülü |
|-------------------|--------------|--------------|------------|
| *örn. s. 3* | *örn. Impulse kuralları* | §3.1 | `elliott.rs` |
| | | | |
| | | | |

---

## 5. Özet: “PDF’de var, bizde yok / zayıf” (öncelik)

1. **Fraktal / çoklu derece hiyerarşisi** (üst TF alt TF ilişkisi) — **TODO**  
2. **Guideline vs rule** ayrımının kullanıcıya net skor olarak yansıması — **KISMEN / TODO** (`confidence` birleşik)  
3. **Failure correction vs irregular flat** ayrı teşhis etiketi — **TODO**  
4. **Üçgen W2 yasağı** tam pozisyon audit — **RİSK**  
5. **ABCDE iç ABC** granülaritesi — **TODO** (`ELLIOTT_WAVE_SPEC.md` notu)  
6. **TradingView tarzı tam state makinesi** (lock/cooldown) — **TODO**  
7. **Notify eşiği** (yüksek confluence) — **TODO** (`notify.rs` ile bağlama)

---

## 6. İlgili dosyalar (IQAI)

| Dosya | İçerik |
|-------|--------|
| `docs/ELLIOTT_CODE_REVIEW_AND_PLAN.md` | XM özet + kod audit maddeleri (A0–C2, Faz 0–3) |
| `docs/ELLIOTT_WAVE_SPEC.md` | Kural referansı (EWM cheat sheet çevirisi) |
| `docs/PINE_EW_SMC_FUSION_PORT_ANALYSIS.md` | Pine/fusion özellik eşlemesi |
| `crates/iqai-core/src/elliott.rs` | Kurallar, setup, dalga derecesi formatı |
| `crates/iqai-core/src/elliott_detector.rs` | Ana tespit ve sonuç üretimi |
| `crates/iqai-core/src/impulse_detector.rs` | CHoCH / W2 / BOS aşamaları |
| `crates/iqai-core/src/elliott_fusion.rs` | EWO, confluence, SMC–W2, overlay (varsa) |
| `crates/iqai-web/src/chart_data.rs`, `index.html` | API + grafik |

---

## 7. Son not

Bu dosya **PDF ikili içeriğini otomatik okuyamaz**; XMSTradeX *Elliott Wave Konuları* ile uyum, repodaki **metin özetleri** ve kod taramasına dayanır. PDF’yi `docs/ref/` altına ekleyip içindekileri **§4**’e işlerseniz, eğitim–kod izlenebilirliği tamamlanır.

**Sonraki adım (öneri):** `46249.pdf` ve `content.pdf` için kısa **içindekiler** (1 sayfa) buraya yapıştırın; madde numaralarını §3 alt başlıklarına bağlarız.
