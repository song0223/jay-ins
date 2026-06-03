use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

const PAGE_MARGIN_X: f32 = 16.0;
const PAGE_MARGIN_Y: f32 = 24.0;
const CONTENT_MARGIN: f32 = 18.0;
const CONTROL_INSET: f32 = 10.0;
const PANEL_MAX_WIDTH: f32 = 456.0;

pub struct JayinsApp {
    url_input: String,
    save_dir: String,
    logs: Arc<Mutex<Vec<String>>>,
    downloading: Arc<Mutex<bool>>,
    runtime: tokio::runtime::Runtime,
}

impl JayinsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        configure_theme(&cc.egui_ctx);
        Self {
            url_input: String::new(),
            save_dir: default_save_dir(),
            logs: Arc::new(Mutex::new(vec!["就绪，粘贴链接开始下载".to_string()])),
            downloading: Arc::new(Mutex::new(false)),
            runtime: tokio::runtime::Runtime::new().expect("无法创建异步运行时"),
        }
    }

    fn add_log(&self, msg: &str) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(msg.to_string());
        }
    }

    fn start_download(&mut self) {
        let url = crate::utils::strip_query_params(self.url_input.trim());
        self.url_input = url.clone();
        let save_dir = self.save_dir.trim().to_string();
        if url.is_empty() {
            self.add_log("⚠ 请输入帖子链接");
            return;
        }
        if save_dir.is_empty() {
            self.add_log("⚠ 请选择保存目录");
            return;
        }
        {
            let d = self.downloading.lock().unwrap();
            if *d {
                return;
            }
        }
        {
            *self.downloading.lock().unwrap() = true;
        }

        let logs = self.logs.clone();
        let downloading = self.downloading.clone();
        self.runtime.spawn(async move {
            let log: crate::downloader::ProgressCallback = Box::new(move |msg: &str| {
                if let Ok(mut l) = logs.lock() {
                    l.push(msg.to_string());
                }
            });
            let r = async {
                let imgs = crate::downloader::fetch_image_urls(&url, "", "", &log).await?;
                if imgs.is_empty() {
                    return Err(anyhow::anyhow!("未找到图片"));
                }
                let d = crate::downloader::download_images(&imgs, &PathBuf::from(&save_dir), &log)
                    .await?;
                Ok::<_, anyhow::Error>(d)
            }
            .await;
            match r {
                Ok(d) => log(&format!("✅ 完成，共 {} 张", d.len())),
                Err(e) => log(&format!("❌ {}", e)),
            }
            *downloading.lock().unwrap() = false;
        });
    }

    fn select_directory(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("选择保存目录")
            .pick_folder()
        {
            self.save_dir = p.to_string_lossy().to_string();
        }
    }
}

fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    #[cfg(target_os = "macos")]
    for path in &[
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
    ] {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("zh".into(), egui::FontData::from_owned(data));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "zh".into());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "zh".into());
            break;
        }
    }
    #[cfg(target_os = "windows")]
    for path in &[r"C:\Windows\Fonts\msyh.ttc"] {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("zh".into(), egui::FontData::from_owned(data));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "zh".into());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "zh".into());
            break;
        }
    }
    ctx.set_fonts(fonts);
}

fn configure_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 9.0);
    style.spacing.text_edit_width = 320.0;
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(28, 35, 50));
    style.visuals.panel_fill = egui::Color32::from_rgb(246, 248, 252);
    style.visuals.window_fill = egui::Color32::WHITE;
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(244, 247, 251);
    style.visuals.faint_bg_color = egui::Color32::from_rgb(248, 250, 253);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(236, 72, 153);
    style.visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(225, 230, 239));
    style.visuals.widgets.inactive.bg_fill = egui::Color32::WHITE;
    style.visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(218, 224, 235));
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(252, 253, 255);
    style.visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(203, 213, 225));
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(248, 250, 252);
    style.visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(236, 72, 153));
    ctx.set_style(style);
}

impl eframe::App for JayinsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.modifiers.mac_cmd && i.key_pressed(egui::Key::W)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let is_downloading = *self.downloading.lock().unwrap();
        if is_downloading {
            ctx.request_repaint();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(246, 248, 252)))
            .show(ctx, |ui| {
                paint_background(ui);
                content_panel(ui, self, is_downloading);
            });
    }
}

