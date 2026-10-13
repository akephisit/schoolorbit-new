#!/usr/bin/env bash
set -euo pipefail

# Observe the final executable link without changing the driver's arguments or
# optimization profile. Keep evidence in the builder, never in the runtime image.
: "${SCHOOLORBIT_LINK_METRICS:?builder link metrics path is required}"
started=$(date +%s%N)
cc "$@"
printf '%s\n' "$(($(date +%s%N) - started))" >>"$SCHOOLORBIT_LINK_METRICS"
