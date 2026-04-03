#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT_DIR/src-tauri/binaries"

detect_target_triple() {
  if rustc --print host-tuple >/dev/null 2>&1; then
    rustc --print host-tuple
    return
  fi

  rustc -vV | awk '/^host:/ { print $2 }'
}

TARGET_TRIPLE="${TARGET_TRIPLE:-$(detect_target_triple)}"

target_suffix() {
  local name="$1"
  if [[ "$TARGET_TRIPLE" == *windows* ]]; then
    printf "%s-%s.exe" "$name" "$TARGET_TRIPLE"
  else
    printf "%s-%s" "$name" "$TARGET_TRIPLE"
  fi
}

probe_command() {
  local status=0
  "$@" >/dev/null 2>&1 || status=$?
  if [[ "$status" -gt 1 ]]; then
    echo "Probe failed (${status}): $*" >&2
    exit "$status"
  fi
}

WHISPER="$BIN_DIR/$(target_suffix whisper-cli)"
FFMPEG="$BIN_DIR/$(target_suffix ffmpeg)"
YT_DLP="$BIN_DIR/$(target_suffix yt-dlp)"
TRANSLATOR_CLI="$BIN_DIR/$(target_suffix translator-cli)"
CT2_TRANSLATOR="$BIN_DIR/$(target_suffix ct2-translator)"
SPM_ENCODE="$BIN_DIR/$(target_suffix spm_encode)"
SPM_DECODE="$BIN_DIR/$(target_suffix spm_decode)"

echo "Target triple: $TARGET_TRIPLE"
echo "Checking sidecars in: $BIN_DIR"

[[ -f "$WHISPER" ]] || { echo "Missing: $WHISPER" >&2; exit 1; }
[[ -f "$FFMPEG" ]] || { echo "Missing: $FFMPEG" >&2; exit 1; }
[[ -f "$YT_DLP" ]] || { echo "Missing: $YT_DLP" >&2; exit 1; }
[[ -f "$TRANSLATOR_CLI" ]] || { echo "Missing: $TRANSLATOR_CLI" >&2; exit 1; }
[[ -f "$CT2_TRANSLATOR" ]] || { echo "Missing: $CT2_TRANSLATOR" >&2; exit 1; }
[[ -f "$SPM_ENCODE" ]] || { echo "Missing: $SPM_ENCODE" >&2; exit 1; }
[[ -f "$SPM_DECODE" ]] || { echo "Missing: $SPM_DECODE" >&2; exit 1; }

echo "Found whisper-cli: $WHISPER"
echo "Found ffmpeg: $FFMPEG"
echo "Found yt-dlp: $YT_DLP"
echo "Found translator-cli: $TRANSLATOR_CLI"
echo "Found ct2-translator: $CT2_TRANSLATOR"
echo "Found spm_encode: $SPM_ENCODE"
echo "Found spm_decode: $SPM_DECODE"

if [[ "$TARGET_TRIPLE" != *windows* ]]; then
  [[ -x "$WHISPER" ]] || { echo "Not executable: $WHISPER" >&2; exit 1; }
  [[ -x "$FFMPEG" ]] || { echo "Not executable: $FFMPEG" >&2; exit 1; }
  [[ -x "$YT_DLP" ]] || { echo "Not executable: $YT_DLP" >&2; exit 1; }
  [[ -x "$TRANSLATOR_CLI" ]] || { echo "Not executable: $TRANSLATOR_CLI" >&2; exit 1; }
  [[ -x "$CT2_TRANSLATOR" ]] || { echo "Not executable: $CT2_TRANSLATOR" >&2; exit 1; }
  [[ -x "$SPM_ENCODE" ]] || { echo "Not executable: $SPM_ENCODE" >&2; exit 1; }
  [[ -x "$SPM_DECODE" ]] || { echo "Not executable: $SPM_DECODE" >&2; exit 1; }
fi

if [[ "$TARGET_TRIPLE" == *windows* ]]; then
  for dll in ggml-base.dll ggml-cpu.dll ggml.dll whisper.dll; do
    candidate="$BIN_DIR/$dll"
    [[ -f "$candidate" ]] || { echo "Missing: $candidate" >&2; exit 1; }
    echo "Found DLL: $candidate"
  done
fi

if [[ "$TARGET_TRIPLE" == *windows* ]]; then
  probe_command "$TRANSLATOR_CLI" --help
  probe_command "$CT2_TRANSLATOR" --help
  probe_command "$SPM_ENCODE" --help
  probe_command "$SPM_DECODE" --help
else
  probe_command "$TRANSLATOR_CLI" --help
  probe_command "$CT2_TRANSLATOR" --help
  probe_command "$SPM_ENCODE" --help
  probe_command "$SPM_DECODE" --help
fi

echo "Sidecar verification passed."
