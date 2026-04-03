# 原生离线翻译模型目录

当前原生翻译 sidecar 默认查找：

```text
models/translation/m2m100_418m/
  ctranslate2/
    model.bin
    config.json
    ...
  sentencepiece.bpe.model
```

说明：

- `ctranslate2/` 是 `facebook/m2m100_418M` 转换后的 CTranslate2 模型目录
- `sentencepiece.bpe.model` 来自原始 Hugging Face tokenizer 资源
- 运行时只支持 `en/ja -> zh`
- 如果模型目录不在默认位置，可通过环境变量 `MUYU_TRANSLATION_MODEL_DIR` 指定
