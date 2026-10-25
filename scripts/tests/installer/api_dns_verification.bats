#!/usr/bin/env bats

setup() {
    export TEST_ROOT
    TEST_ROOT=$(mktemp -d)
    export SCHOOLORBIT_STACK_ROOT="$TEST_ROOT/stack"
    export XDG_RUNTIME_DIR
    XDG_RUNTIME_DIR=$(mktemp -d /dev/shm/schoolorbit-dns-test.XXXXXX)
    export BASE_DOMAIN=schoolorbit.app TARGET_ORIGIN_IP=192.0.2.20
    export ACCEPTED_APP_SHA=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    export API_TLS_CLOUDFLARE_TOKEN=private-fixture-token
    export SMOKE_USERNAME=fixture SMOKE_PASSWORD=private-fixture-password
    export SMOKE_RELEASE_PROBE_TOKEN=must-be-cleared SMOKE_RESOLVE_IP=must-be-cleared SMOKE_CA_CERT=must-be-cleared
    export CLEANUP_RUN_ID=123 CLEANUP_RUN_ATTEMPT=1 CLEANUP_PHASE=prepare
    export SO_PROVIDER_POLL_ATTEMPTS=1 SO_PROVIDER_POLL_DELAY=0
    export FIXTURE_DIR="$BATS_TEST_DIRNAME/fixtures"
    export DNS_OPERATION="$BATS_TEST_DIRNAME/../../lib/pipeline-remote/api-dns-operation.sh"
    export VERIFIER="$BATS_TEST_DIRNAME/../../lib/pipeline-remote/api-dns-verification.sh"
    export COMMAND_LOG="$TEST_ROOT/commands" DNS_STATE="$TEST_ROOT/dns.json"
    root="$SCHOOLORBIT_STACK_ROOT/deployment"
    export PROOF="$root/owned-load-cleanup/dns-verification/cleanup-3962938a-54d0fe6f-73f5-49be-908f-4ec14aee486b/123-1-prepare.json"
    mkdir -p "$root/scripts/lib/pipeline-remote" "$root/scripts/lib/schoolorbit-installer" "$TEST_ROOT/bin"
    mkdir -m 0700 "$root/api-dns"
    cp "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/"{common,cloudflare}.sh "$root/scripts/lib/schoolorbit-installer/"
    printf '%s\n' "$ACCEPTED_APP_SHA" >"$root/accepted-release"
    jq '(.result[].content)="192.0.2.20" | (.result[].proxied)=false' "$FIXTURE_DIR/cloudflare-dns.json" >"$DNS_STATE"
    jq -n --arg accepted "$ACCEPTED_APP_SHA" --slurpfile dns "$DNS_STATE" \
        '{accepted_app_sha:$accepted,base_domain:"schoolorbit.app",target_ip:"192.0.2.20",zone:"zone-123",account:"account-456",snapshot_etag:("a"*64),snapshot:($dns[0].result | map({id,type,name,content,ttl,proxied,modified_on})),status:"applied"}' \
        >"$root/api-dns/39302306278-1.json"
    chmod 0600 "$root/api-dns/39302306278-1.json"
    export LEGACY_HASH
    LEGACY_HASH=$(sha256sum "$root/api-dns/39302306278-1.json" | cut -d ' ' -f 1)
    jq -cn --arg legacy "$LEGACY_HASH" \
        '{schemaVersion:1,operationId:"cleanup-3962938a-54d0fe6f-73f5-49be-908f-4ec14aee486b",inputSha256:("a"*64),legacyDns:{filename:"39302306278-1.json",sha256:$legacy,successfulNoopRun:39306710439,successfulNoopJob:118245556761,successfulNoopSource:("b"*40)}}' \
        >"$TEST_ROOT/catalogue.json"
    catalogue_env
    cat >"$root/scripts/lib/pipeline-remote/api-tls.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ $1 == status ]] || { echo forbidden-TLS-mutation >>"$COMMAND_LOG"; exit 99; }
printf 'tls status\n' >>"$COMMAND_LOG"
[[ ${FAIL_TLS:-false} != true ]]
SH
    cat >"$root/scripts/smoke_test.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ $SMOKE_ACADEMIC_CONTEXT == true && $SMOKE_REQUIRE_AUTH == true && $SMOKE_DIRECT_BACKEND == false ]]
[[ -z $SMOKE_RESOLVE_IP && -z $SMOKE_CA_CERT && -z $SMOKE_RELEASE_PROBE_TOKEN ]]
[[ $SMOKE_API_URL == https://school-api.schoolorbit.app && $SMOKE_ADMIN_API_URL == https://admin-api.schoolorbit.app ]]
[[ $SMOKE_ORIGIN == https://sandbox.schoolorbit.app && $SMOKE_TENANT_URL == https://sandbox.schoolorbit.app ]]
printf 'public auth full context\n' >>"$COMMAND_LOG"
printf 'private-fixture-password private-smoke-error\n' >&2
if [[ ${PENDING_AFTER_PUBLIC:-false} == true ]]; then touch "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"; fi
if [[ ${ACCEPTED_AFTER_PUBLIC:-false} == true ]]; then printf '%040d\n' 0 >"$SCHOOLORBIT_STACK_ROOT/deployment/accepted-release"; fi
if [[ ${DNS_AFTER_PUBLIC:-false} == true ]]; then
    jq '(.result[0].ttl)=300' "$DNS_STATE" >"$DNS_STATE.next"
    mv "$DNS_STATE.next" "$DNS_STATE"
fi
[[ ${FAIL_PUBLIC:-false} != true ]]
SH
    pin_dependencies
    cat >"$root/scripts/lib/pipeline-remote/maintenance.sh" <<'SH'
#!/usr/bin/env bash
printf 'forbidden-maintenance\n' >>"$COMMAND_LOG"
exit 99
SH
    cat >"$TEST_ROOT/bin/curl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
output= method=GET url=
while (($#)); do
    case "$1" in
        --output) output=$2; shift 2 ;;
        --request) method=$2; shift 2 ;;
        --header) shift 2 ;;
        http://*|https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
[[ $method == GET ]] || { printf 'forbidden-provider-mutation\n' >>"$COMMAND_LOG"; exit 99; }
case "$url" in
    */zones\?*)
        printf 'GET zones\n' >>"$COMMAND_LOG"
        if [[ ${ZONE_AFTER_PUBLIC:-false} == true ]] && grep -q 'public auth' "$COMMAND_LOG"; then
            jq '.result[0].account.id="foreign-account"' "$FIXTURE_DIR/cloudflare-zone.json" >"$output"
        else cp "$FIXTURE_DIR/cloudflare-zone.json" "$output"; fi ;;
    */dns_records\?*) printf 'GET records\n' >>"$COMMAND_LOG"; cp "$DNS_STATE" "$output" ;;
    *) printf 'forbidden-network-request\n' >>"$COMMAND_LOG"; exit 99 ;;
esac
SH
    cat >"$TEST_ROOT/bin/getent" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ $1 == ahostsv4 ]]
