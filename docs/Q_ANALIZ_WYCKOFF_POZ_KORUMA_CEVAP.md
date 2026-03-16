# Q-ANALİZ: Wyckoff Kullanımı, Poz Koruma, Q-RADAR→Q-Setup+Elliott Hedefleri

Bu doküman üç konuyu yanıtlar: (1) DIP_TEPE_VE_WYCKOFF_REFERANS.md’deki bilgilerin dip/tepe tespitinde kullanılıp kullanılmadığı, (2) Poz Koruma’nın ne olduğu ve nasıl hesaplandığı, (3) Q-RADAR tespitine Q-Setup skor/giriş/stop/TP eklenmesi ve Elliott Wave hedeflerinin kullanılması. Ayrıca paylaşılan Pine Script (Wyckoff Accumulation Distribution) ile mevcut kodun farkı kısaca özetlenir.

---

## 1. Dokümandaki Bilgiler Dip/Tepe Tespitinde Kullanılıyor mu?

**Kısmen kullanıyoruz.** Referans dokümandaki yöntemlerin bir kısmı kodda var; tam Wyckoff faz etiketleri (SC, AR, ST, BC, DAR, DST) ve RSI-bazlı “Accumulation/Distribution” kutusu **yok**.

### 1.1 Şu an kullandıklarımız

| Referans doküman / Wyckoff | Kodda karşılığı |
|----------------------------|------------------|
| Pivot dip/tepe (destek/direnç) | `reversal.rs`: `pivot_low` / `pivot_high`, `get_dip_price_and_index`, `get_peak_price_and_index` |
| Dipten/tepeden dönüş (fiyat + margin) | `is_reversal_from_dip`, `is_reversal_from_peak` (dip+0.2×ATR üstü / tepe−0.2×ATR altı + mum yönü) |
| Dönüş gücü (bounce/ATR, hacim) | `reversal_strength`, `decline_strength` (reversal.rs) |
| **Wyckoff Spring** (dip altı kırılım, sonra dönüş) | `reversal.rs`: `detect_spring` – dip barından sonra low &lt; dip, 4 bar içinde close &gt; dip |
| **Wyckoff Upthrust** (tepe üstü kırılım, sonra dönüş) | `reversal.rs`: `detect_upthrust` – tepe barından sonra high &gt; peak, 4 bar içinde close &lt; peak |
| RSI uyumsuzluğu (LL+RSI HL / HH+RSI LH) | `dip_confluence.rs`: `bullish_divergence`, `bearish_divergence` (confluence katmanı) |
| BOS (yapı kırılımı) | `dip_confluence.rs`: `bos_ok` (son iki pivot high/low kırılımı) |
| MTF destek, Elliott/Fib bölgesi, absorption | `dip_confluence.rs`: 8 katmanlı confluence |

Yani **dip/tepe noktası** pivot ile bulunuyor; **Spring/Upthrust** Wyckoff referansıyla kullanılıyor; **RSI divergence, BOS, MTF, Elliott/Fib** confluence içinde yer alıyor.

### 1.2 Henüz kullanılmayanlar

- Wyckoff **faz etiketleri**: SC (Selling Climax), AR (Automatic Rally), ST (Secondary Test), BC (Buying Climax), DAR, DST. Referans dokandaki “PS → SC → AR → ST → Spring → Markup” sırası ve bu isimlerle işaretleme **yok**.
- RSI ile **yatay/boğa/ayı bölgesi** ve “Accumulation / Distribution” **kutusu** (paylaşılan Pine Script’teki gibi) yok.
- Pine Script’teki **RSI 50±sensitivity** ile SC/AR/ST/BC/DAR/DST filtrelemesi bizde yok; biz sadece pivot + Spring/Upthrust + confluence kullanıyoruz.

**Özet:** Dip/tepe bulmada **pivot, dipten/tepeden dönüş, Spring, Upthrust, RSI divergence, BOS, MTF, Elliott/Fib** kullanılıyor. Wyckoff dokümanındaki “güçlü kombinasyon” ve Spring/BOS mantığı kısmen uygulanmış; tam Wyckoff faz etiketleri ve RSI-bazlı kutu mantığı eklenmemiş.

---

## 2. Poz Koruma Nedir? Nasıl Hesaplanır?

**Poz Koruma**, açık pozisyon kar ederken “koruma moduna geç” uyarısıdır: stop’u girişe yaklaştırma (breakeven) veya kârı kilitleme önerisi. Sinyal **ProtectSignal** ile temsil edilir.

