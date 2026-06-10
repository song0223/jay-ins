use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActivePage {
    Home,
    Download,
}

const PAGE_MARGIN_X: f32 = 16.0;
const PAGE_MARGIN_Y: f32 = 24.0;
const CONTENT_MARGIN: f32 = 18.0;
const CONTROL_INSET: f32 = 10.0;
const PANEL_MAX_WIDTH: f32 = 456.0;

pub struct JayinsApp {
    active_page: ActivePage,
    url_input: String,
    profile_input: String,
    save_dir: String,
    logs: Arc<Mutex<Vec<String>>>,
    caption: Arc<Mutex<String>>,
    caption_copied_at: Option<std::time::Instant>,
    profile_logs: Arc<Mutex<Vec<String>>>,
    profile_posts: Arc<Mutex<Vec<crate::profile::ProfilePost>>>,
    downloading: Arc<Mutex<bool>>,
    fetching_profile: Arc<Mutex<bool>>,
    copied_post: Option<String>,
    copied_post_at: Option<std::time::Instant>,
    runtime: tokio::runtime::Runtime,
}

impl JayinsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        configure_theme(&cc.egui_ctx);
        Self {
            active_page: ActivePage::Home,
            url_input: String::new(),
            profile_input: "https://www.instagram.com/jaychou/".to_string(),
            save_dir: default_save_dir(),
            logs: Arc::new(Mutex::new(vec!["就绪，粘贴链接开始下载".to_string()])),
            caption: Arc::new(Mutex::new(String::new())),
            caption_copied_at: None,
            profile_logs: Arc::new(Mutex::new(vec!["输入主页链接，获取第一页帖子".to_string()])),
            profile_posts: Arc::new(Mutex::new(Vec::new())),
            downloading: Arc::new(Mutex::new(false)),
            fetching_profile: Arc::new(Mutex::new(false)),
            copied_post: None,
            copied_post_at: None,
            runtime: tokio::runtime::Runtime::new().expect("无法创建异步运行时"),
        }
    }

    fn add_log(&self, msg: &str) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(msg.to_string());
        }
    }

    fn add_profile_log(&self, msg: &str) {
        if let Ok(mut logs) = self.profile_logs.lock() {
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
        // 清空上次文案
        if let Ok(mut c) = self.caption.lock() {
            c.clear();
        }
        self.caption_copied_at = None;

        let logs = self.logs.clone();
        let caption_shared = self.caption.clone();
        let downloading = self.downloading.clone();
        self.runtime.spawn(async move {
            let log: crate::downloader::ProgressCallback = Box::new(move |msg: &str| {
                if let Ok(mut l) = logs.lock() {
                    l.push(msg.to_string());
                }
            });
            let r = async {
                let (imgs, caption) = crate::downloader::fetch_image_urls(&url, "", "", &log).await?;
                if imgs.is_empty() {
                    return Err(anyhow::anyhow!("未找到图片"));
                }
                // 保存文案
                if !caption.is_empty() {
                    if let Ok(mut c) = caption_shared.lock() {
                        *c = caption;
                    }
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

    fn start_fetch_profile(&mut self) {
        let profile_url = self.profile_input.trim().to_string();
        if profile_url.is_empty() {
            self.add_profile_log("⚠ 请输入主页链接");
            return;
        }
        {
            let fetching = self.fetching_profile.lock().unwrap();
            if *fetching {
                return;
            }
        }
        {
            *self.fetching_profile.lock().unwrap() = true;
        }
        if let Ok(mut posts) = self.profile_posts.lock() {
            posts.clear();
        }
        self.copied_post = None;
        self.copied_post_at = None;

        let logs = self.profile_logs.clone();
        let posts = self.profile_posts.clone();
        let fetching = self.fetching_profile.clone();
        self.runtime.spawn(async move {
            if let Ok(mut logs) = logs.lock() {
                logs.push(format!("正在获取主页: {}", profile_url));
            }

            match crate::profile::fetch_profile_posts_with_covers(&profile_url, true).await {
                Ok(found) => {
                    let count = found.len();
                    if let Ok(mut posts) = posts.lock() {
                        *posts = found;
                    }
                    if let Ok(mut logs) = logs.lock() {
                        logs.push(format!("✅ 找到 {} 个帖子", count));
                    }
                }
                Err(e) => {
                    if let Ok(mut logs) = logs.lock() {
                        logs.push(format!("❌ {}", e));
                    }
                }
            }
            *fetching.lock().unwrap() = false;
        });
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
        let is_fetching_profile = *self.fetching_profile.lock().unwrap();
        if is_downloading || is_fetching_profile {
            ctx.request_repaint();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(246, 248, 252)))
            .show(ctx, |ui| {
                paint_background(ui);
                content_panel(ui, self, is_downloading, is_fetching_profile);
            });
    }
}

fn content_panel(
    ui: &mut egui::Ui,
    app: &mut JayinsApp,
    is_downloading: bool,
    is_fetching_profile: bool,
) {
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
            content_form(ui, app, is_downloading, is_fetching_profile, content_width);
        },
    );
}

fn content_form(
    ui: &mut egui::Ui,
    app: &mut JayinsApp,
    is_downloading: bool,
    is_fetching_profile: bool,
    content_width: f32,
) {
    header(ui, is_downloading);
    ui.add_space(14.0);
    nav_tabs(ui, app, content_width);
    ui.add_space(14.0);

    match app.active_page {
        ActivePage::Home => home_page(ui, app, is_fetching_profile, content_width),
        ActivePage::Download => download_page(ui, app, is_downloading, content_width),
    }
}

fn download_page(ui: &mut egui::Ui, app: &mut JayinsApp, is_downloading: bool, content_width: f32) {
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

    ui.add_space(14.0);
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

    ui.add_space(10.0);

    // 文案区域
    let caption_text = app.caption.lock().map(|c| c.clone()).unwrap_or_default();
    if !caption_text.is_empty() {
        caption_panel(ui, &caption_text, &mut app.caption_copied_at, content_width);
        ui.add_space(8.0);
    }

    // 日志
    let log_height = if caption_text.is_empty() { 120.0 } else { 96.0 };
    log_panel(ui, &app.logs, content_width, log_height);
}

fn home_page(ui: &mut egui::Ui, app: &mut JayinsApp, is_fetching: bool, content_width: f32) {
    field_label(ui, "主页链接");
    let response = ui.add(
        egui::TextEdit::singleline(&mut app.profile_input)
            .hint_text("https://www.instagram.com/jaychou/")
            .desired_width(content_width)
            .margin(egui::vec2(12.0, 11.0)),
    );
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        app.start_fetch_profile();
    }

    ui.add_space(12.0);
    if gradient_button(
        ui,
        if is_fetching {
            "获取中..."
        } else {
            "获取第一页帖子"
        },
        is_fetching,
        content_width,
    )
    .clicked()
    {
        app.start_fetch_profile();
    }

    ui.add_space(10.0);

    // 帖子网格 + 日志 共享剩余空间
    let remaining = ui.available_height();
    let log_height = 96.0_f32.min(remaining * 0.35);
    let grid_height = (remaining - log_height - 10.0).max(80.0);

    posts_grid(ui, app, content_width, grid_height);
    ui.add_space(8.0);
    log_panel(ui, &app.profile_logs, content_width, log_height);
}

fn nav_tabs(ui: &mut egui::Ui, app: &mut JayinsApp, content_width: f32) {
    let gap = 8.0;
    let tab_width = (content_width - gap) / 2.0;
    ui.horizontal(|ui| {
        tab_button(
            ui,
            "主页抓取",
            ActivePage::Home,
            &mut app.active_page,
            tab_width,
        );
        ui.add_space(gap - ui.spacing().item_spacing.x);
        tab_button(
            ui,
            "图片下载",
            ActivePage::Download,
            &mut app.active_page,
            tab_width,
        );
    });
}

fn tab_button(
    ui: &mut egui::Ui,
    label: &str,
    page: ActivePage,
    active_page: &mut ActivePage,
    width: f32,
) {
    let active = *active_page == page;
    let text_color = if active {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(86, 97, 118)
    };
    let fill = if active {
        egui::Color32::from_rgb(236, 72, 153)
    } else {
        egui::Color32::from_rgb(241, 245, 249)
    };
    let button = egui::Button::new(egui::RichText::new(label).size(15.0).strong().color(text_color))
        .min_size(egui::vec2(width, 36.0))
        .fill(fill);
    if ui
        .add(button)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        *active_page = page;
    }
}

