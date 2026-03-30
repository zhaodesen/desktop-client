#!/usr/bin/env bash

set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "Usage: bash scripts/publish-gitee-release.sh <asset-file> [asset-file...]" >&2
  exit 1
fi

asset_files=()
for asset_path in "$@"; do
  if [[ ! -f "$asset_path" ]]; then
    echo "Asset file does not exist: $asset_path" >&2
    exit 1
  fi

  asset_dir="$(cd "$(dirname "$asset_path")" && pwd)"
  asset_files+=("$asset_dir/$(basename "$asset_path")")
done

: "${GITEE_ACCESS_TOKEN:?GITEE_ACCESS_TOKEN is required}"
: "${GITEE_REPOSITORY:?GITEE_REPOSITORY is required}"
: "${GITEE_TAG_NAME:?GITEE_TAG_NAME is required}"
: "${GITEE_RELEASE_NAME:?GITEE_RELEASE_NAME is required}"
: "${GITEE_RELEASE_BODY_FILE:?GITEE_RELEASE_BODY_FILE is required}"
: "${GITEE_TARGET_COMMITISH:?GITEE_TARGET_COMMITISH is required}"

if [[ ! -f "$GITEE_RELEASE_BODY_FILE" ]]; then
  echo "Release notes file does not exist: $GITEE_RELEASE_BODY_FILE" >&2
  exit 1
fi

if [[ "$GITEE_REPOSITORY" != */* ]]; then
  echo "GITEE_REPOSITORY must be in owner/repo format." >&2
  exit 1
fi

GITEE_PRERELEASE="${GITEE_PRERELEASE:-false}"
RELEASE_BODY="$(cat "$GITEE_RELEASE_BODY_FILE")"
OWNER="${GITEE_REPOSITORY%%/*}"
REPO="${GITEE_REPOSITORY#*/}"
TAG_NAME_ENCODED="$(jq -rn --arg value "$GITEE_TAG_NAME" '$value | @uri')"
API_BASE="https://gitee.com/api/v5/repos/$OWNER/$REPO"

request_api() {
  local method="$1"
  local url="$2"
  shift 2

  local response
  response="$(curl -sS -w $'\n%{http_code}' -X "$method" "$url" "$@")"
  REQUEST_STATUS="${response##*$'\n'}"
  REQUEST_BODY="${response%$'\n'*}"
}

release_lookup_url="$API_BASE/releases/tags/$TAG_NAME_ENCODED?access_token=$GITEE_ACCESS_TOKEN"
request_api GET "$release_lookup_url"

release_id=""

if [[ "$REQUEST_STATUS" == "200" ]]; then
  release_id="$(jq -r '.id // empty' <<<"$REQUEST_BODY")"
elif [[ "$REQUEST_STATUS" != "404" ]]; then
  echo "Failed to query Gitee release by tag: HTTP $REQUEST_STATUS" >&2
  echo "$REQUEST_BODY" >&2
  exit 1
fi

if [[ -n "$release_id" ]]; then
  echo "Updating Gitee release: $GITEE_TAG_NAME (#$release_id)"
  request_api PATCH "$API_BASE/releases/$release_id" \
    --form-string "access_token=$GITEE_ACCESS_TOKEN" \
    --form-string "tag_name=$GITEE_TAG_NAME" \
    --form-string "name=$GITEE_RELEASE_NAME" \
    --form-string "body=$RELEASE_BODY" \
    --form-string "prerelease=$GITEE_PRERELEASE"

  if [[ "$REQUEST_STATUS" != "200" ]]; then
    echo "Failed to update Gitee release: HTTP $REQUEST_STATUS" >&2
    echo "$REQUEST_BODY" >&2
    exit 1
  fi
else
  echo "Creating Gitee release: $GITEE_TAG_NAME"
  request_api POST "$API_BASE/releases" \
    --form-string "access_token=$GITEE_ACCESS_TOKEN" \
    --form-string "tag_name=$GITEE_TAG_NAME" \
    --form-string "name=$GITEE_RELEASE_NAME" \
    --form-string "body=$RELEASE_BODY" \
    --form-string "prerelease=$GITEE_PRERELEASE" \
    --form-string "target_commitish=$GITEE_TARGET_COMMITISH"

  if [[ "$REQUEST_STATUS" != "201" ]]; then
    echo "Failed to create Gitee release: HTTP $REQUEST_STATUS" >&2
    echo "$REQUEST_BODY" >&2
    exit 1
  fi

  release_id="$(jq -r '.id // empty' <<<"$REQUEST_BODY")"
fi

if [[ -z "$release_id" ]]; then
  echo "Cannot resolve Gitee release id." >&2
  exit 1
fi

request_api GET "$API_BASE/releases/$release_id/attach_files?access_token=$GITEE_ACCESS_TOKEN&per_page=100"
if [[ "$REQUEST_STATUS" != "200" ]]; then
  echo "Failed to list Gitee release assets: HTTP $REQUEST_STATUS" >&2
  echo "$REQUEST_BODY" >&2
  exit 1
fi

existing_assets_json="$REQUEST_BODY"

for asset_path in "${asset_files[@]}"; do
  asset_name="$(basename "$asset_path")"
  attach_file_id="$(
    jq -r --arg name "$asset_name" '.[] | select(.name == $name) | .id' <<<"$existing_assets_json" \
      | head -n 1
  )"

  if [[ -n "$attach_file_id" ]]; then
    echo "Deleting existing Gitee asset: $asset_name (#$attach_file_id)"
    request_api DELETE "$API_BASE/releases/$release_id/attach_files/$attach_file_id?access_token=$GITEE_ACCESS_TOKEN"

    if [[ "$REQUEST_STATUS" != "204" ]]; then
      echo "Failed to delete Gitee asset: HTTP $REQUEST_STATUS" >&2
      echo "$REQUEST_BODY" >&2
      exit 1
    fi
  fi

  echo "Uploading Gitee asset: $asset_name"
  request_api POST "$API_BASE/releases/$release_id/attach_files" \
    --form-string "access_token=$GITEE_ACCESS_TOKEN" \
    -F "file=@$asset_path"

  if [[ "$REQUEST_STATUS" != "201" ]]; then
    echo "Failed to upload Gitee asset: HTTP $REQUEST_STATUS" >&2
    echo "$REQUEST_BODY" >&2
    exit 1
  fi

  download_url="$(jq -r '.browser_download_url // empty' <<<"$REQUEST_BODY")"
  if [[ -n "$download_url" ]]; then
    echo "Uploaded: $download_url"
  fi
done
