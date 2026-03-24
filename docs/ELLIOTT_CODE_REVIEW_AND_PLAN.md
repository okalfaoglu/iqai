# Elliott Wave — Kod İncelemesi ve Geliştirme Planı

**Tarih:** 2026-03-22  
**Referanslar:**
- Repoda: [`ELLIOTT_WAVE_SPEC.md`](./ELLIOTT_WAVE_SPEC.md), `docs/THE_BASICS_OF_THE_ELLIOTT_WAVE_PRINCIPLE.pdf`
- **Backlog:** [`G05_ELLIOTT_Q_BACKLOG.md`](./G05_ELLIOTT_Q_BACKLOG.md) (varsa) + bu dosya.
- Ek PDF’ler (özet): İstanbul kültür üniversitesi tezi (Bitcoin–Elliott ilişkisi), XMSTradeX *Elliott Wave Konuları* (TradingView eğitim notları — konu sınıflandırması, W2/W4 Fib sıklıkları, diagonal/truncation/throw-over başlıkları)

Bu doküman, **otomatik sayım motorunun** ( `crates/iqai-core/src/elliott.rs`, `elliott_detector.rs`, `impulse_detector` ) spesifikasyonla uyumunu ve **eksik/hatalı** kısımları özetler; **UI** (`crates/iqai-web`) ayrı maddelerde yer alır.

---

## 1. Kaynaklardan Ortak Çıkarımlar

### 1.1 XMSTradeX notları (TradingView PDF) — pratik özet
- **İtkisel vs düzeltme** ayrımı net: motive dalgalar trend yönünde; düzeltmeler karşı yönde / yatay.
- **W2 / W4 Fib geri çekilme “sıklık” bandları** (yönerge, kural değil):
  - W2: çoğu zaman **0.618 → 0.786 → 0.5** sırasıyla rastlanır.
  - W4: çoğu zaman **0.382 → 0.5 → 0.236** sırasıyla rastlanır.
- **Majör/minör iç içe yapı** (ör. majör 1 impulse içinde minör 1/3/5 extension, leading/ending diagonal kombinasyonları) — **konu başlıkları** olarak listelenmiş; tam kural seti PDF’de tek tabloda değil, eğitim akışına yayılmış.
- **Irregular / expanded flat** ile **failure (başarısız) düzeltme** kavramının karıştırılmaması gerektiği vurgulanıyor (yanlış sayım ≠ irregular flat).
- **Throw-over / truncation** (özellikle diagonal ve W5/C uçları) ayrı başlıklar.

### 1.2 `ELLIOTT_WAVE_SPEC.md` (IQAI)
- Impulse **katı kurallar**: W2 sınırı, W3 en kısa değil, W4–W1 örtüşmesi (impulse’ta yasak), Fib hedef tabloları.
- Diagonal’da W4–W1 örtüşmesi **serbest**.
- Triangle: **W2 olamaz** (W4 veya B’de).
- **TODO** olarak: ABCDE üçgeninde her bacak için iç `abc` granülaritesi — spesifikasyon açıkça eksik işaretliyor.

### 1.3 Tez PDF’leri
- Teorik çerçeve ve literatür için iyi; **otomatik sayım algoritması** için doğrudan uygulanabilir kural seti sınırlı. Ürün kararları için `ELLIOTT_WAVE_SPEC.md` + Frost & Prechter / EWM cheat sheet öncelikli kalmalı.

---

## 1.4 Dalga dereceleri (Wave degrees) — **şu an yok**

Elliott literatüründeki **derece** kavramı (ör. Grand Supercycle → Subminuette; Türkçe eğitimde “dalga derecesi / büyük–küçük sayım”), IQAI motorunda **ayrı bir model olarak uygulanmıyor**:

| Beklenen (teori / XM / kitap) | Bizde |
|-------------------------------|--------|
| Her sayıma **derece etiketi** (ör. Primary, Intermediate, Minor, Minute …) veya sayısal seviye (0 = en büyük) | Yok; çıktıda dalga numarası (W1–W5, A–C) var, **derece alanı yok** |
| Üst derecedeki bir dalganın **içinde** alt derece 5–3 yapısı (fraktal iç içe) | Kısmi sezgi: farklı TF’lerde ayrı `compute_elliott`; **tek hiyerarşi ağacı / parent-child ilişkisi yok** |
| Aynı grafikte “bu W3 aslında bir derece üstteki (I)’nin iç W3’ü” tutarlılığı | Net **senkron / filtre** yok (Faz 2’de hedeflenen iş) |

