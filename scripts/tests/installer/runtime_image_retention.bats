#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
    export PODMAN_IMAGES_FILE="$TEST_ROOT/images.txt"
    export PODMAN_ACTIVE_IDS_FILE="$TEST_ROOT/active-ids.txt"
    export PODMAN_ALIAS_IDS_FILE="$TEST_ROOT/alias-ids.txt"
    : >"$PODMAN_IMAGES_FILE"
    : >"$PODMAN_ACTIVE_IDS_FILE"
    : >"$PODMAN_ALIAS_IDS_FILE"

    make_fake_command podman '
printf "%s\n" "$*" >>"$FAKE_COMMAND_LOG"
case "${1:-}" in
    images)
        [[ ${PODMAN_FAIL_IMAGES:-0} != 1 ]] || exit 70
        cat "$PODMAN_IMAGES_FILE"
        ;;
    ps)
        cat "$PODMAN_ACTIVE_IDS_FILE"
        ;;
    image)
        case "${2:-}" in
            exists)
                [[ ${PODMAN_FAIL_IMAGE_EXISTS:-0} != 1 ]] || exit 70
                reference=${3:-}
                awk -F "|" -v reference="$reference" "\$1 == reference { found = 1 } END { exit found ? 0 : 1 }" "$PODMAN_ALIAS_IDS_FILE"
                ;;
            inspect)
                reference=${5:-}
                image_id=$(awk -F "|" -v reference="$reference" "\$1 == reference { print \$2 }" "$PODMAN_ALIAS_IDS_FILE")
                [[ -n $image_id ]] || exit 125
                printf "%s\n" "$image_id"
                ;;
            rm)
                ;;
            *) exit 64 ;;
        esac
        ;;
    system)
        [[ ${2:-} == df ]] || exit 64
        printf "Images 3 1.5GB 200MB\n"
        ;;
    *) exit 64 ;;
esac
'
}

teardown() {
    teardown_installer_test
}

seed_five_releases() {
    cat >"$PODMAN_IMAGES_FILE" <<'EOF'
ghcr.io/akephisit/schoolorbit-backend-school|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
ghcr.io/akephisit/schoolorbit-backend-school|bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb|sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
ghcr.io/akephisit/schoolorbit-backend-school|cccccccccccccccccccccccccccccccccccccccc|sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
ghcr.io/akephisit/schoolorbit-backend-school|dddddddddddddddddddddddddddddddddddddddd|sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
ghcr.io/akephisit/schoolorbit-backend-school|eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee|sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
EOF
    cat >"$PODMAN_ALIAS_IDS_FILE" <<'EOF'
ghcr.io/akephisit/schoolorbit-backend-school:latest|sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
ghcr.io/akephisit/schoolorbit-backend-school:rollback|sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
EOF
}

@test "runtime image retention removes only SHA releases older than the keep count" {
    seed_five_releases

    run "$BATS_TEST_DIRNAME/../../prune_runtime_images.sh" \
        ghcr.io/akephisit/schoolorbit-backend-school 3

    [ "$status" -eq 0 ]
    [[ "$output" == *'runtime_image_cleanup repository=ghcr.io/akephisit/schoolorbit-backend-school before=5 retained=3 removed=2'* ]]
    grep -Fxq 'image rm ghcr.io/akephisit/schoolorbit-backend-school:dddddddddddddddddddddddddddddddddddddddd' "$FAKE_COMMAND_LOG"
    grep -Fxq 'image rm ghcr.io/akephisit/schoolorbit-backend-school:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee' "$FAKE_COMMAND_LOG"
    ! grep -Eq 'volume|system prune|image prune|rm --force' "$FAKE_COMMAND_LOG"
}

