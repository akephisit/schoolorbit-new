#!/usr/bin/env bash
set -euo pipefail

template=${1:?template is required}
output=${2:?output is required}
base_domain=${3:?base domain is required}
release_id=${4-}
deployment_status=${5-}
release_placeholder="\${RELEASE_ID}"
status_placeholder="\${DEPLOYMENT_STATUS}"

if [[ ! $base_domain =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]]; then
    printf 'Invalid base domain\n' >&2
    exit 64
fi

if grep -qF "$release_placeholder" "$template"; then
    if [[ ! $release_id =~ ^[0-9a-f]{40}$ ]]; then
        printf 'Invalid release ID\n' >&2
        exit 64
    fi
fi

if grep -qF "$status_placeholder" "$template"; then
    if [[ $deployment_status != maintenance && $deployment_status != ready ]]; then
        printf 'Invalid deployment status\n' >&2
        exit 64
    fi
fi

BASE_DOMAIN=$base_domain
BASE_DOMAIN_REGEX=${base_domain//./\\.}
RELEASE_ID=$release_id
DEPLOYMENT_STATUS=$deployment_status
export BASE_DOMAIN BASE_DOMAIN_REGEX RELEASE_ID DEPLOYMENT_STATUS

temporary=$(mktemp "${output}.XXXXXX")
trap 'rm -f "$temporary"' EXIT
# Keep the substitution allowlist literal so envsubst, not the shell, expands it.
# shellcheck disable=SC2016
envsubst '${BASE_DOMAIN} ${BASE_DOMAIN_REGEX} ${RELEASE_ID} ${DEPLOYMENT_STATUS}' <"$template" >"$temporary"

if grep -Eq '\$\{(BASE_DOMAIN(_REGEX)?|RELEASE_ID|DEPLOYMENT_STATUS)\}' "$temporary"; then
    printf 'Unresolved proxy template variable\n' >&2
    exit 65
fi

chmod 0644 "$temporary"
mv "$temporary" "$output"
trap - EXIT
