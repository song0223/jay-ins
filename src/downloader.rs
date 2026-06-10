use anyhow::{bail, Context, Result};
use headless_chrome::{Browser, LaunchOptions};
use reqwest::Client;
use std::path::Path;
use tokio::fs;

use crate::utils::{ensure_dir, make_filename};

/// 图片信息
#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub url: String,
    pub is_video: bool,
}

/// 下载进度回调
pub type ProgressCallback = Box<dyn Fn(&str) + Send + Sync>;

/// 浏览器 User-Agent
const BROWSER_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

/// 创建下载客户端
fn build_download_client() -> Result<Client> {
    let client = Client::builder()
        .user_agent(BROWSER_UA)
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("Referer", "https://www.instagram.com/".parse().unwrap());
            headers.insert(
                "Accept",
                "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8"
                    .parse()
                    .unwrap(),
            );
            headers
        })
        .build()?;
    Ok(client)
}

/// 用无头浏览器获取帖子图片 URL（原始画质）和文案
pub async fn fetch_image_urls(
    url: &str,
    cookie: &str,
    _csrf_token: &str,
    log: &ProgressCallback,
) -> Result<(Vec<ImageInfo>, String)> {
    log(&format!("正在解析链接: {}", url));

    let shortcode = crate::utils::extract_shortcode(url)
        .context("无法从链接中提取 shortcode，请确认链接格式正确")?;

    log(&format!("Shortcode: {}", shortcode));

    let default_cookie =
        "ds_user_id=6009511404; csrftoken=en2hyrbjkI3AjRBUKDUPcaLyNsGYhocx; wd=1671x626; sessionid=6009511404%3ADt7ylCb1z380Fq%3A6%3AAYi90RxHDVdEZ36B89y1V91Gt64gvDDZ02i1Q7NZBjg";
    let actual_cookie = if cookie.is_empty() {
        // 尝试自动读取 Cookie
        crate::profile::try_load_chrome_cookie().unwrap_or_else(|| default_cookie.to_string())
    } else {
        cookie.to_string()
    };
    let post_url = format!("https://www.instagram.com/p/{}/", shortcode);

    log("启动浏览器...");

    let post_url_clone = post_url.clone();
    let cookie_clone = actual_cookie.to_string();
    let (images, caption) =
        tokio::task::spawn_blocking(move || fetch_with_browser(&post_url_clone, &cookie_clone))
            .await
            .context("浏览器任务失败")??;

    log(&format!("找到 {} 张图片", images.len()));
    for (i, img) in images.iter().enumerate() {
        log(&format!(
            "  图片 {}: {}...",
            i + 1,
            &img.url[..img.url.chars().count().min(80)]
        ));
    }

    if images.is_empty() {
        bail!("该帖子没有图片（可能是纯视频帖）");
    }

    Ok((images, caption))
}

