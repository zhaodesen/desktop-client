#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT_DIR/src-tauri/binaries"
CACHE_DIR="$ROOT_DIR/.cache"
CTRANSLATE2_DIR="${CTRANSLATE2_DIR:-$CACHE_DIR/ctranslate2-src}"
CTRANSLATE2_REF="${CTRANSLATE2_REF:-v4.7.1}"
SENTENCEPIECE_DIR="${SENTENCEPIECE_DIR:-$CACHE_DIR/sentencepiece-src}"
SENTENCEPIECE_REF="${SENTENCEPIECE_REF:-v0.2.1}"
TARGET_TRIPLE="${TARGET_TRIPLE:-}"

mkdir -p "$BIN_DIR" "$CACHE_DIR"

detect_target_triple() {
  if rustc --print host-tuple >/dev/null 2>&1; then
    rustc --print host-tuple
    return
  fi

  rustc -vV | awk '/^host:/ { print $2 }'
}

if [[ -z "$TARGET_TRIPLE" ]]; then
  TARGET_TRIPLE="$(detect_target_triple)"
fi

target_suffix() {
  local name="$1"
  if [[ "$TARGET_TRIPLE" == *windows* ]]; then
    printf "%s-%s.exe" "$name" "$TARGET_TRIPLE"
  else
    printf "%s-%s" "$name" "$TARGET_TRIPLE"
  fi
}

TRANSLATOR_CLI_TARGET="$BIN_DIR/$(target_suffix translator-cli)"
CT2_TRANSLATOR_TARGET="$BIN_DIR/$(target_suffix ct2-translator)"
SPM_ENCODE_TARGET="$BIN_DIR/$(target_suffix spm_encode)"
SPM_DECODE_TARGET="$BIN_DIR/$(target_suffix spm_decode)"

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required command: $1" >&2
    exit 1
  }
}

ensure_repo() {
  local dir="$1"
  local url="$2"
  local ref="$3"

  if [[ -d "$dir/.git" ]]; then
    echo "Updating $(basename "$dir") to $ref"
    git -C "$dir" fetch --depth 1 origin "$ref"
    git -C "$dir" checkout -f FETCH_HEAD
    git -C "$dir" submodule update --init --recursive
    return
  fi

  echo "Cloning $(basename "$dir") at $ref"
  rm -rf "$dir"
  git clone --depth 1 --branch "$ref" "$url" "$dir"
  git -C "$dir" submodule update --init --recursive
}

resolve_built_binary() {
  local build_dir="$1"
  local relative="$2"
  local windows_relative="$3"

  if [[ -f "$build_dir/$relative" ]]; then
    printf "%s" "$build_dir/$relative"
    return
  fi
  if [[ -f "$build_dir/$windows_relative" ]]; then
    printf "%s" "$build_dir/$windows_relative"
    return
  fi

  echo "Cannot locate built binary in $build_dir" >&2
  exit 1
}

build_ct2_translator() {
  require_command cmake
  require_command git

  ensure_repo "$CTRANSLATE2_DIR" "https://github.com/OpenNMT/CTranslate2.git" "$CTRANSLATE2_REF"

  local build_dir="$CACHE_DIR/ctranslate2-build-$TARGET_TRIPLE"
  local cmake_args=(
    -DCMAKE_BUILD_TYPE=Release
    -DBUILD_SHARED_LIBS=OFF
    -DBUILD_CLI=ON
    -DWITH_MKL=OFF
    -DWITH_DNNL=OFF
    -DWITH_OPENBLAS=OFF
    -DWITH_RUY=OFF
    -DWITH_CUDA=OFF
    -DWITH_CUDNN=OFF
    -DOPENMP_RUNTIME=NONE
  )

  if [[ "$TARGET_TRIPLE" == *apple-darwin ]]; then
    cmake_args+=(-DWITH_ACCELERATE=ON)
  else
    cmake_args+=(-DWITH_ACCELERATE=OFF)
  fi

  echo "Building ct2-translator for $TARGET_TRIPLE"
  cmake -S "$CTRANSLATE2_DIR" -B "$build_dir" "${cmake_args[@]}"
  cmake --build "$build_dir" --config Release -j --target translator

  local built
  built="$(resolve_built_binary "$build_dir" "cli/ct2-translator" "cli/Release/ct2-translator.exe")"
  cp "$built" "$CT2_TRANSLATOR_TARGET"
  chmod +x "$CT2_TRANSLATOR_TARGET" || true
  echo "Prepared ct2-translator: $CT2_TRANSLATOR_TARGET"
}

build_sentencepiece_tools() {
  require_command cmake
  require_command git

  ensure_repo "$SENTENCEPIECE_DIR" "https://github.com/google/sentencepiece.git" "$SENTENCEPIECE_REF"

  local build_dir="$CACHE_DIR/sentencepiece-build-$TARGET_TRIPLE"
  local cmake_args=(
    -DCMAKE_BUILD_TYPE=Release
    -DSPM_ENABLE_SHARED=OFF
    -DSPM_ENABLE_TCMALLOC=OFF
    -DSPM_BUILD_TEST=OFF
  )

  echo "Building SentencePiece tools for $TARGET_TRIPLE"
  cmake -S "$SENTENCEPIECE_DIR" -B "$build_dir" "${cmake_args[@]}"
  cmake --build "$build_dir" --config Release -j --target spm_encode spm_decode

  local built_encode built_decode
  built_encode="$(resolve_built_binary "$build_dir" "src/spm_encode" "src/Release/spm_encode.exe")"
  built_decode="$(resolve_built_binary "$build_dir" "src/spm_decode" "src/Release/spm_decode.exe")"

  cp "$built_encode" "$SPM_ENCODE_TARGET"
  cp "$built_decode" "$SPM_DECODE_TARGET"
  chmod +x "$SPM_ENCODE_TARGET" "$SPM_DECODE_TARGET" || true
  echo "Prepared spm_encode: $SPM_ENCODE_TARGET"
  echo "Prepared spm_decode: $SPM_DECODE_TARGET"
}

build_translator_cli() {
  require_command cargo

  echo "Building translator-cli for $TARGET_TRIPLE"
  cargo build \
    --manifest-path "$ROOT_DIR/tools/translator-cli/Cargo.toml" \
    --release \
    --target "$TARGET_TRIPLE"

  local built
  if [[ "$TARGET_TRIPLE" == *windows* ]]; then
    built="$ROOT_DIR/tools/translator-cli/target/$TARGET_TRIPLE/release/translator-cli.exe"
  else
    built="$ROOT_DIR/tools/translator-cli/target/$TARGET_TRIPLE/release/translator-cli"
  fi

  if [[ ! -f "$built" ]]; then
    echo "Cannot find built translator-cli: $built" >&2
    exit 1
  fi

  cp "$built" "$TRANSLATOR_CLI_TARGET"
  chmod +x "$TRANSLATOR_CLI_TARGET" || true
  echo "Prepared translator-cli: $TRANSLATOR_CLI_TARGET"
}

print_summary() {
  cat <<EOF

Native translation sidecars prepared successfully.

Artifacts:
  $TRANSLATOR_CLI_TARGET
  $CT2_TRANSLATOR_TARGET
  $SPM_ENCODE_TARGET
  $SPM_DECODE_TARGET
EOF
}

build_ct2_translator
build_sentencepiece_tools
build_translator_cli
print_summary
