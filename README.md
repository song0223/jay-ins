# Jayins - Instagram 图片下载器

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey" alt="Platform">
  <img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version">
</p>

## ✨ 功能

- 🏠 **主页抓取** — 输入用户主页链接，获取帖子封面，点击复制链接
- 📥 **图片下载** — 输入帖子链接，自动滑动轮播图下载全部高清原图
- 📝 **文案提取** — 下载时自动提取帖子文案，点击复制
- 💻 **命令行模式** — `jayins <链接> [目录]`
- 🖥️ **跨平台 GUI** — 支持 macOS 和 Windows

## 📦 安装

### 方式一：直接下载可执行文件（推荐）

1. 克隆项目或下载 `bin/` 目录
2. 给执行权限（Mac/Linux）：
   ```bash
   chmod +x bin/jayins
   ```
3. 运行：
   ```bash
   ./bin/jayins --help
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

双击 `bin/jayins` 或无参数运行：

```bash
./bin/jayins
```

### 命令行模式

```bash
# 下载帖子图片（保存到 ~/Downloads/jayins/）
./bin/jayins https://www.instagram.com/p/ABC123/

# 指定保存目录
./bin/jayins https://www.instagram.com/p/ABC123/ ~/Pictures

# 查看帮助
./bin/jayins --help
```

## 📁 目录结构

```
bin/
├── jayins               # 启动脚本（自动检测平台）
├── jayins-macos-arm64   # macOS Apple Silicon 版本
└── jayins.cmd           # Windows 启动脚本（需自行编译 jayins.exe）
```

## 🔧 Windows 用户

Windows 版本需要自行编译：

```bash
# 安装 Rust: https://rustup.rs
git clone https://github.com/song0223/jay-ins.git
cd jay-ins
cargo build --release
# 可执行文件在 target/release/jayins.exe
```

## ⚠️ 注意事项

- 需要能访问 Instagram 的网络环境
- Cookie 默认内置，如需更新请修改源码
- 轮播帖会自动滑动加载所有图片

## 📄 License

MIT