fn content_panel(ui: &mut egui::Ui, app: &mut JayinsApp, is_downloading: bool) {
    let available = ui
        .max_rect()
        .shrink2(egui::vec2(PAGE_MARGIN_X, PAGE_MARGIN_Y));
    let panel_width = PANEL_MAX_WIDTH.min(available.width());
    let panel_rect = egui::Rect::from_center_size(
        available.center(),
        egui::vec2(panel_width, available.height()),
    );
    let content_rect = panel_rect.shrink(CONTENT_MARGIN + CONTROL_INSET);

    ui.painter().rect_filled(
        panel_rect,
        18.0,
        egui::Color32::from_rgba_premultiplied(255, 255, 255, 220),
    );
    ui.painter().rect_stroke(
        panel_rect,
        18.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(226, 232, 240)),
    );

    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            let content_width = content_rect.width().floor();
            ui.set_width(content_width);
            content_form(ui, app, is_downloading, content_width);
        },
    );
}

fn content_form(ui: &mut egui::Ui, app: &mut JayinsApp, is_downloading: bool, content_width: f32) {
    header(ui, is_downloading);
    ui.add_space(18.0);

    field_label(ui, "帖子链接");
    let response = ui.add(
        egui::TextEdit::singleline(&mut app.url_input)
            .hint_text("https://www.instagram.com/p/...")
            .desired_width(content_width)
            .margin(egui::vec2(12.0, 11.0)),
    );
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        app.start_download();
    }

    ui.add_space(8.0);
    field_label(ui, "保存位置");
    save_dir_row(ui, app, content_width);

    ui.add_space(18.0);
    if gradient_button(
        ui,
        if is_downloading {
            "下载中..."
        } else {
            "开始下载"
        },
        is_downloading,
        content_width,
    )
    .clicked()
    {
        app.start_download();
    }

    ui.add_space(18.0);
    log_panel(ui, &app.logs, content_width);
}

fn save_dir_row(ui: &mut egui::Ui, app: &mut JayinsApp, row_width: f32) {
    let button_width = 46.0;
    let gap = ui.spacing().item_spacing.x;
    let row_height = 42.0;
    let input_width = (row_width - button_width - gap).max(120.0);
    let (row_rect, _) =
        ui.allocate_exact_size(egui::vec2(row_width, row_height), egui::Sense::hover());
    let input_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(input_width, row_height));
    let button_rect = egui::Rect::from_min_size(
        egui::pos2(input_rect.right() + gap, row_rect.top()),
        egui::vec2(button_width, row_height),
    );

    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(input_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.save_dir)
                    .hint_text("选择文件夹...")
                    .desired_width(input_width)
                    .margin(egui::vec2(12.0, 11.0)),
            );
        },
    );

    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(button_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            let choose = egui::Button::new(
                egui::RichText::new("...")
                    .strong()
                    .color(egui::Color32::from_rgb(86, 97, 118)),
            )
            .min_size(egui::vec2(button_width, 42.0))
            .fill(egui::Color32::from_rgb(241, 245, 249));
            if ui
                .add(choose)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("选择保存目录")
                .clicked()
            {
                app.select_directory();
            }
        },
    );
}

fn paint_background(ui: &egui::Ui) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(246, 248, 252));
    painter.circle_filled(
        egui::pos2(rect.left() + rect.width() * 0.18, rect.top() + 18.0),
        132.0,
        egui::Color32::from_rgba_premultiplied(255, 122, 24, 26),
    );
    painter.circle_filled(
        egui::pos2(rect.right() - rect.width() * 0.12, rect.top() + 44.0),
        148.0,
        egui::Color32::from_rgba_premultiplied(37, 99, 235, 24),
    );
    painter.circle_filled(
        egui::pos2(rect.left() + rect.width() * 0.54, rect.top() + 20.0),
        120.0,
        egui::Color32::from_rgba_premultiplied(236, 72, 153, 18),
    );
}

fn header(ui: &mut egui::Ui, is_downloading: bool) {
    ui.horizontal(|ui| {
        brand_mark(ui);
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Jayins")
                    .size(27.0)
                    .strong()
                    .color(egui::Color32::from_rgb(17, 24, 39)),
            );
            ui.label(
                egui::RichText::new("Instagram 图片下载")
                    .size(13.0)
                    .color(egui::Color32::from_rgb(108, 117, 134)),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            status_badge(ui, is_downloading);
        });
    });
}

fn brand_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
    paint_horizontal_gradient(
        ui.painter(),
        rect,
        14.0,
        &[
            egui::Color32::from_rgb(249, 115, 22),
            egui::Color32::from_rgb(236, 72, 153),
            egui::Color32::from_rgb(37, 99, 235),
        ],
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "J",
        egui::FontId::proportional(24.0),
        egui::Color32::WHITE,
    );
}

