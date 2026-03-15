# Dip tespiti – katmanlı analitik yaklaşım eşlemesi

Bu doküman, “sistematik dip tespiti” için tanımladığınız dört katmanı mevcut kodla eşleştirir ve eksikleri öncelik sırasıyla listeler.

---

## Katman 1: Fiyat hareketinin yapısal analizi (ZigZag + MTF)

| Alt parça | Tarifiniz | Mevcut kod | Eksik / not |
|-----------|-----------|------------|-------------|
| **ZigZag matematiksel filtre** | Gürültü filtresi, yerel min (HL/LL) = potansiyel dip düğümü | `reversal.rs`: **pivot low** (swing low, sabit pivot_len). `elliott_detector`: **collect_swings** (pivot + %0.5 deviation). | ZigZag “düğüm” tek kaynak değil; pivot ve Elliott swing’ler ayrı. Ortak ZigZag düğüm çıktısı yok. |
| **MTF senkronizasyonu** | Üst TF destek/ZigZag düğümü → alt TF’de teyit (MSS) | `signal.rs`: **trend_strength** 7 TF (1M–1D) toplu. Dip tespiti **tek TF** (`reversal.rs`). | Üst TF’de “destek/dip bölgesi” + alt TF’de “yapı kırılımı (MSS)” bağı **yok**. |
| **Market Structure Shift** | Alt TF’de düşen tepelerin kırılıp yukarı dönmesi | `impulse_detector`: mini BOS/ChoCH. `signal.rs`: **structure_score**, **structure_shift** (pozisyon metrikleri). | Dip tespiti / Q-Analiz bu yapıyı **kullanmıyor**; MSS dip teyidi olarak entegre değil. |

**Özet:** Yapı var (pivot, swing, BOS) ama dip tespiti tek TF ve MSS/MTF ile bağlı değil.

---

## Katman 2: Dalga teorisi ve Fibonacci oranları

| Alt parça | Tarifiniz | Mevcut kod | Eksik / not |
|-----------|-----------|------------|-------------|
| **Elliott dalga döngüleri** | Dip = W2, W4 veya ABC’de C tükenmesi | `elliott_detector`: impulse (W1–W5), ZigZag/Flat/Triangle, W2/W4, ZigZag C. | **reversal / q_radar_analysis** Elliott’u **çağırmıyor**; dip “Elliott dibi” olarak işaretlenmiyor. |
| **Fibonacci cluster** | 0.618, 0.786, 0.886 + extension ile kesişim = yüksek olası dip | `elliott.rs`: W2 retrace (0.5, 0.618, 0.786, 0.854), ZigZag B/C, extension’lar. | Fiyatın “Fib cluster bölgesinde mi?” kontrolü **dip tespitinde yok**; cluster dip skoruna katılmıyor. |

**Özet:** Elliott ve Fib tamamen ayrı modülde; dip bölgesi tespiti bunlarla birleştirilmemiş.

---

## Katman 3: Momentum ve volatilite teyidi

| Alt parça | Tarifiniz | Mevcut kod | Eksik / not |
|-----------|-----------|------------|-------------|
| **Pozitif uyumsuzluk (divergence)** | Fiyat LL, momentum HL → satış zayıflıyor | `config`: **enable_divergence_scanner**. Elliott: **w5_divergence** (RSI). | Genel “fiyat LL + momentum HL” **reversal / dip** içinde hesaplanmıyor; dip skoruna girmiyor. |
| **Volatilite büzüşmesi** | Düşüş sonrası vol daralması / mean reversion | `signal.rs`: ATR, **volatility_pct**, trend exhaustion (geç faz + zayıf momentum). | “Vol büzüşmesi” veya bant dışı + geri dönüş **dip kriteri** olarak yok. |

**Özet:** Momentum/hacim dip gücünde kısmen var; divergence ve vol squeeze dip katmanına ekli değil.

---

## Katman 4: Emir akışı / hacim profili

