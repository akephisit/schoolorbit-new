#!/usr/bin/env bash
set -euo pipefail

# Run inside the pinned Docker builder image. No production credentials or DB.
backend=${1:?backend is required}
variant=${2:?variant is required}
output=${3:?output directory is required}
mkdir -p "$output"
unset RUSTC_WRAPPER
export CARGO_INCREMENTAL=0

case "$backend" in backend-school | backend-admin) ;; *) exit 64 ;; esac
case "$variant" in gnu | lld | opt2 | cgu64 | cpu1) ;; *) exit 64 ;; esac
apt-get update -qq
apt-get install -y -qq --no-install-recommends lld python3 >/dev/null
if [[ "$backend" == backend-school ]]; then
    env -i PATH="$PATH" HOME=/tmp "target/release/$backend" export-openapi >"$output/openapi-baseline.json"
fi

cat >/tmp/schoolorbit-benchmark-linker <<'LINKER'
#!/usr/bin/env bash
set -euo pipefail
start=$(date +%s%N)
if [[ "$BENCH_LINKER" == lld ]]; then
    cc -fuse-ld=lld "$@"
else
    cc "$@"
fi
end=$(date +%s%N)
printf '%s\n' "$((end - start))" >>"$BENCH_LINK_LOG"
LINKER
chmod +x /tmp/schoolorbit-benchmark-linker
export BENCH_LINKER=lld
if [[ "$variant" == gnu || "$variant" == cpu1 ]]; then export BENCH_LINKER=gnu; fi
flags=(-C linker=/tmp/schoolorbit-benchmark-linker)
if [[ "$backend" == backend-school ]]; then flags+=(-C lto=off); fi
if [[ "$variant" == opt2 ]]; then flags+=(-C opt-level=2); fi
if [[ "$variant" == cgu64 ]]; then flags+=(-C codegen-units=64); fi
export BENCH_LINK_LOG="$output/link-nanoseconds.txt"
printf 'backend=%s variant=%s cpu_available=%s rust=%s\n' "$backend" "$variant" "$(nproc)" "$(rustc --version)"

for sample in 1 2; do
    # Change application source, not a dependency manifest or toolchain.
    source_file=src/main.rs
    if [[ "$backend" == backend-admin ]]; then source_file=src/handlers/school.rs; fi
    printf '\n// Docker build benchmark: %s sample %s\n' "$variant" "$sample" >>"$source_file"
    : >"$BENCH_LINK_LOG"
    start=$(date +%s%N)
    cargo rustc --release --locked --bin "$backend" --timings -- "${flags[@]}" 2>&1 | tee "$output/compile-$sample.log"
    end=$(date +%s%N)
    cp target/cargo-timings/cargo-timing.html "$output/cargo-timing-$sample.html"
    stat -c '%s' "target/release/$backend" >"$output/binary-bytes-$sample.txt"
    cp "$BENCH_LINK_LOG" "$output/link-nanoseconds-$sample.txt"
    printf '%s\n' "$((end - start))" >"$output/cargo-nanoseconds-$sample.txt"
    if [[ "$backend" == backend-school ]]; then
        env -i PATH="$PATH" HOME=/tmp "target/release/$backend" export-openapi >"$output/openapi-$sample.json"
    fi
done

# This measures persistent target reuse separately from a source-changing build.
start=$(date +%s%N)
cargo rustc --release --locked --bin "$backend" -- "${flags[@]}"
end=$(date +%s%N)
printf '%s\n' "$((end - start))" >"$output/unchanged-nanoseconds.txt"
python3 - "$output" <<'PY'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
result = {"samples": [], "unchanged_seconds": int((p / "unchanged-nanoseconds.txt").read_text()) / 1e9}
for sample in (1, 2):
    result["samples"].append({
        "cargo_seconds": int((p / f"cargo-nanoseconds-{sample}.txt").read_text()) / 1e9,
        "link_seconds": sum(int(n) for n in (p / f"link-nanoseconds-{sample}.txt").read_text().splitlines()) / 1e9,
        "binary_bytes": int((p / f"binary-bytes-{sample}.txt").read_text()),
    })
    if (p / f"openapi-{sample}.json").exists():
        document = json.loads((p / f"openapi-{sample}.json").read_text())
        assert document["openapi"] == "3.1.0" and document["paths"]
        assert document == json.loads((p / "openapi-baseline.json").read_text()), "API contract changed"
(p / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
PY
