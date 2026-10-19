#!/usr/bin/env bats

setup() {
    export TEST_ROOT
    TEST_ROOT=$(mktemp -d)
    export SCHOOLORBIT_STACK_ROOT="$TEST_ROOT/stack"
    export BASE_DOMAIN=schoolorbit.app TARGET_ORIGIN_IP=192.0.2.20
    export ACCEPTED_APP_SHA=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    export API_TLS_CLOUDFLARE_TOKEN=private-dns-fixture-token
    export SMOKE_USERNAME=fixture SMOKE_PASSWORD=fixture-private-password
    export API_DNS_REQUEST_ID=123-1 API_DNS_ROLLBACK_ID=123-1
    export SO_PROVIDER_POLL_ATTEMPTS=1 SO_PROVIDER_POLL_DELAY=0
    export COMMAND_LOG="$TEST_ROOT/commands" DNS_STATE="$TEST_ROOT/dns.json"
    export FIXTURE_DIR="$BATS_TEST_DIRNAME/fixtures"
    export XDG_RUNTIME_DIR="$TEST_ROOT/runtime"
    export DNS_OPERATION="$BATS_TEST_DIRNAME/../../lib/pipeline-remote/api-dns-operation.sh"
    root="$SCHOOLORBIT_STACK_ROOT/deployment"
    mkdir -p "$root/scripts/lib/pipeline-remote" "$root/scripts/lib/schoolorbit-installer" "$TEST_ROOT/bin" "$XDG_RUNTIME_DIR"
    cp "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/"{common,cloudflare}.sh "$root/scripts/lib/schoolorbit-installer/"
    printf '%s\n' "$ACCEPTED_APP_SHA" >"$root/accepted-release"
    jq '(.result[].content)="192.0.2.20" | .result += [{id:"unrelated",type:"CNAME",name:"server.schoolorbit.app",content:"fixture.cfargotunnel.com",ttl:1,proxied:true,modified_on:"fixture"}]' "$FIXTURE_DIR/cloudflare-dns.json" >"$DNS_STATE"
    cat >"$root/scripts/lib/pipeline-remote/api-tls.sh" <<'SH'
#!/usr/bin/env bash
[[ ${FAIL_TLS:-false} != true ]]
SH
    cat >"$root/scripts/lib/pipeline-remote/maintenance.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
test "$RELEASE_SHA" = "$ACCEPTED_APP_SHA"
printf 'maintenance %s\n' "$1" >>"$COMMAND_LOG"
case "$1" in
    enter)
        printf '%s\n' "$RELEASE_SHA" >"$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
        mkdir -p "$SCHOOLORBIT_STACK_ROOT/deployment/releases/$RELEASE_SHA"
        printf probe >"$SCHOOLORBIT_STACK_ROOT/deployment/releases/$RELEASE_SHA/probe-token"
        ;;
    verify) [[ ${FAIL_VERIFY:-false} != true ]] ;;
    accept)
        rm "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
        [[ ${FAIL_ACCEPT:-false} != true ]]
        ;;
esac
SH
    cat >"$root/scripts/smoke_test.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
test "$SMOKE_ACADEMIC_CONTEXT" = true
test "$SMOKE_REQUIRE_AUTH" = true
test -z "$SMOKE_RESOLVE_IP"
test -z "$SMOKE_CA_CERT"
test -z "${SMOKE_RELEASE_PROBE_TOKEN:-}"
test ! -e "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
printf 'public authenticated full-context smoke\n' >>"$COMMAND_LOG"
if [[ ${FOREIGN_PENDING_ON_PUBLIC:-false} == true ]]; then
    printf 'foreign-release-owner\n' >"$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
    exit 1
fi
if [[ ${UNSAFE_OWNED_PENDING_ON_PUBLIC:-false} == true ]]; then
    chmod 0644 "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/$API_DNS_REQUEST_ID.pending-release"
    exit 1
fi
if [[ ${INTERRUPT_PUBLIC:-false} == true ]]; then kill -TERM "$PPID"; fi
[[ ${FAIL_PUBLIC:-false} != true ]]
SH
    cat >"$TEST_ROOT/bin/curl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
output= request= body= url= write_status=false
while (($#)); do
    case "$1" in
        --output) output=$2; shift 2 ;;
        --write-out) write_status=true; shift 2 ;;
        --request) request=$2; shift 2 ;;
        --data-binary) body=${2#@}; shift 2 ;;
        --header|--resolve|--max-time) shift 2 ;;
        http://*|https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