| Alt parça | Tarifiniz | Mevcut kod | Eksik / not |
|-----------|-----------|------------|-------------|
| **Destekte hacim artışı + fiyat tutunması** | Absorption = dip teyidi | `reversal_strength_from_dip`: **hacim oranı** (son mum / ortalama) güç skoruna katılıyor. | “Destek bölgesinde hacim patlaması + fiyat düşmemesi” şeklinde **açık absorption** mantığı yok. |

**Özet:** Hacim güç skorunda var; “destek + absorption” ayrı bir teyit katmanı değil.

---

## Algoritmik karar mekanizması (sizin sıralama)

Sizin mantık:

1. Fiyat MTF destek / Fib bölgesine girdi mi? **(True)**
2. Momentum’da satıcı zayıfladı / uyumsuzluk var mı? **(True)**
3. Alt TF’de ZigZag yapısı yukarı kırıldı mı? **(True)**  
→ Yüksek olasılıklı long.

**Mevcut durum:**  
Q-Analiz dip’i sadece **(tek TF)** pivot dip + margin + son mum yükselişi ile “True” yapıyor. MTF destek, Fib cluster, divergence ve alt TF yapı kırılımı bu karara **bağlı değil**.

---

## Önerilen uygulama sırası

| Öncelik | Katman | Ne yapılabilir | Nereye |
|--------|--------|----------------|--------|
| **A** | MTF + MSS | Üst TF’de pivot/destek bölgesi tespit et; sadece “fiyat bu bölgedeyse” alt TF’de dip adayı aç. Alt TF’de BOS/ChoCH veya structure_shift = yukarı kırılım varsa dip teyidi güçlendir. | `reversal.rs` veya yeni `dip_confluence.rs`; `compute_q_radar_opportunity` buffer’da çok TF kullanacak şekilde. |
| **B** | Elliott + Fib cluster | `compute_elliott` + Fib seviyeleri (W2/W4/ZigZag B: 0.618, 0.786, 0.886) ile “fiyat cluster’da mı?” kontrolü. Dip adayı cluster’daysa güven/erken uyarı artır veya “Elliott dip” etiketi ekle. | `q_radar_analysis.rs`: dip bölgesi + Elliott sonucu + Fib zone; `build_detection_and_recommendation` içinde confluence skoru. |
| **C** | Divergence + vol squeeze | Fiyat LL + RSI (veya benzeri) HL = bullish divergence; ATR veya bant daralması. İkisi de dip teyidi için ek koşul. | Yeni `divergence` / `volatility_squeeze` yardımcıları; `reversal` veya `q_radar_analysis` içinde ek filtre. |
| **D** | Absorption | Destek bandında (dip + margin) hacim > eşik ve son N bar’da fiyat bandı kırmıyorsa “absorption” = ek teyit. | `reversal.rs` veya confluence modülünde hacim + fiyat bandı mantığı. |

---

## Hangi kurguya odaklanalım?

1. **Async ZigZag düğüm hesaplayıcı**  
   Tek, tutarlı ZigZag düğüm çıktısı (deviation/varyans eşiği); hem Elliott hem dip bu düğümlere bassın. Rust’ta `reversal` veya `indicators` tarafında tek bir ZigZag pipeline.

2. **Elliott + Fibonacci kesişimleri**  
   Mevcut `compute_elliott` + Fib seviyeleri; “fiyat Fib cluster’da + Elliott W2/W4/C dibi” ise dip bölgesi skorunu artıran mantıksal sınamalar (Rust, `q_radar_analysis` veya `reversal`).

3. **MTF + MSS dip teyidi**  
   Üst TF destek bölgesi + alt TF’de yapı kırılımı (BOS/ChoCH/structure_shift) ile dip’i “True” yapan event-driven mantık (Rust, buffer çok TF ile).

İsterseniz önce **B (Elliott/Fib confluence)** veya **A (MTF + MSS)** ile somut fonksiyon imzaları ve `compute_q_radar_opportunity` / `compute_reversal_analysis` entegrasyon adımlarını yazabilirim.
