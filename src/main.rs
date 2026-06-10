mod app;
mod downloader;
mod profile;
mod utils;

use app::JayinsApp;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();

    // 命令行模式
    if args.len() >= 2 {
        match args[1].as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "profile" => {
                if args.len() < 3 {
                    eprintln!("❌ 请提供主页链接");
                    eprintln!("用法: jayins profile <主页链接>");
                    std::process::exit(1);
                }
                return run_profile_cli(&args[2]);
            }
            _ => {
                let url = &args[1];
                let save_dir = if args.len() >= 3 {
                    args[2].clone()
                } else {
                    default_save_dir()
                };
                return run_download_cli(url, &save_dir);
            }
        }
    }

    // GUI 模式
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([468.0, 765.0])
            .with_min_inner_size([420.0, 560.0])
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

fn print_help() {
    println!("Jayins - Instagram 图片下载器");
    println!();
    println!("用法:");
    println!("  jayins                              启动图形界面");
    println!("  jayins <帖子链接> [保存目录]          下载帖子图片");
    println!("  jayins profile <主页链接>             获取主页帖子列表");
    println!();
    println!("示例:");
    println!("  jayins https://www.instagram.com/p/ABC123/");
    println!("  jayins https://www.instagram.com/p/ABC123/ ~/Pictures");
    println!("  jayins profile https://www.instagram.com/jaychou/");
    println!();
    println!("Cookie 设置（按优先级）:");
    println!("  1. 环境变量: export INSTAGRAM_COOKIE='sessionid=xxx; ...'");
    println!("  2. 配置文件: ~/.config/jayins/cookie.txt");
    println!("  3. 自动读取: 从 Chrome 浏览器自动获取（需要 browser_cookie3）");
    println!("  4. 内置默认: 使用程序内置的 Cookie");
    println!();
    println!("Linux 用户获取 Cookie:");
    println!("  1. 浏览器登录 Instagram");
    println!("  2. F12 → Console → 输入 document.cookie");
    println!("  3. 复制输出到 ~/.config/jayins/cookie.txt");
}

fn run_download_cli(url: &str, save_dir: &str) -> eframe::Result {
    println!("链接: {}", url);
    println!("保存到: {}", save_dir);

    let rt = tokio::runtime::Runtime::new().expect("无法创建异步运行时");
    let log: downloader::ProgressCallback = Box::new(|msg: &str| println!("{}", msg));

    match rt.block_on(async {
        let (images, caption) =
            downloader::fetch_image_urls(url, "", "", &log).await?;

        if !caption.is_empty() {
            println!("--- 文案 ---");
            println!("{}", caption);
            println!("------------");
        }

        if images.is_empty() {
            anyhow::bail!("未找到图片");
        }

        let save_path = std::path::PathBuf::from(save_dir);
        let downloaded =
            downloader::download_images(&images, &save_path, &log).await?;

        Ok::<_, anyhow::Error>(downloaded)
    }) {
        Ok(downloaded) => {
            println!("✅ 完成，共 {} 张图片", downloaded.len());
        }
        Err(e) => {
            eprintln!("❌ {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn run_profile_cli(profile_url: &str) -> eframe::Result {
    println!("主页: {}", profile_url);
    println!("正在获取帖子列表...");

    let rt = tokio::runtime::Runtime::new().expect("无法创建异步运行时");

    match rt.block_on(profile::fetch_profile_posts(profile_url)) {
        Ok(posts) => {
            if posts.is_empty() {
                println!("未找到帖子");
            } else {
                println!("✅ 找到 {} 个帖子:\n", posts.len());
                for (i, post) in posts.iter().enumerate() {
                    println!("{}. {}", i + 1, post.url);
                }
                println!("\n复制链接即可在浏览器打开或用 jayins <链接> 下载");
            }
        }
        Err(e) => {
            eprintln!("❌ {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn default_save_dir() -> String {
    dirs_next::download_dir()
        .or_else(|| dirs_next::home_dir().map(|h| h.join("Downloads")))
        .map(|p| p.join("jayins").to_string_lossy().to_string())
        .unwrap_or_else(|| "jayins_downloads".to_string())
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