@test "runtime image retention protects active latest and rollback image IDs" {
    seed_five_releases
    printf '%s\n' sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd \
        >"$PODMAN_ACTIVE_IDS_FILE"
    cat >"$PODMAN_ALIAS_IDS_FILE" <<'EOF'
ghcr.io/akephisit/schoolorbit-backend-school:latest|sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
ghcr.io/akephisit/schoolorbit-backend-school:rollback|sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
EOF

    run "$BATS_TEST_DIRNAME/../../prune_runtime_images.sh" \
        ghcr.io/akephisit/schoolorbit-backend-school 1

    [ "$status" -eq 0 ]
    [[ "$output" == *'before=5 retained=4 removed=1'* ]]
    grep -Fxq 'image rm ghcr.io/akephisit/schoolorbit-backend-school:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' "$FAKE_COMMAND_LOG"
    ! grep -Fq 'image rm ghcr.io/akephisit/schoolorbit-backend-school:cccccccccccccccccccccccccccccccccccccccc' "$FAKE_COMMAND_LOG"
    ! grep -Fq 'image rm ghcr.io/akephisit/schoolorbit-backend-school:dddddddddddddddddddddddddddddddddddddddd' "$FAKE_COMMAND_LOG"
    ! grep -Fq 'image rm ghcr.io/akephisit/schoolorbit-backend-school:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee' "$FAKE_COMMAND_LOG"
}

@test "runtime image retention is idempotent when the retained set is already bounded" {
    seed_five_releases
    head -n 3 "$PODMAN_IMAGES_FILE" >"$PODMAN_IMAGES_FILE.next"
    mv "$PODMAN_IMAGES_FILE.next" "$PODMAN_IMAGES_FILE"

    run "$BATS_TEST_DIRNAME/../../prune_runtime_images.sh" \
        ghcr.io/akephisit/schoolorbit-backend-school 3

    [ "$status" -eq 0 ]
    [[ "$output" == *'before=3 retained=3 removed=0'* ]]
    ! grep -Fq 'image rm ' "$FAKE_COMMAND_LOG"
    ! grep -Fq 'image prune' "$FAKE_COMMAND_LOG"
}

@test "runtime image retention rejects repositories outside the deployment allowlist" {
    run "$BATS_TEST_DIRNAME/../../prune_runtime_images.sh" docker.io/library/postgres 3

    [ "$status" -eq 64 ]
    [[ "$output" == *'Unsupported runtime image repository'* ]]
    [ ! -s "$FAKE_COMMAND_LOG" ]
}

@test "runtime image retention rejects invalid keep counts" {
    run "$BATS_TEST_DIRNAME/../../prune_runtime_images.sh" \
        ghcr.io/akephisit/schoolorbit-backend-admin 0

    [ "$status" -eq 64 ]
    [[ "$output" == *'Retention count must be an integer from 1 through 20'* ]]
    [ ! -s "$FAKE_COMMAND_LOG" ]
}

@test "runtime image retention validates the complete image inventory before deleting" {
    seed_five_releases
    printf '%s\n' 'malformed-row' >>"$PODMAN_IMAGES_FILE"

    run "$BATS_TEST_DIRNAME/../../prune_runtime_images.sh" \
        ghcr.io/akephisit/schoolorbit-backend-school 3

    [ "$status" -eq 65 ]
    [[ "$output" == *'Runtime image inventory is malformed'* ]]
    ! grep -Fq 'image rm ' "$FAKE_COMMAND_LOG"
}

@test "runtime image retention fails before deletion when Podman cannot enumerate images" {
    export PODMAN_FAIL_IMAGES=1

    run "$BATS_TEST_DIRNAME/../../prune_runtime_images.sh" \
        ghcr.io/akephisit/schoolorbit-backend-school 3

    [ "$status" -ne 0 ]
    ! grep -Fq 'image rm ' "$FAKE_COMMAND_LOG"
}

@test "runtime image retention fails closed when an alias existence check errors" {
    seed_five_releases
    export PODMAN_FAIL_IMAGE_EXISTS=1

    run "$BATS_TEST_DIRNAME/../../prune_runtime_images.sh" \
        ghcr.io/akephisit/schoolorbit-backend-school 3

    [ "$status" -eq 69 ]
    [[ "$output" == *'Unable to resolve protected runtime images'* ]]
    ! grep -Fq 'image rm ' "$FAKE_COMMAND_LOG"
}
