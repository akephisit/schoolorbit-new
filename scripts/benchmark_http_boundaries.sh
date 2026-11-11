#!/usr/bin/env bash
set -euo pipefail
# shellcheck source-path=SCRIPTDIR
# shellcheck source=backend_benchmark_source.sh
. "$(dirname "$0")/backend_benchmark_source.sh"
unset RUSTC_WRAPPER
export CARGO_INCREMENTAL=0
mkdir -p /results
apt-get update -qq
apt-get install -y -qq --no-install-recommends python3 >/dev/null
build() {
    cargo rustc --release --locked --bin backend-school --timings -- -C lto=off
}
export_api() {
    bash "$(dirname "$0")/export_benchmark_openapi.sh"
}
measure() {
    local phase=$1 owner=$2 sample start end output source original
    source=$(benchmark_owner_source backend-school "$owner")
    original=$(mktemp)
    cp "$source" "$original"
    output="/results/$phase/$owner"
    mkdir -p "$output"
    # Restore the previous owner before measuring a new family.
    build >"$output/pristine-prime.log" 2>&1
    for sample in 1 2; do
        cp "$original" "$source"
        printf '\n// HTTP boundary invalidation: %s %s sample %s\n#[allow(dead_code)]\npub const BUILD_HTTP_BOUNDARY_MARKER: u64 = %s;\n' "$phase" "$owner" "$sample" "$sample" >>"$source"
        start=$(date +%s%N)
        build 2>&1 | tee "$output/compile-$sample.log"
        end=$(date +%s%N)
        printf '%s\n' "$((end - start))" >"$output/cargo-nanoseconds-$sample.txt"
        stat -c '%s' target/release/backend-school >"$output/binary-bytes-$sample.txt"
        cp target/cargo-timings/cargo-timing.html "$output/cargo-timing-$sample.html"
        export_api >"$output/openapi-$sample.json"
    done
    cp "$original" "$source"
    rm -f "$original"
}
# Separate cache/flag priming from timed source-changing builds.
build 2>&1 | tee /results/prime-before.log
export_api >/results/openapi-baseline.json
measure before application
measure before certificates
measure before academic
measure before navigation
measure before notifications
[[ "$PWD" == /app && -d /candidate/src && -d /candidate/crates ]]
rm -rf /app/src /app/crates
cp -a /candidate/src /candidate/crates /app/
cp -a /candidate/Cargo.toml /candidate/Cargo.lock /app/
# Normalize source mtimes after the same-path graph swap, preserving source bytes.
find src crates -type f -exec touch {} +
touch Cargo.toml Cargo.lock
build 2>&1 | tee /results/prime-after.log
measure after application
measure after certificates
measure after academic
measure after navigation
measure after notifications
build >/results/final-pristine-prime.log 2>&1
start=$(date +%s%N)
build
end=$(date +%s%N)
printf '%s\n' "$((end - start))" >/results/unchanged-nanoseconds.txt
python3 - <<'PY'
import json,pathlib,re,statistics
root=pathlib.Path('/results');baseline=json.loads((root/'openapi-baseline.json').read_text());result={}
for phase in ('before','after'):
 result[phase]={}
 for folder in sorted((root/phase).iterdir()):
  samples=[]
  for sample in (1,2):
   assert json.loads((folder/f'openapi-{sample}.json').read_text())==baseline,'API changed'
   samples.append({'cargo_seconds':int((folder/f'cargo-nanoseconds-{sample}.txt').read_text())/1e9,'binary_bytes':int((folder/f'binary-bytes-{sample}.txt').read_text()),'compiled_units':re.findall(r'Compiling (school-\S+|backend-school)',(folder/f'compile-{sample}.log').read_text())})
  result[phase][folder.name]={'samples':samples,'mean_seconds':statistics.mean(s['cargo_seconds'] for s in samples)}
result['unchanged_seconds']=int((root/'unchanged-nanoseconds.txt').read_text())/1e9
(root/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
PY
