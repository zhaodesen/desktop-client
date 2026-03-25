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

构建过程中还会自动完成：

- 在 CI 中安装 `ffmpeg`，并复制为 Tauri 需要的 sidecar 命名
- 在 CI 中编译 `whisper-cli`，并复制为 Tauri 需要的 sidecar 命名
- 如果已配置 secrets，macOS 使用 `Developer ID Application` 证书签名，并提交公证
- 如果已配置 secrets，Windows 使用 `.pfx` 证书签名

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

- GitHub Actions 不再依赖你手工把各平台 sidecar 提前提交到仓库；工作流会按目标平台自动准备。
- 不配置签名 secrets 也可以正常发布安装包。
- 没有签名时，macOS 首次打开可能需要右键打开或在系统设置里手动放行，Windows 也可能提示未知发布者。
- 如果你想减少系统安全提示，再去 GitHub 仓库 `Settings -> Secrets and variables -> Actions` 中配置签名密钥。

### macOS 首次打开说明

如果你下载的是未签名或未公证的 `dmg`，macOS 可能提示“无法验证开发者”或直接建议移入废纸篓。可以按下面顺序处理：

1. 把应用从 `dmg` 拖到 `Applications`
2. 在“应用程序”里对 App 右键，选择“打开”
3. 如果仍被拦截，打开：
   `系统设置 -> 隐私与安全性`
4. 在底部点击“仍要打开”

如果还是被拦截，可以执行：

```bash
xattr -dr com.apple.quarantine "/Applications/字幕工作台.app"
```

然后再次尝试打开。

### 必填 GitHub Secrets

#### macOS 签名与公证

- `APPLE_CERTIFICATE`
  - Base64 编码后的 `Developer ID Application` 证书 `.p12`
- `APPLE_CERTIFICATE_PASSWORD`
  - 导出 `.p12` 时设置的密码
- `APPLE_ID`
  - Apple Developer 登录邮箱
- `APPLE_PASSWORD`
  - Apple 专用 app-specific password
- `APPLE_TEAM_ID`
  - Apple Developer Team ID
- `KEYCHAIN_PASSWORD`
  - CI 临时 keychain 密码，可自定义一个强密码

#### Apple 侧需要提前准备的内容

- Apple Developer 会员账号
- `Developer ID Application` 证书
- 导出的 `.p12` 文件和导出密码
- Apple 账号的 `app-specific password`
- 你的 `Apple Team ID`

#### Apple 准备步骤

1. 登录 Apple Developer
2. 创建或下载 `Developer ID Application` 证书
3. 在“钥匙串访问”中导出为 `.p12`
4. 为导出的 `.p12` 设置密码
5. 在 Apple 账户安全页生成 `app-specific password`
6. 记录你的 `Team ID`
7. 把 `.p12` 转成 Base64 后填入 GitHub Secrets

#### Windows 签名

- `WINDOWS_CERTIFICATE`
  - Base64 编码后的代码签名证书 `.pfx`
- `WINDOWS_CERTIFICATE_PASSWORD`
  - 导出 `.pfx` 时设置的密码

### 证书转换命令

#### macOS `.p12` 转 Base64

```bash
base64 -i developer-id-application.p12 | pbcopy
```

#### Windows `.pfx` 转 Base64

```bash
base64 -i codesign-certificate.pfx | pbcopy
```
