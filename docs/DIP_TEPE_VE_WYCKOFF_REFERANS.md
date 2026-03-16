# Dip ve Tepe Tespiti – Genel Yöntemler ve Wyckoff Referansı

Bu doküman, piyasada dip/tepe tespiti için kullanılan yaygın ve profesyonel yöntemleri özetler. IQAI’deki uygulama için bkz. `DIP_TEPE_YORUM_VE_KARSILASTIRMA.md`, `DIP_TESPITI_KATMANLAR.md`, `Q_ANALIZ_ALANLARI.md`.

---

## 1. Temel Yöntemler

### 1.1 Destek ve Direnç
- **Destek:** Fiyatın tekrar tekrar düşüp tutunduğu bölge → potansiyel **dip**
- **Direnç:** Fiyatın tekrar tekrar yükselip döndüğü bölge → potansiyel **tepe**
- Aynı seviye ne kadar çok test edilirse o kadar güçlü; büyük zaman dilimleri daha güvenilir.

### 1.2 RSI – Aşırı Alım / Aşırı Satım
- RSI &lt; 30 → aşırı satım → dip ihtimali  
- RSI &gt; 70 → aşırı alım → tepe ihtimali  
- **RSI Divergence (uyumsuzluk):**
  - **Boğa:** Fiyat daha düşük dip, RSI daha yüksek dip → satış gücü zayıflıyor → dip ihtimali
  - **Ayı:** Fiyat daha yüksek tepe, RSI daha düşük tepe → alım gücü zayıflıyor → tepe ihtimali

### 1.3 Mum Formasyonları
- **Dipte:** Hammer, Morning Star, Bullish Engulfing  
- **Tepede:** Shooting Star, Evening Star, Bearish Engulfing  

### 1.4 Hareketli Ortalama Tepkileri
- 50 / 100 / 200 MA: Fiyat destekte tepki → dip; dirençte tepki → tepe.

### 1.5 Hacim
- **Diplerde:** Satış hacmi zirve → ardından alıcılar (kapitülasyon).  
- **Tepelerde:** Yüksek alım hacmi → ardından satış.

### 1.6 Trend Çizgisi Kırılımı
- Yükselen trend kırılır → tepe ihtimali  
- Düşen trend kırılır → dip ihtimali  

**Güçlü kombinasyon:** Destek/direnç + RSI uyumsuzluğu + mum formasyonu + hacim artışı.

---

## 2. İleri Yöntemler

### 2.1 RSI Uyumsuzluğu (Divergence)
- Boğa uyumsuzluğu: Fiyat LL, RSI HL → dip sinyali.  
- Ayı uyumsuzluğu: Fiyat HH, RSI LH → tepe sinyali.

### 2.2 Likidite Avı (Stop Hunt / Liquidity Sweep)
- Fiyat önemli destek altına kısa süre iner → stopları patlatır → hızlıca yukarı döner → gerçek dip bölgesi.

### 2.3 Hacim Patlaması
- Diplerde: Çok büyük satış hacmi, ardından güçlü yeşil mum → kapitülasyon; büyük para toplar, fiyat döner.

### 2.4 Trend Kırılımı (Break of Structure – BOS)
- Düşüş: Lower High, Lower Low… Son tepe kırılırsa trend değişir → dip oluşumunun ilk sinyali.

### 2.5 Order Block
- Büyük yükselişten önceki son kırmızı mum / büyük düşüşten önceki son yeşil mum; fiyat bu bölgelere geri gelip tepki verir.

### 2.6 Fair Value Gap (FVG)
- Hızlı hareketlerde 3 mum arasında boşluk; fiyat boşluğu doldurmak ister → destek/direnç.

### 2.7 Güçlü dip kombinasyonu (5 sinyal)
1. Destek bölgesi  
2. RSI uyumsuzluğu  
3. Likidite sweep (stop avı)  
4. Hacim patlaması  
5. Break of Structure  

---

## 3. Wyckoff Birikim Modeli (Accumulation)

Richard D. Wyckoff’a göre büyük oyuncular önce yavaşça mal toplar; dip süreci aşamalıdır.

