# Grafik katmanları (iqai-web)

## Klasik “Kanal” ve mor numaralar (1–5)

- **Backend’de** `detect_classic_patterns` + mümkünse `draw.zigzag_points` / `upper_line` / `lower_line` geometrisi kullanılır; mor numaralar bu zigzag pivotlarıdır.
- Çizim **heuristic** (pivot ve seviye tespitine bağlı); piyasada “tek doğru” kanal yoktur — grafik bir **model önizlemesi**dir, kesin yön garantisi değildir.

## Menü: “Yatay / ölçek” (sol çekmece → Formasyonlar)

| Seçenek | Etki |
|---------|------|
| **Elliott Fibo + W5 yatayları** | Sağdaki kısa yatay Fibo segmentleri ve W5 hedef yatayları. |
| **Elliott kanal (üst/alt)** | Elliott üst/alt kanal çizgileri. |
| **Klasik TP / invalidation yatayları** | Klasik formasyon motorunun TP ve invalidation yatay çizgileri (kalabalığın büyük kısmı). |
| **Sağda çizgi fiyat etiketleri** | Her `LineSeries` için fiyat ölçeğindeki renkli etiketler. Varsayılan **kapalı**; ihtiyaç halinde açın. |

Ayarlar `localStorage` içinde `iqai-overlay-state` ile saklanır.

## Elliott Wave marker’ları (TradingView’e yakın)

- **Etiket biçimi:** Varsayılan **Primary** derece — `(0)` … `(5)`, `(A)` … `(E)` (API `label_display`, `crates/iqai-core/src/elliott.rs`).
- **Grafikte `W` ön eki yok** — sadece dalga + fiyat (kalabalık azaltma).
- **Renk:** itki **1/3/5** yeşil, **2/4** turuncu; üçgen / düzeltme **A/C/E** mavi ton, **B/D** pembe; **0** sarı.
- **Geçmiş formasyonlar** ile ana sayım aynı renk şemasını kullanır (dalga harfine göre); çizgi renkleri formasyon indeksine göre kalır.

İki ayrı formasyon aynı grafikte **C–D–E** ile başlıyormuş gibi görünebilir — biri geçmiş tarama, biri güncel; **Geçmiş Formasyonlar** listesinden ayırt edilir.
