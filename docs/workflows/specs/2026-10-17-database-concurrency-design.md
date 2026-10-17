# Database concurrency

The user explicitly approved implementation of four improvements in chat: cache the tenant directory, enforce an aggregate connection budget, make Attendance reports nonblocking, and shorten Admission critical sections. The Senior approved this design before architectural implementation.

## Tenant directory

`school-tenancy::AdminClient` owns a shared process-local cache of immutable tenant UUID and runtime database URL. Cache only successful lookups, for 30 seconds by default, at most 1024 entries. Configuration is bounded and validated. Never cache owner credentials, serve expired data during outages, or print credentials. Per-entry gates coalesce misses. Every fill verifies its entry identity and revision after awaiting the directory; eviction, invalidation and mutation fences reject obsolete results.

Admin status changes fence the subdomain through the authenticated School internal API before changing the control-plane row. A UUID identifies the mutation. Begin invalidates and blocks lookups; matching finish invalidates again and unblocks only after the write's outcome is known. One current token serializes mutations per subdomain; repeated current begin and completed finish are idempotent. Mismatched completion cannot release another operation. Active fences do not expire or get evicted. Cancellation or uncertain completion remains fail-closed; operators reconcile the writer/row and retry finish with the recorded token. Admin migration011 adds a durable pending operation row before School begin. Internal get/list exclude pending schools. The mutation transaction claims the exact token by DELETE RETURNING before writing the school row and commits both together. Recovery claims the same row lock, so a late writer cannot commit after recovery removed its token. Across School restart, the durable pending row blocks cold directory reads until the final state commits; an absent local finish token safely invalidates, while a different active token is rejected. Direct SQL changes require maintenance/restart or the same fence protocol. No credential rotation endpoint is introduced.

## Connections

Use public SQLx pool APIs and reserve the entire maximum of each admitted tenant pool. Default runtime capacity is 40, per-tenant maximum 5, with a five-second admission deadline. Requests, schedulers and runtime status use the same manager. An owned TenantPool lease protects logical operations, including gaps between queries. Dormant SSE/WebSocket sessions and Attendance registrations release their lease and reacquire for validation or scheduled work. Admission pressure closes a least-recently-used pool only when no logical lease or checked-out connection exists; it never cancels checked-out work or replays writes. Reservations include initialization and retirement, and are released only after the pool physically drains. Cancelled initialization/retirement retains tracked ownership until safe drainage. Stale closed clones cannot create new connections. Attendance refreshes its runtime pool from the directory/manager before scheduled work.

The supported topology is one School process and one Admin process per database server. At PostgreSQL's default 64 connections, allocate School runtime40 + Admin runtime5 + Admin provisioning2 + School privileged5 + operator/headroom12. School provisioning and migration pools share a maximum-five privileged reservation. Multiple replicas must divide these allocations or use a separately reviewed shared pooler; this change adds no service.

## Attendance

Reports establish a read-only repeatable-read snapshot before reads and acquire shared module/lifecycle/term guards. Report reads do not take writer/exclusive attendance locks, while writers preserve existing atomicity and access policy. Verify report/write concurrency and snapshot consistency with disposable database fixtures.

## Admission

A new forward migration118 supplies an atomic high-water application-prefix counter. Enrollment retains the shared academic-year guard and locks only the allocator necessary for numbering. Auto-generated-credential password hashing runs in bounded blocking work inside the narrower allocator transaction when its input depends on the allocated code. Preserve enrollment identities, numbering and rollback behavior; verify competing enrollments and lifecycle exclusions.

## Rollout and verification

`SCHOOL_PRIVILEGED_CONNECTIONS` is validated at five or greater so provisioning can hold its transaction and perform its existing follow-up read without self-deadlock. One-shot schema verification uses a directly owned raw connection that closes before its reservation is released.

No public frontend contract changes or new infrastructure. Forward migration118 is additive; normal maintenance and migration gates apply. Operational configuration and recovery belong in `docs/OPERATIONS.md`; focused commands belong in `docs/TESTING.md`. Independent review and QA cover the integrated candidate, including cancellation, burst, invalidation, tenant isolation, aggregate limits and transaction races.
