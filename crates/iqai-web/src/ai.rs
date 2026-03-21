//! Q-Analiz tespitleri için AI yorumu (Ollama yerel).
//! Tespit olduğunda: tepe/dipte ne var, bundan sonra ne olabilir, formasyon tahmini.

use std::time::Duration;

/// Şablon sürümü (hash ile birlikte denetim için).
pub const PROMPT_TEMPLATE_VERSION_Q_ANALYSIS: &str = "q_analysis_interpret_v1";
/// Büyük resim raporu şablon sürümü.
pub const PROMPT_TEMPLATE_VERSION_BIG_PICTURE: &str = "big_picture_v4";

const PROMPT_PREFIX: &str = "Şu piyasa analizini 2-3 cümlede yorumla: Tepe/dipte ne tespit edilmiş, \
    hangi yapı/senaryo (Elliott, likidite, OB vb.) öne çıkıyor? \
    Birbiriyle çelişen cümleler kurma. Sadece Türkçe, kısa ve net. \
    Kesinlikle yapma: al/sat/emir önerisi; hedef fiyat, dolar tutarı veya yüzde getiri tahmini; \
    \"yatırım tavsiyesi\", \"pozisyon aç\" gibi ifadeler. Güven düşükse bunu açıkça belirt.\n\n";

/// Ollama’ya giden tam kullanıcı mesajı (hash = `iqai_core::sha256_hex(prompt.as_bytes())`).
pub fn build_q_analysis_prompt(context: &str) -> String {
    format!("{PROMPT_PREFIX}{context}")
}

/// Ollama `/api/generate` `system` alanı — Chat API’deki `system` mesajından daha sık uygulanır.
const BIG_PICTURE_SYSTEM: &str = "Sen IQAI için teknik analiz özetleyicisisin.\n\
ZORUNLU: Yanıtın tamamı Türkçe olacak. Tek bir İngilizce cümle bile yazma.\n\
Yasak kalıplar: Here's, Here are, Summary, The analysis, In conclusion, Investment advice, Hold, Wait, Recommended.\n\
Zaman dilimlerini Türkçe yaz: 5 dakika, 15 dakika, 1 saat, 4 saat, 1 gün.\n\
Markdown başlıkları Türkçe olsun (ör. **1 saat:**). Al/sat, emir, hedef fiyat veya yatırım tavsiyesi verme.";

/// Büyük resim: modele giden kullanıcı tarafı (veri + kısa görev).
fn build_big_picture_user_content(symbol: &str, context: &str) -> String {
    format!(
        "Sembol: {symbol}\n\n\
        Aşağıdaki blok, veritabanı snapshot’larından üretilmiş Türkçe etiketli teknik özet metnidir. \
        Bu metne dayanarak çoklu zaman dilimi büyük resmini anlat.\n\n\
        --- VERİ ---\n{}\n---\n\n\
        Çıktıyı yalnızca Türkçe üret.",
        context.trim()
    )
}

/// Denetim / `prompt_hash`: API’ye giden system+user birleşik metin (Ollama’ya parça parça gider).
pub fn build_big_picture_prompt(symbol: &str, context: &str) -> String {
    format!(
        "[OLLAMA system]\n{}\n\n[OLLAMA prompt]\n{}",
        BIG_PICTURE_SYSTEM,
        build_big_picture_user_content(symbol, context)
    )
}

/// İngilizce cevap tespiti (kaba — çeviri yedeği için).
fn big_picture_answer_likely_english(s: &str) -> bool {
    let t = s.to_lowercase();
    t.contains("here's a summary")
        || t.contains("here's ")
        || t.contains("here are")
        || t.contains("investment advice")
        || t.contains("the analysis suggests")
        || t.contains("based on this analysis")
        || t.contains("it's recommended")
        || t.contains("opportunity to enter")
        || (t.contains("the market is") && t.contains("timeframe"))
        || (t.contains("recommended") && t.contains("cautious"))
}

/// `POST /api/generate`, `stream: false` → `response` alanı.
async fn ollama_generate(
    base_url: &str,
    model: &str,
    system: &str,
    prompt: &str,
    timeout_secs: u64,
    num_predict: u32,
    temperature: f64,
) -> Option<String> {
    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .ok()?;
    let body = serde_json::json!({
        "model": model,
        "system": system,
        "prompt": prompt,
        "stream": false,
        "options": { "temperature": temperature, "num_predict": num_predict }
    });
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        log::debug!("Ollama /api/generate hatası: {}", resp.status());
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("response")?
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

async fn ollama_translate_big_picture_to_turkish(
    base_url: &str,
    model: &str,
    text: &str,
) -> Option<String> {
    let sys = "Profesyonel çevirmensin. Yalnızca Türkçe çıktı ver. Giriş veya açıklama ekleme.";
    let prompt = format!(
        "Aşağıdaki teknik analiz özetini eksiksiz Türkçeye çevir. Anlamı ve sayıları koru.\n\
        Yeni al/sat veya yatırım önerisi ekleme; varsa İngilizce öneri cümlelerini nötr açıklamaya çevir veya çıkar.\n\n\
        ---\n{}",
        text.trim()
    );
    ollama_generate(base_url, model, sys, &prompt, 120, 1024, 0.1).await
}

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
        "messages": [{ "role": "user", "content": build_q_analysis_prompt(context) }],
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

/// Çoklu timeframe snapshot özetinden AI büyük resim raporu.
/// Önce `/api/generate` + `system` (Türkçe zorunluluğu); çıktı hâlâ İngilizce kalıpları taşıyorsa tek geçiş Türkçe çeviri.
pub async fn interpret_big_picture(
    base_url: &str,
    model: &str,
    symbol: &str,
    context: &str,
) -> Option<String> {
    let user = build_big_picture_user_content(symbol, context);
    let primary = ollama_generate(
        base_url,
        model,
        BIG_PICTURE_SYSTEM,
        &user,
        120,
        900,
        0.15,
    )
    .await?;

    if big_picture_answer_likely_english(&primary) {
        log::info!(
            target: "iqai_web",
            "büyük resim AI çıktısı İngilizce kalıpları içeriyor; Türkçe çeviri yedeği deneniyor (model={})",
            model
        );
        if let Some(tr) = ollama_translate_big_picture_to_turkish(base_url, model, &primary).await {
            if !tr.is_empty() && tr.len() > 20 {
                return Some(tr);
            }
        }
    }
    Some(primary)
}

#[cfg(test)]
mod big_picture_heuristic_tests {
    use super::big_picture_answer_likely_english;

    #[test]
    fn detects_typical_english_big_picture() {
        let s = "Here's a summary of the technical analysis for ETHUSDT\n**Investment advice:** Hold";
        assert!(big_picture_answer_likely_english(s));
    }

    #[test]
    fn turkish_only_not_flagged() {
        let s = "Özet: 1 saat diliminde tepe bölgesi izleniyor. 5 dakikada yatay seyir.";
        assert!(!big_picture_answer_likely_english(s));
    }
}