if [[ ${FAIL_DNS:-false} == true ]]; then printf '192.0.2.99 STREAM fixture\n'; exit; fi
printf '192.0.2.20 STREAM fixture\n'
SH
    cat >"$TEST_ROOT/bin/podman" <<'SH'
#!/usr/bin/env bash
[[ $1 == container && $2 == exists ]] || { printf 'forbidden-podman-mutation\n' >>"$COMMAND_LOG"; exit 99; }
[[ ${PRESENT_CLIENT:-} != "$3" ]] || exit 0
[[ ${UNKNOWN_CLIENT:-false} != true ]] || exit 125
exit 1
SH
    chmod +x "$TEST_ROOT/bin/"*
    export PATH="$TEST_ROOT/bin:$PATH"
}

catalogue_env() {
    export CLEANUP_CATALOGUE CLEANUP_CATALOGUE_SHA256
    CLEANUP_CATALOGUE=$(base64 -w0 "$TEST_ROOT/catalogue.json")
    CLEANUP_CATALOGUE_SHA256=$(sha256sum "$TEST_ROOT/catalogue.json" | cut -d ' ' -f 1)
}

pin_dependencies() {
    export DNS_TLS_OWNER_SHA256 DNS_SMOKE_OWNER_SHA256
    DNS_TLS_OWNER_SHA256=$(sha256sum "$root/scripts/lib/pipeline-remote/api-tls.sh" | cut -d ' ' -f 1)
    DNS_SMOKE_OWNER_SHA256=$(sha256sum "$root/scripts/smoke_test.sh" | cut -d ' ' -f 1)
}

