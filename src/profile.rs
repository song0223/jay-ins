use anyhow::{Context, Result};
use headless_chrome::{Browser, LaunchOptions};

#[derive(Debug, Clone)]
pub struct ProfilePost {
    pub url: String,
    pub cover_url: String,
    pub cover_bytes: Vec<u8>,
}

const DEFAULT_COOKIE: &str =
    "ds_user_id=6009511404; csrftoken=en2hyrbjkI3AjRBUKDUPcaLyNsGYhocx; wd=1671x626; sessionid=6009511404%3ADt7ylCb1z380Fq%3A6%3AAYi90RxHDVdEZ36B89y1V91Gt64gvDDZ02i1Q7NZBjg";

/// 尝试从 Chrome 读取最新的 Instagram Cookie
pub fn try_load_chrome_cookie() -> Option<String> {
    // 方式1: 从环境变量读取
    if let Ok(cookie) = std::env::var("INSTAGRAM_COOKIE") {
        if !cookie.is_empty() && cookie.contains("sessionid=") {
            eprintln!("[INFO] 从环境变量 INSTAGRAM_COOKIE 读取到 Cookie");
            return Some(cookie);
        }
    }

    // 方式2: 从配置文件读取
    if let Some(cookie) = load_cookie_from_config() {
        if cookie.contains("sessionid=") {
            eprintln!("[INFO] 从配置文件读取到 Cookie");
            return Some(cookie);
        }
    }

    // 方式3: 尝试从 Chrome 读取（需要 browser_cookie3）
    if let Some(cookie) = try_read_chrome_cookie() {
        return Some(cookie);
    }

    None
}

/// 从配置文件读取 Cookie
fn load_cookie_from_config() -> Option<String> {
    let config_path = dirs_next::config_dir()
        .or_else(|| dirs_next::home_dir().map(|h| h.join(".config")))?
        .join("jayins")
        .join("cookie.txt");

    if config_path.exists() {
        std::fs::read_to_string(&config_path).ok()
    } else {
        None
    }
}

/// 尝试从 Chrome 浏览器读取 Cookie
fn try_read_chrome_cookie() -> Option<String> {
    let script = r#"
import browser_cookie3
try:
    cj = browser_cookie3.chrome(domain_name='.instagram.com')
    cookies = {}
    for c in cj:
        cookies[c.name] = c.value
    parts = []
    for k in ['ds_user_id', 'csrftoken', 'wd', 'sessionid']:
        if k in cookies and cookies[k]:
            parts.append(f'{k}={cookies[k]}')
    if 'sessionid' in cookies:
        print('; '.join(parts))
except Exception as e:
    pass
"#;

    let output = std::process::Command::new("python3")
        .args(["-c", script])
        .output()
        .ok()?;

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if result.contains("sessionid=") {
        eprintln!("[INFO] 从 Chrome 读取到最新 Cookie");
        return Some(result);
    }
    None
}

pub async fn fetch_profile_posts(profile_url: &str) -> Result<Vec<ProfilePost>> {
    let profile_url = crate::utils::normalize_profile_url(profile_url)
        .context("请输入正确的 Instagram 主页链接")?;

    tokio::task::spawn_blocking(move || fetch_with_browser(&profile_url))
        .await
        .context("浏览器任务失败")?
}

fn fetch_with_browser(profile_url: &str) -> Result<Vec<ProfilePost>> {
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

    // 设置 User-Agent 让 Chrome 看起来像正常浏览器
    let _ = tab.evaluate(
        r#"Object.defineProperty(navigator, 'webdriver', {get: () => undefined})"#,
        false,
    );

    // 尝试从 Chrome 读取最新 Cookie，否则用默认值
    let chrome_cookie = try_load_chrome_cookie();
    let cookie_to_use = chrome_cookie.as_deref().unwrap_or(DEFAULT_COOKIE);

    eprintln!("[INFO] 设置 Cookie...");
    // 先访问 Instagram 域名以建立会话
    tab.navigate_to("https://www.instagram.com/")?;
    tab.wait_until_navigated()?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    set_cookie_with(&tab, cookie_to_use);
    // 刷新页面让 Cookie 生效
    tab.reload(false, None)?;
    tab.wait_until_navigated()?;
    std::thread::sleep(std::time::Duration::from_secs(2));

    eprintln!("[INFO] 访问主页: {}", profile_url);
    tab.navigate_to(profile_url)?;
    tab.wait_until_navigated()?;
    std::thread::sleep(std::time::Duration::from_secs(5));

    // 诊断页面状态
    diagnose_page(&tab);

    let mut posts = extract_posts(&tab);

    eprintln!("[INFO] 找到 {} 个帖子", posts.len());

    // 通过 Chrome 下载封面图（绕过 CDN 限制）
    for post in &mut posts {
        post.cover_bytes = download_image_via_chrome(&tab, &post.cover_url);
    }

    Ok(posts)
}

