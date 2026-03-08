# Elliott Dalga Teorisi – Algoritmik Spesifikasyon

EWT, finansal piyasalardaki fraktal yapıları analiz etmek için kullanılan kapsamlı bir sistemdir. Algoritmik tespit için katı matematiksel kurallar ve esnek yönergelere (guidelines) ihtiyaç vardır.

---

## 1. İtki (Motive) Dalgaları

### A. Standart İtki (Impulse) – 5-3-5-3-5

**Katı Kurallar (Geçersizlik / Invalidation):**
1. **W2** → W1 başlangıcı (W0) altına inemez / üstüne çıkamaz (geri çekilme %100'den küçük)
2. **W3** → W1, W3, W5 arasında asla en kısa olamaz
3. **W4** → W1 tepe/dip ile örtüşemez (kesişim yasak)

**Fibonacci Fiyat Hedefleri:**
| Dalga | Hedefler |
|-------|----------|
| W2 | W1'in %50, %61.8 veya %78.6 geri çekilmesi |
| W3 | W1'in %161.8, %261.8 veya %423.6 uzantısı |
| W4 | W3'ün %23.6, %38.2 (en yaygın) veya %50 geri çekilmesi |
| W5 | W1 uzunluğuna eşit (1.0) VEYA W0→W3 mesafesinin %61.8'i |

### B. Diyagonaller (Diagonals)

**Öncü Diyagonal (Leading):** Sadece W1 veya A. İç yapı 5-3-5-3-5 veya 3-3-3-3-3.
**Sonlanan Diyagonal (Ending):** Sadece W5 veya C. İç yapı 3-3-3-3-3.

**Kurallar:** W4 ile W1 örtüşebilir. W3 en kısa olamaz. İki trend çizgisi arasında daralan/genişleyen kanal. W5 throw-over veya truncation.

---

## 2. Düzeltme (Corrective) Dalgaları

### A. ZigZag (5-3-5)
- B, A başlangıcını aşamaz
- B: A'nın %38.2, %50, %61.8
- C: A'nın %100 veya %161.8

### B. Flat (3-3-5)
- B, A'nın %90'ından fazlasını geri alır
- **Regular:** B≈%100, C≈%100
- **Expanded:** B > %123.6–138.2, C > %123.6–161.8
- **Running:** B aşar, C trend yönünde patlar

### C. Triangle (3-3-3-3-3)
- Her dalga öncekinin ~%61.8 veya %78.6
- Sadece W4, B veya X konumunda

### D. Karmaşık (W-X-Y / W-X-Y-X-Z)
- X dalgaları bağlantı; herhangi bir 3’lü düzeltme olabilir

---

## 3. Zaman Fibonacci (Time Projections)

- **W3 süresi:** W1 süresinin %100, %161.8 veya %261.8 (bar sayısı)
- **Alternans:** W2 kısa (örn. 10 bar) → W4 uzun (%261.8 veya %423.6)
- **Düzeltme:** W2, W1 süresine eşit veya daha uzun

---

## 4. Formasyon Başlangıcı

- **CHoCH (Market Structure Break):** Son tepe/dip hacimli kırılım → potansiyel W1
- **Momentum Onayı:** RSI/MACD divergence
- **Hacim:** İtki dalgalarında artan, düzeltmede azalan hacim

---

## 5. İşlem Setup’ları

### Setup 1: W3 Yakalama
| | Değer |
|---|-------|
| **Giriş** | W2 alt TF CHoCH veya %61.8 limit |
| **SL** | W0 (W1 başlangıcı) |
| **TP1** | W1 tepe noktası |
| **TP2** | W1 başlangıcı + W1×1.618 |

### Setup 2: W5 Yakalama
| | Değer |
|---|-------|
| **Giriş** | W4 %38.2 veya üçgen/flama kırılımı |
| **SL** | W1 tepe (W4, W1 altına inemez) |
| **TP** | W3 bitiş + W1 (W5=W1) veya 0.618×(W0→W3) extension |
