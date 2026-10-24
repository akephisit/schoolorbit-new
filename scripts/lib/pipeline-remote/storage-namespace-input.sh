#!/usr/bin/env bash
# Transport validation only; the Rust owner revalidates live identities and authority.
storage_namespace_input() {
    local value=${NAMESPACE_ATTESTATION_JSON:-}
    [[ -n $value ]] || return 0
    [[ ${NAMESPACE_ATTESTATION_MANUAL:-false} == true ]] || {
        echo 'storage_rollout_error code=manual_attestation_required' >&2
        return 64
    }
    # The transport schema is flat. Streaming sees duplicate fields that a normal
    # JSON-object parser would otherwise silently overwrite.
    if [[ ${#value} -gt 4096 ]] || ! jq --stream -es '
        [.[] | select(length == 2)] as $fields |
        ($fields | length) == 6 and
        all($fields[]; (.[0] | length) == 1) and
        ($fields | map(.[0][0]) | unique | length) == 6
    ' <<<"$value" >/dev/null 2>&1; then
        echo 'storage_rollout_error code=invalid_attestation_input' >&2
        return 64
    fi
    if ! jq -ces '
        def digest: type == "string" and test("^[0-9a-f]{64}$");
        if length == 1 and (.[0] |
            type == "object" and
            (keys == ["evidence_digest","expected_inventory_sha256","expected_private_namespace_sha256","expected_public_namespace_sha256","schema_version","scope"]) and
            .schema_version == 1 and .scope == "all_unbound" and
            (.evidence_digest | digest) and (.expected_inventory_sha256 | digest) and
            (.expected_public_namespace_sha256 | digest) and (.expected_private_namespace_sha256 | digest))
        then .[0] else error("invalid") end
    ' <<<"$value" 2>/dev/null; then
        echo 'storage_rollout_error code=invalid_attestation_input' >&2
        return 64
    fi
}
