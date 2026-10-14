#!/usr/bin/env bash
set -euo pipefail

wait_for_cache_writes() {
    local observed status deadline=$((SECONDS + 120))
    while ((SECONDS < deadline)); do
        if ! observed=$(timeout 5 /usr/local/bin/sccache --show-stats); then
            return 1
        fi
        # The pinned CLI exposes aggregate counts separately from per-language rows.
        # A miss is settled when its asynchronous write succeeds or reports an error.
        status=0
        awk '
            /^Cache misses[[:space:]]+[0-9]+$/ { misses=$NF; seen++ }
            /^Cache writes[[:space:]]+[0-9]+$/ { writes=$NF; seen++ }
            /^Cache write errors[[:space:]]+[0-9]+$/ { errors=$NF; seen++ }
            END { if (seen != 3) exit 2; if (writes + errors < misses) exit 1 }
        ' <<<"$observed" || status=$?
        case "$status" in
            0) return 0 ;;
            2) return 1 ;;
        esac
        sleep 1
    done
    return 1
}

phase=${1:-application}
case "$phase" in
    application) metrics_dir=target/cargo-timings ;;
    dependencies) metrics_dir=target/dependency-timings ;;
    *)
        echo 'Unsupported school build phase' >&2
        exit 64
        ;;
esac

cache_mode=${SCHOOL_COMPILER_CACHE:-off}
unset RUSTC_WRAPPER
cache_enabled=false
case "$cache_mode" in
    gha)
        if [[ -s /run/secrets/sccache_gha_url && -s /run/secrets/sccache_gha_token ]]; then
            ACTIONS_RESULTS_URL=$(cat /run/secrets/sccache_gha_url)
            ACTIONS_RUNTIME_TOKEN=$(cat /run/secrets/sccache_gha_token)
            export ACTIONS_RESULTS_URL ACTIONS_RUNTIME_TOKEN
            # BuildKit does not forward the runner's cache protocol selector.
            # Without it sccache selects the retired v1 API even with a v2 URL.
            export ACTIONS_CACHE_SERVICE_V2=true
            export SCCACHE_GHA_ENABLED=on
            export SCCACHE_GHA_VERSION=schoolorbit-backend-school
            export SCCACHE_IGNORE_SERVER_IO_ERROR=1
            export RUSTC_WRAPPER=/usr/local/bin/rustc-cache
            cache_enabled=true
        fi
        ;;
    local)
        # Local benchmark only; GitHub builds use the remote cache across runners.
        export SCCACHE_DIR=/var/cache/sccache
        export RUSTC_WRAPPER=/usr/local/bin/rustc-cache
        cache_enabled=true
        ;;
    off) ;;
    *)
        echo 'Unsupported school compiler cache mode' >&2
        exit 64
        ;;
esac

echo "school_compiler_cache phase=$phase mode=$cache_mode enabled=$cache_enabled"
# cc-rs recognizes the basename "sccache" and otherwise wraps native compilers too.
# Invoke the same pinned binary through a Rust-only wrapper to avoid hundreds of extra
# C/C++/assembly cache requests against GitHub's rate-limited cache service.
compile_started=$(date +%s%N)
if [[ "$phase" == dependencies ]]; then
    cargo chef cook --release --recipe-path recipe.json
else
    mkdir -p "$metrics_dir"
    export SCHOOLORBIT_LINK_METRICS="$PWD/$metrics_dir/link-nanoseconds.txt"
    : >"$SCHOOLORBIT_LINK_METRICS"
    cargo rustc --release --locked --bin backend-school --timings -- -C lto=off -C "linker=$PWD/scripts/release_linker.sh"
fi
compile_nanoseconds=$(($(date +%s%N) - compile_started))

cache_writes_settled=true
cache_wait_started=$(date +%s%N)
if [[ -n ${RUSTC_WRAPPER:-} ]] && ! wait_for_cache_writes; then
    cache_writes_settled=false
    echo 'Compiler cache write settlement is unavailable or timed out' >&2
fi
cache_write_wait_nanoseconds=$(($(date +%s%N) - cache_wait_started))

mkdir -p "$metrics_dir"
printf '{"schemaVersion":1,"compileNanoseconds":%s,"cacheMode":"%s","cacheEnabled":%s,"cacheWriteWaitNanoseconds":%s,"cacheWritesSettled":%s}\n' \
    "$compile_nanoseconds" "$cache_mode" "$cache_enabled" "$cache_write_wait_nanoseconds" "$cache_writes_settled" >"$metrics_dir/build-metrics.json"
printf '{}\n' >"$metrics_dir/sccache-stats.json"
if [[ -n ${RUSTC_WRAPPER:-} ]]; then
    # Metrics failure must not turn a successful compile into a failed release.
    /usr/local/bin/sccache --show-stats || true
    if ! /usr/local/bin/sccache --show-stats --stats-format=json >"$metrics_dir/sccache-stats.json"; then
        printf '{}\n' >"$metrics_dir/sccache-stats.json"
        echo 'Compiler cache statistics are unavailable' >&2
    fi
fi
