# CTranslate2 离线翻译改造方案

## 1. 背景

当前项目的离线翻译链路为：

1. Tauri/Rust 发起字幕翻译请求
2. 调用 `scripts/offline_translator/translate.py`
3. Python 脚本通过 `argostranslate` 执行翻译
4. 运行时依赖 Python、`.venv`、`site-packages`、Argos 模型及其底层推理依赖

这套方案在 Apple Silicon macOS 上可以打包为自包含运行时，但在 `macOS Intel` 上已经暴露出明显问题：

- 打包和 CI 强依赖 Python 虚拟环境
- 运行时签名链路复杂
- 上游 Python wheel 生态并不稳定
- `onnxruntime` 在 `macOS x86_64` 上缺少当前版本可用 wheel，导致 Intel mac 无法稳定自包含分发

因此，建议将离线翻译运行时逐步迁移到 `CTranslate2` 路线。

## 2. 改造目标

### 2.1 产品目标

- 桌面端离线翻译可稳定随安装包分发
- 减少对 Python/uv/.venv 的依赖
- 优先支持：
  - macOS Apple Silicon
  - macOS Intel
  - Windows x64

### 2.2 工程目标

- 将翻译运行时从“Python 工程”降为“原生 sidecar + 模型文件”
- 降低 CI 和代码签名复杂度
- 让打包逻辑与现有 `ffmpeg / whisper-cli / yt-dlp` 的 sidecar 体系保持一致

## 3. 为什么考虑 CTranslate2

根据官方文档，`CTranslate2` 的定位是高性能 Transformer 推理引擎，支持 CPU/GPU、量化和多平台优化，并明确支持 `x86-64` 与 `AArch64/ARM64`。

参考资料：

