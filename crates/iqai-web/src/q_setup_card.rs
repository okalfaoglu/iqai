//! Q-Setup kartı görseli – Telegram için PNG üretir (ekteki kart formatı benzeri).

use anyhow::Result;
use image::{ImageBuffer, ImageEncoder, Rgba};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use iqai_core::{QSetup, SignalType};
use rusttype::{Font, Scale};

// Ekran görüntüsüne yakın oran: ~680x200
const W: u32 = 680;
const H: u32 = 200;
const HEADER_H: u32 = 40;
const ROW_H: u32 = 32;
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

fn pct(from: f64, to: f64) -> Option<f64> {
    if from.abs() < 1e-12 {
        return None;
    }
    Some((to - from) / from * 100.0)
}

fn arrow(pct: f64) -> &'static str {
    if pct >= 0.0 { "▲" } else { "▼" }
}

/// Q-Setup kartını PNG bytes olarak döndürür.
///
/// `current_price` yoksa entry referans alınır (Telegram kartı yine üretilebilir).
pub fn render_q_setup_card(setup: &QSetup, current_price: Option<f64>) -> Result<Vec<u8>> {
    let font = try_load_font()
        .ok_or_else(|| anyhow::anyhow!("Font bulunamadı (IQAI_FONT_PATH veya fonts/DejaVuSans.ttf)"))?;

    let scale_small = Scale::uniform(14.0);
    let scale_title = Scale::uniform(18.0);

    let bg = Rgba([28, 30, 35, 255]);
    let mut img = ImageBuffer::from_pixel(W, H, bg);

    // Header blue (ekrandaki tona yakın)
    let blue = Rgba([70, 90, 255, 255]);
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(0, 0).of_size(W / 2, HEADER_H),
        blue,
    );
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at((W / 2) as i32, 0).of_size(W / 2, HEADER_H),
        blue,
    );

    let white = Rgba([255, 255, 255, 255]);
    let gray = Rgba([165, 170, 180, 255]);
    let green = Rgba([110, 231, 183, 255]);
    let red = Rgba([248, 113, 113, 255]);
    let yellow = Rgba([250, 220, 100, 255]);

    let header_left = format!("{} (Q)", setup.symbol);
    draw_text_mut(&mut img, white, 16, 11, scale_title, &font, &header_left);
    draw_text_mut(&mut img, white, (W / 2) as i32 + 16, 11, scale_title, &font, "DİNAMİK MOD");

    // Grid lines (subtle)
    let grid = Rgba([55, 58, 66, 255]);
    for i in 0..=4 {
        let y = HEADER_H as i32 + (i as i32) * ROW_H as i32;
        draw_filled_rect_mut(
            &mut img,
            imageproc::rect::Rect::at(0, y).of_size(W, 1),
            grid,
        );
    }
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at((W / 2) as i32, HEADER_H as i32).of_size(1, H - HEADER_H),
        grid,
    );

    let y0 = HEADER_H as i32 + 8;
    let mut y = y0;

    let price = current_price.unwrap_or(setup.entry);
    let side_long = matches!(setup.side, SignalType::Buy | SignalType::ChochBuy | SignalType::BosBuy);
    let side_str = if side_long { "LONG" } else { "SHORT" };

    // Left labels
    // Güncel fiyat + entry'e göre % değişim (ekrandaki gibi)
    draw_text_mut(&mut img, gray, PAD, y, scale_small, &font, "Güncel Fiyat:");
    let chg = pct(setup.entry, price).unwrap_or(0.0);
    let price_color = if chg >= 0.0 { green } else { red };
    let price_text = format!("{:.2}  {} {:+.2}%", price, arrow(chg), chg);
    draw_text_mut(&mut img, price_color, (W / 2) as i32 + PAD, y, scale_small, &font, &price_text);
    y += ROW_H as i32;

    draw_text_mut(&mut img, gray, PAD, y, scale_small, &font, "Durum:");
    let b = bar_10(setup.q_score);
    let bar: String = (0..10).map(|i| if i < b { "■" } else { "□" }).collect();
    let status = format!("{} [{}] {}/10 {}", side_str, bar, b, strength_label(setup.q_score));
    draw_text_mut(&mut img, green, (W / 2) as i32 + PAD, y, scale_small, &font, &status);
    y += ROW_H as i32;

    draw_text_mut(&mut img, gray, PAD, y, scale_small, &font, "Giriş (Ort):");
    draw_text_mut(&mut img, yellow, (W / 2) as i32 + PAD, y, scale_small, &font, &format!("{:.2}", setup.entry));
    y += ROW_H as i32;

    draw_text_mut(&mut img, gray, PAD, y, scale_small, &font, "Stop (SL):");
    // SL yüzdesi entry bazlı (ekrandaki gibi)
    let sl_pct = pct(setup.entry, setup.stop_loss).unwrap_or(0.0);
    let sl_text = format!("{:.2} ({:+.2}%)", setup.stop_loss, sl_pct);
    draw_text_mut(&mut img, red, (W / 2) as i32 + PAD, y, scale_small, &font, &sl_text);
    y += ROW_H as i32;

    draw_text_mut(&mut img, gray, PAD, y, scale_small, &font, "Kar Al (TP):");
    // TP yüzdesi entry bazlı (ekrandaki gibi)
    let tp_pct = pct(setup.entry, setup.take_profit).unwrap_or(0.0);
    let tp_text = format!("{:.2} ({:+.2}%)", setup.take_profit, tp_pct);
    draw_text_mut(&mut img, green, (W / 2) as i32 + PAD, y, scale_small, &font, &tp_text);

    // Encode PNG
    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    encoder.write_image(img.as_raw(), W, H, image::ColorType::Rgba8)?;
    Ok(buf)
}

