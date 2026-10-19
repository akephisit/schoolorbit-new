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
export RELEASE_SHA=$accepted
umask 077
journal="$root/api-tls-operation.json"
resuming=false
if [[ -e $root/pending-release || -L $root/pending-release ]]; then
    # The symlink is the ownership marker. A matching SHA in an application-
    # owned regular pending file cannot authorize this TLS recovery path.
    test ! -L "$journal" && test -f "$journal"
    test "$(stat -c '%a:%u' "$journal")" = "600:$(id -u)"
    jq -e --arg accepted "$accepted" --arg mode "$mode" --arg base "$BASE_DOMAIN" '
        .schema_version == 1 and .owner == "public-api-tls" and
        .accepted_app_sha == $accepted and .mode == $mode and .base_domain == $base and
        (.operation_id | test("^[0-9a-f-]{36}$")) and
        (.staged_fingerprint | test("^[0-9a-f]{64}$")) and
        (.phase == "staged" or .phase == "maintenance" or .phase == "activated" or .phase == "verified")
    ' "$journal" >/dev/null
    operation_id=$(jq -er .operation_id "$journal")
    expected_pending="api-tls-operations/$operation_id/pending-release"
    test -L "$root/pending-release"
    test "$(readlink "$root/pending-release")" = "$expected_pending"
    test "$(cat "$root/pending-release")" = "$accepted"
    test "$(stat -Lc '%a:%u' "$root/pending-release")" = "600:$(id -u)"
    fingerprint=$(jq -er .staged_fingerprint "$journal")
    resuming=true
fi

write_phase() {
    local phase=$1 temporary
    temporary=$(mktemp "$journal.XXXXXX")
    jq --arg phase "$phase" '.phase=$phase' "$journal" >"$temporary"
    mv "$temporary" "$journal"
}

before=$(openssl x509 -in "$stack/nginx/ssl/schoolorbit-origin.pem" -outform DER | sha256sum | cut -d ' ' -f 1)
if [[ $resuming == false ]]; then
    details=$(jq -n '{cloudflare_token:env.API_TLS_CLOUDFLARE_TOKEN}' | bash "$owner" stage)
    fingerprint=$(jq -er '.fingerprint | select(test("^[0-9a-f]{64}$"))' <<<"$details")
    if [[ $fingerprint == "$before" ]]; then
        bash "$owner" status >/dev/null
        echo 'Public API certificate is current; no maintenance or reload needed.'
        exit 0
    fi
    # Issuance can take time. Recheck before reserving either proxy for TLS.
    test "$(cat "$root/accepted-release")" = "$accepted"
    test ! -e "$root/pending-release" && test ! -L "$root/pending-release"
    operation_id=$(cat /proc/sys/kernel/random/uuid)
    install -d -m 0700 "$root/api-tls-operations" "$root/api-tls-operations/$operation_id"
    temporary=$(mktemp "$journal.XXXXXX")
    jq -cn --arg accepted "$accepted" --arg mode "$mode" --arg base "$BASE_DOMAIN" \
        --arg operation "$operation_id" --arg fingerprint "$fingerprint" --arg previous "$before" \
        '{schema_version:1,owner:"public-api-tls",accepted_app_sha:$accepted,mode:$mode,base_domain:$base,
          operation_id:$operation,staged_fingerprint:$fingerprint,previous_fingerprint:$previous,phase:"staged"}' >"$temporary"
    mv "$temporary" "$journal"
    printf '%s\n' "$accepted" >"$root/api-tls-operations/$operation_id/pending-release"
    ln -s "api-tls-operations/$operation_id/pending-release" "$root/pending-release"
fi
unset API_TLS_CLOUDFLARE_TOKEN
test "$(cat "$root/accepted-release")" = "$accepted"
if [[ $mode == migrate-origin-ca ]] && ! bash "$owner" status >/dev/null 2>&1; then
    bash "$maintenance" enter-origin-ca-migration
else
    bash "$maintenance" enter
fi
write_phase maintenance
if [[ $fingerprint != "$before" ]]; then
    bash "$owner" activate "$fingerprint" >/dev/null
fi
write_phase activated
bash "$owner" status >/dev/null
# These calls use system trust exclusively, including the initial migration.
bash "$maintenance" verify
write_phase verified
bash "$maintenance" accept
test "$(cat "$root/accepted-release")" = "$accepted"
write_phase accepted
echo 'Public API TLS accepted; application release identity preserved.'
