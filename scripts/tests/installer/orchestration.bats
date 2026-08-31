#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/github.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/cloudflare.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/cloudflare_tunnel.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/vps.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/verification.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/phases.sh"

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
write_out=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output|-o) output=$2; shift 2 ;;
        --write-out) write_out=$2; shift 2 ;;
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
    https://server.example.test/ping) printf "%s\n" "{\"service\":\"cockpit\"}" >"$output" ;;
    https://server.example.test/) printf "%s\n" "<!doctype html><html><title>Cockpit Login</title></html>" >"$output" ;;
    http://192.0.2.20:9090/ping) exit 7 ;;
    *) printf "%s\n" "{}" >"$output" ;;
esac
if [[ $write_out == *url_effective* ]]; then
    printf "200\\n%s" "${FAKE_COCKPIT_EFFECTIVE_URL:-$url}"
else
    printf 200
fi
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

install_orchestration_fakes() {
    make_fake_command gh 'exit 0'
    make_fake_command ssh-keygen 'exit 0'
    make_fake_command openssl 'exit 0'
    generate_run_id() { printf '%s\n' run-test; }
    load_inputs() {
        SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]=bootstrap-token-value
        SO_SECRETS[SMOKE_SUBDOMAIN]=smoke-school
        SO_SECRETS[SMOKE_USERNAME]=smoke.operator
        SO_SECRETS[SMOKE_PASSWORD]=Smoke-Pass-7vK9nM3q
    }
    load_cloudflare_bootstrap_token() {
        SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]=bootstrap-token-value
    }
    load_cockpit_inputs() {
        SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]=bootstrap-token-value
        SO_SECRETS[SCHOOLORBIT_SERVER_PASSWORD]=Strong-Cockpit-Password-2026
    }
    github_preflight() { printf '%s\n' github-preflight >>"$FAKE_COMMAND_LOG"; }
    vps_preflight() { printf '%s\n' vps-preflight >>"$FAKE_COMMAND_LOG"; }
    cf_preflight() {
        SO_CF_ZONE_ID=zone-123
        SO_CF_ACCOUNT_ID=account-456
        SO_CF_ADMIN_RECORD_ID=dns-admin-1
        SO_CF_SCHOOL_RECORD_ID=dns-school-1
        if [[ ${FAKE_DNS_CUTOVER:-false} == true ]]; then
            SO_CF_DNS_RECORDS='[{"id":"dns-admin-1","type":"A","name":"admin-api.example.test","content":"192.0.2.20","ttl":1,"proxied":true,"modified_on":"2026-08-02T00:00:00Z"},{"id":"dns-school-1","type":"A","name":"school-api.example.test","content":"192.0.2.20","ttl":1,"proxied":true,"modified_on":"2026-08-02T00:00:00Z"}]'
        else
            SO_CF_DNS_RECORDS='[{"id":"dns-admin-1","type":"A","name":"admin-api.example.test","content":"198.51.100.10","ttl":1,"proxied":true,"modified_on":"2026-08-01T00:00:00Z"},{"id":"dns-school-1","type":"A","name":"school-api.example.test","content":"198.51.100.10","ttl":1,"proxied":true,"modified_on":"2026-08-01T00:00:00Z"}]'
        fi
    }
    cf_snapshot_dns() {
        SO_DNS_SNAPSHOT=$SO_CF_DNS_RECORDS
        SO_DNS_SNAPSHOT_ETAG=fixture-snapshot-etag
        SO_DNS_ORIGINAL_IP=198.51.100.10
    }
    cf_cockpit_preflight() {
        SO_CF_COCKPIT_HOSTNAME=server.example.test
        SO_CF_COCKPIT_CURRENT_RECORD=null
        if [[ ${SO_CF_COCKPIT_SNAPSHOT_READY:-false} != true ]]; then
            SO_CF_COCKPIT_RECORD_ID=
            SO_CF_COCKPIT_RECORD_EXISTED=false
        fi
    }
    cf_cockpit_snapshot() {
        SO_CF_COCKPIT_DNS_SNAPSHOT=$SO_CF_COCKPIT_CURRENT_RECORD
        SO_CF_COCKPIT_SNAPSHOT_READY=true
    }
    cf_cockpit_restore_checkpoint() {
        SO_CF_COCKPIT_HOSTNAME=$1
        SO_CF_COCKPIT_DNS_SNAPSHOT=$2
        SO_CF_COCKPIT_RECORD_ID=$3
        SO_CF_COCKPIT_RECORD_EXISTED=$4
        SO_CF_COCKPIT_TUNNEL_ID=$5
        SO_CF_COCKPIT_TUNNEL_NAME=$6
        SO_CF_COCKPIT_SNAPSHOT_READY=true
    }
    cf_cockpit_provision_tunnel() {
        SO_CF_COCKPIT_TUNNEL_ID=11111111-1111-4111-8111-111111111111
        SO_CF_COCKPIT_TUNNEL_NAME="schoolorbit-cockpit-$SO_RUN_ID"
        printf '%s\n' cockpit-tunnel-provisioned >>"$FAKE_COMMAND_LOG"
    }
    cf_cockpit_get_token() {
        SO_SECRETS[SCHOOLORBIT_COCKPIT_TUNNEL_TOKEN]=fixture-cockpit-tunnel-token-value
    }
    cf_cockpit_wait_connector() { printf '%s\n' cockpit-connector-ready >>"$FAKE_COMMAND_LOG"; }
    cf_cockpit_publish() {
        SO_CF_COCKPIT_RECORD_ID=cockpit-record-1
        if [[ ${FAKE_MANAGEMENT_PUBLISHED:-false} == true ]]; then
            printf '%s\n' cockpit-cname-reused >>"$FAKE_COMMAND_LOG"
        else
            FAKE_MANAGEMENT_PUBLISHED=true
            printf '%s\n' cockpit-cname-published >>"$FAKE_COMMAND_LOG"
        fi
    }
    cf_cockpit_assert_no_dns_drift() { return 0; }
    cf_cockpit_assert_published_state() { [[ ${FAKE_MANAGEMENT_PUBLISHED:-false} == true ]]; }
    cf_cockpit_restore_dns() {
        FAKE_MANAGEMENT_PUBLISHED=false
        printf '%s\n' cockpit-cname-restored >>"$FAKE_COMMAND_LOG"
    }
    cf_assert_no_dns_drift() { return 0; }
    cf_assert_cutover_state() { [[ ${FAKE_DNS_CUTOVER:-false} == true ]]; }
    cf_apply_dns_batch() {
        printf '%s\n' "$1-batch-applied" >>"$FAKE_COMMAND_LOG"
        [[ $1 != cutover ]] || FAKE_DNS_CUTOVER=true
    }
    cf_wait_for_record_content() { printf 'wait-content %s\n' "$1" >>"$FAKE_COMMAND_LOG"; }
    cf_wait_for_proxy_resolution() { printf '%s\n' wait-proxy >>"$FAKE_COMMAND_LOG"; }
    vps_bootstrap() { printf '%s\n' apt-get-bootstrap >>"$FAKE_COMMAND_LOG"; }
    vps_install_runtime_env() { printf '%s\n' runtime-env-installed >>"$FAKE_COMMAND_LOG"; }
    vps_configure_cockpit() { printf '%s\n' cockpit-configured >>"$FAKE_COMMAND_LOG"; }
    vps_reverify_cockpit() { return 0; }
    vps_create_deployment_key() {
        SO_SECRETS[SSH_PRIVATE_KEY]=fixture-private-key
        printf '%s\n' deployment-key-created >>"$FAKE_COMMAND_LOG"
    }
    vps_cleanup_deployment_key() { printf '%s\n' deployment-key-cleaned >>"$FAKE_COMMAND_LOG"; }
    vps_issue_and_install_tls() {
        SO_CF_CERTIFICATE_ID=origin-cert-123
        SO_CF_CERTIFICATE_EXPIRES=2041-08-02T00:00:00Z
        SO_CF_ORIGIN_ROOT_FILE="$TEST_ROOT/origin-root.pem"
        printf '%s\n' fixture-root >"$SO_CF_ORIGIN_ROOT_FILE"
        printf '%s\n' tls-installed >>"$FAKE_COMMAND_LOG"
    }
    github_configure_repository() { printf '%s\n' 'variable set rollout=false; secret set runtime' >>"$FAKE_COMMAND_LOG"; }
    github_dispatch_and_wait() {
        printf 'workflow run %s deployment_id=%s\n' "$1" "$2" >>"$FAKE_COMMAND_LOG"
        SO_GITHUB_RUN_ID=$((700 + $(grep -c '^workflow run' "$FAKE_COMMAND_LOG")))
        SO_GITHUB_RUN_URL="https://github.invalid/runs/$SO_GITHUB_RUN_ID"
    }
    github_set_variable() { printf 'variable set %s=%s\n' "$1" "$2" >>"$FAKE_COMMAND_LOG"; }
    github_variable_equals() { return 0; }
    github_runs_succeeded() { return 0; }
    verify_direct_origin() { printf '%s\n' origin-verified >>"$FAKE_COMMAND_LOG"; }
    verify_public_services() {
        [[ ${FAKE_PUBLIC_VERIFY_FAILURE:-0} != 1 ]] || return 1
        printf '%s\n' public-verified >>"$FAKE_COMMAND_LOG"
    }
    verify_public_cockpit() {
        [[ ${FAKE_MANAGEMENT_VERIFY_FAILURE:-0} != 1 ]] || return 1
        printf '%s\n' cockpit-public-verified >>"$FAKE_COMMAND_LOG"
    }
    vps_reverify_bootstrap() { return 0; }
    vps_reverify_tls() {
        SO_CF_ORIGIN_ROOT_FILE="$TEST_ROOT/origin-root.pem"
        printf '%s\n' fixture-root >"$SO_CF_ORIGIN_ROOT_FILE"
    }
    confirm_exact() {
        printf '%s\n' "$1" >"$TEST_ROOT/confirmation"
        [[ ${FAKE_CONFIRM:-yes} == yes ]]
    }
}

