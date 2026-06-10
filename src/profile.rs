use anyhow::{Context, Result};
use headless_chrome::{Browser, LaunchOptions};

#[derive(Debug, Clone)]
pub struct ProfilePost {
    pub url: String,
    pub cover_url: String,
    pub cover_bytes: Vec<u8>,
}

const DEFAULT_COOKIE: &str =
    "ds_user_id=6009511404; csrftoken=en2hyrbjkI3AjRBUKDUPcaLyNsGYhocx; wd=1671x626";

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

    tab.navigate_to("https://www.instagram.com/")?;
    tab.wait_until_navigated()?;
    set_cookie(&tab);

    tab.navigate_to(profile_url)?;
    tab.wait_until_navigated()?;
    std::thread::sleep(std::time::Duration::from_secs(3));

    let mut posts = extract_posts(&tab);

    // 通过 Chrome 下载封面图（绕过 CDN 限制）
    for post in &mut posts {
        post.cover_bytes = download_image_via_chrome(&tab, &post.cover_url);
    }

    Ok(posts)
}

fn set_cookie(tab: &headless_chrome::Tab) {
    for part in DEFAULT_COOKIE.split(';') {
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
