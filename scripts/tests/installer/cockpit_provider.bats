#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
    export FIXTURE_DIR="$BATS_TEST_DIRNAME/fixtures"
    export CF_COCKPIT_DNS_FIXTURE="$FIXTURE_DIR/cloudflare-cockpit-dns-none.json"
    export CF_COCKPIT_TUNNELS_FIXTURE="$FIXTURE_DIR/cloudflare-cockpit-tunnels-none.json"
    export CF_COCKPIT_CONNECTIONS_FIXTURE="$FIXTURE_DIR/cloudflare-cockpit-connections.json"
    export TMPDIR="$TEST_ROOT/tmp"
    mkdir -p "$TMPDIR"

    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/cloudflare.sh"
    if [[ -f $BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/cloudflare_tunnel.sh ]]; then
        source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/cloudflare_tunnel.sh"
    fi

    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=schoolorbit.app
    SO_CF_ZONE_ID=zone-123
    SO_CF_ACCOUNT_ID=account-456
    SO_RUN_ID=cockpit-run-1
    SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]=cf-bootstrap-7vK9nM3qR8wX2zLp6tY4

    make_fake_command curl '
set -eu
method=GET
output=
request=
url=
printf "curl" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --request) method=$2; shift 2 ;;
        --output) output=$2; shift 2 ;;
        --data-binary) request=${2#@}; shift 2 ;;
        http://*|https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
[ -n "$request" ] && cp "$request" "$CAPTURED_REQUEST_BODY"
case "$method $url" in
    "GET "*/dns_records*) cp "$CF_COCKPIT_DNS_FIXTURE" "$output" ;;
    "GET "*/cfd_tunnel\?*) cp "$CF_COCKPIT_TUNNELS_FIXTURE" "$output" ;;
    "POST "*/cfd_tunnel) cp "$FIXTURE_DIR/cloudflare-cockpit-tunnel-created.json" "$output" ;;
    "PUT "*/configurations) printf "%s\n" "{\"success\":true,\"result\":{}}" >"$output" ;;
    "GET "*/token) cp "$FIXTURE_DIR/cloudflare-cockpit-token.json" "$output" ;;
    "GET "*/connections) cp "$CF_COCKPIT_CONNECTIONS_FIXTURE" "$output" ;;
    "POST "*/dns_records) printf "%s\n" "{\"success\":true,\"result\":{\"id\":\"cockpit-record-1\"}}" >"$output" ;;
    "PATCH "*/dns_records/*) printf "%s\n" "{\"success\":true,\"result\":{}}" >"$output" ;;
    "DELETE "*/dns_records/*) printf "%s\n" "{\"success\":true,\"result\":{}}" >"$output" ;;
    *) printf "%s\n" "{\"success\":false,\"errors\":[{\"code\":7000,\"message\":\"unknown fake endpoint\"}]}" >"$output" ;;
esac
'
}

teardown() {
    teardown_installer_test
}

@test "cockpit preflight accepts zero or one exact proxied CNAME" {
    cf_cockpit_preflight
    [ "$SO_CF_COCKPIT_HOSTNAME" = server.schoolorbit.app ]
    [ "$SO_CF_COCKPIT_RECORD_EXISTED" = false ]

    export CF_COCKPIT_DNS_FIXTURE="$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json"
    cf_cockpit_preflight
    [ "$SO_CF_COCKPIT_RECORD_EXISTED" = true ]
    [ "$SO_CF_COCKPIT_RECORD_ID" = cockpit-record-old ]
}

@test "cockpit preflight rejects wrong record types and duplicate names" {
    local invalid="$TEST_ROOT/invalid-cockpit-dns.json"
    jq '(.result[0].type) = "A"' "$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json" >"$invalid"
    export CF_COCKPIT_DNS_FIXTURE=$invalid
    run cf_cockpit_preflight
    [ "$status" -eq 78 ]

    jq '.result += [.result[0] | .id = "cockpit-record-duplicate"]' \
        "$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json" >"$invalid"
    run cf_cockpit_preflight
    [ "$status" -eq 78 ]
}

@test "tunnel provisioning sends exact Cockpit ingress and 404 catch-all" {
    cf_cockpit_preflight
    cf_cockpit_provision_tunnel

    [ "$SO_CF_COCKPIT_TUNNEL_ID" = 11111111-1111-4111-8111-111111111111 ]
    jq -e '.config.ingress == [
      {"hostname":"server.schoolorbit.app","service":"http://127.0.0.1:9090","originRequest":{}},
      {"service":"http_status:404"}
    ]' "$CAPTURED_REQUEST_BODY"
}

@test "tunnel token is memory-only and redacted from command logs" {
    cf_cockpit_preflight
    cf_cockpit_provision_tunnel
    cf_cockpit_get_token

    [ "${SO_SECRETS[SCHOOLORBIT_COCKPIT_TUNNEL_TOKEN]}" = eyJhIjoiYWNjb3VudC00NTYiLCJ0IjoiY29ja3BpdC10dW5uZWwtdG9rZW4ifQ ]
    run grep -R -F 'eyJhIjoiYWNjb3VudC00NTYiLCJ0IjoiY29ja3BpdC10dW5uZWwtdG9rZW4ifQ' "$FAKE_COMMAND_LOG" "$TMPDIR"
    [ "$status" -eq 1 ]
}

@test "connector polling requires an active connection from the target IP" {
    SO_CF_COCKPIT_TUNNEL_ID=11111111-1111-4111-8111-111111111111
    cf_cockpit_wait_connector

    local wrong="$TEST_ROOT/wrong-connector.json"
    jq '(.result[0].conns[0].origin_ip) = "198.51.100.30"' \
        "$FIXTURE_DIR/cloudflare-cockpit-connections.json" >"$wrong"
    export CF_COCKPIT_CONNECTIONS_FIXTURE=$wrong
    run cf_cockpit_wait_connector
    [ "$status" -eq 75 ]
}

@test "DNS drift blocks management publication" {
    export CF_COCKPIT_DNS_FIXTURE="$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json"
    cf_cockpit_preflight
    cf_cockpit_snapshot
    SO_CF_COCKPIT_TUNNEL_ID=11111111-1111-4111-8111-111111111111

    local drift="$TEST_ROOT/drifted-cockpit-dns.json"
    jq '(.result[0].content) = "operator-change.cfargotunnel.com"' \
        "$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json" >"$drift"
    export CF_COCKPIT_DNS_FIXTURE=$drift

    run cf_cockpit_publish
    [ "$status" -eq 78 ]
    ! grep -Eq 'PATCH .*/dns_records/|POST .*/dns_records($| )' "$FAKE_COMMAND_LOG"
}

