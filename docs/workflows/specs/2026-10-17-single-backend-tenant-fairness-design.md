# Single School process and tenant fairness

Approved scope: the user authorized implementation on 2026-10-17 after the architecture survey. School runs exactly one backend-school process permanently. Process-local identity, permission, directory and realtime coordination are supported architecture; adding replicas is outside this design.

## Owners and invariants

`school-notifications` owns SSE transport. A notification or work broadcast overrun emits `reconcile_required` with `{}`. The browser's existing central notification store reconciles authoritative HTTP state on that signal and reconnection. Session or permission channel loss still fails closed. Existing tenant/user filters remain authoritative.

`school-tenancy` owns runtime directory coalescing, admission and stale-fill fencing. A flight shares its success or failure with callers already waiting for that flight. Failed results never become persistent cache entries; a later independent request can retry immediately. A dropped leader completes its cohort with a safe unavailable result and releases its admission reservation. Distinct-key fills share a process-wide bound and admission deadline. Mutation begin/finish uses exact operation tokens, and revision/entry identity prevents old or evicted fills from publishing credentials. Owner credentials never enter the cache.

The initial directory defaults are four admitted distinct-key fills and a one-second admission timeout. Their environment names and validation belong in `AdminClientConfig`, separate from the existing success TTL/capacity and HTTP retry settings.

The proposed bounded turn helper belongs in `school-tenancy`, where tenant runtime admission and lease lifecycle already live. It exposes execution admission, timeout and per-tenant/job-kind exclusion only. Domain work, SQL, durable progress and retry meaning stay in Calendar and Auth. No new crate is needed: a separate crate would expose a similarly sized API without a separate lifecycle or compile boundary. Final helper API and limits are coordinated with the Senior before scheduler edits.

Calendar processes a bounded number of attempted reminders per tenant turn rather than draining a tenant's whole backlog. Existing reminder rows remain the durable work owner. Failed work remains pending and later turns revisit it fairly. Its advisory-lock connection is marked for physical closure on cancellation before attempting the session lock; successful explicit unlock can restore normal pooling. Preserve recipient snapshots, all-failed retry, partial-success completion and missing/deleted event behavior.

Auth retains bounded 100-row `SKIP LOCKED` retention operations and the existing retention policy. Rotation across tenant sweeps must reach later tenants when earlier work exhausts the budget. Progress remains Auth-owned, while existing database rows determine remaining durable retention work. Background turns retain their `TenantPool` lease across all database operations and hold no idle registration leases.

## Scope and verification

Preserve pool reservation accounting, Behavior locks, retention policy, Calendar recipient/partial-success policies and existing central notification, Attendance and File Platform worker concurrency. No migration is currently planned. Any schema requirement returns to the Coordinator before writing migrations.

Verification includes stream overflow/tenant isolation and fail-close tests; directory outcome sharing, cancellation, admission bounds, mutation and eviction tests; controlled background fairness, timeout, no-overlap and lease lifetime tests; and Calendar/Auth disposable PostgreSQL coverage for the changed execution boundaries. SQL/source assertions alone are not runtime evidence. The final candidate also runs owning formatting, architecture, compilation and documentation-policy checks. Deployed proxy/device receipt and production capacity remain separate acceptance evidence.
