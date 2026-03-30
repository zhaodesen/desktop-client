#!/usr/bin/env bash

set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "Usage: bash scripts/publish-gitee-assets.sh <asset-file> [asset-file...]" >&2
  exit 1
fi

: "${GITEE_USERNAME:?GITEE_USERNAME is required}"
: "${GITEE_ACCESS_TOKEN:?GITEE_ACCESS_TOKEN is required}"
: "${GITEE_REPOSITORY:?GITEE_REPOSITORY is required}"
: "${GITEE_TAG_NAME:?GITEE_TAG_NAME is required}"

if [[ "$GITEE_REPOSITORY" != */* ]]; then
  echo "GITEE_REPOSITORY must be in owner/repo format." >&2
  exit 1
fi

GITEE_ASSETS_BRANCH="${GITEE_ASSETS_BRANCH:-release-assets}"
GITEE_ASSETS_PREFIX="${GITEE_ASSETS_PREFIX:-desktop-releases}"

remote_url="https://${GITEE_USERNAME}:${GITEE_ACCESS_TOKEN}@gitee.com/${GITEE_REPOSITORY}.git"
temp_dir="$(mktemp -d)"
repo_dir="$temp_dir/gitee-assets"
target_dir=""

cleanup() {
  rm -rf "$temp_dir"
}

trap cleanup EXIT

branch_exists="false"
if git ls-remote --exit-code --heads "$remote_url" "$GITEE_ASSETS_BRANCH" >/dev/null 2>&1; then
  branch_exists="true"
fi

if [[ "$branch_exists" == "true" ]]; then
  git clone --depth=1 --single-branch --branch "$GITEE_ASSETS_BRANCH" "$remote_url" "$repo_dir"
else
  git clone --depth=1 "$remote_url" "$repo_dir"
fi

cd "$repo_dir"

if [[ "$branch_exists" != "true" ]]; then
  git checkout --orphan "$GITEE_ASSETS_BRANCH"
  git rm -rf . >/dev/null 2>&1 || true
fi

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"

target_dir="$repo_dir/$GITEE_ASSETS_PREFIX/$GITEE_TAG_NAME"
latest_dir="$repo_dir/$GITEE_ASSETS_PREFIX/latest"

rm -rf "$target_dir" "$latest_dir"
mkdir -p "$target_dir" "$latest_dir"

for asset_path in "$@"; do
  if [[ ! -f "$asset_path" ]]; then
    echo "Asset file does not exist: $asset_path" >&2
    exit 1
  fi

  asset_name="$(basename "$asset_path")"
  cp "$asset_path" "$target_dir/$asset_name"
  cp "$asset_path" "$latest_dir/$asset_name"
done

cat > "$repo_dir/$GITEE_ASSETS_PREFIX/README.md" <<EOF
# 安装包归档

- 当前版本：\`$GITEE_TAG_NAME\`
- 版本目录：\`$GITEE_ASSETS_PREFIX/$GITEE_TAG_NAME\`
- 最新目录：\`$GITEE_ASSETS_PREFIX/latest\`

下载方式：

- 打开对应分支：\`$GITEE_ASSETS_BRANCH\`
- 进入目录：\`$GITEE_ASSETS_PREFIX/$GITEE_TAG_NAME\`
- 或直接使用 \`latest\` 目录获取最新安装包
EOF

git add "$GITEE_ASSETS_PREFIX"

if git diff --cached --quiet; then
  echo "No asset changes to commit."
  exit 0
fi

git commit -m "chore: publish assets for $GITEE_TAG_NAME"
git push origin "HEAD:$GITEE_ASSETS_BRANCH"