### 2.1 Ne zaman üretilir?

- **Giriş:** `entry`, `stop_loss`, mevcut mum verisi (buffer, chart_tf, symbol).
- **risk_r** = |entry − stop_loss| (1R).
- **profit_r** = (Long: current_price − entry, Short: entry − current_price) / risk_r.
- **Koşul:** `profit_r ≥ q_protect_min_r` (varsayılan **1.5R**). Yani en az 1.5R kârda olmalı; yoksa Poz Koruma sinyali **üretilmez**.

### 2.2 Nasıl hesaplanır?

1. **Reason (sebep):**
   - Fibo-zaman fazı `phase ≥ q_late_phase` (varsayılan 0.7) ise → **"LATE_PHASE"** (geç fazda, çıkış alanına girildi).
   - Değilse → **"TRAILING_PROFIT"** (kar koruma / trailing).

2. **Kilitlenecek kâr (locked_r):**  
   `locked_r = min(q_protect_lock_r, profit_r)` (varsayılan **q_protect_lock_r = 0.5**).  
   Yani en fazla 0.5R’lik kâr “kilitlenir” (stop bu seviyeye taşınabilir).

3. **Tetik fiyatı (trigger_price):**
   - **Long:** trigger_price = entry + locked_r × risk_r  
   - **Short:** trigger_price = entry − locked_r × risk_r  

Bu fiyat, stop’u taşıma önerisi için referans seviyedir (SL’i breakeven veya bu seviyeye çekmek anlamında).

### 2.3 Config parametreleri

| Parametre | Varsayılan | Açıklama |
|-----------|------------|----------|
| q_protect_min_r | 1.5 | Poz Koruma için minimum kâr (R cinsinden). |
| q_protect_lock_r | 0.5 | Kilitlenecek minimum kâr (R). |

### 2.4 Çıktı (ProtectSignal)

- symbol, timeframe, reason ("LATE_PHASE" | "TRAILING_PROFIT"), trigger_price, entry_price, locked_r.

**Kaynak:** `iqai-core/src/signal.rs` → `compute_protect_signal`.

---

## 3. Q-RADAR Tespitine Q-Setup (Skor, Giriş, Stop, TP) ve Elliott Hedefleri Ekleme

İstenen davranış: **Q-RADAR bir tespit verdiğinde** (DİP/TEPE BÖLGESİ), bu tespit için:
- Q-Setup tarzı **skor, giriş, stop, TP** üretilsin,
- **Elliott Wave formasyon hedefleri** (W5 hedefleri, Zigzag C / Triangle E giriş-hedef) TP veya hedef seviyesi olarak kullanılsın.

### 3.1 Mevcut durum

- **Q-RADAR:** Sadece `QRadarSignal` (yön, confidence, reference_price, suggested_sl, expected_window_bars). Skor/giriş/stop/TP **yok**.
- **Q-Setup:** Ayrı hesaplanıyor (`compute_q_setup`); isteğe bağlı olarak RADAR referans alınabiliyor (radar_early bayrağı). Giriş/SL/TP pivot+ATR ve yapı TP’den geliyor; **Elliott hedefleri Q-Setup TP’ye katılmıyor**.
- **Elliott:** `compute_elliott` → `ElliottDetectorResult`: `w5_targets: Option<(f64, f64, f64)>`, `corr_setup: Option<CorrSetup>` (Zigzag C / Triangle E entry, stop, target). Bu hedefler şu an Q-Setup veya Q-RADAR çıktısına **entegre değil**.

### 3.2 Nasıl eklenebilir? (Tasarım özeti)

1. **Q-RADAR tespiti olduğunda Q-Setup hesapla**  
   Zaten yapılabiliyor: `compute_q_radar_opportunity` sonrası aynı buffer/TF/symbol ile `compute_q_setup(buffer, chart_tf, symbol, opportunity.radar.as_ref())` çağrılır. Eksik olan: bu Q-Setup’ın **Q-RADAR çıktısıyla tek yapıda sunulması** veya daemon/API’de “RADAR + Setup” birlikte dönmesi.