@test "direct verification pins both API hostnames to the target" {
    local insecure_flag='--in''secure'

    verify_direct_origin

    grep -F -- '--resolve admin-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    grep -F -- '--resolve school-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    grep -F -- '--cacert' "$FAKE_COMMAND_LOG"
    ! grep -Fq -- "$insecure_flag" "$FAKE_COMMAND_LOG"
    grep -Fq 'podman-compose -f podman-compose.yml --dry-run up -d' "$FAKE_COMMAND_LOG"
    grep -Fq "podman inspect --format '{{if .State.Health.Status}}" "$FAKE_COMMAND_LOG"
    ! grep -Fq 'podman exec schoolorbit-nginx nginx -t' "$FAKE_COMMAND_LOG"
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

@test "public Cockpit verification checks the edge and rejects direct target access" {
    verify_public_cockpit

    grep -Fq 'https://server.example.test/ping' "$FAKE_COMMAND_LOG"
    grep -Fq 'https://server.example.test/' "$FAKE_COMMAND_LOG"
    grep -Fq 'http://192.0.2.20:9090/ping' "$FAKE_COMMAND_LOG"
    ! grep -Eq 'Strong-Cockpit-Password|Authorization:|Cookie:' "$FAKE_COMMAND_LOG"
}

@test "public Cockpit verification rejects a Cloudflare Access redirect" {
    export FAKE_COCKPIT_EFFECTIVE_URL=https://schoolorbit.cloudflareaccess.com/cdn-cgi/access/login

    run verify_public_cockpit
    [ "$status" -eq 78 ]
}

@test "smoke script covers resolve-pinned APIs SSE CORS and optional private files" {
    local smoke="$BATS_TEST_DIRNAME/../../smoke_test.sh"
    grep -Fq 'SMOKE_RESOLVE_IP' "$smoke"
    grep -Fq '/api/notifications/stream' "$smoke"
    grep -Fq 'text/event-stream' "$smoke"
    grep -Fq 'access-control-allow-credentials' "$smoke"
    grep -Fq 'FILE_SMOKE_PNG' "$smoke"
    grep -Fq '/api/files' "$smoke"
    grep -Fq 'expect_status "login validation" "$status" "400"' "$smoke"
    grep -Fq '__Host-schoolorbit_session' "$smoke"
    grep -Fq 'X-CSRF-Token' "$smoke"
    grep -Fq '/api/auth/sessions' "$smoke"
    grep -Fq '/api/auth/logout' "$smoke"
    grep -Fq '/api/academic/timetable-versions?academicTermId=$term_id' "$smoke"
    grep -Fq 'timetableVersionId=$timetable_version_id&academicTermId=$term_id' "$smoke"
    ! grep -Fq '"/api/academic/timetable?academicTermId=$term_id"' "$smoke"
    ! grep -Fq 'pass "login auth_token cookie"' "$smoke"
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
cookie_input=
has_session_cookie=false
has_legacy_cookie=false
has_csrf_header=false
url=
while [ "$#" -gt 0 ]; do
    case "$1" in
        -X) method=$2; shift 2 ;;
        -D) headers=$2; shift 2 ;;
        -o|--output) output=$2; shift 2 ;;
        -c) cookie_output=$2; shift 2 ;;
        -b)
            cookie_input=$2
            case "$cookie_input" in
                auth_token=*) has_legacy_cookie=true ;;
            esac
            shift 2
            ;;
        -H)
            case "$2" in
                X-CSRF-Token:*) has_csrf_header=true ;;
            esac
            shift 2
            ;;
        -F|-w|--write-out|--resolve|--max-time|--data|--data-binary) shift 2 ;;
        http://*|https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
