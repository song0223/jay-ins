use anyhow::{bail, Context, Result};
use headless_chrome::{Browser, LaunchOptions};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProfilePost {
    pub url: String,
    pub cover_url: String,
    pub timestamp: String,
    #[serde(skip)]
    pub cover_bytes: Vec<u8>,
}

const DEFAULT_COOKIE: &str =
    "ds_user_id=6009511404; csrftoken=en2hyrbjkI3AjRBUKDUPcaLyNsGYhocx; wd=1671x626; sessionid=6009511404%3ADt7ylCb1z380Fq%3A6%3AAYi90RxHDVdEZ36B89y1V91Gt64gvDDZ02i1Q7NZBjg";

/// 续期 Cookie（访问 Instagram 保持会话活跃）
pub async fn keepalive() -> Result<()> {
    let cookie = try_load_chrome_cookie()
        .unwrap_or_else(|| DEFAULT_COOKIE.to_string());

    eprintln!("[INFO] Cookie: {}...", &cookie[..cookie.len().min(30)]);

    // 使用 curl 发送请求（兼容性更好）
    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-o", "/dev/null",
            "-w", "%{http_code}",
            "-H", &format!("Cookie: {}", cookie),
            "-H", "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
            "https://www.instagram.com/",
        ])
        .output()
        .map_err(|e| anyhow::anyhow!("执行 curl 失败: {}", e))?;

    let status_code = String::from_utf8_lossy(&output.stdout).trim().to_string();
    eprintln!("[INFO] Instagram 响应: {}", status_code);

    if status_code == "200" {
        eprintln!("[INFO] Cookie 有效，会话已续期");
        Ok(())
    } else {
        bail!("请求失败: HTTP {}", status_code)
    }
}

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
    fetch_profile_posts_with_covers(profile_url, false).await
}

pub async fn fetch_profile_posts_with_covers(profile_url: &str, download_covers: bool) -> Result<Vec<ProfilePost>> {
    let profile_url = crate::utils::normalize_profile_url(profile_url)
        .context("请输入正确的 Instagram 主页链接")?;

    tokio::task::spawn_blocking(move || fetch_with_browser(&profile_url, download_covers))
        .await
        .context("浏览器任务失败")?
}