2. **Elliott hedeflerini TP’ye katma**  
   - Aynı TF’de `compute_elliott(candles, config, false)` çağrılır.
   - **Impulse:** `result.w5_targets` (W1=W5, %61.8×(0–3), W4 inv 123.6%) – üç hedef fiyat.
   - **Düzeltme:** `result.corr_setup` (Zigzag C veya Triangle E) → entry, stop, take_profit/target.
   - Q-Setup’ın mevcut TP’si: `max(minimum_RR_TP, structure_based_tp)`. **Elliott hedefi** şu şekilde kullanılabilir:
     - Örneğin TP_final = Q-Setup TP ile Elliott hedeflerinden (w5_targets veya corr_setup.target) uyumlu olanı seçmek (aynı yönde ve makul mesafede ise),
     - veya Elliott hedefini “ikinci hedef” / “extended TP” olarak ayrı alanla sunmak.

3. **Kod yerleri**  
   - **q_radar_analysis.rs:** `compute_q_radar_opportunity` sonrası isteğe bağlı `compute_q_setup` + `compute_elliott` çağrısı; çıktı yapısına `q_setup: Option<QSetup>`, `elliott_targets: Option<ElliottTargets>` gibi alanlar eklenebilir.
   - **signal.rs:** `compute_q_setup` içinde TP hesaplamasından sonra, Elliott sonucu parametre olarak verilip TP’nin Elliott hedefleriyle birleştirildiği bir branch eklenebilir.
   - **Elliott hedef tipi:** `w5_targets` ve `corr_setup` (entry, stop, take_profit) zaten var; bunları ortak bir “ElliottTargets” veya “ElliottLevels” yapısında toplayıp Q-Setup/Q-RADAR çıktısına eklemek yeterli.

Bu adımlar yapıldığında: Q-RADAR tespiti → aynı anda Q-Setup (skor, giriş, stop, TP) + Elliott formasyon hedefleri tek akışta üretilmiş ve raporlanabilir olur.

---

## 4. Pine Script (Wyckoff Accumulation Distribution) ile Fark

Paylaşılan indikatör:

- **RSI 50±sensitivity** ile yatay / boğa / ayı bölgesi tanımlıyor; “Accumulation” / “Distribution” **kutusu** çiziyor.
- **Pivot + RSI at pivot** ile Wyckoff faz etiketleri: **SC** (pivotLow + RSI&lt;rsiLow, spring değil), **AR** (SC sonrası pivotHigh), **ST** (ilk pivotLow + RSI&lt;rsiLow), **BC** (pivotHigh + RSI&gt;rsiHigh), **DAR**, **DST**.

Bizde:

- **Pivot + dipten/tepeden dönüş + Spring/Upthrust** var; **RSI at pivot** ile SC/AR/ST/BC/DAR/DST **etiketlemesi yok**.
- Confluence’ta **RSI divergence** (LL+RSI HL / HH+RSI LH) ve **spring_ok / upthrust_ok** kullanılıyor; yani Wyckoff fikri var, fakat Pine’daki RSI-bazlı faz kutusu ve etiket seti farklı.

İstenirse ileride: pivot + RSI(pivot) eşikleri ile SC/AR/ST/BC/DAR/DST bayrakları eklenebilir; bu, referans dokümandaki “PS→SC→AR→ST→Spring→Markup” ve dağıtım fazlarıyla daha yakın hizalanır.

---

## 5. Özet

| Soru | Cevap |
|------|--------|
| Dokümandaki bilgiler dip/tepe’de kullanılıyor mu? | **Kısmen.** Pivot, dipten/tepeden dönüş, Spring, Upthrust, RSI divergence, BOS, MTF, Elliott/Fib confluence’ta kullanılıyor. Wyckoff faz etiketleri (SC/AR/ST/BC/DAR/DST) ve RSI-bazlı kutu yok. |
| Poz Koruma nedir? | Kar içindeyken (en az 1.5R) “koruma moduna geç” uyarısı; trigger_price = entry ± locked_r×risk (locked_r ≤ 0.5R). |
| Poz Koruma nasıl hesaplanır? | profit_r ≥ q_protect_min_r; reason = LATE_PHASE veya TRAILING_PROFIT; locked_r = min(q_protect_lock_r, profit_r); trigger_price = entry ± locked_r×risk_r. |
| Q-RADAR’a skor, giriş, stop, TP + Elliott hedefleri? | Tasarım: Q-RADAR tespiti olduğunda `compute_q_setup` + `compute_elliott` çağrılıp TP’yi Elliott w5_targets / corr_setup hedefleriyle birleştirmek; çıktı yapısına q_setup ve elliott_targets alanları eklenebilir. |

Bu doküman, Wyckoff referansı kullanımı, Poz Koruma formülü ve Q-RADAR→Q-Setup+Elliott entegrasyonu için tek referans olarak kullanılabilir.
