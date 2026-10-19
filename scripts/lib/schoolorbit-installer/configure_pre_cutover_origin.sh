#!/usr/bin/env bash
set -euo pipefail

target_ip=${1:?Target IPv4 address is required}
base_domain=${2:?Base domain is required}

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
printf '%s %s %s\n' \
    "$target_ip" \
    "admin-api.$base_domain" \
    "school-api.$base_domain" |
    sudo tee -a /etc/hosts >/dev/null
