//! İşlem kapanış (İŞLEM SONUCU) kartı – Telegram için PNG üretir.

use anyhow::Result;
use image::{ImageBuffer, ImageEncoder, Rgba};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use rusttype::{Font, Scale};

const W: u32 = 420;
const H: u32 = 260;
const HEADER_H: u32 = 44;
const PAD: i32 = 16;

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

/// İŞLEM SONUCU kartını PNG bytes olarak döndürür.
/// symbol, side (LONG/SHORT), entry, exit, pnl_pct (yüzde kâr/zarar).
pub fn render_trade_close_card(
    symbol: &str,
    side: &str,
    entry: f64,
    exit: f64,
    pnl_pct: f64,
) -> Result<Vec<u8>> {
    let font = try_load_font().ok_or_else(|| {
        anyhow::anyhow!("Font bulunamadı (IQAI_FONT_PATH veya fonts/DejaVuSans.ttf)")
    })?;

    let scale_title = Scale::uniform(20.0);
    let scale_mid = Scale::uniform(16.0);

    let bg = Rgba([16, 18, 30, 255]);
    let mut img = ImageBuffer::from_pixel(W, H, bg);

    let white = Rgba([255, 255, 255, 255]);
    let green = Rgba([0, 230, 118, 255]); // parlak yeşil
    let red = Rgba([248, 113, 113, 255]);
    let separator = Rgba([50, 52, 60, 255]);

    // Header: yeşil kare + İŞLEM SONUCU
    let header_bg = Rgba([20, 22, 28, 255]);
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(0, 0).of_size(W, HEADER_H),
        header_bg,
    );
    let check_green = Rgba([0, 200, 83, 255]);
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(PAD, 10).of_size(24, 24),
        check_green,
    );
    draw_text_mut(
        &mut img,
        green,
        PAD + 32,
        12,
        scale_title,
        &font,
        "İŞLEM SONUCU",
    );

    // Ayırıcı
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(0, HEADER_H as i32).of_size(W, 1),
        separator,
    );

    // Sembol · Yön (bar chart ikonu yerine metin)
    let y_row1 = HEADER_H as i32 + 20;
    draw_text_mut(
        &mut img,
        white,
        PAD,
        y_row1,
        scale_mid,
        &font,
        &format!("{} · {}", symbol, side),
    );

    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(0, (HEADER_H as i32) + 52).of_size(W, 1),
        separator,
    );

    // Giriş ———▶ Çıkış (TL)
    let y_price = (HEADER_H as i32) + 70;
    let price_line = format!("{:.2} TL ———▶ {:.2} TL", entry, exit);
    draw_text_mut(&mut img, white, PAD, y_price, scale_mid, &font, &price_line);

    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(0, (HEADER_H as i32) + 110).of_size(W, 1),
        separator,
    );

    // Sonuç: 💰 +21.71% (veya zarar kırmızı)
    let y_result = (HEADER_H as i32) + 135;
    let pct_str = format!("{:+.2}%", pnl_pct);
    let pct_color = if pnl_pct >= 0.0 { green } else { red };
    draw_text_mut(&mut img, pct_color, PAD, y_result, scale_title, &font, "💰");
    draw_text_mut(&mut img, pct_color, PAD + 36, y_result, scale_title, &font, &pct_str);

    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    encoder.write_image(img.as_raw(), W, H, image::ColorType::Rgba8)?;
    Ok(buf)
}
