#!/usr/bin/env bash

set -euo pipefail

runtime_env="${1:?usage: upgrade_file_platform_env.sh /path/to/runtime.env}"
test -f "$runtime_env"

read_setting() {
  sed -n "s/^${1}=//p" "$runtime_env" | tail -n 1
}

has_setting() {
  grep -Eq "^${1}=.+" "$runtime_env"
}

append_setting() {
  printf '\n%s=%s\n' "$1" "$2" >>"$runtime_env"
}

if ! has_setting R2_PUBLIC_BUCKET_NAME; then
  legacy_public_bucket="$(read_setting R2_BUCKET_NAME)"
  if [ -z "$legacy_public_bucket" ]; then
    echo "Missing required File Platform public bucket configuration" >&2
    exit 1
  fi
  if ! printf '%s' "$legacy_public_bucket" |
    grep -Eq '^[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]$'; then
    echo "Legacy File Platform public bucket name is invalid" >&2
    exit 1
  fi
  append_setting R2_PUBLIC_BUCKET_NAME "$legacy_public_bucket"
fi

if ! has_setting R2_PRIVATE_BUCKET_NAME; then
  r2_account_id="$(read_setting R2_ACCOUNT_ID)"
  if ! printf '%s' "$r2_account_id" | grep -Eq '^[A-Fa-f0-9]{32}$'; then
    echo "R2 account ID is missing or invalid" >&2
    exit 1
  fi
  private_bucket_suffix="$(
    printf '%s' "$r2_account_id" | tr '[:upper:]' '[:lower:]' | cut -c 1-24
  )"
  append_setting \
    R2_PRIVATE_BUCKET_NAME \
    "schoolorbit-files-private-${private_bucket_suffix}"
fi
