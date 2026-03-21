# OpenAPI / Swagger (G-02)

- **YAML:** `crates/iqai-web/openapi.yaml` (kaynak kontrolde tutulur).
- **HTTP:** `GET /api/openapi.yaml` — ham şema.
- **Swagger UI:** `GET /api/docs` — tarayıcıda şema görüntüleme (CDN üzerinden Swagger UI 5).

Üretimde şemayı güncellediğinizde `openapi.yaml` dosyasını commit edin; otomatik üretim yoktur (ileride `utoipa` ile bağlanabilir).

**Test:** `cargo test -p iqai-web --test http_smoke` — tam uygulama `iqai_web::build_router()` ile `/api/openapi.yaml`, `/api/docs` ve birkaç statik rota duman testi.
