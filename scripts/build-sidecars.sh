#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT_DIR/src-tauri/binaries"
CACHE_DIR="$ROOT_DIR/.cache"
WHISPER_CPP_DIR="${WHISPER_CPP_DIR:-$CACHE_DIR/whisper.cpp}"
WHISPER_CPP_REF="${WHISPER_CPP_REF:-master}"
FFMPEG_SOURCE="${FFMPEG_SOURCE:-}"
YT_DLP_SOURCE="${YT_DLP_SOURCE:-}"
YT_DLP_DOWNLOAD_URL="${YT_DLP_DOWNLOAD_URL:-}"

mkdir -p "$BIN_DIR"
mkdir -p "$CACHE_DIR"

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

WHISPER_TARGET_NAME="$(target_suffix whisper-cli)"
FFMPEG_TARGET_NAME="$(target_suffix ffmpeg)"
YT_DLP_TARGET_NAME="$(target_suffix yt-dlp)"
WHISPER_TARGET_PATH="$BIN_DIR/$WHISPER_TARGET_NAME"
FFMPEG_TARGET_PATH="$BIN_DIR/$FFMPEG_TARGET_NAME"
YT_DLP_TARGET_PATH="$BIN_DIR/$YT_DLP_TARGET_NAME"

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required command: $1" >&2
    exit 1
  }
}

default_yt_dlp_download_url() {
  if [[ -n "$YT_DLP_DOWNLOAD_URL" ]]; then
    printf "%s" "$YT_DLP_DOWNLOAD_URL"
    return
  fi

  case "$TARGET_TRIPLE" in
    aarch64-apple-darwin|x86_64-apple-darwin)
      printf "%s" "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
      ;;
    x86_64-pc-windows-msvc)
      printf "%s" "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
      ;;
    aarch64-pc-windows-msvc)
      printf "%s" "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_arm64.exe"
      ;;
    x86_64-unknown-linux-gnu)
      printf "%s" "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
      ;;
    aarch64-unknown-linux-gnu)
      printf "%s" "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64"
      ;;
    *)
      printf "%s" ""
      ;;
  esac
}

validate_yt_dlp_candidate() {
  local candidate="$1"

  if ! command -v file >/dev/null 2>&1; then
    return
  fi

  local description
  description="$(file -b "$candidate" || true)"

  case "$description" in
    *"script text executable"*|*"ASCII text"*|*"Unicode text"*)
      cat >&2 <<EOF
Invalid yt-dlp sidecar candidate: $candidate
Detected file type: $description

Use the official standalone yt-dlp binary instead of a brew/pip/uv wrapper script.
EOF
      exit 1
      ;;
  esac
}

echo "Target triple: $TARGET_TRIPLE"
echo "Output dir: $BIN_DIR"

ensure_whisper_source() {
  if [[ -d "$WHISPER_CPP_DIR/.git" ]]; then
    echo "Using existing whisper.cpp repo: $WHISPER_CPP_DIR"
    return
  fi

  echo "Cloning whisper.cpp into $WHISPER_CPP_DIR"
  rm -rf "$WHISPER_CPP_DIR"
  git clone --depth 1 --branch "$WHISPER_CPP_REF" https://github.com/ggml-org/whisper.cpp.git "$WHISPER_CPP_DIR"
}

build_whisper_cli() {
  ensure_whisper_source

  pushd "$WHISPER_CPP_DIR" >/dev/null

  echo "Building whisper.cpp"
  cmake -B build -DCMAKE_BUILD_TYPE=Release
  cmake --build build --config Release -j

  local built
  if [[ -f "$WHISPER_CPP_DIR/build/bin/whisper-cli" ]]; then
    built="$WHISPER_CPP_DIR/build/bin/whisper-cli"
  elif [[ -f "$WHISPER_CPP_DIR/build/bin/Release/whisper-cli.exe" ]]; then
    built="$WHISPER_CPP_DIR/build/bin/Release/whisper-cli.exe"
  else
    echo "Cannot find built whisper-cli binary." >&2
    exit 1
  fi

  cp "$built" "$WHISPER_TARGET_PATH"
  chmod +x "$WHISPER_TARGET_PATH" || true
  popd >/dev/null

  echo "Prepared whisper sidecar: $WHISPER_TARGET_PATH"
}

prepare_ffmpeg() {
  if [[ -z "$FFMPEG_SOURCE" ]]; then
    cat >&2 <<EOF
FFMPEG_SOURCE is not set.

Provide a prebuilt ffmpeg binary path, for example:
  FFMPEG_SOURCE=/absolute/path/to/ffmpeg scripts/build-sidecars.sh
EOF
    exit 1
  fi

  if [[ ! -f "$FFMPEG_SOURCE" ]]; then
    echo "FFMPEG_SOURCE does not exist: $FFMPEG_SOURCE" >&2
    exit 1
  fi

  cp "$FFMPEG_SOURCE" "$FFMPEG_TARGET_PATH"
  chmod +x "$FFMPEG_TARGET_PATH" || true
  echo "Prepared ffmpeg sidecar: $FFMPEG_TARGET_PATH"
}

prepare_yt_dlp() {
  if [[ -n "$YT_DLP_SOURCE" ]]; then
    if [[ ! -f "$YT_DLP_SOURCE" ]]; then
      echo "YT_DLP_SOURCE does not exist: $YT_DLP_SOURCE" >&2
      exit 1
    fi

    cp "$YT_DLP_SOURCE" "$YT_DLP_TARGET_PATH"
  else
    local download_url
    download_url="$(default_yt_dlp_download_url)"
    if [[ -z "$download_url" ]]; then
      cat >&2 <<EOF
Cannot determine a yt-dlp download URL for target: $TARGET_TRIPLE

Provide a prebuilt standalone yt-dlp binary path, for example:
  YT_DLP_SOURCE=/absolute/path/to/yt-dlp scripts/build-sidecars.sh
EOF
      exit 1
    fi

    require_command curl
    curl -L --fail --retry 3 -o "$YT_DLP_TARGET_PATH" "$download_url"
  fi

  chmod +x "$YT_DLP_TARGET_PATH" || true
  validate_yt_dlp_candidate "$YT_DLP_TARGET_PATH"
  echo "Prepared yt-dlp sidecar: $YT_DLP_TARGET_PATH"
}

print_summary() {
  cat <<EOF

Sidecars prepared successfully.

Expected Tauri externalBin files:
  $WHISPER_TARGET_PATH
  $FFMPEG_TARGET_PATH
  $YT_DLP_TARGET_PATH

Next steps:
  1. Verify all binaries run on the target platform.
  2. Run: npm run tauri dev
  3. Build release: npm run tauri build
EOF
}

build_whisper_cli
prepare_ffmpeg
prepare_yt_dlp
print_summary
