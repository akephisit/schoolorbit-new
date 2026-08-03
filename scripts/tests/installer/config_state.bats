#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
}

teardown() {
    teardown_installer_test
}

@test "rejects command-line secret flags without echoing their values" {
    run parse_args migrate-vps --target 192.0.2.20 --internal-api-secret exposed-value
    [ "$status" -eq 64 ]
    [[ "$output" != *exposed-value* ]]
}

@test "parses migrate-vps non-secret inputs and applies safe defaults" {
    parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --dry-run --secrets-stdin

    [ "${SO_COMMAND}" = migrate-vps ]
    [ "${SO_CONFIG[repository]}" = owner/repo ]
    [ "${SO_CONFIG[target]}" = 192.0.2.20 ]
    [ "${SO_CONFIG[base_domain]}" = schoolorbit.app ]
    [ "${SO_CONFIG[ref]}" = main ]
    [ "${SO_CONFIG[bootstrap_user]}" = root ]
    [ "${SO_CONFIG[server_user]}" = schoolorbit ]
    [ "${SO_CONFIG[ssh_port]}" = 22 ]
    [ "${SO_DRY_RUN}" = true ]
    [ "${SO_SECRETS_STDIN}" = true ]
}

@test "rejects invalid targets repositories domains ports and unknown options" {
    run parse_args migrate-vps --repository owner/repo --target 999.0.2.20
    [ "$status" -eq 64 ]

    run parse_args migrate-vps --repository ../repo --target 192.0.2.20
    [ "$status" -eq 64 ]

    run parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --base-domain '-invalid.test'
    [ "$status" -eq 64 ]

    run parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --ssh-port 70000
    [ "$status" -eq 64 ]

    run parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --surprise value
    [ "$status" -eq 64 ]
}

@test "accepts only the documented resume and rollback forms" {
    parse_args migrate-vps --resume run-123
    [ "${SO_RESUME_RUN_ID}" = run-123 ]

    parse_args rollback-dns --run-id run-456
    [ "${SO_ROLLBACK_RUN_ID}" = run-456 ]

    run parse_args rollback-dns --run-id run-456 --target 192.0.2.20
    [ "$status" -eq 64 ]
}

@test "loads secret and public runtime values from one stdin JSON object" {
    parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --secrets-stdin
    load_inputs <"$BATS_TEST_DIRNAME/fixtures/secrets.json"

    [ "${SO_SECRETS[INTERNAL_API_SECRET]}" = 'internal-api-7vK9nM3qR8wX2zLp' ]
    [ "${SO_SECRETS[SMOKE_PASSWORD]}" = 'Smoke-Pass-7vK9nM3q' ]
    [ "${SO_CONFIG[runtime:NEON_PROJECT_ID]}" = 'silent-moon-24680' ]
    [ "${SO_CONFIG[runtime:R2_PUBLIC_BUCKET_NAME]}" = 'schoolorbit-public-assets' ]
}

@test "accepts a non-empty existing smoke password without imposing creation policy" {
    local input="$TEST_ROOT/short-smoke-password.json"
    jq '.SMOKE_PASSWORD = "short-ok"' \
        "$BATS_TEST_DIRNAME/fixtures/secrets.json" >"$input"
    parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --secrets-stdin

    load_inputs <"$input"

    [ "${SO_SECRETS[SMOKE_PASSWORD]}" = short-ok ]
}

@test "resume reuses checkpointed public runtime values while reloading secrets" {
    local name
    parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --secrets-stdin
    load_inputs <"$BATS_TEST_DIRNAME/fixtures/secrets.json"
    for name in "${SO_REQUIRED_SECRETS[@]}"; do
        export "$name=${SO_SECRETS[$name]}"
    done
    SO_SECRETS=()
    SO_SECRETS_STDIN=false

    load_inputs </dev/null

    [ "${SO_CONFIG[runtime:NEON_PROJECT_ID]}" = silent-moon-24680 ]
    [ "${SO_SECRETS[INTERNAL_API_SECRET]}" = internal-api-7vK9nM3qR8wX2zLp ]
}

