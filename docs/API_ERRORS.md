# Web API – hata JSON şeması

`iqai-web` hata yanıtlarında tutarlı alanlar:

| Alan | Açıklama |
|------|-----------|
| `ok` | Hata durumunda `false` (başarılı cevaplarda alan olmayabilir veya `true`). |
| `error` | İnsan tarafından okunabilir mesaj (string). |

Yardımcılar: `crates/iqai-web/src/api_json.rs`

- `api_error(message)` → `{ "ok": false, "error": "..." }`
- `api_error_with_extras(message, { ... })` → `ok` + `error` + ek alanlar (ör. `"symbols": []`).

İstek bazlı `request_id` şu an kullanılmıyor; proxy/load balancer üzerinden eklenebilir.
