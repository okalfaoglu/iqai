# IQAI Proje Dokümantasyonu

Bu doküman projenin satır satır incelenmesiyle oluşturulmuş teknik referans ve mimari özetidir. Eksik, hatalı veya iyileştirme noktaları son bölümde listelenir.

---

## 1. Genel Bakış

**IQAI**, Smart Money Structure (SMS) tabanlı bir kripto/hisse trading motorudur. Q-ANALİZ (Q-RADAR erken uyarı + Q-Setup giriş/TP/SL), Elliott Wave formasyonları, dip/tepe dönüş tespiti ve otomatik trade yönetimi sunar.

- **Dil:** Rust  
- **Workspace:** Cargo workspace (iqai-core, iqai-binance, iqai-tv, iqai-cli, iqai-web; iqai-gui opsiyonel)  
- **Config:** `config.json` (IQAI_CONFIG env → `./config.json` → `~/.config/iqai/config.json`)  
- **Veri:** Binance (spot/futures) veya TradingView connector (HTTP/subprocess/native)

---

## 2. Workspace ve Crate Yapısı

| Crate | Amaç |
|-------|------|
| **iqai-core** | Motor: SMS sinyal, Q-Setup/Q-RADAR, Elliott, dip/tepe, trade manager, auto-trader, SQLite trade DB, backtest stub |
| **iqai-binance** | Binance Spot ve USDT-M Futures API (klines, emir, imza, exchangeInfo) |
| **iqai-tv** | TradingView veri: Rust WebSocket veya HTTP/subprocess (tv_connector) |
| **iqai-cli** | Tek binary `iqai`: scan, scan-batch, trade, watch, robot, q-analiz-daemon, formations, config, ollama-check |
| **iqai-web** | Axum sunucu (8080): grafik, Q-Analiz, PnL, metrics, settings, bildirim (Telegram/WhatsApp/X/Email vb.) |
| **iqai-gui** | Desktop GUI (egui/eframe) – stub; workspace’e varsayılan dahil değil |

**Bağımlılık grafiği:** iqai-core ← iqai-binance, iqai-tv, iqai-web ← iqai-cli (ve iqai-gui).

---

## 3. Konfigürasyon (config.json)

- **notification:** Telegram (bot_token, chat_id), WhatsApp/Instagram/Facebook/X/Email (webhook_url, webhook_token).  
- **logging:** level (trace/debug/info/warn/error), target (console/file/both), file_path.  
- **data:** max_bars, native_tf_mode (eski uyumluluk).  
- **ai:** enabled, model, ollama_base_url (Q-Analiz AI yorumu).  
- **trading:** enabled, mode (live/dry/paper), market (futures/spot), api_key, secret_key, db_path, risk_per_trade_pct, max_positions, max_leverage, daily_loss_limit_pct, min_q_score, min_rr, use_radar_filter, min_radar_confidence, symbols, timeframes, commission_bps, slippage_bps, use_limit_order, limit_slippage_bps.  
- **tv_username, tv_password, tv_totp_secret, tradingview_auth_token:** TradingView auth (opsiyonel).  
- **smart_money:** Pine Script uyumlu parametreler (Config::from_smart_money ile Config’e dönüşür).

**Önemli:** Hassas alanlar (token, şifre, API key) için ortam değişkenleri desteklenir; `config.json`’ın repo’da veya paylaşılan ortamda düz metin tutulmaması gerekir.

---

## 4. iqai-core Modülleri (Özet)

### 4.1 types.rs
- **Candle:** OHLCV, hlc3, typical_price, is_bullish/is_bearish.  
- **Timeframe:** M1–D1, minutes(), from_str, to_binance_interval, Serialize/Deserialize.  
- **MarketType, Exchange, SignalType, Signal, TrendMomentumRiskScores, PositionMetrics.**  
- **QSetup, QRadarSignal, ProtectSignal:** Q-ANALİZ çıktı tipleri.

### 4.2 config.rs
- **Config:** Pine Script uyumlu tam parametre seti (pivot_length, momentum, TP/SL, filtreler, trade management, Elliott görsel, Q-ANALİZ parametreleri, q_entry_atr_alpha/beta, q_sl_atr_gamma, q_tp_structure_ext, q_tp_max_r, q_require_mtf_for_dip_zone, q_rsi_oversold/overbought, q_weight_*).  
- **Config::from_smart_money:** AppConfig.smart_money ile override.