fn posts_grid(ui: &mut egui::Ui, app: &mut JayinsApp, content_width: f32, max_height: f32) {
    let posts = app
        .profile_posts
        .lock()
        .map(|posts| posts.clone())
        .unwrap_or_default();

    if posts.is_empty() {
        empty_posts_box(ui, content_width);
        return;
    }

    egui::ScrollArea::vertical()
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .max_height(max_height)
        .show(ui, |ui| {
            let gap = 10.0;
            let card_width = ((content_width - gap) / 2.0).floor();
            for chunk in posts.chunks(2) {
                ui.allocate_ui_with_layout(
                    egui::vec2(content_width, card_width + 42.0),
                    egui::Layout::left_to_right(egui::Align::Min),
                    |ui| {
                        for (i, post) in chunk.iter().enumerate() {
                            if i > 0 {
                                ui.add_space(gap);
                            }
                            post_card(ui, app, post, card_width);
                        }
                    },
                );
                ui.add_space(gap);
            }
        });
}

fn empty_posts_box(ui: &mut egui::Ui, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 122.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 14.0, egui::Color32::from_rgb(247, 249, 253));
    ui.painter().rect_stroke(
        rect,
        14.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(233, 238, 246)),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "获取后显示帖子封面",
        egui::FontId::proportional(15.0),
        egui::Color32::from_rgb(108, 117, 134),
    );
}

