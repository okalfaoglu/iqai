//! Pozisyon açılışı ve saatlik **CANLI POZİSYON** kartı — Telegram için PNG üretir.
//!
//! Referans düzen: koyu kart, başlık, giriş→güncel fiyat, büyük PnL %, güç çubuğu, T/M/R, SL / TP / Koruma.

use anyhow::Result;
use image::{ImageBuffer, ImageEncoder, Rgba};
use imageproc::drawing::draw_text_mut;
use rusttype::{Font, Scale};

const W: u32 = 420;
const H: u32 = 460;
const PAD: i32 = 16;

/// Crate köküne göre `fonts/DejaVuSans.ttf` (CI / prod için dosyayı buraya koyun; bkz. `fonts/README.md`).
fn manifest_font_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fonts").join("DejaVuSans.ttf")
}

fn try_load_font() -> Option<Font<'static>> {
    let paths: Vec<std::path::PathBuf> = [
        Some(manifest_font_path()),
        std::env::var("IQAI_FONT_PATH").ok().map(std::path::PathBuf::from),
        Some(std::path::PathBuf::from("fonts/DejaVuSans.ttf")),
        Some(std::path::PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")),
        Some(std::path::PathBuf::from("/usr/share/fonts/TTF/DejaVuSans.ttf")),
        Some(std::path::PathBuf::from(
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        )),
        Some(std::path::PathBuf::from("C:\\Windows\\Fonts\\arial.ttf")),
        Some(std::path::PathBuf::from("C:\\Windows\\Fonts\\segoeui.ttf")),
    ]
    .into_iter()
    .flatten()
    .collect();

    for path in paths {
        if let Ok(font_data) = std::fs::read(&path) {
            if let Some(font) = Font::try_from_vec(font_data) {
                return Some(font);
            }
        }
    }
    None
}

/// 0–100 skor → 0–10 segment (güç çubuğu).
fn bar_10(q_score: f64) -> u32 {
    if !q_score.is_finite() {
        return 0;
    }
    ((q_score / 10.0).round() as i32).clamp(0, 10) as u32
}

fn strength_word_10(seg: u32) -> &'static str {
    match seg {
        0..=2 => "ÇOK ZAYIF",
        3..=4 => "ZAYIF",
        5..=6 => "ORTA",
        7..=8 => "GÜÇLÜ",
        _ => "ÇOK GÜÇLÜ",
    }
}

fn pct_move(entry: f64, current: f64) -> f64 {
    if entry.abs() < 1e-12 || !entry.is_finite() || !current.is_finite() {
        return 0.0;
    }
    (current - entry) / entry * 100.0
}

fn arrow(pct: f64) -> &'static str {
    if pct >= 0.0 {
        "▲"
    } else {
        "▼"
    }
}

fn fmt_price(p: f64) -> String {
    if !p.is_finite() {
        return "—".into();
    }
    if p >= 1000.0 {
        format!("{:.2}", p)
    } else if p >= 1.0 {
        format!("{:.4}", p)
    } else {
        format!("{:.6}", p)
    }
}

/// Skor + RR ile T/M/R satırı (TradeSignal’da ayrı alan yok; yaklaşık gösterim).
fn approximate_tmr(score: f64, rr: f64) -> (u32, u32, u32) {
    let sp = bar_10(score);
    let t = sp.saturating_mul(4).saturating_div(10).min(4);
    let m = sp.saturating_mul(3).saturating_div(10).min(3);
    let r = if rr >= 2.4 {
        3
    } else if rr >= 1.6 {
        2
    } else if rr >= 0.9 {
        1
    } else {
        0
    };
    (t, m, r)
}

fn draw_hline(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, y: u32, color: Rgba<u8>) {
    if y >= H {
        return;
    }
    for x in 0..W {
        img.put_pixel(x, y, color);
    }
}

