# Floating Subtitle Player — 项目审查报告

> 审查范围：代码逻辑、项目架构、页面样式、用户体验、Git 工程化
> 项目技术栈：Tauri v2 + TypeScript (Vite) + Rust

---

## 一、Git / 工程化问题

### 1.1 `.cache/` 目录被误提交（148 MB）
`.cache/ffmpeg/` 下的 ffmpeg 二进制压缩包和解压产物已经进入 Git 历史，这会让 clone 体积暴涨。需要用 `git rm --cached -r .cache` 清理，并确认 `.gitignore` 已覆盖（已修复）。

### 1.2 Sidecar 二进制提交到仓库（60 MB）
`src-tauri/binaries/ffmpeg-aarch64-apple-darwin` 和 `whisper-cli-aarch64-apple-darwin` 是平台特定的大文件。建议使用 Git LFS 管理，或在 CI 中自动下载/构建，避免直接入库。

### 1.3 `src-tauri/gen/schemas/` 在子目录 `.gitignore` 中忽略但被根目录覆盖
`src-tauri/.gitignore` 写了 `/gen/schemas`，但 4 个 schema JSON 文件已被追踪。根目录 `.gitignore` 现已统一管理此规则（已修复）。

### 1.4 缺少 `.editorconfig`、`prettier`/`eslint` 等一致性工具
项目没有代码格式化和静态检查配置，多人协作容易出现风格不一致。

---

## 二、项目架构问题

### 2.1 `app.ts` 是 773 行的"上帝函数"
`bootstrapMainApp()` 把 UI 绑定、状态管理、事件监听、数据持久化全部塞在一个函数里。任何功能改动都需要在这个巨大函数内上下翻找。建议拆分为独立模块：

- `ui/tabs.ts` — 标签页切换
- `ui/library.ts` — 素材库渲染
- `ui/transport.ts` — 播放器控件
- `ui/settings.ts` — 设置面板
- `services/asr-listener.ts` — ASR 事件处理
- `services/model-listener.ts` — 模型下载事件处理
- `state/app-state.ts` — 集中的状态管理

### 2.2 前端没有使用任何框架或响应式状态管理
所有 UI 更新都通过 `dom.xxx.textContent = ...` 和 `innerHTML = ...` 手动完成。当数据变更时要手动调用 `renderLibrary()` → `attachListEvents()` 两步，很容易漏掉或顺序错误。建议至少引入轻量响应式方案（如 Preact、Solid、或 lit-html）。

### 2.3 `innerHTML` 拼接 + 事后绑定事件 = 性能浪费 + 维护困难
`renderLibrary()` 和 `renderPlaylist()` 用模板字符串拼 HTML，每次都要全量替换 + 重新 `querySelectorAll` 绑定事件。正确做法是使用事件委托（在父容器上监听一次），或改用声明式渲染。

### 2.4 Rust 端 `media.rs` 和 `asr.rs` 存在大量重复代码
两个文件各自实现了 `locate_ffmpeg`、`resolve_local_candidates`、`with_target_triple`、`with_exe_suffix` 等完全相同的函数。应抽取到共享模块（如 `binary.rs` 或 `sidecar.rs`）。

### 2.5 Rust 错误处理全部使用 `Result<T, String>`
没有使用 `thiserror` 或自定义 Error 枚举，所有错误都是字符串拼接。这导致：调用方无法按错误类型匹配处理、错误信息不可国际化、堆栈信息丢失。

### 2.6 前后端类型定义重复
`types.ts`（前端）和 Rust 各结构体（后端）手动维护两套相同的类型。建议使用 `ts-rs` crate 自动从 Rust 生成 TypeScript 类型定义，或用 JSON Schema 做中间层。

### 2.7 `tauri.ts` 中 Tauri command 调用传递了双份参数
```typescript
startAsrJob(input) {
  return callCommand("start_asr_job", {
    audioPath: input.audioPath,     // camelCase
    audio_path: input.audioPath,    // snake_case
  });
}
```
同时传 camelCase 和 snake_case 是为了兼容不同版本？这是 hack，说明对 Tauri v2 的 `rename_all` 机制理解不到位。Tauri v2 的 `#[tauri::command]` 默认接收 snake_case，只需传 snake_case 即可。

---

