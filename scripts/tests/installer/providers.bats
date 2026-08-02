#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
    export FIXTURE_DIR="$BATS_TEST_DIRNAME/fixtures"
    export GITHUB_RUN_FIXTURE="$FIXTURE_DIR/github-runs.json"
    export CF_DNS_FIXTURE="$FIXTURE_DIR/cloudflare-dns.json"

    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/github.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/cloudflare.sh"

    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=schoolorbit.app
    SO_CONFIG[ref]=main
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    SO_CONFIG[runtime:R2_ACCOUNT_ID]=9a8b7c6d5e4f32100123456789abcdef
    SO_CONFIG[runtime:VAPID_PUBLIC_KEY]=BHT7mN3qP9vK5xT2rL8wC4sF6dG1hJ0kZyUeIoaS
    SO_CF_ACCOUNT_ID=account-456

    SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]=cf-bootstrap-7vK9nM3qR8wX2zLp6tY4
    SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_DEPLOY_TOKEN]=cf-deploy-8wL2pN4rT7xZ5cV9mQ3s
    SO_SECRETS[INTERNAL_API_SECRET]=internal-api-7vK9nM3qR8wX2zLp
    SO_SECRETS[DEPLOY_KEY]=deploy-4rT8yP2mN6vK9xC3sL7q
    SO_SECRETS[SMOKE_USERNAME]=smoke.operator
    SO_SECRETS[SMOKE_PASSWORD]=Smoke-Pass-7vK9nM3q
    SO_SECRETS[SSH_PRIVATE_KEY]='ssh-private-value-kept-on-stdin'

    make_fake_command gh '
set -eu
printf "gh" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"
case "${1-} ${2-}" in
    "auth status") exit 0 ;;
    "api repos/owner/repo/actions/permissions/workflow") printf "write\n" ;;
    "api repos/owner/repo/actions/variables/RUNTIME_DEPLOY_ENABLED") printf "true\n" ;;
    "secret set") cat >"$CAPTURED_STDIN" ;;
    "run list") cat "$GITHUB_RUN_FIXTURE" ;;
    "run watch") exit 0 ;;
esac
'

    make_fake_command curl '
set -eu
output=
request=
url=
printf "curl" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) output=$2; shift 2 ;;
        --data-binary) request=${2#@}; shift 2 ;;
        http://*|https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
[ -n "$request" ] && cp "$request" "$CAPTURED_REQUEST_BODY"
case "$url" in
    */zones\?*) cp "$FIXTURE_DIR/cloudflare-zone.json" "$output" ;;
    */settings/ssl) printf "%s\n" "{\"success\":true,\"result\":{\"value\":\"strict\"}}" >"$output" ;;
    */dns_records/batch) printf "%s\n" "{\"success\":true,\"result\":{}}" >"$output" ;;
    */dns_records*) cp "$CF_DNS_FIXTURE" "$output" ;;
    */certificates) cp "$FIXTURE_DIR/cloudflare-certificate.json" "$output" ;;
    *) printf "%s\n" "{\"success\":false,\"errors\":[{\"code\":7000,\"message\":\"unknown fake endpoint\"}]}" >"$output" ;;
esac
'
}

teardown() {
    teardown_installer_test
}

@test "GitHub secrets are delivered through stdin" {
    github_set_secret INTERNAL_API_SECRET "${SO_SECRETS[INTERNAL_API_SECRET]}"

    run grep -F -- '--body internal-api-7vK9nM3qR8wX2zLp' "$FAKE_COMMAND_LOG"
    [ "$status" -eq 1 ]
    grep -F 'gh secret set INTERNAL_API_SECRET --repo owner/repo' "$FAKE_COMMAND_LOG"
    [ "$(<"$CAPTURED_STDIN")" = "${SO_SECRETS[INTERNAL_API_SECRET]}" ]
}

