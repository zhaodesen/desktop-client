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

## 上传到 GitHub

如果你还没有远程仓库，可以先在 GitHub 上创建一个空仓库，然后在本地执行：

```bash
cd /Users/zhaodesen/Desktop/desktop-client
git remote add origin <你的 GitHub 仓库地址>
git push -u origin master
```

如果已经有远程仓库，只需要：

```bash
cd /Users/zhaodesen/Desktop/desktop-client
git push -u origin master
```

## GitHub Actions 自动打包

项目已添加工作流：

- [release.yml](/Users/zhaodesen/Desktop/desktop-client/.github/workflows/release.yml)

触发方式：

- 推送版本标签：`v0.1.0`、`v0.2.0` 这类 tag
- 或在 GitHub Actions 页面手动执行 `Release Desktop App`

当前工作流会自动构建：

- macOS Apple Silicon：`aarch64-apple-darwin`
- macOS Intel：`x86_64-apple-darwin`
- Windows x64：`x86_64-pc-windows-msvc`

构建完成后，安装包会上传到当前版本对应的 GitHub Release 页面，你可以直接在 Release 的 `Assets` 区域下载。

### 发版命令

每次想发新版本时，建议先同步版本号：

- [package.json](/Users/zhaodesen/Desktop/desktop-client/package.json)
- [src-tauri/tauri.conf.json](/Users/zhaodesen/Desktop/desktop-client/src-tauri/tauri.conf.json)

然后执行：

```bash
cd /Users/zhaodesen/Desktop/desktop-client
git add .
git commit -m "release: v0.1.0"
git push origin master
git tag v0.1.0
git push origin v0.1.0
```

### 重要说明

- `src-tauri/binaries/` 下必须提前准备好对应平台的 `ffmpeg` 和 `whisper-cli` sidecar，否则 GitHub Actions 打包会失败。
- 目前这个工作流会生成未签名安装包。macOS 首次打开可能提示安全警告，Windows 也可能提示未知发布者。
- 如果后续你要做正式分发，可以继续补充 macOS 签名、公证，以及 Windows 代码签名。
