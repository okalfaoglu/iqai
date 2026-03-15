# IQAI – CLI ve Web kullanım senaryoları

Bu dokümanda CLI komutları, Web API çağrıları ve yapılandırma örnekleri yer alır.  
**Tüm komutların kısa listesi:** `COMMANDS.md`

---

## 0. Q-ANALİZ vs Q-Setup ve Elliott Wave

- **Q-ANALİZ:** Yöntemin genel adı. İçinde **Q-RADAR** (erken uyarı), **Q-Setup** (giriş/TP/SL çıktısı) ve **Poz Koruma** var.
- **Q-Setup:** Somut işlem fırsatı — yön, giriş bölgesi, SL, TP, Q-skor, süre. “Analiz” bu skorlarla yapılıyor; “setup” ise tek bir trade çıktısı.
- **Q-Setup Elliott Wave kullanmıyor.** Girdileri: çok zaman dilimli trend (EMA/VWAP), pivot (swing high/low), ATR, Fibo-zaman fazı (zaman penceresi), yapı skoru (HH/HL), momentum/hacim. Elliott Wave ayrı modülde — grafikte Elliott çizimi ve formasyon listesi için kullanılıyor; Q-Setup hesaplamasına dahil değil.

### Entry, SL, TP nasıl belirleniyor?

Hepsi **pivot** (swing high/low) + **ATR(14)** + **config katsayıları** ile; Elliott Wave yok.

| Çıktı | Long formülü | Short formülü |
|--------|----------------|----------------|
| **Giriş bölgesi** | `L_pivot + α·ATR` … `L_pivot + β·ATR` | `H_pivot − β·ATR` … `H_pivot − α·ATR` |
| **Entry** | Son mum kapanışı, giriş bölgesine clamp edilir | Aynı mantık (bölge içinde) |
| **SL** | `L_pivot − γ·ATR` | `H_pivot + γ·ATR` |
| **TP** | `entry + max(q_min_rr × risk, 2×ATR)` | `entry − max(q_min_rr × risk, 2×ATR)` |

- **L_pivot / H_pivot:** Son pivot low / pivot high (TradingView tarzı `pivot_length` ile).
- **risk:** `|entry − SL|`.
- **α, β, γ:** `q_entry_atr_alpha`, `q_entry_atr_beta`, `q_sl_atr_gamma` (config).
- **q_min_rr:** Minimum risk/ödül oranı (örn. 1.5).

Yani seviyeler tamamen **fiyat yapısı (pivot)** ve **volatilite (ATR)** ile; zaman penceresi için Fibo-zaman fazı kullanılıyor, dalga sayısı veya Elliott formasyonu kullanılmıyor.

---

## 1. Yapılandırma

### Config dosyası

Bildirimler için `config.json` kullanılır. Örnek oluşturmak:

```bash
cp config.json.example config.json
# config.json içinde telegram_bot_token, telegram_chat_id vb. doldur
```

Aranan sıra: `IQAI_CONFIG` env ile verilen dosya → `./config.json` → `~/.config/iqai/config.json`.

### Sinyalleri bir Telegram grubuna göndermek

Tüm bildirimler (Q-Analiz, Q-Setup, pozisyon aç/kapa, günlük özet vb.) tek bir Telegram sohbetine gider. Bu sohbet **özel sohbet** veya **grup** olabilir.

