#!/usr/bin/env bash

# Source from the release script. The credential is scoped to this function call.
schoolorbit_personnel_preflight() {
    local jq_image=${1:?Missing jq image}
    local response_file=${2:?Missing preflight response file}
    local credential=${SCHOOLORBIT_INTERNAL_API_SECRET:-}

    if [[ -z $credential ]]; then
        echo 'Personnel preflight credential is unavailable; maintenance remains enabled' >&2
        return 1
    fi
    if ! curl --fail --silent --show-error --connect-timeout 10 --max-time 300 \
        -H 'X-Internal-Caller: school-release' \
        -H "X-Internal-Secret: ${credential}" \
        -o "$response_file" http://127.0.0.1:8081/internal/personnel-preflight; then
        echo 'Personnel preflight request failed; maintenance remains enabled' >&2
        return 1
    fi
    if ! podman run --rm -i "$jq_image" -e '
        .success == true and .data.passed == true and .data.totalSchools > 0
        and (.data.schools | length) == .data.totalSchools
        and all(.data.schools[]; .passed == true and (.checks | length) > 0
          and all(.checks[]; .passed == true and .count >= 0))
      ' <"$response_file" >/dev/null 2>&1; then
        echo 'Personnel preflight failed; no tenant migration started and maintenance remains enabled' >&2
        podman run --rm -i "$jq_image" -r '
          .data.schools[]? | .subdomain as $school | .checks[]?
          | select(.passed != true)
          | "personnel_preflight subdomain=\($school) code=\(.code) count=\(.count)"
        ' <"$response_file" 2>/dev/null || return 1
        return 1
    fi
}