## 三、代码逻辑问题

### 3.1 ID 生成使用时间戳，存在碰撞风险
```rust
fn generate_id(prefix: &str) -> String {
    format!("{prefix}-{}", now_millis())
}
```
如果两次操作发生在同一毫秒内（例如快速连续导入），会产生相同 ID。建议使用 UUID v4 或至少加入随机后缀。

### 3.2 每次操作都完整读写 `library.json`
`import_media`、`delete_media`、`record_playback`、`update_media_subtitle` 每个函数都：
1. 读取整个 JSON 文件
2. 反序列化
3. 修改
4. 序列化
5. 写回磁盘

在素材量增大后会有性能问题，且在并发场景下（虽然目前单线程调用）没有文件锁保护，可能出现数据丢失。

### 3.3 模型下载使用 `reqwest::blocking` 且没有进度回调
`model.rs` 中使用 `response.copy_to(&mut file)` 一次性写入，对于 ~150MB 的模型文件，用户看到的进度停留在"正在下载"直到完成或失败。应使用异步流式下载并定期报告已下载字节数。

### 3.4 ASR 管道是同步阻塞 + `thread::spawn`
整个 ASR 管道（ffmpeg 转码 + whisper 识别）在 `std::thread` 中以阻塞方式运行。没有取消机制——用户关闭窗口或切换素材时，后台线程仍会跑完。应支持 `CancellationToken` 或通过 kill child process 实现取消。

### 3.5 `LoopController` 定义了但未在 UI 中暴露
`loop-controller.ts` 实现了单句循环和区间循环功能，`PlayerController` 也集成了调用，但 `app.ts` 中从未使用 `loopCurrentCue()` 或 `setRange()`。这是未完成的功能还是死代码？

### 3.6 前端事件监听器未清理（内存泄漏）
```typescript
void asrEvents.onStarted(({ jobId }) => { ... });
```
`listen()` 返回的 `unlisten` 函数被 `void` 丢弃了，如果页面生命周期中多次调用或 HMR 重载，会累积监听器。

### 3.7 视频导入转音频的采样率不一致
- `media.rs`（视频转音频保存）：`-ar 44100 -ac 2`（44.1kHz 双声道）
- `asr.rs`（识别前标准化）：`-ar 16000 -ac 1`（16kHz 单声道）

视频导入时先存成 44.1kHz，ASR 时再转成 16kHz，做了两次 ffmpeg 转码。建议视频导入时直接存 16kHz 单声道，省一次转码。

### 3.8 `playlistMode` 状态没有持久化
切换"单曲循环/顺序循环"后刷新窗口会回到默认的 `sequential`。应该和其他设置一样保存到 `AppSettings`。

---

## 四、页面样式问题

### 4.1 CSP 设置为 `null`（安全隐患）
```json
"security": { "csp": null }
```
完全关闭了内容安全策略，允许任意来源的脚本和资源加载。虽然是本地应用，但 `assetProtocol.scope: ["**"]` 配合空 CSP 可能被恶意字幕文件利用（XSS）。应配置合理的 CSP 白名单。

### 4.2 没有加载 IBM Plex Sans 字体
CSS 声明了 `font-family: "IBM Plex Sans"` 但项目中没有引入该字体（无 `@font-face`、无 CDN 链接、无 npm 包）。实际会 fallback 到 PingFang SC 或系统默认字体，`IBM Plex Sans` 声明等于无效。

### 4.3 固定侧边栏宽度 280px 在窄屏下的断点过大
`@media (max-width: 1100px)` 时侧边栏变为全宽堆叠，但 1100px 之上的窗口（如 980px 最小窗口宽度）中，280px 侧边栏会占据近 29%，主内容区偏窄。建议考虑可折叠侧边栏或减小断点。

### 4.4 `settings-grid` 三列布局在中等宽度下会挤压
设置页面用 `grid-template-columns: repeat(3, minmax(0, 1fr))` 三列等分，但设置面板内容较多时，在 980-1100px 区间会非常拥挤。响应式断点只有一档 (`max-width: 1100px` → 单列)，缺少中间态（如两列）。

