#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
    export ORIGIN_ROUTING_SCRIPT="$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/configure_pre_cutover_origin.sh"
    export GITHUB_OUTPUT="$TEST_ROOT/github-output"
    make_fake_command sudo '
printf "sudo" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"
cat >"$CAPTURED_STDIN"
'
}

teardown() {
    teardown_installer_test
}

@test "pre-cutover routing pins both API hostnames without replacing public CA trust" {
    bash "$ORIGIN_ROUTING_SCRIPT" 192.0.2.20 example.test

    [ "$(<"$CAPTURED_STDIN")" = '192.0.2.20 admin-api.example.test school-api.example.test' ]
    [ ! -e "$GITHUB_OUTPUT" ]
    grep -F 'sudo tee -a /etc/hosts' "$FAKE_COMMAND_LOG"
}

@test "pre-cutover routing rejects an invalid target before changing runner state" {
    run bash "$ORIGIN_ROUTING_SCRIPT" '192.0.2.20 --bad' example.test

    [ "$status" -eq 64 ]
    [ ! -e "$GITHUB_OUTPUT" ]
    [ ! -s "$CAPTURED_STDIN" ]
}
