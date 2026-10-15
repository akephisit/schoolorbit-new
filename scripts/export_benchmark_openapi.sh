#!/usr/bin/env bash
set -euo pipefail

# Disposable builders only. Keep documentation compilation outside the measured release target.
if grep -q '^api-docs = ' Cargo.toml; then
    unset RUSTC_WRAPPER
    docs_target=${BENCH_DOCS_TARGET_DIR:-/tmp/schoolorbit-benchmark-api-docs}
    CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS=2 \
        cargo build --quiet --locked --features api-docs --bin backend-school \
        --target-dir "$docs_target"
    executable="$docs_target/debug/backend-school"
else
    # Historical baselines included the exporter in their production binary.
    executable=target/release/backend-school
fi
env -i PATH="$PATH" HOME=/tmp "$executable" export-openapi
