#!/usr/bin/env bash
set -euo pipefail

ORIGIN_ROOT_URL=https://developers.cloudflare.com/ssl/static/origin_ca_rsa_root.pem
ORIGIN_ROOT_SHA256=91a8a5567efa6bf941162aa806b3ba476aaddf7867640e53053b35fb225a5dae

target_ip=${1:?Target IPv4 address is required}
base_domain=${2:?Base domain is required}
origin_ca_root=${3:?Origin CA root destination is required}

valid_ipv4() {
    local address=$1 octet
    local -a octets
    IFS=. read -r -a octets <<<"$address"
    ((${#octets[@]} == 4)) || return 1
    for octet in "${octets[@]}"; do
        [[ $octet =~ ^[0-9]{1,3}$ ]] || return 1
        ((10#$octet <= 255)) || return 1
    done
}

valid_ipv4 "$target_ip" || {
    printf 'Invalid target IPv4 address\n' >&2
    exit 64
}
[[ $base_domain =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]] || {
    printf 'Invalid base domain\n' >&2
    exit 64
}
[[ $origin_ca_root == /* && -d ${origin_ca_root%/*} ]] || {
    printf 'Origin CA root destination must use an existing absolute directory\n' >&2
    exit 64
}

temporary=$(mktemp "${origin_ca_root}.XXXXXX")
trap 'rm -f "$temporary"' EXIT
curl --silent --show-error --fail --location "$ORIGIN_ROOT_URL" --output "$temporary"
actual_hash=$(sha256sum "$temporary" | awk '{print $1}')
[[ $actual_hash == "$ORIGIN_ROOT_SHA256" ]] || {
    printf 'Cloudflare Origin CA root checksum is invalid\n' >&2
    exit 78
}
chmod 0644 "$temporary"
mv "$temporary" "$origin_ca_root"
trap - EXIT

printf '%s %s %s\n' \
    "$target_ip" \
    "admin-api.$base_domain" \
    "school-api.$base_domain" |
    sudo tee -a /etc/hosts >/dev/null

if [[ -n ${GITHUB_OUTPUT:-} ]]; then
    printf 'origin_ca_root=%s\n' "$origin_ca_root" >>"$GITHUB_OUTPUT"
fi
