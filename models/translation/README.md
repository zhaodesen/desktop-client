# 原生离线翻译模型目录

当前原生翻译 sidecar 默认查找：

```text
models/translation/m2m100_418m/
  model.bin
  config.json
  sentencepiece.bpe.model
  shared_vocabulary.json
  source_vocabulary.json
  target_vocabulary.json
  vocab.json
```

说明：

- `model.bin`、`config.json`、`shared_vocabulary.json` 来自转换后的 CTranslate2 模型目录
- `source_vocabulary.json`、`target_vocabulary.json` 可由 `shared_vocabulary.json` 在本地自动生成
- `sentencepiece.bpe.model`、`vocab.json` 来自原始 Hugging Face tokenizer 资源
- 运行时目标语言固定为中文 `zh`
- 源语言支持 `M2M100 418M` 模型内置的全部 100 种 FAIRSEQ 语言代码
- 常见区域码/别名（如 `pt-BR`、`zh-CN`、`jpn`）会在运行时归一化到模型支持的语言代码
- 如果模型目录不在默认位置，可通过环境变量 `MUYU_TRANSLATION_MODEL_DIR` 指定
