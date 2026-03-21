//! HTTP entegrasyon duman testi — tam uygulama router’ı (`build_router`) + OpenAPI uçları.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

fn test_app() -> Router {
    iqai_web::build_router()
}

#[tokio::test]
async fn get_openapi_yaml_returns_200_and_yaml() {
    let app = test_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/openapi.yaml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");

    assert_eq!(res.status(), StatusCode::OK);
    let ct = res.headers().get("content-type").and_then(|v| v.to_str().ok());
    assert!(
        ct.is_some_and(|s| s.contains("yaml")),
        "content-type: {:?}",
        ct
    );

    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("openapi:"), "first lines: {}", text.chars().take(80).collect::<String>());
}

#[tokio::test]
async fn get_api_docs_returns_200_and_swagger_ui() {
    let app = test_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/docs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let html = String::from_utf8_lossy(&bytes);
    assert!(
        html.contains("swagger") || html.contains("Swagger"),
        "swagger ui html expected"
    );
}

#[tokio::test]
async fn get_root_returns_200_html() {
    let app = test_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let html = String::from_utf8_lossy(&bytes);
    assert!(
        html.contains("IQAI") || html.contains("html") || html.contains("<!"),
        "index html expected"
    );
}

#[tokio::test]
async fn get_metrics_prometheus_returns_200_text() {
    let app = test_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/metrics/prometheus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(res.status(), StatusCode::OK);
    let ct = res.headers().get("content-type").and_then(|v| v.to_str().ok());
    assert!(
        ct.is_some_and(|s| s.contains("text/plain")),
        "content-type: {:?}",
        ct
    );
}

/// TFAI §3.3 — `tower-http` `X-Request-Id` (istemcide yoksa UUID üretilir).
#[tokio::test]
async fn response_includes_x_request_id_header() {
    let app = test_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/openapi.yaml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(res.status(), StatusCode::OK);
    let rid = res
        .headers()
        .get("x-request-id")
        .or_else(|| res.headers().get("X-Request-Id"));
    let s = rid.and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(
        !s.is_empty(),
        "expected non-empty x-request-id, headers={:?}",
        res.headers()
    );
}

/// `GET /api/pnl/symbols` — DB yolu yoksa veya hata olsa bile JSON gövde (smoke).
#[tokio::test]
async fn get_api_pnl_symbols_returns_200_json_with_symbols_key() {
    let app = test_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/pnl/symbols?mode=paper")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("valid json");
    assert!(
        v.get("symbols").is_some(),
        "expected symbols key, got: {}",
        v
    );
}

/// TFAI-Q05: hata JSON'unda `request_id` = `X-Request-Id` (tower-http).
#[tokio::test]
async fn api_error_json_request_id_matches_x_request_id_header() {
    let app = test_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/q-analiz/snapshot?tf=__invalid_tf__&symbol=ETHUSDT")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(res.status(), StatusCode::OK);
    let hdr: String = res
        .headers()
        .get("x-request-id")
        .or_else(|| res.headers().get("X-Request-Id"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_default();
    assert!(!hdr.is_empty(), "x-request-id header missing");
    let bytes = res
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(v.get("ok"), Some(&serde_json::json!(false)));
    let body_rid = v.get("request_id").and_then(|x| x.as_str()).unwrap_or("");
    assert_eq!(
        body_rid,
        hdr.as_str(),
        "request_id in JSON should match X-Request-Id header"
    );
}
