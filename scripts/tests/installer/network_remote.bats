#!/usr/bin/env bats

load test_helper
bats_require_minimum_version 1.5.0

setup() {
    setup_installer_test
    export NETWORK_SCRIPT="$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/remote/ensure_container_network.sh"
    export PODMAN_NETWORK_STATE="$TEST_ROOT/podman-network-state"
    export PODMAN_DNS_COUNT="$TEST_ROOT/podman-dns-count"
    export PODMAN_DNS_FAILURES=0
    export PODMAN_PROBE_AVAILABLE=true
    export PODMAN_CONTAINER_EXISTS=true
    export PODMAN_INSPECT_FAIL=false
    printf '%s\n' attached-good >"$PODMAN_NETWORK_STATE"
    printf '%s\n' 0 >"$PODMAN_DNS_COUNT"
    # shellcheck disable=SC2016
    make_fake_command podman '
set -eu
printf "podman" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"
case "$1" in
    container) [ "$PODMAN_CONTAINER_EXISTS" = true ] ;;
    inspect)
        [ "$PODMAN_INSPECT_FAIL" = false ] || exit 1
        case "$(cat "$PODMAN_NETWORK_STATE")" in
            detached) printf "%s\n" "{}" ;;
            attached-good | attached-stale)
                printf "%s\n" "{\"schoolorbit-web\":{\"IPAddress\":\"10.89.0.42\",\"Aliases\":[\"backend-admin\",\"schoolorbit-backend-admin\"]}}" ;;
            attached-no-alias)
                printf "%s\n" "{\"schoolorbit-web\":{\"IPAddress\":\"10.89.0.42\",\"Aliases\":[\"backend-admin\"]}}" ;;
        esac ;;
    exec)
        [ "$PODMAN_PROBE_AVAILABLE" = true ] || exit 1
        if [ "$3" = true ]; then exit 0; fi
        count=$(cat "$PODMAN_DNS_COUNT")
        count=$((count + 1))
        printf "%s\n" "$count" >"$PODMAN_DNS_COUNT"
        [ "$count" -gt "$PODMAN_DNS_FAILURES" ] || exit 1
        if [ "$(cat "$PODMAN_NETWORK_STATE")" = attached-stale ]; then
            printf "10.89.0.7 %s\n" "$5"
        else
            printf "10.89.0.42 %s\n" "$5"
        fi ;;
    network) exit 0 ;;
    *) exit 99 ;;
esac
'
    make_fake_command sleep ':'
}

teardown() {
    teardown_installer_test
}

run_network_validation() {
    # shellcheck disable=SC2016
    run env PATH="$FAKE_BIN:$ORIGINAL_PATH" bash -c '
        source "$1"
        schoolorbit_validate_container_network_aliases \
            schoolorbit-web schoolorbit-backend-admin backend-admin \
            schoolorbit-backend-admin schoolorbit-nginx
    ' _ "$NETWORK_SCRIPT"
}

assert_network_untouched() {
    ! grep -Eq 'podman (network|start|stop|restart|rm)' "$FAKE_COMMAND_LOG"
}

@test "network validation preserves an already-correct attachment and checks both aliases" {
    run_network_validation
    [ "$status" -eq 0 ]
    grep -Fq 'getent hosts backend-admin' "$FAKE_COMMAND_LOG"
    grep -Fq 'getent hosts schoolorbit-backend-admin' "$FAKE_COMMAND_LOG"
    assert_network_untouched
}

@test "network validation refuses missing aliases without interrupting live connections" {
    printf '%s\n' attached-no-alias >"$PODMAN_NETWORK_STATE"
    run_network_validation
    [ "$status" -ne 0 ]
    assert_network_untouched
}

@test "network validation refuses a detached container without connecting it after startup" {
    printf '%s\n' detached >"$PODMAN_NETWORK_STATE"
    run_network_validation
    [ "$status" -ne 0 ]
    assert_network_untouched
}

@test "network validation waits for DNS propagation without changing attachment" {
    PODMAN_DNS_FAILURES=2
    run_network_validation
    [ "$status" -eq 0 ]
    [ "$(<"$PODMAN_DNS_COUNT")" -eq 4 ]
    assert_network_untouched
}

@test "network validation rejects DNS pointing at the old IP" {
    printf '%s\n' attached-stale >"$PODMAN_NETWORK_STATE"
    run_network_validation
    [ "$status" -ne 0 ]
    [ "$(<"$PODMAN_DNS_COUNT")" -eq 5 ]
    assert_network_untouched
}

@test "network validation fails closed when its probe is unavailable" {
    PODMAN_PROBE_AVAILABLE=false
    run_network_validation
    [ "$status" -ne 0 ]
    assert_network_untouched
}

@test "network validation fails closed when inspect fails" {
    PODMAN_INSPECT_FAIL=true
    run_network_validation
    [ "$status" -ne 0 ]
    assert_network_untouched
}

@test "network validation refuses a missing container" {
    PODMAN_CONTAINER_EXISTS=false
    run_network_validation
    [ "$status" -ne 0 ]
    assert_network_untouched
}
