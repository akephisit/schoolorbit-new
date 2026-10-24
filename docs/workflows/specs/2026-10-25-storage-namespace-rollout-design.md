# Storage namespace rollout

The School release must apply the canonical tenant migrations and verify every retained storage
namespace before starting matching application writers. The current release starts the new HTTP
server and its background workers before calling its migration endpoint. That ordering is unsafe
for migrations 136 and 137, which leave historical watch namespaces unbound until explicitly attested.

## Owners and executable contract

The prepared, digest-pinned School runtime image supplies its existing main executable. Early
headless commands return before constructing application state, opening a listener or spawning
application workers. The local `file_storage_namespace` executable remains a compatible wrapper
around the same application-owned namespace implementation; release does not build or distribute
a second executable.

- `backend-school migrate-tenants` uses the canonical migration owner, permission reconciliation,
  cutover audits and Admin migration-status updates. It does not register runtime schedules.
- `backend-school storage-namespace check-admin` checks the complete authenticated Admin inventory,
  database identities, canonical schema history and retained watches. It never binds namespaces.
- `backend-school storage-namespace context-admin` emits only the validated tenant count, inventory
  digest and configured public/private namespace digests.
- `backend-school storage-namespace routing-fingerprints` emits only configured public/private
  namespace digests and performs no network or database access.
- `backend-school storage-namespace attest-admin` performs the explicitly authorized one-time
  binding described below, then requires the same complete readiness check.

Routing output is one JSON object with `schema_version: 1`, `public_namespace_sha256` and
`private_namespace_sha256`. Context output additionally contains `tenants` and `inventory_sha256`.
Digests are lowercase SHA-256 hex. Operational commands exit zero on success, one on validation,
readiness, database or Admin failure, and two for unsupported or malformed command arguments.
Errors expose fixed safe codes; successful operational output contains aggregate counts and only
the approved inventory/routing digests, without tenant identities or private routing values.

The authoritative inventory comes from the authenticated, unfiltered Admin school-list owner. Its
snapshot is fenced against pending tenant-directory mutations and includes UUIDs, status and a
complete total. Duplicate, missing or malformed identities, incomplete responses and unavailable
databases fail closed. Inactive tenants are included; migration credentials require an explicit
internal `include_inactive=true` opt-in while existing caller authorization and default behavior
remain unchanged. An authoritative empty inventory is valid for first installation. Shell scripts
must not create a separate inventory or migrate databases themselves.

Normal application startup repeats the read-only complete namespace check before its first
application spawn. Directly starting the image therefore cannot bypass the deployment gate.

## Release order and recovery

1. Validate the exact prepared image, runtime configuration and any explicit attestation inputs.
   Before R2 reconciliation or maintenance, compare expected routing and inventory context using
   read-only commands. Capture old routing from the actual existing container's allowlisted
   configuration using the prepared image's offline fingerprint command. Never reconstruct old
   routing from the new environment or perform provider PUT/DELETE calls to establish identity.
2. Enter and verify universal maintenance under the existing production lock.
3. Preserve the verified old immutable image identity, routing evidence and private configuration
   handle. Check the old-container identity still matches the preflight context before stopping it.
4. Stop the old School container, verify it has stopped and remove it to prevent boot-time restart.
5. Record the migration-attempt boundary and run the headless canonical migration command.
6. For an explicitly requested attestation only, verify the evidence context and bind exact watches.
7. Run `check-admin`; only success permits creation of the matching application container.
8. Run existing ordinary readiness, complete migration/cutover audits, authenticated smoke and
   frontend acceptance. Global acceptance alone opens traffic and publishes accepted baselines.

Preflight failure preserves the old runtime. Maintenance failures remain in maintenance. After the
first migration attempt, the old binary must never restart: recovery uses the same compatible
release or a forward release. Preserve the previous image/allowlisted routing configuration and aggregate progress
evidence without treating them as post-migration rollback permission. Existing timing, lease,
client timeout, resource and health policies are unchanged.

Stop verification proves that the old process is gone; it does not prove an external PUT has
settled. Retained watches and the existing late-write cleanup protocol remain necessary. Neither
container removal nor rollout acceptance makes database and external-provider effects atomic.

## Explicit historical attestation

The user independently states that the production R2 account and public/private buckets have never
changed since files were first stored. That assertion authorizes this bounded historical binding;
it is not a standing authorization to infer future namespaces from current configuration.

Only an explicit manual Pipeline dispatch with `automatic=false` can supply `namespace_attestation`.
Nonempty input is rejected on automatic dispatches, including automatic `workflow_dispatch` runs.
Its validated JSON is forwarded as `FILE_STORAGE_NAMESPACE_ATTESTATION_JSON`, never interpolated
into executable shell code or credential-bearing arguments. The strict object has exactly these fields:

```json
{
  "schema_version": 1,
  "evidence_digest": "<64 lowercase hex digits>",
  "expected_inventory_sha256": "<64 lowercase hex digits>",
  "expected_public_namespace_sha256": "<64 lowercase hex digits>",
  "expected_private_namespace_sha256": "<64 lowercase hex digits>",
  "scope": "all_unbound"
}
```

The operator obtains context from the real deployment and supplies the independent assertion's
evidence digest. Before removing the old container, release compares both expected namespace
digests with the actual old routing and the target configuration. A create-new mode-0600 journal
binds the verified old-routing digests to the exact release SHA and prepared image digest, allowing
an interrupted retry after old-container removal without inventing replacement historical proof.
Missing, mismatched or malformed proof blocks attestation.

The Rust owner validates the live complete inventory digest and target routing again, plans exact
watch IDs and revisions in private mode-0600 manifests, and calls existing dry-run/apply operations.
Their owner checks, namespace identity, row revisions and claim exclusions remain authoritative.
Partial binding failure leaves maintenance active and can be retried under the same verified
context. No raw credentials, database URLs, bucket names or object locators enter logs or public
artifacts. Ordinary pushes, empty inputs and automatic dispatches execute check-only behavior;
unknown or mismatched namespaces block them without mutation.

## Verification

Rust ownership covers headless argument routing, complete inventory validation, empty and inactive
tenants, canonical migrations, owner authority, explicit binding and the pre-spawn startup gate.
Pipeline ownership covers actual shell orchestration with synthetic commands: stage ordering,
immutable image selection, no server before readiness, failure boundaries, historical-proof
preservation/replay, strict manual-only input transport and aggregate diagnostics. Existing release,
migration-completion, build-provenance, maintenance and deployment guards remain required. Run
scoped Node/Bats, ShellCheck/shfmt, actionlint and documentation-policy checks locally. Successful
production migration, namespace readiness and authenticated acceptance remain separate deployment
evidence; local checks and the Pipeline build gate do not replace them.
