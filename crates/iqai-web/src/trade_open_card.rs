//! Pozisyon açıldı (CANLI POZİSYON) kartı – Telegram için PNG üretir.

use anyhow::Result;
use image::{ImageBuffer, ImageEncoder, Rgba};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use rusttype::{Font, Scale};

const W: u32 = 420;
const H: u32 = 320;
const HEADER_H: u32 = 40;
const ROW_H: u32 = 28;
const PAD: i32 = 14;

fn try_load_font() -> Option<Font<'static>> {
    let paths: Vec<std::path::PathBuf> = [
        std::env::var("IQAI_FONT_PATH").ok().map(std::path::PathBuf::from),
        Some(std::path::PathBuf::from("fonts/DejaVuSans.ttf")),
        Some(std::path::PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")),
        Some(std::path::PathBuf::from("/usr/share/fonts/TTF/DejaVuSans.ttf")),
        Some(std::path::PathBuf::from("C:\\Windows\\Fonts\\arial.ttf")),
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

fn strength_label(q_score: f64) -> &'static str {
    if q_score >= 90.0 {
        "ÇOK GÜÇLÜ"
    } else if q_score >= 80.0 {
        "GÜÇLÜ"
    } else if q_score >= 70.0 {
        "ORTA"
    } else if q_score >= 60.0 {
        "ZAYIF"
    } else {
        "ÇOK ZAYIF"
    }
}

fn bar_10(q_score: f64) -> u32 {
    ((q_score / 10.0).round() as i32).clamp(0, 10) as u32
}

fn pct(from: f64, to: f64) -> f64 {
    if from.abs() < 1e-12 {
        return 0.0;
    }
    (to - from) / from * 100.0
}

fn arrow(pct: f64) -> &'static str {
    if pct >= 0.0 {
        "▲"
    } else {
        "▼"
    }
}

/// CANLI POZİSYON kartını PNG bytes olarak döndürür.
pub fn render_trade_open_card(
    symbol: &str,
    side: &str,
    mode: &str,
    entry: f64,
    current_price: f64,
    stop_loss: f64,
    take_profit: f64,
    score: f64,
    rr: f64,
) -> Result<Vec<u8>> {
    let font = try_load_font().ok_or_else(|| {
        anyhow::anyhow!("Font bulunamadı (IQAI_FONT_PATH veya fonts/DejaVuSans.ttf)")
    })?;

    let scale_title = Scale::uniform(18.0);
    let scale_mid = Scale::uniform(15.0);
    let scale_small = Scale::uniform(13.0);

    let bg = Rgba([16, 18, 30, 255]);
    let mut img = ImageBuffer::from_pixel(W, H, bg);

    let header_bg = Rgba([9, 140, 220, 255]);
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(0, 0).of_size(W, HEADER_H),
        header_bg,
    );

    let cyan = Rgba([120, 245, 255, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let gray = Rgba([165, 170, 180, 255]);
    let green = Rgba([110, 231, 183, 255]);
    let red = Rgba([248, 113, 113, 255]);
    let yellow = Rgba([250, 220, 100, 255]);

    draw_text_mut(&mut img, cyan, PAD, 11, scale_title, &font, "CANLI POZİSYON");
    draw_text_mut(
        &mut img,
        white,
        PAD,
        (HEADER_H + 6) as i32,
        scale_mid,
        &font,
        &format!("{} · {}", symbol, side),
    );
    draw_text_mut(
        &mut img,
        gray,
        (W as i32) - PAD - 120,
        (HEADER_H + 6) as i32,
        scale_small,
        &font,
        mode,
    );

    let y_price = (HEADER_H + 30) as i32;
    let chg = pct(entry, current_price);
    let chg_color = if chg >= 0.0 { green } else { red };
    let price_text = format!("{:.2}", current_price);
    let change_text = format!("{} {:+.2}%", arrow(chg), chg);

    draw_text_mut(&mut img, white, PAD, y_price, scale_title, &font, &price_text);
    draw_text_mut(&mut img, chg_color, PAD, y_price + 24, scale_mid, &font, &change_text);

    let strength_points = bar_10(score);
    let bar: String = (0..10)
        .map(|i| if i < strength_points { "■" } else { "□" })
        .collect();
    let strength_txt = format!("Güç [{}] {}/10 {}", bar, strength_points, strength_label(score));
    let y_strength = y_price + 64;
    draw_text_mut(&mut img, yellow, PAD, y_strength, scale_mid, &font, &strength_txt);
    draw_text_mut(
        &mut img,
        gray,
        PAD,
        y_strength + 26,
        scale_small,
        &font,
        &format!("RR: {:.2}", rr),
    );

    let y_sl_label = y_strength + 50;
    draw_text_mut(&mut img, gray, PAD, y_sl_label, scale_small, &font, "Stop Loss");
    draw_text_mut(
        &mut img,
        red,
        (W as i32) / 2,
        y_sl_label,
        scale_mid,
        &font,
        &format!("{:.2}", stop_loss),
    );

    let y_tp_label = y_sl_label + ROW_H as i32;
    draw_text_mut(&mut img, gray, PAD, y_tp_label, scale_small, &font, "Take Profit");
    draw_text_mut(
        &mut img,
        green,
        (W as i32) / 2,
        y_tp_label,
        scale_mid,
        &font,
        &format!("{:.2}", take_profit),
    );

    let y_koruma = y_tp_label + ROW_H as i32;
    draw_text_mut(&mut img, gray, PAD, y_koruma, scale_small, &font, "Koruma");
    draw_text_mut(
        &mut img,
        white,
        (W as i32) - PAD - 70,
        y_koruma,
        scale_small,
        &font,
        "—",
    );

    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    encoder.write_image(img.as_raw(), W, H, image::ColorType::Rgba8)?;
    Ok(buf)
}
