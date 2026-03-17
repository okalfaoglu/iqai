//! Q-Analiz tespit kartı görseli – Telegram için PNG üretir (ekteki kart formatı).

use anyhow::Result;
use image::{ImageBuffer, ImageEncoder, Rgba};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use iqai_core::QRadarOpportunityAnalysis;
use rusttype::{Font, Scale};

const W: u32 = 420;
const H: u32 = 320;
const HEADER_H: u32 = 36;
const ROW_H: u32 = 28;
const PAD: i32 = 12;
const LEFT_COL_W: i32 = 140;

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

/// Ekteki kart formatında PNG bytes döndürür. Font bulunamazsa Err (metin bildirimi kullanılır).
pub fn render_q_analiz_card(opp: &QRadarOpportunityAnalysis) -> Result<Vec<u8>> {
    let font = try_load_font().ok_or_else(|| anyhow::anyhow!("Font bulunamadı (IQAI_FONT_PATH veya fonts/DejaVuSans.ttf)"))?;
    let scale_small = Scale::uniform(12.0);
    let scale_title = Scale::uniform(14.0);

    let mut img = ImageBuffer::from_pixel(W, H, Rgba([40, 42, 52, 255]));
    let blue = Rgba([41, 98, 255, 255]);
    let red = Rgba([180, 50, 50, 255]);
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(0, 0).of_size(W / 2, HEADER_H),
        blue,
    );
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at((W / 2) as i32, 0).of_size(W / 2, HEADER_H),
        red,
    );
    let white = Rgba([255, 255, 255, 255]);
    draw_text_mut(&mut img, white, 8, 10, scale_title, &font, &format!("{} (Q)", opp.symbol));
    draw_text_mut(&mut img, white, (W / 2) as i32 + 8, 10, scale_title, &font, "RADAR");

    let y_start = HEADER_H as i32 + 8;
    let gray_label = Rgba([160, 165, 175, 255]);
    let yellow = Rgba([250, 220, 100, 255]);
    let green = Rgba([110, 231, 183, 255]);

    let price_str = format!("{:.2}", opp.reference_price);
    let dir_str = if opp.direction.is_empty() { "—" } else { opp.direction.as_str() };
    let detection = if opp.detection.is_empty() { "—" } else { opp.detection.as_str() };
    let conf_10 = (opp.confidence_score.round() as i32).clamp(0, 10) as u32;
    let early_10 = (opp.early_warning_score.round() as i32).clamp(0, 10) as u32;
    let early_label = if opp.direction == "LONG" {
        format!("DİP {}/10", early_10)
    } else if opp.direction == "SHORT" {
        format!("TEPE {}/10", early_10)
    } else {
        format!("{}/10", early_10)
    };
    let rec = if opp.recommendation.is_empty() { "—" } else { opp.recommendation.as_str() };
    let layers = opp.confirmation_layers.as_deref().unwrap_or("");
    let bar: String = (0..10).map(|i| if i < conf_10 { "■" } else { "□" }).collect();

    let mut y = y_start;
    draw_text_mut(&mut img, gray_label, PAD, y, scale_small, &font, "Fiyat:");
    draw_text_mut(&mut img, white, LEFT_COL_W, y, scale_small, &font, &price_str);
    y += ROW_H as i32;
    draw_text_mut(&mut img, gray_label, PAD, y, scale_small, &font, "YÖN:");
    draw_text_mut(&mut img, white, LEFT_COL_W, y, scale_small, &font, dir_str);
    y += ROW_H as i32;
    draw_text_mut(&mut img, gray_label, PAD, y, scale_small, &font, "Tespit:");
    draw_text_mut(&mut img, yellow, LEFT_COL_W, y, scale_small, &font, detection);
    y += ROW_H as i32;
    draw_text_mut(&mut img, gray_label, PAD, y, scale_small, &font, "Güven:");
    draw_text_mut(&mut img, green, LEFT_COL_W, y, scale_small, &font, &format!("{} {}/10", bar, conf_10));
    y += ROW_H as i32;
    draw_text_mut(&mut img, gray_label, PAD, y, scale_small, &font, "Erken Uyarı:");
    draw_text_mut(&mut img, yellow, LEFT_COL_W, y, scale_small, &font, &early_label);
    y += ROW_H as i32;
    draw_text_mut(&mut img, gray_label, PAD, y, scale_small, &font, "Tavsiye:");
    draw_text_mut(&mut img, yellow, LEFT_COL_W, y, scale_small, &font, rec);
    y += ROW_H as i32;
    if !layers.is_empty() {
        draw_text_mut(&mut img, gray_label, PAD, y, scale_small, &font, "Onay:");
        draw_text_mut(&mut img, white, LEFT_COL_W, y, scale_small, &font, layers);
        y += ROW_H as i32;
    }
    if let Some(ref ds) = opp.discrete_score {
        let score_line = format!("{} /10 · {}", ds.total, ds.recommendation);
        draw_text_mut(&mut img, gray_label, PAD, y, scale_small, &font, "Skor:");
        draw_text_mut(&mut img, green, LEFT_COL_W, y, scale_small, &font, &score_line);
    }

    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    encoder.write_image(img.as_raw(), W, H, image::ColorType::Rgba8)?;
    Ok(buf)
}
