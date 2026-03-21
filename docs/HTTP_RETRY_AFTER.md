# GET retry ve `Retry-After` (Binance)

`crates/iqai-binance/src/http_retry.rs` içindeki `send_get_retry`:

- 429 / 502 / 503 için en fazla **4** deneme.
- Yanıtta **`Retry-After: <saniye>`** (tam sayı) varsa bu süre kadar beklenir (üst sınır **120 sn**); yoksa üstel geri çekilme (max 5 sn) kullanılır.
- **POST** emir çağrıları bu yardımcıyı kullanmaz (idempotent değil).

Tam HTTP tarih biçimli `Retry-After` şu an ayrıştırılmaz; gerekirse sonraki iterasyon.
