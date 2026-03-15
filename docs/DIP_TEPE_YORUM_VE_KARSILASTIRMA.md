# Dip/tepe tespiti – IQAI yapısı ve “tepe → düzeltme → dip” mantığıyla karşılaştırma

Bu doküman, IQAI’de dip/tepenin **nasıl bulunduğunu** ve “tepede formasyon → düzeltme hedefi → o seviyede dip izleme → dipten dönüş sinyal/formasyon” tarzı bir yapıyla **nasıl yorumlanabileceğini** özetler.

---

## 1. Web’de bu formatı görmek

Evet, mümkün. Aynı panel (Fiyat, YÖN, Tespit, Güven, Erken Uyarı, Tavsiye) **web’de** gösteriliyor:

- **Nerede:** http://localhost:8080 → grafik sayfası → sağdaki **Q-ANALİZ** paneli.
- Sembol ve timeframe seçip veri geldiğinde, Tespit doluysa ekran görüntüsündekine benzer şekilde:
  - **Fiyat:** Anlık fiyat ve görünen mum aralığına göre % değişim (▲/▼ +%X.XX).
  - **YÖN:** ▲ LONG veya ▼ SHORT.
  - **Tespit:** DİP BÖLGESİ (TEPKİ DİBİ) / TEPE BÖLGESİ (TEPKİ TEPESİ).
  - **Güven:** x/10.
  - **Erken Uyarı:** DİP x/10 veya TEPE x/10.
  - **Tavsiye:** ZAYIF DİP – İzle / GÜÇLÜ DİP – İzle vb.

Detaylı alan hesapları için: `docs/Q_ANALIZ_ALANLARI.md`.

---

## 2. IQAI’de dip nasıl bulunuyor?

Bizim yapı **yerel (mevcut mum seti içinde)** dip/tepe ve **dipten/tepeden dönüş** odaklı; “uzun vadeli tepe 4832, dip 1930” gibi makro seviye takibi yok.

### 2.1 Tepe / dip fiyatı

- **Dip fiyatı:** Son **pivot low** (swing low) – `reversal.rs` içinde `pivot_low` ile, sabit bir pencere (örn. 5 bar sol, merkez, 5 bar sağ) kullanılarak bulunur.
- **Tepe fiyatı:** Son **pivot high** (swing high) – aynı mantıkla `pivot_high`.

Yani tepe/dip, “şu ana kadarki mumların yapısından” çıkan **yerel** tepe/dip; geçmişteki büyük cycle’lar (4832 tepe, 1930 dip) tek başına bu modülde takip edilmez.

### 2.2 Dipten dönüş / tepeden dönüş

- **Dipten dönüş:** Dip barı oluştuktan sonra fiyat, dip + (ATR × margin) üzerine çıkmış ve son mum(lar) yükseliş yönünde (bullish, close ≥ prev close). Bu koşul sağlanırsa “dipten dönüş tespit edildi” denir.
- **Tepeden dönüş:** Tepe barından sonra fiyat tepe altına (ATR margin ile) inmiş ve düşüş mumları; “tepeden dönüş” tespit edilir.

### 2.3 Dönüş gücü (Erken Uyarı ile ilişkisi)

- **Dipten dönüş gücü (`reversal_strength`):** Bounce mesafesi (ATR cinsinden), hacim oranı (son mum / ortalama hacim) ve yapı (higher low vb.) ile 0–1 arası skor. Ekranda “DİP 8/10” gibi gösterilen **Erken Uyarı**, bu gücün 10 ile çarpılmış hali (veya RADAR confidence) ile doldurulur.
- **Tepeden dönüş gücü (`decline_strength`):** Aynı mantık, tepe–son kapanış farkı ve düşüş yapısı ile.

Yani “dip bölgesine geldikten sonra farklı bilgilerle dipten dönüşü bulma” kısmı bizde: **pivot dip + ATR margin + mum yönü + bounce/hacim/yapı skoru** ile yapılıyor.

### 2.4 Q-RADAR ve Tespit / Tavsiye

