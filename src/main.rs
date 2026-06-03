mod app;
mod downloader;
mod utils;

use app::JayinsApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 500.0])
            .with_min_inner_size([420.0, 460.0])
            .with_titlebar_shown(true)
            .with_icon(app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "Jayins",
        options,
        Box::new(|cc| Ok(Box::new(JayinsApp::new(cc)))),
    )
}

fn app_icon() -> egui::IconData {
    let width = 64;
    let height = 64;
    let mut rgba = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - 31.5;
            let dy = y as f32 - 31.5;
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = if dist > 31.0 { 0 } else { 255 };
            let t = x as f32 / (width - 1) as f32;
            let (r, g, b) = if t < 0.5 {
                let t = t * 2.0;
                lerp_rgb((249, 115, 22), (236, 72, 153), t)
            } else {
                let t = (t - 0.5) * 2.0;
                lerp_rgb((236, 72, 153), (37, 99, 235), t)
            };
            rgba.extend_from_slice(&[r, g, b, alpha]);
        }
    }

    draw_j(&mut rgba, width, height);

    egui::IconData {
        rgba,
        width: width as u32,
        height: height as u32,
    }
}

fn lerp_rgb(from: (u8, u8, u8), to: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    (lerp(from.0, to.0), lerp(from.1, to.1), lerp(from.2, to.2))
}

fn draw_j(rgba: &mut [u8], width: usize, height: usize) {
    for y in 15..43 {
        draw_pixel_block(rgba, width, height, 36, y, 5, [255, 255, 255, 255]);
    }
    for x in 22..39 {
        draw_pixel_block(rgba, width, height, x, 43, 5, [255, 255, 255, 255]);
    }
    for y in 37..45 {
        draw_pixel_block(rgba, width, height, 21, y, 5, [255, 255, 255, 255]);
    }
}

fn draw_pixel_block(
    rgba: &mut [u8],
    width: usize,
    height: usize,
    cx: usize,
    cy: usize,
    size: usize,
    color: [u8; 4],
) {
    let radius = size / 2;
    let min_x = cx.saturating_sub(radius);
    let max_x = (cx + radius).min(width - 1);
    let min_y = cy.saturating_sub(radius);
    let max_y = (cy + radius).min(height - 1);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let idx = (y * width + x) * 4;
            rgba[idx..idx + 4].copy_from_slice(&color);
        }
    }
}
