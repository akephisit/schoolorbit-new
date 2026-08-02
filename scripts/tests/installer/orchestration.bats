#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/vps.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/verification.sh"

    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    SO_SECRETS[SMOKE_SUBDOMAIN]=smoke-school
    SO_SECRETS[SMOKE_USERNAME]=smoke.operator
    SO_SECRETS[SMOKE_PASSWORD]=Smoke-Pass-7vK9nM3q
    SO_CF_ORIGIN_ROOT_FILE="$TEST_ROOT/cloudflare-origin-root.pem"
    printf '%s\n' fixture-root >"$SO_CF_ORIGIN_ROOT_FILE"
    export FAKE_ADMIN_IDENTITY='SchoolOrbit Backend Admin'
    export FAKE_SCHOOL_IDENTITY='SchoolOrbit Backend School'

    make_fake_command curl '
set -eu
printf "curl" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"
output=
url=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output|-o) output=$2; shift 2 ;;
        http://*|https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
case "$url" in
    https://admin-api.example.test/ready) printf "%s\n" "{\"status\":\"ready\",\"database\":\"connected\"}" >"$output" ;;
    https://admin-api.example.test/) printf "{\"service\":\"%s\"}\n" "$FAKE_ADMIN_IDENTITY" >"$output" ;;
    https://school-api.example.test/ready) printf "%s\n" "{\"status\":\"ready\",\"controlPlane\":\"connected\",\"filePlatform\":\"ready\"}" >"$output" ;;
    https://school-api.example.test/) printf "{\"service\":\"%s\"}\n" "$FAKE_SCHOOL_IDENTITY" >"$output" ;;
    https://admin.example.test/|https://smoke-school.example.test/) printf "%s\n" "<!doctype html><html></html>" >"$output" ;;
    *) printf "%s\n" "{}" >"$output" ;;
esac
printf 200
'

    make_fake_command ssh '
printf "ssh" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"
cat >>"$FAKE_COMMAND_LOG"
'

    make_fake_command smoke-check '
set -eu
[ "$SMOKE_REQUIRE_AUTH" = true ]
[ "$SMOKE_SUBDOMAIN" = smoke-school ]
[ "$SMOKE_USERNAME" = smoke.operator ]
[ "$SMOKE_PASSWORD" = Smoke-Pass-7vK9nM3q ]
printf "%s\n" "smoke authenticated" >>"$FAKE_COMMAND_LOG"
printf "verified %s\n" "$SMOKE_SUBDOMAIN"
'
    export SCHOOLORBIT_SMOKE_SCRIPT="$FAKE_BIN/smoke-check"
}

teardown() {
    vps_cleanup_transients
    teardown_installer_test
}

@test "direct verification pins both API hostnames to the target" {
    verify_direct_origin

    grep -F -- '--resolve admin-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    grep -F -- '--resolve school-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    grep -F -- '--cacert' "$FAKE_COMMAND_LOG"
    ! grep -Fq -- '--insecure' "$FAKE_COMMAND_LOG"
    grep -Fq 'podman-compose -f podman-compose.yml config' "$FAKE_COMMAND_LOG"
    grep -Fq 'podman exec schoolorbit-nginx nginx -t' "$FAKE_COMMAND_LOG"
}

@test "direct verification fails when either service identity is wrong" {
    export FAKE_ADMIN_IDENTITY=wrong

    run verify_direct_origin
    [ "$status" -ne 0 ]
}

@test "public verification fails when either service identity is wrong" {
    export FAKE_ADMIN_IDENTITY=wrong

    run verify_public_services
    [ "$status" -ne 0 ]
}

@test "public verification requires authenticated smoke credentials" {
    SO_SECRETS[SMOKE_PASSWORD]=

    run verify_public_services
    [ "$status" -ne 0 ]
}

@test "public verification checks APIs frontends and authenticated smoke" {
    verify_public_services

    grep -Fq 'https://admin.example.test/' "$FAKE_COMMAND_LOG"
    grep -Fq 'https://smoke-school.example.test/' "$FAKE_COMMAND_LOG"
    grep -Fq 'smoke authenticated' "$FAKE_COMMAND_LOG"
}

@test "public verification redacts smoke credentials from child output" {
    run verify_public_services

    [ "$status" -eq 0 ]
    [[ $output == *'[REDACTED]'* ]]
    [[ $output != *'smoke-school'* ]]
    [[ $output != *'Smoke-Pass-7vK9nM3q'* ]]
}

