use regex::Regex;
use std::path::Path;

/// 从 Instagram 链接中提取 shortcode
pub fn extract_shortcode(url: &str) -> Option<String> {
    // 支持 /p/xxx、/reel/xxx、/tv/xxx，以及带用户名前缀的 /user/p/xxx
    let re = Regex::new(r"instagram\.com/(?:[A-Za-z0-9._]+/)?(?:p|reel|tv)/([A-Za-z0-9_-]+)").ok()?;
    re.captures(url)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

pub fn strip_query_params(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

pub fn normalize_profile_url(url: &str) -> Option<String> {
    let stripped = strip_query_params(url.trim())
        .trim_end_matches('/')
        .to_string();
    let re = Regex::new(r"^https://(?:www\.)?instagram\.com/([A-Za-z0-9._]+)/?$").ok()?;
    let username = re
        .captures(&stripped)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str())?;
    Some(format!("https://www.instagram.com/{}/", username))
}

pub fn absolute_instagram_url(href: &str) -> Option<String> {
    let clean = strip_query_params(href.trim());
    if clean.starts_with("https://www.instagram.com/") {
        return Some(ensure_trailing_slash(clean));
    }
    // 匹配 /p/xxx、/reel/xxx、/tv/xxx（带或不带用户名前缀）
    if clean.contains("/p/") || clean.contains("/reel/") || clean.contains("/tv/") {
        let path = if clean.starts_with('/') {
            clean.clone()
        } else {
            format!("/{}", clean)
        };
        return Some(format!("https://www.instagram.com{}", ensure_trailing_slash(path)));
    }
    None
}

fn ensure_trailing_slash(mut url: String) -> String {
    if !url.ends_with('/') {
        url.push('/');
    }
    url
}

/// 从 CDN URL 中提取原始文件名
/// 例: https://scontent.cdninstagram.com/v/t51.82787-15/713784089_xxx_n.jpg?stp=...
/// → 713784089_xxx_n.jpg
pub fn make_filename(url: &str) -> String {
    // 从 URL 路径中提取文件名
    if let Some(name) = extract_original_filename(url) {
        return name;
    }
    // 兜底：用时间戳
    format!(
        "image_{}.jpg",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    )
}

/// 从 CDN URL 提取原始文件名
fn extract_original_filename(url: &str) -> Option<String> {
    // URL 格式: .../v/t51.82787-15/713784089_xxx_n.jpg?stp=...
    let re = Regex::new(r"/([0-9]+_[0-9]+_[0-9]+_n\.\w+)").ok()?;
    re.captures(url)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

/// 确保目录存在
pub fn ensure_dir(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_shortcode() {
        assert_eq!(
            extract_shortcode("https://www.instagram.com/p/ABC123/"),
            Some("ABC123".to_string())
        );
    }

    #[test]
    fn test_strip_query_params() {
        assert_eq!(
            strip_query_params("https://www.instagram.com/p/ABC123/?igsh=abc&x=1"),
            "https://www.instagram.com/p/ABC123/"
        );
    }

    #[test]
    fn test_normalize_profile_url() {
        assert_eq!(
            normalize_profile_url("https://www.instagram.com/jaychou/?igsh=abc"),
            Some("https://www.instagram.com/jaychou/".to_string())
        );
    }

    #[test]
    fn test_absolute_post_url() {
        assert_eq!(
            absolute_instagram_url("/p/ABC123/"),
            Some("https://www.instagram.com/p/ABC123/".to_string())
        );
    }

    #[test]
    fn test_make_filename() {
        let url = "https://scontent-nrt6-1.cdninstagram.com/v/t51.82787-15/713784089_18432697033193087_6645053608862494014_n.jpg?stp=dst-jpg_e35_s1080x1080";
        assert_eq!(
            make_filename(url),
            "713784089_18432697033193087_6645053608862494014_n.jpg"
        );
    }
}