/// 在无头浏览器中获取图片 URL 和文案
fn fetch_with_browser(post_url: &str, cookie: &str) -> Result<(Vec<ImageInfo>, String)> {
    let browser = Browser::new(
        LaunchOptions::default_builder()
            .headless(true)
            .build()
            .map_err(|e| {
                anyhow::anyhow!(
                    "无法启动浏览器: {}\n\n请安装 Chrome 或 Chromium:\n  macOS: brew install --cask google-chrome\n  Ubuntu/Debian: sudo apt install chromium-browser\n  Fedora: sudo dnf install chromium\n  Arch: sudo pacman -S chromium",
                    e
                )
            })?,
    )?;

    let tab = browser.new_tab()?;

    // 先访问 Instagram 域名以设置 cookie
    tab.navigate_to("https://www.instagram.com/")?;
    tab.wait_until_navigated()?;

    // 设置 cookie
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(eq_pos) = part.find('=') {
            let name = &part[..eq_pos];
            let value = &part[eq_pos + 1..];
            let _ = tab.set_cookies(vec![headless_chrome::protocol::cdp::Network::CookieParam {
                name: name.to_string(),
                value: value.to_string(),
                url: None,
                domain: Some(".instagram.com".to_string()),
                path: Some("/".to_string()),
                secure: Some(true),
                http_only: None,
                same_site: None,
                expires: None,
                priority: None,
                same_party: None,
                source_scheme: None,
                partition_key: None,
                source_port: None,
            }]);
        }
    }

    // 导航到帖子页面
    tab.navigate_to(post_url)?;
    tab.wait_until_navigated()?;
    std::thread::sleep(std::time::Duration::from_secs(3));

    // 提取文案
    let caption = extract_caption(&tab);
    if !caption.is_empty() {
        let preview: String = caption.chars().take(50).collect();
        eprintln!("[INFO] 帖子文案: {}...", preview);
    }

    // 获取轮播图总数
    let total = get_carousel_count(&tab).unwrap_or(1);
    eprintln!("[INFO] 轮播图总数: {}", total);

    // 边滑边提取
    let mut all_urls: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    collect_visible_images(&tab, &mut seen, &mut all_urls);
    eprintln!("[INFO] 初始提取 {} 张", all_urls.len());

    for i in 1..total {
        if all_urls.len() >= total {
            break;
        }

        let before = all_urls.len();
        click_next_button(&tab);

        for retry in 0..=4 {
            let wait = if i == total - 1 || retry > 0 { 900 } else { 1400 };
            std::thread::sleep(std::time::Duration::from_millis(wait));
            collect_visible_images(&tab, &mut seen, &mut all_urls);

            if all_urls.len() > before || all_urls.len() >= total {
                break;
            }
            if retry < 4 {
                eprintln!("[INFO] 滑动第 {} 次后未发现新图，重试 {}/4", i, retry + 1);
            }
        }
        eprintln!("[INFO] 已提取 {}/{} 张", all_urls.len(), total);
    }

    let mut images = Vec::new();
    for url in all_urls {
        images.push(ImageInfo { url, is_video: false });
    }

    Ok((images, caption))
}