Bu tablo **bilinçli eksiklik** olarak plana bağlandı; aşağıdaki **A0** ve **Faz 2 — Dalga dereceleri** maddeleri bunu kapatmayı hedefler.

---

## 2. Mevcut Kodda Güçlü Yönler (Özet)

Aşağıdakiler son dönemde kodda **bilinçli olarak** güçlendirildi veya doğrulandı:

| Konu | Durum |
|------|--------|
| Impulse W4’ün W3 ekstremunu aşmaması (bull/bear) | `validate_impulse_with_w5` — `w4_vs_w3_valid` |
| Flat B yönü / negatif `b_retrace` ile sahte “expanded” | `flat_valid_detailed` — `b_ratio` işaretli; Expanded üst sınırları |
| 6’lı pencerede W5’in yanlış etiketlenmesi | `find_impulse_window` — son pivot alternasyon düzeltmesi |
| Aynı barda çift swing (high+low) | `collect_swings` — `else if` |
| Geçerli impulse üzerine flat yazılması | `compute_elliott` — flat yalnızca geçersiz/boş sonuçta |
| Grafikte tepe/dip marker | `ElliottWavePointCore.is_high` + `index.html` |

---

## 3. Eksikler ve Hatalı / Zayıf Kısımlar

### 3.1 Kural / kapsam (yüksek öncelik)

| # | Sorun | Açıklama |
|---|--------|----------|
| A0 | **Dalga dereceleri modeli yok** | Klasik derece şeması (etiket + seviye), API’de `degree` / `parent_wave_id`, UI’da “(Minor) W3” gibi gösterim **tanımlı değil**. Üst/alt TF birleştirme olmadan “derece” tam anlamıyla tamamlanamaz. |
| A1 | **Çoklu derece (fraktal)** | XM notları majör/minör iç içe sayımları listeler; motor çoğunlukla **tek zaman diliminde** son swing penceresi ile çalışıyor. Üst derece trend ile alt derece impulse çakışması net değil. (**A0 ile birlikte** ele alınmalı.) |
| A2 | **“Guideline” vs “rule” ayrımı** | W2/W4 Fib “sıklık” listeleri **yönerge**; ikili geçer/geçersiz yerine **güven skoruna** dökülmediği sürece kullanıcı “neden bu seviye?” diye kafa karıştırıyor. |
| A3 | **Irregular / failure ayrımı** | Kod “flat” veya “zigzag” reddeder; **failure correction** / “yanlış sayım” etiketi yok. XM’in vurgusu ile uyum için ayrı sınıf veya `validation_msg` gerekir. |
| A4 | **Üçgen W2 yasağı** | Spesifikasyon: üçgen W2 olamaz. `try_triangle` / sınıflandırma **son swinglere** bakıyor; pozisyon (W2 vs W4) her zaman doğrulanmıyor olabilir — **audit** gerekli. |
| A5 | **İç ABC (ABCDE üçgen)** | `ELLIOTT_WAVE_SPEC.md` TODO: her bacakta `abc` — `validate_triangle_inner_abc` toleranslı; tam EWM uyumu yok. |

### 3.2 Algoritma / heuristik (orta öncelik)

| # | Sorun | Açıklama |
|---|--------|----------|
| B1 | **Kanal / zaman projeksiyonları** | `time_projection_*`, kanal fonksiyonları var; **doğrulama skoruna** ve GUI’de “neden bu W5 hedefi?” açıklamasına tam bağlı değil. |
| B2 | **Hacim** | Teoride W3 hacim vb.; kısmi alan `w3_volume_ok` — strateji katmanında tutarlı mı netleştirilmeli. |
| B3 | **Kombinasyon (W-X-Y)** | `try_double_three` / zigzag kuralları basitleştirilmiş; Post kuralları (ör. kombinasyonda max 1 zigzag) ile **kenar durum** testleri az. |