@test "GitHub repository configuration keeps rollout gates disabled" {
    github_configure_repository

    grep -F 'variable set BASE_DOMAIN --body schoolorbit.app --repo owner/repo' "$FAKE_COMMAND_LOG"
    grep -F 'variable set CLOUDFLARE_ACCOUNT_ID --body account-456 --repo owner/repo' "$FAKE_COMMAND_LOG"
    grep -F 'variable set RUNTIME_DEPLOY_ENABLED --body false --repo owner/repo' "$FAKE_COMMAND_LOG"
    grep -F 'variable set FRONTEND_DEPLOY_ENABLED --body false --repo owner/repo' "$FAKE_COMMAND_LOG"
    grep -F 'secret set SSH_PRIVATE_KEY --repo owner/repo' "$FAKE_COMMAND_LOG"
    grep -F 'secret set SERVER_PORT --repo owner/repo' "$FAKE_COMMAND_LOG"
    run grep -F -- '--body ssh-private-value-kept-on-stdin' "$FAKE_COMMAND_LOG"
    [ "$status" -eq 1 ]
}

@test "GitHub rollout gate verification uses the Actions variable API" {
    github_variable_equals RUNTIME_DEPLOY_ENABLED true

    grep -F 'gh api repos/owner/repo/actions/variables/RUNTIME_DEPLOY_ENABLED --jq .value' "$FAKE_COMMAND_LOG"
}

@test "workflow dispatch waits for exactly correlated display title" {
    github_dispatch_and_wait deploy-backend-admin.yml deploy-123

    [ "$SO_GITHUB_RUN_ID" = 731 ]
    [ "$SO_GITHUB_RUN_URL" = 'https://github.invalid/owner/repo/actions/runs/731' ]
    grep -F 'workflow run deploy-backend-admin.yml --repo owner/repo --ref main -f deployment_id=deploy-123' "$FAKE_COMMAND_LOG"
    grep -F 'run watch 731 --repo owner/repo --exit-status' "$FAKE_COMMAND_LOG"
}

@test "workflow dispatch rejects ambiguous correlated runs" {
    local duplicate="$TEST_ROOT/duplicate-runs.json"
    jq '. + [.[0]]' "$FIXTURE_DIR/github-runs.json" >"$duplicate"
    export GITHUB_RUN_FIXTURE=$duplicate

    run github_dispatch_and_wait deploy-backend-admin.yml deploy-123
    [ "$status" -eq 78 ]
}

@test "Cloudflare preflight requires strict mode and unambiguous A records" {
    SO_CF_ACCOUNT_ID=
    cf_preflight

    [ "$SO_CF_ZONE_ID" = zone-123 ]
    [ "$SO_CF_ACCOUNT_ID" = account-456 ]
    [ "$SO_CF_ADMIN_RECORD_ID" = dns-admin-1 ]
    [ "$SO_CF_SCHOOL_RECORD_ID" = dns-school-1 ]
    run grep -F 'cf-bootstrap-7vK9nM3qR8wX2zLp6tY4' "$FAKE_COMMAND_LOG"
    [ "$status" -eq 1 ]
}

@test "Cloudflare preflight rejects non-A records for either API host" {
    local invalid_dns="$TEST_ROOT/invalid-dns.json"
    jq '(.result[] | select(.name == "admin-api.schoolorbit.app") | .type) = "AAAA"' \
        "$FIXTURE_DIR/cloudflare-dns.json" >"$invalid_dns"
    export CF_DNS_FIXTURE=$invalid_dns

    run cf_preflight
    [ "$status" -eq 78 ]
}

@test "Origin CA request uses exact hosts and keeps only certificate metadata beside memory" {
    local csr="$TEST_ROOT/origin.csr"
    printf '%s\n' '-----BEGIN CERTIFICATE REQUEST-----' 'fixture-csr-data' '-----END CERTIFICATE REQUEST-----' >"$csr"
    cf_issue_origin_certificate "$csr"

    jq -e '
        .hostnames == ["admin-api.schoolorbit.app", "school-api.schoolorbit.app"] and
        .request_type == "origin-rsa" and .requested_validity == 5475
    ' "$CAPTURED_REQUEST_BODY"
    [[ $CF_CERTIFICATE == *'BEGIN CERTIFICATE'* ]]
    [ "$SO_CF_CERTIFICATE_ID" = origin-cert-123 ]
    [ "$SO_CF_CERTIFICATE_EXPIRES" = '2041-08-02T00:00:00Z' ]
}

