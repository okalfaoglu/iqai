# Analiz Snapshot Kullanımı

Her **sembol × timeframe** için tek satır tutan `analysis_snapshots` tablosu ve bunun Web/API kullanımı.

---

## 1) Veriyi doldurmak (daemon)

Snapshot'lar **Q-Analiz daemon** çalıştığında güncellenir. Her turda tüm `config.json` sembolleri ve timeframe'leri taranır, her (sembol, TF) için bir satır **upsert** edilir (eski kayıt güncellenir).

```bash
iqai q-analiz-daemon -i 300
```

- `-i 300`: tur aralığı (saniye). 300 = 5 dakikada bir tarama.
- Daemon çalışırken `data/trades.db` içindeki `analysis_snapshots` tablosu sürekli güncellenir.

---

## 2) API

### Snapshot listesi

- **GET** `/api/analysis-snapshots`
  - Tüm snapshot'ları döner (sembol × TF).
- **GET** `/api/analysis-snapshots?symbol=BTCUSDT`
  - Sadece o sembole ait satırlar.

Yanıt örneği:

```json
{
  "snapshots": [
    {
      "symbol": "BTCUSDT",
      "timeframe": "5m",
      "updated_at": 1710500000000,
      "detection": "DİP BÖLGESİ (TEPKİ DİBİ)",
      "direction": "LONG",
      "confidence_score": 6.5,
      "rsi_14": 32.1,
      "elliott_formation": "Impulse W3",
      ...
    }
  ]
}
```

### Büyük resim raporu (AI)

- **GET** `/api/analysis-snapshots/report?symbol=ETHUSDT`

Önce o sembolün tüm TF snapshot'ları metne çevrilir, ardından (Ollama açıksa) AI'dan kısa Türkçe “büyük resim” özeti istenir.

Yanıt:

```json
{
  "symbol": "ETHUSDT",
  "snapshot_count": 5,
  "report": "Sembol: ETHUSDT | 5 timeframe özeti:\n[5m] Tespit: ...",
  "ai": "Kısa AI özeti metni..."
}
```

---

## 3) Web arayüzü

- **Sayfa:** http://localhost:8080/snapshots
- **Özellikler:**
  - Sembol filtresi (opsiyonel): Sadece o sembolün satırlarını gösterir.
  - **Yenile:** Snapshot listesini API'den tekrar çeker.
  - Tablo: Sembol, TF, güncellenme, tespit, yön, güven, erken uyarı, tavsiye, fiyat, Elliott, strateji.
  - **Büyük resim raporu:** Sembol yazıp “Rapor al” ile hem ham özet hem (varsa) AI yorumu gösterilir.

Menüden **Snapshot'lar** linki ile Q-Analiz ve Kar/Zarar sayfalarından bu sayfaya geçilebilir.

---

## 4) Özet

| Ne | Nasıl |
|----|--------|
| Tabloyu doldurmak | `iqai q-analiz-daemon -i 300` |
| Tüm snapshot'ları almak | `GET /api/analysis-snapshots` |
| Tek sembol snapshot'ları | `GET /api/analysis-snapshots?symbol=BTCUSDT` |
| AI büyük resim raporu | `GET /api/analysis-snapshots/report?symbol=ETHUSDT` veya /snapshots sayfasında “Rapor al” |
| Web’de tablo + rapor | http://localhost:8080/snapshots |

Tablo şeması ve alan açıklamaları için: `docs/ANALIZ_SNAPSHOT_TABLO_ALANLARI.md`.
