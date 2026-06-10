# Jayins - Instagram 图片下载器

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey" alt="Platform">
  <img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version">
</p>

## ✨ 功能

- 🏠 **主页抓取** — 输入用户主页链接，获取帖子封面，点击复制链接
- 📥 **图片下载** — 输入帖子链接，自动滑动轮播图下载全部高清原图
- 📝 **文案提取** — 下载时自动提取帖子文案，点击复制
- 💻 **命令行模式** — `jayins <链接> [目录]`
- 🖥️ **跨平台 GUI** — 支持 macOS、Windows、Linux

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
./jayins
```

### 命令行模式

```bash
# 下载帖子图片（保存到 ~/Downloads/jayins/）
./jayins https://www.instagram.com/p/ABC123/

# 指定保存目录
./jayins https://www.instagram.com/p/ABC123/ ~/Pictures

# 查看帮助
./jayins --help
```

## 📁 项目结构

```
bin/
├── jayins                 # 启动脚本（自动检测平台）
├── jayins-macos-arm64     # macOS ARM64
├── jayins-macos-x86_64    # macOS Intel
├── jayins-linux-x86_64    # Linux x86_64
└── jayins.cmd             # Windows 启动脚本

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
- Cookie 默认内置，如需更新请修改源码
- 轮播帖会自动滑动加载所有图片
- Linux 需要 GTK3 和 WebKit2GTK：`sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev`

## 📄 License

MIT
