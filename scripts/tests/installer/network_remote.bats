#!/usr/bin/env bats

load test_helper
bats_require_minimum_version 1.5.0

setup() {
    setup_installer_test
    export NETWORK_SCRIPT="$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/remote/ensure_container_network.sh"
    export PODMAN_NETWORK_STATE="$TEST_ROOT/podman-network-state"
    export PODMAN_ALIAS_CONNECT_COUNT="$TEST_ROOT/podman-alias-connect-count"
    export PODMAN_ALIAS_CONNECT_FAILURES=0
    printf '%s\n' detached >"$PODMAN_NETWORK_STATE"
    printf '%s\n' 0 >"$PODMAN_ALIAS_CONNECT_COUNT"

    # shellcheck disable=SC2016
    make_fake_command podman '
set -eu
printf "podman" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"

case "$1" in
    container)
        [ "$2" = exists ]
        exit 0
        ;;
    inspect)
        case "$(cat "$PODMAN_NETWORK_STATE")" in
            detached) printf "%s\n" "{}" ;;
            attached-good)
                printf "%s\n" "{\"schoolorbit-web\":{\"Aliases\":[\"backend-admin\",\"schoolorbit-backend-admin\"]}}"
                ;;
            attached-no-alias)
                printf "%s\n" "{\"schoolorbit-web\":{\"Aliases\":[\"schoolorbit-backend-admin\"]}}"
                ;;
            attached-plain)
                printf "%s\n" "{\"schoolorbit-web\":{\"Aliases\":[]}}"
                ;;
        esac
        ;;
    network)
        case "$2" in
            disconnect)
                printf "%s\n" detached >"$PODMAN_NETWORK_STATE"
                ;;
            connect)
                case " $* " in
                    *" --alias "*)
                        count=$(cat "$PODMAN_ALIAS_CONNECT_COUNT")
                        count=$((count + 1))
                        printf "%s\n" "$count" >"$PODMAN_ALIAS_CONNECT_COUNT"
                        if [ "$count" -le "$PODMAN_ALIAS_CONNECT_FAILURES" ]; then
                            exit 1
                        fi
                        printf "%s\n" attached-good >"$PODMAN_NETWORK_STATE"
                        ;;
                    *) printf "%s\n" attached-plain >"$PODMAN_NETWORK_STATE" ;;
                esac
                ;;
        esac
        ;;
esac
'
    make_fake_command sleep ':'
}

teardown() {
    teardown_installer_test
}

run_network_repair() {
    # shellcheck disable=SC2016
    run env \
        PATH="$FAKE_BIN:$ORIGINAL_PATH" \
        FAKE_COMMAND_LOG="$FAKE_COMMAND_LOG" \
        PODMAN_NETWORK_STATE="$PODMAN_NETWORK_STATE" \
        PODMAN_ALIAS_CONNECT_COUNT="$PODMAN_ALIAS_CONNECT_COUNT" \
        PODMAN_ALIAS_CONNECT_FAILURES="$PODMAN_ALIAS_CONNECT_FAILURES" \
        bash -c '
          source "$1"
          schoolorbit_ensure_container_network_aliases \
            schoolorbit-web schoolorbit-backend-admin backend-admin schoolorbit-backend-admin
        ' _ "$NETWORK_SCRIPT"
}

@test "network alias repair leaves an already-correct attachment unchanged" {
    printf '%s\n' attached-good >"$PODMAN_NETWORK_STATE"

    run_network_repair

    [ "$status" -eq 0 ]
    run ! grep -Fq 'podman network disconnect' "$FAKE_COMMAND_LOG"
    run ! grep -Fq 'podman network connect' "$FAKE_COMMAND_LOG"
}

@test "network alias repair retries a detached container with bounded alias connects" {
    PODMAN_ALIAS_CONNECT_FAILURES=2

    run_network_repair

    [ "$status" -eq 0 ]
    [ "$(<"$PODMAN_ALIAS_CONNECT_COUNT")" -eq 3 ]
    [ "$(<"$PODMAN_NETWORK_STATE")" = attached-good ]
    run ! grep -Fq 'podman network disconnect' "$FAKE_COMMAND_LOG"
}

@test "network alias repair restores prior membership before reporting alias failure" {
    printf '%s\n' attached-no-alias >"$PODMAN_NETWORK_STATE"
    PODMAN_ALIAS_CONNECT_FAILURES=5

    run_network_repair

    [ "$status" -ne 0 ]
    [ "$(<"$PODMAN_ALIAS_CONNECT_COUNT")" -eq 5 ]
    [ "$(<"$PODMAN_NETWORK_STATE")" = attached-plain ]
    [ "$(grep -c 'podman network disconnect' "$FAKE_COMMAND_LOG")" -eq 1 ]
    grep -Fq 'podman network connect schoolorbit-web schoolorbit-backend-admin' "$FAKE_COMMAND_LOG"
}