| Aşama | Kısaltma | Açıklama |
|-------|----------|----------|
| Ön destek | **PS** | Sert düşüşten sonra hacim artar; ilk alımlar, düşüş hızı azalır. |
| Satış zirvesi | **SC** | Panik satış, dev hacim, uzun kırmızı mumlar; market maker agresif alır → büyük dip bölgesi. |
| Otomatik tepki | **AR** | SC sonrası hızlı yükseliş (satış bitti, alıcılar iter). |
| İkinci test | **ST** | Fiyat tekrar dip bölgesini test eder; hacim düşük, satış zayıf → satıcılar bitiyor. |
| Spring | **Spring** | Fiyat eski dip altına kısa süre iner (fake kırılım), stopları toplar, sonra hızla yukarı döner = **likidite sweep**. |
| Markup | **Markup** | Mal toplandı, satış kalmadı, yükseliş başlar. |

**Sıra:** PS → SC → AR → ST → Spring → Markup.

---

## 4. Wyckoff Tuzakları (Market Maker)

- **Spring tuzağı:** Destek kırılır gibi görünür → shortlar / stoplar tetiklenir → fiyat sert yukarı döner.  
- **Upthrust tuzağı:** Direnç kırılır gibi → longlar açılır → fiyat sert aşağı döner.  
- **Fake Break of Structure:** Trend kırıldı sanılır; aslında likidite toplama.  
- **Range manipülasyonu:** Uzun süre yatay; sabırsızları elemek, stop toplamak, mal biriktirmek.

---

## 5. Wyckoff ile Profesyonel Dip Stratejisi

1. **Accumulation alanı:** Uzun yatay, hacim düşüşü, satış zayıflaması.  
2. **Selling Climax:** Dev hacim, uzun kırmızı mum, panik.  
3. **Spring bekle:** Dip altı fake kırılım (stop avı).  
4. **Break of Structure:** Spring sonrası son tepe kırılsın.  
5. **Retestte giriş:** BOS sonrası retest = güvenli giriş; stop spring altı.

**Güçlü formül (5 sinyal):** Accumulation range + Selling climax + Spring + BOS + Retest.

---

## 6. %80 Doğrulukta Dip Checklisti

- [ ] Range / accumulation var mı?  
- [ ] Panik satış (büyük kırmızı mum, dev hacim) oldu mu?  
- [ ] Spring (dip altı kısa kırılım, sonra dönüş) var mı?  
- [ ] RSI divergence (fiyat LL, RSI HL) var mı?  
- [ ] Trend kırıldı mı (BOS)?  
- [ ] Giriş: Spring → BOS → Retest sonrası; stop spring altı.

---

## 7. TradingView’de Wyckoff / Dip Yardımcıları

- **Wyckoff Accumulation / Wyckoff Range Detector:** Accumulation, distribution, spring bölgeleri.  
- **Smart Money Concepts:** Likidite, structure break, order block.  
- **Volume Profile:** Dip bölgelerinde yüksek hacimli node = olası akıllı para girişi.

---

## 8. IQAI ile İlişki

| Kavram | IQAI’de karşılığı |
|--------|-------------------|
| Pivot dip/tepe | `reversal.rs`: pivot low/high, dipten/tepeden dönüş |
| Dönüş gücü | `reversal_strength` / `decline_strength`; Erken Uyarı, Tavsiye |
| Confluence | `dip_confluence.rs`: MTF destek, MSS, Elliott+Fib, divergence |
| RADAR | Q-RADAR; Tespit (DİP/TEPE BÖLGESİ), Güven, Tavsiye |
| Elliott | `elliott_detector`; Q-Analiz daemon tespit sonrası Elliott özeti + AI yorumu |

Wyckoff Spring, BOS, RSI divergence, hacim patlaması gibi kriterlerin kodda nasıl genişletilebileceği için `DIP_TESPITI_KATMANLAR.md` ve `DIP_TEPE_YORUM_VE_KARSILASTIRMA.md` dokümanlarına bakılabilir.
