# Database concurrency implementation

Scope and architectural direction were approved by the user and Senior in chat; see the corresponding approved design.

1. Tenant/runtime writer: implement bounded AdminClient directory cache, stale-fill rejection and authenticated mutation fencing; cover TTL, bursts, outages, disable/credential changes and token races.
2. Tenant/runtime writer: implement validated aggregate runtime/privileged reservations, bounded admission, safe idle eviction and cancellation-safe drainage; refresh retained Attendance handles. Cover physical aggregate usage, overload, cancellation and recovery.
3. Domain writer: change Attendance reports to shared guards in a read-only repeatable-read snapshot; add concurrent report/write fixtures.
4. Domain writer: add migration118 prefix counter, shorten enrollment locks and move bounded password hashing outside transactions; add allocation/rollback/lifecycle fixtures.
5. Tenant/runtime writer: update durable rules, operations and focused testing recipes, preserving the single-process topology and fail-closed fence recovery.
6. Senior: serialize Cargo ownership, save reviewable checkpoints and maintain one draft PR. Freeze the integrated candidate for independent Reviewer and QA, repair findings, run affected architecture/workspace/database/smoke checks, and follow the authorized integration/deployment gates.

Writers deliver edits in place without Git mutations. The Senior owns manifests/locks, shared Git and build scheduling. Tests use the owned disposable PostgreSQL runner and pinned shared Rust toolchain; no production fixtures or additional infrastructure.
