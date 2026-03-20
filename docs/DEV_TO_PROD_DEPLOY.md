# IQAI Dev → Prod Kod Taşıma Standartları (Rocky Linux)

Bu doküman, **dev ortamındaki** `/app/iqai` kodunun **prod sunucuya** (örn. `100.120.77.57:/app/iqai`) taşınması, derlenmesi ve çalıştırılması için standart süreci içerir.

---

## 1. Temel prensipler (standartlar)

- **Kaynak kod taşınır, binary taşınmaz (önerilen)**  
  Prod sunucuda derlemek, OS/arch/glibc/openssl uyumsuzluklarını azaltır.
- **Prod’a state kopyalanmaz**  
  `data/` (DB), `logs/` (log arşivi), `target/` (derleme çıktısı) dev’den prod’a taşınmaz.
- **Yetki net olmalı**  
  Prod’da uygulama klasörü ve özellikle `data/` + `logs/` yazılabilir olmalı (uygulamayı hangi user çalıştırıyorsa ona ait olmalı).
- **Gizli bilgiler repoya girmez**  
  `config.json` prod’da ayrı yönetilir (gerekirse `config.json.example` baz alınır). API key vb. repo dışı tutulur.

---

## 2. Hedef sunucuda (prod) bir kere yapılacak kurulum

### 2.1 Dizin ve kullanıcı

Prod’da uygulamayı çalıştıracak kullanıcı: örn. `qtss`.

```bash
sudo mkdir -p /app/iqai/{data,logs}
sudo chown -R qtss:qtss /app/iqai
```

### 2.2 Build bağımlılıkları + Rust

```bash
sudo dnf install -y git gcc make openssl-devel pkgconfig
curl https://sh.rustup.rs -sSf | sh
source ~/.cargo/env
```

---

## 3. Dev → Prod kopyalama (rsync)

### 3.1 Önerilen rsync (state hariç)

Dev sunucuda:

```bash
rsync -avz --delete \
  --exclude .git/ \
  --exclude target/ \
  --exclude data/ \
  --exclude logs/ \
  /app/iqai/ qtss@100.120.77.57:/app/iqai/
```

### 3.2 Permission hataları için

Eğer `Permission denied` görürsen:

- Ya prod’da `/app/iqai` ownership düzelt:

```bash
ssh root@100.120.77.57
chown -R qtss:qtss /app/iqai
```

- Ya da timestamp yazmayı kapat (ikincil çözüm):

```bash
rsync -avz --delete --no-times \
  --exclude .git/ --exclude target/ --exclude data/ --exclude logs/ \
  /app/iqai/ qtss@100.120.77.57:/app/iqai/
```

---

## 4. Prod’da derleme

Prod’da:

```bash
cd /app/iqai

# Sadece CLI+Web derlemek çoğu kullanım için yeterli:
cargo build --release --package iqai-cli --package iqai-web
```

Çalıştırılacak binary’ler:

- CLI: `./target/release/iqai`
- Web: `./target/release/iqai-web`

---

## 5. Prod’da çalıştırma komutları

### 5.1 Q-Analiz daemon

```bash
cd /app/iqai
./target/release/iqai q-analiz-daemon -i 300
```

### 5.2 Robot (dry/paper/live)

```bash
cd /app/iqai

# Dry/paper
./target/release/iqai robot --mode dry --interval 60

# Live (config.json içinde api_key/secret_key gerekli)
./target/release/iqai robot --mode live --interval 60
```

### 5.3 Web

Varsayılan port 8080.

```bash
cd /app/iqai
./target/release/iqai-web
```

### 5.4 Tek komutla hepsi (Stack)

`iqai` CLI’daki `stack` komutu:

```bash
cd /app/iqai
./target/release/iqai stack --q-interval 30 --robot-interval 60 --robot-mode dry --web-port 8080
```

---

## 6. Konfigürasyon standardı (öneri)

- Prod’da `config.json` dosyasını **repodan bağımsız** yönetin:
  - İlk kurulum için `config.json.example` → `config.json`
  - API key/secret gibi alanlar prod’da doldurulur.
- `data/trades.db` prod’da kalıcıdır; deploy sırasında overwrite edilmez.

---

## 7. Hızlı kontrol listesi

- **Kopyalama**: `rsync` exclude’leri doğru mu? (`.git`, `target`, `data`, `logs`)
- **Yetki**: prod’da `/app/iqai/data` ve `/app/iqai/logs` yazılabilir mi?
- **Derleme**: `cargo build --release --package iqai-cli --package iqai-web`
- **Çalıştırma**: `iqai stack` veya ayrı ayrı daemon/robot/web

