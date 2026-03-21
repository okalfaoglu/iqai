//! OpenAPI YAML + Swagger UI sayfası — `iqai-web` binary ve entegrasyon testleri ortak.

use axum::{
    http::header,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};

pub async fn openapi_yaml() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/yaml; charset=utf-8",
        )],
        include_str!("../openapi.yaml"),
    )
}

pub async fn api_docs_page() -> impl IntoResponse {
    Html(include_str!("../docs_swagger.html"))
}

/// Sadece şema + docs uçları (HTTP smoke test için).
pub fn router() -> Router {
    Router::new()
        .route("/api/openapi.yaml", get(openapi_yaml))
        .route("/api/docs", get(api_docs_page))
}
