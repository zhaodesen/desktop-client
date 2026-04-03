#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_TRIPLE="${1:-${TARGET_TRIPLE:-}}"

if [[ -z "$TARGET_TRIPLE" ]]; then
  echo "Usage: scripts/prepare-translation-sidecars-ci.sh <target-triple>" >&2
  exit 1
fi

TARGET_TRIPLE="$TARGET_TRIPLE" "$ROOT_DIR/scripts/build-translation-sidecars.sh"