@test "DNS rollback reloads only the Cloudflare bootstrap credential" {
    export SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN=cf-bootstrap-7vK9nM3qR8wX2zLp6tY4
    SO_SECRETS=()

    load_cloudflare_bootstrap_token </dev/null

    [ "${SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]}" = cf-bootstrap-7vK9nM3qR8wX2zLp6tY4 ]
    [ "${#SO_SECRETS[@]}" -eq 1 ]
}

@test "rejects shared public and private buckets" {
    local input="$TEST_ROOT/shared-buckets.json"
    jq '.R2_PRIVATE_BUCKET_NAME = .R2_PUBLIC_BUCKET_NAME' \
        "$BATS_TEST_DIRNAME/fixtures/secrets.json" >"$input"
    parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --secrets-stdin

    run load_inputs <"$input"
    [ "$status" -eq 64 ]
}

@test "rejects known placeholder markers even when the value is long enough" {
    local input="$TEST_ROOT/placeholder.json"
    jq '.INTERNAL_API_SECRET = "change-me-secret-value-long-enough"' \
        "$BATS_TEST_DIRNAME/fixtures/secrets.json" >"$input"
    parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --secrets-stdin

    run load_inputs <"$input"
    [ "$status" -eq 64 ]
}

@test "redacts every loaded secret from output" {
    SO_SECRETS[INTERNAL_API_SECRET]=highly-sensitive-value
    run warn 'provider rejected highly-sensitive-value'

    [ "$status" -eq 0 ]
    [[ "$output" == *'[REDACTED]'* ]]
    [[ "$output" != *highly-sensitive-value* ]]
}

@test "retry returns the transient failure code after exhausting attempts" {
    make_fake_command always-fails 'exit 1'
    run retry 2 0 always-fails
    [ "$status" -eq 75 ]
}

@test "checkpoint contains no supplied secret and is private" {
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    SO_CONFIG[ref]=main
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    SO_SECRETS[INTERNAL_API_SECRET]=highly-sensitive-test-value
    state_init run-123
    state_mark_phase preflight '{"status":"passed","code":"ready"}'

    run grep -R 'highly-sensitive-test-value' "$SCHOOLORBIT_STATE_HOME"
    [ "$status" -eq 1 ]
    [ "$(stat -c '%a' "$SO_STATE_FILE")" = 600 ]
    state_phase_done preflight
}

@test "new state refuses to overwrite an existing run checkpoint" {
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    state_init run-123

    run state_init run-123
    [ "$status" -eq 78 ]
}

@test "phase details containing a loaded secret are rejected" {
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    SO_SECRETS[INTERNAL_API_SECRET]=highly-sensitive-test-value
    state_init run-123

    run state_mark_phase preflight '{"status":"failed","code":"highly-sensitive-test-value"}'
    [ "$status" -eq 64 ]
    run grep -R 'highly-sensitive-test-value' "$SCHOOLORBIT_STATE_HOME"
    [ "$status" -eq 1 ]
}

@test "resume loads a checkpoint and rejects a changed non-secret fingerprint" {
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    SO_CONFIG[ref]=main
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    SO_CONFIG[runtime:NEON_PROJECT_ID]=silent-moon-24680
    state_init run-123

    state_load run-123
    state_assert_fingerprint

    SO_CONFIG[target]=192.0.2.21
    run state_assert_fingerprint
    [ "$status" -eq 78 ]
}

@test "entry point help documents safe input channels without secret flags" {
    run scripts/schoolorbit-installer --help
    [ "$status" -eq 0 ]
    [[ "$output" == *'--secrets-stdin'* ]]
    [[ "$output" == *'environment variables'* ]]
    [[ "$output" != *'--internal-api-secret'* ]]
}
