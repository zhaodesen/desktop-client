#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT_DIR/src-tauri/binaries"
CACHE_DIR="$ROOT_DIR/.cache"
WHISPER_CPP_DIR="${WHISPER_CPP_DIR:-$CACHE_DIR/whisper.cpp}"
WHISPER_CPP_REF="${WHISPER_CPP_REF:-master}"
TARGET_TRIPLE="${1:-}"
FFMPEG_SOURCE="${FFMPEG_SOURCE:-${FFMPEG_BIN:-}}"
WHISPER_CLI_SOURCE="${WHISPER_CLI_SOURCE:-${WHISPER_CLI_BIN:-}}"
YT_DLP_SOURCE="${YT_DLP_SOURCE:-${YT_DLP_BIN:-}}"

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
YT_DLP_TARGET_PATH="$BIN_DIR/$(target_suffix yt-dlp)"
YT_DLP_DOWNLOAD_URL="${YT_DLP_DOWNLOAD_URL:-}"

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
  local description

  if ! command -v file >/dev/null 2>&1; then
    return
  fi

  description="$(file -b "$candidate" || true)"
  case "$description" in
    *"script text executable"*|*"ASCII text"*|*"Unicode text"*)
      echo "yt-dlp candidate is not a standalone binary: $candidate ($description)" >&2
      exit 1
      ;;
  esac
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
  if [[ -n "$FFMPEG_SOURCE" ]]; then
    if [[ ! -f "$FFMPEG_SOURCE" ]]; then
      echo "FFMPEG_SOURCE does not exist: $FFMPEG_SOURCE" >&2
      exit 1
    fi

    cp "$FFMPEG_SOURCE" "$FFMPEG_TARGET_PATH"
    chmod +x "$FFMPEG_TARGET_PATH" || true
    echo "Prepared ffmpeg sidecar: $FFMPEG_TARGET_PATH"
    return
  fi

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

install_yt_dlp() {
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
      echo "Unsupported target for yt-dlp sidecar preparation: $TARGET_TRIPLE" >&2
      exit 1
    fi

    require_command curl
    curl -L --fail --retry 3 -o "$YT_DLP_TARGET_PATH" "$download_url"
  fi

  chmod +x "$YT_DLP_TARGET_PATH" || true
  validate_yt_dlp_candidate "$YT_DLP_TARGET_PATH"
  echo "Prepared yt-dlp sidecar: $YT_DLP_TARGET_PATH"
}

build_whisper_cli() {
  if [[ -n "$WHISPER_CLI_SOURCE" ]]; then
    if [[ ! -f "$WHISPER_CLI_SOURCE" ]]; then
      echo "WHISPER_CLI_SOURCE does not exist: $WHISPER_CLI_SOURCE" >&2
      exit 1
    fi

    cp "$WHISPER_CLI_SOURCE" "$WHISPER_TARGET_PATH"
    chmod +x "$WHISPER_TARGET_PATH" || true
    echo "Prepared whisper sidecar: $WHISPER_TARGET_PATH"
    return
  fi

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
    "$YT_DLP_TARGET_PATH" --help > /dev/null
  else
    "$WHISPER_TARGET_PATH" --help >/dev/null 2>&1
    "$FFMPEG_TARGET_PATH" -version >/dev/null 2>&1
    "$YT_DLP_TARGET_PATH" --help >/dev/null 2>&1
  fi
}

echo "Preparing CI sidecars for: $TARGET_TRIPLE"
install_ffmpeg
install_yt_dlp
build_whisper_cli
verify_sidecars
echo "CI sidecar preparation passed."
