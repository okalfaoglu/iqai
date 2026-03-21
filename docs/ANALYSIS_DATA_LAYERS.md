# Analysis Data Layers

Bu doküman Q-ANALIZ verisinin 3 katmanda nasıl kullanılacağını netleştirir.

## 1) State Katmanı (`analysis_snapshots`)

- Amaç: UI/API için "son durum" gösterimi
- Yazım tipi: `upsert` (her `symbol + timeframe` için tek satır)
- Kullanım:
  - Web panelleri
  - Anlık API yanıtı
  - Debug: "şu an sistem ne görüyor?"

## 2) Event Katmanı (`q_analiz_detections`)

- Amaç: Zaman serisi şeklinde "anlamlı değişim" geçmişi
- Yazım tipi: `insert`
- Kullanım:
  - Alarm geçmişi
  - Değişim analizi
  - Olay bazlı raporlama

## 3) Outcome Katmanı (`analysis_outcomes`)

- Amaç: Event sonrası performans/doğruluk ölçümü
- Yazım tipi: `insert` (event + horizon)
- Alanlar:
  - `event_id`, `symbol`, `timeframe`, `direction`, `recommendation`, `reference_price`
  - `horizon_bars`, `return_pct`, `mfe_pct`, `mae_pct`
  - `tp_hit`, `sl_hit`, `quality_label`, `created_at`
- Kullanım:
  - Model kalibrasyonu (`7/10` gerçekten daha iyi mi?)
  - Segment performansı (TF, sembol, recommendation, direction)
  - Backtest/forward-test doğrulaması

### Partial close notu (kapsam)

`analysis_outcomes` kayıtları `AutoTrader::close_position()` içinde üretilir (pozisyon tamamen kapanınca).
`AutoTrader::partial_close()` çağrıları için `analysis_outcomes` yazılmaz; bu sayede outcome ölçümü “tüm trade” seviyesinde kalır ve remaining_pct / kısmi kapanışlardan kaynaklı çift sayım riski önlenir.

## Çalışma Prensibi

- Q-RADAR karar üretimi canlı candle akışından yapılır.
- DB katmanı kararın girdisi değil; karar çıktılarının kayıt/servis/analiz katmanıdır.
- Bu sayede hem düşük gecikme korunur hem de sonradan doğruluk analizi yapılır.

