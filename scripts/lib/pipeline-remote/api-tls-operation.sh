#!/usr/bin/env bash
set -euo pipefail

mode=${1:?Operation is required}
case "$mode" in renew | migrate-origin-ca) ;; *) exit 64 ;; esac
: "${SMOKE_USERNAME:?Authenticated smoke username is required}"
: "${SMOKE_PASSWORD:?Authenticated smoke password is required}"
unset PREPARED_IMAGES
stack=${SCHOOLORBIT_STACK_ROOT:-/opt/stack}
root="$stack/deployment"
owner="$root/scripts/lib/pipeline-remote/api-tls.sh"
maintenance="$root/scripts/lib/pipeline-remote/maintenance.sh"
accepted=${ACCEPTED_APP_SHA:?Trusted accepted application SHA is required}
[[ $accepted =~ ^[0-9a-f]{40}$ ]] || exit 64
test "$(cat "$root/accepted-release")" = "$accepted" || {
    echo 'Accepted runtime identity changed; TLS operation refused' >&2
    exit 78
}
test ! -e "$root/pending-release" || {
    echo 'A pending application release must be recovered before TLS maintenance' >&2
    exit 78
}
export RELEASE_SHA=$accepted
before=$(openssl x509 -in "$stack/nginx/ssl/schoolorbit-origin.pem" -outform DER | sha256sum | cut -d ' ' -f 1)
details=$(jq -n '{cloudflare_token:env.API_TLS_CLOUDFLARE_TOKEN}' | bash "$owner" stage)
unset API_TLS_CLOUDFLARE_TOKEN
fingerprint=$(jq -er '.fingerprint | select(test("^[0-9a-f]{64}$"))' <<<"$details")
if [[ $fingerprint == "$before" ]]; then
    bash "$owner" status >/dev/null
    echo 'Public API certificate is current; no maintenance or reload needed.'
    exit 0
fi
# Issuance can take time. Recheck before touching either proxy.
test "$(cat "$root/accepted-release")" = "$accepted"
test ! -e "$root/pending-release"
if [[ $mode == migrate-origin-ca ]]; then
    bash "$maintenance" enter-origin-ca-migration
else
    bash "$maintenance" enter
fi
bash "$owner" activate "$fingerprint" >/dev/null
bash "$owner" status >/dev/null
# These calls use system trust exclusively, including the initial migration.
bash "$maintenance" verify
bash "$maintenance" accept
test "$(cat "$root/accepted-release")" = "$accepted"
echo 'Public API TLS accepted; application release identity preserved.'