if [ -n "$cookie_input" ] && [ -f "$cookie_input" ] && grep -q "__Host-schoolorbit_session" "$cookie_input"; then
    has_session_cookie=true
fi
[ -z "$headers" ] || {
    printf "%s\n" "HTTP/2 200" >"$headers"
    case "$url" in
        https://school-api.example.test/*)
            printf "%s\n" "Access-Control-Allow-Origin: https://smoke-school.example.test" >>"$headers"
            printf "%s\n" "Access-Control-Expose-Headers: x-csrf-token" >>"$headers"
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
        if [ "$has_session_cookie" = true ]; then
            body="{\"data\":{\"user\":{\"username\":\"smoke.operator\"}}}"
            printf "%s\n" "X-CSRF-Token: fixture-csrf-token" >>"$headers"
        else
            status=401
        fi
        ;;
    https://school-api.example.test/api/auth/login)
        if [ "$method" = OPTIONS ]; then
            status=204
            printf "%s\n" "Access-Control-Allow-Headers: content-type,x-school-subdomain,x-csrf-token" >>"$headers"
        else
            body="{\"data\":{\"user\":{\"username\":\"smoke.operator\"}}}"
            printf "%s\n" "X-CSRF-Token: fixture-csrf-token" >>"$headers"
            printf "%s\n" "#HttpOnly_school-api.example.test FALSE / TRUE 0 __Host-schoolorbit_session fixture" >"$cookie_output"
        fi
        ;;
    https://school-api.example.test/api/auth/sessions)
        if [ "$has_session_cookie" = true ]; then
            body="{\"data\":{\"sessions\":[{\"id\":\"00000000-0000-0000-0000-000000000001\",\"isCurrent\":true}]}}"
        else
            status=401
        fi
        ;;
    https://school-api.example.test/api/auth/logout)
        if [ "$has_session_cookie" = true ] && [ "$has_csrf_header" = true ]; then
            body="{\"data\":{}}"
            [ -z "$cookie_output" ] || : >"$cookie_output"
        else
            status=403
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
    [[ $output == *'legacy cookie /me status 401'* ]]
    [[ $output == *'session list exactly one current session'* ]]
    [[ $output == *'current session logout status 200'* ]]
    grep -F -- '--resolve admin-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    grep -F -- '--resolve school-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    grep -Fq -- '/api/auth/sessions' "$FAKE_COMMAND_LOG"
    grep -Fq -- '/api/auth/logout' "$FAKE_COMMAND_LOG"
    grep -Fq -- 'X-CSRF-Token: fixture-csrf-token' "$FAKE_COMMAND_LOG"
    tenant_request=$(grep -F 'https://smoke-school.example.test/' "$FAKE_COMMAND_LOG")
    [[ $tenant_request != *'--resolve'* ]]
    [[ $output != *'Smoke-Pass-7vK9nM3q'* ]]
}

