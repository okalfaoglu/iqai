# Config: `config.json` ve DB (`app_kv`) override

## `config.json`

- **`notification`**: kanal kimlikleri + **throttle** (ms):
  - `throttle_q_setup_ms`, `throttle_q_analysis_ms`, `throttle_q_radar_ms`, `throttle_protect_ms`
- **`smart_money`**: motor / Q-RADAR ile ilgili ayarlar; örnek:
  - `candlestick_noise_atr_period`, `candlestick_noise_min_range_atr_ratio`
  - `q_confluence_boost_per_layer`, `q_confluence_boost_cap`

`AppConfig::load()` önce `config.json` dosyasını okur, ardından (dosya varsa) trade veritabanındaki `app_kv` satırlarını **üstüne yazar**.

## SQLite: `app_kv` tablosu

`trading.db_path` (varsayılan `data/trades.db`) içinde:

```sql
CREATE TABLE IF NOT EXISTS app_kv (
  key         TEXT PRIMARY KEY NOT NULL,
  value       TEXT NOT NULL,
  updated_at  INTEGER NOT NULL
);
```

- **`key`**: nokta ile ayrılmış yol, `config.json` yapısıyla aynı (örn. `notification.throttle_q_setup_ms`).
- **`value`**: geçerli **JSON** (sayı/string/bool) veya düz metin.

### Örnek

```sql
INSERT INTO app_kv (key, value, updated_at) VALUES
  ('notification.throttle_q_radar_ms', '20000', strftime('%s','now') * 1000),
  ('smart_money.candlestick_noise_min_range_atr_ratio', '0.2', strftime('%s','now') * 1000);
```

Veya Rust API: `TradeDb::upsert_app_kv(key, value, updated_at_ms)`.

## Benzer bildirim çok geliyorsa

1. **Throttle sürelerini** artırın (`throttle_q_analysis_ms` vb., örn. 60_000–120_000 ms).
2. Dedup anahtarı artık **anlık fiyata** bağlı değil (küçük tick hareketleri spam tetiklemez); yine de çok sık geliyorsa süreyi yükseltin.

## Ortam değişkenleri

- **`IQAI_CONFIG`**: `config.json` yolu.
- **`IQAI_TRADING_DB`**: override okunacak DB dosyası (yoksa `config.json` içindeki `trading.db_path`, o da yoksa `data/trades.db`).

## Kod API

- `AppConfig::load()` — JSON + DB birleşik.
- `AppConfig::load_file_only()` — sadece dosya (test / eski davranış).
