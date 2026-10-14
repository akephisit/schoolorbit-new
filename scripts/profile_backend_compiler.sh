#!/usr/bin/env bash
set -euo pipefail

backend=${1:?backend is required}
output=${2:?profile output is required}
case "$backend" in backend-school | backend-admin) ;; *) exit 64 ;; esac
mkdir -p "$output"
temporary=$(mktemp -d /tmp/schoolorbit-compiler-profile.XXXXXX)
trap 'rm -rf "$temporary"' EXIT
cat >"$temporary/rustc-profile" <<'WRAPPER'
#!/usr/bin/env bash
set -euo pipefail
# Cargo and dependency build scripts retain their normal environment/fingerprints.
# Only the selected rustc invocation receives bootstrap for diagnostic -Z flags.
if [[ " $* " == *" --crate-name $SCHOOLORBIT_PROFILE_CRATE "* ]]; then
    exec env RUSTC_BOOTSTRAP=1 "$@"
fi
exec "$@"
WRAPPER
chmod +x "$temporary/rustc-profile"
unset RUSTC_BOOTSTRAP
export RUSTC_WRAPPER="$temporary/rustc-profile"
export SCHOOLORBIT_PROFILE_CRATE=${backend//-/_}
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
flags=(-Z time-passes -Z "dump-mono-stats=$output/mono" -Z dump-mono-stats-format=json)
if [[ "$backend" == backend-school ]]; then flags+=(-C lto=off); fi
cargo rustc --release --locked --bin "$backend" -- "${flags[@]}"
