#!/usr/bin/env bash
set -euo pipefail

# Run only in a disposable pinned Docker builder, never the developer's source tree.
backend=${1:?backend is required}
variant=${2:?variant is required}
output=${3:?output directory is required}
owner=${4:-application}
samples=${BENCH_SAMPLES:-2}
case "$backend" in backend-school | backend-admin) ;; *) exit 64 ;; esac
case "$variant" in default | opt2 | cgu64 | cgu256 | api-batches) ;; *) exit 64 ;; esac
[[ $samples =~ ^[2-5]$ ]] || exit 64
case "$backend/$owner" in
    */application) source_file=src/main.rs ;;
    backend-school/academic) source_file=crates/school-academic-http/src/core/handlers.rs ;;
    backend-school/attendance) source_file=crates/school-attendance/src/services/sessions.rs ;;
    *) exit 64 ;;
esac
test -f "$source_file"
if [[ "$variant" == api-batches ]]; then
    [[ "$backend/$owner" == backend-school/application ]] || exit 64
    cp /candidate-api.rs src/api_contract.rs
fi
mkdir -p "$output"
unset RUSTC_WRAPPER
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2

# Instrument only the final crate, leaving dependency flags/cache identities intact.
cat >/tmp/schoolorbit-benchmark-linker <<'LINKER'
#!/usr/bin/env bash
set -euo pipefail
start=$(date +%s%N)
cc "$@"
end=$(date +%s%N)
printf '%s\n' "$((end - start))" >>"$BENCH_LINK_LOG"
printf '%s\n' "$@" | sed -n '/^-fuse-ld=/p' >>"${BENCH_LINK_LOG}.driver"
LINKER
chmod +x /tmp/schoolorbit-benchmark-linker
export BENCH_LINK_LOG="$output/link-nanoseconds.txt"
flags=(-C linker=/tmp/schoolorbit-benchmark-linker)
if [[ "$backend" == backend-school ]]; then flags+=(-C lto=off); fi
case "$variant" in
    opt2) flags+=(-C opt-level=2) ;;
    cgu64) flags+=(-C codegen-units=64) ;;
    cgu256) flags+=(-C codegen-units=256) ;;
esac
build() {
    cargo rustc --release --locked --bin "$backend" --timings -- "${flags[@]}"
}
export_api() {
    env -i PATH="$PATH" HOME=/tmp "target/release/$backend" export-openapi
}

# Changing final-crate flags can require a complete application rebuild. Exclude it.
: >"$BENCH_LINK_LOG"
: >"${BENCH_LINK_LOG}.driver"
start=$(date +%s%N)
build 2>&1 | tee "$output/prime.log"
end=$(date +%s%N)
printf '%s\n' "$((end - start))" >"$output/prime-nanoseconds.txt"
if [[ "$backend" == backend-school ]]; then
    export_api >"$output/openapi-baseline.json"
    reference="$output/../../openapi-baseline.json"
    if [[ "$variant/$owner" == default/application && ! -f "$reference" ]]; then
        cp "$output/openapi-baseline.json" "$reference"
    fi
fi
sha256sum src/main.rs >"$output/source-sha256.txt"
if [[ "$backend" == backend-school ]]; then sha256sum src/api_contract.rs >>"$output/source-sha256.txt"; fi

original=$(mktemp)
cp "$source_file" "$original"
restore() {
    cp "$original" "$source_file"
    rm -f "$original"
}
trap restore EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
for ((sample = 1; sample <= samples; sample++)); do
    cp "$original" "$source_file"
    printf '\n// Docker invalidation benchmark: %s %s sample %s\n' "$variant" "$owner" "$sample" >>"$source_file"
    : >"$BENCH_LINK_LOG"
    : >"${BENCH_LINK_LOG}.driver"
    start=$(date +%s%N)
    build 2>&1 | tee "$output/compile-$sample.log"
    end=$(date +%s%N)
    cp target/cargo-timings/cargo-timing.html "$output/cargo-timing-$sample.html"
    stat -c '%s' "target/release/$backend" >"$output/binary-bytes-$sample.txt"
    cp "$BENCH_LINK_LOG" "$output/link-nanoseconds-$sample.txt"
    cp "${BENCH_LINK_LOG}.driver" "$output/link-driver-$sample.txt"
    printf '%s\n' "$((end - start))" >"$output/cargo-nanoseconds-$sample.txt"
    if [[ "$backend" == backend-school ]]; then
        export_api >"$output/openapi-$sample.json"
    fi
done

# Keep the final source unchanged for this measurement; restore only on exit.
start=$(date +%s%N)
build
end=$(date +%s%N)
printf '%s\n' "$((end - start))" >"$output/unchanged-nanoseconds.txt"
python3 "$(dirname "$0")/summarize_backend_benchmark.py" "$output" "$backend" "$variant" "$owner" "$samples"