- **Q-RADAR** (`signal.rs`): Trend gücü + son mum yönü ile **erken** yön (LONG/SHORT) verir; Fibo zaman fazı “erken bölge”de (örn. 0.1–0.3) ve confidence yeterliyse RADAR sinyali üretilir.
- **Tespit:** RADAR LONG ise “DİP BÖLGESİ (TEPKİ DİBİ)”, SHORT ise “TEPE BÖLGESİ (TEPKİ TEPESİ)”. RADAR yoksa sadece dip/tepe analizi ile de aynı etiketler üretilebilir (dipten/tepeden dönüş tespit edildiyse).
- **Tavsiye:** Güven ve Erken Uyarı skorlarına göre “ZAYIF DİP – İzle”, “GÜÇLÜ DİP – İzle” vb. Ayrıca **confluence** (MTF destek, yapı kırılımı, Elliott+Fib bölgesi, divergence) ile bu skorlar artırılabiliyor (`dip_confluence.rs` → `q_radar_analysis.rs`).

Özet: **Dip** = yerel pivot low + dipten dönüş (fiyat dip+margin üstünde, yükseliş mumları) + dönüş gücü; **tepe** = yerel pivot high + tepeden dönüş + düşüş gücü. “O değere gelince (örn. 1930) farklı indikatörlerle dip bulma” bizde bu yerel yapı + RADAR + confluence ile yapılıyor; “1930’a kadar düşüş, 1930’dan beri dip” gibi makro seviye ise şu an **tek bir sayı olarak** takip edilmiyor.

---

## 3. “Tepe → düzeltme → dip” mantığıyla karşılaştırma

Bahsettiğiniz mantık kabaca:

1. **Tepe tespiti:** Örn. 4832’de tepe (formasyon, sinyal).
2. **Düzeltme:** Tepeden düzeltme hedefi (formasyon/sinyala göre).
3. **Dip seviyesi izleme:** O hedefe (örn. 1930) gelince “dip bölgesi” olarak izleme.
4. **O seviyede dip bulma:** Farklı indikatörler ve bilgilerle dip tespiti.
5. **Dipten dönüş:** Sinyal ve formasyon arama.

IQAI tarafında:

| Adım | Sizin tarif | IQAI’de karşılık |
|------|-------------|-------------------|
| Tepe tespiti | Tepe formasyonu, sinyal (4832) | **Yerel** tepe: pivot high + tepeden dönüş. Elliott/impulse modülleri tepe formasyonu üretebilir ama Q-Analiz “4832 tepe” gibi tek seviye takibi yapmıyor. |
| Düzeltme hedefi | Formasyona göre düzeltme | Elliott Fib (W2/W4, Zigzag C) retrace/extension seviyeleri var; “düzeltme hedefi” olarak Q-Analiz dip tespitiyle **doğrudan** bağlı değil. |
| Dip seviyesi izleme | 1930’a gelince dip izleme | Yok. Biz “şu anki mum setinde pivot dip nerede?” diye bakıyoruz; “1930 seviyesine indi mi?” takibi yok. |
| O seviyede dip bulma | İndikatörlerle dip | **Var:** Pivot dip + dipten dönüş (ATR margin, mum yönü) + dönüş gücü (bounce, hacim, yapı) + isteğe bağlı confluence (MTF, MSS, Elliott+Fib, divergence). |
| Dipten dönüş sinyal/formasyon | Sinyal, formasyon | **Var:** Q-Setup (CHoCH/BOS, yapı), Elliott W3/W5 setup’ları; robot bu sinyalleri kullanıyor. Q-Analiz paneli “ZAYIF DİP – İzle” / “GÜÇLÜ DİP – İzle” ile dipten dönüş gücünü özetliyor. |

Yani **aynı şekilde yorumlayabileceğiniz kısım:** “Dipte miyiz?” sorusuna cevap (DİP BÖLGESİ), “ne kadar güçlü?” (Güven, Erken Uyarı), “ne yapalım?” (Tavsiye) ve dipten dönüş sinyal/formasyon (Q-Setup, Elliott). **Fark:** IQAI şu an “tepe 4832, düzeltme 1930” gibi **makro seviye takibi** ve “1930’a gelince özel dip modu” yapmıyor; her zaman **mevcut veri penceresindeki** yerel pivot dip/tepe ve RADAR/confluence ile çalışıyor. İsterseniz ileride “Elliott/impulse tepe + Fib düzeltme hedefi → o fiyat bandında dip bölgesi aç” gibi bir katman eklenebilir; bu `DIP_TESPITI_KATMANLAR.md` ve confluence önerileriyle uyumlu olur.