### 4.5 Overlay 窗口在 macOS 以外没有透明背景
```rust
#[cfg(not(target_os = "macos"))]
let builder = builder.transparent(true);
```
这个逻辑反了——应该是 macOS 上不设 transparent（因为 macOS 需要 `NSVisualEffectView`），Windows/Linux 上设置 transparent。当前写法导致 macOS 上没有 transparent，Windows/Linux 上反而有了。

### 4.6 颜色对比度不足
多处文本使用 `rgba(255, 255, 255, 0.52)` 和 `rgba(245, 239, 227, 0.64)` 作为前景色，在深色背景上对比度约 3:1，不符合 WCAG AA 标准（最低 4.5:1）。

---

## 五、用户体验问题

### 5.1 没有确认对话框保护危险操作
"删除素材"、"清理字幕缓存"、"清理音频缓存"、"删除默认模型"、"重置应用数据"这些不可逆操作全部是点击即执行，没有二次确认。尤其"重置应用数据"会清空所有内容。

### 5.2 ASR 失败后没有重试入口
如果自动 ASR 失败（模型缺失、ffmpeg 出错），用户只能看到状态栏提示。没有"重新识别"按钮，唯一的办法是删除素材再重新导入。

### 5.3 导入后自动 ASR 无法禁用
每次导入媒体都会自动触发 ASR。如果用户只是想播放音频、手动导入字幕文件，没有办法跳过。应提供"仅导入不识别"的选项。

### 5.4 不支持手动导入/编辑字幕文件
用户无法为已有素材手动指定 SRT/VTT 文件，只能依赖自动识别。对于已有字幕的场景（如从其他工具导出）非常不方便。

### 5.5 模型下载没有进度百分比
下载 ~150MB 的 whisper 模型时，状态栏只显示"正在下载默认模型，这一步可能需要一点时间"，没有已下载量/总量、预计剩余时间等信息。用户无法判断是卡住了还是在正常下载。

### 5.6 播放列表"顺序循环"的行为不直观
顺序循环是按 `playbackHistory`（播放历史）的顺序，而不是按素材库的导入顺序。用户以为在"按列表顺序播放"，但实际顺序取决于之前播放过什么，这个心智模型不一致。

### 5.7 字幕解析器只支持 SRT/VTT，但没有明确提示
`subtitle-parser.ts` 可以解析 SRT 和 VTT 格式，但如果未来支持手动导入字幕，需要明确告知用户支持的格式。Whisper 输出的是 SRT，如果用户尝试加载 ASS/SSA 格式会静默失败。

### 5.8 overlayer 窗口没有关闭/最小化按钮
悬浮窗 `decorations: false` 且没有自定义关闭按钮，用户只能回到主窗口取消勾选"开启悬浮窗"来关闭它。应在悬浮窗上提供一个小型关闭/隐藏按钮。

### 5.9 没有键盘快捷键支持
作为字幕工作台，缺少播放/暂停（空格）、前进/后退（←/→）、单句循环（L）等常用快捷键，严重影响使用效率。

### 5.10 应用标题中英混杂
窗口标题是 "Floating Subtitle Player"，侧边栏品牌是"字幕工作台"，品牌 kicker 是 "Desktop Subtitle Studio"。建议统一使用中文或英文。

---

## 六、改进优先级建议

| 优先级 | 问题 | 影响 |
|--------|------|------|
| **P0** | CSP 为 null | 安全 |
| **P0** | 危险操作无确认 | 数据安全 |
| **P0** | 大文件入 Git 历史 | 工程化 |
| **P1** | app.ts 巨型函数 | 可维护性 |
| **P1** | Rust 重复代码 | 可维护性 |
| **P1** | ID 碰撞风险 | 数据完整性 |
| **P1** | 没有键盘快捷键 | 用户体验 |
| **P1** | 模型下载无进度 | 用户体验 |
| **P2** | 双份参数 hack | 代码质量 |
| **P2** | 不支持手动导入字幕 | 功能缺失 |
| **P2** | overlay 透明逻辑反了 | 跨平台兼容 |
| **P2** | 字体未加载 | 样式 |
| **P2** | 无响应式中间态 | 样式 |
| **P3** | playlistMode 未持久化 | 体验细节 |
| **P3** | LoopController 死代码 | 代码卫生 |
| **P3** | 事件监听器泄漏 | 内存 |
| **P3** | 应用名称中英混杂 | 品牌一致性 |
