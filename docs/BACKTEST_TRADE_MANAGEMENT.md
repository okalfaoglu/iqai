# Backtest ↔ canlı pozisyon yönetimi (TradeManager) uyumu

## Özet

| Bileşen | Rol |
|---------|-----|
| `trade_manager::TradeManager` | Canlı/dry: bar bazında `evaluate` → SL, tam TP, TP1/TP2 kısmi, breakeven, trailing. |
| `strategy_engine::run_strategy_plan_backtest` | Tek plan + tek seri: `entry_zone` ile giriş; **öncelik sırası** `TradeManager::evaluate` ile aynı (SL → tam TP → TP1 → breakeven → TP2 → trailing). R-tabanlı eşikler mum **kapanışı** (`close`) ile; SL tetiklemesi **low/high** ile. |
| `backtest::run_backtest` | Q-Setup odaklı genel backtest; çoğunlukla tek SL/TP (kısmi TP yok). |

## Bilinen farklar (T-05 kapsamı)

1. **Intrabar / tick:** Canlıda `evaluate` her tick’te çağrılır; backtestte bar başına en fazla bir yönetim adımı vardır. Aynı mumda fitil ile SL ve TP’ye dokunma gibi durumlar gerçek borsada sıra belirsizdir; strateji motoru SL’yi önce kontrol eder, R-hedefleri kapanış fiyatına göre hesaplar (canlıda `current_price` ≈ mum kapanışı senaryosu).

2. **TP2 / `remaining_pct`:** Kısmi kapama sonrası kalan miktar ve çoklu hedefler, canlı tarafta `trade_manager.apply_action` ile güncellenir; plan-backtest `EnginePosition` ile aynı matematiği kullanır — çok hızlı ardışık kısmi çıkışlar tek mumda tek adımla özetlenir.

3. **Trailing:** Her iki tarafta da Chandelier formülü aynı kaynak fonksiyonlardan (`chandelier_long` / `chandelier_short`); bar sınırında tek güncelleme, canlıda ise tick başına mümkün.

## T-05 (regresyon testleri)

`strategy_engine.rs` içinde referans simülasyon `run_plan_backtest_via_trade_manager` (`TradeManager` + `evaluate` + bar uçlarında SL) ile `run_strategy_plan_backtest` karşılaştırılır:

```bash
cargo test -p iqai-core strategy_plan_backtest
```

Senaryolar: long SL, long tam TP, long TP1+TP, short SL.

## Ne zaman bu dokümana bakılır?

Yeni bir kural eklerken (ör. TP3 veya farklı trailing) **hem** `trade_manager.rs` **hem** `strategy_engine.rs` içinde simetrik güncelleme yapın; mümkünse küçük bir birim testi veya sabit veri setiyle iki yolu karşılaştırın.
