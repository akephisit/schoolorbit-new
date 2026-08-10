#!/usr/bin/env bats

setup() {
    export TEST_ROOT
    TEST_ROOT=$(mktemp -d)
    export SCHOOLORBIT_INSTALLER_TEST_ROOT=$TEST_ROOT
    RUNTIME_ENV="$TEST_ROOT/opt/stack/.env"
    SCRIPT="$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/remote/provision_school_session_runtime.sh"
    mkdir -p "$TEST_ROOT/opt/stack"

    printf '%s\n' \
        "JWT_SECRET='admin-secret-that-must-stay'" \
        "OTHER_VALUE='preserved'" \
        "SCHOOL_LEGACY_JWT_SECRET='obsolete-school-secret'" \
        "BASE_DOMAIN='old.invalid'" >"$RUNTIME_ENV"
    chmod 0640 "$RUNTIME_ENV"
}

teardown() {
    rm -rf "$TEST_ROOT"
}

@test "production session provisioning requires root" {
    if ((EUID == 0)); then
        skip "root-only guard requires a non-root test process"
    fi

    run env -u SCHOOLORBIT_INSTALLER_TEST_ROOT "$SCRIPT"

    [ "$status" -eq 1 ]
    [[ "$output" == *'run this file with sudo'* ]]
}

@test "session provisioning preserves admin JWT and writes private distinct school keys" {
    run "$SCRIPT"

    [ "$status" -eq 0 ]
    [[ "$output" == *'Runtime configuration updated successfully'* ]]
    grep -Fxq "JWT_SECRET='admin-secret-that-must-stay'" "$RUNTIME_ENV"
    grep -Fxq "OTHER_VALUE='preserved'" "$RUNTIME_ENV"
    ! grep -q '^SCHOOL_LEGACY_JWT_SECRET=' "$RUNTIME_ENV"
    grep -Fxq "BASE_DOMAIN='schoolorbit.app'" "$RUNTIME_ENV"
    grep -Fxq "TRUSTED_PROXY_CIDRS='10.0.0.0/8,172.16.0.0/12'" "$RUNTIME_ENV"
    grep -Fxq "SCHOOL_ALLOWED_DEV_ORIGINS=''" "$RUNTIME_ENV"
    [ "$(stat -c '%a' "$RUNTIME_ENV")" = 600 ]

    local session_line rollback_line session_value rollback_value
    session_line=$(grep -m1 '^SESSION_HMAC_KEY=' "$RUNTIME_ENV")
    rollback_line=$(grep -m1 '^SCHOOL_ROLLBACK_JWT_SECRET=' "$RUNTIME_ENV")
    session_value=${session_line#*=\'}
    session_value=${session_value%\'}
    rollback_value=${rollback_line#*=\'}
    rollback_value=${rollback_value%\'}

    [[ $session_value =~ ^[0-9a-f]{64}$ ]]
    [[ $rollback_value =~ ^[0-9a-f]{64}$ ]]
    [ "$session_value" != "$rollback_value" ]
    [[ "$output" != *"$session_value"* ]]
    [[ "$output" != *"$rollback_value"* ]]

    local -a backups=("$RUNTIME_ENV".before-session-*)
    [ "${#backups[@]}" -eq 1 ]
    [ -f "${backups[0]}" ]
    [ "$(stat -c '%a' "${backups[0]}")" = 600 ]
    grep -Fxq "SCHOOL_LEGACY_JWT_SECRET='obsolete-school-secret'" "${backups[0]}"
}

@test "session provisioning refuses an accidental second rotation without changing runtime" {
    "$SCRIPT"
    local before after
    before=$(sha256sum "$RUNTIME_ENV")

    run "$SCRIPT"

    [ "$status" -eq 2 ]
    [[ "$output" == *'already exist; no changes were made'* ]]
    after=$(sha256sum "$RUNTIME_ENV")
    [ "$after" = "$before" ]

    local -a backups=("$RUNTIME_ENV".before-session-*)
    [ "${#backups[@]}" -eq 1 ]
}