@test "migration runs verified phases in the approved order" {
    install_orchestration_fakes

    run schoolorbit_main migrate-vps --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test

    [ "$status" -eq 0 ]
    expected='preflight input snapshot bootstrap tls deploy origin-verify cutover-gate dns-cutover public-verify management-provision management-publish handoff'
    [ "$(tr '\n' ' ' <"$PHASE_LOG" | sed 's/ $//')" = "$expected" ]
}

@test "standalone Cockpit setup avoids application DNS and GitHub deployment mutations" {
    install_orchestration_fakes

    run schoolorbit_main configure-cockpit --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test

    [ "$status" -eq 0 ]
    expected='preflight input management-snapshot bootstrap management-provision management-publish management-handoff'
    [ "$(tr '\n' ' ' <"$PHASE_LOG" | sed 's/ $//')" = "$expected" ]
    ! grep -Eq 'workflow run|cutover-batch-applied|rollback-batch-applied|tls-installed' "$FAKE_COMMAND_LOG"
}

@test "standalone Cockpit dry run is read-only" {
    install_orchestration_fakes

    run schoolorbit_main configure-cockpit --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test --dry-run

    [ "$status" -eq 0 ]
    [ "$(tr '\n' ' ' <"$PHASE_LOG" | sed 's/ $//')" = 'preflight input management-snapshot' ]
    ! grep -Eq 'cockpit-tunnel-provisioned|cockpit-configured|cockpit-cname-published|workflow run|batch-applied' "$FAKE_COMMAND_LOG"
}

