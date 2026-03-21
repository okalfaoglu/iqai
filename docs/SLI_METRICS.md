# SLI / Prometheus metrikleri (TFAI-O06)

IQAI, operasyonel görünürlük için **Prometheus exposition formatında** (metin) bir uç nokta sunar.

## Scrape

- **URL:** `GET http://<host>:8080/metrics/prometheus` (`iqai-web`)
- **Content-Type:** `text/plain; version=0.0.4; charset=utf-8`
- **Veritabanı:** `IQAI_TRADING_DB` ortam değişkeni veya `config.json` → `trading.db_path`; ikisi de yoksa `data/trades.db`. DB dosyası açılamazsa yanıtta yalnızca `iqai_info` ve `iqai_db_reachable 0` döner (HTTP 200).

HTML metrik paneli **`/metrics`** ile karışmaması için Prometheus yolu **`/metrics/prometheus`** olarak ayrıldı.

## Sabit gauge’lar

| Metrik | Tip | Anlam |
|--------|-----|--------|
| `iqai_info{version="..."}` | gauge | Çalışan `iqai-core` crate sürümü (etiket). |
| `iqai_db_reachable` | gauge | Trade DB açılabildi mi (1/0). |
| `iqai_open_positions{mode="..."}` | gauge | `positions.status = 'open'` sayısı, mod bazında. |
| `iqai_analysis_snapshot_oldest_age_seconds` | gauge | `analysis_snapshots` en eski `updated_at` yaşı (saniye). |
| `iqai_analysis_snapshot_newest_age_seconds` | gauge | En yeni snapshot satırının yaşı (saniye). |

## Sayaçlar (`sli_counters` tablosu)

Canlı modda (`TradingMode::live` → `sends_real_orders()`) `iqai-cli` `auto_trader`, gerçek emir denemelerinde SQLite içindeki `sli_counters` tablosunu günceller; exporter bu anahtarları **counter** satırları olarak yazar.

| Anahtar | Ne zaman artar |
|---------|----------------|
| `exec_order_open_attempt_total` | Açılış emri gönderilmeden hemen önce |
| `exec_order_open_success_total` | Açılış emri başarılı |
| `exec_order_open_failure_total` | Açılış emri hata |
| `exec_order_close_attempt_total` | Tam kapanış emri denemesi |
| `exec_order_close_success_total` / `exec_order_close_failure_total` | Sonuç |
| `exec_order_partial_close_attempt_total` | Kısmi kapanış denemesi |
| `exec_order_partial_close_success_total` / `exec_order_partial_close_failure_total` | Sonuç |

DRY/PAPER modlarında bu sayaçlar **artmaz** (yalnızca canlı borsa emirleri).

## TFAI-Q04 — normalize borsa hataları (`sli_counters` + bellek)

| Metrik | Tip | Anlam |
|--------|-----|--------|
| `iqai_exchange_normalized_errors_total{exchange,category,tier}` | counter | `classify_binance_json` çağrı başına (Binance JSON hata cevabı). `exchange`: `binance_futures`, `binance_spot`, `other`. |

**Akış:** Sınıflandırma sırasında sayaçlar önce **bellekte** güncellenir; her Prometheus scrape’inde (`render_prometheus_sli`) bu birikim `sli_counters` içine **`q04_norm:v1:{exchange}:{category}:{tier}`** anahtarlarıyla eklenir (SQLite `ON CONFLICT` ile toplam). Böylece süreç açıkken değerler kalıcıdır; yalnızca scrape ile flush edilmemiş son dilim süreç çökerse kaybolabilir.

Örnek uyarı kuralları (Prometheus `alerting_rules`): **`docs/PROMETHEUS_ALERT_EXAMPLES.md`**.

## Prometheus örnek job

```yaml
scrape_configs:
  - job_name: iqai
    metrics_path: /metrics/prometheus
    static_configs:
      - targets: ['localhost:8080']
```

## Kod referansı

- `crates/iqai-core/src/sli.rs` — `render_prometheus_sli`, `render_prometheus_sli_minimal`
- `crates/iqai-core/src/trade_db.rs` — `sli_counters`, `sli_incr`, `persist_q04_normalized_errors_from_memory`, `migrate_sli_counters`
- `crates/iqai-core/src/auto_trader.rs` — canlı emir yollarında `sli_incr`
- `crates/iqai-web/src/http_app.rs` — route `metrics_prometheus`
- `crates/iqai-core/src/binance_error.rs` — `iqai_exchange_normalized_errors_total`