@test "management publication is idempotent after a journaled cutover" {
    export CF_COCKPIT_DNS_FIXTURE="$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json"
    cf_cockpit_preflight
    cf_cockpit_snapshot
    SO_CF_COCKPIT_TUNNEL_ID=11111111-1111-4111-8111-111111111111
    cf_cockpit_publish

    local published="$TEST_ROOT/idempotent-published-cockpit-dns.json"
    jq '(.result[0].content) = "11111111-1111-4111-8111-111111111111.cfargotunnel.com"' \
        "$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json" >"$published"
    export CF_COCKPIT_DNS_FIXTURE=$published

    cf_cockpit_publish

    [ "$(grep -Ec -- '--request PATCH .*dns_records/cockpit-record-old' "$FAKE_COMMAND_LOG")" -eq 1 ]
}

@test "new CNAME publication is recovered if the process stopped before journaling" {
    cf_cockpit_preflight
    cf_cockpit_snapshot
    SO_CF_COCKPIT_TUNNEL_ID=11111111-1111-4111-8111-111111111111

    local published="$TEST_ROOT/uncheckpointed-published-cockpit-dns.json"
    jq --arg id cockpit-record-recovered --arg target '11111111-1111-4111-8111-111111111111.cfargotunnel.com' '
      .result = [{id:$id,type:"CNAME",name:"server.schoolorbit.app",content:$target,ttl:1,proxied:true,comment:"SchoolOrbit Cockpit Cloudflare Tunnel",tags:["schoolorbit:management"],settings:{},modified_on:"2026-08-04T01:00:00Z"}]
    ' "$FIXTURE_DIR/cloudflare-cockpit-dns-none.json" >"$published"
    export CF_COCKPIT_DNS_FIXTURE=$published

    cf_cockpit_publish

    [ "$SO_CF_COCKPIT_RECORD_ID" = cockpit-record-recovered ]
    ! grep -Eq -- '--request POST .*zones/zone-123/dns_records($| )' "$FAKE_COMMAND_LOG"
}

@test "new CNAME rollback deletes only the run-owned record and retains the Tunnel" {
    cf_cockpit_preflight
    cf_cockpit_snapshot
    SO_CF_COCKPIT_TUNNEL_ID=11111111-1111-4111-8111-111111111111
    cf_cockpit_publish

    local published="$TEST_ROOT/published-cockpit-dns.json"
    jq --arg id cockpit-record-1 --arg target '11111111-1111-4111-8111-111111111111.cfargotunnel.com' '
      .result = [{id:$id,type:"CNAME",name:"server.schoolorbit.app",content:$target,ttl:1,proxied:true,comment:"SchoolOrbit Cockpit Cloudflare Tunnel",tags:["schoolorbit:management"],settings:{},modified_on:"2026-08-04T01:00:00Z"}]
    ' "$FIXTURE_DIR/cloudflare-cockpit-dns-none.json" >"$published"
    export CF_COCKPIT_DNS_FIXTURE=$published

    cf_cockpit_restore_dns

    grep -Eq -- '--request DELETE .*zones/zone-123/dns_records/cockpit-record-1' "$FAKE_COMMAND_LOG"
    ! grep -Eq 'DELETE .*/cfd_tunnel' "$FAKE_COMMAND_LOG"
}

