# Jayins - Instagram 图片下载器

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey" alt="Platform">
  <img src="https://img.shields.io/badge/version-0.2.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/language-Rust-orange" alt="Language">
</p>

## ✨ 功能

### GUI 模式
- 🏠 **主页抓取** — 输入用户主页链接，获取帖子封面网格，点击复制链接
- 📥 **图片下载** — 输入帖子链接，自动滑动轮播图下载全部高清原图
- 📝 **文案提取** — 下载时自动提取帖子文案，点击复制
- 🖼️ **封面预览** — 主页帖子以封面网格展示，点击可复制链接

### 命令行模式
- 💻 **帖子下载** — `jayins <链接> [目录]`
- 📋 **主页抓取** — `jayins profile <主页链接>` 输出 JSON 格式
- 🔑 **Cookie 管理** — 支持 `-c` 参数、环境变量、配置文件、自动读取 Chrome

## 📦 安装

### 方式一：下载预编译版本（推荐）

从 [Releases](https://github.com/song0223/jay-ins/releases) 下载对应平台的版本：

| 平台 | 文件 |
|------|------|
| macOS (Apple Silicon) | `jayins-macos-arm64.tar.gz` |
| macOS (Intel) | `jayins-macos-x86_64.tar.gz` |
| Linux (x86_64) | `jayins-linux-x86_64.tar.gz` |
| Windows (x86_64) | `jayins-windows-x86_64.zip` |

解压后运行：
```bash
chmod +x jayins-*  # Mac/Linux
./jayins-* --help
```

### 方式二：从源码编译

```bash
git clone https://github.com/song0223/jay-ins.git
cd jay-ins
cargo build --release
./target/release/jayins --help
```

## 🚀 使用方法

### GUI 模式

无参数运行启动图形界面：

```bash
jayins
```

### 命令行模式

```bash
# 查看帮助
jayins --help

# 下载帖子图片（保存到 ~/Downloads/jayins/）
jayins https://www.instagram.com/p/ABC123/

# 指定保存目录
jayins https://www.instagram.com/p/ABC123/ ~/Pictures

# 获取主页帖子列表（JSON 格式）
jayins profile https://www.instagram.com/jaychou/

# 保存到文件
jayins profile https://www.instagram.com/jaychou/ > posts.json

# 用 jq 处理
jayins profile https://www.instagram.com/jaychou/ | jq '.[].url'
```

### Cookie 设置

macOS 会自动从 Chrome 读取 Cookie，无需手动配置。

Linux 或其他平台需要手动设置 Cookie（按优先级）：

```bash
# 方式1: 命令行参数
jayins -c 'sessionid=xxx; ds_user_id=xxx' profile https://www.instagram.com/jaychou/

# 方式2: 环境变量
export INSTAGRAM_COOKIE='sessionid=xxx; ds_user_id=xxx'
jayins profile https://www.instagram.com/jaychou/

# 方式3: 配置文件
mkdir -p ~/.config/jayins
echo 'sessionid=xxx; ds_user_id=xxx' > ~/.config/jayins/cookie.txt
```

**获取 Cookie 方法：**
1. 浏览器登录 Instagram
2. F12 → Console → 输入 `document.cookie`
3. 复制输出（需要包含 `sessionid`）

## 📁 项目结构

```
bin/
├── jayins                 # 启动脚本（自动检测平台）
├── jayins-macos-arm64     # macOS ARM64
├── jayins-macos-x86_64    # macOS Intel
├── jayins-linux-x86_64    # Linux x86_64
└── jayins.cmd             # Windows 启动脚本

src/
├── main.rs                # 入口（GUI/CLI 分发）
├── app.rs                 # GUI 界面
├── downloader.rs          # 图片下载核心
├── profile.rs             # 主页抓取
└── utils.rs               # 工具函数

.github/workflows/
└── build.yml              # GitHub Actions 自动编译
```

## 🔨 自动构建

项目使用 GitHub Actions 自动编译所有平台版本。推送 tag 时自动触发：

```bash
git tag v0.2.0
git push origin v0.2.0
# 自动编译并发布到 Releases
```

## ⚠️ 注意事项

- 需要能访问 Instagram 的网络环境
- macOS 自动从 Chrome 读取 Cookie，需保持 Chrome 登录状态
- 轮播帖会自动滑动加载所有图片
- Linux 需要 Chrome/Chromium：`sudo apt install chromium-browser`
- Linux 需要 GTK3：`sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev`
- Cookie 有时效性，过期后需重新获取

## 📄 License

MIT
