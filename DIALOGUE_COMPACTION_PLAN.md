# Muyu 对白精炼与长音频转写改进方案

> 状态：设计草案  
> 基线版本：Muyu `0.1.12`  
> 目标平台：第一阶段仅支持 Apple Silicon macOS  
> 建议开发分支：`feat/dialogue-compaction`

## 1. 结论

本功能直接在 Muyu 仓库内二次开发，不新建项目。

现有项目已经具备 Tauri 2、Svelte 5、Rust、FFmpeg、whisper.cpp、模型下载、本地转写、字幕编辑、音频播放、任务进度、取消任务和 Sidecar 打包。新增功能应作为独立的“对白精炼”工作区接入，复用这些基础设施，同时避免继续扩大现有 `asr.rs` 和导入流程的职责。

第一版保持现有技术栈：

```text
UI                 Svelte 5 + TypeScript + Vite
桌面与任务调度      Tauri 2 + Rust
媒体分析            ffprobe
音频处理            FFmpeg
人声检测            whisper.cpp + Silero VAD v6.2
语音转写            whisper.cpp + Whisper large-v3-turbo
结果存储            JSON + SRT + TXT + M4A
```

第一版不引入 Python、PyInstaller、ONNX Runtime 或新的本地 HTTP 服务。

## 2. 产品目标

输入一段最长数小时的电影或音频，自动识别人类对白，把较长的无人声区间压缩为短停顿，同时完成离线转写。

最终输出：

```text
MovieName/
├── movie.compact.m4a
├── movie.compact.srt
├── movie.transcript.txt
└── movie.timeline.json
```

用户可以在 Muyu 中试听、检查字幕和单句循环，也可以把 `movie.compact.m4a` 上传到 AmericanSounds。

## 3. 非目标

第一版不做：

- 说话人区分或角色命名。
- 云端转写。
- 在 iPhone 上执行两小时音频推理。
- 自动识别电影版权或判断素材能否分发。
- 降噪、分离人声与歌声、声纹识别。
- 完整的多轨非线性音频编辑器。
- Windows 和 Intel macOS 的首发支持。

## 4. 现状与主要缺口

### 4.1 可直接复用

- `src-tauri/src/asr.rs`：转码、whisper-cli 调用、进度解析、取消任务。
- `src-tauri/src/model.rs`：Whisper 模型目录、下载、暂停、恢复和删除。
- `src-tauri/src/sidecar.rs`：外部二进制定位和进程启动。
- `src-tauri/src/subtitle.rs`：SRT/JSON 字幕生成。
- `src/lib/SubtitleEditor.svelte`：字幕检查和编辑。
- `src/lib/PlayerPage.svelte`：播放、字幕同步和单句循环。
- `scripts/prepare-sidecars-ci.sh` 等脚本：跨平台 Sidecar 准备流程。

### 4.2 必须改进

1. 当前视频导入会直接生成 `16kHz mono PCM WAV`。它适合 ASR，但不适合作为最终听力音频，且会丢失原始声道信息。
2. 当前没有 `ffprobe` Sidecar，无法可靠列出电影中的音轨、语言、编码和声道布局。
3. 当前 ASR 固定使用一个线程，两小时素材处理时间会过长。
4. 当前 ASR 输入只有 `audioPath`，没有模型、语言、音轨、VAD 和输出参数。
5. 当前只生成字幕，没有紧凑音频和原始时间轴映射。
6. 当前模型管理只覆盖 Whisper 模型，没有 Silero VAD 模型。
7. 现有“导入素材”会复制或提取媒体；多 GB 电影不应先复制进应用数据目录。

## 5. 目标工作流

```text
选择本地电影或音频
  ↓
ffprobe 读取音轨、声道布局和时长
  ↓
用户确认主音轨与声道策略
  ↓
FFmpeg 生成 16kHz mono 分析 WAV
  ↓
Silero VAD 输出纯语音区间
  ↓
区间扩边、合并和停顿压缩
  ↓
FFmpeg 从原始高质量音轨渲染 compact 音频
  ↓
Whisper 转写 compact 分析音频
  ↓
生成 SRT、TXT 和新旧时间轴 JSON
  ↓
试听检查，选择导入 Muyu 或打开输出目录
```

