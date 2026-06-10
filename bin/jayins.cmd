@echo off
REM Jayins - Instagram 图片下载器
REM Windows 用户请先编译: cargo build --release
REM 然后将 jayins.exe 复制到此目录

if exist "%~dp0jayins.exe" (
    "%~dp0jayins.exe" %*
) else (
    echo 未找到 jayins.exe，请先编译: cargo build --release
    echo 然后将 target\release\jayins.exe 复制到此目录
    exit /b 1
)
