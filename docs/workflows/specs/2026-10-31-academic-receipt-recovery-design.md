# Academic receipt recovery through migration 149

Status: approved by the Senior after Data assessment on 2026-10-31. This records the approved
implementation contract; the existing authorization to finish integration and production
acceptance remains in force. Follow [the development rules](../../../.rules),
[Operations](../../OPERATIONS.md) and [Testing](../../TESTING.md).

## Problem and preserved meaning

The source-confirmed draft delete/reapply path before `91e23daad` could legitimately recreate
an offering with the same UUID. Consequently, migration 148's assumption that an accepted
receipt's created subset remains identifiable by current offering timestamps does not hold for
every possible legacy history. The cause of the three observed production mismatches remains
unknown. A stored created count `N` is an accepted historical fact; a smaller current
timestamp-matched count `M` does not establish that `N` is wrong.

Legacy requests had no explicit offering-date constraint, independently of their created counts.
Preserve every accepted receipt, identity, count, checksum, actual date and related fact. This
change supersedes the invalid transition premise through migration 149. Applied migrations 147
and 148, including their checksums, remain byte-for-byte unchanged. There is no manual tenant
remediation, SQLx-history patch, guessed timestamp, or restoration of an older writer.

## Responsibility and finite lifetime

The canonical School migration runner owns one transaction and one connection spanning the
pre-148 preparation, unchanged native migration 148, and migration 149 finalization. Migration
149 owns this finite protocol and its immutable evidence. Academic Core owns the resulting
reconciliation/readiness provider; the Pipeline consumes that provider through its existing
all-tenant gate. There is no alternate migration engine or continuing count-normalization path.

The temporary projection exists only inside this transaction. No external connection may observe
a committed version 148 or staging state from the version-147 recovery branch. All staging
columns, relations and helper objects are removed before commit. Permission synchronization and
runtime grants occur after the outer transaction commits and its connection is released; a
single-connection privileged pool must remain sufficient.

## Version-147 recovery branch

1. Verify the exact successful immutable migration prefix through 147, acquire the locks needed
   to keep the complete receipt cohort and preservation proof stable, and refuse unexpected
   receipt shapes, triggers, dependencies or staging-name collisions before projecting anything.
2. Admit only receipts whose sole migration-148 violation is mask 1 with `N > M` and
   `N <=` the distinct nonnull output cardinality. Every other migration-148 predicate must pass.
   All receipts participate in preservation; admission is not permission to rewrite only a
   convenient subset or bypass another refusal.
3. Capture the complete original receipt JSON multiset `S0`. Physically rename the original `N`
   column, retaining that column's identity, default and checks. Derive a temporary same-name
   `M` projection for migration 148; do not overwrite the original values or mutate another
   domain relation during preparation.
4. Reconstruct each original receipt from the projected shape and retained original column.
   Require bidirectional `EXCEPT ALL` equality, matching counts and SHA evidence for the complete
   multiset before entering migration 148. Hash equality alone is insufficient.
5. Run the actual unchanged migration-148 SQL through SQLx's native migration/savepoint path.
   Its audit truthfully describes the projected phase; it must not be presented as proof that
   original historical `N` equaled current `M`.
6. Migration 149 restores the original physical column and drops the temporary `M` column.
   Every original receipt JSON, excluding only the newly introduced `starts_on`, must equal
   `S0` by the same exact multiset/count/SHA checks. Every legacy receipt's new `starts_on`
   must be null. Preserve current offering dates and every other accepted field.
7. Reuse migration 148's `pg_temp.academic148_snapshot` before and after restoration to prove
   that restoration changed no nonreceipt relation in its complete transitive foreign-key
   graph. Do not copy or invent a second graph or snapshot algorithm.
8. Publish migration-149 immutable bounded evidence linking original, projected,
   migration-148-audit and final phases. Remove all staging/helper objects and commit only
   after all final reconciliation checks succeed.

Any failed admission, dependency check, equality proof, audit, restoration or cleanup rolls back
the whole version-147 transaction. Never commit the projection to make progress or mark a failed
proof as passed.

## Existing version-148 and new-tenant branches

An exact verified version-148 database needs no receipt projection. Migration 149 preserves its
receipt JSON exactly, including legitimate nonnull `starts_on`, and records truthful evidence for
that branch rather than claiming a version-147 preparation occurred. Do not apply an all-null
legacy assertion to this branch.

Fresh provisioning uses the same canonical runner and transaction boundary at the 147-to-149
transition. It must reach the ordinary complete version-149 schema without a special provisioning
bypass. A completed 149 replays without reprojecting receipts or reinterpreting later business data.

## Readiness, failure and delivery

Academic context readiness at version 149 requires the existing five checks plus
`ACADEMIC_CONTEXT_RECEIPT_RECOVERY_EVIDENCE`: valid immutable migration-149 evidence and absence
of all staging/helper objects. Context status reports pending below version 149; update the
provider and consumer together from version 148/five checks to version 149/six checks. It does
not continually compare historical `N` against mutable
current `M`. The exact compiled count/version/checksum fence continues to reject incompatible
schemas and prior binaries.

On cancellation or connection loss, a fresh connection reconciles committed history and phase
evidence. An ambiguous commit may have left the recovery branch at 147 or at 149; never infer an
outcome from a disconnected client, blindly repair history, or expose a projected state. Investigate
any inconsistent or unsupported state while maintenance remains active. Post-commit permission or
grant failure retains the committed migration outcome and follows the canonical reconciliation path.

Delivery retains the shared production lock, universal maintenance, exact prepared artifacts,
all-tenant migration/readiness checks and authenticated acceptance. Recovery rolls forward with a
compatible binary; old binaries remain prohibited. The receipt observer remains read-only evidence,
not an authority to authorize a tenant-specific repair. Another diagnostic-only production cycle
is not a prerequisite for this approved source-confirmed transition correction.

Keep the registered observer explicitly bounded to historical schemas 147/148. Its preparation
must recognize and validate the trusted 001–149 source inventory while deriving the unchanged
001–148 checksum prefix required by that historical query. Document refusal on a version-149
database; do not silently break manifest preparation when 149 is added or make the observer a
business fallback. This compatibility of diagnostic preparation does not authorize another
production observation or an expansion of the query's accepted schemas.

## Acceptance criteria

Fixtures must reproduce the legitimate historical same-UUID delete/reapply path, demonstrate the
original 148 refusal, and prove exact preservation through 149. Cover every admission/refusal and
history/dependency/trigger drift; replay; already-148 receipts with nonnull dates; and fresh
provisioning with a one-connection pool. Inject failure/cancellation after projection, after native
148 and inside 149. A separate connection must never observe committed 148 or staging on the
147-to-149 path. Missing, malformed or inconsistent 149 evidence must fail the sixth readiness
check. Independent Review and QA accept the complete frozen candidate before integration and
selected production acceptance.
