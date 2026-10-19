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
acceptance_window=false
cleanup() {
    local status=$?
    trap - EXIT
    if [[ $acceptance_window == true ]]; then
        if ! reserve_maintenance; then
            echo 'Failed to restore owned DNS maintenance after interrupted acceptance' >&2
        fi
        ((status != 0)) || status=69
    fi
    rm -rf "$operation_temp"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
install -d -m 0700 "$root/api-dns"
cf_dns_preflight

public_smoke() {
    SMOKE_SUBDOMAIN=sandbox SMOKE_API_URL="https://school-api.$base" \
        SMOKE_ADMIN_API_URL="https://admin-api.$base" SMOKE_TENANT_URL="https://sandbox.$base" \
        SMOKE_ORIGIN="https://sandbox.$base" SMOKE_REQUIRE_AUTH=true SMOKE_ACADEMIC_CONTEXT=true \
        SMOKE_DIRECT_BACKEND=false SMOKE_RESOLVE_IP='' SMOKE_CA_CERT='' \
        SMOKE_RELEASE_PROBE_TOKEN='' bash "$root/scripts/smoke_test.sh"
}

public_maintenance() {
    local part body status
    for part in admin school; do
        body=$(mktemp)
        status=$(curl --silent --show-error --max-time 30 --output "$body" --write-out '%{http_code}' \
            "https://$part-api.$base/ready")
        if [[ $status != 503 ]] || ! jq -e '.error == "maintenance"' "$body" >/dev/null; then
            echo 'Public API maintenance TLS check failed' >&2
            return 69
        fi
        rm -f "$body"
    done
}

mark_status() {
    local temporary
    temporary=$(mktemp "$journal.XXXXXX")
    jq --arg status "$1" '.status=$status' "$journal" >"$temporary"
    mv "$temporary" "$journal"
}

load_owned_journal() {
    request=$1
    [[ $request =~ ^[0-9]+-[0-9]+$ ]] || return 64
    journal="$root/api-dns/$request.json"
    test ! -L "$journal" && test -f "$journal"
    test "$(stat -c '%a:%u' "$journal")" = "600:$(id -u)"
    jq -e --arg accepted "$accepted" --arg base "$base" --arg target "$target" --arg request "$request" \
        --arg zone "$SO_CF_ZONE_ID" --arg account "$SO_CF_ACCOUNT_ID" '
        .schema_version == 1 and .owner == "public-api-dns" and .request_id == $request and
        .accepted_app_sha == $accepted and .base_domain == $base and .target_ip == $target and
        .zone == $zone and .account == $account
    ' "$journal" >/dev/null
    cf_restore_snapshot "$SO_CF_ZONE_ID" "$SO_CF_ACCOUNT_ID" \
        "$(jq -c .snapshot "$journal")" "$(jq -r .snapshot_etag "$journal")" "$target"
}

reserve_maintenance() {
    local owned_pending="api-dns/$request.pending-release" pending_target pending_sha pending_metadata
    # Explicit returns are required: cleanup calls this function in a conditional,
    # where Bash disables errexit for the entire function body.
    test "$(cat "$root/accepted-release")" = "$accepted" || return 78
    if [[ -e $root/pending-release || -L $root/pending-release ]]; then
        test -L "$root/pending-release" || return 78
        pending_target=$(readlink "$root/pending-release") || return 78
        test "$pending_target" = "$owned_pending" || return 78
    fi
    if [[ -e $root/$owned_pending || -L $root/$owned_pending ]]; then
        test ! -L "$root/$owned_pending" && test -f "$root/$owned_pending" || return 78
        pending_sha=$(cat "$root/$owned_pending") || return 78
        test "$pending_sha" = "$accepted" || return 78
        pending_metadata=$(stat -c '%a:%u' "$root/$owned_pending") || return 78
        test "$pending_metadata" = "600:$(id -u)" || return 78
    else
        (
            set -o noclobber
            printf '%s\n' "$accepted" >"$root/$owned_pending"
        ) || return 78
    fi
    if [[ ! -e $root/pending-release && ! -L $root/pending-release ]]; then
        ln -s "$owned_pending" "$root/pending-release" || return 78
    fi
    bash "$maintenance" enter || return
}

resume=${API_DNS_RESUME_ID:-}
[[ -z $resume || ($mode == dns-only && $resume =~ ^[0-9]+-[0-9]+$) ]] || exit 64
if [[ $mode == dns-only && -n $resume ]]; then
    load_owned_journal "$resume"
    jq -e '.status == "applied"' "$journal" >/dev/null
    cf_assert_cutover_state
    bash "$tls" status >/dev/null
    reserve_maintenance
    bash "$maintenance" verify
    cf_wait_for_target_resolution
elif [[ $mode == dns-only ]]; then
    if [[ -e $root/pending-release || -L $root/pending-release ]]; then
        die 78 'Recover the pending application release before DNS cutover'
    fi
    cf_snapshot_dns
    [[ $SO_DNS_ORIGINAL_IP == "$target" ]] || die 78 'Current API A records must already point to the verified origin'
    bash "$tls" status >/dev/null
    for part in admin school; do
        host="$part-api.$base"
        curl --fail --silent --show-error --max-time 30 --resolve "$host:443:127.0.0.1" \
            "https://$host/ready" | jq -e '.status == "ready"' >/dev/null
    done
    if jq -e 'all(.[]; .proxied == false)' <<<"$SO_DNS_SNAPSHOT" >/dev/null; then
        cf_wait_for_target_resolution
        public_smoke
        cf_assert_cutover_state
        echo 'Both DNS-only APIs passed public TLS and authenticated full-context smoke; no DNS or proxy mutation needed.'
        exit 0
    fi
    request=${API_DNS_REQUEST_ID:?Request identity is required}
    [[ $request =~ ^[0-9]+-[0-9]+$ ]] || exit 64
    journal="$root/api-dns/$request.json"
    test ! -e "$journal"
    jq -cn --arg accepted "$accepted" --arg base "$base" --arg target "$target" --arg request "$request" \
        --arg zone "$SO_CF_ZONE_ID" --arg account "$SO_CF_ACCOUNT_ID" \
        --arg etag "$SO_DNS_SNAPSHOT_ETAG" --argjson snapshot "$SO_DNS_SNAPSHOT" \
        '{schema_version:1,owner:"public-api-dns",request_id:$request,
          accepted_app_sha:$accepted,base_domain:$base,target_ip:$target,zone:$zone,account:$account,
          snapshot_etag:$etag,snapshot:$snapshot,status:"prepared"}' >"$journal"
    reserve_maintenance
    bash "$maintenance" verify
    cf_apply_dns_batch cutover
    mark_status applied
    cf_assert_cutover_state
    cf_wait_for_target_resolution
