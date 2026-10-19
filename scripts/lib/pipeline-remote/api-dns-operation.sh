#!/usr/bin/env bash
set -euo pipefail

mode=${1:?DNS operation is required}
case "$mode" in dns-only | dns-rollback) ;; *) exit 64 ;; esac
: "${SMOKE_USERNAME:?Authenticated smoke username is required}"
: "${SMOKE_PASSWORD:?Authenticated smoke password is required}"
unset PREPARED_IMAGES
stack=${SCHOOLORBIT_STACK_ROOT:-/opt/stack}
root="$stack/deployment"
library="$root/scripts/lib/schoolorbit-installer"
# shellcheck source=scripts/lib/schoolorbit-installer/common.sh
source "$library/common.sh"
declare -A SO_CONFIG=() SO_SECRETS=()
# shellcheck source=scripts/lib/schoolorbit-installer/cloudflare.sh
source "$library/cloudflare.sh"
accepted=${ACCEPTED_APP_SHA:?Trusted accepted application SHA is required}
[[ $accepted =~ ^[0-9a-f]{40}$ ]] || exit 64
base=${BASE_DOMAIN:?BASE_DOMAIN is required}
target=${TARGET_ORIGIN_IP:?Target origin IPv4 is required}
[[ $base =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]] || exit 64
[[ $target =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] || exit 64
test "$(cat "$root/accepted-release")" = "$accepted"
export RELEASE_SHA=$accepted
SO_CONFIG[base_domain]=$base
SO_CONFIG[target]=$target
SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]=${API_TLS_CLOUDFLARE_TOKEN:?DNS token is required}
unset API_TLS_CLOUDFLARE_TOKEN
maintenance="$root/scripts/lib/pipeline-remote/maintenance.sh"
tls="$root/scripts/lib/pipeline-remote/api-tls.sh"
umask 077
runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
test -d "$runtime" && test -w "$runtime"
operation_temp=$(mktemp -d "$runtime/schoolorbit-api-dns.XXXXXX")
export TMPDIR=$operation_temp
trap 'rm -rf "$operation_temp"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
install -d -m 0700 "$root/api-dns"
cf_preflight

public_smoke() {
    local probe
    probe=$(cat "$root/releases/$accepted/probe-token")
    SMOKE_SUBDOMAIN=sandbox SMOKE_API_URL="https://school-api.$base" \
        SMOKE_ADMIN_API_URL="https://admin-api.$base" SMOKE_TENANT_URL="https://sandbox.$base" \
        SMOKE_ORIGIN="https://sandbox.$base" SMOKE_REQUIRE_AUTH=true SMOKE_ACADEMIC_CONTEXT=true \
        SMOKE_DIRECT_BACKEND=false SMOKE_RESOLVE_IP='' SMOKE_CA_CERT='' \
        SMOKE_RELEASE_PROBE_TOKEN="$probe" bash "$root/scripts/smoke_test.sh"
}

if [[ $mode == dns-only ]]; then
    test ! -e "$root/pending-release" || die 78 'Recover the pending application release before DNS cutover'
    cf_snapshot_dns
    [[ $SO_DNS_ORIGINAL_IP == "$target" ]] || die 78 'Current API A records must already point to the verified origin'
    # Validates exact SANs, full public chain, key match and expiry before DNS.
    bash "$tls" status >/dev/null
    for part in admin school; do
        host="$part-api.$base"
        curl --fail --silent --show-error --max-time 30 --resolve "$host:443:127.0.0.1" \
            "https://$host/ready" | jq -e '.status == "ready"' >/dev/null
    done
    if jq -e 'all(.[]; .proxied == false)' <<<"$SO_DNS_SNAPSHOT" >/dev/null; then
        echo 'Both verified API records are already DNS only.'
        exit 0
    fi
    request=${API_DNS_REQUEST_ID:?Request identity is required}
    [[ $request =~ ^[0-9]+-[0-9]+$ ]] || exit 64
    journal="$root/api-dns/$request.json"
    test ! -e "$journal"
    jq -cn --arg accepted "$accepted" --arg base "$base" --arg target "$target" \
        --arg zone "$SO_CF_ZONE_ID" --arg account "$SO_CF_ACCOUNT_ID" \
        --arg etag "$SO_DNS_SNAPSHOT_ETAG" --argjson snapshot "$SO_DNS_SNAPSHOT" \
        '{accepted_app_sha:$accepted,base_domain:$base,target_ip:$target,zone:$zone,account:$account,
          snapshot_etag:$etag,snapshot:$snapshot,status:"prepared"}' >"$journal"
    bash "$maintenance" enter
    bash "$maintenance" verify
    cf_apply_dns_batch cutover
    cf_assert_cutover_state
    temporary=$(mktemp "$journal.XXXXXX")
    jq '.status="applied"' "$journal" >"$temporary"
    mv "$temporary" "$journal"
    cf_wait_for_target_resolution
    public_smoke
else
    request=${API_DNS_ROLLBACK_ID:?Rollback journal identity is required}
    [[ $request =~ ^[0-9]+-[0-9]+$ ]] || exit 64
    journal="$root/api-dns/$request.json"
    jq -e --arg accepted "$accepted" --arg base "$base" --arg target "$target" \
        '.accepted_app_sha == $accepted and .base_domain == $base and .target_ip == $target and (.status == "applied" or .status == "prepared")' "$journal" >/dev/null
    cf_restore_snapshot "$(jq -r .zone "$journal")" "$(jq -r .account "$journal")" \
        "$(jq -c .snapshot "$journal")" "$(jq -r .snapshot_etag "$journal")" "$target"
    cf_assert_cutover_state
    if [[ -e $root/pending-release ]]; then
        test "$(cat "$root/pending-release")" = "$accepted"
    else
        bash "$maintenance" enter
    fi
    bash "$maintenance" verify
    cf_apply_dns_batch rollback
    cf_wait_for_snapshot_resolution
    public_smoke
    temporary=$(mktemp "$journal.XXXXXX")
    jq '.status="rolled_back"' "$journal" >"$temporary"
    mv "$temporary" "$journal"
fi
bash "$maintenance" accept
test "$(cat "$root/accepted-release")" = "$accepted"
echo "API DNS operation accepted; private rollback journal: $request"