### 3.3 Sunum / UX (orta öncelik)

| # | Sorun | Açıklama |
|---|--------|----------|
| C1 | **Panel karmaşası** | Birden fazla W5 hedefi + projeksiyon + setup aynı kutuda; sadeleştirme / “detay aç” ile katmanlanmalı (kısmen `index.html` güncellendi). |
| C2 | **Tamamlanan vs hedef çizgisi** | Kullanıcı beklentisi: tamamlanan bacak düz, hedefler noktalı — `wave_legs.dotted` ile destekleniyor; **renk/legend** tutarlılığı sürdürülmeli. |

---

## 4. Geliştirme Planı (Fazlar)

### Faz 0 — Stabilizasyon (1–2 gün)
- [ ] `cargo test -p iqai-core --lib` CI’da zorunlu; **tek commit** ile prod/dev hizası (`docs/DEV_TO_PROD_DEPLOY.md` ile uyumlu).
- [ ] Elliott paneli: **varsayılan sade görünüm** + “Detaylı setup / ham projeksiyonları göster” toggle (`localStorage`).
- [ ] `validation_msg` sözlüğü: kullanıcıya kısa Türkçe açıklama (ör. W4–W3 ihlali, flat reddi nedeni).

### Faz 1 — Kural denetimi ve güven skoru (1–2 hafta)
- [ ] Impulse için ** guideline skoru**: W2 geri çekilmenin 0.5/0.618/0.786 bandına yakınlığı; W4’ün 0.236–0.382 bandına yakınlığı → `confidence` içinde göster.
- [ ] **Failure vs irregular**: geçersiz düzeltmede `formation_subreason: "FailureCorrectionSuspect"` gibi opsiyonel alan (yalnızca teşhis).
- [ ] Üçgen: **W2 yasağı** için pozisyon kontrolü — `elliott_detector` içinde net test + birim test.

### Faz 2 — Yapısal derinlik (2+ hafta)
- [ ] **Dalga dereceleri (çekirdek):**
  - [ ] `ElliottWaveDegree` veya benzeri enum + `degree: u8` / `degree_label` (Türkçe kısaltma opsiyonu) — `iqai-core` tiplerde.
  - [ ] Üst TF sayımı → alt TF’ye **referans** (ör. “Primary W3 içinde Minor sayımı”); isteğe bağlı `parent_context` veya üst TF sonuç özeti ile bağlama.
  - [ ] Web: panelde dalga etiketlerine derece gösterimi (ör. `[Minute] ③` veya metin — tasarım kararı).
- [ ] ABCDE üçgen: bacak bazlı **iç swing** toplama iyileştirmesi (`collect_inner_swings_between` + pivot yoğunluğu).
- [ ] Çoklu derece: üst TF’den gelen trend yönü / son majör swing ile alt TF impulse **çakışma** filtresi (ör. `market_context` / üst TF özeti ile).

### Faz 3 — Ürün / eğitim
- [ ] `docs/` içinde **tek sayfalık** “Elliott motoru ne yapar / ne yapmaz” (otomatik sayım sınırları).
- [ ] İsteğe bağlı: XM başlık listesi ile **eşleştirme tablosu** (hangi başlık kodda karşılık buluyor / bulmuyor).

---

## 5. Önerilen İzleme Metrikleri

- Geçersiz sayım oranı (per symbol, per TF)
- Kullanıcı “override” veya manuel etiket talebi (ileride)
- CI: `iqai-core` test sayısı ve süre

---

## 6. Son Not

Ekli **üç PDF** tam metin olarak burada işlenmedi; içerikleri **özet ve başlık düzeyinde** kullanıldı. Ürünün teknik sözleşmesi **`docs/ELLIOTT_WAVE_SPEC.md`** ve repodaki `elliott` modülleri ile devam etmeli; PDF’ler **eğitim ve doğrulama** için referans kalır.

**Sorumlu modüller:** `crates/iqai-core/src/elliott.rs`, `elliott_detector.rs`, `impulse_detector.rs`, `crates/iqai-web/src/index.html`, `chart_data.rs`.
