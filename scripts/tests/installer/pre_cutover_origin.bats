#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
    export ORIGIN_ROUTING_SCRIPT="$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/configure_pre_cutover_origin.sh"
    export GITHUB_OUTPUT="$TEST_ROOT/github-output"
    export ORIGIN_CA_ROOT="$TEST_ROOT/cloudflare-origin-rsa-root.pem"

    make_fake_command curl '
set -eu
output=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) output=$2; shift 2 ;;
        *) shift ;;
    esac
done
printf "%s\n" fixture-origin-root >"$output"
'
    make_fake_command sha256sum '
printf "%s  %s\n" "91a8a5567efa6bf941162aa806b3ba476aaddf7867640e53053b35fb225a5dae" "$1"
'
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

@test "pre-cutover routing pins both API hostnames and publishes the verified CA path" {
    bash "$ORIGIN_ROUTING_SCRIPT" 192.0.2.20 example.test "$ORIGIN_CA_ROOT"

    [ "$(<"$CAPTURED_STDIN")" = '192.0.2.20 admin-api.example.test school-api.example.test' ]
    [ "$(<"$GITHUB_OUTPUT")" = "origin_ca_root=$ORIGIN_CA_ROOT" ]
    [ "$(<"$ORIGIN_CA_ROOT")" = 'fixture-origin-root' ]
    grep -F 'sudo tee -a /etc/hosts' "$FAKE_COMMAND_LOG"
}

@test "pre-cutover routing rejects an invalid target before changing runner state" {
    run bash "$ORIGIN_ROUTING_SCRIPT" '192.0.2.20 --bad' example.test "$ORIGIN_CA_ROOT"

    [ "$status" -eq 64 ]
    [ ! -e "$ORIGIN_CA_ROOT" ]
    [ ! -e "$GITHUB_OUTPUT" ]
    [ ! -s "$CAPTURED_STDIN" ]
}
