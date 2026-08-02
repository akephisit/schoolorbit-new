#!/usr/bin/env bash
set -euo pipefail

template=${1:?template is required}
output=${2:?output is required}
base_domain=${3:?base domain is required}

if [[ ! $base_domain =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]]; then
    printf 'Invalid base domain\n' >&2
    exit 64
fi

BASE_DOMAIN=$base_domain
BASE_DOMAIN_REGEX=${base_domain//./\\.}
export BASE_DOMAIN BASE_DOMAIN_REGEX

temporary=$(mktemp "${output}.XXXXXX")
trap 'rm -f "$temporary"' EXIT
# Keep the substitution allowlist literal so envsubst, not the shell, expands it.
# shellcheck disable=SC2016
envsubst '${BASE_DOMAIN} ${BASE_DOMAIN_REGEX}' <"$template" >"$temporary"

if grep -Eq '\$\{BASE_DOMAIN(_REGEX)?\}' "$temporary"; then
    printf 'Unresolved proxy template variable\n' >&2
    exit 65
fi

chmod 0644 "$temporary"
mv "$temporary" "$output"
trap - EXIT