### 5.1 不复制原始电影

“对白精炼”任务直接引用用户选择的原始文件路径。处理期间不把电影复制进 Muyu 的媒体目录，只在缓存目录保存分析 WAV、波形 peaks 和临时渲染文件。

任务完成后，只把紧凑音频和字幕加入素材库。原始文件不会被修改或删除。

### 5.2 两路音频

必须区分：

- **分析流**：`16kHz mono PCM WAV`，只供 VAD 和 Whisper 使用。
- **渲染流**：从原始主音轨直接裁剪，最终编码为 `AAC mono 64kbps` 或用户选择的质量。

不允许先把电影永久转换成低质量 WAV，再用该 WAV 导出最终成品。

### 5.3 声道策略

默认规则：

1. 5.1/7.1 且存在 `FC`：使用 `pan=mono|c0=FC`。
2. 没有 `FC`：使用标准单声道 downmix。
3. 无法识别声道布局：提示用户试听，不静默假设中置声道存在。

音轨选择不能只看响度。界面应显示音轨序号、语言、标题、编码、声道数和声道布局，由用户确认主音轨。

## 6. VAD 与转写方案

### 6.1 VAD

仓库当前的 `whisper-cli` 已支持 `--vad`、`--vad-model` 和 Silero VAD 参数。为了获得独立、可编辑的纯 VAD 区间，优先从同一版本 whisper.cpp 构建并打包 `vad-speech-segments` Sidecar，不从字幕句子反推裁剪边界。

默认参数建议：

```text
threshold                 0.50
minSpeechDurationMs       250
minSilenceDurationMs      800
speechPadMs               250
maxSpeechDurationSeconds  30
```

参数必须通过预设暴露，不在 UI 中一次展示所有底层选项：

- 保守：尽量不漏对白，保留更多环境声。
- 均衡：默认。
- 激进：删除更多非对白内容，误删风险更高。

### 6.2 区间后处理

VAD 原始区间不能直接裁剪，需统一执行：

1. 每段前后增加 `250ms` padding。
2. 相邻区间间隔小于 `500ms` 时合并。
3. 只处理超过 `1000ms` 的无人声间隔。
4. 被压缩的间隔默认仍保留 `250ms`，避免对白完全粘连。
5. 合并重叠区间并限制在音频有效时长内。
6. 输出单调递增、互不重叠的最终区间。

### 6.3 ASR

默认模型使用 `large-v3-turbo`，保留现有 Tiny、Base、Small、Medium 选项。

新增性能模式：

```text
节能    1 个 CPU 线程
均衡    4 个 CPU 线程
最快    根据机器逻辑核心数计算上限
```

Whisper 在 compact 分析音频上运行，因此输出字幕天然使用 compact 时间轴。`timeline.json` 再负责映射回原始电影时间。

## 7. 时间轴数据契约

建议新增 Rust 结构并同步生成 TypeScript 类型：

```rust
struct CompactionSegment {
    original_start_ms: u64,
    original_end_ms: u64,
    compact_start_ms: u64,
    compact_end_ms: u64,
}
```

`movie.timeline.json` 示例：

```json
{
  "version": 1,
  "sourceFileName": "movie.mkv",
  "sourceDurationMs": 7200000,
  "compactDurationMs": 3680000,
  "audioStreamIndex": 1,
  "channelStrategy": "frontCenter",
  "vad": {
    "model": "silero-v6.2.0",
    "threshold": 0.5,
    "speechPadMs": 250,
    "minimumRemovedGapMs": 1000,
    "retainedGapMs": 250
  },
  "segments": [
    {
      "originalStartMs": 125420,
      "originalEndMs": 130810,
      "compactStartMs": 82200,
      "compactEndMs": 87590
    }
  ]
}
```

约束：

- 时间单位统一使用整数毫秒。
- 版本字段必填，后续格式升级不能破坏已有文件。
- 映射必须可从 compact 时间定位回 original 时间。
- 字幕文本继续保存在字幕文档中，不在 timeline 内复制一份。