@test "existing CNAME rollback restores its exact snapshotted fields" {
    export CF_COCKPIT_DNS_FIXTURE="$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json"
    cf_cockpit_preflight
    cf_cockpit_snapshot
    SO_CF_COCKPIT_TUNNEL_ID=11111111-1111-4111-8111-111111111111
    cf_cockpit_publish

    local published="$TEST_ROOT/published-existing-cockpit-dns.json"
    jq '(.result[0].content) = "11111111-1111-4111-8111-111111111111.cfargotunnel.com"' \
        "$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json" >"$published"
    export CF_COCKPIT_DNS_FIXTURE=$published
    cf_cockpit_restore_dns

    jq -e '. == {
      type:"CNAME",name:"server.schoolorbit.app",content:"old-cockpit.cfargotunnel.com",
      ttl:1,proxied:true,comment:"operator-managed",tags:["schoolorbit:management"],settings:{}
    }' "$CAPTURED_REQUEST_BODY"
}

@test "rollback refuses to overwrite post-publication CNAME drift" {
    export CF_COCKPIT_DNS_FIXTURE="$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json"
    cf_cockpit_preflight
    cf_cockpit_snapshot
    SO_CF_COCKPIT_TUNNEL_ID=11111111-1111-4111-8111-111111111111
    cf_cockpit_publish

    local drifted="$TEST_ROOT/drifted-published-cockpit-dns.json"
    jq '
      (.result[0].content) = "11111111-1111-4111-8111-111111111111.cfargotunnel.com" |
      (.result[0].comment) = "operator changed this after cutover"
    ' "$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json" >"$drifted"
    export CF_COCKPIT_DNS_FIXTURE=$drifted

    run cf_cockpit_restore_dns
    [ "$status" -eq 78 ]
}

@test "checkpoint restore validates hostname record ownership and Tunnel identity" {
    cf_cockpit_restore_checkpoint \
        server.schoolorbit.app \
        '{"id":"cockpit-record-old","type":"CNAME","name":"server.schoolorbit.app","content":"old-cockpit.cfargotunnel.com","ttl":1,"proxied":true,"comment":"","tags":[],"settings":{},"modified_on":"2026-08-03T10:00:00Z"}' \
        cockpit-record-old true \
        11111111-1111-4111-8111-111111111111 \
        schoolorbit-cockpit-cockpit-run-1

    [ "$SO_CF_COCKPIT_RECORD_ID" = cockpit-record-old ]
    [ "$SO_CF_COCKPIT_TUNNEL_ID" = 11111111-1111-4111-8111-111111111111 ]

    run cf_cockpit_restore_checkpoint \
        attacker.example.test null '' false \
        11111111-1111-4111-8111-111111111111 \
        schoolorbit-cockpit-cockpit-run-1
    [ "$status" -eq 78 ]
}

@test "checkpoint restore accepts a pre-provision snapshot without Tunnel metadata" {
    cf_cockpit_restore_checkpoint \
        server.schoolorbit.app null '' false '' ''

    [ "$SO_CF_COCKPIT_SNAPSHOT_READY" = true ]
    [ "$SO_CF_COCKPIT_DNS_SNAPSHOT" = null ]
    [ -z "$SO_CF_COCKPIT_TUNNEL_ID" ]
}

@test "published-state verification requires the exact run-owned CNAME" {
    local published="$TEST_ROOT/published-state.json"
    jq '
      (.result[0].content) = "11111111-1111-4111-8111-111111111111.cfargotunnel.com"
    ' "$FIXTURE_DIR/cloudflare-cockpit-dns-existing.json" >"$published"
    export CF_COCKPIT_DNS_FIXTURE=$published
    cf_cockpit_restore_checkpoint \
        server.schoolorbit.app \
        '{"id":"cockpit-record-old","type":"CNAME","name":"server.schoolorbit.app","content":"old-cockpit.cfargotunnel.com","ttl":1,"proxied":true,"comment":"operator-managed","tags":["schoolorbit:management"],"settings":{},"modified_on":"2026-08-03T10:00:00Z"}' \
        cockpit-record-old true \
        11111111-1111-4111-8111-111111111111 \
        schoolorbit-cockpit-cockpit-run-1

    cf_cockpit_assert_published_state

    jq '(.result[0].ttl) = 300' "$published" >"$TEST_ROOT/published-state-drift.json"
    export CF_COCKPIT_DNS_FIXTURE="$TEST_ROOT/published-state-drift.json"
    run cf_cockpit_assert_published_state
    [ "$status" -eq 78 ]
}
