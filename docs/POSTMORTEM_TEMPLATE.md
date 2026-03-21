# Olay sonrası özet (postmortem) — şablon

**TFAI-Q13** — blameless; yatırım tavsiyesi değil, teknik süreç kaydı.

## Meta

| Alan | Değer |
|------|--------|
| **Olay ID** | |
| **Tarih/saat (UTC)** | |
| **Etki** | (örn. emir gönderilemedi, gecikme, veri eksik) |
| **Süre** | (kesinti / anomali süresi) |
| **İlgili sürüm / commit** | |
| **Yazar** | |
| **İnceleme tarihi** | |

## Özet (exec summary)

1–3 cümle: ne oldu, kullanıcı/operasyon etkisi.

## Zaman çizelgesi

| Saat (UTC) | Olay |
|------------|------|
| | |

## Kök neden (5 neden / hipotez)

- **Doğrudan neden:**
- **Kök neden (hipotez):**
- **Doğrulama:** (log, DB, metrik, replay)

## IQAI bağlamı (varsa)

- `trace_id` / `position_uuid` / `signal_id`:
- İlgili log satırları (örnek grep):
- `close_reason` / borsa kodu:

## Düzeltme (hemen)

- [ ] Kod / config değişikliği:
- [ ] Dağıtım:

## Önleyici (takip)

- [ ] Test / alarm / runbook güncellemesi:
- [ ] Sahip / hedef tarih:

## Ekler

- Linkler, grafik, ham log özeti.
