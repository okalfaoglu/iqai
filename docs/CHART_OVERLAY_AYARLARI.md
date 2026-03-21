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