terminal_journal() {
    jq --arg status "$1" '. + {schema_version:1,owner:"public-api-dns",request_id:"400-1",status:$status}' \
        "$root/api-dns/39302306278-1.json" >"$root/api-dns/400-1.json"
    chmod 0600 "$root/api-dns/400-1.json"
}

assert_untouched() {
    [ "$(sha256sum "$root/api-dns/39302306278-1.json" | cut -d ' ' -f 1)" = "$LEGACY_HASH" ]
    if [[ -e $COMMAND_LOG ]]; then ! grep -q forbidden "$COMMAND_LOG"; fi
    ! grep -R -q 'private-fixture-password\|private-fixture-token' "$root/owned-load-cleanup" 2>/dev/null
    [ -z "$(find "$XDG_RUNTIME_DIR" -mindepth 1 -print -quit)" ]
}

teardown() { rm -rf "$TEST_ROOT" "$XDG_RUNTIME_DIR"; }

@test "source-only library has no automatic actions" {
    run bash -c 'source "$VERIFIER"; declare -F api_dns_verify_only api_dns_public_smoke'
    [ "$status" -eq 0 ]
    [ ! -e "$COMMAND_LOG" ]
    [ ! -e "$PROOF" ]
}

@test "GET-only verification preserves historical journal and creates exact private immutable proof" {
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -eq 0 ]
    [[ "$output" != *private-* ]]
    jq -e --arg legacy "$LEGACY_HASH" --arg accepted "$ACCEPTED_APP_SHA" '
        keys == ["acceptedRelease","checks","legacyJournal","observedAt","operationId","owner","phase","runAttempt","runId","schemaVersion","stableDnsSha256","stableFacts"] and
        .schemaVersion==1 and .owner=="public-api-dns-verification" and .phase=="prepare" and .runId=="123" and .runAttempt=="1" and
        .legacyJournal=={filename:"39302306278-1.json",sha256:$legacy} and .acceptedRelease==$accepted and
        (.checks|length==6 and all(.[];.==true)) and (.stableFacts.records|length==2 and all(.[];.type=="A" and .proxied==false))
    ' "$PROOF"
    facts=$(jq -cS .stableFacts "$PROOF")
    [ "$(printf %s "$facts" | sha256sum | cut -d ' ' -f 1)" = "$(jq -r .stableDnsSha256 "$PROOF")" ]
    [ "$(stat -c '%a:%u' "$PROOF")" = "600:$(id -u)" ]
    [ "$(stat -c '%a:%u' "$(dirname "$PROOF")")" = "700:$(id -u)" ]
    proof_hash=$(sha256sum "$PROOF")
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    [ "$(sha256sum "$PROOF")" = "$proof_hash" ]
    assert_untouched
}

@test "unknown legacy and invalid catalogue or pinned journal hash refuse" {
    export CLEANUP_CATALOGUE_SHA256=$(printf '%064d' 0)
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    catalogue_env
    cp "$root/api-dns/39302306278-1.json" "$root/api-dns/999-1.json"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    rm "$root/api-dns/999-1.json"
    printf '\n' >>"$root/api-dns/39302306278-1.json"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    [ ! -e "$PROOF" ]
    [ ! -e "$COMMAND_LOG" ]
}