/// CANLI POZİSYON / pozisyon açılışı kartı (PNG).
///
/// `stop_loss`: aktif SL (`current_sl`); `protection_sl`: Koruma seviyesi (genelde başlangıç SL / `initial_sl`).
pub fn render_trade_open_card(
    symbol: &str,
    side: &str,
    mode: &str,
    entry: f64,
    current_price: f64,
    stop_loss: f64,
    take_profit: f64,
    protection_sl: f64,
    score: f64,
    rr: f64,
) -> Result<Vec<u8>> {
    let font = try_load_font().ok_or_else(|| {
        anyhow::anyhow!(
            "Font bulunamadı: crates/iqai-web/fonts/DejaVuSans.ttf ekleyin, fonts-dejavu-core kurun veya IQAI_FONT_PATH ayarlayın (fonts/README.md)."
        )
    })?;

    let scale_title = Scale::uniform(17.0);
    let scale_big = Scale::uniform(26.0);
    let scale_mid = Scale::uniform(15.0);
    let scale_small = Scale::uniform(12.0);

    let bg = Rgba([5, 8, 22, 255]);
    let line = Rgba([55, 65, 81, 255]);
    let title_green = Rgba([110, 231, 183, 255]);
    let cyan = Rgba([0, 245, 255, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let gray = Rgba([156, 163, 175, 255]);
    let green = Rgba([110, 231, 183, 255]);
    let red = Rgba([248, 113, 113, 255]);
    let purple = Rgba([192, 132, 252, 255]);
    let orange_weak = Rgba([251, 146, 60, 255]);

    let mut img = ImageBuffer::from_pixel(W, H, bg);

    let mut y: i32 = 12;
    draw_text_mut(
        &mut img,
        title_green,
        PAD,
        y,
        scale_title,
        &font,
        "CANLI POZİSYON",
    );

    y += 28;
    draw_hline(&mut img, y as u32, line);
    y += 10;

    let sym_line = format!("{} · {}", symbol, side);
    draw_text_mut(&mut img, white, PAD, y, scale_mid, &font, &sym_line);
    let mode_w = (mode.len() as f32 * 7.0) as i32;
    draw_text_mut(
        &mut img,
        gray,
        (W as i32) - PAD - mode_w.max(40),
        y,
        scale_small,
        &font,
        mode,
    );

    y += 30;
    let price_row = format!(
        "{} ————▶ {}",
        fmt_price(entry),
        fmt_price(current_price)
    );
    draw_text_mut(&mut img, gray, PAD, y, scale_mid, &font, &price_row);

    y += 34;
    let chg = pct_move(entry, current_price);
    let chg_color = if chg >= 0.0 { green } else { red };
    let big_pct = format!("{} {:+.2}%", arrow(chg), chg);
    draw_text_mut(&mut img, chg_color, PAD, y, scale_big, &font, &big_pct);

    let strength_points = bar_10(score);
    let bar: String = (0..10)
        .map(|i| if i < strength_points { '■' } else { '□' })
        .collect();
    let bar_color = if strength_points < 5 {
        orange_weak
    } else {
        cyan
    };
    let word = strength_word_10(strength_points);
    let strength_txt = format!(
        "Güç [{}] {}/10 {}",
        bar, strength_points, word
    );
    y += 40;
    draw_text_mut(&mut img, bar_color, PAD, y, scale_mid, &font, &strength_txt);

    let (t, m, r) = approximate_tmr(score, rr);
    let tmr = format!("T:{}/4 · M:{}/3 · R:{}/3", t, m, r);
    y += 26;
    draw_text_mut(&mut img, gray, PAD, y, scale_small, &font, &tmr);

    y += 22;
    draw_hline(&mut img, y as u32, line);
    y += 14;

    // Stop Loss satırı (etiket + büyük rakam)
    draw_text_mut(&mut img, gray, PAD, y, scale_small, &font, "Stop Loss");
    let sls = fmt_price(stop_loss);
    draw_text_mut(
        &mut img,
        red,
        (W as i32) / 2 - 20,
        y - 2,
        scale_mid,
        &font,
        &sls,
    );

    y += 36;
    draw_text_mut(&mut img, gray, PAD, y, scale_small, &font, "Take Profit");
    let tps = fmt_price(take_profit);
    draw_text_mut(
        &mut img,
        green,
        (W as i32) / 2 - 20,
        y - 2,
        scale_mid,
        &font,
        &tps,
    );

    y += 36;
    draw_text_mut(&mut img, gray, PAD, y, scale_small, &font, "Koruma");
    let koruma_s = if protection_sl.abs() > 1e-12 && protection_sl.is_finite() {
        fmt_price(protection_sl)
    } else {
        "—".into()
    };
    draw_text_mut(
        &mut img,
        purple,
        (W as i32) / 2 - 20,
        y - 2,
        scale_mid,
        &font,
        &koruma_s,
    );

    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    encoder.write_image(img.as_raw(), W, H, image::ColorType::Rgba8)?;
    Ok(buf)
}

/// PNG üretilemezse Telegram HTML yedeği (parse_mode=HTML).
pub fn format_live_position_html(
    symbol: &str,
    side: &str,
    entry: f64,
    current_price: f64,
    stop_loss: f64,
    take_profit: f64,
    protection_sl: f64,
    score: f64,
    rr: f64,
) -> String {
    let chg = pct_move(entry, current_price);
    let sp = bar_10(score);
    let word = strength_word_10(sp);
    let (t, m, r) = approximate_tmr(score, rr);
    let bar: String = (0..10)
        .map(|i| if i < sp { '█' } else { '░' })
        .collect();
    let koruma = if protection_sl.abs() > 1e-12 && protection_sl.is_finite() {
        fmt_price(protection_sl)
    } else {
        "—".into()
    };
    format!(
        "<b>CANLI POZİSYON</b>\n\
         <b>{sym}</b> · <b>{side}</b>\n\
         Giriş → Güncel: <code>{e}</code> → <code>{c}</code>\n\
         {arr} <b>{chg:+.2}%</b>\n\
         Güç [{bar}] {sp}/10 <b>{word}</b>\n\
         <i>T:{t}/4 · M:{m}/3 · R:{r}/3</i>\n\
         ─────────────\n\
         <b>Stop Loss</b> <code>{sl}</code>\n\
         <b>Take Profit</b> <code>{tp}</code>\n\
         <b>Koruma</b> <code>{koruma}</code>\n\
         <i>RR {rr:.2}</i>",
        sym = html_escape(symbol),
        side = html_escape(side),
        e = fmt_price(entry),
        c = fmt_price(current_price),
        arr = arrow(chg),
        sl = fmt_price(stop_loss),
        tp = fmt_price(take_profit),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_fallback_has_sections() {
        let s = format_live_position_html(
            "ETHUSDT",
            "LONG",
            2182.87,
            2145.42,
            2005.9,
            2263.68,
            2005.9,
            40.0,
            1.5,
        );
        assert!(s.contains("CANLI POZİSYON"));
        assert!(s.contains("ETHUSDT"));
        assert!(s.contains("Koruma"));
    }
}
