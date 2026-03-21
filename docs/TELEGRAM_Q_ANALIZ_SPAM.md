# Q-Analiz Telegram tekrarları ve AI metni

## Sorun

Aynı sembol / timeframe için **tespit metni ve skorlar** değişmeden kısa aralıklarda tekrar bildirim; `Güven (Radar) 2/10 ↔ 3/10` gibi küçük titreşimler eski throttle anahtarını değiştirip pencereyi deliyordu. Ayrıca **Ollama** her turda çağrılıyor, 🤖 paragrafı hem gereksiz üretiliyor hem de model **al/sat / hedef fiyat** uydurabiliyordu.

## Çözüm (kod)

1. **Dedup anahtarı** (`iqai-web` `notify::q_analysis_throttle_key`): sembol, TF, tespit, yön, tavsiye özeti, dip/tepe ve Smart Money **toplam skorları** — **güven ve erken uyarı skorları anahtarda yok**.
2. **Varsayılan aralık** `throttle_q_analysis_ms`: **300_000 ms (5 dk)** (`config.json` `notification.throttle_q_analysis_ms`).
3. **Q-Analiz daemon** (`iqai-cli`): Ollama yalnızca `Notifier::q_analysis_would_skip` **false** iken çağrılır (throttle ile aynı mantık).
4. **AI prompt** (`iqai-web` `ai.rs`): al/sat, hedef fiyat, dolar tutarı, “yatırım tavsiyesi” yasağı netleştirildi.

## Yapılandırma

- Daha seyrek: `throttle_q_analysis_ms` örn. `600000` (10 dk).
- Daha sık (önerilmez): `120000` (2 dk) — yine de aynı **semantik** tespitte tekrar sınırlı kalır.

## Test

```bash
cargo test -p iqai-web q_analysis_throttle_key
```