fn fetch_with_browser(profile_url: &str, download_covers: bool) -> Result<Vec<ProfilePost>> {
    // 尝试从 Chrome 读取最新 Cookie，否则用默认值
    let chrome_cookie = try_load_chrome_cookie();
    let cookie_to_use = chrome_cookie.as_deref().unwrap_or(DEFAULT_COOKIE);

    // 方式1: 尝试 API 方式（更可靠）
    eprintln!("[INFO] 尝试 API 方式获取...");
    match fetch_via_api(profile_url, cookie_to_use) {
        Ok(posts) if !posts.is_empty() => {
            eprintln!("[INFO] API 方式成功，找到 {} 个帖子", posts.len());
            return Ok(posts);
        }
        Ok(_) => eprintln!("[INFO] API 方式未返回帖子，尝试浏览器方式..."),
        Err(e) => eprintln!("[INFO] API 方式失败: {}，尝试浏览器方式...", e),
    }

    // 方式2: 浏览器方式
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
    let _ = tab.set_default_timeout(std::time::Duration::from_secs(30));

    eprintln!("[INFO] 设置 Cookie...");
    set_cookie_with(&tab, cookie_to_use);

    eprintln!("[INFO] 访问主页: {}", profile_url);
    match tab.navigate_to(profile_url) {
        Ok(_) => {}
        Err(_) => eprintln!("[WARN] 导航超时，继续尝试..."),
    }
    std::thread::sleep(std::time::Duration::from_secs(5));

    // 等待帖子元素出现
    for i in 0..10 {
        let count = count_post_anchors(&tab);
        if count > 0 {
            eprintln!("[INFO] 帖子已加载: {} 个", count);
            break;
        }
        eprintln!("[INFO] 等待帖子加载... ({}/10)", i + 1);
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    let _ = tab.evaluate("window.scrollTo(0, document.body.scrollHeight)", false);
    std::thread::sleep(std::time::Duration::from_secs(2));

    diagnose_page(&tab);

    let mut posts = extract_posts(&tab);
    eprintln!("[INFO] 找到 {} 个帖子", posts.len());

    if download_covers {
        for post in &mut posts {
            post.cover_bytes = download_image_via_chrome(&tab, &post.cover_url);
        }
    }

    Ok(posts)
}

/// 通过 Instagram GraphQL API 获取主页帖子（更可靠）
fn fetch_via_api(profile_url: &str, cookie: &str) -> Result<Vec<ProfilePost>> {
    let username = crate::utils::normalize_profile_url(profile_url)
        .and_then(|u| {
            u.trim_end_matches('/').rsplit('/').next().map(|s| s.to_string())
        })
        .context("无法从链接提取用户名")?;

    eprintln!("[INFO] 用户名: {}", username);

    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
        .build()?;

    // 获取用户 ID（通过 web_profile_info）
    let resp = client
        .get(&format!("https://www.instagram.com/api/v1/users/web_profile_info/?username={}", username))
        .header("X-IG-App-ID", "936619743392459")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Cookie", cookie)
        .send()?;

    if !resp.status().is_success() {
        bail!("用户信息请求失败: {}", resp.status());
    }

    let data: serde_json::Value = resp.json()?;

    // 从 GraphQL 数据中提取帖子
    let user_data = data.pointer("/data/user")
        .context("无法获取用户数据")?;

    let edges = user_data
        .pointer("/edge_owner_to_timeline_media/edges")
        .and_then(|v| v.as_array())
        .context("无法获取帖子列表")?;

    let mut posts = Vec::new();
    for edge in edges {
        let node = edge.get("node").unwrap_or(edge);
        let shortcode = node.get("shortcode")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let is_video = node.get("is_video")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let display_url = node.get("display_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let url = if is_video {
            format!("https://www.instagram.com/reel/{}/", shortcode)
        } else {
            format!("https://www.instagram.com/p/{}/", shortcode)
        };

        posts.push(ProfilePost {
            url,
            cover_url: display_url.to_string(),
            timestamp: String::new(),
            cover_bytes: Vec::new(),
        });
    }

    Ok(posts)
}

fn count_post_anchors(tab: &headless_chrome::Tab) -> usize {
    let js = r#"
        (function() {
            var anchors = document.querySelectorAll('a[href*="/p/"], a[href*="/reel/"], a[href*="/tv/"]');
            var postRe = /^\/[A-Za-z0-9._]+\/(p|reel|tv)\/[A-Za-z0-9_-]+\/?$/;
            var count = 0;
            for (var i = 0; i < anchors.length; i++) {
                if (postRe.test(anchors[i].getAttribute('href') || '')) count++;
            }
            return count;
        })()
    "#;
    match tab.evaluate(js, false) {
        Ok(result) => result.value.and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        Err(_) => 0,
    }
}

fn diagnose_page(tab: &headless_chrome::Tab) {
    let js = r#"
        (function() {
            var url = window.location.href;
            var title = document.title;
            var body = document.body ? document.body.innerText.substring(0, 500) : 'no body';
            var anchors = document.querySelectorAll('a[href*="/p/"], a[href*="/reel/"]');
            var postRe = /^\/[A-Za-z0-9._]+\/(p|reel|tv)\/[A-Za-z0-9_-]+\/?$/;
            var postCount = 0;
            for (var i = 0; i < anchors.length; i++) {
                if (postRe.test(anchors[i].getAttribute('href') || '')) postCount++;
            }
            var allLinks = document.querySelectorAll('a').length;
            var imgs = document.querySelectorAll('img').length;
            return 'url=' + url + '\ntitle=' + title + '\npostAnchors=' + postCount +
                   '\nallLinks=' + allLinks + '\nimgCount=' + imgs +
                   '\nbodyPreview=' + body.replace(/\n/g, ' ').substring(0, 200);
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
            let shortcode = href.split('/').filter(|s| !s.is_empty()).last().unwrap_or("").to_string();
            Some(ProfilePost {
                url,
                cover_url: cover_url.to_string(),
                timestamp: String::new(),
                cover_bytes: Vec::new(),
            })
        })
        .collect()
}