@test "current accepted and rolled_back journals are terminal while intermediate states block" {
    terminal_journal accepted
    bash "$DNS_OPERATION" verify-only
    rm "$PROOF"
    terminal_journal rolled_back
    bash "$DNS_OPERATION" verify-only
    rm "$PROOF"
    for state in prepared applied rollback_applied; do
        terminal_journal "$state"
        run bash "$DNS_OPERATION" verify-only
        [ "$status" -ne 0 ]
        [ ! -e "$PROOF" ]
    done
    assert_untouched
}

@test "ambiguous proxy foreign ownership and changing DNS facts refuse without writes" {
    for mutation in '(.result[0].proxied)=true' '.result += [(.result[0] | .id="extra" | .type="AAAA")]' \
        '.result += [(.result[0] | .id="duplicate")]' '(.result[0].content)="192.0.2.99"'; do
        cp "$DNS_STATE" "$DNS_STATE.original"
        jq "$mutation" "$DNS_STATE" >"$DNS_STATE.next"
        mv "$DNS_STATE.next" "$DNS_STATE"
        run bash "$DNS_OPERATION" verify-only
        [ "$status" -ne 0 ]
        [ ! -e "$PROOF" ]
        mv "$DNS_STATE.original" "$DNS_STATE"
    done
    for variable in DNS_AFTER_PUBLIC ZONE_AFTER_PUBLIC; do
        export "$variable=true"
        run bash "$DNS_OPERATION" verify-only
        [ "$status" -ne 0 ]
        [ ! -e "$PROOF" ]
        unset "$variable"
        jq '(.result[0].ttl)=1' "$DNS_STATE" >"$DNS_STATE.next"
        mv "$DNS_STATE.next" "$DNS_STATE"
    done
    assert_untouched
}

@test "TLS DNS and authenticated public failures remain private and never create proof" {
    for variable in FAIL_TLS FAIL_DNS FAIL_PUBLIC; do
        export "$variable=true"
        run bash "$DNS_OPERATION" verify-only
        [ "$status" -ne 0 ]
        [[ "$output" != *private-* ]]
        [ ! -e "$PROOF" ]
        unset "$variable"
    done
    assert_untouched
}

@test "pending accepted-release drift orphan client and unknown Podman state block before and after" {
    for variable in PENDING_AFTER_PUBLIC ACCEPTED_AFTER_PUBLIC UNKNOWN_CLIENT; do
        export "$variable=true"
        run bash "$DNS_OPERATION" verify-only
        [ "$status" -ne 0 ]
        [ ! -e "$PROOF" ]
        unset "$variable"
        rm -f "$root/pending-release"
        printf '%s\n' "$ACCEPTED_APP_SHA" >"$root/accepted-release"
    done
    for client in schoolorbit-school-rollout schoolorbit-admin-migration schoolorbit-owned-load-cleanup-3962938a; do
        export PRESENT_CLIENT=$client
        run bash "$DNS_OPERATION" verify-only
        [ "$status" -ne 0 ]
        [ ! -e "$PROOF" ]
    done
    unset PRESENT_CLIENT
    assert_untouched
}

