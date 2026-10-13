# PostgreSQL VPS cutover design

Approved in the task on 2026-10-13. Preserve Admin and both tenant databases,
identities, encryption keys and immutable SQLx history. Do not rebaseline.

PostgreSQL 18.6 on the database-only VPS replaces Neon. Backend production remains
rootless Podman on its existing host. Require verify-full TLS and backend-only
database ingress; human access uses SSH. Keep pgBackRest 2.59.3 encrypted R2 backup
chains and WAL; prove restoration of the migrated data before reopening.

Admin owns PostgreSQL provisioning, encrypted per-tenant owner/runtime credentials
and private credential delivery. Public Admin responses never contain credentials.
Runtime roles cannot migrate; provisioning/deployment alone invoke the canonical
tenant migration runner. Check resource ownership before compensation or deletion.

New business identities use UUIDv7; preserve existing UUIDs, deterministic UUIDv5
and security token generation. Academic year/term audit uses PostgreSQL OLD/NEW.
Bound caches, session cleanup, logs and connection use. Monitor with pgAdmin/SSH,
pg_stat_statements and privacy-preserving application timings, without another stack.

Cutover runs under the production lock and Maintenance Mode, stopping all writers.
Restore without source ACL/ownership, reconcile data and migration history, then
switch consumers. Missing/stale reconciliation blocks switching and cleanup.
Before target writes, restore the source-compatible release/config if necessary;
after target writes, recover/roll forward on PostgreSQL. Retain the source without
application writes for at least seven days; source deletion is a separate operation.