@test "standalone management failure reports only Cockpit rollback" {
    install_orchestration_fakes
    export FAKE_MANAGEMENT_VERIFY_FAILURE=1

    run schoolorbit_main configure-cockpit --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test

    [ "$status" -ne 0 ]
    [[ $output == *'rollback-cockpit --run-id run-test'* ]]
    [[ $output != *'rollback-dns --run-id'* ]]
}

@test "standalone resume reuses a journaled management publication" {
    install_orchestration_fakes
    export FAKE_MANAGEMENT_VERIFY_FAILURE=1
    run schoolorbit_main configure-cockpit --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test
    [ "$status" -ne 0 ]
    [ "$(jq -r '.phases["management-publish"].status' "$SCHOOLORBIT_STATE_HOME/runs/run-test/state.json")" = published ]

    export FAKE_MANAGEMENT_VERIFY_FAILURE=0
    export FAKE_MANAGEMENT_PUBLISHED=true
    run schoolorbit_main configure-cockpit --resume run-test

    [ "$status" -eq 0 ]
    [ "$(grep -c '^cockpit-cname-published$' "$FAKE_COMMAND_LOG")" -eq 1 ]
    grep -Fxq cockpit-cname-reused "$FAKE_COMMAND_LOG"
    [ "$(jq -r '.phases["management-publish"].status' "$SCHOOLORBIT_STATE_HOME/runs/run-test/state.json")" = passed ]
}

