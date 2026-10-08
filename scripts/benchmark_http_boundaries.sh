#!/usr/bin/env bash
set -euo pipefail
unset RUSTC_WRAPPER
export CARGO_INCREMENTAL=0
mkdir -p /results
apt-get update -qq
apt-get install -y -qq --no-install-recommends python3 >/dev/null
build() {
    cargo rustc --release --locked --bin backend-school --timings -- -C lto=off
}
export_api() {
    env -i PATH="$PATH" HOME=/tmp target/release/backend-school export-openapi
}
measure() {
    local phase=$1 owner=$2 source=$3 sample start end output
    output="/results/$phase/$owner"
    mkdir -p "$output"
    for sample in 1 2; do
        printf '\n// HTTP boundary invalidation: %s %s sample %s\n' "$phase" "$owner" "$sample" >>"$source"
        start=$(date +%s%N)
        build 2>&1 | tee "$output/compile-$sample.log"
        end=$(date +%s%N)
        printf '%s\n' "$((end - start))" >"$output/cargo-nanoseconds-$sample.txt"
        stat -c '%s' target/release/backend-school >"$output/binary-bytes-$sample.txt"
        cp target/cargo-timings/cargo-timing.html "$output/cargo-timing-$sample.html"
        export_api >"$output/openapi-$sample.json"
    done
}
# Separate cache/flag priming from timed source-changing builds.
build 2>&1 | tee /results/prime-before.log
export_api >/results/openapi-baseline.json
measure before application src/main.rs
measure before certificates src/modules/certificates/handlers.rs
measure before academic src/modules/academic/core/handlers.rs
measure before navigation src/modules/menu/services/menu_service.rs
measure before notifications src/services/notification.rs
rm -rf /app/src /app/crates
cp -a /candidate/src /candidate/crates /app/
cp -a /candidate/Cargo.toml /candidate/Cargo.lock /app/
build 2>&1 | tee /results/prime-after.log
measure after application src/main.rs
measure after certificates crates/school-certificates-http/src/handlers.rs
measure after academic crates/school-academic-http/src/core/handlers.rs
measure after navigation crates/school-navigation/src/services/menu_service.rs
measure after notifications crates/school-notifications/src/publisher.rs
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
