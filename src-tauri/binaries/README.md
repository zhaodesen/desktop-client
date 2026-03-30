# Sidecar 说明

这些文件是随仓库一起维护的预置 sidecar。CI Release 和本地构建现在都只做校验，不再在构建阶段临时下载。

## 当前来源

### macOS

- `yt-dlp-aarch64-apple-darwin`
  - 来源：`yt-dlp` 官方 Release
  - 版本：`2026.03.17`
  - 说明：官方 `yt-dlp_macos` 是通用二进制，同时兼容 `arm64` / `x86_64`
- `yt-dlp-x86_64-apple-darwin`
  - 来源：同上
  - 版本：`2026.03.17`
- `whisper-cli-aarch64-apple-darwin`
  - 来源：`whisper.cpp` 官方源码 `v1.8.4`
  - 说明：由仓库维护者本地编译 `arm64` / `x86_64` 后合成通用二进制
- `whisper-cli-x86_64-apple-darwin`
  - 来源：同上
- `ffmpeg-aarch64-apple-darwin`
  - 来源：仓库原有文件
  - 当前版本：`8.0.1-https://www.martin-riedl.de`
- `ffmpeg-x86_64-apple-darwin`
  - 来源：`evermeet.cx` 提供的 macOS 预编译包
  - 当前版本：`N-123741-g368f58109e-tessus`

### Windows x64

- `yt-dlp-x86_64-pc-windows-msvc.exe`
  - 来源：`yt-dlp` 官方 Release
  - 版本：`2026.03.17`
- `whisper-cli-x86_64-pc-windows-msvc.exe`
  - 来源：`whisper.cpp` 官方 Release `v1.8.4`
- `ggml-base.dll`
  - 来源：同上
- `ggml-cpu.dll`
  - 来源：同上
- `ggml.dll`
  - 来源：同上
- `whisper.dll`
  - 来源：同上
- `ffmpeg-x86_64-pc-windows-msvc.exe`
  - 来源：`gyan.dev` 的 `ffmpeg-8.1-essentials_build.zip`

## 维护原则

- 构建流程不再自动拉取 sidecar，缺文件直接失败。
- Windows `whisper-cli` 依赖的 DLL 必须和 `exe` 一起维护。
- `yt-dlp` 运行时允许在应用数据目录下载更新覆盖版，优先级高于包内版本，但不会回写安装包目录。
- 如需切到自建镜像，可设置 `MUYU_YT_DLP_DOWNLOAD_URL` 或 `YT_DLP_DOWNLOAD_URL` 覆盖运行时更新地址。
