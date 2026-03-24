# Git yardımcı scriptleri

Tüm komutlar repo kökünden veya `scripts/` içinden çalışır.

## Çalıştırma izni (bir kez)

```bash
chmod +x scripts/*.sh
```

### `bash\r: No such file or directory`

Dosyalar Windows (CRLF) satır sonu ile kaydedildiyse Linux shebang’i bozulur. Repoda `.gitattributes` ile `*.sh` → LF zorunlu. Eski kopyada düzeltmek için:

```bash
sed -i 's/\r$//' scripts/*.sh
```

## GitHub’a gönder (dev)

```bash
cd /app/iqai
./scripts/git-push.sh "fix: açıklama"
```

Mesaj vermeden çalıştırırsan: değişiklik yoksa sadece `git push`; değişiklik varsa mesaj ister.

## GitHub’dan çek (dev)

```bash
./scripts/git-pull.sh
```

## Prod: repoyu GitHub ile tam eşitle

Yerel prod patch’lerini **silmek** için (tek kaynak GitHub olsun):

```bash
./scripts/git-prod-reset.sh
```

Onaysız:

```bash
IQAI_YES=1 ./scripts/git-prod-reset.sh
```

Ardından derleme ve servis:

```bash
cargo build --release -p iqai-core -p iqai-web -p iqai-binance -p iqai-cli
sudo systemctl restart iqai-stack.service
```

Detaylı deploy: `docs/DEV_TO_PROD_DEPLOY.md`.