fn status_badge(ui: &mut egui::Ui, is_downloading: bool) {
    let text = if is_downloading { "下载中" } else { "Ready" };
    let bg = if is_downloading {
        egui::Color32::from_rgb(255, 247, 237)
    } else {
        egui::Color32::from_rgb(232, 248, 239)
    };
    let color = if is_downloading {
        egui::Color32::from_rgb(194, 65, 12)
    } else {
        egui::Color32::from_rgb(18, 112, 68)
    };
    egui::Frame::none()
        .fill(bg)
        .rounding(egui::Rounding::same(999.0))
        .inner_margin(egui::Margin::symmetric(11.0, 6.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(12.0).strong().color(color));
        });
}

fn field_label(ui: &mut egui::Ui, label: &str) {
    ui.label(
        egui::RichText::new(label)
            .size(12.0)
            .strong()
            .color(egui::Color32::from_rgb(98, 108, 128)),
    );
}

fn gradient_button(ui: &mut egui::Ui, text: &str, disabled: bool, width: f32) -> egui::Response {
    let desired_size = egui::vec2(width, 48.0);
    let (rect, response) = ui.allocate_exact_size(
        desired_size,
        if disabled {
            egui::Sense::hover()
        } else {
            egui::Sense::click()
        },
    );

    let rounding = 13.0;
    if disabled {
        ui.painter()
            .rect_filled(rect, rounding, egui::Color32::from_rgb(190, 197, 210));
    } else {
        let fill = if response.hovered() {
            egui::Color32::from_rgb(239, 68, 158)
        } else {
            egui::Color32::from_rgb(236, 72, 153)
        };
        ui.painter().rect_filled(rect, rounding, fill);
        ui.painter().rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 90),
            ),
        );
    }

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(16.0),
        egui::Color32::WHITE,
    );

    if disabled {
        response
    } else {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}

fn log_panel(ui: &mut egui::Ui, logs: &Arc<Mutex<Vec<String>>>, width: f32) {
    let width = width.max(260.0);
    let height = 144.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let inner = rect.shrink(14.0);

    ui.painter()
        .rect_filled(rect, 14.0, egui::Color32::from_rgb(247, 249, 253));
    ui.painter().rect_stroke(
        rect,
        14.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(233, 238, 246)),
    );

    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("日志")
                        .size(13.0)
                        .strong()
                        .color(egui::Color32::from_rgb(48, 58, 78)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let count = logs.lock().map(|l| l.len()).unwrap_or_default();
                    ui.label(
                        egui::RichText::new(format!("{} 条", count))
                            .size(12.0)
                            .color(egui::Color32::from_rgb(124, 134, 152)),
                    );
                });
            });
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .max_height(84.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if let Ok(logs) = logs.lock() {
                        for log in logs.iter() {
                            ui.label(egui::RichText::new(log).size(12.5).color(log_color(log)));
                        }
                    }
                });
        },
    );
}

fn log_color(log: &str) -> egui::Color32 {
    if log.starts_with("✅") {
        egui::Color32::from_rgb(22, 163, 74)
    } else if log.starts_with("❌") || log.starts_with("⚠") {
        egui::Color32::from_rgb(220, 38, 38)
    } else if log.starts_with("[OK]") {
        egui::Color32::from_rgb(37, 99, 235)
    } else {
        egui::Color32::from_rgb(86, 97, 118)
    }
}

fn paint_horizontal_gradient(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: f32,
    colors: &[egui::Color32; 3],
) {
    let strips = 36;
    for i in 0..strips {
        let t0 = i as f32 / strips as f32;
        let t1 = (i + 1) as f32 / strips as f32;
        let left = egui::lerp(rect.left()..=rect.right(), t0);
        let right = egui::lerp(rect.left()..=rect.right(), t1);
        let strip = egui::Rect::from_min_max(
            egui::pos2(left, rect.top()),
            egui::pos2(right + 1.0, rect.bottom()),
        );
        painter.rect_filled(strip, rounding, gradient_color(colors, (t0 + t1) * 0.5));
    }
}

fn gradient_color(colors: &[egui::Color32; 3], t: f32) -> egui::Color32 {
    let (from, to, local_t) = if t < 0.5 {
        (colors[0], colors[1], t * 2.0)
    } else {
        (colors[1], colors[2], (t - 0.5) * 2.0)
    };
    lerp_color(from, to, local_t)
}

fn lerp_color(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let r = egui::lerp(from.r() as f32..=to.r() as f32, t).round() as u8;
    let g = egui::lerp(from.g() as f32..=to.g() as f32, t).round() as u8;
    let b = egui::lerp(from.b() as f32..=to.b() as f32, t).round() as u8;
    egui::Color32::from_rgb(r, g, b)
}

fn default_save_dir() -> String {
    dirs_next::download_dir()
        .or_else(|| dirs_next::home_dir().map(|h| h.join("Downloads")))
        .map(|p| p.join("jayins").to_string_lossy().to_string())
        .unwrap_or_else(|| "jayins_downloads".to_string())
}
