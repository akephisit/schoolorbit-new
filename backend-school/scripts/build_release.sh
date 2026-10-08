#!/usr/bin/env bash
set -euo pipefail

cache_mode=${SCHOOL_COMPILER_CACHE:-off}
unset RUSTC_WRAPPER
cache_enabled=false
case "$cache_mode" in
    gha)
        if [[ -s /run/secrets/sccache_gha_url && -s /run/secrets/sccache_gha_token ]]; then
            ACTIONS_RESULTS_URL=$(cat /run/secrets/sccache_gha_url)
            ACTIONS_RUNTIME_TOKEN=$(cat /run/secrets/sccache_gha_token)
            export ACTIONS_RESULTS_URL ACTIONS_RUNTIME_TOKEN
            export SCCACHE_GHA_ENABLED=on
            export SCCACHE_GHA_VERSION=schoolorbit-backend-school
            export SCCACHE_IGNORE_SERVER_IO_ERROR=1
            export RUSTC_WRAPPER=/usr/local/bin/sccache
            cache_enabled=true
        fi
        ;;
    local)
        # Local benchmark only; GitHub builds use the remote cache across runners.
        export SCCACHE_DIR=/var/cache/sccache
        export RUSTC_WRAPPER=/usr/local/bin/sccache
        cache_enabled=true
        ;;
    off) ;;
    *)
        echo 'Unsupported school compiler cache mode' >&2
        exit 64
        ;;
esac

echo "school_compiler_cache mode=$cache_mode enabled=$cache_enabled"
cargo rustc --release --locked --bin backend-school --timings -- -C lto=off

mkdir -p target/cargo-timings
printf '{}\n' >target/cargo-timings/sccache-stats.json
if [[ -n ${RUSTC_WRAPPER:-} ]]; then
    # Metrics failure must not turn a successful compile into a failed release.
    /usr/local/bin/sccache --show-stats || true
    if ! /usr/local/bin/sccache --show-stats --stats-format=json >target/cargo-timings/sccache-stats.json; then
        printf '{}\n' >target/cargo-timings/sccache-stats.json
        echo 'Compiler cache statistics are unavailable' >&2
    fi
fi