@test "deployment phase dispatches the four workflows in dependency order" {
    install_orchestration_fakes
    schoolorbit_main migrate-vps --repository owner/repo --target 192.0.2.20 --base-domain example.test

    actual=$(awk '/^workflow run/ { print $3 }' "$FAKE_COMMAND_LOG" | tr '\n' ' ' | sed 's/ $//')
    expected='deploy-backend-admin.yml deploy-backend-school.yml deploy-frontend-admin.yml deploy-all-schools.yml'
    [ "$actual" = "$expected" ]
}

@test "dry run performs read-only phases and no mutation" {
    install_orchestration_fakes

    run schoolorbit_main migrate-vps --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test --dry-run

    [ "$status" -eq 0 ]
    [ "$(tr '\n' ' ' <"$PHASE_LOG" | sed 's/ $//')" = 'preflight input snapshot' ]
    ! grep -Eq 'secret set|variable set|batch-applied|apt-get|workflow run|tls-installed' "$FAKE_COMMAND_LOG"
}

@test "post-cutover failure offers rollback but does not execute it" {
    install_orchestration_fakes
    export FAKE_PUBLIC_VERIFY_FAILURE=1

    run schoolorbit_main migrate-vps --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test

    [ "$status" -ne 0 ]
    [[ $output == *'rollback-dns --run-id run-test'* ]]
    ! grep -Fq 'rollback-batch-applied' "$FAKE_COMMAND_LOG"
}

@test "resume skips only a reverified passed phase" {
    install_orchestration_fakes
    seed_checkpoint_with_passed_phase preflight
    : >"$PHASE_LOG"

    run schoolorbit_main migrate-vps --resume run-123

    [ "$status" -eq 0 ]
    [ "$(grep -c '^preflight$' "$PHASE_LOG")" -eq 0 ]
    grep -Fxq snapshot "$PHASE_LOG"
}

@test "cutover refusal leaves DNS unchanged" {
    install_orchestration_fakes
    export FAKE_CONFIRM=no

    run schoolorbit_main migrate-vps --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test

    [ "$status" -ne 0 ]
    ! grep -Fq 'cutover-batch-applied' "$FAKE_COMMAND_LOG"
}

@test "preflight authentication failure stops before every mutation" {
    install_orchestration_fakes
    github_preflight() { return 69; }

    run schoolorbit_main migrate-vps --repository owner/repo \
        --target 192.0.2.20 --base-domain example.test

    [ "$status" -eq 69 ]
    [[ $output == *'migrate-vps --resume run-test'* ]]
    ! grep -Eq 'secret set|variable set|batch-applied|apt-get|workflow run|tls-installed' "$FAKE_COMMAND_LOG"
}

@test "resume aborts instead of reapplying a passed phase that fails revalidation" {
    install_orchestration_fakes
    seed_checkpoint_with_passed_phase preflight
    vps_preflight() { return 1; }
    : >"$PHASE_LOG"

    run schoolorbit_main migrate-vps --resume run-123

    [ "$status" -eq 78 ]
    [ ! -s "$PHASE_LOG" ]
    [[ $output == *'Checkpoint phase failed revalidation: preflight'* ]]
}

@test "rollback requires the exact original IP confirmation and applies one reverse batch" {
    install_orchestration_fakes
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    SO_CONFIG[ref]=main
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    state_init run-rollback
    snapshot='[{"id":"dns-admin-1","type":"A","name":"admin-api.example.test","content":"198.51.100.10","ttl":1,"proxied":true,"modified_on":"2026-08-01T00:00:00Z"},{"id":"dns-school-1","type":"A","name":"school-api.example.test","content":"198.51.100.10","ttl":1,"proxied":true,"modified_on":"2026-08-01T00:00:00Z"}]'
    state_mark_phase snapshot "$(jq -n --arg zone zone-123 --arg account account-456 --arg original 198.51.100.10 --arg etag fixture-snapshot-etag --argjson dns "$snapshot" '{status:"passed",cloudflare_zone_id:$zone,cloudflare_account_id:$account,original_ip:$original,dns_snapshot_etag:$etag,dns_snapshot:$dns}')"
    state_mark_phase dns-cutover '{"status":"passed"}'
    export FAKE_DNS_CUTOVER=true

    run schoolorbit_main rollback-dns --run-id run-rollback

    [ "$status" -eq 0 ]
    [ "$(<"$TEST_ROOT/confirmation")" = 'ROLLBACK 198.51.100.10' ]
    [ "$(grep -c '^rollback-batch-applied$' "$FAKE_COMMAND_LOG")" -eq 1 ]
}