### 4.3 app_config.rs
- **LogTarget, LoggingConfig, NotificationConfig, DataConfig, SmartMoneyConfig, TradingConfig, AiConfig, AppConfig.**  
- **AppConfig::config_path(), load().**

### 4.4 exchange.rs
- **ExchangeConnector** (async_trait): exchange(), market_type(), fetch_klines(), place_market_order(), place_limit_order_ioc() (default: not supported), get_commission_bps(), get_balance().  
- **ExchangeError, OrderSide, OrderResponse.**

### 4.5 indicators.rs
- EMA, SMA, ATR, RSI, pivot_high/pivot_low, VWAP, highest, lowest (Pine Script portu).

### 4.6 signal.rs
- **CandleBuffer:** TF bazlı mum map.  
- **SignalEngine:** process() (Buy/Sell sinyalleri), trend_for_tf(), trend_strength(), system_confidence(), compute_position_metrics(), fibo_time_phase(), last_pivots(), structure_based_tp(), structure_score(), compute_q_setup(), compute_q_radar(), compute_protect_signal().  
- Q-Setup: pivot + ATR giriş bölgesi, 5 bileşenli Q-skor, time_window_bars, radar_early.  
- Poz koruma: q_protect_min_r, q_protect_lock_r, LATE_PHASE / TRAILING_PROFIT.

### 4.7 reversal.rs
- **DipAnalysis, PeakAnalysis, ReversalAnalysis.**  
- get_dip_price_and_index, get_peak_price_and_index, compute_reversal_analysis.  
- Dipten/tepeden dönüş, reversal_strength, Wyckoff Spring/Upthrust.

### 4.8 dip_confluence.rs
- **DipConfluenceResult:** mtf_support_near, ltf_structure_ok, fib_elliott_zone, divergence_ok, spring_ok, rsi_zone_ok, bos_ok, absorption_ok, layers_passed.  
- compute_dip_confluence(): 8 katmanlı doğrulama; q_require_mtf_for_dip_zone ile MTF zorunluluğu.  
- Bullish/bearish divergence (RSI + pivot).

### 4.9 q_radar_analysis.rs
- **QRadarOpportunityAnalysis:** symbol, timeframe, radar, dip, peak, detection, confidence_score, early_warning_score, recommendation, confirmation_layers, direction, reference_price.  
- compute_q_radar_opportunity(): SignalEngine + reversal + dip_confluence; “DİP BÖLGESİ” / “TEPE BÖLGESİ” etiketleri ve tavsiye metinleri.

### 4.10 elliott.rs / elliott_detector.rs / impulse_detector.rs
- Elliott kuralları, dalga dereceleri, formasyon tipleri, W3/W5 setup, Zigzag C, Triangle E, projeksiyonlar, alternation, truncation, divergence.  
- **ElliottDetectorResult:** wave_points, wave_legs, fibo_levels, impulse_state, corr_setup, w5_targets, vb.  
- **ImpulseDetectorState, ImpulseStage:** Watching → W1Candidate → W2Validating → ImpulseConfirmed / Invalidated.

### 4.11 trade_manager.rs
- **Position, PositionSide, TradeAction** (MoveSlToBreakeven, PartialClose, UpdateTrailingStop, FullClose, None).  
- calculate_position_size(), TradeManager::create_position(), evaluate(), chandelier_long/short(), apply_action().  
- Breakeven, TP1/TP2 kısmi, Chandelier/ATR trailing.

### 4.12 auto_trader.rs
- **TradingMode:** Live, Dry, Paper.  
- **TradeSignal, ManagedPosition, TradeLog, TradeEvent.**  
- AutoTrader: should_take_signal(), process_signal(), tick_positions(), close_position(), partial_close(), full_tick(), restore_open_positions(), drain_events(), emit_daily_summary(), save_daily_summary().  
- RADAR filtresi, günlük kayıp limiti, limit IOC / piyasa emri, commission/slippage.

