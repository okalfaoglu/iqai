//! Q-Analiz tespitleri için AI yorumu (Ollama yerel).
//! Tespit olduğunda: tepe/dipte ne var, bundan sonra ne olabilir, formasyon tahmini.

use std::time::Duration;

const PROMPT_PREFIX: &str = "Şu piyasa analizini 2-3 cümlede yorumla: Tepe/dipte ne tespit edilmiş, \
    bundan sonra fiyat için ne beklenebilir, hangi Elliott formasyonu veya senaryo olası? \
    Sadece Türkçe, kısa ve net yanıt ver. Yatırım tavsiyesi verme.\n\n";

/// Ollama servisinin erişilebilir olup olmadığını kontrol eder.
/// GET {base_url}/api/tags ile liste alınır; 200 ise kurulum tamam sayılır.
/// Döner: (erişilebilir: bool, yüklü modeller: Vec<String>)
pub async fn check_ollama(base_url: &str) -> (bool, Vec<String>) {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return (false, vec![]),
    };
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            log::debug!("Ollama bağlantı hatası: {}", e);
            return (false, vec![]);
        }
    };
    if !resp.status().is_success() {
        return (false, vec![]);
    }
    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(_) => return (true, vec![]),
    };
    let models = json
        .get("models")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (true, models)
}

/// Ollama (yerel) ile kısa Türkçe yorum.
pub async fn interpret_q_analysis(
    base_url: &str,
    model: &str,
    context: &str,
) -> Option<String> {
    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .ok()?;
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": format!("{}{}", PROMPT_PREFIX, context) }],
        "stream": false,
        "options": { "temperature": 0.3, "num_predict": 300 }
    });
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        log::debug!("Ollama yanıt hatası: {}", resp.status());
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("message")?
        .get("content")?
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

const BIG_PICTURE_PREFIX: &str = "Aşağıda bir kripto/forex sembolü için tüm zaman dilimlerinde (5m, 15m, 1h, 4h, 1d) \
    toplanan teknik analiz + pozisyon metrikleri özeti var. Büyük resimde: Hangi timeframe'de trend ne yönde, \
    dip/tepe sinyalleri tutarlı mı, metrik gücü (TSK), volatilite, momentum ve RR ne söylüyor; \
    alım/satım için hangi senaryo daha güçlü? Kısa paragraflar halinde (3-5 cümle) Türkçe özet yaz. Yatırım tavsiyesi verme.\n\n";

/// Çoklu timeframe snapshot özetinden AI büyük resim raporu.
pub async fn interpret_big_picture(
    base_url: &str,
    model: &str,
    symbol: &str,
    context: &str,
) -> Option<String> {
    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .ok()?;
    let content = format!("{}{}\n\nSembol: {}", BIG_PICTURE_PREFIX, context, symbol);
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": content }],
        "stream": false,
        "options": { "temperature": 0.3, "num_predict": 500 }
    });
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        log::debug!("Ollama büyük resim yanıt hatası: {}", resp.status());
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("message")?
        .get("content")?
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
