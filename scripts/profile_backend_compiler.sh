#!/usr/bin/env bash
set -euo pipefail

# Cargo's dependency build scripts can track RUSTC_BOOTSTRAP. Enable diagnostics
# only inside the selected executable's rustc invocation, preserving their cache.
case ${BENCH_PROFILE_CRATE:-} in backend_school | backend_admin) ;; *) exit 64 ;; esac
compiler=${1:?compiler is required}
shift
crate_name=
previous=
for argument in "$@"; do
    if [[ $previous == --crate-name ]]; then
        crate_name=$argument
        break
    fi
    previous=$argument
done
unset RUSTC_BOOTSTRAP
if [[ $crate_name == "$BENCH_PROFILE_CRATE" ]]; then
    export RUSTC_BOOTSTRAP="$BENCH_PROFILE_CRATE"
fi
exec "$compiler" "$@"
