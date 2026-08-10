# Muyu

一个面向本地音频、语言学习和完全离线转写的跨平台悬浮字幕播放器。

[English](./README.md) | [简体中文](./README.zh-CN.md)

## 主要功能

- 导入本地音频文件
- 加载 `.srt` 与 `.vtt` 字幕
- 在桌面悬浮窗口中显示字幕
- 单句循环，适合精听练习
- 使用 `ffmpeg` 和 `whisper-cli` 在本地生成字幕
- 可在应用内下载默认 Whisper 模型
- 为 macOS 与 Windows 发布包准备平台 sidecar

## 技术栈

Tauri 2 · Rust · Svelte 5 · TypeScript · Vite · ffmpeg · whisper.cpp

## 快速开始

### 环境要求

- Node.js 与 npm
- Rust stable
- `ffmpeg` 和 `whisper-cli`，可放入 `PATH` 或准备为 Tauri sidecar
- 兼容的 Whisper 模型，例如 `ggml-base.bin`

### 本地开发

```bash
npm install
npm run tauri dev
```

应用会依次从环境变量、本地开发路径和打包 sidecar 中查找离线工具。可使用 `FFMPEG_BIN`、`WHISPER_CLI_BIN` 与 `WHISPER_MODEL_PATH` 指定路径。

## 离线转写流程

```text
导入音频
  → ffmpeg 转换为 16 kHz 单声道 WAV
  → whisper-cli 本地转写
  → 生成 SRT
  → 自动载入播放器
```

## 构建与发布

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

GitHub Actions 可通过版本 Tag 构建 Apple Silicon、Intel macOS 和 Windows x64 安装包。平台签名为可选配置，通过仓库 Secrets 提供。sidecar 发布流程见 [`docs/sidecar-release.md`](./docs/sidecar-release.md)。

## 隐私说明

字幕转写在本地完成，音频和字幕内容无需上传到远程服务。
