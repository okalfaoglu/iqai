//! TradingView veri çekme – saf Rust WebSocket (Python/HTTP yok).
//! Protokol: wss://data.tradingview.com/socket.io/websocket, ~m~len~m~json mesajları.

use futures_util::{SinkExt, StreamExt};
use iqai_core::types::{Candle, Timeframe};
use rand::Rng;
use serde_json::json;
use std::time::Duration;
use http::header::HeaderValue;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};

const TV_WS_URL: &str = "wss://data.tradingview.com/socket.io/websocket";
const ORIGIN: &str = "https://data.tradingview.com";

/// Gelen ham metinden ~m~ bloklarını çıkarıp her birinde "m" (metod) varsa loglar; (symbol_resolved, symbol_error, series_*) döner.
fn log_tv_frames(raw_chunk: &str) -> (bool, bool) {
    let mut has_resolved = false;
    let mut has_symbol_error = false;
    let mut pos = 0;
    while let Some(open) = raw_chunk[pos..].find("~m~") {
        let start = pos + open + 3;
        let rest = &raw_chunk[start..];
        let len_str = match rest.find("~m~") {
            Some(i) => rest.get(..i).unwrap_or(""),
            None => break,
        };
        let len: usize = len_str.trim().parse().unwrap_or(0);
        let body_start = start + len_str.len() + 3;
        let body = raw_chunk.get(body_start..body_start + len).unwrap_or("");
        pos = body_start + len;
        if body.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
            let m = v.get("m").and_then(|x| x.as_str()).unwrap_or("");
            let p_preview = v.get("p").map(|p| {
                let s = p.to_string();
                if s.len() > 120 { format!("{}...", &s[..120]) } else { s }
            }).unwrap_or_else(|| "—".to_string());
            eprintln!("[TV] < {} p:{}", m, p_preview);
            match m {
                "symbol_resolved" => has_resolved = true,
                "symbol_error" => has_symbol_error = true,
                _ => {}
            }
        }
    }
    (has_resolved, has_symbol_error)
}

fn gen_session(prefix: &str) -> String {
    let s: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect::<String>()
        .to_lowercase();
    format!("{}{}", prefix, s)
}

fn tv_message(func: &str, params: serde_json::Value) -> String {
    let body = json!({"m": func, "p": params});
    let body_str = body.to_string();
    format!("~m~{}~m~{}", body_str.len(), body_str)
}

fn timeframe_to_interval(tf: Timeframe) -> &'static str {
    match tf {
        Timeframe::M1 => "1",
        Timeframe::M5 => "5",
        Timeframe::M15 => "15",
        Timeframe::M30 => "30",
        Timeframe::H1 => "1H",
        Timeframe::H4 => "4H",
        Timeframe::D1 => "1D",
    }
}

/// TV için sembol: vadeli kodu → perpetual (ETHUSDT27H2026 → ETHUSDT); Binance için .P eklenir.
fn format_symbol(symbol: &str, exchange: &str) -> String {
    let sym = symbol.trim();
    if sym.contains(':') {
        return sym.to_string();
    }
    // Vadeli: ETHUSDT27H2026 → base = ETHUSDT (slice ..off+4 zaten USDT ile bitiyor, tekrar USDT ekleme)
    let base = if let Some(off) = sym.find("USDT") {
        let after = sym.get(off + 4..).unwrap_or("");
        if !after.is_empty() && after.chars().any(|c| c.is_ascii_digit()) {
            sym.get(..off + 4).unwrap_or(sym).to_string()
        } else {
            sym.to_string()
        }
    } else {
        sym.to_string()
    };
    // Binance'te TV perpetual için BINANCE:ETHUSDT.P kullanılıyor
    let ticker = if exchange.eq_ignore_ascii_case("BINANCE") && base.contains("USDT") && !base.ends_with(".P") {
        format!("{}.P", base)
    } else {
        base
    };
    format!("{}:{}", exchange, ticker)
}