1. **Botu gruba ekleyin:** Telegram’da grubu açın → Üyeler / Add Members → @YourBot kullanıcı adını arayıp ekleyin. Gerekirse “gruba mesaj atabilir” yetkisi verin.
2. **Grup chat ID’sini alın:**  
   - Gruba herhangi bir mesaj yazın (veya bota `/start` atın).  
   - Tarayıcıdan: `https://api.telegram.org/bot<BOT_TOKEN>/getUpdates` açın. En alttaki mesajda `"chat":{"id": -123456789, "type":"group", ...}` görünür. **id** değeri (negatif sayı) grubun chat ID’sidir.  
   - Alternatif: [@userinfobot](https://t.me/userinfobot) veya [@getidsbot](https://t.me/getidsbot) gibi botları gruba ekleyip gönderdiği ID’yi kullanabilirsiniz.
3. **config.json’a yazın:**  
   `notification.telegram_chat_id` değerini **grubun chat ID’si** yapın (örn. `-1001234567890`). Özel sohbet için pozitif sayı, grup için negatif.
   ```json
   "notification": {
     "telegram_bot_token": "BOT_TOKEN_BURAYA",
     "telegram_chat_id": "-1001234567890"
   }
   ```
4. Robot veya web çalıştığında sinyaller bu gruba düşer. Ortam değişkeni kullanıyorsanız: `TELEGRAM_CHAT_ID=-1001234567890`.

### Loglama

Seviyeler: **trace**, **debug**, **info**, **warn**, **error**, **critical** (critical = error).  
CLI ve Web başlarken `env_logger` otomatik başlar; seviye **RUST_LOG** ile ayarlanır:

```bash
RUST_LOG=info cargo run -p iqai-cli -- scan -s ETHUSDT -t 5M
RUST_LOG=debug cargo run -p iqai-web
```

Örnekler: `RUST_LOG=info` (varsayılan), `RUST_LOG=debug`, `RUST_LOG=iqai_core=info,warn`, `RUST_LOG=error`.

### Ortam değişkenleri (isteğe bağlı)

- **Bildirim:** `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`, `WHATSAPP_WEBHOOK_URL`, `X_WEBHOOK_URL`, vb.
- **Web TV:** `TV_CONNECTOR_URL`, `TV_CONNECTOR_SCRIPT`, `TV_CONNECTOR_PYTHON`, borsa için `exchange` query ile.

### TV'den veri gelmiyorsa (grafik boş)

Web varsayılan olarak TV verisi için **HTTP** ile `http://localhost:8765` adresine istek atar. Bu portta servis yoksa grafik boş kalır. İki yol:

**Yol 1 – Subprocess (uvicorn gerekmez)**  
Web’i TV script’i ile çalıştır; veri Python script ile çekilir:

```bash
export TV_CONNECTOR_SCRIPT=tv_connector/fetch_hist.py
export TV_CONNECTOR_PYTHON=python3   # veya tv_connector/.venv/bin/python
cargo run -p iqai-web
```

Proje kökünden (`/app/iqai`) çalıştır. `tv_connector` içinde `pip install -r requirements.txt` (ve gerekirse `python3 -m venv .venv` + `source .venv/bin/activate`) yapılmış olmalı.

**Yol 2 – HTTP (ayrı servis)**  
TV Connector’ı 8765 portunda ayağa kaldır; sonra web’i normal başlat:

```bash
cd tv_connector
source .venv/bin/activate
pip install -r requirements.txt
uvicorn main:app --host 0.0.0.0 --port 8765
```

Başka terminalde:

```bash
cargo run -p iqai-web
```

Tarayıcıda market=TradingView, borsa=BINANCE, sembol örn. ETHUSDT veya ETHUSDT27H2026 seçip yenile. Veri yine gelmezse: `curl "http://localhost:8765/history?symbol=ETHUSDT&exchange=BINANCE&interval=5&n_bars=100"` ile connector’ı doğrudan test et.

---

## 2. CLI senaryoları

### Tek sembol tarama (Binance futures/spot)

```bash
# Futures, 5M, 500 bar
cargo run -p iqai-cli -- scan -s ETHUSDT -m futures -t 5M -l 500

# Spot, 15M
cargo run -p iqai-cli -- scan -s BNBUSDT -m spot -t 15M
```

Çıktıda Smart Money sinyalleri, trend, Q-RADAR ve Q-Setup (varsa) yazdırılır; Q-Setup/Q-RADAR bulunursa yapılandırılmış kanallara bildirim gider (routing_rules’a göre).

### Watchlist ile toplu tarama

`watchlist.json` örneği:

```json
[
  { "symbol": "ETHUSDT", "market": "futures", "timeframe": "5M" },
  { "symbol": "BTCUSDT", "market": "futures", "timeframe": "5M" },
  { "symbol": "XU100", "market": "tv", "exchange": "BIST", "timeframe": "15M" },
  { "symbol": "AAPL", "market": "tv", "exchange": "NASDAQ", "timeframe": "1H" }
]
```

- **Tek tur:**  
  `cargo run -p iqai-cli -- scan-batch -w watchlist.json -l 500`

- **Sürekli (daemon):**  
  `cargo run -p iqai-cli -- scan-batch -w watchlist.json --daemon --interval 300`  
  BIST/NASDAQ sadece piyasa açıkken taranır; her turda Q-Setup/Q-RADAR bulunursa bildirim gönderilir.

### Poz izleme ve Poz Koruma bildirimi (Watch)

Açık pozisyonu izler; kar koruma (breakeven, trailing) önerir ve **Poz Koruma** koşulu sağlanırsa bir kez bildirim gönderir (Telegram, WhatsApp, X, Email – routing’e göre).

```bash
cargo run -p iqai-cli -- watch \
  --symbol ETHUSDT \
  --side long \
  --entry 3500.0 \
  --sl 3400.0 \
  --tp 3700.0 \
  --quantity 1.0 \
  --market futures \
  --interval 10
```

- `interval`: Fiyat/kontrol aralığı (saniye).  
- Poz Koruma tetiklenirse `notify_protect` bir kez çağrılır.

### Q-Analiz daemon (sürekli tarama, DB, Telegram)

Web sayfası açık olmadan, komut satırından sürekli Q-Analiz taraması yapar. Tespit edilen sonuçlar **DB'ye yazılır** ve **Telegram'a** (görsel kart veya metin) gönderilir. Web’deki **Q-Analiz** sayfası bu kayıtları listeler.

```bash
# Varsayılan 300 saniye aralık
cargo run --bin iqai -- q-analiz-daemon

# 60 saniyede bir tara
cargo run --bin iqai -- q-analiz-daemon -i 60
```

- **Config:** `config.json` → `trading.symbols`, `trading.timeframes`, `trading.db_path` kullanılır (robot ile aynı).
- **DB:** Tespitler `q_analiz_detections` tablosuna yazılır (`data/trades.db` veya `trading.db_path`).
- **Web:** `http://localhost:8080/q-analiz` sayfasında "Son tespitler (DB)" tablosu bu kayıtları gösterir; API: `GET /api/q-analiz/detections?limit=100&symbol=ETHUSDT`.

### Elliott formasyonları (geçmiş veri)

```bash
cargo run -p iqai-cli -- formations -s ETHUSDT -m futures -t 15M --limit 500
```

### Config çıktısı

```bash
cargo run -p iqai-cli -- config              # varsayılan config JSON
cargo run -p iqai-cli -- config -f cfg.json  # dosyaya yaz
```

---

## 3. Web API senaryoları

Sunucu: `cargo run -p iqai-web` → varsayılan `http://localhost:8080`.

### Grafik + Q-Analiz: GET /api/chart

| Parametre   | Açıklama                          | Örnek    |
|------------|------------------------------------|----------|
| `symbol`   | Sembol                             | ETHUSDT  |
| `market`   | futures / spot / tv                 | futures  |
| `exchange` | TV için borsa (BINANCE, BIST, NASDAQ) | BINANCE  |
| `tf`       | Zaman dilimi                       | 5M, 15M, 1H |
| `invert`   | Invert pattern (1 / true)          | 1        |
| `entry`    | Poz Koruma giriş fiyatı (opsiyonel) | 3500.5   |
| `sl`       | Poz Koruma SL (opsiyonel)          | 3400.0   |

**Örnekler:**

- Binance futures, ETHUSDT, 5M:  
  `http://localhost:8080/api/chart?symbol=ETHUSDT&market=futures&tf=5M`

- TradingView BIST, XU100, 15M:  
  `http://localhost:8080/api/chart?symbol=XU100&market=tv&exchange=BIST&tf=15M`

- Aynı grafik + Poz Koruma (entry/sl verilirse `protect_signal` hesaplanır ve varsa bildirim gider):  
  `http://localhost:8080/api/chart?symbol=ETHUSDT&tf=5M&entry=3500&sl=3400`

Yanıtta `candles`, `signals`, `trend`, `q_setup`, `q_radar`, `protect_signal` (entry/sl verildiyse), `annotations`, `formations` döner.

### Q-Analiz sayfası ve tespit kayıtları

- **Sayfa:** `http://localhost:8080/q-analiz` — İzlenen semboller × timeframe için anlık Q-Analiz kartları + **Son tespitler (DB)** tablosu (daemon’un yazdığı kayıtlar).
- **Tespit listesi API:** `GET /api/q-analiz/detections?limit=100&symbol=ETHUSDT` — DB’deki Q-Analiz tespit kayıtları (yeniden eskiye).

### Elliott formasyonları: GET /api/formations

`http://localhost:8080/api/formations?symbol=ETHUSDT&market=futures&tf=15M&limit=500`

---

## 4. Bildirim routing (özet)

| Olay      | Kanallar (ör.)                          |
|-----------|------------------------------------------|
| Q-Setup   | Telegram, WhatsApp, Instagram, Facebook, X, Email |
| Q-RADAR   | Telegram, X                              |
| Poz Koruma| Telegram, WhatsApp, X, Email             |
| Info      | Telegram, Email                          |

Sadece yapılandırılmış kanallara gönderilir; örneğin sadece Telegram doluysa diğerleri atlanır.

---

## 5. Testler

Birim testleri çalıştırma:

```bash
cargo test -p iqai-core   # backtest: yetersiz bar → boş, yeterli bar → çalışır
cargo test -p iqai-web   # notify: routing_rules (QSetup, QRadar, Protect, Info)
```

Tüm workspace:

```bash
cargo test
```

Bu senaryolarla CLI ve Web kullanımını netleştirebilir; testler de backtest ve bildirim routing davranışını doğrular.