## 8. 任务输入与事件

新增独立任务，不复用只有 `audioPath` 的 `StartAsrJobInput`：

```ts
type StartCompactionJobInput = {
  sourcePath: string;
  audioStreamIndex: number;
  channelStrategy: "frontCenter" | "downmix";
  language: "en" | "auto";
  whisperModelId: string;
  performanceMode: "eco" | "balanced" | "fast";
  vadPreset: "conservative" | "balanced" | "aggressive";
  outputDirectory: string;
};
```

事件阶段：

```text
compaction://started
compaction://progress
  probing
  preparing
  detecting
  rendering
  transcribing
  writing
compaction://completed
compaction://failed
```

所有长任务必须支持取消。取消时终止当前子进程，并删除本次任务产生的不完整临时文件，但不删除已存在的用户文件。

## 9. UI 方案

侧边栏新增“对白精炼”，不要把复杂配置塞进现有导入页。

### 9.1 选择素材

- 拖入或选择本地视频/音频。
- 显示文件名、时长、大小和媒体格式。
- 展示音轨列表并允许试听主音轨。

### 9.2 处理设置

- 音轨和声道策略。
- VAD 预设。
- Whisper 模型。
- 性能模式。
- 输出位置。
- 预计输出时长在完成 VAD 后更新。

### 9.3 处理进度

- 总进度与当前阶段。
- 已耗时。
- 当前处理时间点。
- 已检测对白时长。
- 原始时长与预计紧凑时长。
- 取消按钮。

### 9.4 结果检查

- 上方显示整段低分辨率波形。
- 保留区间高亮，压缩区间置灰。
- 点击区间试听。
- 允许禁用误判区间；修改后只重新渲染，不重新执行 ASR/VAD，除非用户明确要求。
- 下方沿用现有字幕编辑器。
- 提供“导入素材库”“打开输出目录”“重新处理”。

两小时音频不能让 WebView 一次解码完整 WAV。后端应流式生成有限数量的 min/max peaks，前端只渲染 peaks 和区间数据。

## 10. 代码组织建议

### 10.1 Rust

```text
src-tauri/src/
├── compaction/
│   ├── mod.rs          # 管线编排
│   ├── probe.rs        # ffprobe 与音轨信息
│   ├── vad.rs          # VAD Sidecar 和参数
│   ├── segments.rs     # 区间扩边、合并、映射
│   ├── render.rs       # FFmpeg filter script 与导出
│   ├── transcript.rs   # compact 音频转写
│   └── manifest.rs     # timeline.json
├── asr.rs              # 保留普通转写
├── media.rs            # 保留现有素材库职责
└── sidecar.rs          # 继续统一定位外部二进制
```

不要把 `compaction` 逻辑直接堆进 `asr.rs` 或 `media.rs`。

### 10.2 前端

```text
src/lib/compaction/
├── CompactionPage.svelte
├── SourcePicker.svelte
├── AudioTrackPicker.svelte
├── CompactionSettings.svelte
├── CompactionProgress.svelte
├── SegmentReview.svelte
└── WaveformOverview.svelte
```

任务状态集中到一个独立 store/composable，事件监听必须在组件销毁时解除。

### 10.3 Sidecar 与模型

需要新增：

- `ffprobe` Sidecar。
- 与 `whisper-cli` 同版本构建的 `vad-speech-segments` Sidecar。
- `ggml-silero-v6.2.0.bin` 模型下载和状态管理。

同步更新：

- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/default.json`
- `scripts/build-sidecars.sh`
- `scripts/prepare-sidecars-ci.sh`
- `scripts/prepare-sidecars-windows.ps1`
- `scripts/verify-sidecars.sh`
- `docs/sidecar-release.md`

## 11. FFmpeg 实现约束

- 大量区间不要全部拼进命令行参数，避免超过系统命令长度限制；生成临时 `filter_complex_script` 文件。
- 所有裁剪点使用同一份最终 VAD 区间，不能让字幕、音频和 timeline 各自重新计算。
- 最终音频只编码一次，避免逐段有损编码后再拼接。
- 输出先写入临时文件，成功后原子移动到目标路径。
- 输出文件已存在时必须询问覆盖、重命名或取消。
- 进度通过 FFmpeg `-progress pipe:2` 解析，并映射到总任务进度。

## 12. 缓存与恢复

缓存目录按任务 ID 隔离：

```text
cache/compaction/<job-id>/
├── analysis.wav
├── compact-analysis.wav
├── waveform-peaks.json
├── vad-segments.json
├── render-filter.txt
└── partial-output.m4a
```

第一版不要求进程退出后断点续跑，但必须：

- 正常完成后清理大体积 WAV。
- 取消或失败后删除不完整输出。
- 启动时可以识别并清理过期任务缓存。
- 不删除 timeline、字幕和最终音频。

## 13. 分阶段实施

### 阶段一：基础能力

- [ ] 新建 `feat/dialogue-compaction` 分支。
- [ ] 加入 `ffprobe` 和 `vad-speech-segments` Sidecar。
- [ ] 增加 Silero VAD 模型管理。
- [ ] 定义任务输入、进度事件和 timeline schema。
- [ ] 实现媒体探测与音轨选择数据。

### 阶段二：处理管线

- [ ] 生成分析 WAV。
- [ ] 获取 VAD 区间。
- [ ] 实现区间扩边、合并和停顿压缩。
- [ ] 生成高质量 compact 音频。
- [ ] 转写 compact 音频。
- [ ] 输出 SRT、TXT 和 timeline JSON。
- [ ] 支持取消与失败清理。

### 阶段三：UI

- [ ] 新增“对白精炼”页面。
- [ ] 实现音轨选择和参数预设。
- [ ] 显示分阶段进度。
- [ ] 显示长音频波形与区间。
- [ ] 实现区间试听和禁用。
- [ ] 复用字幕编辑器和播放器。

### 阶段四：验证与发布

- [ ] 用 10 分钟复杂片段调 VAD 参数。
- [ ] 用完整两小时电影验证内存、磁盘和耗时。
- [ ] 验证取消、覆盖、磁盘不足和原文件移动后的错误处理。
- [ ] 验证普通导入、普通转写和播放没有回归。
- [ ] 更新 README、Sidecar 发布说明和许可清单。

## 14. 最小验收标准

1. 可以选择两小时本地电影并正确列出音轨。
2. 5.1/7.1 主音轨可以选择中置声道，普通立体声可以安全 downmix。
3. 处理期间 UI 保持响应，进度可见，任务可取消。
4. 轻声对白首尾不被明显截断。
5. compact 音频、SRT 和 timeline 使用一致的时间轴。
6. timeline 能把任意 compact 时间映射回原电影时间。
7. 原始电影不被复制、修改或删除。
8. 完成后临时 WAV 被清理。
9. 现有普通转写、字幕播放和单句循环继续工作。

## 15. 风险与处理

| 风险 | 处理方式 |
|---|---|
| 配乐中的歌声被 VAD 当作语音 | 保留预设与区间复核，不承诺自动区分歌声 |
| 轻声、耳语或强音效下的对白漏检 | 默认使用保守 padding，并用真实电影样本调参 |
| 电影不存在 FC 或声道标签错误 | 提供 downmix 回退和试听确认 |
| 两小时 WAV 占用大量空间 | 缓存目录预检可用空间，完成后立即清理 |
| 数千区间导致 FFmpeg 命令过长 | 使用 filter script，不拼接超长命令行 |
| ASR 时间戳与切点轻微漂移 | 字幕基于 compact 音频生成，统一使用整数毫秒 |
| Sidecar 更新导致 macOS 签名失败 | 同步更新构建、校验、签名和发布脚本 |
| 模型文件增大安装包 | 模型首次使用时下载，不直接打进应用包 |

## 16. 后续扩展

第一版稳定后再评估：

- Qwen3-ASR 或其他 ASR 后端对照测试。
- 可恢复任务队列。
- 自动提取内嵌字幕作为转写参考。
- 用户手动拖动区间边界。
- 根据 timeline 从 compact 音频跳回原电影。
- Windows 与 Intel macOS 发布。