/// Native Rust ile TradingView'dan tarihsel mum çeker (WebSocket).
pub async fn fetch_klines_native(
    symbol: &str,
    exchange: &str,
    interval: Timeframe,
    n_bars: u32,
) -> Result<Vec<Candle>, String> {
    let n_bars = n_bars.min(5000).max(1);
    let sym = format_symbol(symbol, exchange);
    let interval_str = timeframe_to_interval(interval);
    // TV WebSocket: 3. parametre "=" ile başlayan string olmalı (çalışan Python örnekleriyle uyumlu)
    let resolve_param = format!(r#"={{"symbol":"{}","adjustment":"splits"}}"#, sym);

    eprintln!("[TV] istek symbol={:?} exchange={:?} -> sym={:?} interval={} n_bars={}", symbol, exchange, sym, interval_str, n_bars);
    eprintln!("[TV] resolve_param={}", resolve_param);

    let mut req = TV_WS_URL
        .into_client_request()
        .map_err(|e| e.to_string())?;
    req.headers_mut()
        .insert("Origin", HeaderValue::from_static(ORIGIN));

    let ws_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
        max_message_size: Some(64 << 20),
        ..Default::default()
    };
    let (ws_stream, _) = connect_async_tls_with_config(req, Some(ws_config), false, None)
    .await
    .map_err(|e| e.to_string())?;

    let (mut write, mut read) = ws_stream.split();

    let chart_session = gen_session("cs_");
    let token = "unauthorized_user_token";

    // 1) Önce oturum aç + sembol çöz; symbol_resolved gelene kadar bekle (veya symbol_error)
    let msg_auth = tv_message("set_auth_token", json!([token]));
    let msg_session = tv_message("chart_create_session", json!([chart_session, ""]));
    let msg_resolve = tv_message("resolve_symbol", json!([chart_session, "symbol_1", resolve_param.clone()]));

    for (name, m) in [("set_auth_token", &msg_auth), ("chart_create_session", &msg_session), ("resolve_symbol", &msg_resolve)] {
        eprintln!("[TV] > {}", name);
        write.send(Message::Text(m.clone())).await.map_err(|e| e.to_string())?;
    }

    let mut raw = String::new();
    let timeout = Duration::from_secs(15);
    let mut symbol_resolved_ok = false;
    loop {
        let msg = match tokio::time::timeout(timeout, read.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => t,
            Ok(Some(Ok(Message::Close(_)))) => break,
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => return Err(e.to_string()),
            Ok(None) => break,
            Err(_) => return Err("TV: symbol_resolved/symbol_error beklerken zaman aşımı".to_string()),
        };
        raw.push_str(&msg);
        raw.push('\n');
        let (has_resolved, has_err) = log_tv_frames(&msg);
        if has_err || has_symbol_error_in_raw(&msg) {
            return Err("TV: sembol geçersiz. Piyasa: Futures seçip Binance API ile veri alın.".to_string());
        }
        if has_resolved {
            symbol_resolved_ok = true;
            eprintln!("[TV] symbol_resolved alındı, create_series gönderiliyor");
            break;
        }
    }
    if !symbol_resolved_ok {
        return Err("TV: sembol çözülemedi (bağlantı kapandı veya zaman aşımı). Piyasa: Futures seçip Binance API ile veri alın.".to_string());
    }

    // 2) Seri oluştur + timezone; series_completed veya bar verisi gelene kadar oku
    let msg_series = tv_message("create_series", json!([chart_session, "s1", "s1", "symbol_1", interval_str, n_bars]));
    let msg_tz = tv_message("switch_timezone", json!([chart_session, "exchange"]));
    eprintln!("[TV] > create_series (interval={} n_bars={})", interval_str, n_bars);
    write.send(Message::Text(msg_series)).await.map_err(|e| e.to_string())?;
    eprintln!("[TV] > switch_timezone");
    write.send(Message::Text(msg_tz)).await.map_err(|e| e.to_string())?;
    drop(write);

    let timeout2 = Duration::from_secs(25);
    while let Ok(Some(Ok(msg))) = tokio::time::timeout(timeout2, read.next()).await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        raw.push_str(&text);
        raw.push('\n');
        log_tv_frames(&text);
        if raw.contains("series_completed") {
            break;
        }
    }

    if raw.contains("symbol_error") || raw.contains("invalid symbol") {
        return Err("TV: sembol geçersiz. Piyasa: Futures seçip Binance API ile veri alın.".to_string());
    }
    if raw.contains("series_error") && !raw.contains("\"v\":") && !raw.contains("\"t\":") {
        return Err("TV: seri hatası. Piyasa: Futures seçip Binance API ile veri alın.".to_string());
    }
    parse_bars_from_response(&raw)
}

fn has_symbol_error_in_raw(s: &str) -> bool {
    s.contains("symbol_error") || s.contains("invalid symbol")
}

/// TV yanıtındaki bar serisini çıkarır.
/// Desteklenen formatlar:
/// 1) "v":[ts, o, h, l, c, volume] tekrarlayan bloklar (eski tvDatafeed)
/// 2) Paralel diziler: "t" veya "time":[...], "o":[...], "h":[...], "l":[...], "c":[...], "v":[...]
fn parse_bars_from_response(raw: &str) -> Result<Vec<Candle>, String> {
    let mut candles = parse_bars_v_arrays(raw);
    if candles.is_empty() {
        candles = parse_bars_data_array_of_arrays(raw);
    }
    if candles.is_empty() {
        candles = parse_bars_parallel_arrays(raw).unwrap_or_else(|e| {
            eprintln!("[TV] Parse uyarısı: {}. Ham yanıt (ilk 1200 karakter): {:?}", e, &raw.get(..1200.min(raw.len())).unwrap_or(raw));
            vec![]
        });
    }
    if candles.is_empty() {
        return Err("yanıtta bar verisi bulunamadı (TV formatı değişmiş olabilir)".to_string());
    }
    candles.sort_by_key(|c| c.time);
    Ok(candles)
}

