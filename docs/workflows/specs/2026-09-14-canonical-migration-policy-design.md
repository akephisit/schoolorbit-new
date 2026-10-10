# Canonical Migration Policy Design

## Context

SchoolOrbit needs a durable rule for replacing legacy schemas, APIs, DTOs, and runtime paths without allowing old shapes to distort the long-term design. Runtime backward compatibility and preservation of persisted data are separate concerns: the application may cut over to one clean canonical contract, while existing valid data must be transformed into that contract without silent loss.

The repository already requires forward-only immutable migrations and contains phased cutover patterns. This policy makes the intended design principle explicit across analysis, implementation, migration, deployment, rollback, and verification.

## Goals

- Design new work around one orderly canonical model suitable for long-term maintenance.
- Treat persisted legacy data as migration input rather than a reason to preserve legacy runtime interfaces.
- Preserve the meaning, relationships, and required history of valid existing data through deterministic migration and reconciliation.
- Remove legacy endpoints, fields, tables, aliases, fallbacks, and dual paths after a verified cutover.
- Fail closed when data cannot be mapped safely, without leaking sensitive values.
- Permit only bounded, explicitly owned compatibility needed for a safe rollout or rollback.

## Non-goals

- This policy does not require indefinite retention. Authorized retention, deletion, legal-hold, and PDPA rules remain authoritative.
- This change does not itself migrate an existing feature or remove existing legacy code.
- It does not guarantee that an old application binary can run against a post-cutover schema.
- It does not require a multi-phase rollout for a non-destructive change whose risk does not justify one; verification remains proportional to the affected data and runtime boundary.

## Selected Approach: Phased Canonical Cutover

For a destructive or semantic replacement, use the following sequence:

1. **Expand**: introduce the canonical destination model and constraints without deleting the source data.
2. **Migrate/backfill**: map and normalize legacy values deterministically into the destination.
3. **Reconcile**: verify aggregate counts, relationships, checksums where appropriate, and business invariants.
4. **Switch consumers**: only after reconciliation succeeds, move all runtime reads and writes to the canonical contract.
5. **Contract/cleanup**: remove temporary compatibility and legacy schema only after current reconciliation evidence and acceptance checks succeed.

New product behavior must not be added to the legacy path except when required to keep migration safe. The final runtime has one owner and one canonical path.

## Data Preservation and Failure Handling

- Every database change uses a new sequential forward migration; applied migrations remain immutable.
- Transformations must be deterministic and safely repeatable at the level required by the migration runner and recovery procedure.
- Reconciliation must prove preservation of relevant row populations, identities, relationships, and domain invariants before consumer cutover or destructive cleanup.
- If any source value cannot be mapped unambiguously, or reconciliation is absent, stale, incomplete, or failed, both consumer cutover and destructive cleanup stop. The operator receives bounded finding codes and aggregate counts, never raw rows, PII, credentials, or secrets.
- Unmappable data is neither discarded nor guessed. It must be corrected through an explicit reviewed remediation or recovery plan, then reconciliation must run again.
- Data may still be deleted under the system's approved retention, PDPA, legal-hold, or user-authorized deletion policy. Migration preservation must not become unauthorized indefinite retention or a duplicate sensitive archive.

## Compatibility, Deployment, and Recovery

Legacy runtime compatibility is not a default requirement. A temporary bridge is allowed only when a safe deployment or rollback needs it. The design must name its owner, exact scope, removal condition, and cleanup step before implementation.

After consumer cutover, old application versions are not assumed to work with the canonical schema. The approved cutover design records whether rollback remains safe, the schema boundary compatible with the prior version, and the point after which old binaries are prohibited. When backward binary compatibility would require retaining a harmful legacy contract, recovery uses a tested roll-forward path instead.

Temporary compatibility must not become an indefinite endpoint alias, field fallback, dual-read, dual-write, duplicate schema owner, or parallel business model.

## Verification

Migration work must use sanitized legacy-shaped fixtures and cover:

- successful mapping into the canonical model;
- preservation of counts, stable identities, relationships, and domain invariants;
- safe retry or idempotent reconciliation behavior;
- rejection of every known unmappable or ambiguous state;
- refusal of both consumer cutover and destructive cleanup when reconciliation is absent, stale, incomplete, or failed;
- successful consumer cutover after reconciliation with no dependency on the legacy path; and
- guards that prevent removed legacy contracts and fallbacks from returning after cleanup.

External rehearsals must use disposable or protected copies and report only bounded aggregate evidence. They must never expose production rows or sensitive values.

For this rules-only change, verification is limited to the documentation-policy test, `git diff --check`, final diff review, and clean status. No application behavior changes in this branch.

## Required Rule Changes

Update `.rules` without duplicating operational recipes:

- analysis and feature-design rules will require a long-term canonical model and prohibit shaping new architecture around legacy runtime compatibility;
- database migration rules will require deterministic data transformation, reconciliation evidence, and fail-closed handling of unmappable data;
- deployment rules will define phased cutover, bounded temporary compatibility, planned cleanup, and explicit rollback-versus-roll-forward decisions; and
- verification rules will require legacy-shaped fixtures, reconciliation failures, safe retry, consumer cutover, and post-cleanup legacy guards when those boundaries are affected.

`docs/TESTING.md` and `docs/OPERATIONS.md` will change only when implementation introduces a new durable command or operational procedure. This policy-only change adds neither.

## Acceptance Criteria

- `.rules` clearly separates removal of runtime compatibility from preservation of persisted data.
- No wording permits silent deletion, guessed transformation, sensitive logging, or indefinite duplicate storage.
- Temporary compatibility is bounded and carries an explicit cleanup condition.
- The policy favors one maintainable canonical structure after cutover.
- Existing migration immutability, security, PDPA, deployment, and recovery invariants remain intact.
