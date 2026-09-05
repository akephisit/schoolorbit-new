#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
    export MATCHER="$BATS_TEST_DIRNAME/../../clamd_runtime_matches.sh"
    seed_matching_runtime

    make_fake_command podman '
printf "%s\n" "$*" >>"$FAKE_COMMAND_LOG"

if [[ ${1:-} == container && ${2:-} == exists ]]; then
    [[ ${CLAMD_CONTAINER_EXISTS:-1} == 1 ]]
    exit
fi

if [[ ${1:-} == image && ${2:-} == inspect ]]; then
    [[ ${CLAMD_IMAGE_INSPECT_OK:-1} == 1 ]] || exit 125
    printf "%s\n" "${CLAMD_DESIRED_IMAGE_ID:?}"
    exit
fi

if [[ ${1:-} != inspect || ${2:-} != --format ]]; then
    exit 64
fi

case ${3:-} in
    "{{.Image}}") printf "%s\n" "${CLAMD_RUNNING_IMAGE_ID:?}" ;;
    "{{.HostConfig.Memory}}") printf "%s\n" "${CLAMD_MEMORY:?}" ;;
    "{{.HostConfig.NanoCpus}}") printf "%s\n" "${CLAMD_NANO_CPUS:?}" ;;
    "{{.HostConfig.PidsLimit}}") printf "%s\n" "${CLAMD_PIDS_LIMIT:?}" ;;
    "{{.HostConfig.RestartPolicy.Name}}") printf "%s\n" "${CLAMD_RESTART_POLICY:?}" ;;
    "{{json .HostConfig.SecurityOpt}}") printf "%s\n" "${CLAMD_SECURITY_OPT:?}" ;;
    "{{json .HostConfig.PortBindings}}") printf "%s\n" "${CLAMD_PORT_BINDINGS:?}" ;;
    "{{json .Mounts}}") printf "%s\n" "${CLAMD_MOUNTS:?}" ;;
    "{{json .NetworkSettings.Networks}}") printf "%s\n" "${CLAMD_NETWORKS:?}" ;;
    "{{.State.Running}}") printf "%s\n" "${CLAMD_RUNNING:?}" ;;
    "{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}")
        printf "%s\n" "${CLAMD_HEALTH:?}"
        ;;
    *) exit 65 ;;
esac
'
    make_fake_command jq '
