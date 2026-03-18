# Floating Subtitle Player

基于 `Tauri 2 + Rust + Vanilla TypeScript` 的跨平台桌面悬浮字幕播放器，当前已经支持：

- 导入本地音频
- 导入 `.srt` / `.vtt` 字幕
- 悬浮字幕窗显示
- 单句循环
- 本地离线字幕生成：`ffmpeg + whisper-cli`

## 本地运行

```bash
cd /Users/zhaodesen/Desktop/desktop-client
. "$HOME/.cargo/env"
npm install
npm run tauri dev
```

## 离线识别依赖

当前离线识别会按下面顺序查找依赖。正式版会优先使用打包进去的 sidecar：

### `ffmpeg`

- 环境变量 `FFMPEG_BIN`
- `PATH` 中的 `ffmpeg`
- 项目目录下的 `./bin/ffmpeg`
- 项目目录下的 `./src-tauri/binaries/ffmpeg`

### `whisper-cli`

- 环境变量 `WHISPER_CLI_BIN`
- `PATH` 中的 `whisper-cli`
- 项目目录下的 `./bin/whisper-cli`
- 项目目录下的 `./src-tauri/binaries/whisper-cli`

正式版请把平台对应的二进制放到：

```text
src-tauri/binaries/
```

并遵守 Tauri 的 `externalBin` 命名规则，例如 macOS Apple Silicon：

```text
src-tauri/binaries/ffmpeg-aarch64-apple-darwin
src-tauri/binaries/whisper-cli-aarch64-apple-darwin
```

macOS Intel：

```text
src-tauri/binaries/ffmpeg-x86_64-apple-darwin
src-tauri/binaries/whisper-cli-x86_64-apple-darwin
```

Windows：

```text
src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe
src-tauri/binaries/whisper-cli-x86_64-pc-windows-msvc.exe
```

Linux：

```text
src-tauri/binaries/ffmpeg-x86_64-unknown-linux-gnu
src-tauri/binaries/whisper-cli-x86_64-unknown-linux-gnu
```

### Whisper 模型

模型文件默认查找：

- 环境变量 `WHISPER_MODEL_PATH`
- `应用数据目录/models/ggml-base.bin`
- 项目目录下的 `./models/ggml-base.bin`
- 项目目录下的 `./src-tauri/models/ggml-base.bin`

推荐先使用：

```text
./models/ggml-base.bin
```

应用内也已经支持直接下载默认 `base` 模型。

## 推荐的最小本地准备

准备 sidecar：

```bash
cd /Users/zhaodesen/Desktop/desktop-client
FFMPEG_SOURCE=/absolute/path/to/ffmpeg ./scripts/build-sidecars.sh
./scripts/verify-sidecars.sh
```

然后准备模型：

```bash
mkdir -p /Users/zhaodesen/Desktop/desktop-client/models
```

再选择其一：

- 把 Whisper 模型放到 [models/ggml-base.bin](/Users/zhaodesen/Desktop/desktop-client/models/ggml-base.bin)
- 或直接在应用内下载默认 `base` 模型

更完整的发布规范见：
- [sidecar-release.md](/Users/zhaodesen/Desktop/desktop-client/docs/sidecar-release.md)

## 当前离线识别流程

```text
导入音频
→ 点击“生成字幕”
→ Rust 启动后台任务
→ ffmpeg 转 16k 单声道 wav
→ whisper-cli 输出 srt
→ 主窗口自动加载生成的字幕
```

## 已验证

以下命令已经通过：

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```