fn post_card(
    ui: &mut egui::Ui,
    app: &mut JayinsApp,
    post: &crate::profile::ProfilePost,
    width: f32,
) {
    let height = width + 42.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let image_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(6.0, 6.0),
        egui::vec2(width - 12.0, width - 12.0),
    );
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, image_rect.bottom() + 4.0),
        egui::pos2(rect.right() - 8.0, rect.bottom() - 4.0),
    );

    // 卡片背景
    ui.painter()
        .rect_filled(rect, 10.0, egui::Color32::from_rgb(247, 249, 253));
    ui.painter().rect_stroke(
        rect,
        10.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(233, 238, 246)),
    );

    // 封面图
    if !post.cover_bytes.is_empty() {
        if let Some(tex) = load_texture(ui.ctx(), &post.cover_url, &post.cover_bytes) {
            ui.painter().image(
                tex.id(),
                image_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    } else {
        ui.painter()
            .rect_filled(image_rect, 8.0, egui::Color32::from_rgb(240, 243, 248));
        ui.painter().text(
            image_rect.center(),
            egui::Align2::CENTER_CENTER,
            "📷",
            egui::FontId::proportional(20.0),
            egui::Color32::from_rgb(160, 170, 185),
        );
    }

    // 帖子 shortcode 标签 + 复制状态
    let short = post
        .url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("post");

    // 自动重置复制状态（1.5 秒后）
    let copied = if app.copied_post.as_deref() == Some(post.url.as_str()) {
        if let Some(t) = app.copied_post_at {
            if t.elapsed().as_millis() > 1500 {
                app.copied_post = None;
                app.copied_post_at = None;
                false
            } else {
                true
            }
        } else {
            false
        }
    } else {
        false
    };

    if copied {
        ui.ctx().request_repaint();
    }

    let (label_text, label_color) = if copied {
        (
            format!("已复制 {}", short),
            egui::Color32::from_rgb(22, 163, 74),
        )
    } else {
        (
            short.to_string(),
            egui::Color32::from_rgb(86, 97, 118),
        )
    };
    ui.painter().text(
        label_rect.left_center(),
        egui::Align2::LEFT_CENTER,
        &label_text,
        egui::FontId::proportional(13.0),
        label_color,
    );

    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("点击复制帖子链接")
        .clicked()
    {
        ui.ctx().copy_text(post.url.clone());
        app.copied_post = Some(post.url.clone());
        app.copied_post_at = Some(std::time::Instant::now());
        app.add_profile_log("已复制帖子链接");
    }
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
                    .size(30.0)
                    .strong()
                    .color(egui::Color32::from_rgb(17, 24, 39)),
            );
            ui.label(
                egui::RichText::new("Instagram 图片下载")
                    .size(15.0)
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
            ui.label(egui::RichText::new(text).size(13.0).strong().color(color));
        });
}