### 4.13 trade_db.rs
- **TradeDb:** SQLite (signals, positions, trade_log, daily_summary, q_analiz_detections).  
- insert_signal, insert_position, close_position, update_position_sl, insert_trade_log, insert_daily_summary, load_open_positions, insert_q_analiz_detection, get_q_analiz_detections, get_symbol_pnl_stats.  
- **QAnalizDetectionRecord, SymbolPnlStats.**

### 4.14 backtest.rs
- scan_historical_q_setups(): Geçmiş mum üzerinde bar bar Q-Setup tarar; (bar_index, QSetup) listesi.

### 4.15 logging.rs
- init_from_config(LoggingConfig), flexi_logger, critical! makro.

---

## 5. iqai-binance

- **BinanceSpotClient, BinanceFuturesClient:** new(), with_credentials().  
- Futures: exchangeInfo (LOT_SIZE, MIN_NOTIONAL), fetch_klines, place_market_order, place_limit_order_ioc, get_balance, get_commission_bps (API’den), fetch_ticker_price.  
- round_down_to_step, format_quantity (LOT_SIZE uyumu).  
- sign modülü: HMAC imza, query string.

---

## 6. iqai-tv

- **TvConnectorClient:** subprocess (Python script), with_exchange (HTTP), auto (tradingview-rs veya native).  
- fetch_klines_native, fetch_klines: sembol/interval/limit.

---

## 7. iqai-cli

- **Komutlar:** Scan, ScanBatch (daemon, interval), Trade, Watch, Robot, QAnalizDaemon, Config, Formations, OllamaCheck.  
- Scan: Binance/TV, CandleBuffer, SignalEngine, Q-Setup/Q-RADAR, bildirim.  
- Robot: config.json trading, AutoTrader, full_tick, Notifier (TradeEvent).  
- QAnalizDaemon: trading.symbols/timeframes, DB’ye tespit yazma, Telegram.  
- Watch: pozisyon izleme, Poz Koruma bildirimi.  
- Watchlist: symbol, market, exchange, timeframe; BIST/NASDAQ piyasa saatleri kontrolü.

---

## 8. iqai-web

- **Rotalar:** /, /settings, /metrics, /pnl, /q-analiz; /api/chart, /api/formations, /api/pnl/symbols, /api/q-analysis, /api/q-analiz/detections, /api/config (GET/POST).  
- **api/chart:** market (futures/spot/tv), exchange, symbol, tf, invert, entry, sl; Binance veya TV klines, SignalEngine, Q-Setup, Q-RADAR, Poz Koruma, annotations, formations, position_metrics; Q-Setup/Q-RADAR/Q-Analiz/Protect bildirimleri (Notifier).  
- **api/q-analysis:** config trading.symbols/timeframes üzerinden tüm sembol×TF Q-RADAR fırsat listesi.  
- **api/config:** AppConfig GET/POST (dosyaya yazma).  
- **notify:** routing_rules (QSetup/QRadar/QAnalysis/Protect/Info/Trade*), Telegram + webhook (WhatsApp, X, Email vb.).  
- **ai:** check_ollama, interpret_q_analysis (Ollama Türkçe yorum).

---

## 9. Testler

- **iqai-core:** backtest (yetersiz bar boş, yeterli bar çalışır), reversal (get_dip_peak).  
- **iqai-web:** notify routing_rules testleri.  
- `cargo test -p iqai-core`, `cargo test -p iqai-web`, `cargo test`.

---

## 10. Eksik, Hatalı ve İyileştirme Noktaları

Aşağıda, inceleme sırasında tespit edilen noktalar listelenmiştir.

### 10.1 Güvenlik

1. **config.json içinde hassas veri:** Proje kökündeki `config.json` (git status’ta modified) içinde gerçek Telegram bot token, chat_id ve TradingView şifresi görülmektedir. `.gitignore`’da `config.json` var; buna rağmen dosya takip ediliyorsa veya başka yollarla sızdıysa **token/şifre hemen değiştirilmeli** ve hassas config örnekleri sadece `config.json.example` ile verilmelidir.  
2. **API anahtarları:** Canlı modda api_key/secret_key config veya env’den okunuyor; env tercih edilmeli ve config örneklerinde asla gerçek değer kullanılmamalı.

