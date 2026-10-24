# Shared tenant database runtime implementation

1. Extend the existing tenancy executor with all five fixed job kinds, same-key phase guards and safe duration observations; make PoolManager its single composition owner without altering physical reservations or drainage.
2. Route Calendar hydration, recipient selection and direct acceptance through the held advisory connection; preserve transactions, partial success and cancellation disposal. Add a one-connection reminder regression.
3. Split Notification preparation, transport and completion, retaining durable claims/source guards and token fencing. Release admission and pool leases before external I/O, fetch subscriptions once, and bound endpoint concurrency/timeouts. Add slow-transport regression cases.
4. Integrate Attendance (`AttendanceMaintenance`) and File Platform through their domain owners. Reuse one connection for evidence authorization, one typed preview context and a read-only history/report snapshot with settings loaded once; retain the existing finalization guard and batch runtime days. Fence every File Platform worker database effect by typed lease owner, attempt and expiry, preserving synchronous upload/delete behavior. Add cleaner sweep exclusion and bounded fair tenant continuation; retain soft budget semantics while checking after completed external operations and starting each tenant budget at first admission. Bound DB phases inside the lease budget and preserve the narrow cancellation-owned durable cleanup exception without adding crash-recovery schema.
5. Update `.rules`, Testing and Operations for the shared owner and phased lease requirements. Run scoped formatting, tenancy/notification pure tests, focused disposable database suites and the required architecture/documentation checks under the Senior's serialized build slot.
6. Freeze the integrated candidate for independent review and QA; repair assigned findings. Senior owns Git and final handoff. Open a Draft PR early and push reviewable checkpoints under Senior ownership; do not merge, deploy or run production/load tests.

Focused validation recipes: run the existing package suites for `school-tenancy`, `school-calendar`, `school-notifications`, `school-attendance` and `school-file-platform` using `./scripts/test_backend_school.sh --package <package>` for database cases. Add focused filters for single-connection evidence/reminders, preview/history/report settings reuse, guarded finalization, batched maintenance and stale File Platform lease transitions. Run `./scripts/pipeline verify --scope documentation` for canonical documentation and the existing backend architecture checks. Senior assigns the sole Cargo/fixture slot and exact filtered commands before execution.


## Expanded query-boundary implementation

1. Preserve the unchanged 6943dd5d baseline evidence and previous shared-admission changes. Implement `school-database` with SQLx/tracing only, workspace membership and no version upgrades. Compile a public API probe for raw rows, typed rows, scalars, binds and native QueryBuilder adapters before dependent source migration.
2. Add tenancy's observed checked-out connection guard retaining the originating logical lease. Keep pool reservation/drain/cancellation semantics and query-only observations in the lower crate.
3. Migrate the approved production PostgreSQL consumers by assigned nonoverlapping owners using the stable connection-bound API. Keep SQL meaning, authorization, transaction/snapshot/advisory locks, recipient snapshots and domain error mapping intact; record justified native-SQLx exceptions rather than bypassing safe typing.
4. Verify type-safety and rejection of alternate executors/concurrent connection reborrows, and use disposable root integration fixtures for query results, rollback, cancellation observations and safe trace fields. Re-run the existing Attendance/Calendar one-connection regressions through the actual new API.
5. Update canonical rules/testing/operations and architecture guards for dependency direction, query boundary and documented exceptions. Compare before/after compilation on pinned 1.98.1 with the shared cache and jobs4; report actual evidence without attributing cache effects to the crate split.
6. Freeze each coherent API/migration checkpoint in Draft PR 171 under Senior ownership. Independent review and QA remain required; do not merge, deploy, run production/load tests or upgrade dependencies.

7. Close the reviewed control-plane/admission gap: issue opaque current-cache work
   snapshots outside DB admission, validate locally before/after nonmigrating pool
   acquisition, and retain each original phase/item deadline. Do not fall back to HTTP
   or replay SQL under a stale proof. Cover queued fences, same-UUID credential rotation,
   expiry/eviction/cancellation, real physical capacity waiting, endpoint/ack and file
   metadata suppression using the actual managed orchestration. Distinguish local
   revision proof guarantees from atomic directory/SQL serialization.
8. Keep private query/hydration helpers connection-only, with exact documented domain
   transaction/snapshot work-entry scopes. Run macro/alias/cfg-aware syntax guard negative
   cases plus actual root fixture and package checks on the final frozen candidate;
   standalone guard diagnostics are development evidence, not final Cargo acceptance.
