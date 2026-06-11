mod app;
mod downloader;
mod profile;
mod utils;

use app::JayinsApp;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();

    // 提取选项参数
    let cookie_arg = extract_arg(&args, "--cookie")
        .or_else(|| extract_arg(&args, "-c"));

    // 收集非选项参数（跳过 -c value 等）
    let mut positional: Vec<&str> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "-c" || args[i] == "--cookie" {
            i += 2; // 跳过 key 和 value
            continue;
        }
        if !args[i].starts_with('-') {
            positional.push(&args[i]);
        }
        i += 1;
    }

    // 判断是否有命令行参数
    if positional.is_empty() && !args.contains(&"--help".to_string()) && !args.contains(&"-h".to_string()) {
        // 启动 GUI
        return run_gui();
    }

    // 帮助
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_help();
        return Ok(());
    }

    // profile 子命令
    if positional.first() == Some(&"profile") {
        match positional.get(1) {
            Some(url) => return run_profile_cli(url, cookie_arg.as_deref()),
            None => {
                eprintln!("❌ 请提供主页链接");
                eprintln!("用法: jayins profile <主页链接>");
                std::process::exit(1);
            }
        }
    }

    // keepalive 子命令
    if positional.first() == Some(&"keepalive") {
        return run_keepalive_cli(cookie_arg.as_deref());
    }

    // 下载模式
    let url = positional[0];
    let save_dir = positional.get(1).cloned().map(String::from).unwrap_or_else(default_save_dir);
    return run_download_cli(url, &save_dir, cookie_arg.as_deref());
}

fn run_gui() -> eframe::Result {
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

/// 从参数列表中提取 --key value 形式的参数
fn extract_arg(args: &[String], key: &str) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == key && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}

fn print_help() {
    println!("Jayins - Instagram 图片下载器");
    println!();
    println!("用法:");
    println!("  jayins [选项] [链接] [保存目录]       启动 GUI 或命令行下载");
    println!("  jayins profile [选项] <主页链接>       获取主页帖子列表");
    println!("  jayins keepalive [选项]               续期 Cookie（保持登录状态）");
    println!();
    println!("选项:");
    println!("  -c, --cookie <COOKIE>    指定 Instagram Cookie");
    println!("  -h, --help               显示帮助信息");
    println!();
    println!("示例:");
    println!("  jayins https://www.instagram.com/p/ABC123/");
    println!("  jayins https://www.instagram.com/p/ABC123/ ~/Pictures");
    println!("  jayins profile https://www.instagram.com/jaychou/");
    println!("  jayins keepalive");
    println!();
    println!("Cookie 设置（按优先级）:");
    println!("  1. 命令行参数:  -c 'sessionid=xxx; ...'");
    println!("  2. 环境变量:    export INSTAGRAM_COOKIE='sessionid=xxx; ...'");
    println!("  3. 配置文件:    ~/.config/jayins/cookie.txt");
    println!("  4. 自动读取:    从 Chrome 浏览器自动获取");
    println!("  5. 内置默认:    使用程序内置的 Cookie");
    println!();
    println!("服务器定时续期 Cookie:");
    println!("  # 每天凌晨 2 点续期");
    println!("  0 2 * * * INSTAGRAM_COOKIE='sessionid=xxx' /usr/local/bin/jayins keepalive >> /var/log/jayins.log 2>&1");
}

fn run_download_cli(url: &str, save_dir: &str, cookie: Option<&str>) -> eframe::Result {
    println!("链接: {}", url);
    println!("保存到: {}", save_dir);

    let rt = tokio::runtime::Runtime::new().expect("无法创建异步运行时");
    let log: downloader::ProgressCallback = Box::new(|msg: &str| println!("{}", msg));

    let cookie_str = cookie.unwrap_or("");
    match rt.block_on(async {
        let (images, caption) =
            downloader::fetch_image_urls(url, cookie_str, "", &log).await?;

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

fn run_profile_cli(profile_url: &str, cookie: Option<&str>) -> eframe::Result {
    eprintln!("主页: {}", profile_url);
    eprintln!("正在获取帖子列表...");

    // 设置 Cookie 环境变量（如果通过命令行传入）
    if let Some(c) = cookie {
        std::env::set_var("INSTAGRAM_COOKIE", c);
    }

    let rt = tokio::runtime::Runtime::new().expect("无法创建异步运行时");

    match rt.block_on(profile::fetch_profile_posts(profile_url)) {
        Ok(posts) => {
            if posts.is_empty() {
                eprintln!("未找到帖子");
            } else {
                // 输出 JSON 到 stdout
                let output: Vec<serde_json::Value> = posts.iter().map(|p| {
                    let shortcode = p.url.trim_end_matches('/').rsplit('/').next().unwrap_or("");
                    serde_json::json!({
                        "url": p.url,
                        "id": shortcode,
                    })
                }).collect();

                println!("{}", serde_json::to_string_pretty(&output).unwrap());
                eprintln!("\n✅ 共 {} 个帖子", posts.len());
            }
        }
        Err(e) => {
            eprintln!("❌ {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn run_keepalive_cli(cookie: Option<&str>) -> eframe::Result {
    // 设置 Cookie 环境变量（如果通过命令行传入）
    if let Some(c) = cookie {
        std::env::set_var("INSTAGRAM_COOKIE", c);
    }

    let rt = tokio::runtime::Runtime::new().expect("无法创建异步运行时");

    eprintln!("[INFO] 正在续期 Cookie...");

    match rt.block_on(profile::keepalive()) {
        Ok(_) => {
            eprintln!("✅ Cookie 续期成功");
        }
        Err(e) => {
            eprintln!("❌ Cookie 续期失败: {}", e);
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
