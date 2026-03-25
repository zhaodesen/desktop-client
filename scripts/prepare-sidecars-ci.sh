#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT_DIR/src-tauri/binaries"
CACHE_DIR="$ROOT_DIR/.cache"
WHISPER_CPP_DIR="${WHISPER_CPP_DIR:-$CACHE_DIR/whisper.cpp}"
WHISPER_CPP_REF="${WHISPER_CPP_REF:-master}"
TARGET_TRIPLE="${1:-}"

if [[ -z "$TARGET_TRIPLE" ]]; then
  echo "Usage: scripts/prepare-sidecars-ci.sh <target-triple>" >&2
  exit 1
fi

mkdir -p "$BIN_DIR" "$CACHE_DIR"

target_suffix() {
  local name="$1"
  if [[ "$TARGET_TRIPLE" == *windows* ]]; then
    printf "%s-%s.exe" "$name" "$TARGET_TRIPLE"
  else
    printf "%s-%s" "$name" "$TARGET_TRIPLE"
  fi
}

FFMPEG_TARGET_PATH="$BIN_DIR/$(target_suffix ffmpeg)"
WHISPER_TARGET_PATH="$BIN_DIR/$(target_suffix whisper-cli)"

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required command: $1" >&2
    exit 1
  }
}

ensure_whisper_source() {
  if [[ -d "$WHISPER_CPP_DIR/.git" ]]; then
    echo "Using cached whisper.cpp repo: $WHISPER_CPP_DIR"
    git -C "$WHISPER_CPP_DIR" fetch --depth 1 origin "$WHISPER_CPP_REF"
    git -C "$WHISPER_CPP_DIR" checkout -f FETCH_HEAD
    return
  fi

  echo "Cloning whisper.cpp into $WHISPER_CPP_DIR"
  rm -rf "$WHISPER_CPP_DIR"
  git clone --depth 1 --branch "$WHISPER_CPP_REF" https://github.com/ggml-org/whisper.cpp.git "$WHISPER_CPP_DIR"
}

install_ffmpeg() {
  case "$TARGET_TRIPLE" in
    aarch64-apple-darwin|x86_64-apple-darwin)
      require_command brew
      brew install ffmpeg
      local ffmpeg_bin
      ffmpeg_bin="$(command -v ffmpeg)"
      cp "$ffmpeg_bin" "$FFMPEG_TARGET_PATH"
      ;;
    x86_64-pc-windows-msvc)
      local choco_bin=""
      if [[ -x "/c/ProgramData/chocolatey/bin/choco.exe" ]]; then
        choco_bin="/c/ProgramData/chocolatey/bin/choco.exe"
      elif command -v choco.exe >/dev/null 2>&1; then
        choco_bin="$(command -v choco.exe)"
      elif command -v choco >/dev/null 2>&1; then
        choco_bin="$(command -v choco)"
      fi

      if [[ -z "$choco_bin" ]]; then
        echo "Cannot find Chocolatey on Windows runner." >&2
        exit 1
      fi

      if ! command -v ffmpeg >/dev/null 2>&1; then
        "$choco_bin" install ffmpeg --yes
        export PATH="/c/ProgramData/chocolatey/bin:/c/tools/ffmpeg/bin:$PATH"
      fi

      local ffmpeg_bin=""
      if command -v ffmpeg >/dev/null 2>&1; then
        ffmpeg_bin="$(command -v ffmpeg)"
      elif [[ -x "/c/tools/ffmpeg/bin/ffmpeg.exe" ]]; then
        ffmpeg_bin="/c/tools/ffmpeg/bin/ffmpeg.exe"
      fi

      if [[ -z "$ffmpeg_bin" ]]; then
        echo "Cannot locate ffmpeg after Chocolatey installation." >&2
        exit 1
      fi

      cp "$ffmpeg_bin" "$FFMPEG_TARGET_PATH"
      ;;
    *)
      echo "Unsupported target for CI sidecar preparation: $TARGET_TRIPLE" >&2
      exit 1
      ;;
  esac

  chmod +x "$FFMPEG_TARGET_PATH" || true
  echo "Prepared ffmpeg sidecar: $FFMPEG_TARGET_PATH"
}

build_whisper_cli() {
  ensure_whisper_source
  require_command cmake

  pushd "$WHISPER_CPP_DIR" >/dev/null
  rm -rf build
  cmake -B build -DCMAKE_BUILD_TYPE=Release
  cmake --build build --config Release -j

  local built=""
  if [[ -f "$WHISPER_CPP_DIR/build/bin/whisper-cli" ]]; then
    built="$WHISPER_CPP_DIR/build/bin/whisper-cli"
  elif [[ -f "$WHISPER_CPP_DIR/build/bin/Release/whisper-cli.exe" ]]; then
    built="$WHISPER_CPP_DIR/build/bin/Release/whisper-cli.exe"
  else
    echo "Cannot find built whisper-cli for $TARGET_TRIPLE" >&2
    exit 1
  fi

  cp "$built" "$WHISPER_TARGET_PATH"
  chmod +x "$WHISPER_TARGET_PATH" || true
  popd >/dev/null

  echo "Prepared whisper sidecar: $WHISPER_TARGET_PATH"
}

verify_sidecars() {
  if [[ "$TARGET_TRIPLE" == *windows* ]]; then
    "$WHISPER_TARGET_PATH" --help > /dev/null
    "$FFMPEG_TARGET_PATH" -version > /dev/null
  else
    "$WHISPER_TARGET_PATH" --help >/dev/null 2>&1
    "$FFMPEG_TARGET_PATH" -version >/dev/null 2>&1
  fi
}

echo "Preparing CI sidecars for: $TARGET_TRIPLE"
install_ffmpeg
build_whisper_cli
verify_sidecars
echo "CI sidecar preparation passed."
