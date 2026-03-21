//! Web API JSON hata şeması: tüm uç noktalarda `ok: false` + `error` (string) tutarlılığı.
//! İsteğe bağlı `request_id` — HTTP `X-Request-Id` ile aynı değer (`tower-http`).
//!
//! **Not:** `json!` yalnızca bu dosyadaki `#[cfg(test)]` bloğunda `use serde_json::json` ile
//! kullanılır; lib kökünde `use serde_json::{json, Value}` yapmayın — release build’de
//! `unused import: json` uyarısı üretir.

use serde_json::Value;

/// Sadece hata mesajı (`ok: false`, `error`).
pub fn api_error(message: impl Into<String>) -> Value {
    api_error_with_request_id(message, None)
}

/// Hata + isteğe bağlı `request_id` (gövde; başlıkla aynı korelasyon).
pub fn api_error_with_request_id(message: impl Into<String>, request_id: Option<&str>) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("ok".to_string(), false.into());
    m.insert("error".to_string(), message.into().into());
    if let Some(r) = request_id.filter(|s| !s.is_empty()) {
        m.insert("request_id".to_string(), r.to_string().into());
    }
    Value::Object(m)
}

/// Hata + ek alanlar (ör. `"symbols": []`, `"detections": []`).
pub fn api_error_with_extras(message: impl Into<String>, extra: Value) -> Value {
    api_error_with_extras_and_request_id(message, extra, None)
}

/// Hata + ek alanlar + isteğe bağlı `request_id`.
pub fn api_error_with_extras_and_request_id(
    message: impl Into<String>,
    extra: Value,
    request_id: Option<&str>,
) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("ok".to_string(), false.into());
    m.insert("error".to_string(), message.into().into());
    if let Some(r) = request_id.filter(|s| !s.is_empty()) {
        m.insert("request_id".to_string(), r.to_string().into());
    }
    if let Value::Object(map) = extra {
        m.extend(map);
    }
    Value::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn error_with_request_id_field() {
        let v = api_error_with_request_id("bad", Some("abc-123"));
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "bad");
        assert_eq!(v["request_id"], "abc-123");
    }

    #[test]
    fn extras_with_request_id() {
        let v = api_error_with_extras_and_request_id(
            "e",
            json!({ "symbols": [] }),
            Some("rid"),
        );
        assert_eq!(v["ok"], false);
        assert_eq!(v["request_id"], "rid");
        assert_eq!(v["symbols"], json!([]));
    }
}