/// "data":[[t,o,h,l,c,v],[t,o,h,l,c,v],...] formatı (TV bazen bu yapıyı kullanır)
fn parse_bars_data_array_of_arrays(raw: &str) -> Vec<Candle> {
    let re = match regex::Regex::new(
        r#"\[([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*\]"#,
    ) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    const MIN_MS: i64 = 1_000_000_000_000; // ~2001
    const MAX_MS: i64 = 2_100_000_000_000; // ~2036
    re.captures_iter(raw)
        .filter_map(|cap| {
            let ts: f64 = cap.get(1)?.as_str().parse().ok()?;
            let time_ms = if ts >= 1e12 { ts as i64 } else { (ts * 1000.0) as i64 };
            if time_ms < MIN_MS || time_ms > MAX_MS {
                return None;
            }
            let open: f64 = cap.get(2)?.as_str().parse().ok()?;
            let high: f64 = cap.get(3)?.as_str().parse().ok()?;
            let low: f64 = cap.get(4)?.as_str().parse().ok()?;
            let close: f64 = cap.get(5)?.as_str().parse().ok()?;
            let volume: f64 = cap.get(6)?.as_str().parse().unwrap_or(0.0);
            Some(Candle { time: time_ms, open, high, low, close, volume })
        })
        .collect()
}

/// Eski format: "v":[ts, o, h, l, c, volume] blokları
fn parse_bars_v_arrays(raw: &str) -> Vec<Candle> {
    let re = match regex::Regex::new(
        r#""v":\s*\[\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*\]"#,
    ) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    re.captures_iter(raw)
        .filter_map(|cap| {
            let ts: f64 = cap.get(1)?.as_str().parse().ok()?;
            let time_ms = (ts * 1000.0) as i64;
            let open: f64 = cap.get(2)?.as_str().parse().ok()?;
            let high: f64 = cap.get(3)?.as_str().parse().ok()?;
            let low: f64 = cap.get(4)?.as_str().parse().ok()?;
            let close: f64 = cap.get(5)?.as_str().parse().ok()?;
            let volume: f64 = cap.get(6)?.as_str().parse().unwrap_or(0.0);
            Some(Candle {
                time: time_ms,
                open,
                high,
                low,
                close,
                volume,
            })
        })
        .collect()
}

/// Paralel dizi formatı: "t":[ts,...], "o":[...], "h":[...], "l":[...], "c":[...], "v":[...]
fn parse_bars_parallel_arrays(raw: &str) -> Result<Vec<Candle>, String> {
    fn extract_array(s: &str, key: &str) -> Option<Vec<f64>> {
        let pattern = format!(r#""{}"\s*:\s*\["#, key);
        let start = s.find(&pattern)?;
        let bracket_start = start + pattern.len() - 1;
        let mut depth = 0u32;
        let mut begin = None;
        for (i, c) in s[bracket_start..].chars().enumerate() {
            match c {
                '[' => {
                    depth += 1;
                    if depth == 1 {
                        begin = Some(bracket_start + i + 1);
                    }
                }
                ']' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        let end = bracket_start + i;
                        let slice = s.get(begin?..end)?;
                        return slice
                            .split(',')
                            .map(|x| x.trim().trim_matches('"').parse::<f64>().unwrap_or(0.0))
                            .collect::<Vec<_>>()
                            .into();
                    }
                }
                _ => {}
            }
        }
        None
    }
    // TV bazen "t" bazen "time" kullanıyor; önce "time" dene (iç içe objede "t" başka anlama gelebilir)
    let t = extract_array(raw, "time")
        .or_else(|| extract_array(raw, "t"))
        .ok_or("paralel dizi 't'/'time' bulunamadı")?;
    let o = extract_array(raw, "o").ok_or("paralel dizi 'o' bulunamadı")?;
    let h = extract_array(raw, "h").ok_or("paralel dizi 'h' bulunamadı")?;
    let l = extract_array(raw, "l").ok_or("paralel dizi 'l' bulunamadı")?;
    let c = extract_array(raw, "c").ok_or("paralel dizi 'c' bulunamadı")?;
    let v = extract_array(raw, "v").unwrap_or_else(|| vec![0.0; t.len()]);
    let len = t.len().min(o.len()).min(h.len()).min(l.len()).min(c.len()).min(v.len());
    if len == 0 {
        return Err("bar sayısı 0".to_string());
    }
    let candles: Vec<Candle> = (0..len)
        .map(|i| {
            let time_ms = (t[i] * 1000.0) as i64;
            Candle {
                time: time_ms,
                open: o[i],
                high: h[i],
                low: l[i],
                close: c[i],
                volume: v.get(i).copied().unwrap_or(0.0),
            }
        })
        .collect();
    Ok(candles)
}
