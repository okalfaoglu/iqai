# AI çıktı izlenebilirliği (TFAI-O08 / Q11)

Ollama ile üretilen metinler, denetim için SQLite **`ai_explanations`** tablosuna yazılır.

## Şema (özet)

| Alan | Anlam |
|------|--------|
| `explanation_id` | UUID |
| `generated_at` | ms epoch |
| `kind` | `q_analysis_interpret` (daemon) veya `big_picture` (web rapor) |
| `model_id` | Ollama model adı |
| `prompt_template_version` | Şablon sürümü (`q_analysis_interpret_v1`, `big_picture_v4` — büyük resim: Ollama **`/api/generate` + `system`**, İngilizce kalıp tespitinde otomatik Türkçe çeviri yedeği; `prompt_hash` system+prompt birleşik metin) |
| `prompt_hash` | Gönderilen **tam** kullanıcı mesajının SHA-256 hex |
| `context_hash` | Modele giden bağlam metninin SHA-256 hex |
| `query_fingerprint` | Büyük resimde: snapshot satırlarının sıralı parmak izi |
| `symbol`, `timeframe` | İsteğe bağlı |
| `source_refs_json` | Örn. `["BTCUSDT@5m",...]` |
| `event_ids_json` | İleride `position_events` ile doldurulabilir; şimdilik `[]` |
| `explanation_text` | Model çıktısı |

## Yapılandırma

`config.json` → `ai`:

```json
"ai": {
  "enabled": true,
  "model": "mistral",
  "ollama_base_url": "http://localhost:11434",
  "persist_explanations": true
}
```

`persist_explanations` yoksa **true** kabul edilir; `false` ile DB yazımı kapatılır.

## API

- `GET /api/ai-explanations?symbol=ETHUSDT&limit=30` — son kayıtlar (sembol filtreli veya tümü).

## Kod

- Tablo + CRUD: `iqai-core::trade_db::TradeDb`
- Prompt inşası + şablon sabitleri: `iqai-web::ai`
- Hash: `iqai_core::sha256_hex`
