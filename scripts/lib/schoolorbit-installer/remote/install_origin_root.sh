#!/usr/bin/env bash
set -euo pipefail

ORIGIN_ROOT_URL=https://developers.cloudflare.com/ssl/static/origin_ca_rsa_root.pem
ORIGIN_ROOT_SHA256=91a8a5567efa6bf941162aa806b3ba476aaddf7867640e53053b35fb225a5dae
ORIGIN_ROOT_TARGET=/opt/stack/nginx/ssl/cloudflare-origin-rsa-root.pem

destination=${1:?Cloudflare Origin CA root destination is required}
[[ $destination == "$ORIGIN_ROOT_TARGET" ]] || {
    printf 'Unexpected Cloudflare Origin CA root destination\n' >&2
    exit 64
}

file_hash() {
    sha256sum "$1" | awk '{print $1}'
}

if [[ -s $destination && $(file_hash "$destination") == "$ORIGIN_ROOT_SHA256" ]]; then
    chmod 0644 "$destination"
    exit 0
fi

temporary=$(mktemp "${destination}.XXXXXX")
trap 'rm -f "$temporary"' EXIT

curl --silent --show-error --fail --location "$ORIGIN_ROOT_URL" --output "$temporary"
[[ $(file_hash "$temporary") == "$ORIGIN_ROOT_SHA256" ]] || {
    printf 'Cloudflare Origin CA root checksum is invalid\n' >&2
    exit 78
}

chmod 0644 "$temporary"
mv "$temporary" "$destination"
trap - EXIT
