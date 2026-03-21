# Log hacmi ve örnekleme (TFAI-O07)

## Sorun

`/api/chart` canlı modda birkaç saniyede bir çağrıldığında, her zaman dilimi için TV/Binance uyarıları **stderr’e** düşerse günlük hacim çok büyür (TFAI-Q07’deki “orderbook tick” benzeri gürültü).

## Çözüm (IQAI)

`config.json` → `logging`:

| Alan | Varsayılan | Açıklama |
|------|------------|----------|
| `verbose_chart_poll` | yok → `false` | `true` ise chart TF döngüsündeki mesajlar **`iqai_chart`** hedefinde **info** seviyesinde loglanır. `false`/yok: yalnızca **debug** (tipik `logging.level=info` iken görünmez). |

Örnek:

```json
"logging": {
  "level": "info",
  "target": "console",
  "verbose_chart_poll": false
}
```

Teşhis için geçici olarak `verbose_chart_poll: true` veya `level: "debug"` kullanın.

## İleri seviye (TFAI)

- Kritik yollar (emir, pozisyon yaşam döngüsü, borsa hatası) ayrı `target` veya yapılandırılmış log ile her zaman **info/warn** kalmalıdır.
- Üretimde OTel span örnekleme (head/tail) ayrı iterasyondur (O-05 sonrası).
