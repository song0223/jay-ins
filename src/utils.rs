use regex::Regex;
use std::path::Path;

/// 从 Instagram 链接中提取 shortcode
pub fn extract_shortcode(url: &str) -> Option<String> {
    let re = Regex::new(r"instagram\.com/(?:p|reel|tv)/([A-Za-z0-9_-]+)").ok()?;
    re.captures(url)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

pub fn strip_query_params(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
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
    fn test_make_filename() {
        let url = "https://scontent-nrt6-1.cdninstagram.com/v/t51.82787-15/713784089_18432697033193087_6645053608862494014_n.jpg?stp=dst-jpg_e35_s1080x1080";
        assert_eq!(
            make_filename(url),
            "713784089_18432697033193087_6645053608862494014_n.jpg"
        );
    }
}