/// 提取帖子文案
fn extract_caption(tab: &headless_chrome::Tab) -> String {
    let js = r#"
        (function() {
            // 文案 span 有独特类名 x126k92a
            var el = document.querySelector('span.x126k92a');
            if (el) return (el.innerText || '').trim();
            // 兜底: 找不嵌套在 a 标签内、不含 time 的长文本 span
            var spans = document.querySelectorAll('span');
            for (var i = 0; i < spans.length; i++) {
                var s = spans[i];
                if (s.children.length > 0) continue;
                if (s.closest('a')) continue;
                if (s.closest('time')) continue;
                var t = (s.innerText || '').trim();
                if (t.length >= 2 && !/^\d+\s*(天|小时|分钟|秒|周|月|年|day|hour|min)/i.test(t)) {
                    return t;
                }
            }
            return '';
        })()
    "#;
    match tab.evaluate(js, false) {
        Ok(result) => result
            .value
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// 获取轮播图总数（通过圆点指示器 _acnb 判断，单图返回 1）
fn get_carousel_count(tab: &headless_chrome::Tab) -> Option<usize> {
    let js = r#"
        (function() {
            var dots = document.querySelectorAll('._acnb');
            if (dots.length > 1) return dots.length;
            // 检查是否有轮播按钮
            var hasCarousel = document.querySelector('button._afxw') || document.querySelector('button[aria-label="下一步"]');
            if (hasCarousel) return dots.length || 2;
            return 1;
        })()
    "#;
    let result = tab.evaluate(js, false).ok()?;
    match result.value {
        Some(val) => val.as_u64().map(|n| n as usize),
        None => Some(1),
    }
}

/// 点击"下一步"按钮
fn click_next_button(tab: &headless_chrome::Tab) {
    let js = r#"
        (function() {
            var btn = document.querySelector('button._afxw') ||
                      document.querySelector('button[aria-label="下一步"]');
            if (btn) { btn.click(); return true; }
            return false;
        })()
    "#;
    let _ = tab.evaluate(js, false);
}

fn collect_visible_images(
    tab: &headless_chrome::Tab,
    seen: &mut std::collections::HashSet<String>,
    all_urls: &mut Vec<String>,
) {
    let urls = extract_visible_images(tab);
    if urls.is_empty() {
        log_image_diagnostics(tab);
    }

    for url in urls {
        if seen.insert(url.clone()) {
            let preview: String = url.chars().take(90).collect();
            eprintln!("[INFO] 捕获图片 URL: {}...", preview);
            all_urls.push(url);
        }
    }
}

fn log_image_diagnostics(tab: &headless_chrome::Tab) {
    let js = r#"
        (function() {
            var imgs1 = document.querySelectorAll('ul li ._aagu img');
            var imgs2 = document.querySelectorAll('article ._aagu img');
            var imgs3 = document.querySelectorAll('._aagu img');
            var lines = [];
            lines.push('carouselImgCount=' + imgs1.length);
            lines.push('articleImgCount=' + imgs2.length);
            lines.push('allAaguImgCount=' + imgs3.length);
            var imgs = imgs1.length > 0 ? imgs1 : (imgs2.length > 0 ? imgs2 : imgs3);
            Array.prototype.slice.call(imgs).slice(0, 5).forEach(function(img, index) {
                lines.push(
                    'img[' + index + '] src=' + (img.src || '').substring(0, 100) +
                    ' size=' + (img.naturalWidth || 0) + 'x' + (img.naturalHeight || 0)
                );
            });
            return lines.join('\n');
        })()
    "#;

    match tab.evaluate(js, false) {
        Ok(result) => eprintln!("[DEBUG] 轮播图片诊断: {:?}", result.value),
        Err(e) => eprintln!("[DEBUG] 轮播图片诊断失败: {}", e),
    }
}

/// 提取当前 DOM 中帖子图片 URL（支持轮播和单图）
fn extract_visible_images(tab: &headless_chrome::Tab) -> Vec<String> {
    let js = r#"
        (function() {
            // 优先从 ul li（轮播）提取
            var imgs = document.querySelectorAll('ul li ._aagu img');
            // 单图帖：用 ._aa20 精确匹配（帖子图片独有）
            if (imgs.length === 0) {
                imgs = document.querySelectorAll('._aagu._aa20 img');
            }
            // 还没有，从 article 内提取
            if (imgs.length === 0) {
                imgs = document.querySelectorAll('article ._aagu img');
            }
            var urls = [];
            var seen = {};
            for (var i = 0; i < imgs.length; i++) {
                var img = imgs[i];
                var src = img.currentSrc || img.src || '';
                if (!src || src.indexOf('scontent') === -1) continue;
                src = src.replace(/&amp;/g, '&').replace(/\\u0026/g, '&');
                if (seen[src]) continue;
                seen[src] = true;
                urls.push(src);
            }
            return urls.join('\n');
        })()
    "#;
    let result = match tab.evaluate(js, false) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("[WARN] 图片提取脚本执行失败: {}", e);
            return Vec::new();
        }
    };
    match result.value {
        Some(val) => val
            .as_str()
            .map(|items| {
                items
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// 下载所有图片到指定目录
pub async fn download_images(
    images: &[ImageInfo],
    save_dir: &Path,
    log: &ProgressCallback,
) -> Result<Vec<String>> {
    ensure_dir(save_dir)?;

    let client = build_download_client()?;
    let mut downloaded = Vec::new();

    for (i, img) in images.iter().enumerate() {
        let filename = make_filename(&img.url);
        let filepath = save_dir.join(&filename);

        log(&format!(
            "正在下载 {}/{}: {}",
            i + 1,
            images.len(),
            filename
        ));

        match download_single(&client, &img.url, &filepath).await {
            Ok(size) => {
                log(&format!(
                    "[OK] 已保存: {} ({:.1} KB)",
                    filename,
                    size as f64 / 1024.0
                ));
                downloaded.push(filename);
            }
            Err(e) => {
                log(&format!("[ERR] 下载失败 {}: {}", filename, e));
            }
        }
    }

    Ok(downloaded)
}

/// 下载单个图片
async fn download_single(client: &Client, url: &str, path: &Path) -> Result<u64> {
    let resp = client.get(url).send().await?;

    if !resp.status().is_success() {
        bail!("HTTP {}", resp.status());
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if !content_type.starts_with("image/") {
        bail!("响应不是图片 (Content-Type: {})", content_type);
    }

    let bytes = resp.bytes().await?;
    let size = bytes.len() as u64;

    if size < 1000 {
        bail!("文件太小 ({} bytes)", size);
    }

    fs::write(path, &bytes).await?;
    Ok(size)
}