input=$(cat)
case "$*" in
    *no-new-privileges*)
        [[ $input == *\"no-new-privileges\"* || $input == *\"no-new-privileges:true\"* ]]
        ;;
    *schoolorbit-clamav-signatures*)
        [[ $input == *\"Type\":\"volume\"* ]]
        [[ $input == *\"Name\":\"schoolorbit-clamav-signatures\"* ]]
        [[ $input == *\"Destination\":\"/var/lib/clamav\"* ]]
        ;;
    *schoolorbit-file-platform-internal*)
        [[ $input == *\"schoolorbit-file-platform-internal\"* ]]
        [[ $input == *\"schoolorbit-clamav-egress\"* ]]
        ;;
    *)
        [[ $input == null || $input == "{}" ]]
        ;;
esac
'
}

teardown() {
    teardown_installer_test
}

seed_matching_runtime() {
    export CLAMD_CONTAINER_EXISTS=1
    export CLAMD_IMAGE_INSPECT_OK=1
    export CLAMD_DESIRED_IMAGE_ID=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    export CLAMD_RUNNING_IMAGE_ID=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    export CLAMD_MEMORY=3221225472
    export CLAMD_NANO_CPUS=1000000000
    export CLAMD_PIDS_LIMIT=256
    export CLAMD_RESTART_POLICY=unless-stopped
    export CLAMD_SECURITY_OPT='["no-new-privileges:true"]'
    export CLAMD_PORT_BINDINGS='{}'
    export CLAMD_MOUNTS='[{"Type":"volume","Name":"schoolorbit-clamav-signatures","Destination":"/var/lib/clamav"}]'
    export CLAMD_NETWORKS='{"schoolorbit-file-platform-internal":{},"schoolorbit-clamav-egress":{}}'
    export CLAMD_RUNNING=true
    export CLAMD_HEALTH=healthy
}

run_matcher() {
    run env \
        PATH="$FAKE_BIN:$ORIGINAL_PATH" \
        FAKE_COMMAND_LOG="$FAKE_COMMAND_LOG" \
        CLAMD_CONTAINER_EXISTS="$CLAMD_CONTAINER_EXISTS" \
        CLAMD_IMAGE_INSPECT_OK="$CLAMD_IMAGE_INSPECT_OK" \
        CLAMD_DESIRED_IMAGE_ID="$CLAMD_DESIRED_IMAGE_ID" \
        CLAMD_RUNNING_IMAGE_ID="$CLAMD_RUNNING_IMAGE_ID" \
        CLAMD_MEMORY="$CLAMD_MEMORY" \
        CLAMD_NANO_CPUS="$CLAMD_NANO_CPUS" \
        CLAMD_PIDS_LIMIT="$CLAMD_PIDS_LIMIT" \
        CLAMD_RESTART_POLICY="$CLAMD_RESTART_POLICY" \
        CLAMD_SECURITY_OPT="$CLAMD_SECURITY_OPT" \
        CLAMD_PORT_BINDINGS="$CLAMD_PORT_BINDINGS" \
        CLAMD_MOUNTS="$CLAMD_MOUNTS" \
        CLAMD_NETWORKS="$CLAMD_NETWORKS" \
        CLAMD_RUNNING="$CLAMD_RUNNING" \
        CLAMD_HEALTH="$CLAMD_HEALTH" \
        "$MATCHER" docker.io/clamav/clamav-debian:1.5.3 schoolorbit-clamd
}

@test "clamd matcher accepts the exact healthy runtime without mutation" {
    run_matcher

    [ "$status" -eq 0 ]
    [ "$output" = 'clamd_action=reused' ]
    ! grep -Eq '(^| )(stop|rm|run|create)( |$)' "$FAKE_COMMAND_LOG"
}

@test "clamd matcher reports every relevant runtime drift without mutation" {
    local drift_case variable value expected
    while IFS='|' read -r drift_case variable value expected; do
        seed_matching_runtime
        export "$variable=$value"

        run_matcher

        [ "$status" -eq 1 ]
        [ "$output" = "clamd_drift reason=$expected" ]
    done <<'EOF'
missing|CLAMD_CONTAINER_EXISTS|0|missing_container
image|CLAMD_RUNNING_IMAGE_ID|bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb|image
memory|CLAMD_MEMORY|2147483648|memory
cpu|CLAMD_NANO_CPUS|500000000|cpu
pids|CLAMD_PIDS_LIMIT|128|pids
restart|CLAMD_RESTART_POLICY|always|restart
security|CLAMD_SECURITY_OPT|[]|security
port|CLAMD_PORT_BINDINGS|{"3310/tcp":[{"HostIp":"0.0.0.0","HostPort":"3310"}]}|published_port
volume|CLAMD_MOUNTS|[]|signature_volume
network|CLAMD_NETWORKS|{"schoolorbit-file-platform-internal":{}}|network
running|CLAMD_RUNNING|false|running
health|CLAMD_HEALTH|unhealthy|health
EOF
    ! grep -Eq '(^| )(stop|rm|run|create)( |$)' "$FAKE_COMMAND_LOG"
}

@test "clamd matcher accepts the semantic no-new-privileges spelling and null ports" {
    CLAMD_SECURITY_OPT='["no-new-privileges"]'
    CLAMD_PORT_BINDINGS=null

    run_matcher

    [ "$status" -eq 0 ]
    [ "$output" = 'clamd_action=reused' ]
}

@test "clamd matcher fails closed when the pinned image cannot be inspected" {
    CLAMD_IMAGE_INSPECT_OK=0

    run_matcher

    [ "$status" -eq 1 ]
    [ "$output" = 'clamd_drift reason=image_inspect' ]
    ! grep -Eq '(^| )(stop|rm|run|create)( |$)' "$FAKE_COMMAND_LOG"
}

@test "clamd matcher rejects arguments outside the canonical runtime" {
    run "$MATCHER" docker.io/clamav/clamav:latest schoolorbit-clamd

    [ "$status" -eq 64 ]
    [ "$output" = 'Unsupported ClamAV runtime target' ]
    [ ! -s "$FAKE_COMMAND_LOG" ]
}
