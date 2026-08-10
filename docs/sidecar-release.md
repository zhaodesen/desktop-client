# Sidecar 获取与发布规范

## 目标

正式版不依赖用户本机安装：

- 不依赖系统 `ffmpeg`
- 不依赖系统 `whisper-cli`
- 通过 Tauri `externalBin` 将二进制随应用一起打包

当前配置已经在 [tauri.conf.json](/Users/zhaodesen/Desktop/desktop-client/src-tauri/tauri.conf.json) 中启用：

```json
"externalBin": ["binaries/ffmpeg", "binaries/whisper-cli"]
```

根据 Tauri 官方文档，外部二进制必须遵守：

```text
binary-name{-target-triple}{.system-extension}
```

来源：
- [Tauri Embedding External Binaries](https://v2.tauri.app/develop/sidecar/)

## 命名规则

### macOS Apple Silicon

```text
src-tauri/binaries/ffmpeg-aarch64-apple-darwin
src-tauri/binaries/whisper-cli-aarch64-apple-darwin
```

### macOS Intel

```text
src-tauri/binaries/ffmpeg-x86_64-apple-darwin
src-tauri/binaries/whisper-cli-x86_64-apple-darwin
```

### Windows

```text
src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe
src-tauri/binaries/whisper-cli-x86_64-pc-windows-msvc.exe
```

### Linux

```text
src-tauri/binaries/ffmpeg-x86_64-unknown-linux-gnu
src-tauri/binaries/whisper-cli-x86_64-unknown-linux-gnu
```

## 获取策略

Windows 本地开发和构建现在会在缺失 sidecar 时自动调用 [prepare-sidecars-windows.ps1](/Users/zhaodesen/Desktop/desktop-client/scripts/prepare-sidecars-windows.ps1)，自动补齐：

- `ffmpeg-x86_64-pc-windows-msvc.exe`
- `whisper-cli-x86_64-pc-windows-msvc.exe`

如果机器上已经有现成的原生可执行文件，也可以直接复用这些运行时环境变量：

- `FFMPEG_BIN`
- `WHISPER_CLI_BIN`

## 1. whisper-cli

推荐从官方源码自行编译，而不是依赖第三方散装二进制。

官方仓库：
- [ggml-org/whisper.cpp](https://github.com/ggml-org/whisper.cpp)

脚本已提供自动化处理：

```bash
cd /Users/zhaodesen/Desktop/desktop-client
FFMPEG_SOURCE=/absolute/path/to/ffmpeg ./scripts/build-sidecars.sh
```

脚本行为：

- 自动识别当前 target triple
- 如无本地源码则浅克隆 `whisper.cpp`
- 编译 `whisper-cli`
- 重命名并复制到 `src-tauri/binaries/`

可选环境变量：

- `WHISPER_CPP_DIR`
  - 指定本地 whisper.cpp 源码目录
- `WHISPER_CPP_REF`
  - 指定克隆分支或 tag，默认 `master`
- `WHISPER_CLI_SOURCE`
  - 直接指定要复制进 `src-tauri/binaries/` 的 `whisper-cli`
- `WHISPER_CLI_BIN`
  - 与运行时变量同名，脚本会把它当作 `WHISPER_CLI_SOURCE` 使用

## 2. ffmpeg

FFmpeg 官方主要提供源码，已编译二进制通常通过其下载页列出的来源获取。

官方页面：
- [FFmpeg 下载页](https://ffmpeg.org/download.html)

建议策略：

- 团队内部固定一个可信来源
- 固定版本号
- 下载后做校验
- 将目标平台的产物纳入发布流水线

当前脚本不负责构建 ffmpeg，只负责把你准备好的 `ffmpeg` 复制到正确命名的位置：

```bash
FFMPEG_SOURCE=/absolute/path/to/ffmpeg ./scripts/build-sidecars.sh
```

也可以直接复用运行时变量：

```bash
FFMPEG_BIN=/absolute/path/to/ffmpeg ./scripts/build-sidecars.sh
```

## 发布前检查

准备好 sidecar 后，执行：

```bash
cd /Users/zhaodesen/Desktop/desktop-client
./scripts/verify-sidecars.sh
```

检查项：

- 当前平台对应的 `ffmpeg` 是否存在
- 当前平台对应的 `whisper-cli` 是否存在
- 两个二进制是否都能执行

## 构建流程建议

### 本地调试

```bash
. "$HOME/.cargo/env"
./scripts/verify-sidecars.sh
npm run tauri dev
```

Windows 上如果缺少 sidecar，可以直接执行 `npm run tauri dev`，预检脚本会先自动准备。

### 正式构建

```bash
. "$HOME/.cargo/env"
./scripts/verify-sidecars.sh
npm run tauri build
```

Windows 上如果缺少 sidecar，可以直接执行 `npm run tauri build`，预检脚本会先自动准备。

## 模型文件

模型不应进入安装包。

当前项目会优先使用：

- 应用数据目录中的 `models/ggml-base.bin`
- 项目目录中的 `models/ggml-base.bin`

应用内已经支持下载默认 `base` 模型。
