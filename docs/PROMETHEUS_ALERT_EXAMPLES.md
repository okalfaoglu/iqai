# Prometheus uyarı örnekleri (IQAI / TFAI P2)

Bu dosya **`GET /metrics/prometheus`** çıktısı için örnek [Prometheus](https://prometheus.io/docs/prometheus/latest/configuration/alerting_rules/) uyarı kuralları verir. Gerçek eşikler ortamınıza göre ayarlanmalıdır (`scrape_interval`, iş hacmi).

İlgili metrik açıklamaları: **`docs/SLI_METRICS.md`**.

## Ön koşullar

- Prometheus job’u IQAI web hedefini scrape ediyor (`/metrics/prometheus`).
- Q04 sayaçları `iqai_exchange_normalized_errors_total` olarak export edilir; kalıcı toplamlar `sli_counters` ile uyumludur.

## 1) Normalize borsa hata oranı (TFAI-Q04)

Son 15 dakikada bir önceki 15 dakikaya göre hata artışı (ör. ani API/auth sorunu):

```yaml
groups:
  - name: iqai_exchange_errors
    rules:
      - alert: IqaiExchangeErrorsElevated
        expr: |
          increase(iqai_exchange_normalized_errors_total[15m]) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "IQAI normalize borsa hataları yükseldi"
          description: "15m içinde toplam artış > 10 (exchange/category/tier etiketlerine göre ayrı seriler)."
```

Daha sıkı eşik için `> 3` veya belirli bir `category`/`tier` ile alt sorgu kullanılabilir.

## 2) Canlı emir açılış başarısızlığı (SLI)

Yalnızca canlı modda anlamlıdır (`exec_order_*` sayaçları canlıda artar):

```yaml
      - alert: IqaiOpenOrderFailuresSpike
        expr: |
          increase(exec_order_open_failure_total[10m]) > 0
            and
          increase(exec_order_open_attempt_total[10m]) > 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Canlı açılış emrinde başarısızlık"
          description: "exec_order_open_failure_total arttı; log ve Binance yanıtını kontrol edin."
```

Not: `exec_order_*` metrik isimleri exporter’da `sli_counters` anahtarından türetilir; tam isim `docs/SLI_METRICS.md` tablosu ile aynı olmalıdır.

## 3) Veritabanı erişilemezliği

Exporter DB açamazsa `iqai_db_reachable 0` yazar:

```yaml
      - alert: IqaiTradeDbUnreachable
        expr: iqai_db_reachable == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "IQAI trade DB scrape edilemiyor"
          description: "IQAI_TRADING_DB / config db_path ve dosya izinlerini kontrol edin."
```

## Referanslar

- `docs/SLI_METRICS.md` — metrik listesi ve Q04 akışı
- `docs/TRADE_FAILURE_PROGRESS.md` — P2 panel/alert pipeline durumu
