# Q-Analiz ekran alanları – nasıl hesaplanıyor?

Bu doküman, Q-Analiz / Q-RADAR panelinde gördüğünüz alanların (Fiyat, YÖN, Tespit, Güven, Erken Uyarı, Tavsiye) kodda nasıl üretildiğini özetler. Hesaplama `crates/iqai-core/src/q_radar_analysis.rs` ve `signal.rs` (Q-RADAR) içinde yapılır.

---

## Web'de bu format

Evet. Aynı format web arayüzünde görüntülenebilir: Ana sayfa (grafik) → sağda **Q-ANALİZ** paneli. Sembol ve timeframe seçip grafik yüklediğinizde, Tespit doluysa **Fiyat** (anlık + % değişim), **YÖN** (▲/▼ LONG/SHORT), **Tespit**, **Güven**, **Erken Uyarı**, **Tavsiye** aynı sırayla gösterilir. Bildirimler (Telegram, WhatsApp vb.) de aynı alanlarla gider.

---

## 1. Fiyat (reference_price)

- **Kaynak:** Son mumun kapanış fiyatı (`candles.last().close`) veya RADAR sinyali varsa onun `reference_price` alanı.
- **Mantık:** `compute_q_radar_opportunity` içinde önce RADAR’ın referans fiyatına bakılır; yoksa son mum kapanışı kullanılır. Ekrandaki “anlık fiyat” bu değerdir.

---

## 2. YÖN (direction)

- **Değerler:** `"LONG"`, `"SHORT"` veya `"—"`.
- **Hesaplama:**
  - RADAR sinyali varsa: RADAR’ın `side` alanına göre — `SignalType::Buy` (ve ChochBuy, BosBuy) → LONG, `SignalType::Sell` → SHORT.
  - RADAR yoksa sadece dip/tepe analizi varsa: Dip dönüşü tespit edildiyse LONG, tepe dönüşü tespit edildiyse SHORT.
  - Hiçbiri yoksa: `"—"`.
- **RADAR’da yön:** `signal.rs` → `compute_q_radar`: Trend gücü (`trend_strength`) + son mum yönü. Yükseliş (trend > 0, bullish mum, close ≥ prev close) → Buy; düşüş (trend < 0, bearish mum, close ≤ prev close) → Sell. Aksi halde RADAR üretilmez.

---

## 3. Tespit (detection)

- **Değerler:** `"DİP BÖLGESİ (TEPKİ DİBİ)"`, `"TEPE BÖLGESİ (TEPKİ TEPESİ)"` veya `"—"`.
- **Hesaplama:**
  - RADAR varsa ve yön LONG ise: `"DİP BÖLGESİ (TEPKİ DİBİ)"`.
  - RADAR varsa ve yön SHORT ise: `"TEPE BÖLGESİ (TEPKİ TEPESİ)"`.
  - RADAR yok, sadece dip analizi dönüş tespit ettiyse: `"DİP BÖLGESİ (TEPKİ DİBİ)"`.
  - RADAR yok, sadece tepe analizi dönüş tespit ettiyse: `"TEPE BÖLGESİ (TEPKİ TEPESİ)"`.
  - Hiçbiri yoksa: `"—"`.
- **Dip/tepe analizi:** `reversal.rs` → `compute_reversal_analysis`. Pivot low/high, dip/tepe barından sonra fiyatın margin (ATR) üzerine/altına çıkması ve yükseliş/düşüş mumları ile “dipten dönüş” / “tepeden dönüş” tespit edilir.

---

## 4. Güven (confidence_score)

- **Aralık:** 0–10 (ekranda “x/10” ve çubuk).
- **Hesaplama:**
  - RADAR varsa: RADAR’ın `confidence` değeri (0–1) × 10 ile 0–10’a çevrilir: `conf_10 = (r.confidence * 10).min(10)`.
  - RADAR’ın confidence’ı (`signal.rs` → `compute_q_radar`):  
    `dir_score_norm * 0.5 + conf_norm * 0.3 + phase_score * 0.2` (0–1).  
    - `dir_score`: Yön skoru (trend gücü 0–100’den normalize).  
    - `conf_norm`: Sistem güveni (system_confidence, 50–90 bandından normalize).  
    - `phase_score`: Fibo zaman fazının erken bölgeye ne kadar yakın olduğu.
  - RADAR yok, sadece dip/tepe dönüşü varsa: `early * 0.4` ile 0–10’a indirgenir (erken uyarı gücünün bir kısmı güven olarak kullanılır).

---

## 5. Erken Uyarı (early_warning_score)

- **Aralık:** 0–10 (ekranda “DİP x/10” veya “TEPE x/10”).
- **Hesaplama:**
  - RADAR + LONG: Dip analizi varsa `dip.reversal_strength * 10` (max 10), yoksa RADAR confidence × 10.
  - RADAR + SHORT: Tepe analizi varsa `peak.decline_strength * 10` (max 10), yoksa RADAR confidence × 10.
  - RADAR yok, sadece dip/tepe: `reversal_strength * 10` veya `decline_strength * 10`.
- **reversal_strength / decline_strength:** `reversal.rs` içinde bounce/ATR oranı, hacim oranı ve yapı (higher low vb.) ile 0–1 arası skor.

---

## 6. Tavsiye (recommendation)

- **Değerler:** Örn. `"ZAYIF DİP – İzle"`, `"GÜÇLÜ DİP – İzle"`, `"DİP BÖLGESİ – İzle"`, `"ZAYIF TEPE – İzle"`, `"GÜÇLÜ TEPE – İzle"`, `"TEPE BÖLGESİ – İzle"`, `"—"`.
- **Hesaplama (RADAR + LONG):**
  - Güven ≥ 7 ve erken uyarı ≥ 7 → `"GÜÇLÜ DİP – İzle"`.
  - Güven ≥ 4 veya erken uyarı ≥ 5 → `"ZAYIF DİP – İzle"`.
  - Diğer → `"DİP BÖLGESİ – İzle"`.
- **RADAR + SHORT:** Aynı eşikler, “DİP” yerine “TEPE”.
- RADAR yok, sadece dip/tepe: `reversal_strength >= 0.7` → güçlü, değilse zayıf.

---

## Özet akış

1. **Q-RADAR** (`signal.rs`): Trend + son mum ile yön (Buy/Sell); Fibo erken fazında ve confidence ≥ 0.4 ise RADAR sinyali üretilir.
2. **Dip/tepe analizi** (`reversal.rs`): Pivot low/high, dipten/tepeden dönüş ve güç skorları.
3. **Birleştirme** (`q_radar_analysis.rs` → `build_detection_and_recommendation`): RADAR varsa Tespit/Güven/Erken Uyarı/Tavsiye/YÖN RADAR + dip/tepe ile; yoksa sadece dip/tepe ile doldurulur. Fiyat yukarıda anlatıldığı gibi seçilir.

Bu yapı hem web panelinde hem de bildirim mesajında (Telegram, WhatsApp, vb.) aynı alanlarla kullanılır; bildirim metni ekrandaki sıraya göre (Fiyat, YÖN, Tespit, Güven, Erken Uyarı, Tavsiye) formatlanır.
