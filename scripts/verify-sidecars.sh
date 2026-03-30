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

WHISPER="$BIN_DIR/$(target_suffix whisper-cli)"
FFMPEG="$BIN_DIR/$(target_suffix ffmpeg)"
YT_DLP="$BIN_DIR/$(target_suffix yt-dlp)"

echo "Target triple: $TARGET_TRIPLE"
echo "Checking sidecars in: $BIN_DIR"

[[ -f "$WHISPER" ]] || { echo "Missing: $WHISPER" >&2; exit 1; }
[[ -f "$FFMPEG" ]] || { echo "Missing: $FFMPEG" >&2; exit 1; }
[[ -f "$YT_DLP" ]] || { echo "Missing: $YT_DLP" >&2; exit 1; }

echo "Found whisper-cli: $WHISPER"
echo "Found ffmpeg: $FFMPEG"
echo "Found yt-dlp: $YT_DLP"

if [[ "$TARGET_TRIPLE" != *windows* ]]; then
  [[ -x "$WHISPER" ]] || { echo "Not executable: $WHISPER" >&2; exit 1; }
  [[ -x "$FFMPEG" ]] || { echo "Not executable: $FFMPEG" >&2; exit 1; }
  [[ -x "$YT_DLP" ]] || { echo "Not executable: $YT_DLP" >&2; exit 1; }
fi

if [[ "$TARGET_TRIPLE" == *windows* ]]; then
  for dll in ggml-base.dll ggml-cpu.dll ggml.dll whisper.dll; do
    candidate="$BIN_DIR/$dll"
    [[ -f "$candidate" ]] || { echo "Missing: $candidate" >&2; exit 1; }
    echo "Found DLL: $candidate"
  done
fi

echo "Sidecar verification passed."