@test "Cloudflare cutover sends one two-record proxied batch" {
    cf_preflight
    cf_snapshot_dns
    cf_apply_dns_batch cutover

    run jq -e '
        .patches | length == 2 and
        all(.[]; .content == "192.0.2.20" and .proxied == true)
    ' "$CAPTURED_REQUEST_BODY"
    [ "$status" -eq 0 ]
}

@test "Cloudflare rollback restores the complete snapshotted record fields" {
    cf_preflight
    cf_snapshot_dns
    cf_apply_dns_batch rollback

    jq -e '
        .patches | length == 2 and
        any(.[]; .id == "dns-admin-1" and .content == "198.51.100.10" and .ttl == 1 and .proxied == true) and
        any(.[]; .id == "dns-school-1" and .content == "198.51.100.10" and .ttl == 1 and .proxied == true)
    ' "$CAPTURED_REQUEST_BODY"
}

@test "Cloudflare snapshot rejects API records on different original IPs" {
    local split_dns="$TEST_ROOT/split-dns.json"
    jq '(.result[] | select(.name == "school-api.schoolorbit.app") | .content) = "198.51.100.11"' \
        "$FIXTURE_DIR/cloudflare-dns.json" >"$split_dns"
    export CF_DNS_FIXTURE=$split_dns
    cf_preflight

    run cf_snapshot_dns
    [ "$status" -eq 78 ]
}

@test "DNS drift blocks cutover" {
    SO_DNS_SNAPSHOT_ETAG=original
    SO_DNS_CURRENT_ETAG=changed

    run cf_assert_no_dns_drift
    [ "$status" -eq 78 ]
}

@test "Cloudflare record polling requires both API records to reach the target" {
    local cutover_dns="$TEST_ROOT/cutover-dns.json"
    jq '(.result[].content) = "192.0.2.20"' "$FIXTURE_DIR/cloudflare-dns.json" >"$cutover_dns"
    export CF_DNS_FIXTURE=$cutover_dns
    cf_preflight

    cf_wait_for_record_content 192.0.2.20
}

@test "cutover revalidation compares record identity target proxy and TTL" {
    local cutover_dns="$TEST_ROOT/cutover-state.json"
    cf_preflight
    cf_snapshot_dns
    jq '(.result[].content) = "192.0.2.20"' "$FIXTURE_DIR/cloudflare-dns.json" >"$cutover_dns"
    export CF_DNS_FIXTURE=$cutover_dns

    cf_assert_cutover_state

    jq '(.result[] | select(.name == "school-api.schoolorbit.app") | .ttl) = 300' \
        "$cutover_dns" >"$TEST_ROOT/cutover-drift.json"
    export CF_DNS_FIXTURE="$TEST_ROOT/cutover-drift.json"
    run cf_assert_cutover_state
    [ "$status" -eq 78 ]
}

@test "proxied DNS polling accepts Cloudflare addresses and rejects the origin address" {
    make_fake_command getent 'printf "%s %s\n" "203.0.113.41" "STREAM fake"'
    cf_wait_for_proxy_resolution 192.0.2.20

    make_fake_command getent 'printf "%s %s\n" "192.0.2.20" "STREAM fake"'
    run cf_wait_for_proxy_resolution 192.0.2.20
    [ "$status" -eq 75 ]
}

@test "provider failure fixture performs one poll without retry backoff" {
    make_fake_command getent '
printf "%s %s\n" "192.0.2.20" "STREAM fake"
printf "%s\n" getent >>"$FAKE_COMMAND_LOG"
'
    sleep() {
        printf 'sleep %s\n' "$*" >>"$FAKE_COMMAND_LOG"
    }

    run cf_wait_for_proxy_resolution 192.0.2.20

    [ "$status" -eq 75 ]
    [ "$(grep -c '^getent$' "$FAKE_COMMAND_LOG")" -eq 1 ]
    ! grep -q '^sleep ' "$FAKE_COMMAND_LOG"
}
