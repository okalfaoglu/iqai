# Açık pozisyonların değerlendirme süreci

Q-Analiz fırsat kovalarken (yeni sinyaller taranırken), **zaten açık olan işlemler** her turda aynı döngü içinde değerlendirilir. Süreç özetle şöyle:

---

## 1. Tur akışı (full_tick)

Her turda sıra:

1. **Sinyal toplama** (CLI tarafında): Tüm sembol × timeframe için Q-RADAR, Q-Setup, Elliott sinyalleri üretilir → `signals` listesi.
2. **Zıt yön kapatma**: Her sinyal için, aynı sembolde zıt yönde açık pozisyon varsa önce o pozisyon(lar) piyasa fiyatından kapatılır.
3. **Yeni pozisyon açma**: Her sinyal için `process_signal` çağrılır. Kullanılan bakiye = **toplam bakiye − açık pozisyonların kullandığı margin** (madde 3 sermaye yönetimi).
4. **Açık pozisyonları değerlendirme**: `tick_positions` ile her açık pozisyon için anlık fiyat ve ilgili timeframe mumları verilir; TradeManager **evaluate** → aksiyon (FullClose, PartialClose, MoveSlToBreakeven, UpdateTrailingStop) döner.
5. **Aksiyonları uygulama**: FullClose → pozisyon kapatılır; PartialClose → kısmi kapatma; MoveSlToBreakeven / UpdateTrailingStop → SL güncellenir, DB’ye yazılır.

Yani hem **yeni fırsatlar** (sinyal → açılış) hem **açık işlemler** (fiyat + mum → SL/TP/kısmi/trailing) **aynı turda** işlenir.

---

## 2. Açık pozisyon değerlendirmesi (tick_positions + TradeManager.evaluate)

Her açık pozisyon için:

- **Girdi:** O anki fiyat (`current_prices[sembol]`), pozisyonun timeframe’ine ait mum listesi (`candles_map[sembol_tf]`).
- **Mantık:** `TradeManager::evaluate(position, current_price, candles)` — config’te `enable_trade_management` açıksa aşağıdaki sıra uygulanır.

### Long pozisyon (sırayla)

| Kontrol | Aksiyon |
|--------|--------|
| Fiyat ≤ current_sl | **FullClose** — Stop Loss |
| Fiyat ≥ initial_tp | **FullClose** — Take Profit |
| Kâr ≥ TP1_R (örn. 1R) ve tp1_done değilse | **PartialClose** — TP1 yüzdesi kapat |
| Kâr ≥ breakeven_R (örn. 1R) ve breakeven_done değilse | **MoveSlToBreakeven** — SL = giriş |
| Kâr ≥ TP2_R (örn. 2R) ve tp2_done değilse | **PartialClose** — TP2 yüzdesi kapat |
| Breakeven yapıldıysa + yeterli mum varsa | **UpdateTrailingStop** — Chandelier/ATR ile yeni SL (sadece lehine güncelleme) |

### Short pozisyon

Aynı mantık, yön ters: SL üstte, TP altta; kâr = entry − price; trailing stop Chandelier short formülü.

### Trailing stop (Chandelier)

- Long: `Highest(high, N) − ATR(N) × mult`
- Short: `Lowest(low, N) + ATR(N) × mult`  
SL sadece kâr yönünde güncellenir (long’da yeni SL > eski SL ve < fiyat).

---

## 3. Bu tick’te açılan pozisyonlar (look-ahead önlemi)

Aynı turda **yeni açılan** pozisyonlar, o turda TP/SL ile **değerlendirilmez** (`skip_keys` ile atlanır). Böylece aynı bar’da “açıldı ve hemen TP’ye de dokundu” sayılmaz (look-ahead bias önlenir). Bir sonraki turda bu pozisyonlar da normal şekilde evaluate edilir.

---

## 4. Özet

- **Fırsat tarafı:** Q-RADAR / Q-Setup / Elliott sinyalleri → zıt yön kapatma → kullanılabilir bakiye ile yeni pozisyon açma.
- **Açık işlem tarafı:** Her turda her açık pozisyon için anlık fiyat + TF mumları ile SL/TP/kısmi TP/breakeven/trailing stop kontrolü; aksiyonlar anında uygulanır (kapatma veya SL güncelleme).

Bu süreç, Q-Analiz fırsat kovalarken açılan işlemlerin de sürekli ve aynı mimari içinde değerlendirilmesini sağlar.
