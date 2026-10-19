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
    accept) rm "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ;;
esac
SH
    cat >"$root/scripts/smoke_test.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
test "$SMOKE_ACADEMIC_CONTEXT" = true
test "$SMOKE_REQUIRE_AUTH" = true
test -z "$SMOKE_RESOLVE_IP"
test -z "$SMOKE_CA_CERT"
printf 'public authenticated full-context smoke\n' >>"$COMMAND_LOG"
[[ ${FAIL_PUBLIC:-false} != true ]]
SH
    cat >"$TEST_ROOT/bin/curl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
output= request= body= url=
while (($#)); do
    case "$1" in
        --output) output=$2; shift 2 ;;
        --request) request=$2; shift 2 ;;
        --data-binary) body=${2#@}; shift 2 ;;
        --header|--resolve|--max-time) shift 2 ;;
        http://*|https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
case "$url" in
    */zones\?*) cp "$FIXTURE_DIR/cloudflare-zone.json" "$output" ;;
    */settings/ssl) printf '{"success":true,"result":{"value":"strict"}}\n' >"$output" ;;
    */dns_records\?*) cp "$DNS_STATE" "$output" ;;
    */dns_records/batch)
        cp "$body" "$TEST_ROOT/last-batch.json"
        jq --slurpfile batch "$body" '.result |= map(. as $record | ($batch[0].patches | map(select(.id == $record.id)) | .[0]) as $patch | if $patch then $record + $patch else $record end)' "$DNS_STATE" >"$DNS_STATE.next"
        mv "$DNS_STATE.next" "$DNS_STATE"
        printf 'batch\n' >>"$COMMAND_LOG"
        printf '{"success":true}\n' >"$output"
        ;;
    */ready) printf '{"status":"ready"}\n' ;;
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
    [ "$(cat "$COMMAND_LOG")" = $'maintenance enter\nmaintenance verify\nbatch\npublic authenticated full-context smoke\nmaintenance accept' ]
    [ "$(stat -c %a "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns/123-1.json")" = 600 ]
    ! grep -R -Fq private-dns-fixture-token "$SCHOOLORBIT_STACK_ROOT/deployment/api-dns"
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
    ! grep -Fq 'maintenance accept' "$COMMAND_LOG"
}
