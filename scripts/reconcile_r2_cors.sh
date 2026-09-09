#!/usr/bin/env bash
# Sourced by deployment after defining r2_cli. Never print raw provider errors.

schoolorbit_normalize_r2_cors() {
    jq -ceS '
        def strings: type == "array" and all(.[]; type == "string");
        if type == "object" and (.CORSRules | type == "array")
           and all(.CORSRules[];
               type == "object"
               and (.AllowedOrigins | strings)
               and (.AllowedMethods | strings)
               and ((.AllowedHeaders // []) | strings)
               and ((.ExposeHeaders // []) | strings)
               and ((has("MaxAgeSeconds") | not) or (.MaxAgeSeconds | type == "number")))
        then .CORSRules |= (map(
            .AllowedOrigins |= sort |
            .AllowedMethods |= sort |
            .AllowedHeaders = ((.AllowedHeaders // []) | sort) |
            .ExposeHeaders = ((.ExposeHeaders // []) | sort)
        ) | sort_by(tojson))
        else error("invalid CORS policy") end
    ' 2>/dev/null
}

schoolorbit_read_r2_cors() {
    local bucket="$1" allow_missing="$2" error_file policy
    error_file="$(mktemp)" || return 1
    if policy="$(r2_cli s3api get-bucket-cors --bucket "$bucket" --output json 2>"$error_file")"; then
        rm -f "$error_file"
        printf '%s\n' "$policy"
        return 0
    fi
    if [ "$allow_missing" = true ] && grep -Fq '(NoSuchCORSConfiguration)' "$error_file"; then
        rm -f "$error_file"
        printf '%s\n' '{"CORSRules":[]}'
        return 0
    fi
    rm -f "$error_file"
    echo 'R2 CORS read failed; refusing to continue' >&2
    return 1
}

schoolorbit_reconcile_r2_cors() {
    local bucket="$1" desired="$2" current normalized_desired normalized_current
    normalized_desired="$(printf '%s\n' "$desired" | schoolorbit_normalize_r2_cors)" || return 1
    current="$(schoolorbit_read_r2_cors "$bucket" true)" || return 1
    if ! normalized_current="$(printf '%s\n' "$current" | schoolorbit_normalize_r2_cors)"; then
        echo 'R2 CORS response is invalid; refusing to continue' >&2
        return 1
    fi
    if [ "$normalized_current" = "$normalized_desired" ]; then
        echo 'r2_cors_action=unchanged'
        return 0
    fi
    if ! r2_cli s3api put-bucket-cors --bucket "$bucket" \
        --cors-configuration "$desired" >/dev/null 2>&1; then
        echo 'R2 CORS write failed; refusing to continue' >&2
        return 1
    fi
    current="$(schoolorbit_read_r2_cors "$bucket" false)" || return 1
    normalized_current="$(printf '%s\n' "$current" | schoolorbit_normalize_r2_cors)" || return 1
    if [ "$normalized_current" != "$normalized_desired" ]; then
        echo 'R2 CORS verification failed; refusing to continue' >&2
        return 1
    fi
    echo 'r2_cors_action=updated'
}
