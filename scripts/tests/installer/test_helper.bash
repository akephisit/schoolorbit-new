setup_installer_test() {
    export TEST_ROOT
    TEST_ROOT=$(mktemp -d)
    export HOME="$TEST_ROOT/home"
    export FAKE_COMMAND_LOG="$TEST_ROOT/commands.log"
    export PHASE_LOG="$TEST_ROOT/phases.log"
    export CAPTURED_REQUEST_BODY="$TEST_ROOT/request.json"
    export CAPTURED_STDIN="$TEST_ROOT/stdin.txt"
    export FAKE_BIN="$TEST_ROOT/bin"
    export ORIGINAL_PATH=${ORIGINAL_PATH:-$PATH}
    mkdir -p "$HOME" "$FAKE_BIN"
    : >"$FAKE_COMMAND_LOG"
    : >"$PHASE_LOG"
    : >"$CAPTURED_STDIN"
    export PATH="$FAKE_BIN:$ORIGINAL_PATH"
    export SCHOOLORBIT_STATE_HOME="$HOME/.local/state/schoolorbit-installer"
    export SO_PROVIDER_POLL_ATTEMPTS=1
    export SO_PROVIDER_POLL_DELAY=0

    local name
    for name in \
        SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN \
        SCHOOLORBIT_CLOUDFLARE_DEPLOY_TOKEN \
        SCHOOLORBIT_CLOUDFLARE_RUNTIME_TOKEN \
        SCHOOLORBIT_SERVER_PASSWORD \
        DATABASE_URL JWT_SECRET SESSION_HMAC_KEY SCHOOL_ROLLBACK_JWT_SECRET \
        INTERNAL_API_SECRET ENCRYPTION_KEY BLIND_INDEX_KEY DEPLOY_KEY \
        NEON_API_KEY NEON_DB_PASSWORD R2_ACCESS_KEY_ID R2_SECRET_ACCESS_KEY \
        VAPID_PRIVATE_KEY SCHOOLORBIT_RUNTIME_GITHUB_TOKEN \
        SMOKE_SUBDOMAIN SMOKE_USERNAME SMOKE_PASSWORD \
        NEON_PROJECT_ID NEON_HOST R2_ACCOUNT_ID R2_PUBLIC_BUCKET_NAME \
        R2_PRIVATE_BUCKET_NAME R2_PUBLIC_URL VAPID_PUBLIC_KEY; do
        unset "$name"
    done

    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/common.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/config.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/state.sh"
}

teardown_installer_test() {
    rm -rf "$TEST_ROOT"
}

make_fake_command() {
    local name=$1 body=$2
    printf '#!/usr/bin/env bash\n%s\n' "$body" >"$FAKE_BIN/$name"
    chmod 0755 "$FAKE_BIN/$name"
}

seed_checkpoint_with_passed_phase() {
    local phase=$1
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=schoolorbit.app
    SO_CONFIG[ref]=main
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    state_init run-123
    state_mark_phase "$phase" '{"status":"passed"}'
}
