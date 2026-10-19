#!/usr/bin/env bats

setup() {
    export TEST_ROOT
    TEST_ROOT=$(mktemp -d)
    export SCHOOLORBIT_STACK_ROOT="$TEST_ROOT/stack"
    export BASE_DOMAIN=example.test
    export ACCEPTED_APP_SHA=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    export API_TLS_CLOUDFLARE_TOKEN=fixture-dns-token
    export SMOKE_USERNAME=fixture SMOKE_PASSWORD=fixture-private-password
    export STAGED_FINGERPRINT
    STAGED_FINGERPRINT=$(printf new-certificate | sha256sum | cut -d ' ' -f 1)
    export COMMAND_LOG="$TEST_ROOT/commands"
    export TLS_OPERATION="$BATS_TEST_DIRNAME/../../lib/pipeline-remote/api-tls-operation.sh"
    owner="$SCHOOLORBIT_STACK_ROOT/deployment/scripts/lib/pipeline-remote"
    mkdir -p "$owner" "$SCHOOLORBIT_STACK_ROOT/nginx/ssl" "$TEST_ROOT/bin"
    printf '%s\n' "$ACCEPTED_APP_SHA" >"$SCHOOLORBIT_STACK_ROOT/deployment/accepted-release"
    cat >"$TEST_ROOT/bin/openssl" <<'SH'
#!/usr/bin/env bash
if [[ -e $TEST_ROOT/current-certificate ]]; then cat "$TEST_ROOT/current-certificate"; else printf '%s' old-certificate; fi
SH
    cat >"$owner/api-tls.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf 'tls %s\n' "$1" >>"$COMMAND_LOG"
case "$1" in
    stage)
        jq -e '.cloudflare_token == "fixture-dns-token"' >/dev/null
        [[ ${FAIL_STAGE:-false} != true ]] || exit 1
        jq -n --arg fingerprint "${STAGED_FINGERPRINT:-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}" '{fingerprint:$fingerprint}'
        ;;
    activate)
        test -s "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
        [[ ${FAIL_ACTIVATE:-false} != true ]] || exit 1
        printf new-certificate >"$TEST_ROOT/current-certificate"
        ;;
    status) [[ -e $TEST_ROOT/current-certificate || ${CURRENT_CERT_PUBLIC:-false} == true ]] ;;
esac
SH
    cat >"$owner/maintenance.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
test "$RELEASE_SHA" = "$ACCEPTED_APP_SHA"
printf 'maintenance %s\n' "$1" >>"$COMMAND_LOG"
case "$1" in
    enter|enter-origin-ca-migration) printf '%s\n' "$RELEASE_SHA" >"$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ;;
    verify) [[ ${FAIL_VERIFY:-false} != true ]] ;;
    accept) rm "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ;;
esac
SH
    chmod +x "$TEST_ROOT/bin/openssl"
    export PATH="$TEST_ROOT/bin:$PATH"
}

teardown() { rm -rf "$TEST_ROOT"; }

@test "renewal no-op validates current lineage and avoids maintenance" {
    export CURRENT_CERT_PUBLIC=true
    export STAGED_FINGERPRINT
    STAGED_FINGERPRINT=$(printf old-certificate | sha256sum | cut -d ' ' -f 1)
    bash "$TLS_OPERATION" renew
    [ "$(cat "$COMMAND_LOG")" = $'tls stage\ntls status' ]
}

