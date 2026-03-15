# IQAI – Tüm komutlar (kopyala-yapıştır)

## Web arayüzü (grafik + Q-ANALİZ)

```bash
cargo run -p iqai-web
```

Tarayıcı: **http://localhost:8080**

**TradingView verisi için (grafik boşsa):** Önce TV connector’ı aç:
- **Subprocess:** `export TV_CONNECTOR_SCRIPT=tv_connector/fetch_hist.py` ve `TV_CONNECTOR_PYTHON=python3` ile aynı komutu çalıştır.
- **HTTP:** Başka terminalde `cd tv_connector && source .venv/bin/activate && uvicorn main:app --host 0.0.0.0 --port 8765`

---

## CLI

### Tek sembol tara
```bash
cargo run -p iqai-cli -- scan -s ETHUSDT -m futures -t 5M -l 500
```

### Watchlist ile toplu tarama
```bash
cargo run -p iqai-cli -- scan-batch -w watchlist.json -l 500
```

### Daemon (sürekli tarama, bildirim açık)
```bash
cargo run -p iqai-cli -- scan-batch -w watchlist.json --daemon --interval 300
```

### Poz izleme + Poz Koruma bildirimi
```bash
cargo run -p iqai-cli -- watch --symbol ETHUSDT --side long --entry 3500.0 --sl 3400.0 --tp 3700.0 --quantity 1.0 --market futures --interval 10
```

### Elliott formasyonları
```bash
cargo run -p iqai-cli -- formations -s ETHUSDT -m futures -t 15M --limit 500
```

### Config (varsayılan JSON / dosyaya yaz)
```bash
cargo run -p iqai-cli -- config
cargo run -p iqai-cli -- config -f cfg.json
```

### Trade (API key gerekir)
```bash
cargo run -p iqai-cli -- trade -s ETHUSDT -c long -q 1.0 -m futures
```

---

## Testler

```bash
cargo test -p iqai-core
cargo test -p iqai-web
cargo test
```

---

## Derleme / kontrol

```bash
cargo check -p iqai-core -p iqai-web -p iqai-cli
cargo build --release
```

---

Detaylı senaryolar ve API parametreleri için: **USAGE.md**