case "$url" in
    */zones\?*) cp "$FIXTURE_DIR/cloudflare-zone.json" "$output" ;;
    */settings/ssl) printf 'forbidden SSL settings lookup\n' >>"$COMMAND_LOG"; exit 22 ;;
    */dns_records\?*) cp "$DNS_STATE" "$output" ;;
    */dns_records/batch)
        cp "$body" "$TEST_ROOT/last-batch.json"
        jq --slurpfile batch "$body" '.result |= map(. as $record | ($batch[0].patches | map(select(.id == $record.id)) | .[0]) as $patch | if $patch then $record + $patch else $record end)' "$DNS_STATE" >"$DNS_STATE.next"
        mv "$DNS_STATE.next" "$DNS_STATE"
        printf 'batch\n' >>"$COMMAND_LOG"
        printf '{"success":true}\n' >"$output"
        ;;
    */ready)
        if [[ $write_status == true ]]; then
            test -s "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
            printf '{"error":"maintenance"}\n' >"$output"
            printf 'public maintenance TLS\n' >>"$COMMAND_LOG"
            if [[ ${FAIL_PUBLIC_TLS:-false} == true ]]; then printf 502; else printf 503; fi
        else
            printf '{"status":"ready"}\n'
        fi
        ;;
    *) exit 64 ;;
esac
SH
    cat >"$TEST_ROOT/bin/getent" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
jq -r --arg host "$2" '.result[] | select(.name == $host) | if .proxied then "203.0.113.41 STREAM fixture" else "\(.content) STREAM fixture" end' "$DNS_STATE"
SH
    chmod +x "$TEST_ROOT/bin/"*
    export PATH="$TEST_ROOT/bin:$PATH"
}

teardown() { rm -rf "$TEST_ROOT"; }

@test "existing origin cutover changes only both API proxy flags after direct and authenticated checks" {
    bash "$DNS_OPERATION" dns-only
    jq -e '.patches | length == 2 and all(.[]; .type=="A" and .proxied==false and .content=="192.0.2.20")' "$TEST_ROOT/last-batch.json"
    jq -e '.result[] | select(.id=="unrelated") | .proxied==true and .content=="fixture.cfargotunnel.com"' "$DNS_STATE"
    [ "$(cat "$COMMAND_LOG")" = $'maintenance enter\nmaintenance verify\nbatch\npublic maintenance TLS\npublic maintenance TLS\nmaintenance accept\npublic authenticated full-context smoke' ]
    jq -e '.status=="accepted"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
    [ "$(stat -c %a "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json")" = 600 ]
    ! grep -R -Fq private-dns-fixture-token "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns"
    ! grep -Fq 'SSL settings' "$COMMAND_LOG"
}

@test "TLS failure and extra AAAA refuse DNS mutation" {
    export FAIL_TLS=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    [ ! -e "$TEST_ROOT/last-batch.json" ]
    unset FAIL_TLS
    jq '.result += [(.result[0] | .id="extra-v6" | .type="AAAA" | .content="2001:db8::1")]' "$DNS_STATE" >"$DNS_STATE.next"
    mv "$DNS_STATE.next" "$DNS_STATE"
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    [ ! -e "$TEST_ROOT/last-batch.json" ]
}