### 10.2 Olası Hatalar / Tutarsızlıklar

3. **api_q_analysis_all (iqai-web):** `app_cfg.trading.as_ref().and_then(|t| t.symbols.clone()).filter(|s| !s.is_empty())` ifadesinde `filter` Option üzerinde; `Option<Vec<String>>` için doğru kullanım. Ancak `unwrap_or_else(|| vec!["ETHUSDT".into(), ...])` ile varsayılan kullanılıyor – config’te `symbols: []` boş array ise filtreden sonra None kalıp varsayılan gelir; davranış dokümante edilmeli.  
4. **trade_db get_symbol_pnl_stats:** “opened_count” için yapılan sorgu `SELECT symbol, COUNT(*) FROM positions WHERE mode=?1 GROUP BY symbol` ile **tüm pozisyonları** (açık + kapalı) sayar. Alan adı “açık pozisyon sayısı” gibi anlaşılabilir; gerçekte “toplam pozisyon sayısı”. Ya sorgu `status='open'` ile kısıtlanmalı ya da alan adı (ve dokümantasyon) “total_positions” vb. olacak şekilde güncellenmeli.  
5. **TradingMode::from_str:** "paper" için `_ => Self::Paper` kullanılıyor; "paper" açıkça eşleştirilmediği için şu an doğru çalışıyor ama ileride başka mod eklenirse karışabilir; "paper" için açık branch eklenmesi okunabilirlik için iyi olur.

### 10.3 Eksikler

6. **Birim/integrasyon test kapsamı:** Sadece backtest stub ve notify routing testleri var. Sinyal motoru, Q-Setup, confluence, Elliott detektörü için birim testleri; exchange mock ile kısa entegrasyon testleri eklenebilir.  
7. **Hata mesajları ve loglama:** Bazı API/DB hataları sadece eprintln veya log ile veriliyor; istemciye dönen JSON’da tutarlı bir `error` alanı ve (geliştirme modunda) request id ile ilişkilendirme iyileştirilebilir.  
8. **Rate limit / retry:** Binance ve TV çağrılarında rate limit veya geçici hata için retry/backoff politikası yok; eklenmesi production için faydalı olur.  
9. **Config validasyonu:** config.json yüklendikten sonra (min_q_score, min_rr, risk_per_trade_pct vb.) anlamlı aralık kontrolü yok; başlangıçta bir validasyon katmanı eklenebilir.

### 10.4 İyileştirmeler

10. **Dokümantasyon:** COMMANDS.md ve USAGE.md iyi; API endpoint’leri için OpenAPI/Swagger tanımı yok; eklenirse frontend ve entegrasyon geliştirmeyi kolaylaştırır.  
11. **iqai-gui:** “Coming soon” stub; workspace’e dahil değil. Ya tamamlanıp dokümante edilmeli ya da README’de “experimental/stub” olarak belirtilmeli.  
12. **Watchlist ve piyasa saatleri:** BIST/NASDAQ saatleri sabit kodlu; dil/locale veya config’ten okunabilir.  
13. **Web API CORS:** Axum tarafında CORS middleware açıkça belirtilmemiş; farklı origin’den erişim gerekiyorsa CORS yapılandırması eklenmeli.  
14. **Log seviyesi:** RUST_LOG ile override ediliyor; config.json “logging.level” ile birlikte öncelik sırası (örn. env > config) dokümante edilmeli.  
15. **Pozisyon boyutu:** calculate_position_size leverage ile sınırlıyor; bazı borsalarda sembol bazlı max notional/pozisyon limiti de olabilir; exchangeInfo veya config ile ek sınır kontrolü düşünülebilir.

---

## 11. Referanslar

- **Mevcut dokümanlar:** COMMANDS.md, USAGE.md, docs/Q_ANALIZ_ALANLARI.md, DIP_TESPITI_KATMANLAR.md, ELLIOTT_WAVE_SPEC.md, ACIK_POZISYON_SURECI.md, DIP_TEPE_*.md.  
- **Config örneği:** config.json.example.  
- **Watchlist:** watchlist.json, watchlist.json.example.

Bu doküman proje kodunun satır satır incelenmesiyle üretilmiştir; güncel kodla birlikte güncellenmesi önerilir.