@test "Cockpit rollback requires exact hostname confirmation and changes no API DNS" {
    install_orchestration_fakes
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    SO_CONFIG[ref]=main
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    state_init cockpit-rollback
    state_mark_phase management-snapshot '{"status":"passed","cloudflare_zone_id":"zone-123","cloudflare_account_id":"account-456","management_hostname":"server.example.test","management_dns_snapshot":null,"management_record_id":"","management_record_existed":false}'
    state_mark_phase management-provision '{"status":"passed","management_hostname":"server.example.test","management_dns_snapshot":null,"management_record_id":"","management_record_existed":false,"management_tunnel_id":"11111111-1111-4111-8111-111111111111","management_tunnel_name":"schoolorbit-cockpit-cockpit-rollback"}'
    state_mark_phase management-publish '{"status":"passed","management_hostname":"server.example.test","management_dns_snapshot":null,"management_record_id":"cockpit-record-1","management_record_existed":false,"management_tunnel_id":"11111111-1111-4111-8111-111111111111","management_tunnel_name":"schoolorbit-cockpit-cockpit-rollback"}'
    export FAKE_MANAGEMENT_PUBLISHED=true

    run schoolorbit_main rollback-cockpit --run-id cockpit-rollback

    [ "$status" -eq 0 ]
    [ "$(<"$TEST_ROOT/confirmation")" = 'ROLLBACK COCKPIT server.example.test' ]
    grep -Fxq cockpit-cname-restored "$FAKE_COMMAND_LOG"
    ! grep -Eq 'cutover-batch-applied|rollback-batch-applied' "$FAKE_COMMAND_LOG"
}

@test "full DNS rollback also restores a published management CNAME" {
    install_orchestration_fakes
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    SO_CONFIG[ref]=main
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    state_init full-rollback
    snapshot='[{"id":"dns-admin-1","type":"A","name":"admin-api.example.test","content":"198.51.100.10","ttl":1,"proxied":true,"modified_on":"2026-08-01T00:00:00Z"},{"id":"dns-school-1","type":"A","name":"school-api.example.test","content":"198.51.100.10","ttl":1,"proxied":true,"modified_on":"2026-08-01T00:00:00Z"}]'
    state_mark_phase snapshot "$(jq -n --arg zone zone-123 --arg account account-456 --arg original 198.51.100.10 --arg etag fixture-snapshot-etag --argjson dns "$snapshot" --arg management_hostname server.example.test '{status:"passed",cloudflare_zone_id:$zone,cloudflare_account_id:$account,original_ip:$original,dns_snapshot_etag:$etag,dns_snapshot:$dns,management_hostname:$management_hostname,management_dns_snapshot:null,management_record_id:"",management_record_existed:false}')"
    state_mark_phase dns-cutover '{"status":"passed"}'
    state_mark_phase management-publish '{"status":"passed","management_hostname":"server.example.test","management_dns_snapshot":null,"management_record_id":"cockpit-record-1","management_record_existed":false,"management_tunnel_id":"11111111-1111-4111-8111-111111111111","management_tunnel_name":"schoolorbit-cockpit-full-rollback"}'
    export FAKE_DNS_CUTOVER=true
    export FAKE_MANAGEMENT_PUBLISHED=true

    run schoolorbit_main rollback-dns --run-id full-rollback

    [ "$status" -eq 0 ]
    grep -Fxq rollback-batch-applied "$FAKE_COMMAND_LOG"
    grep -Fxq cockpit-cname-restored "$FAKE_COMMAND_LOG"
}