else
    load_owned_journal "${API_DNS_ROLLBACK_ID:?Rollback journal identity is required}"
    jq -e '.status == "accepted" or .status == "applied" or .status == "prepared" or .status == "rollback_applied"' "$journal" >/dev/null
    if jq -e '.status == "rollback_applied"' "$journal" >/dev/null; then
        cf_assert_snapshot_state
    else
        cf_assert_cutover_state
    fi
    reserve_maintenance
    bash "$maintenance" verify
    if ! jq -e '.status == "rollback_applied"' "$journal" >/dev/null; then
        cf_apply_dns_batch rollback
        mark_status rollback_applied
        cf_assert_snapshot_state
    fi
    cf_wait_for_snapshot_resolution
fi
public_maintenance
acceptance_window=true
bash "$maintenance" accept
# Public credentials are used only after reopening. The private release token
# remains loopback-only in the shared smoke owner, without any bypass here.
if ! public_smoke; then
    echo 'Public authenticated smoke failed; restoring owned DNS maintenance' >&2
    exit 69
fi
test "$(cat "$root/accepted-release")" = "$accepted"
if [[ $mode == dns-rollback ]]; then mark_status rolled_back; else mark_status accepted; fi
acceptance_window=false
echo "API DNS operation accepted; private rollback journal: $request"