@test "smoke script covers resolve-pinned APIs SSE CORS and optional private files" {
    local smoke="$BATS_TEST_DIRNAME/../../smoke_test.sh"
    grep -Fq 'SMOKE_RESOLVE_IP' "$smoke"
    grep -Fq '/api/notifications/stream' "$smoke"
    grep -Fq 'text/event-stream' "$smoke"
    grep -Fq 'access-control-allow-credentials' "$smoke"
    grep -Fq 'FILE_SMOKE_PNG' "$smoke"
    grep -Fq '/api/files' "$smoke"
}

@test "smoke resolve pins only API calls and accepts a bounded authenticated SSE stream" {
    make_fake_command curl '
set -eu
printf "curl" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"
method=GET
headers=
output=
cookie_output=
had_cookie=false
url=
while [ "$#" -gt 0 ]; do
    case "$1" in
        -X) method=$2; shift 2 ;;
        -D) headers=$2; shift 2 ;;
        -o|--output) output=$2; shift 2 ;;
        -c) cookie_output=$2; shift 2 ;;
        -b) had_cookie=true; shift 2 ;;
        -H|-F|-w|--write-out|--resolve|--max-time|--data|--data-binary) shift 2 ;;
        http://*|https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
[ -z "$headers" ] || {
    printf "%s\n" "HTTP/2 200" >"$headers"
    case "$url" in
        https://school-api.example.test/*)
            printf "%s\n" "Access-Control-Allow-Origin: https://smoke-school.example.test" >>"$headers"
            ;;
    esac
}
status=200
body="{}"
case "$url" in
    https://smoke-school.example.test/) body="<!doctype html><html></html>" ;;
    https://admin-api.example.test/health) body="{\"status\":\"healthy\"}" ;;
    https://admin-api.example.test/ready) body="{\"status\":\"ready\"}" ;;
    https://school-api.example.test/health) body="{\"status\":\"healthy\"}" ;;
    https://school-api.example.test/ready) body="{\"status\":\"ready\",\"controlPlane\":\"connected\",\"filePlatform\":\"ready\"}" ;;
    https://school-api.example.test/api/auth/me)
        if [ "$had_cookie" = true ]; then
            body="{\"data\":{\"user\":{\"username\":\"smoke.operator\"}}}"
        else
            status=401
        fi
        ;;
    https://school-api.example.test/api/auth/login)
        if [ "$method" = OPTIONS ]; then
            status=204
            printf "%s\n" "Access-Control-Allow-Headers: content-type,authorization,x-school-subdomain" >>"$headers"
        else
            body="{\"data\":{\"user\":{\"username\":\"smoke.operator\"}}}"
            printf "%s\n" "#HttpOnly_example.test TRUE / TRUE 0 auth_token fixture" >"$cookie_output"
        fi
        ;;
    https://school-api.example.test/api/notifications/stream)
        printf "%s\n" "Content-Type: text/event-stream" >>"$headers"
        printf "%s\n" "Access-Control-Allow-Credentials: true" >>"$headers"
        [ "$output" = /dev/null ] || printf "%s" "" >"$output"
        printf 200
        exit 28
        ;;
esac
[ "$output" = /dev/null ] || printf "%s\n" "$body" >"$output"
printf "%s" "$status"
'

    run env \
        SMOKE_ENV_FILE="$TEST_ROOT/missing-smoke-env" \
        SMOKE_SUBDOMAIN=smoke-school \
        SMOKE_API_URL=https://school-api.example.test \
        SMOKE_ADMIN_API_URL=https://admin-api.example.test \
        SMOKE_TENANT_URL=https://smoke-school.example.test \
        SMOKE_ORIGIN=https://smoke-school.example.test \
        SMOKE_USERNAME=smoke.operator \
        SMOKE_PASSWORD=Smoke-Pass-7vK9nM3q \
        SMOKE_REQUIRE_AUTH=true \
        SMOKE_RESOLVE_IP=192.0.2.20 \
        scripts/smoke_test.sh

    [ "$status" -eq 0 ]
    [[ $output == *'notification SSE status 200'* ]]
    grep -F -- '--resolve admin-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    grep -F -- '--resolve school-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    tenant_request=$(grep -F 'https://smoke-school.example.test/' "$FAKE_COMMAND_LOG")
    [[ $tenant_request != *'--resolve'* ]]
    [[ $output != *'Smoke-Pass-7vK9nM3q'* ]]
}