fn field_label(ui: &mut egui::Ui, label: &str) {
    ui.label(
        egui::RichText::new(label)
            .size(14.0)
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
        egui::FontId::proportional(18.0),
        egui::Color32::WHITE,
    );

    if disabled {
        response
    } else {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}

fn caption_panel(ui: &mut egui::Ui, text: &str, copied_at: &mut Option<std::time::Instant>, width: f32) {
    let width = width.max(260.0);

    // 自动重置复制状态（1.5 秒后）
    let is_copied = if let Some(t) = copied_at {
        if t.elapsed().as_millis() > 1500 {
            *copied_at = None;
            false
        } else {
            true
        }
    } else {
        false
    };

    if is_copied {
        ui.ctx().request_repaint();
    }

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, 80.0),
        egui::Sense::click(),
    );

    let bg = if is_copied {
        egui::Color32::from_rgb(232, 248, 239)
    } else {
        egui::Color32::from_rgb(247, 249, 253)
    };
    ui.painter()
        .rect_filled(rect, 14.0, bg);
    ui.painter().rect_stroke(
        rect,
        14.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(233, 238, 246)),
    );

    let inner = rect.shrink(12.0);
    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_width(inner.width());
            ui.add(
                egui::Label::new(
                    egui::RichText::new("帖子文案")
                        .size(14.0)
                        .strong()
                        .color(egui::Color32::from_rgb(98, 108, 128)),
                )
                .sense(egui::Sense::hover()),
            );
            ui.add_space(4.0);
            if is_copied {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("✅ 已复制")
                            .size(14.0)
                            .color(egui::Color32::from_rgb(22, 163, 74)),
                    )
                    .sense(egui::Sense::hover()),
                );
            } else {
                let display = if text.chars().count() > 100 {
                    let truncated: String = text.chars().take(100).collect();
                    format!("{}…（点击复制全文）", truncated)
                } else {
                    text.to_string()
                };
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&display)
                            .size(14.0)
                            .color(egui::Color32::from_rgb(48, 58, 78)),
                    )
                    .sense(egui::Sense::hover()),
                );
            }
        },
    );

    if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        ui.ctx().copy_text(text.to_string());
        *copied_at = Some(std::time::Instant::now());
    }
}

fn log_panel(ui: &mut egui::Ui, logs: &Arc<Mutex<Vec<String>>>, width: f32, height: f32) {
    let width = width.max(260.0);

    egui::Frame::none()
        .fill(egui::Color32::from_rgb(247, 249, 253))
        .rounding(egui::Rounding::same(14.0))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(233, 238, 246)))
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.set_width(width - 28.0);

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("日志")
                        .size(14.0)
                        .strong()
                        .color(egui::Color32::from_rgb(48, 58, 78)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let count = logs.lock().map(|l| l.len()).unwrap_or_default();
                    ui.label(
                        egui::RichText::new(format!("{} 条", count))
                            .size(13.0)
                            .color(egui::Color32::from_rgb(124, 134, 152)),
                    );
                });
            });

            ui.add_space(6.0);

            let scroll_h = (height - 56.0).max(20.0);
            let label_w = width - 42.0;
            egui::ScrollArea::vertical()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .max_height(scroll_h)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.set_width(label_w);
                    if let Ok(logs) = logs.lock() {
                        for log in logs.iter() {
                            ui.label(
                                egui::RichText::new(log).size(13.0).color(log_color(log)),
                            );
                        }
                    }
                });
        });
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

fn load_texture(ctx: &egui::Context, key: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    Some(ctx.load_texture(key, color_image, egui::TextureOptions::LINEAR))
}

fn default_save_dir() -> String {
    dirs_next::download_dir()
        .or_else(|| dirs_next::home_dir().map(|h| h.join("Downloads")))
        .map(|p| p.join("jayins").to_string_lossy().to_string())
        .unwrap_or_else(|| "jayins_downloads".to_string())
}