@test "migration stages before explicit old-trust maintenance and preserves accepted application" {
    bash "$TLS_OPERATION" migrate-origin-ca
    [ "$(cat "$COMMAND_LOG")" = $'tls stage\ntls status\nmaintenance enter-origin-ca-migration\ntls activate\ntls status\nmaintenance verify\nmaintenance accept' ]
    [ "$(cat "$SCHOOLORBIT_STACK_ROOT/deployment/accepted-release")" = "$ACCEPTED_APP_SHA" ]
    [ ! -e "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    ! grep -Fq fixture-dns-token "$COMMAND_LOG"
}

@test "migration retries failed activation from the owned pending journal without another issuance" {
    export FAIL_ACTIVATE=true
    run bash "$TLS_OPERATION" migrate-origin-ca
    [ "$status" -ne 0 ]
    [ -L "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    jq -e '.phase=="maintenance" and .owner=="public-api-tls"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-tls-operation.json"
    unset FAIL_ACTIVATE
    bash "$TLS_OPERATION" migrate-origin-ca
    [ "$(grep -c '^tls stage$' "$COMMAND_LOG")" -eq 1 ]
    [ "$(grep -c '^tls activate$' "$COMMAND_LOG")" -eq 2 ]
    [ "$(grep -c '^maintenance enter-origin-ca-migration$' "$COMMAND_LOG")" -eq 2 ]
    [ ! -e "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    jq -e '.phase=="accepted"' "$SCHOOLORBIT_STACK_ROOT/deployment/api-tls-operation.json"
    [ "$(cat "$SCHOOLORBIT_STACK_ROOT/deployment/accepted-release")" = "$ACCEPTED_APP_SHA" ]
}

@test "already activated pending migration retries public verification and acceptance instead of no-op" {
    export FAIL_VERIFY=true
    run bash "$TLS_OPERATION" migrate-origin-ca
    [ "$status" -ne 0 ]
    unset FAIL_VERIFY
    bash "$TLS_OPERATION" migrate-origin-ca
    [ "$(grep -c '^tls stage$' "$COMMAND_LOG")" -eq 1 ]
    [ "$(grep -c '^tls activate$' "$COMMAND_LOG")" -eq 1 ]
    [ "$(grep -c '^maintenance verify$' "$COMMAND_LOG")" -eq 2 ]
    [ "$(grep -c '^maintenance enter-origin-ca-migration$' "$COMMAND_LOG")" -eq 1 ]
    [ "$(grep -c '^maintenance enter$' "$COMMAND_LOG")" -eq 1 ]
    grep -Fq 'maintenance accept' "$COMMAND_LOG"
    [ ! -e "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
}

@test "TLS recovery refuses a different mode, foreign pending marker or changed accepted identity" {
    export FAIL_VERIFY=true
    run bash "$TLS_OPERATION" migrate-origin-ca
    [ "$status" -ne 0 ]
    unset FAIL_VERIFY
    run bash "$TLS_OPERATION" renew
    [ "$status" -ne 0 ]
    pending="$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
    rm "$pending"
    printf '%s\n' "$ACCEPTED_APP_SHA" >"$pending"
    run bash "$TLS_OPERATION" migrate-origin-ca
    [ "$status" -ne 0 ]
    [ "$(grep -c '^maintenance verify$' "$COMMAND_LOG")" -eq 1 ]
    [ "$(grep -c '^tls stage$' "$COMMAND_LOG")" -eq 1 ]
}

@test "normal renewal uses public maintenance and acceptance failure retains maintenance" {
    export FAIL_VERIFY=true
    run bash "$TLS_OPERATION" renew
    [ "$status" -ne 0 ]
    [ -s "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
    ! grep -Fq 'maintenance accept' "$COMMAND_LOG"
    ! grep -Fq enter-origin-ca-migration "$COMMAND_LOG"
}

@test "pending application release and changed accepted identity refuse issuance" {
    touch "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
    run bash "$TLS_OPERATION" renew
    [ "$status" -ne 0 ]
    [ ! -e "$COMMAND_LOG" ]
    rm "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release"
    printf '%040d\n' 1 >"$SCHOOLORBIT_STACK_ROOT/deployment/accepted-release"
    run bash "$TLS_OPERATION" renew
    [ "$status" -ne 0 ]
    [ ! -e "$COMMAND_LOG" ]
}

@test "failed DNS issuance never enters maintenance or activates" {
    export FAIL_STAGE=true
    run bash "$TLS_OPERATION" renew
    [ "$status" -ne 0 ]
    [ "$(cat "$COMMAND_LOG")" = 'tls stage' ]
    [ ! -e "$SCHOOLORBIT_STACK_ROOT/deployment/pending-release" ]
}
