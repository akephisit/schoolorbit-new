# Shared tenant database runtime

Approved scope: one School process owns background database admission in `school-tenancy`, composed by `PoolManager`. Calendar, Auth retention, Notification dispatch, Attendance maintenance and File Platform reconciliation share the existing default two turns, clamped to feasible resident pool capacity. This does not change pool sizing, admission timeouts, schedules, business retention or notification durability.

`PoolManager::background_turns()` returns the process owner's executor. A typed job kind supplies fixed safe observation labels. Existing `run` admits a complete database-only turn. A same-key `begin` guard spans phased work; its `run` admits individual database phases and releases the global permit before external I/O. Domain providers retain all SQL, transactions, source guards, progress and fencing. No scheduler stores idle pools, unbounded pending futures or overlapping cron sweeps.

Calendar hydrates events, resolves recipients and accepts each direct notification on its already-held advisory-lock connection. Per-recipient transactions and partial-success behavior remain unchanged. The lock is refetched after acquisition and released only after fanout; cancellation or an unconfirmed unlock closes the session.

Notification dispatch claims at most 32 jobs, prepares under admission, releases pool leases before bounded external delivery, and reacquires for invalid-endpoint cleanup and token-fenced acknowledgment. Four item futures retain a five-second item deadline. Push subscriptions are fetched once and attempted concurrently with a fixed endpoint bound and timeout. The durable attempt returned by claim rotates the initial endpoint position so a repeatedly stalled prefix cannot starve the tail. Failed work remains durable for retry. Source validity, exact ledger identity, recipient snapshots, physical deletion and missing-inbox suppression remain enforced.

Cleaner uses a bounded completion-driven tenant queue, one reconciliation batch per continuation, same-key exclusion and a whole-sweep guard. Its existing 120-second budget starts at first admitted work and is checked after completed operations and between batches. Database phases time out within the existing lease budget, while current external operations finish before yielding. File Platform remains the authority for leased state transitions and fences stale completions.

Observations expose fixed operation/work labels and admission, checkout, connection hold and logical lease durations where the typed boundary owns them. Database URLs, SQL parameters, endpoint credentials and personal data are excluded.

Verification covers all-kind admission/cancellation, phase release during slow external I/O, one-connection Calendar fanout, bounded slow endpoint delivery, durable notification/source behavior and File Platform stale-lease transitions using disposable fixtures. Draft PR creation and checkpoint pushes are authorized. Merge, deployment, production and load work remain prohibited.

Attendance uses the `AttendanceMaintenance` kind. Evidence authorization holds one connection through its guard and operation. Preview uses a typed context once; history and reports reuse one read-only snapshot and settings load. Finalization uses its already-held guard, and runtime maintenance batches days. File Platform worker effects use typed lease owner, attempt and expiry fences for every database effect; synchronous upload/delete paths preserve their existing behavior. Prepared external-I/O values own their data and retain neither a pool nor a transaction. Finish phases resolve the immutable tenant UUID before applying acknowledgment or endpoint deletion, rejecting a remapped tenant.

Fenced File Platform finish has one explicit cancellation-owned cleanup exception: a successful or uncertain derivative PUT from the exact cancelled attempt may queue a new durable delete while its file is being deleted; metadata remains revoked and newer/expired/reclaimed attempts remain untouched. Process death or task cancellation before finish may leave an externally committed object without a durable in-flight PUT marker. A separate persisted upload-intent/reconciliation protocol is tracked in TODO; this resource-management change adds no schema or crash-recovery protocol.


## Approved connection-bound query boundary

The expanded approved scope adds `school-database` as the lower PostgreSQL query-execution owner. `DbSession` borrows one `&mut PgConnection`; connection-bound query, row and scalar builders retain that exclusive borrow through execution, and accept no replacement executor. SQL, binding values, authorization, transaction/snapshot/lock boundaries and error mapping remain domain-owned. There is no pool acquisition, automatic transaction or automatic write retry in this crate.

The API preserves SQLx 0.9 `SqlSafeStr`, `Type`, `Encode`, `FromRow`, typed binds and `sqlx::Error`. Native QueryBuilder results use prepared adapters rather than reconstructing SQL or weakening safe-string checks. Streaming is added only for a verified consumer need. Operation names are static source-owned labels. Query duration/status/error-class observations exclude SQL text, parameters, database URLs, provider error text and personal data; cancelled executions produce a cancellation observation through an owned drop guard.

`school-tenancy` remains the connection/pool/logical-lease and background-admission owner. An observed checked-out guard retains both its SQLx connection and originating logical lease, measures acquire/hold, and lends a connection to `DbSession`. `school-database` depends only on SQLx and tracing, with no tenancy, error, migration or domain backedges. This crate is justified by the concrete connection-bound execution contract shared by workspace consumers; no generic business repository or transaction manager is introduced.

Implementation begins with a compiled public API probe and focused type/cancellation/rollback/trace regressions before migrating production consumers. Existing one-connection Attendance and Calendar regressions will exercise the new API. QA recorded the unchanged 6943dd5d warm workspace baseline at 0.59 seconds; this is a same-environment comparison input, not a speedup claim. New membership/dependency edges are captured through Cargo without version upgrades, and the same toolchain/cache/build concurrency is used for the after check. Root disposable integration fixtures avoid a development dependency cycle through test-db, migrations and this new lower owner.


Owned pool transactions use `ObservedTransaction`, retaining the origin lease before
beginning and measuring the combined checkout/BEGIN wait plus the owned hold. Explicit
consuming commit/rollback releases transaction ownership without a borrowed checkout
surviving it. Pool adapters are declared work entries; helpers reuse a connection or
borrowed transaction. Pure cache/direct-grant/denied/empty/validation paths stay before
checkout. Repository construction under a borrowed transaction is permitted only as
an inert factory, with all operation database effects using that transaction.

The tracked syntax guard scans production module graphs in default and api-docs
configurations, including negative feature gates and supported async join macros.
It rejects executor-bearing native execution, hidden opaque database macros, native
pool entry outside exact infrastructure/bootstrap functions, and pool helper calls
while an observed connection/transaction is live. Schema identity/history/privilege
setup has function-scoped exceptions; permission synchronization uses `DbSession`
with a cycle-required native transaction boundary. Query observations are client-await
durations and cancellation lifetimes, not server execution/cancellation guarantees.


Control-plane resolution is outside shared database admission. Notification and Cleaner
phases use opaque `ResolvedTenantDatabase` proofs issued from one current cache value
and its revision/expiry atomically. Every successful fill changes the value revision,
including same-UUID credential rotation. Admission uses local proof validation before
and after the nonmigrating physical pool acquisition; an expired, evicted or fenced
proof defers without an HTTP fallback or implicit query retry. Original whole-phase and
whole-item deadlines include resolution and all admission/pool/SQL waits. Logical
same-key exclusion survives external I/O while DB permits and leases are released.
The post-acquisition proof closes the queued stale-work window without claiming atomic
serialization between directory mutations and later SQL. Domain fences remain intact.

Managed disposable regressions exercise slow loopback Admin HTTP and actual other-kind
SQL with a sole shared permit, cancellation without mutation, queued UUID/credential
proof invalidation before endpoint cleanup/ack or file metadata finish, TTL/eviction,
and a real physical capacity wait followed by revision invalidation. Fixed safe phase
observations coordinate these tests; the actual production resolver/phase/completion
algorithm runs unchanged. Test-only cache capacity/TTL configuration is feature-gated.