- [CTranslate2 GitHub](https://github.com/OpenNMT/CTranslate2)
- [CTranslate2 Hardware Support](https://opennmt.net/CTranslate2/hardware_support.html)
- [CTranslate2 Quantization](https://opennmt.net/CTranslate2/quantization.html)
- [onnxruntime PyPI](https://pypi.org/project/onnxruntime/)

对比当前方案，`CTranslate2` 更适合桌面应用的原因是：

- 运行时更接近原生库，而不是整套 Python 生态
- 对 CPU 平台支持更清晰
- 模型可量化，包体和推理成本更可控
- 更容易接入现有 Tauri sidecar 打包、签名和资源查找逻辑

## 4. 与现方案的对比

| 维度 | 现有方案（Argos + Python） | CTranslate2 方案 |
| --- | --- | --- |
| 打包复杂度 | 高，需要 Python、`.venv`、大量动态库 | 中，主要是原生二进制和模型 |
| CI 稳定性 | 受 Python wheel 平台支持影响大 | 明显更容易控制 |
| macOS Intel 支持 | 当前不可稳定自包含 | 理论上更可行 |
| 代码签名成本 | 高，需要签大量 `.so/.dylib` | 中，二进制边界更清晰 |
| 包体控制 | 较差 | 可通过量化和模型裁剪优化 |
| 迁移成本 | 无 | 中高，需要替换运行时 |

## 5. 总体改造思路

建议分两阶段推进。

### 阶段 A：能力验证

目标：先验证 `CTranslate2` 是否能满足字幕翻译质量、性能和平台支持要求。

输出物：

- 一个最小可运行的翻译原型
- 一组可离线分发的模型文件
- 一个 CLI 或 sidecar，输入 JSON，输出 JSON

建议原型接口：

```json
{
  "sourceLanguage": "en",
  "targetLanguage": "zh",
  "lines": ["hello world", "how are you"]
}
```

输出：

```json
{
  "sourceLanguage": "en",
  "translations": ["你好，世界", "你怎么样"]
}
```

### 阶段 B：工程替换

目标：将现有 Python 离线翻译链路切换为 `CTranslate2` sidecar。

输出物：

- Rust 侧统一的翻译执行入口
- 新的打包资源和签名逻辑
- 旧 Python 路线的降级保留或清理

## 6. 推荐架构

### 6.1 运行时架构

建议新增一个独立翻译 sidecar，例如：

- `translator-cli-aarch64-apple-darwin`
- `translator-cli-x86_64-apple-darwin`
- `translator-cli-x86_64-pc-windows-msvc.exe`

Rust 侧通过标准输入/输出与该进程通信。

建议协议：

- stdin：接收 JSON 请求
- stdout：输出 JSON 响应
- stderr：只打印诊断日志

这样有几个好处：

- 与现有 `whisper-cli` 的调用模式一致
- 便于崩溃隔离
- 便于平台定向打包和签名

### 6.2 模型资源

建议将翻译模型作为独立资源打包到：

- `src-tauri/resources/models/translation/...`

或者沿用现有资源逻辑：

- `models/translation/...`

Rust 侧资源查找逻辑应同时兼容：

- 开发态目录
- Tauri 打包后的 `Resources/_up_/...`

### 6.3 语言与模型策略

建议第一期只做核心语言对：

- 英文 -> 中文
- 日文 -> 中文

不要一开始追求全语言支持。字幕产品最重要的是主流语言对先稳定。

## 7. 技术实施拆分

### 7.1 第一阶段：PoC

任务：

1. 选定模型格式与语言对
2. 写一个最小 `translator-cli`
3. 在本地验证：
   - Apple Silicon mac
   - Intel mac
   - Windows x64
4. 建立基准测试：
   - 翻译延迟
   - 内存占用
   - 包体大小
   - 字幕质量

验收标准：

- 同一段字幕在三平台都能跑通
- 输出格式稳定
- 单次翻译失败能返回明确错误

### 7.2 第二阶段：Rust 集成

任务：

1. 新增 `translator sidecar` 定位逻辑
2. 在 Rust 中新增统一入口，例如：
   - `request_offline_translation_v2`
3. 保留旧入口一段时间作为回退
4. 在 UI 层增加运行时来源标记：
   - `ctranslate2`
   - `python-legacy`

验收标准：

- UI 不需要改交互
- 现有字幕翻译按钮继续工作
- 错误信息可明确区分“模型缺失 / sidecar 缺失 / 翻译失败”

### 7.3 第三阶段：打包与签名

任务：

1. 把 `translator-cli` 纳入 `src-tauri/binaries`
2. 增加各平台资源复制逻辑
3. 在 `pre-tauri` 与 `tauri` 打包脚本中补签名
4. 在 GitHub Actions 中增加翻译 sidecar 自检

验收标准：

- CI 可构建三平台
- 包内直接执行 `translator-cli --help` 成功
- 安装包内实际翻译一段文本成功

### 7.4 第四阶段：移除旧 Python 运行时

任务：

1. 删除 `scripts/offline_translator/.venv` 打包依赖
2. 删除与 Python 相关的 `pre-tauri` 构建步骤
3. 删除离线翻译 Python 运行时签名逻辑
4. 简化 CI

验收标准：

- 构建不再依赖 `uv sync`
- `macOS Intel` 不再因为 Python wheel 问题失败
- 打包脚本和工作流明显简化

## 8. 风险与注意事项

### 8.1 模型来源与质量风险

`CTranslate2` 是推理引擎，不直接等于“可立即替代 Argos 的现成翻译质量”。  
真正的难点在于：

- 模型格式
- 语言对覆盖
- 中英文字幕场景下的质量
- 断句与标点恢复

因此 PoC 阶段必须先做质量验证，不能只看跑通。

### 8.2 包体风险

如果模型过大，安装包体积会明显上升。  
建议优先考虑：

- 量化模型
- 按语言对拆分模型
- 首次下载而非全部预置

### 8.3 CPU 性能风险

字幕翻译通常是批量短句推理。  
如果模型较重，Intel 机器上延迟可能明显高于 Apple Silicon，需要单独做性能基线。

### 8.4 回退策略

迁移期间不建议一步删掉旧方案。  
建议至少保留一个版本周期：

- Apple Silicon 默认走 `CTranslate2`
- 若失败可回退旧 Python 路线

待新路线稳定后，再彻底移除旧实现。

## 9. 推荐落地顺序

建议按下面顺序执行：

1. 完成 `CTranslate2` 翻译 PoC
2. 只接入英文 -> 中文
3. 在 Apple Silicon 上先替换现有离线翻译
4. 验证 Intel mac 是否能稳定打包与运行
5. 再扩展到 Windows
6. 最后移除 Python 运行时

## 10. 当前建议

短期建议：

- 保持现有 Python 方案，仅对 Apple Silicon 开启自包含离线翻译
- Intel mac 先禁用或降级提示

中期建议：

- 启动 `CTranslate2` PoC
- 目标是把离线翻译从“Python 工程”迁为“原生 sidecar”

长期目标：

- 统一三平台离线翻译运行时
- 降低 CI、签名、打包和维护成本

## 11. 结论

这次改造的核心价值，不只是解决 `macOS Intel` 当前构建失败，而是从根上把离线翻译运行时从脆弱的 Python 打包链路，迁移到更适合桌面应用分发的原生推理架构。

如果只考虑短期发版，继续修补现有方案可以工作。  
如果考虑长期稳定性、跨平台和维护成本，`CTranslate2` 路线更值得投入。
