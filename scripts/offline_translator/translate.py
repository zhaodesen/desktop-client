from __future__ import annotations

import json
import os
import sys
from pathlib import Path

import argostranslate.package
import argostranslate.translate


def load_request() -> dict:
    raw = sys.stdin.read()
    if not raw.strip():
        raise RuntimeError("翻译请求为空")
    return json.loads(raw)


def normalize_language_code(code: str | None) -> str:
    if not code:
        return "en"
    lowered = code.lower()
    if lowered.startswith("zh"):
        return "zh"
    return lowered


def detect_source_language(lines: list[str]) -> str:
    sample = "\n".join(line for line in lines if line.strip()).strip()
    if not sample:
        return "en"
    if any("\u4e00" <= char <= "\u9fff" for char in sample):
        return "zh"
    return "en"


def install_local_models(model_dirs: list[Path]) -> None:
    for model_dir in model_dirs:
        if not model_dir.exists():
            continue
        for model_path in sorted(model_dir.rglob("*.argosmodel")):
            try:
                argostranslate.package.install_from_path(model_path)
            except Exception:
                continue


def has_translation(source_code: str, target_code: str) -> bool:
    try:
        return argostranslate.translate.get_translation_from_codes(source_code, target_code) is not None
    except Exception:
        return False


def ensure_translation(source_code: str, target_code: str, model_dirs: list[Path]) -> None:
    if has_translation(source_code, target_code):
        return

    install_local_models(model_dirs)
    if has_translation(source_code, target_code):
        return

    success = argostranslate.package.install_package_for_language_pair(source_code, target_code)
    if not success or not has_translation(source_code, target_code):
        raise RuntimeError(f"未找到 {source_code} -> {target_code} 的离线翻译模型")


def translate_lines(lines: list[str], source_code: str, target_code: str) -> list[str]:
    if source_code == target_code:
        return lines

    translation = argostranslate.translate.get_translation_from_codes(source_code, target_code)
    if translation is None:
        raise RuntimeError(f"离线翻译模型未就绪: {source_code} -> {target_code}")

    translated: list[str] = []
    for line in lines:
        if not line.strip():
            translated.append("")
            continue
        translated.append(translation.translate(line))
    return translated


def main() -> int:
    try:
        request = load_request()
        lines = [str(item) for item in request.get("lines", [])]
        target_language = normalize_language_code(str(request.get("targetLanguage", "zh")))
        model_dirs = [
            Path(path)
            for path in os.environ.get("OFFLINE_TRANSLATOR_MODEL_DIRS", "").split(os.pathsep)
            if path.strip()
        ]

        source_language = detect_source_language(lines)
        if source_language == target_language:
            translations = lines
        else:
            ensure_translation(source_language, target_language, model_dirs)
            translations = translate_lines(lines, source_language, target_language)

        sys.stdout.write(
            json.dumps(
                {
                    "sourceLanguage": source_language,
                    "translations": translations,
                },
                ensure_ascii=False,
            )
        )
        return 0
    except Exception as exc:
        sys.stderr.write(str(exc))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
