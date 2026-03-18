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

TARGET_TRIPLE="$(detect_target_triple)"

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

echo "Target triple: $TARGET_TRIPLE"
echo "Checking sidecars in: $BIN_DIR"

[[ -f "$WHISPER" ]] || { echo "Missing: $WHISPER" >&2; exit 1; }
[[ -f "$FFMPEG" ]] || { echo "Missing: $FFMPEG" >&2; exit 1; }

echo "Found whisper-cli: $WHISPER"
echo "Found ffmpeg: $FFMPEG"

"$WHISPER" --help >/dev/null 2>&1 || {
  echo "whisper-cli exists but failed to execute: $WHISPER" >&2
  exit 1
}

"$FFMPEG" -version >/dev/null 2>&1 || {
  echo "ffmpeg exists but failed to execute: $FFMPEG" >&2
  exit 1
}

echo "Sidecar verification passed."