@test "reconcile permits only its own cleanup reservation and private output refuses unsafe paths" {
    mkdir -m0700 "$root/owned-load-cleanup"
    jq -n --arg cat "$CLEANUP_CATALOGUE_SHA256" \
        '{schemaVersion:1,operationId:"cleanup-3962938a-54d0fe6f-73f5-49be-908f-4ec14aee486b",inputSha256:("a"*64),catalogueSha256:$cat,status:"reserved",run:"122-1"}' \
        >"$root/owned-load-cleanup/active"
    chmod 0600 "$root/owned-load-cleanup/active"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    export CLEANUP_PHASE=reconcile
    bash "$DNS_OPERATION" verify-only
    reconcile="${PROOF%-prepare.json}-reconcile.json"
    [ -s "$reconcile" ]
    rm "$reconcile"
    jq '.operationId="foreign"' "$root/owned-load-cleanup/active" >"$TEST_ROOT/active"
    cat "$TEST_ROOT/active" >"$root/owned-load-cleanup/active"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    rm "$root/owned-load-cleanup/active"
    export CLEANUP_PHASE=prepare
    chmod 0755 "$(dirname "$PROOF")"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    chmod 0700 "$(dirname "$PROOF")"
    ln -s "$TEST_ROOT/forbidden-proof" "$PROOF"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    [ ! -e "$TEST_ROOT/forbidden-proof" ]
    assert_untouched
}

@test "deployed smoke and TLS source pins refuse stale or symlinked helpers" {
    printf '\n' >>"$root/scripts/lib/pipeline-remote/api-tls.sh"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    pin_dependencies
    mv "$root/scripts/smoke_test.sh" "$TEST_ROOT/smoke"
    ln -s "$TEST_ROOT/smoke" "$root/scripts/smoke_test.sh"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    [ ! -e "$PROOF" ]
    [ ! -e "$COMMAND_LOG" ]
    assert_untouched
}

@test "malformed private journal schema and pending TLS recovery refuse before public checks" {
    terminal_journal accepted
    for mutation in '.schema_version=true' '.owner="foreign"' '.snapshot[0].ttl=true' '.account="foreign-account"'; do
        cp "$root/api-dns/400-1.json" "$TEST_ROOT/terminal"
        jq "$mutation" "$TEST_ROOT/terminal" >"$root/api-dns/400-1.json"
        run bash "$DNS_OPERATION" verify-only
        [ "$status" -ne 0 ]
        [ ! -e "$PROOF" ]
        cat "$TEST_ROOT/terminal" >"$root/api-dns/400-1.json"
    done
    chmod 0644 "$root/api-dns/400-1.json"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    chmod 0600 "$root/api-dns/400-1.json"
    printf '{"phase":"activated"}\n' >"$root/api-tls-operation.json"
    chmod 0600 "$root/api-tls-operation.json"
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    [ ! -e "$COMMAND_LOG" ]
    assert_untouched
}

@test "commit phase requires matching reservation run while proof transport fields stay separate" {
    mkdir -m0700 "$root/owned-load-cleanup"
    jq -n --arg cat "$CLEANUP_CATALOGUE_SHA256" \
        '{schemaVersion:1,operationId:"cleanup-3962938a-54d0fe6f-73f5-49be-908f-4ec14aee486b",inputSha256:("a"*64),catalogueSha256:$cat,status:"prepared",run:"122-1"}' \
        >"$root/owned-load-cleanup/active"
    chmod 0600 "$root/owned-load-cleanup/active"
    export CLEANUP_PHASE=commit
    run bash "$DNS_OPERATION" verify-only
    [ "$status" -ne 0 ]
    jq '.run="123-1"' "$root/owned-load-cleanup/active" >"$TEST_ROOT/active"
    cat "$TEST_ROOT/active" >"$root/owned-load-cleanup/active"
    bash "$DNS_OPERATION" verify-only
    commit="${PROOF%-prepare.json}-commit.json"
    [ -s "$commit" ]
    export CLEANUP_PHASE=reconcile
    bash "$DNS_OPERATION" verify-only
    reconcile="${PROOF%-prepare.json}-reconcile.json"
    [ "$(jq -c .stableFacts "$commit")" = "$(jq -c .stableFacts "$reconcile")" ]
    [ "$(jq -r .stableDnsSha256 "$commit")" = "$(jq -r .stableDnsSha256 "$reconcile")" ]
    [ "$(jq -r .phase "$commit")" != "$(jq -r .phase "$reconcile")" ]
    assert_untouched
}