@test "failed private authenticated smoke refuses DNS mutation and keeps maintenance" {
    export FAIL_VERIFY=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    [ ! -e "$TEST_ROOT/last-batch.json" ]
    [ -s "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
}

@test "rollback preserves snapshot fields and refuses changed cutover records" {
    bash "$DNS_OPERATION" dns-only
    bash "$DNS_OPERATION" dns-rollback
    jq -e '.patches | length==2 and all(.[]; .proxied==true and .content=="192.0.2.20" and .ttl==1)' "$TEST_ROOT/last-batch.json"
    export API_DNS_REQUEST_ID=124-1 API_DNS_ROLLBACK_ID=124-1
    bash "$DNS_OPERATION" dns-only
    jq '(.result[] | select(.id=="dns-school-1") | .ttl)=300' "$DNS_STATE" >"$DNS_STATE.next"
    mv "$DNS_STATE.next" "$DNS_STATE"
    run bash "$DNS_OPERATION" dns-rollback
    [ "$status" -ne 0 ]
    jq -e '.result[] | select(.id=="dns-school-1") | .proxied==false and .ttl==300' "$DNS_STATE"
}

@test "public smoke failure keeps maintenance active and retains rollback evidence" {
    export FAIL_PUBLIC=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    [ -s "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    jq -e '.status=="applied"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
    grep -Fq 'maintenance accept' "$COMMAND_LOG"
    [ "$(tail -n 1 "$COMMAND_LOG")" = 'maintenance enter' ]
    [ -L "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
}

@test "explicit applied-journal resume repeats acceptance without a second DNS mutation" {
    export FAIL_PUBLIC=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    unset FAIL_PUBLIC
    export API_DNS_RESUME_ID=123-1 API_DNS_REQUEST_ID=124-1
    bash "$DNS_OPERATION" dns-only
    [ "$(grep -c '^batch$' "$COMMAND_LOG")" -eq 1 ]
    [ ! -e "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    jq -e '.status=="accepted"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
    [ "$(cat "$SCHOOLORBIT_STACK_ROOT/deployment/accepted-release")" = "$ACCEPTED_APP_SHA" ]
}

@test "resume refuses a foreign pending file stale accepted identity and invalid journal ownership" {
    export FAIL_PUBLIC=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    unset FAIL_PUBLIC
    export API_DNS_RESUME_ID=123-1
    pending="$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
    original=$(readlink "$pending")
    rm "$pending"
    printf '%s\n' "$ACCEPTED_APP_SHA" >"$pending"
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    rm "$pending"
    ln -s "$original" "$pending"
    journal="$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
    chmod 0644 "$journal"
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    chmod 0600 "$journal"
    export ACCEPTED_APP_SHA=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    [ "$(grep -c '^batch$' "$COMMAND_LOG")" -eq 1 ]
}

@test "already DNS-only verification still performs public authenticated smoke without maintenance or DNS writes" {
    jq '(.result[] | select(.type=="A") | .proxied)=false' "$DNS_STATE" >"$DNS_STATE.next"
    mv "$DNS_STATE.next" "$DNS_STATE"
    bash "$DNS_OPERATION" dns-only
    [ "$(cat "$COMMAND_LOG")" = 'public authenticated full-context smoke' ]
    [ ! -e "$TEST_ROOT/last-batch.json" ]
    [ ! -e "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
}

@test "public TLS maintenance failure refuses reopening and retains the applied journal" {
    export FAIL_PUBLIC_TLS=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    ! grep -Fq 'maintenance accept' "$COMMAND_LOG"
    ! grep -Fq 'public authenticated' "$COMMAND_LOG"
    [ -L "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    jq -e '.status=="applied"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
}

@test "interruption after reopening restores owned maintenance before exiting" {
    export INTERRUPT_PUBLIC=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -eq 143 ]
    [ -L "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    [ "$(tail -n 1 "$COMMAND_LOG")" = 'maintenance enter' ]
    jq -e '.status=="applied"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
}

@test "failed normal acceptance also restores the owned pending marker" {
    export FAIL_ACCEPT=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    [ -L "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    [ "$(tail -n 1 "$COMMAND_LOG")" = 'maintenance enter' ]
    ! grep -Fq 'public authenticated full-context smoke' "$COMMAND_LOG"
}

@test "rollback acceptance retries validate restored records without another DNS batch" {
    bash "$DNS_OPERATION" dns-only
    export FAIL_PUBLIC=true
    run bash "$DNS_OPERATION" dns-rollback
    [ "$status" -ne 0 ]
    journal="$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
    jq -e '.status=="rollback_applied"' "$journal"
    [ -L "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    jq -e '.result[] | select(.id=="dns-school-1") | .proxied==true' "$DNS_STATE"
    unset FAIL_PUBLIC
    bash "$DNS_OPERATION" dns-rollback
    [ "$(grep -c '^batch$' "$COMMAND_LOG")" -eq 2 ]
    [ ! -e "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    jq -e '.status=="rolled_back"' "$journal"
}

@test "rollback interruption retains acceptance-only recovery and refuses restored DNS drift" {
    bash "$DNS_OPERATION" dns-only
    export INTERRUPT_PUBLIC=true
    run bash "$DNS_OPERATION" dns-rollback
    [ "$status" -eq 143 ]
    jq -e '.status=="rollback_applied"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
    unset INTERRUPT_PUBLIC
    jq '(.result[] | select(.id=="dns-school-1") | .ttl)=300' "$DNS_STATE" >"$DNS_STATE.next"
    mv "$DNS_STATE.next" "$DNS_STATE"
    run bash "$DNS_OPERATION" dns-rollback
    [ "$status" -ne 0 ]
    [ "$(grep -c '^batch$' "$COMMAND_LOG")" -eq 2 ]
    [ -L "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    jq '(.result[] | select(.id=="dns-school-1") | .ttl)=1' "$DNS_STATE" >"$DNS_STATE.next"
    mv "$DNS_STATE.next" "$DNS_STATE"
    bash "$DNS_OPERATION" dns-rollback
    [ "$(grep -c '^batch$' "$COMMAND_LOG")" -eq 2 ]
    jq -e '.status=="rolled_back"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
}

@test "conditional cleanup refuses a foreign pending marker without overwriting its owner" {
    export FOREIGN_PENDING_ON_PUBLIC=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    [[ "$output" == *'Failed to restore owned DNS maintenance'* ]]
    pending="$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
    [ ! -L "$pending" ]
    [ "$(cat "$pending")" = foreign-release-owner ]
    [ "$(grep -c '^maintenance enter$' "$COMMAND_LOG")" -eq 1 ]
    jq -e '.status=="applied"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json"
}

@test "conditional cleanup refuses unsafe private pending permissions before entering maintenance" {
    export UNSAFE_OWNED_PENDING_ON_PUBLIC=true
    run bash "$DNS_OPERATION" dns-only
    [ "$status" -ne 0 ]
    [[ "$output" == *'Failed to restore owned DNS maintenance'* ]]
    [ ! -e "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    [ "$(grep -c '^maintenance enter$' "$COMMAND_LOG")" -eq 1 ]
}
