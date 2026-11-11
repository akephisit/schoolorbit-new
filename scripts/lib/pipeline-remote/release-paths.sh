#!/usr/bin/env bash
# Executable inputs and mutable operational state have separate lifecycles.
schoolorbit_release_paths() {
    local caller=$1 actual expected
    schoolorbit_state_root="${SCHOOLORBIT_STACK_ROOT:-/opt/stack}/deployment"
    schoolorbit_code_root=$schoolorbit_state_root
    actual=$(cd -- "$(dirname -- "$caller")/../../.." && pwd -P) || return 78
    if [[ $actual == "$schoolorbit_state_root/release-bundles/"* && -z ${RELEASE_BUNDLE_SHA256-} ]]; then return 64; fi
    if [[ -n ${RELEASE_BUNDLE_SHA256-} ]]; then
        [[ ${RELEASE_SHA-} =~ ^[0-9a-f]{40}$ && $RELEASE_BUNDLE_SHA256 =~ ^[0-9a-f]{64}$ ]] || return 64
        expected="$schoolorbit_state_root/release-bundles/$RELEASE_SHA/$RELEASE_BUNDLE_SHA256"
        [[ $actual == "$expected" && ! -L $expected && -f $expected/release-bundle.json ]] || return 78
        schoolorbit_code_root=$expected
    fi
    # Public path results are also available to nested release invocations.
    export schoolorbit_code_root schoolorbit_state_root
}