fn diagnose_page(tab: &headless_chrome::Tab) {
    let js = r#"
        (function() {
            var url = window.location.href;
            var title = document.title;
            var anchors = document.querySelectorAll('a[href*="/p/"], a[href*="/reel/"]');
            var postRe = /^\/[A-Za-z0-9._]+\/(p|reel|tv)\/[A-Za-z0-9_-]+\/?$/;
            var postCount = 0;
            for (var i = 0; i < anchors.length; i++) {
                if (postRe.test(anchors[i].getAttribute('href') || '')) postCount++;
            }
            return 'url=' + url + '\ntitle=' + title + '\npostAnchors=' + postCount;
        })()
    "#;
    match tab.evaluate(js, false) {
        Ok(result) => {
            if let Some(info) = result.value.and_then(|v| v.as_str().map(String::from)) {
                eprintln!("[DEBUG] 页面诊断:\n{}", info);
            }
        }
        Err(e) => eprintln!("[DEBUG] 诊断失败: {}", e),
    }
}

fn set_cookie(tab: &headless_chrome::Tab) {
    set_cookie_with(tab, DEFAULT_COOKIE);
}

fn set_cookie_with(tab: &headless_chrome::Tab, cookie_str: &str) {
    for part in cookie_str.split(';') {
        let part = part.trim();
        if let Some(eq_pos) = part.find('=') {
            let name = &part[..eq_pos];
            let value = &part[eq_pos + 1..];
            eprintln!("[INFO] 设置 Cookie: {}={}", name, value);
            match tab.set_cookies(vec![headless_chrome::protocol::cdp::Network::CookieParam {
                name: name.to_string(),
                value: value.to_string(),
                url: Some("https://www.instagram.com/".to_string()),
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
            }]) {
                Ok(_) => eprintln!("[OK] Cookie 设置成功: {}", name),
                Err(e) => eprintln!("[WARN] Cookie 设置失败: {} - {}", name, e),
            }
        }
    }
}

fn download_image_via_chrome(tab: &headless_chrome::Tab, url: &str) -> Vec<u8> {
    eprintln!("[INFO] 下载封面: {}...", &url[..url.len().min(80)]);

    // 用页面的 Image 对象绘制到 canvas，再导出 base64
    let js = format!(
        r#"
        (async function() {{
            try {{
                var img = new Image();
                img.crossOrigin = "anonymous";
                img.src = "{}";
                await new Promise(function(resolve, reject) {{
                    img.onload = resolve;
                    img.onerror = reject;
                    setTimeout(reject, 10000);
                }});
                var c = document.createElement('canvas');
                c.width = img.naturalWidth;
                c.height = img.naturalHeight;
                c.getContext('2d').drawImage(img, 0, 0);
                return c.toDataURL('image/jpeg', 0.8).split(',')[1] || '';
            }} catch(e) {{
                return '';
            }}
        }})()
        "#,
        url.replace('"', "\\\"").replace('\'', "\\'")
    );

    match tab.evaluate(js.as_str(), true) {
        Ok(result) => {
            if let Some(b64) = result.value.and_then(|v| v.as_str().map(ToString::to_string)) {
                if !b64.is_empty() {
                    use base64::Engine;
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64) {
                        eprintln!("[OK] 封面下载成功 {} bytes", bytes.len());
                        return bytes;
                    }
                }
            }
            eprintln!("[WARN] 封面下载失败: base64 为空");
            Vec::new()
        }
        Err(e) => {
            eprintln!("[WARN] 封面下载失败: {}", e);
            Vec::new()
        }
    }
}

fn extract_posts(tab: &headless_chrome::Tab) -> Vec<ProfilePost> {
    let js = r#"
        (function() {
            function cleanUrl(url) {
                if (!url) return '';
                return url.replace(/&amp;/g, '&').replace(/\\u0026/g, '&');
            }

            var anchors = document.querySelectorAll('a[href*="/p/"], a[href*="/reel/"], a[href*="/tv/"]');
            var seen = {};
            var lines = [];
            var postRe = /^\/[A-Za-z0-9._]+\/(p|reel|tv)\/[A-Za-z0-9_-]+\/?$/;
            for (var i = 0; i < anchors.length; i++) {
                var a = anchors[i];
                var href = a.getAttribute('href') || '';
                if (!href || seen[href]) continue;
                if (!postRe.test(href)) continue;

                var img = a.querySelector('img');
                var cover = img ? cleanUrl(img.currentSrc || img.src || '') : '';
                if (!cover || cover.indexOf('scontent') === -1) continue;

                seen[href] = true;
                lines.push(href + '\t' + cover);
            }
            return lines.join('\n');
        })()
    "#;

    let result = match tab.evaluate(js, false) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("[WARN] 主页帖子提取失败: {}", e);
            return Vec::new();
        }
    };

    result
        .value
        .and_then(|val| val.as_str().map(ToString::to_string))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (href, cover_url) = line.split_once('\t')?;
            let url = crate::utils::absolute_instagram_url(href)?;
            Some(ProfilePost {
                url,
                cover_url: cover_url.to_string(),
                cover_bytes: Vec::new(),
            })
        })
        .collect()
}
