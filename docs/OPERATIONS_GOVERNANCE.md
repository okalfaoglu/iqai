# Operasyon ve sahiplik (TFAI-Q14 — özet)

Küçük ekipler için **minimum süreç** çerçevesi; yasal uyumluluk yerine mühendislik disiplini.

## Roller

| Rol | Sorumluluk (özet) |
|-----|-------------------|
| **Mühendislik** | Kod, test, deploy, `TRADE_FAILURE_PROGRESS.md` güncellemesi |
| **Risk / işleten** | Canlı mod, günlük kayıp limiti, acil durdurma kararı (insan) |

Tek kişide birleşebilir; yine de **“kim canlıyı açtı / kapattı”** kaydı tutulmalıdır (`config`, systemd, not).

## Dağıtım

- Mümkünse **staging / paper** ile aynı commit’i doğrula (`DEV_TO_PROD_DEPLOY.md`).
- Canlıya geçişte **sürüm / commit** notu (log veya `TRADE_FAILURE_PROGRESS`).

## Acil durum

- **Otomatik günlük kayıp limiti** zaten `auto_trader` içinde varsa ona güven; ek olarak manuel **daemon durdurma** prosedürü tanımla (systemd `stop`).
- Aynı kişinin hem “kırık kodu yaz” hem “tek başına canlı onay” yapması **risk**; mümkünse ikinci onay veya paper kanıtı.

## İnceleme

- Olay sonrası: `docs/POSTMORTEM_TEMPLATE.md`
- Teknik borç: `docs/TRADE_FAILURE_PROGRESS.md` içinde `[ ]` maddeleri

*Bu dosya hukuki tavsiye değildir.*
