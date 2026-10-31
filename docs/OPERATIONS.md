# Operations

## PostgreSQL VPS cutover and recovery

The database-only Debian 13 host runs PostgreSQL 18.6 and pgBackRest 2.59.3 as
native systemd services. Backend services remain on the existing rootless Podman
host. Production allows database ingress only from the backend address; operators
use SSH. All backend connections use SCRAM and `sslmode=verify-full` with the
private deployment CA. The CA signing key stays on the operator laptop.

Preserve the immutable migration timeline, existing identities, password hashes,
national-ID ciphertext, blind indexes, R2 references and encryption/authentication keys.
The Neon transfer completed on 2026-10-13. Admin's forward migration 009 removes
only the transfer reconciliation columns after migration 008 retired source credentials.
The one-time transfer CLI, partial migrator and Operations cutover job are retired.
After migration 009, Admin images that expect schema 008 are prohibited; recover
with a schema-009-capable release or roll forward. Never bypass SQLx history checks.

### Identities, provisioning and owner jobs

- Admin runtime: `schoolorbit_admin_runtime`, DML-only on the Admin database.
- Admin migration: `schoolorbit_admin_owner`, used only by explicit migration jobs.
- Provisioner: `schoolorbit_provisioner`, non-superuser with CREATEDB/CREATEROLE.
- Tenant owner/runtime: `so_<tenant-uuid-without-dashes>_owner` and `_runtime`,
  distinct random passwords, no SUPERUSER/CREATEDB/CREATEROLE/BYPASSRLS.
- Operators: separate observer (`pg_monitor`) and human administration accounts,
  restricted to SSH-forwarded loopback access.

Admin persists its tenant UUID and both encrypted passwords before resource
creation. AES-256-GCM authenticates tenant/purpose/version under the independent
64-hex-character `DATABASE_CREDENTIALS_KEY`. Preserve this key in the recovery
secret store: physical database backups cannot decrypt credentials without it.
Check role markers and database owner before adoption or cleanup; ambiguous creates
retain a failed school record. Public API/SSE payloads contain no credentials.

The Admin image runs `/app/backend-admin --migrate-admin` as a one-shot job with
`ADMIN_MIGRATION_DATABASE_URL` before starting the ordinary service. Runtime startup
validates SQLx history without applying migrations. Tenant provisioning/deployment
alone runs `school_migrations::run_tenant_migrations` and reconciles runtime grants.
Owner pools drain before runtime/scheduler use. Each deployment checks the complete
SQLx history through one directly owned runtime connection; its verified timeline
also supplies the version without another query or pool hand-off. Connection setup,
verification and closure share a 30-second deadline. This verification never borrows
a cached request pool. Deployment retires pre-migration pools before creating verified
pools and registering runtime work. Retirement prevents further acquisitions and
releases the tenant creation gate before waiting up to 30 seconds for borrowers.
Failure keeps maintenance active; it never means a transaction was rolled back.
Normal requests fail closed when
the tenant schema does not match the release.

### Recovery after the completed transfer

The application has accepted PostgreSQL writes. Recover or roll forward using the
retained PostgreSQL release artifacts and encrypted pgBackRest backups with WAL;
never reconnect an application to stale Neon data. Future database moves require a
new reviewed rehearsal and cutover plan rather than reusing retired transfer tools.

The owner explicitly ended source retention on 2026-10-13 after actual-data PITR
acceptance and a fresh PostgreSQL backup/check. The retired source, compatibility-test
and older SchoolOrbit Neon projects and source-only recovery credentials were deleted.
Recovery now depends on PostgreSQL backups/WAL and the current private key store;
there is no source-provider fallback.
Delete redundant transfer dumps, rehearsal copies, temporary SQL/configuration and
logs after verifying the actual-data PITR result and preserving current recovery
secrets. Retain the CA signing key, pgBackRest repository cipher password,
`DATABASE_CREDENTIALS_KEY`, current business/session/storage keys and operator
credentials in the private recovery store. Physical backups cannot replace these keys.

Native Podman migration jobs decode installer dotenv encoding through
`scripts/lib/pipeline-remote/runtime-env.sh`; pass only the owner credential by
environment name. Never pass the Compose dotenv file directly to Podman's `--env-file`.
Production changes still use the shared lock, exact Pipeline build gate, Maintenance
Mode, canonical migrations and authenticated proxy/browser acceptance.

### Workload and retention

Start with the tracked `scripts/lib/postgres-vps/workload.conf` budget for 2 vCPU /
4 GiB: 64 connections, 512 MiB shared buffers, 4 MiB work memory, 128 MiB maintenance
memory. The supported topology is one School process and one Admin process per
PostgreSQL server. The default connection allocation is School runtime 40 + Admin 5 +
Admin provisioning 2 + School privileged 5 + operator/headroom 12 = 64. Runtime pools
reserve their complete per-tenant maximum (ten by default), so at most four tenant
pools are resident. This gives each active tenant more query concurrency while
reducing the number of resident pools within the unchanged 40-connection budget.
Logical operation leases protect the gaps between queries;
idle LRU pools are evicted under pressure only after all leases and borrowers end.
Admission waits at most five seconds, then returns 503; writes are never replayed.
Draining and cancelled pools retain their allocation until physical closure.
Dormant Attendance registrations and realtime sessions reacquire a lease for work.
School owner/provisioning pools share the separate privileged reserve; Admin
provisioning uses a global limit around directly owned connections. Pools have zero
minimum and five-minute idle expiry; migration is sequential.

The examples and both Compose files expose `TENANT_RUNTIME_CONNECTIONS`,
`TENANT_POOL_MAX_CONNECTIONS`, `TENANT_POOL_ADMISSION_TIMEOUT_MS`,
`ADMIN_POOL_MAX_CONNECTIONS`, `PG_PROVISIONING_MAX_CONNECTIONS`,
`SCHOOL_PRIVILEGED_CONNECTIONS`, `POSTGRES_MAX_CONNECTIONS`, and
`POSTGRES_CONNECTION_HEADROOM`. `SCHOOL_PRIVILEGED_CONNECTIONS` must be at least
five, including provisioning's transaction and follow-up read. School startup rejects allocations exceeding the
server budget. Keep the server setting and values in both services synchronized.
Run exactly one backend-school process on one VPS; do not add School replicas or
overlap live School processes. These allocations are process-local. Per-operation
work memory can multiply across
sort/hash operations, connections and parallel workers; these are initial budgets,
not a guarantee of fixed RAM consumption. Keep autovacuum and worker AIO enabled.

School bcrypt work shares the [password worker](../backend-school/crates/school-crypto/src/password_work.rs)
across authentication, enrollment, student/guardian and staff creation, and tenant provisioning.
It admits at most four blocking jobs per School process; excess callers wait at most one second
before a safe 503. There is no environment override or automatic mutation retry. Cancelling a
queued request removes its wait, while an already-started hash/verification retains its slot until
it finishes. This protects async executor responsiveness and limits CPU concurrency; it does not
promise lower hash latency. Existing costs and legacy truncation rules remain unchanged. Login
revalidates the verified credential/account snapshot under a user row lock before session insertion,
so a concurrent password change or disabled/deleted/renamed/retyped account rejects stale login.
Admin has its own application-owned budget. Keep the supported single-process topology; additional
processes multiply these independent CPU limits. Diagnose overload using safe status/reason codes;
never record plaintext passwords or database URLs in observations.

Tenant directory lookups cache only successful UUID/runtime credentials, for 30
seconds and at most 1024 entries by default (`TENANT_DIRECTORY_TTL_SECONDS`,
`TENANT_DIRECTORY_CAPACITY`). At most four distinct directory fills run concurrently
(`TENANT_DIRECTORY_MAX_IN_FLIGHT`, supported range 1–64); excess fills wait up to
one second (`TENANT_DIRECTORY_ADMISSION_TIMEOUT_MS`, 10–30000 ms) before a safe
admission error. Waiting callers share the active fill's success or failure; a later
independent call retries failures immediately. Cancelling a leader wakes its cohort
and releases its slot. Mutation/eviction fences are checked again when followers
consume a completed fill. Expired lookups fail closed during Admin outages;
owner credentials never enter this cache. Status changes first persist an Admin
`tenant_directory_mutations` barrier, then fence the School cache. The exact token
is claimed and removed in the same transaction as the school mutation; School
finish invalidates before reopening. Pending barriers fail the internal all-tenant
list as a whole, preventing incomplete migration acceptance.

Notification and Cleaner database phases resolve control-plane metadata before shared
DB admission. `AdminClient::resolve_database_work` issues an opaque work snapshot from
one current cache value, revision and expiry under the cache lock. Every successful
fill changes its value revision, including same-UUID credential rotation; mutation,
eviction or expiry invalidates older proofs. Inside admission,
`pool_for_resolved_work` validates the proof and expected tenant UUID locally before
and after the nonmigrating PoolManager acquisition. It never refetches HTTP or retries
a failed proof inside admission. Never-consumed Notification work defers to a bounded later probe; consumed presentations never replay.
Notification DB phases each have a five-second deadline and no whole-item transport deadline;
Cleaner phases retain `min(5 seconds, lease / 4)`. These DB deadlines include resolution,
admission, pool waiting and database work. The logical same-key guard spans network phases; database permits, pools and checked-out
connections end before external delivery/storage I/O.

The second local validation covers a mapping/credential change during a physical
pool wait. It does not make directory mutation and subsequent SQL atomic; the existing
domain transaction/source guards remain authoritative; Notification adds irreversible presentation
consumption and exact device-version cleanup, while Cleaner retains its claim-token/lease fences. A
fixed `tenant_physical_budget`/`waiting` event identifies contested physical admission;
its completion/cancellation event retains the separate admission duration. No event
contains credentials or a database URL.

PostgreSQL query execution uses the connection-bound `school-database::DbSession`
provider. Domains retain SQL, transactions, snapshots, locks and typed error mapping;
the provider neither acquires pools nor retries writes. Its fixed operation labels,
duration, status and safe error class identify query work and cancellation without
logging statements or values. Tenant observed checkout guards separately measure
connection wait/hold and retain logical eviction protection. A query duration excludes
pool admission; a connection hold includes gaps between that work boundary's queries.
Query duration is the client await, including driver/network work and decoding; a
cancelled client future does not prove PostgreSQL has already stopped the query.
Owned transactions use tenancy's `ObservedTransaction`: `begin_wait_us` includes
checkout plus `BEGIN`, and the owned hold/lease ends after consuming commit, rollback,
or drop. No borrowed checkout survives a successful consuming commit. Request-side
physical-budget admission is measured separately from background-turn admission,
checkout, transaction begin and query duration.
Use these observations together with PostgreSQL aggregate statistics, and never enable
raw SQL/parameter logging to diagnose saturation.

Native execution is limited to schema identity/history and privilege setup:
`school-migrations::{run_tenant_migrations,verify_tenant_schema,grant_runtime_privileges}`
and the migration handler's `get_current_version`, plus SQLx execution inside the
connection-bound provider itself. Permission reconciliation is application SQL: both
its deactivation and typed bulk upsert execute through `DbSession`; only its native
pool transaction entry remains bootstrap-owned to avoid a tenancy/migrations cycle.
Disposable fixture/fault setup is separate from this production inventory. Native
QueryBuilder construction is allowed; executing its result uses a prepared adapter.

Calendar, Auth retention, Notification dispatch, Attendance maintenance and File Platform
reconciliation share `TENANT_BACKGROUND_MAX_TURNS` database admission (default
2, range 1–64), clamped to `max(1, floor(runtime connections / per-tenant maximum) - 1)`.
This leaves a feasible resident tenant slot with ordinary allocations; when the
physical budget fits only one tenant, the minimum one background turn shares that
slot with foreground work. It does not reserve exclusive foreground sockets.
The single `PoolManager` owns the executor. Each owner bounds its tenant futures and
excludes overlapping turns for the same tenant/job kind. A logical turn may span
external I/O, but releases its shared database permit and logical pool lease before
network delivery. Only owned payloads cross that boundary; completion resolves the
same tenant UUID; Notification cleanup validates the exact device version, while File Platform
validates its durable claim. Independent item database
phases each consume a permit. No idle scheduler pins a pool. Fixed work/operation
labels report admission, phase hold, checkout and logical lease durations without
SQL parameters, database URLs, endpoint credentials or personal data.

Calendar's daily 07:00 Bangkok invocation has a process-local sweep guard; a cron
tick skips while the prior invocation is still running, preventing accumulated
waiting queues. It requeues unfinished tenants behind other
waiting tenants as each turn completes. A stalled tenant does not hold up healthy
backlog revisits in another available slot. Each turn attempts at most 200 reminders and yields after a ten-second soft budget checked
between reminders. A recipient fan-out finishes before yielding; an unusually slow
reminder can exceed that budget. A stable `(remind_on, created_at, id)` keyset advances
past failed or contended rows for this invocation, then resets on the next cron
invocation. Healthy due backlogs continue in the same invocation. New rows behind
the cursor are picked up next invocation. An interrupted lock attempt or failed
unlock closes the advisory-lock session instead of returning it to the pool.
Calendar event hydration, recipient snapshot resolution, bulk inbox acceptance and reminder
`sent_at` update use one transaction on the advisory-lock connection, including with a one-connection
tenant pool. All recipients commit together; acceptance failure leaves the reminder unsent for
business-level recovery. `sent_at` records accepted inboxes, never external delivery.

File reconciliation has a process-local sweep guard and gives unfinished tenants a
bounded batch before requeuing behind other tenants. The existing 120-second budget
starts at the tenant's first admitted phase and is checked after each completed
external operation and between batches; an in-flight operation finishes before yielding.
Each DB phase, including admission, times out at the smaller of five seconds and one
quarter of the existing lease. Cancellation defers durable work without replaying it. Expiry requests durably queue deletion before the worker
leases one operation immediately before external I/O; database completion checks the
same tenant UUID and the exact unexpired owner/attempt claim. A stale worker cannot
change metadata, acknowledge a newer claim or clean up its objects. If the exact
cancelled attempt completed a derivative PUT, or its timeout/error left that PUT
uncertain while the file was being deleted, fenced finish may queue a new durable
delete for that cancelled generation. It does not revive metadata or alter a newer,
expired or reclaimed claim.

File storage cleanup watches (migrations 136 and 137) preserve physical locators independently
of files, versions, operations and Certificate campaign purge. Every new PUT has a committed
namespace-bound write permission first. Deletion revokes that physical identity permanently;
cleanup records observations only for its exact unexpired claim. A stale/expired writer cannot
revive metadata or authorize deletion of a still-accepted physical key. The worker shares its
existing total batch and tenant budget between ordinary operations and watches; a persisted
starting preference survives new sweeps, and claim-time due advancement prevents a crashed
prefix from monopolizing later keys. No database lease or admission permit spans storage I/O.

Storage PUTs remain external to PostgreSQL. R2 supplies no settlement receipt proving all earlier
PUTs finished: cancellation, DELETE success and HEAD 404 are observations, not permanent absence.
Revoked watches therefore remain and are reprobed indefinitely, including after a restart or
actual Certificate hard purge; late bytes are deleted on a later successful probe. Recovery assumes
the tenant database/watch survives, the configured namespace remains the same physical namespace,
and the provider eventually answers probes/deletes. It does not promise instantaneous absence,
finite watch retention, versioned-bucket cleanup, or reconstruction of already-purged historical
locators. Same-name bucket recreation cannot be detected by the current provider; operators must
maintain namespace continuity. Account/endpoint/bucket routing changes fail closed. Credential-only
rotation does not change namespace identity.

Roll out only during maintenance after draining all old writers. The exact prepared School image
supplies worker-free `backend-school migrate-tenants` and `backend-school storage-namespace`
commands; no separately built tool is required for release. The normal startup path also checks
complete namespace readiness before spawning application workers. The standalone
`file_storage_namespace` wrapper remains available for explicit local operator work through the
[same locked source recipe](./TESTING.md#standalone-namespace-operator-tool).

The ordinary Pipeline validates inputs before R2 reconciliation, then verifies maintenance,
stops and verifies the old School process, removes its container to prevent boot restart, applies
canonical migrations through 137, and runs `storage-namespace check-admin` before creating the
matching server. Existing readiness, cutover audits, authenticated smoke and global acceptance
still control reopening. No pool/lease/client timeout or health policy changes. A stopped process
does not prove an external PUT has settled: retained watches and existing late-write cleanup
remain necessary; neither database/provider atomicity nor recovery of already-purged historical
locators is promised.

`context-admin` obtains the complete authenticated, mutation-fenced Admin inventory, including
inactive tenants; an authoritative empty list is valid for first installation. It prints only
`schema_version`, `tenants`, `inventory_sha256`, `public_namespace_sha256` and
`private_namespace_sha256`. Its inventory digest excludes migration-progress bookkeeping but
binds tenant identity and database routing. `routing-fingerprints` prints only the configured
public/private digests and performs no network access. `check-admin` verifies every listed
database's owner/identity, canonical schema and every retained watch, including locators whose
business records were purged. Any missing target, connection/identity/schema failure, unknown
namespace or provider mismatch blocks readiness. Ordinary releases check only; they never bind.

Historical namespaces require explicit attestation. For this initial rollout the user independently
confirms the R2 account and both buckets never changed since files were first stored. This is
bounded historical evidence, not permission to infer future namespaces from current configuration
or a successful HEAD response. The remote adapter fingerprints the actual old container's three
routing values with the exact prepared image, compares operator expectations with both those
values and the target context, and retains a private journal under
`/opt/stack/deployment/releases/<release-sha>/storage-namespace/`. Only allowlisted old routing,
immutable image/container identities, integrity digests and progress markers are retained; no
provider/session/database credentials are copied into this journal. Files are mode 0600 and
published with file/directory fsync before removal or migration; the directory is mode 0700.

The first check-only release can stop in maintenance with aggregate unresolved/mismatched counts.
Read the approved context JSON and old-routing digests from that exact release's preflight output.
If any digest is missing or masked, obtain verified original bytes; never reconstruct it by guess.
Construct a private JSON file with exactly these fields, using independently verified context and
an evidence digest that records the historical assertion and its exact rollout scope:

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

Dispatch Pipeline explicitly on the same reviewed main revision with `scope=backend-school`,
`automatic=false` and that file as `namespace_attestation`, for example:

```bash
gh workflow run pipeline.yml --ref main -f scope=backend-school -f automatic=false   --field namespace_attestation=@/absolute/private/namespace-attestation.json
```

Nonempty attestation is rejected for pushes and automatic dispatches. The input is a strict,
bounded hash-only JSON object, never credentials or raw routing. It is canonicalized before SSH
transport and passed only through `FILE_STORAGE_NAMESPACE_ATTESTATION_JSON` to the explicit
`storage-namespace attest-admin` command. That command checks the live complete inventory and
routing again, writes/fsyncs exact mode-0600 watch manifests, then uses the existing plan/apply
owner checks, expected revisions and active-claim refusal. Its private host manifest directory is
mounted only for attestation; rootless one-shots use `--userns=keep-id --user <host-uid>:<host-gid>`
so the image user does not need world-writable permissions. No command guesses a namespace or
performs provider PUT/DELETE calls to verify identity. An active claim requires an explicit retry
after its existing expiry; do not alter lease times or bypass the claim fence.

The binding tool remains available after rollout for explicit recovery of an unbound backup;
normal deployments and restarts retain only the check/startup gates. There are no permanent
legacy compatibility flags or automatic attestation defaults. Completed workflow design/plan
artifacts can be removed once the pull request records the outcome; durable commands and tests
remain owned by their normal implementation.

Failure keeps maintenance enabled. Before migration attempts, preflight preserves the old
runtime; after the first attempt the old binary must not restart, even if a later grant, audit,
binding or readiness step fails. An interrupted retry automatically reuses proof only for the
same release SHA and image digest. Its stopped-container and migration-attempt markers survive
container removal; the old routing cannot be reconstructed from the new `.env`.

Every adapter-launched headless command reserves the global rootless Podman name
`schoolorbit-school-rollout`, with immutable release, image and operation labels. Podman reserves
that name atomically: a surviving command blocks the next command even if its SSH client or
controller disappears and GitHub releases the production lock. The journal records intent and
proof, not process completion. Normal `--rm` removes a completed one-shot; any still-existing named
container, including a stopped or unrecognized residual, blocks release. Diagnostics expose only
the fixed name and validated release/operation labels. This interlock assumes the same canonical
rootless runtime owner; do not bypass it with another user, a random name or a second runtime.

On `headless_operation_exists`, keep maintenance enabled and inspect the named operation privately.
Verify its actual immutable image, release/operation labels, process/exit state and relevant
migration/binding journal. A lost SSH connection is not evidence that work ended. Do not kill or
remove a running operation or one whose ownership/completion cannot be established. After an
operator verifies that the process has ended and resolves any migration/claim uncertainty, an
explicit `podman rm schoolorbit-school-rollout` can remove a stopped residual; never use force
or an automatic cleanup/retry to bypass the interlock. Then retry the exact reviewed target under
the production lock and the existing journal rules. This does not prove external PUT completion;
the retained-watch limitations still apply.

A different compatible target requires a separately reviewed operator carry-forward under the
production lock, not automatic journal re-pinning. Preserve the original journal and routing
configuration; validate their owner/mode, original release/image identities and configuration
SHA-256. Verify the new prepared image and complete context. Record evidence linking the original
journal SHA-256, immutable old image and routing digests to the exact new release/image and inventory.
Only then create a new target-bound private journal from that original verified routing/configuration
and a new explicit manual attestation file. Keep the original evidence intact, sync new files and
parent directories, and verify the new target journal before retrying. This is an operator procedure
requiring protected host access and independent review; Pipeline does not supply an automated
cross-release carry-forward operation. If original proof cannot be verified, stop for recovery;
never copy new runtime routing as supposed historical evidence or run an incompatible old binary.

Expiry discovery alone never authorizes deletion: File Platform locks and rechecks current retention,
expiry and lifecycle in the deletion transaction. Domain Save locks/revalidates files before promotion
to standard retention. Thus an abandoned temporary ready upload can expire, while an accepted,
referenced or historical file is not deleted simply because an earlier candidate list included it.

Auth retains its hourly schedule, 120-second sweep budget and 100-row `SKIP LOCKED`
cleanup transactions. Each tenant gets a ten-second timeout including pool admission;
cancellation rolls back an unfinished transaction. Its process-local rotation cursor
advances before waiting for shared admission, so repeated exhaustion reaches later
tenants. A restart resets traversal but preserves all database-backed retention work.

Deploy matching School and Admin writers under normal Maintenance Mode. After
Admin migration 011 and School migration 118 are active and traffic has reopened,
prohibit rollback to pre-counter School or pre-fence Admin binaries and mixed
writer generations. Additive schema changes still require compatible writer
protocols: an old School `MAX+1` writer leaves the counter behind, causing repeated
unique collisions when counter allocation resumes; an old Admin writer bypasses
the durable barrier and cache invalidation. On failure, retain maintenance and
recover with a compatible forward release. Preserve the counters and SQLx migration
history; never drop, reset or rewrite them to recover.

If a mutation is interrupted, find its subdomain/token in the
`tenant_directory_mutation` log and inspect the corresponding Admin pending row.
Never delete a pending barrier by age. Stop/reconcile the old operation and its
external provisioning/deletion effects, then use a transaction to delete only that
school UUID and exact token with `DELETE ... RETURNING token`. This serializes with
the writer's identical claim: a late writer without its token must refuse to write.
Verify the canonical school state, commit, and send authenticated
`POST /internal/tenant-directory/<subdomain>/finish` to School with JSON
`{"token":"<recorded UUID>"}` and the normal internal caller/secret headers. Retry
that same finish token after transport failure; a different active token is never
released. No secrets belong in logs or command arguments. If the School process
restarts during an operation, Admin's durable barrier continues to deny cold reads.
Direct SQL status/credential changes must use this same barrier/fence protocol or
run under maintenance with School restarted after the final state commits.

pgAdmin Desktop connects to `127.0.0.1:5432`, database `postgres`, with observer or
human administration credentials, verify-full TLS and the laptop CA file. Configure
its SSH tunnel to the database host using the existing SSH key. Human superuser
credentials must never become application credentials.
Grant the observer `CONNECT` on application databases when inspecting their table
statistics; keep application-table read/write grants absent. This allows per-tenant
vacuum/I/O diagnostics without reading business records. Backend replacement uses
the canonical maintenance owner before first start and after readiness, explicitly
passing the validated domain and release SHA even after Compose exports are cleared.

Use pgAdmin activity/dashboard and observer queries against `pg_stat_activity`,
`pg_stat_database`, `pg_stat_user_tables` and `pg_stat_io`. Query aggregates use
`pg_stat_statements(false)` so routine reports exclude statement text. Look at calls,
total/mean execution time, rows, shared-block reads/hits and temporary-block writes.
Backend route-template response-header timings and `sqlx::pool::acquire` wait times
separate application/pool latency from database time. SSH into each VPS for its own
CPU, available RAM, disk and network; the database dashboard cannot measure the
backend host's CPU/RAM. No public metrics endpoint or additional monitoring stack.

Production School logging defaults to one JSON event per line through `LOG_FORMAT`
in [`podman-compose.yml`](../podman-compose.yml). Ordinary pool acquisition timings
use DEBUG; acquisitions exceeding 250 ms retain WARN at the default INFO filter.
For a bounded diagnostic window, set
`RUST_LOG=info,sqlx::pool::acquire=debug`, then restore INFO after collecting timings
to limit journal volume. `LOG_FORMAT=pretty` is available for interactive diagnosis;
restore `json` for normal production operation.

Operations `diagnostics` on main uploads and executes the bounded
[`pipeline-runtime-observations.py`](../scripts/lib/pipeline-runtime-observations.py)
collector through the existing SSH workflow. It keeps the existing readiness,
container/topology, ClamAV, storage/image and control-plane activity checks. The
collector alone reads application logs; its output contains closed labels and
numeric aggregates, never raw lines, identities, diagnostic UUIDs, SQL, URLs or
environment values. Each of School, Admin and Nginx uses a requested 15-minute
lookback capped at 5,000 lines, 4 MiB and five seconds. Lines over 16 KiB are
discarded; at most 256 groups are retained per source. A limit, failed command or
malformed recognized event marks coverage partial. Accepted, aggregated and
dropped-event counts are separate. `observed_event_span_seconds` describes the
span between timestamped accepted events, not container uptime or continuous
15-minute coverage; missing timestamps remain unavailable.

Download the `runtime-observations-<run>-<attempt>` Actions artifact for numeric
evidence. It retains only `runtime.json` and, when requested and successfully
validated, `tenant-probe.json`, for three days. Probe stdout and direct SSH
collector stdout reach private runner files before any Actions log processing.
The [artifact validator](../scripts/lib/pipeline-runtime-observation-artifacts.py)
accepts exactly one expected aggregate object, rejects unknown fields/labels,
nonfinite numbers and output over 2 MiB, then atomically preserves its original
JSON bytes. Missing or failed transport, extra stdout or malformed JSON cannot
promote a report. The SSH adapter follows the existing release owner's job-local
host-key discovery, pins that discovered key for the job with strict checking,
and bounds scan, connection and command time. This is trust on first use within
the job, not an independently verified server fingerprint. Its mode-0600 private
key, raw candidates and private stderr are cleaned up and are never uploaded.

Actions log masking stays enabled and can replace coincidental digit sequences
inside otherwise safe numeric aggregates. Masked log JSON is not numeric evidence;
never reconstruct its digits, round around masking or treat missing measurements
as complete. The ordinary validated JSON artifact is the authoritative transport,
without encoding, obfuscation or changes to the producer's measurements. It
contains no application logs, credentials, cookies, identities or browser captures.

HTTP response-header completion, cancelled request futures and proxy request/body
lifetimes remain separate distributions, including status/error groups. Selected
backend dashboard, student-list, work-count and authentication routes have explicit
sample availability; other routes are bucketed. Selected route names refer to
School; Admin's `/api/v1` traffic is bucketed as `other` rather than reported as
missing School-route traffic. Current proxy families are broader: dashboard,
student-list and work-count requests fall into `other`, so that proxy
bucket cannot identify those individual routes. Millisecond summaries use sorted
nearest rank `ceil(p*n)`: p50 for any nonempty group, p95 only at 20 or more samples,
and p99 only at 100 or more. Smaller tails are null with `unavailable_sparse`;
eligible tails are marked `empirical_only`. These thresholds do not establish
population confidence. Empty groups have no latency value. SQLx summaries contain
only counts/mean/max for logged successful acquisitions, which include connection
checks as well as waiting. School's default slow-only samples cannot describe the
pool population or failed acquisitions.

Password observations split admission, blocking-queue and bcrypt execution time by
operation kind and terminal outcome. Unreached stages are unavailable.
`admission_timeout` identifies admission saturation; `admission_unavailable`
identifies a closed worker. An HTTP 503 alone does not establish either cause.
Completion can arrive after the awaiting request was cancelled; verification match results,
credentials and hashes are never observations. The collector also takes a one-second
backend-host CPU sample, available/total host memory and numeric resource snapshots
for the four fixed runtime containers. Missing or unsupported counters and zero or
unreported container memory limits remain unavailable. These are backend-host
observations, not database-host CPU/RAM or query-plan evidence.

To opt into representative reads, dispatch main Operations `diagnostics` with
`options={"run_tenant_probe":true,"probe_subdomain":"snwsb"}`, selecting the intended
school explicitly. The probe is off by default; its tenant input defaults to empty,
which fails closed when opted in. The runner uses only dedicated
`RUNTIME_PROBE_USERNAME`/`RUNTIME_PROBE_PASSWORD` Actions secrets and trusted
`BASE_DOMAIN` and `BACKEND_SCHOOL_URL` repository variables. Set the secrets for
the selected school's existing staff account before dispatch; deployment smoke
credentials and local E2E credentials are independent and are never fallbacks.

The canonical `--tenant-probe` client accepts only the normal HTTPS
`school-api.<domain>` API. Before any network request it validates the explicitly
selected lowercase DNS label against the release smoke boundary (1–63 characters,
letters/digits/hyphens, no edge hyphens), rejecting infrastructure hostnames, full
hosts and URLs. Existing `sandbox` remains a valid explicit tenant selection.
Every request, including login and logout, sends exactly
`Origin: https://<selected-label>.<domain>` and matching `X-School-Subdomain`.
The client verifies TLS and refuses redirects. It checks the expected active
staff identity before business reads and selects only the existing active year,
then discards one warmup and records five sequential reads each for dashboard,
the first 20 students and work counts. The account needs actual student-read
access: the backend student API owns authorization and scope. The probe does not
infer access from permission metadata or elevate permissions. A student HTTP 403
on the initial warmup records `http_403` with zero measured samples after one
attempt. A later 403 retains already collected samples; either denial stops the
remaining reads for that endpoint without retries. Missing credentials or context
are explicit. This API-client sample
reports count, body bytes and client wall-time median/min/max, not browser paint,
population p95, load capacity or a performance improvement.
Python's urllib HTTP transport closes the connection after each request, so this
GitHub-runner wall time includes fresh DNS/TCP/TLS setup and the response body.
The warmup warms application data; it does not provide browser-like connection
reuse. A large client/backend timing difference alone is not a database slowdown.

The probe retains bodies, cookies, CSRF, IDs and credentials only in process memory,
uses 15-second request and 1-MiB response limits, at most 24 requests, a six-minute
total budget plus at most 15 seconds reserved for cleanup, and creates no browser
artifacts. Its own session is logged out in `finally`, using CSRF captured before parsing the login
body and the canonical host-only, Secure, HttpOnly, SameSite=Lax, root-path session
cookie. Logout must send that owned credential; an idempotent unauthenticated 200
does not establish cleanup. Cleanup failure is explicit; a timed-out login without received session state
is `cleanup_unknown` and is not retried. Normal login session limits still apply
to the dedicated probe account. The selected label is available in dispatch
provenance; the aggregate artifact contains no tenant label, identity or raw URL.
The probe runs before log aggregation. Its API-client sample targets the selected
tenant; runtime HTTP-header, bcrypt and pool distributions remain process-wide
across all tenants. They cannot be attributed to the selected school or combined
with its small sequential sample as if they were the same population.

For frontend navigation and first useful paint, use the existing credentialed
[deployed browser recipe](./TESTING.md) and
[route acceptance spec](../frontend-school/tests/e2e/route-loading-live-acceptance.spec.ts).
It requires locally supplied E2E credentials and browser execution; successful
Actions API sampling alone does not provide those measurements.

Backend DNS aliases are declared in `podman-compose.yml` before process startup.
The release network gate checks attachment metadata and resolves both the service
and container hostname from Nginx to the current container address. DNS retries are
bounded and read-only. Missing aliases or stale DNS keeps maintenance active; repair
configuration and recreate the affected backend through the canonical release.
Never disconnect/reconnect a running backend's network to update aliases. This can
change its source address after PostgreSQL connections have opened, leaving local
TCP sockets apparently established on an address the container no longer owns.
When diagnosing that condition, compare the process network namespace's interface
addresses and socket source addresses with Podman network events and database activity.
A controlled backend replacement clears those sockets; a VPS reboot is unnecessary.

When investigating pool stalls, correlate `tenant_pool_creation`,
`runtime_schema_verification`, `migration_owner_drain`, `tenant_pool_retirement`
and `attendance_schedule_registration` events with PostgreSQL aggregate activity.
Creation logs distinguish waiting for the tenant gate from connection/schema work,
with explicit schema-start/schema-verified events. Drainage failures include pool
size and idle count, never connection strings or SQL parameters. A zero idle count
alone does not prove a database lock; inspect `pg_stat_activity` wait classes and
ungranted `pg_locks` on the database host and transport/process health on the backend.

Tenant initialization has a 30-second overall deadline, including its per-tenant
creation gate; failed/cancelled creation is not cached and later requests may try
again. The first connection verifies schema inside SQLx's connection initialization
hook, before pool admission. Cancellation drops that owned socket directly instead
of leaving a background pool-return task. Subsequent connections in the verified
pool skip the history scan; ordinary cache hits perform no schema query. Closed cached handles are rejected. A drain timeout reports outstanding
borrowers instead of blocking deployment indefinitely. It does not forcibly terminate
transactions or guarantee that a broken network connection has disappeared. Restore
connectivity or resolve the diagnosed borrower, then retry the canonical Pipeline
under maintenance. Do not use a VPS reboot as the first diagnostic step. If process
replacement is necessary, use the normal release/recovery flow and preserve safe
stage/count evidence first. Business writes are never automatically replayed after
ambiguous transport failures.

Install `scripts/postgres_status.py` as
`/usr/local/libexec/schoolorbit-postgres-status.py` with the tracked monitor unit/timer.
It checks every five minutes and emits bounded JSON warning codes to the journal:
available RAM below 512 MiB, disk below 15% free, connections over 80%, backup older
than 36 hours, outstanding WAL archive failure or certificate renewal within 30 days.
`sudo -u postgres /usr/local/libexec/schoolorbit-postgres-status.py` is read-only;
the systemd job passes `--cleanup` for closed logs. No external notification channel
is configured. Review `journalctl -u schoolorbit-postgres-monitor.service` over SSH.

Keep pgBackRest backup at 02:00 Asia/Bangkok, full weekly and differential otherwise,
with four full chains and their required WAL in the private R2 repository. Retention
is chain-based, not an exact fixed number of days. Do not put an independent R2
object-age deletion rule on that repository. Preserve repository cipher password
and R2 access in the private recovery store; verify `pgbackrest check`, backup status
and periodic isolated restore with aggregate evidence.

The monitor removes closed PostgreSQL logs older than 14 days or above a 256 MiB
budget while preserving current log files. Tracked logrotate and cleanup cap closed
pgBackRest logs at 64 MiB/14 days; journald uses 256 MiB/14 days. Active files can
temporarily exceed a budget until rotation. Session cleanup runs hourly in bounded
batches with the existing 30-day expired/revoked-session and one-day throttle
retention. File cleanup keeps its durable queue, lease/retry and domain retention.
Neither autovacuum nor cleanup deletes academic/business/audit history to free space.

This guide describes production-facing procedures and invariants. Development conventions live in [`.rules`](../.rules), and executable verification commands live in [Testing](./TESTING.md).

## Runtime Topology

- `backend-admin` listens on container port `8080`, owns the admin database, school records, tenant database provisioning metadata, and deployment coordination.
- `backend-school` listens on container port `8081`, calls backend-admin over the internal network, resolves the tenant from the request, and connects to that tenant's PostgreSQL database.
- `frontend-admin` is the administrative web application.
- `frontend-school` is built once per input/configuration and deployed per tenant/subdomain and calls the school API.

Local source-build topology is defined in [`compose.local.yml`](../compose.local.yml), and the production topology is defined in [`podman-compose.yml`](../podman-compose.yml). Use Docker Compose locally and rootless `podman-compose` in production. The production file is the sole Compose owner for both backends, Nginx, clamd, their explicitly named networks, and the scanner volume. The production host publishes backend ports only on `127.0.0.1`; containers use service DNS names internally and must not use `localhost` to reach another container.

For first-time production server bootstrap, follow [Podman server setup](./PODMAN_SETUP.md).

## Zero weekly timetable targets

Migration 090 permits nonnegative actual weekly targets in the opening change journal.
The canonical opening snapshot owns the effective target; no retired catalog column is restored.
Courses and activities (independent or synchronized) set to zero remain opened with their
curriculum workload, credits, groups, teachers, rosters and assessments/results unchanged; only
recurring weekly timetable demand becomes zero. Zero does not waive activity evaluation or
attendance requirements. It does not create weekly timetable-derived attendance sessions.
Opening defaults stay unchanged; select zero explicitly when an activity has no weekly slot.
Publish the opening revision and explicitly update a timetable draft's opening source before the
new target applies there. Existing placed lessons and published history are preserved; remove
zero-target lessons from the draft before publishing it.

Migration [143](../backend-school/migrations/143_delivery_added_target_adjustments.sql) also fixes
weekly adjustments to offerings added to the same draft. It repairs only a differing cached weekly
target backed by that draft's add and adjustment commands, preserving every other captured field
and array order. Changed draft and parent revision counters advance once; reload a stale editor.
Published/cancelled revisions, journals, registry data and downstream history remain unchanged.
This repair never rebuilds a draft graph or restores excluded offerings/groups.

Migration090 expanded constraints without rewriting data. Older binaries remain compatible until
zero targets are stored in opening revisions. After that point, use a roll-forward fix: older
validation rejects those revisions. Do not roll back by deleting opening or timetable history.

## Automatic room roster tracking

Migration [100](../backend-school/migrations/100_learning_group_roster_tracking.sql) adds opt-in group roster policy. Existing groups remain manual. An authorized group manager selects room tracking and its effective date using the group's existing source-room coverage. Published group memberships then reconcile in the same transaction as room add, transfer, removal, admission enrollment, account deactivation, promotion execution/correction or term activation. Capacity and date conflicts roll back the originating write. Draft rosters reconcile when prepared and published. Planned placements in a planning or ready year produce dated upcoming memberships; activation reconciles them again. Reads never synchronize memberships.

Tracking is operational roster policy and does not change immutable opening or timetable snapshots. Existing membership identities, results and scores are retained. Cancelled-before-start (`removed`) episodes retain their history but occupy no teaching dates. Switching to manual preserves the current roster. Group managers must explicitly publish rosters before automatic room mutations apply to them.

Deploy the backend, generated API contracts and frontend through the normal maintenance and all-tenant migration gates. An older binary can read the expanded schema, but must not be restored once automatic tracking is enabled: it cannot maintain that invariant. Recover with a compatible forward fix; never delete membership history or alter migration checksums to roll back.

## Subject catalog revisions

Deploy [migration 131](../backend-school/migrations/131_subject_catalog_revisions.sql) with
its matching catalog provider through the normal universal maintenance and all-tenant
migration gates. It removes subject-only legacy code/year uniqueness and overlapping-range
rejection without changing historical rows, pinned curriculum/offering version IDs, published immutability. Migration 138 later retires activity range guards with start-only revisions. New same-date revisions become the default only
through the subject provider's date and revision selection; prior pinned consumers retain
their versions.

Deploy [migration 138](../backend-school/migrations/138_catalogs_without_academic_year.sql)
with its matching School catalog provider through universal maintenance and the canonical
all-tenant migration gates. Central subjects and activities use start-only civil dates independently
of academic years and have no expiry. The migration preserves every existing version's former year UUID and exact nullable expiry under
`migration_provenance.historicalCatalogContext`, retains the prior provenance object, and removes
the active year and expiry columns, foreign keys, year indexes, activity name/year uniqueness
and obsolete activity overlap/range guards.
Historical metadata has no foreign key and never selects or validates an academic year or expiry.
Formerly expired revisions become eligible under the start-only policy; explicit historical pins
retain their facts. Current metadata uses latest eligible start and highest revision; future
revisions never become current early.

The migration locks and reconciles catalog facts and actual reference owners, including
publication and delivery snapshots, without changing IDs, timestamps, revisions, statuses,
published values or pinned references. It temporarily disables only the two published-version
triggers and the inherited subject timestamp trigger within the migration transaction, then
restores and verifies them. Published guards, stable catalog locks and revision uniqueness remain
required; the obsolete activity interval trigger/function is retired before backfill.
The bounded `catalog.start_only_reconciled` audit records preservation evidence.
A reserved provenance-key collision, unsupported provenance, disabled guard or unexpected
column dependency refuses the migration atomically. Keep maintenance active and investigate
bounded findings on a protected copy; repair forward with a matching release. Pre-138 writers
require the retired columns and must not be restored against schema 138. Never retarget dates,
rewrite academic years, delete catalog/history rows or alter SQLx history to force success.

Curriculum course and activity additions use the catalog provider's current published revision
of the same resource kind and stable identity on Bangkok's civil date. Deploy the matching backend, generated contracts and frontend together
under normal maintenance. Existing curriculum rows, published snapshots, copied programs
and actual delivery references retain their version IDs; no migration or automatic upgrade
is part of this rule. Refresh stale options after a conflict and explicitly replace an old
row when a new revision is wanted. Historical duplicate subject or activity buckets may be retained or
reduced, but a new tuple cannot introduce another version into a duplicate bucket.

After migration 131, pre-revision School writers are prohibited: they do not enforce semantic
duplicate rejection under the stable-subject lock. Keep maintenance active on failure and
recover with the matching release or a compatible forward fix. Do not restore an older writer,
recreate the removed guards, repair tenant content in the migration, or edit SQLx history.

## Catalog grade snapshots

Deploy [migration 141](../backend-school/migrations/141_catalog_grade_snapshots.sql) with the
matching catalog provider and School frontend through universal maintenance and the canonical
all-tenant migration gates. Empty all-grades commands snapshot active school grade IDs at save;
explicit nonempty snapshots retain their coverage, including inactive existing IDs. Later registry
additions do not expand saved revisions. With no active grades an all-grades save fails atomically.

The migration fills empty never-published drafts of unarchived catalogs and appends corrective
published revisions for empty winning revisions at each effective-date boundary. Past, current
and scheduled dates retain their meaning; a newer same-day explicit scope is never widened.
Archived versions/catalogs and all old published rows, grade links, publication/offering snapshots
and reference IDs stay unchanged. Corrective revisions publish at migration time, retain the
source effective date, and carry deterministic source mapping and captured grade IDs under
`migration_provenance.catalogGradeSnapshot141`. Historical empty pins, including copied pins,
may be retained unchanged on save; new unsupported grade/slot tuples remain invalid.

The bounded `catalog.grade_snapshots_reconciled` audit proves source, mapping and reference
checksums and repair counts. Existing published guards remain enabled throughout. Missing active
grades when repair is needed, malformed relevant activity scopes, unsupported provenance,
deterministic-ID collisions, revision overflow or disabled guards refuse the transaction with a
bounded `CATALOG_141_*` code. A clean tenant requiring no repair may migrate without active grades.
Keep maintenance active on refusal; investigate aggregate findings on a protected copy and prepare
an explicit forward correction. Do not modify old publications, dates, reference IDs or SQLx history.
Atomic rollback leaves no partial correction; after resolving the cause rerun the canonical runner.
Applied migrations replay as a no-op.

Pre-141 binaries are prohibited after cutover: they can persist empty scopes and do not implement
the matched snapshot owner. Compiled migration-history checks and coordinated deployment enforce
this boundary. Recover with the matching release or a compatible forward fix while maintenance
stays active; do not restore an older writer or add a runtime wildcard fallback.

## Notification inbox-first cutover

Tenant [migration 139](../backend-school/migrations/139_notification_inbox_first.sql) and
[migration 140](../backend-school/migrations/140_notification_receipt_contraction.sql) must deploy with the matching backend, generated API and frontend
under maintenance with all older writers/dispatchers quiesced. Migration 139 checks inbox/source
identity, materializes only source-valid pending never-stored rows using original IDs (including
claims that failed before storage), preserves intentionally absent stored/terminal inboxes and
marks every legacy presentation `legacy_skipped`. No historical alert is replayed. Its hashed
migration-owned evidence persists across an interrupted deployment; 140 refuses changed inbox
content/read/timestamps, receipts or source ledgers before removing redundant payload and obsolete
lease/retry fields. On refusal retain maintenance, investigate the bounded finding code and repair
through the owner; never delete inboxes, alter evidence or skip the reconciliation gate.

The typed [acceptance provider](../backend-school/crates/school-notifications/src/dispatch.rs)
accepts `NewNotification` content, source, stable recipient/business key and presentation policy.
Producers resolve recipients once and call `create_in_transaction` inside their business save;
all recipient inboxes and canonical receipts commit together. A failed acceptance rolls back the
business mutation. Standalone `create(pool, tenant, items)` validates newly accepted recipients and
wakes the worker only after commit; transactional producers retain their application-owned
post-commit wake. Replays return the original IDs without changing read state, recreating an absent
inbox or reopening a presentation. The browser has no arbitrary-recipient notification-send API.

The sole [central worker](../backend-school/crates/school-notifications/src/runtime.rs) reads at most
32 pending references per tenant pass and runs four items concurrently. It validates source and
exact ledger/inbox identity, then commits irreversible consumption before SSE or Push. SSE is
attempted before subscription lookup, so Push configuration/query failure cannot suppress it.
Push uses four endpoint futures per item, each with a two-second deadline; there is no whole-item
deadline or endpoint rotation that can exclude later devices. All DB phases have their own
five-second deadline and release admission, connections and logical pool leases before network I/O.
Tenant/directory proofs and exact registration versions fence later device cleanup. Startup,
post-commit hints and the five-minute directory refresh recover never-consumed work without pinning
idle tenant pools. Transient pre-consumption DB failures defer the tenant probe fifteen seconds;
identity conflicts terminally block the exact pending reference without transport. Consumed work
never retries after network, timeout or cleanup uncertainty. Freshness expires after fifteen minutes,
and Push TTL is bounded by the remaining freshness budget.

Migrations [119](../backend-school/migrations/119_notification_dispatch.sql) and
[120](../backend-school/migrations/120_notification_dispatch_cutover.sql) are historical predecessors:
they moved Attendance's former transport queue into the previous central dispatch schema. Their
preservation/refusal checks remain part of migration history; their lease/retry runtime is superseded
by 139→140 and must not be restored against the current schema. Historical `published_at` could
mean suppression, not delivery; the cutover preserves that evidence without replaying old alerts.

The current canonical owner is `notification_receipts`; `notification_presentations` contains
pending receipt/expiry references only. Presentation consumes that reference and persists an
attempted/skipped/expired/blocked receipt before SSE/network I/O. Process loss or uncertain network/cleanup
can lose a presentation and cannot replay it. These outcomes never prove human delivery. Read
state is independent. Source reset/cancellation prevents unconsumed work; it cannot recall an
in-flight request or an OS notification. Preserve receipts, source/dedup identities and unread
inboxes during recovery; roll forward with a 140-compatible writer rather than restoring an old
lease/retry runtime.

Configure matching `VAPID_PUBLIC_KEY`, `VAPID_PRIVATE_KEY` and `VAPID_SUBJECT` (HTTPS or mailto contact).
The runtime config API returns availability and a public-key SHA256 identifier. A malformed or
mismatched key pair disables Push while inbox/SSE stay available. Browser refresh preserves a
binding only for the same installation, crypto, key and authenticated session; key/session/account
changes rotate its version and worker binding token. Revoked/expired sessions are immediately
ineligible for new Push snapshots. Devices expire after ninety days without confirmation; admission
allows twenty active bindings per user and bounded cleanup removes at most 100 stale bindings per
turn. 404/410 retires only the attempted version; 401/403 and global VAPID errors retain devices and
emit aggregate diagnostics. Never log endpoint URLs, crypto keys or private notification payloads.
The browser uses the runtime public key; a compiled `PUBLIC_VAPID_KEY` is no longer authoritative.

Automatic browser synchronization conditionally refreshes only an existing local device binding
using `expected_version`. A remotely revoked/missing/stale binding returns 409 and cannot silently
re-enroll. New and legacy unproven devices require the user to explicitly select
“เปิดและซิงก์การแจ้งเตือน”; inbox and SSE work immediately without Push enrollment. Session/key
changes may require explicit reactivation. Configuration failure preserves an existing good binding.

Notification device eligibility mirrors Auth's session validity, including active-account state,
through the narrow device/session adapter. Deactivation excludes a binding and bounded cleanup may
revoke it; reactivation does not recreate that removed binding. This does not reselect recipients,
change accepted inbox/receipt rows or alter source-owned business snapshots.

## Attendance rollout and retention

Migration [101](../backend-school/migrations/101_attendance.sql) introduces the attendance owner; [104](../backend-school/migrations/104_attendance_rosters_expand.sql) and [105](../backend-school/migrations/105_attendance_sparse_results_cutover.sql) establish its sparse-result model. Deploy the backend, generated contracts and School frontend through the coordinated maintenance and all-tenant migration gates described below. In the selected academic term, save the central attendance settings, choose counted weekdays/calendar overrides, and assign device operators. Calendar selection controls reports and teacher digests; captures and individual notices remain available on unselected days. Opening a round is read-only. Save persists only explicit changed results; missing results stay unchecked, and absence always requires an explicit check. A saved round retains its compact participant roster so later academic changes cannot rewrite that round's expected coverage.

History and reports use one `REPEATABLE READ READ ONLY` snapshot with shared module, lifecycle and term guards, allowing parallel readers and ordinary saves while retaining exclusion against reset, purge and lifecycle transitions. The [Attendance runtime](../backend-school/crates/school-attendance/src/runtime.rs) owns scheduled teacher digests, finalization and domain work; it does not backfill result placeholders or dispatch transport. Attendance resolves active students/recipients and guardian relationships once in the business save transaction, freezing recipients in the inbox and retaining its thin term/student notification ledger. Result and digest transactions call central `create_in_transaction`, so business facts, ledger, all inboxes and canonical receipts commit atomically. Later account deactivation or guardian unlink does not change that accepted recipient snapshot. Dated academic providers supply unchecked coverage when reports or digests are read. Arrival notices deduplicate per recipient/day; corrections have revision keys, and student notifications include active linked guardians resolved at that save. The operational kinds remain arrival, lesson and special; times use Asia/Bangkok.

Use the [inbox-first cutover](#notification-inbox-first-cutover) for the current notification runtime, migration and recovery contract. Source/module guards precede the central receipt lock, including exact Attendance ledger notification/recipient identity. Reset removes only resolved owned receipt/inbox identities before deleting source rows, so prefetched work cannot recreate an inbox. Purge requires accepted inbox or explicit terminal progress and retains source ledgers needed by never-consumed presentation; it does not wait for external delivery. Stable receipts preserve original IDs/read state and prevent resurrection of a missing inbox. Cancellation can prevent an unconsumed attempt but cannot retract in-flight SSE, Push or an OS notification.

Face enrollment requires its own scoped permission and recorded consent acknowledgment. Operators use an assigned enabled station on HTTPS with a webcam; browser models are pinned, locally served assets. Descriptors are encrypted with the existing `ENCRYPTION_KEY`, remain in bounded short-lived process memory for matching, and are never persisted in browser storage. Withdrawal removes templates and invalidates the gallery cache. The turn-and-return camera challenge needs physical acceptance testing and is not a certified liveness detector. Manual attendance remains available.

Accepted JPEG evidence uses the existing private File Platform/R2 path and its configured scanner. Verify that private storage and the scanner are healthy before testing scans. Student and linked-parent access is rechecked on download; evidence is not a public push attachment. The default evidence retention is 30 days and can be set centrally. Expiry enqueues durable File Platform object deletion; a pending storage operation is not proof that bytes have already disappeared from R2.

Term detail purge is restricted to authorized managers and closed terms. Preview the record/byte impact, verify authoritative notification acceptance/terminal progress, provide a reason and confirm the expected record count. The service reconciles explicit results plus virtual unchecked coverage, freezes arrival/lesson/special and assigned-teacher summaries, and enqueues evidence deletion in the same transaction before removing details. Purged terms cannot be edited; summaries and their unchecked coverage remain readable. Export any required individual evidence/history before purge, because aggregate summaries cannot reconstruct deleted details. Do not test purge on a live tenant. Roll forward with a compatible fix rather than altering migration checksums or deleting history.

## Required Environment and Secrets

All secrets and environment-specific URLs come from the runtime environment or deployment secret store. Example files are templates only.

Core backend-school secrets:

- `SESSION_HMAC_KEY`
- `INTERNAL_API_SECRET`
- `ENCRYPTION_KEY`
- `BLIND_INDEX_KEY`
- `DEPLOY_KEY`

Backend-school also requires `BASE_DOMAIN` and `TRUSTED_PROXY_CIDRS`; `SCHOOL_ALLOWED_DEV_ORIGINS` must be empty in production. Admin uses independent opaque sessions; it never consumes the School session key. School has no JWT runtime key or pre-session rollback mapping. Backend-admin retains ownership of its admin `DATABASE_URL`, and the provider credentials required by the operations it performs. Tenant provisioning uses `PG_PROVISIONING_DATABASE_URL` (CREATEDB/CREATEROLE,
non-superuser), `PG_TENANT_DATABASE_URL` (TLS connection template without tenant
passwords), and `DATABASE_CREDENTIALS_KEY` (32 random bytes as 64 hex characters).
`ADMIN_MIGRATION_DATABASE_URL` belongs only to explicit migration jobs; the normal
Admin `DATABASE_URL` is DML-only. Mount `POSTGRES_CA_FILE` read-only at
`/run/secrets/postgresql-ca.crt` in both backends and require `sslmode=verify-full`.
Deployment/DNS operations use the configured GitHub and Cloudflare values.

Internal service calls identify the caller with `X-Internal-Caller`. A caller-specific `INTERNAL_API_SECRET_<CALLER>` may override the shared `INTERNAL_API_SECRET`; the shared value is the controlled fallback during rotation. Keep both sides synchronized while rotating and remove the old value only after all callers are confirmed.

Never commit, print, or paste production secrets into logs, issues, screenshots, test fixtures, or generated artifacts.

## Health and Readiness

Both backends expose:

- `/health` for process liveness without dependency checks;
- `/ready` for dependency readiness and deployment gating.

Recurring Compose healthchecks use `/health` so process monitoring stays independent of external dependencies. Backend deployment workflows and smoke tests use `/ready`; backend-school readiness verifies its backend-admin control-plane connection without waking every tenant database. External uptime monitors must use `/health`, because polling `/ready` would continuously probe the control-plane database. A dependency failure must fail the deployment readiness gate, while a live process remains diagnosable through `/health`.

## Deployment Workflows

There are four public workflow entry points:

| Owner                                                   | Trigger and responsibility                                                |
| ------------------------------------------------------- | ------------------------------------------------------------------------- |
| [pipeline.yml](../.github/workflows/pipeline.yml)       | PR scope/base planning; main or manual preparation and production release |
| [merge.yml](../.github/workflows/merge.yml)             | Serialize trusted team PR integration after `Pipeline gate`               |
| [operations.yml](../.github/workflows/operations.yml)   | Explicit deployed smoke, diagnostics, benchmarks and tenant provisioning  |
| [maintenance.yml](../.github/workflows/maintenance.yml) | Bounded GHCR retention and serialized public API certificate renewal      |

The callable [prepare.yml](../.github/workflows/prepare.yml) and [release.yml](../.github/workflows/release.yml) implement the pipeline. The [deploy-school-release.yml](../.github/workflows/deploy-school-release.yml) and other `deploy-*` files
are callable component implementations, with no separate push or manual writers.

The flow is `plan → selected artifact preparation → Pipeline gate → production lock
→ recheck main/baselines → read-only private preflight → universal maintenance → selected deployment → readiness/acceptance
→ publish accepted baselines → open maintenance`. PRs only plan affected scopes and require an up-to-date main base. Main builds selected immutable
images and frontend bundles. Test suites execute on developer machines before a ready PR, as described
in [Testing](./TESTING.md); GitHub neither runs those suites nor imports local test results.
All production mutations, including Admin and tenant provisioning, share `schoolorbit-production`
with cancellation disabled. Cloudflare Workers Builds must remain disconnected from the managed
Admin/School Workers: repository-linked automatic deployment is a competing writer and bypasses
the production lock and maintenance. Pipeline stages and promotes Worker versions; do not enable
an independent Git-triggered deployment in the Cloudflare dashboard. An obsolete candidate is rejected before maintenance. GitHub can
replace a queued run; component baselines therefore include every outstanding runtime change,
not only the immediately preceding commit. A newer push during an active release waits for that
release to finish.

The planner distinguishes verification, build and deployment paths. Frontend-only changes do not
compile a backend; body-only backend edits do not rebuild a frontend. Generated contract changes
include their School backend/frontend consumers. Test-only and documentation changes do not
normally deploy. Manual `scope` can add one component or `full`, and cannot omit queued changes.
The plan reports why each component is selected: unaccepted runtime paths, changed artifact inputs,
an unavailable accepted baseline, or explicit scope. Component-specific deployment files reconcile
their owners; shared runtime/maintenance/release controls still select all affected components.
Preflight recomputes scope under the production lock. If an earlier release accepted queued work
while this run was preparing, remove those redundant components before entering maintenance.
Explicit manual scope is retained. Changed hashes or newly required unprepared components fail
preflight; an empty reconciled scope performs no deployment and does not enter maintenance.
The reconciliation job emits the public plan without deployment secrets. A separate private
preflight validates that transported plan, then checks Cloudflare ownership and operator inputs
without forwarding identity outputs. Missing or malformed plan transport blocks maintenance;
this separation prevents GitHub's secret-output filtering from dropping release hashes.
Admin frontend and School execute concurrently after the selected Admin backend is ready. School
still stages tenant Workers before backend replacement and promotes them only after backend readiness;
global acceptance waits for both branches and leaves maintenance enabled on either failure.
The accepted schema-2 `pipeline-state` artifact records each component SHA and input hash, immutable
OCI digest or frontend bundle digest/run, and exact Worker versions. The reader checks repository,
main ref, workflow, accepted attempt and successful acceptance job. It reads every artifact page
and selects the newest trusted successful acceptance by artifact publication time; numeric artifact
IDs and a run's later rerun update time do not establish acceptance order. Missing/invalid publication
times or distinct trusted states tied for newest publication fail closed. A cache hit alone never proves
verification or acceptance. Missing/expired/divergent state requires reconciliation; the first
transition from the previous workflows deliberately reconciles all four components.

`RUNTIME_DEPLOY_ENABLED` and `FRONTEND_DEPLOY_ENABLED` control automatic releases. Manual installer
releases remain possible while these are disabled. The installer dispatches one full Pipeline with
an exact deployment ID and selected target origin, and enables automatic releases only after its
public handoff verification. Direct-origin checks use the intended API hostname, loopback resolution
and public system CA trust; the still-public DNS name is not evidence of a replacement VPS.

Every production deployment enters maintenance for both School and Admin, including frontend-only
updates and provisioning. `/deployment-status` remains available with no-store typed status;
public API requests return CORS-safe `503`, with `204` preflights. Frontends show maintenance and
poll every 10 seconds while visible, then reload when ready. Per-release mode-0600 probe tokens permit readiness and authenticated smoke
through the proxy while users remain blocked; tokens bypass only maintenance, not authorization.
Backend-authenticated Admin `/internal/` remains available for control-plane calls. A fresh origin
starts the maintenance proxy before its first backend exists, without unresolved upstream names.

Rootless boot recovery uses `restart: always` in the canonical Compose definition and
`podman-restart.service` enabled in the linger-enabled `schoolorbit` user manager.
The installer enables the unit without starting application containers; every release
reconciles it only after both public APIs serve maintenance responses. Boot restarts
existing containers with their existing proxy mode and does not accept a pending
release or open maintenance. Check the user unit with
`systemctl --user is-enabled podman-restart.service`; use the normal Pipeline for
failed or pending release recovery.

The School backend build pulls its official Rust and Debian bases from Docker Hub, pinned by
index digest. The registry location changed after repeated Amazon ECR Public pulls failed with
`toomanyrequests: Data limit exceeded`; both Docker Hub index responses were verified against the
existing digests, with Linux AMD64 manifests present. This preserves the exact base content,
versions and build settings. Docker Hub has its own pull limits; a future registry failure requires
checking that registry's availability and authentication. Admin's base-image sources are unchanged.
Disposable PostgreSQL and Nginx verification images still use the Docker Official Images mirror
on Amazon ECR Public, and actionlint uses Google's Docker Hub cache; all keep verified upstream
digests. Update tags and digests together when upgrading bases or verification images.

Selected backend services use immutable digests, canonical rootless Podman topology, readiness,
migration completion and cutover audits. Admin starts before School on a full release. School
frontends consume one prepared bundle, synchronize menus through authenticated VPS loopback,
promote exactly their staged Worker version and apply routes. Asset propagation and real browser
mount checks precede acceptance. Admin uploads its credential-free code in one inactive version,
promotes the exact ID from structured Wrangler output and verifies it. Node 24 and the application
lockfile own Wrangler versions. Temporary secrets are mode `0600` and always removed.

The Admin release supports first-time creation or recovery after deletion of
`schoolorbit-frontend-admin`. Under the production lock and maintenance, it checks
that exact Cloudflare service. Existing Workers are preserved. Only a recognized
missing-service response permits a minimal 503 bootstrap; permission, transport
and malformed-response failures stop release. The bootstrap has no application
assets/bindings, custom routes, workers.dev or preview access. The prepared
application then follows normal credential-free inactive-version upload,
exact-version promotion, route application and browser acceptance. No manual
Worker creation or Git integration is required. Wrangler reapplies only the exact
apex zone route over the existing proxied apex DNS, without restoring the retired
Admin alias.
Deleting the Admin Worker removes its previous deployment history and managed
domains and makes the Admin frontend unavailable until release recreates it.
This recovery path never targets a School Worker, tenant DNS record or database.

To deliberately test deletion/recreation, dispatch **Operations** on `main` with
operation `recreate-admin-frontend`. Under the shared production lock it rejects
queued changes outside frontend-admin, starts a normal frontend-only Pipeline and
waits for that run's selected build and gate. It then enters universal maintenance,
deletes only the Admin script without forcing removal of dependent resources, and
releases the lock so the prepared Pipeline can recreate it and finish ordinary
acceptance. Do not delete the Worker manually outside the production lock. A stale
main or failed build stops before deletion. After deletion, maintenance remains
until the Pipeline succeeds; retry the current reviewed main frontend release to
recover. The operation uses only Cloudflare/SSH credentials, never database owner
credentials, and does not run account or tenant migrations.

The final acceptance owner verifies both proxy readiness paths and authenticated School academic
smoke, advances only selected image aliases, persists component baselines, and opens maintenance
last. Any earlier failure leaves maintenance active. A normal-proxy readiness failure restores
maintenance. Retry the same reviewed run or dispatch the current main; retained Worker recovery
manifests bind candidates to the exact trusted pipeline SHA and preserve the rollback boundary.
Successful replay uses accepted component baselines rather than a parallel legacy replay owner.
Provisioning reuses the trusted accepted School artifact and its public configuration, with an
exact request UUID; it has the same maintenance and acceptance discipline.

Team development uses one branch/worktree per independent developer session; a coordinated feature
team normally shares a checkout under `.rules`. Every main change enters through a PR.
Write collaborators automatically merge with a two-parent merge commit when the latest-main candidate passes;
no human approval is mandatory. Fork/external PRs cannot auto-merge. The trusted controller
leaves stale branches blocked until their developer updates and reruns affected local tests, merges one at a time, then dispatches the main
pipeline because `GITHUB_TOKEN` merges do not emit push workflows. Native protection for `main`
requires a PR, the GitHub Actions `Pipeline gate`, and an up-to-date branch, including for admins.
It requires zero human approvals and forbids force pushes and branch deletion. The required
repository merge settings enable merge commits and disable squash/rebase. Required linear history
must be off so both parent histories can be retained; preserve all other branch protections.
The `Pipeline gate` certifies planning and selected build success, not local test execution.
Local commands/results belong in each PR; keep work in draft until affected checks pass.
GitHub does not maintain test receipts or compiler snapshots. Deployed readiness, migrations,
browser mount/assets and authenticated smoke always execute under universal maintenance.

For the first PR that changes the merge strategy, verify the repository settings above before
marking it ready. The trusted controller still runs from main until that PR is merged, so disabling
squash first prevents it from using the previous strategy. Once local checks and the latest-main
Pipeline gate pass, a maintainer merges this transition PR with `Create a merge commit`; subsequent
team PRs use the updated controller automatically. Do not bypass PR/status checks or rewrite old
history to add graph connections. If repository administration is unavailable, keep the transition
PR in draft and report the exact settings that remain unverified or unapplied.

### Build, deployment timing, and image retention

Operations `cache-benchmark` compares School BuildKit `gha` and GHCR `registry` exporters
with `mode=max`, fresh pinned builders and the same production release flags on one Ubuntu 24.04
runner. It seeds a local Rust cache once, then copies that identical frozen snapshot into each
builder with compiler artifact writes disabled. Cache mounts allow sccache's LRU timestamp updates;
the original snapshot remains immutable. This isolates BuildKit from compiler-cache service failures; production Rust
cache policy is unchanged. Cold cache seeding, two source-only edits and two dependency-free workspace-member
edits are measured separately with reversed pair ordering. Scratch contexts never become releases.
The comparison includes runtime image export/load; image push, evidence extraction, builder
bootstrap, compiler snapshot priming and offline acceptance are outside the build wall clock.
Snapshot priming runs in a separate graph so compiler artifacts cannot become extra remote cache layers. Builder network bytes
include all external traffic, not just cache transfers; unavailable cache sizes remain unavailable.
Full timing, cache counters, source hashes and runtime/OpenAPI checks are retained as Actions
evidence. Each sample removes its exact local builder/image. The workflow deletes only its unique
`schoolorbit-cache-benchmark-<run>-<attempt>` GHCR package, never a runtime package; GitHub's cache
retention owns the isolated BuildKit entries. Compiler snapshots stay on the disposable runner and
are excluded from uploaded evidence; their hashes and sizes are recorded. Do not delete shared BuildKit blobs.
Keep the production cache backend until repeated warm build wall times and cache reuse justify a change.

School BuildKit layers use the dedicated `schoolorbit-backend-school-cache:buildcache` GHCR
package; Admin uses its existing GHA scope. The [cache selector](../scripts/lib/pipeline-build-cache.mjs)
owns this policy. Only main exports the School registry cache; other refs read it without replacing it.
Rust compiler caching still uses GHA, independently of the BuildKit layer backend. Rust, cargo-chef, and
sccache are pinned in their Dockerfiles. GitHub's cache runtime reaches Cargo as BuildKit secrets;
a missing or unavailable compiler cache falls back to an ordinary Cargo build. School uses the
same compiler cache for dependency cooking and the final application build. A changed workspace
recipe still invalidates the cargo-chef layer, but unchanged eligible dependencies can be restored
by sccache. This does not avoid compiling new dependencies or the final application executable.
The `rustc-cache` wrapper executes the pinned sccache binary. This prevents cc-rs
from also wrapping C/C++ and assembly compilers, reducing requests to GitHub's cache service.
School builds sccache 0.17.0 from the immutable upstream source and checksum in its Dockerfile,
with a narrow GHA retry patch. OpenDAL retries only temporary failures, including HTTP 429 and
transient 5xx responses, with jitter, 1–8 second exponential backoff and at most five retries per
operation. Permanent errors still propagate to the cache counters. The tool stage is cached;
Rust application profiles, native compiler wrapping and the existing cache-key format are unchanged.
The shared sccache configuration allows 120 seconds for server startup so temporary storage
checks can finish their retries before the client gives up. Both release and diagnostic images
use this configuration; the timeout changes no compiler options.
After Cargo finishes, the build waits at most 120 seconds for asynchronous cache writes to settle
before recording statistics. Settlement uses JSON counters because the human-readable CLI omits
successful write counts. This wait has its own metric, separate from Cargo and link time;
unavailable or unsettled writes are explicit and do not turn a successful compile into a failure.
Inspect dependency-cook write errors as well as application statistics before treating a run as
successful cache population.
Operations `compiler-cache-diagnostic` compiles an isolated library with the pinned toolchain,
then restores its new GHA entry in a fresh container. Daemon logs remain on private tmpfs mounts;
only allowlisted error classes, HTTP statuses and numeric counters enter the evidence artifact.
The probe publishes no release image and fails unless the new write and restoration are observed.
Set `options={"workload":"dependencies"}` to repeat the canonical School dependency cook in
two fresh containers, importing only its GHCR preparation layers. This preserves the release
toolchain, recipe and compiler settings, checks the canonical build's write-settlement result,
all eligible writes and subsequent hits, and
exports neither BuildKit layers nor a release image. Feature-ref writes remain scoped to that ref;
a successful branch diagnostic alone does not prove that a main-only failure is resolved.
Use `options={"workload":"cold-dependencies"}` to verify full population under load. It adds
a run-specific Cargo environment key only inside the diagnostic containers, forcing a separate
compiler-cache identity without changing optimization. The first container must write every miss;
the second must restore all eligible outputs without misses or write errors. Retry warnings stay
private; only allowlisted classes/statuses and the observed retry count enter the artifact.
School enables JPEG, PNG and WebP codecs to match the File Platform's inspected-content policy,
with Rayon processing retained. GIF, BMP and TIFF decoding additionally supports temporary supply image sources, which are normalized before canonical storage. The School image dependency keeps its codec features explicit.
Compiler-cache builds explicitly select GitHub cache API v2 inside Docker and use
`SCCACHE_GHA_VERSION` to separate admin and school entries. Passing the cache URL and token alone
does not select v2; a build that reports write errors has not populated a usable cache.
Component hashes exclude application READMEs and top-level integration tests already excluded
from image contexts. Public frontend configuration and the artifact-producing
[backend](../.github/actions/build-backend-image/action.yml) or
[frontend](../.github/actions/build-frontend-bundle/action.yml) recipe participate in keys.
Cache lookup, reporting and orchestration in `prepare.yml` do not participate. The initial cache-policy
input wiring changes the hashed backend action and prepares fresh backend manifests once; later cache
selector/orchestration edits preserve the artifact keys. Change actual build
commands at these recipe owners; their source and application Dockerfiles/scripts invalidate the
relevant artifact family. Migrating the hash format requires one fresh artifact preparation per
component before exact-input reuse is available in the new namespace.
Prepared OCI images are tagged by input hash and release SHA; runtime replacement uses their exact
digest. Prepared frontend bundles are checked by content digest and consumed without rebuilding.
Compiler evidence artifacts include Cargo HTML timings and sccache statistics when a build actually
runs. Compare compilation, restore, push and deployment durations separately; an exact cache marker
alone is not evidence of a faster release. A failed release does not advance accepted baselines.

Preparation primes the main-owned `deploy-tools-v1-*` cache through
[setup-deployment-tooling](../.github/actions/setup-deployment-tooling/action.yml).
The [selector](../scripts/deployment_tooling.mjs) extracts Wrangler, School browser verification
and route scanning dependencies from the existing application lock, including nested versions and
peers. Stage/promote/Admin jobs restore that closure, validate its identity and versions, and use
the same locked install on a miss. It contains dependency files only, with no application secrets
or build output, and is distinct from prepared Worker bundles and browser binaries.
Admin browser acceptance also restores the School tooling subset instead of installing the full
School application. Preparation and all deployment consumers use Ubuntu 24.04 so dependency and
browser caches have compatible runner environments.

[Compiler summaries](../scripts/render_backend_build_summary.py) show measured Cargo duration,
the largest compiler units, Rust library cache counters and available BuildKit phase durations.
School also exports separate dependency-cook wall time and cache counters. The dependency layer,
application compiler layer and GitHub Actions cache export each have their own current BuildKit
duration; a reused dependency layer may carry older cook metrics. Empty observed Rust counter maps
mean zero for that counter, while absent counters remain unavailable.
A cached compiler layer reuses its previous evidence; the summary labels it accordingly. Manifest
import time does not include every lazy cache-layer transfer. Full evidence remains in the compiler
artifact, and metric/log unavailability is displayed without changing successful build acceptance.
School's final-crate linker wrapper forwards the same `cc` arguments and records successful link
duration inside the builder. Link time is included in the executable Cargo unit and must not be
added to it. An absent invocation is unavailable, not zero. API-readable annotations expose current
layer reuse/execution, recorded executable/link time and BuildKit export phases even when signed
artifact downloads are inaccessible. Optimization, codegen units and runtime behavior are unchanged;
use this evidence before choosing a compiler profile or runner size.
The School caller proof and selected scope share `verify-release-ci`; all downstream runtime and
acceptance guards still require its success.

When retiring a cache-producing workflow, inspect the repository Actions cache inventory and
remove only exact IDs in its unused namespace after confirming no current workflow consumes it.
The retired `rust-ci-v3-*` test snapshots have no consumer in the local-only test flow. Keeping
large obsolete snapshots consumes the repository cache allowance and can evict useful release
entries. Preserve current `index-backend-*`, shared `buildkit-blob-*`, `sccache/*`, prepared
frontend bundles and deployed-browser caches; never use a broad cache purge. Cache cleanup
improves retention capacity, not the compilation speed of an already-warm application binary.

A source-only edit invalidates the final source/build layer, so the changed application crate and
final executable must be compiled and linked again. It does not discard the cargo-chef dependency
layer when `Cargo.toml` and `Cargo.lock` are unchanged. sccache may reuse eligible compiler outputs,
but it cannot avoid every changed-crate compile or the final link. If two ordinary warm builds show
no useful hits or increase total duration, remove the sccache integration while retaining the pinned
toolchain, Cargo timing artifact, and BuildKit cache.

School release builds use GitHub compiler caching. The release workflow passes
`SCHOOL_COMPILER_CACHE=gha` to both the image and timing export so exporting
metrics reuses the completed build. Credentials enter only through BuildKit secrets and never the
runtime image. A missing credential or cache I/O failure falls back to compilation. Standalone builds
default to `off` without runner credentials. To disable production compiler caching, change the backend
build recipe and timing export arguments to `SCHOOL_COMPILER_CACHE=off`; update the cache summary alongside them. Local Docker
experiments may explicitly select `local`, which is not the cross-runner GitHub cache.

After the workspace split, a source-changing GitHub comparison measured 24 eligible library hits
with zero write errors in each of two warm builds. Cargo duration fell from 6m25s without compiler
caching to 4m39s and 4m35s; the complete measured build, including BuildKit cache export, also fell.
These are build measurements, not production request latency or an entire deployment duration.
The application binary still compiles and links. Observe two ordinary warm releases after changes
to the workspace graph, toolchain, or cache integration; disable this cache if useful hits stop
reducing total duration. A new branch or cache namespace must first populate its own entries.

Exam scheduling models and persistence now compile in the existing assessment crate, while
the application retains HTTP/router/OpenAPI and lifecycle adapters. A same-runner Docker
comparison with a two-CPU quota and unchanged release optimization measured two application-source
rebuilds at 308 seconds before the move and 261/260 seconds afterward. Changing an exam service
after the move rebuilt assessment, its dependent academic crates, and the application in 297 seconds;
it does not eliminate the final application build. OpenAPI documents were identical. These are
warm Cargo measurements, not complete build/push/deployment timings. The first changed workspace
graph may require dependency-cache priming before those savings apply.

Academic and certificate handlers now compile in `school-academic-http` and
`school-certificates-http`. Menu models, templates, route synchronization and feature
toggles compile in `school-navigation`; notification persistence, tenant/user events,
publishing and Web Push compile in `school-notifications`. `school-auth-http` provides
their shared HTTP authentication context. The application owns routing, middleware,
startup and OpenAPI composition, and supplies narrow realtime, result-lock, lifecycle
and file-deletion adapters through `FromRef`. All contexts retain the same process-owned
identity and permission caches. New crates must never depend on the application or
accept its whole `AppState`.

Use the manual [HTTP Boundary Benchmark](../.github/workflows/backend-http-boundary-experiment.yml)
to compare the pre-extraction commit and a candidate on one Docker runner. Its two samples
per source owner separate first-graph priming from warm compilation and record rebuilt
packages, binary size, persistent-target reuse and exact OpenAPI equality. This comparison
disables sccache to isolate Cargo invalidation; use production build statistics to assess
cross-runner compiler-cache hits. A changed workspace graph may make its first image build
slower while dependency caches are populated. Do not equate a warm Cargo result with
the entire build, push, deployment, or request latency.

The [paired extraction experiment](https://github.com/akephisit/schoolorbit-new/actions/runs/37749256658)
used baseline `75b46a68`, candidate `27c5aba8`, the same Docker container and two
source-changing samples per case. Mean warm Cargo time changed from 265.4 to 236.5
seconds for application edits, 282.1 to 264.8 for Academic HTTP, 268.8 to 246.6 for
Certificates HTTP, 269.5 to 232.9 for Navigation, and 258.5 to 235.6 for Notifications.
Application edits rebuilt only the application; each extracted-owner edit rebuilt
that owner and the application, with no other domain rebuild. All twenty OpenAPI
exports were byte-identical to baseline. Unchanged target reuse took 0.40 seconds.
The candidate's first graph prime took 7m30s and is excluded from these means;
the binary grew from 109.2 to 111.2 MiB (about 1.8%). These two-sample compile savings
do not establish a percentage reduction in complete deployment time.

The pinned Rust toolchain already passes `-fuse-ld=lld` on this Linux target. Selecting lld again
does not remove compiler work. The manual build benchmark records the actual driver flag and
one linker invocation per timed build, separating flag priming from warm samples. It compares
default and selected final-crate codegen profiles sequentially in one two-CPU Docker container;
Academic HTTP and attendance edits show the default profile's dependent rebuild scope.
Keep production optimization unchanged
unless both repeatable compile improvements and representative runtime performance support a
new profile; profile/exporter experiments alone do not establish production request latency.

The application-owned explicit OpenAPI registry uses bounded schema constructors instead of
one very large generated function; release optimization settings are unchanged. The
[paired source experiment](https://github.com/akephisit/schoolorbit-new/actions/runs/38028625695)
compared baseline `fe3cce4a` with candidate `81f76e30` on the same runner and persistent
two-CPU Docker container. Application-only warm builds took 249.19/245.08 seconds before
and 186.99/190.98 seconds after, reducing the mean from 247.13 to 188.98 seconds (23.5%).
Only `backend-school` rebuilt in all four samples; links took 0.62–0.65 seconds and the
binary grew by 38,512 bytes (0.03%). All eight API exports were equal. Prime builds of
256.91 and 200.23 seconds and the final diagnostic compilation are excluded from these
means. This source comparison did not remeasure domain edits or complete deployment time;
do not compare its absolute times with another runner's profile experiment.

The school Dockerfile uses `cargo rustc` with `-C lto=off` only for the final application
crate to reduce source-changing release build work. Dependencies retain their existing
release settings and cargo-chef cache; release optimization level is unchanged. This is
not a project-wide LTO override. Keep the existing runtime acceptance gates: faster local
compilation does not establish production latency or GitHub build/push duration. To roll
back this setting, restore the final `cargo build --release --bin backend-school --timings`
command; no runtime configuration or database change is needed.

Each backend deploy emits bounded `deployment_timing phase=<name> seconds=<integer>` records.
School also times `r2_reconciliation`: bucket access is still verified on each deployment,
but the private-bucket CORS policy is read once and compared in full, ignoring array order.
Only drift or an explicit missing-CORS response permits a write; a write requires a fresh
read-back match before deployment continues. Other read errors fail closed. The
`r2_cors_action` log distinguishes unchanged and updated policies without exposing credentials.
Admin reports image pull, backend readiness, and origin verification. School additionally reports
scanner readiness, tenant migration/status, authenticated smoke, and proxy cutover when those phases
run. These records contain no environment values or credentials.

After the existing release-acceptance gate succeeds, the deploy removes only old, exact 40-character
SHA tags from its own SchoolOrbit repository and keeps the newest three. Image IDs used by any
container or addressed by `latest` or `rollback` are always protected, so a protected older SHA can
temporarily remain in addition to the newest three. Cleanup never uses Podman system, image,
container, or volume prune and never removes `schoolorbit-clamav-signatures`.

Run **Runtime Diagnostics** manually before and after the first rollout. It reports the filesystem
capacity for Podman's graph root, `podman system df` accounting, per-backend SHA-tag counts,
container state/network aliases, Nginx validation, bounded database activity, and endpoint status.
It does not print container environments or application logs.

The school deploy pulls the pinned ClamAV image and reuses `schoolorbit-clamd` only when its image,
3 GiB memory, CPU/PID limits, restart/security settings, lack of published ports, named signature
volume, two networks, running state, and health all match the canonical Compose definition. It emits
`clamd_action=reused` when exact; otherwise it recreates only that container, emits a bounded reason,
preserves the signature volume, and waits through the same health gate.
Scanner creation also passes `--pids-limit=256` directly to Podman because podman-compose before
1.4 ignores `pids_limit`. This compatibility flag applies only to clamd, with `--no-deps`;
deployment verifies the resulting PID limit before proceeding. Keep this value aligned with Compose.
Image-exposed ports with null or empty binding lists do not publish host ports; the matcher accepts
those Podman inspect entries but rejects every actual host binding, including loopback bindings.

Runtime GHCR retention runs weekly and can be dispatched manually. It preserves `latest`, the 30 newest
SHA-tagged releases for each backend, and every untagged or unrecognized version such as an
attestation. Manual dispatch defaults to dry-run. Scheduled runtime release cleanup stays dry-run unless the
repository variable `GHCR_RETENTION_ENABLED` is exactly `true`. Before enabling it, run a manual
dry-run and review both package candidate lists. The repository must have the package's Actions
admin access; a `403` is an access/configuration failure and must not be bypassed with another token
in source. Execution re-reads all selected candidates before mutation and again immediately before
each delete, processes oldest first, and deletes at most 100 versions per package per run.

The separate School BuildKit cache package has automatic weekly maintenance. It retains every tagged
version and the two newest versions, and selects only untagged versions at least 24 hours old.
It uses the same complete-inventory and immediate revalidation guards, with at most 100 deletions;
a candidate gaining any tag aborts deletion. An uninitialized cache package (first inventory `404`)
is empty; `403` and other errors fail. Run and review a manual dry-run after the first cache export
before its first scheduled cleanup. Runtime release retention keeps its existing opt-in variable.

For the first post-merge observation, use the next ordinary admin and school deployments rather
than triggering benchmark-only production changes:

1. Dispatch Runtime Diagnostics and record graph-root capacity, image accounting, and SHA counts.
2. Observe both build records, Cargo timing artifacts, and sccache hit/miss statistics. Repeat on the
   following ordinary source-changing deployment to obtain two warm samples.
3. Confirm each accepted deploy reaches all existing readiness, migration, smoke, and origin gates;
   check the phase timing records and `clamd_action` result.
4. Dispatch Runtime Diagnostics again. Confirm the active and rollback images resolve and each
   backend has the newest three SHA tags, allowing only additional protected image IDs.
5. Run GHCR Retention manually in dry-run mode. Enable `GHCR_RETENTION_ENABLED=true` only after the
   candidate order and protected versions are verified.

If cleanup fails after acceptance, keep the accepted service running and diagnose the exact protected
reference or Podman error; do not substitute a broad prune. If a cache path regresses, revert only
that cache integration. If ClamAV is recreated unexpectedly, use its bounded drift reason and the
canonical Compose definition to repair forward; never delete its signature volume.

## Reverse Proxy and Realtime

The current proxy sources are
[`nginx-configs/school-api.conf.template`](../nginx-configs/school-api.conf.template)
for the school API and
[`nginx-configs/admin-api.conf.template`](../nginx-configs/admin-api.conf.template)
for the admin API. The backend deployment workflows render them with the validated base domain,
install the result, validate Nginx, and then reload it.

Ordinary School API requests reuse HTTP/1.1 backend connections through a cache
of at most 32 idle sockets per Nginx worker, scoped to that location. This is an
idle-cache bound, not a limit on active requests or database connections. File,
SSE and WebSocket locations retain their dedicated proxy configuration. Diagnose
connection churn by comparing proxy network-namespace TCP active opens with
request counts; backend replacement still requires the canonical readiness and
proxy reload gates so cached sockets cannot retain the retired backend.

School API access logs use the JSON `school_proxy_request` event with a generated
`request_id`, static `route_family`, standard method (`OTHER` for custom methods),
status and numeric `request_time`, `upstream_connect_time`, `upstream_header_time`
and `upstream_response_time` in seconds. Upstream timings are `null` when absent
or when Nginx reports multiple attempts; do not treat `null` as zero. The School
normal proxy overrides the default access format, including local responses;
WebSocket access logging stays disabled. No raw path, query, client address,
credential, cookie or request body is included in this timing event.

For a bounded diagnostic run, clients may send `X-SchoolOrbit-Diagnostic-Id` with a
fresh lowercase, canonical UUIDv4 per request. The proxy forwards only that exact
format; missing, invalid or duplicate values become `-`, which the backend omits.
This optional opaque correlation value grants no access and must never encode a
user, tenant, national ID or other business identity. Keep it in private request
evidence, not metric labels. Backend `http_response_headers` records response-header
completion; `http_request_cancelled` records the request future being dropped before
headers, exactly once, with numeric `duration_ms`, the static matched route (or
`unmatched`), standard method and optional validated `diagnostic_id`. Health and
readiness requests remain excluded from these backend timing events.

Compare client failure timing, the proxy event and the backend event for the same
UUID to locate delay across the client/edge connection, proxy upstream and application.
The backend timer includes authentication, pool acquisition and handler work up to
response headers. It does not measure body transport or prove a mutation rolled back
when the client times out. A proxy can continue waiting after its client disconnects,
and a dropped application future does not identify its cause. Proxy request time
covers its own request lifetime, including body transport; inspect status (including
499 for a client disconnect) alongside upstream timings. Use the normal public API
path for capacity acceptance and retain the existing timeout, retry and pool settings.

Preserve:

- WebSocket upgrade headers and long-lived connection timeouts;
- direct, uncached realtime paths;
- the expected tenant/frontend CORS origins and credentials;
- forwarded origin/host information required for tenant resolution;
- access-log redaction so tokens, cookies, raw query strings, and PII are not recorded.

Validate WebSocket heartbeat, reconnect, and authenticated server-owned identity through the same proxy path clients use.

Timetable draft collaboration uses the supported single backend process. The application composes
one domain runtime for HTTP writes and version-scoped sockets; legacy term observers remain
separate consumers. Reservations expire after ten seconds and are renewed by active clients.
Cursor, drag, target and renewal traffic stays in memory. No database migration, reservation table,
worker, external broker or new runtime environment setting is required.

Accepted writes keep their resource exclusion through COMMIT and scoped patch publication even
when the initiating request or socket disconnects. The version's database revision and constraints
remain authoritative. A failed/ambiguous COMMIT or unavailable publication invalidates the cached
draft and asks clients to reconcile; never retry a possibly committed create automatically. Cold
recovery waits on the version row before reading its canonical graph. Upstream source and central
permission changes invalidate retained rooms even when they have no members.

A process restart discards roster, leases and replay history and creates a new epoch. Clients join
with a fresh authorized snapshot; committed timetable data is preserved in PostgreSQL. Do not scale
this runtime to multiple backend processes without a separately designed shared authority. Keep
proxy acceptance and the direct HTTP/WebSocket fixture recipes in [testing](./TESTING.md).
The frontend decoder and standalone smoke acceptance support at most 64 MiB of UTF-8 bytes per
complete JSON event; this is a client/acceptance transport budget, not a per-note/schema limit or
a server outbound enforcement claim. Server incoming interaction commands remain limited to 64 KiB.

## School Session Runtime and Cutover

`SESSION_HMAC_KEY` is the stable backend-school owner for domain-separated CSRF HMACs and reproducible pending session credentials. Generate a unique random value of at least 32 characters in the deployment secret store, never print it, and keep it unchanged across ordinary deployments and restarts. A key change requires explicit session-revocation recovery because it changes CSRF and pending derivation. Keep it independent of other service credentials.

`BASE_DOMAIN` owns the production tenant-domain boundary. `TRUSTED_PROXY_CIDRS` must contain only the networks of proxies that are allowed to supply forwarded client addresses; broad or unverified networks let clients spoof rate-limit identity. `SCHOOL_ALLOWED_DEV_ORIGINS` is only for explicit local origins such as `http://localhost:5173` and `http://127.0.0.1:5173`; leave it empty in production. Nginx must allow credentials and expose `X-CSRF-Token` while preserving exact tenant-origin validation.

Session policy is fixed in backend-school:

- normal sessions: two-hour idle and twelve-hour absolute lifetime;
- remembered sessions: seven-day idle and thirty-day (30-day) absolute lifetime;
- credential rotation: activity stages one pending credential after 15 minutes; first valid use promotes it and starts a 60-second previous-token grace window and the next rotation interval;
- human activity/idle renewal interval: five minutes; ordinary HTTP, SSE and WebSocket traffic never renew idle;
- revoked or expired session retention: 30 days.

Login creates the initial deadline; an explicit password change retains its existing atomic idle refresh, credential replacement and other-session revocation, clamped to the original absolute expiry, while clearing pending. Replacement cookies never outlive the remaining absolute lifetime. `POST /api/auth/activity` is the sole periodic replacement-cookie owner. A lost stage reply leaves the current credential valid; a repeated activity request reproduces the same pending value from the stable key and its persisted microsecond timestamp. Pending is bounded by session expiry, revocation and active account status, with no separate short expiry. Any valid request may acknowledge pending after its required origin/CSRF checks; acknowledgment never extends idle. Login, staging/promotion, revocation, rejection and realtime disconnect logs use structured event/reason fields. Session rejection distinguishes known idle/absolute expiry, revocation, inactive accounts, expired previous credentials and unknown credentials without exposing credentials or hashes. Never log passwords, raw session credentials, cookies, CSRF values, request bodies, or database URLs.

Login and `/auth/me` supply `sessionId`, server time, idle/absolute deadlines and the next five-minute renewal time using already loaded session metadata. The central authenticated-page controller reports recent human input only when due; background auth/realtime recovery does not count as input. Activity fences its expected session ID before maintenance. A 409 `session_changed` tells the client to reconcile `/me` once without clearing the new session cookie. Session responses are no-store.

Migration 145 expands the existing session row without changing applied migration 034 or backfilling credentials. Before the first pending credential is staged, older binaries can read existing rows. Once staging is active, recovery must roll forward with an acknowledged-rotation-capable backend and matching frontend: older binaries do not clear pending during immediate rotation and cannot implement this lifecycle. Keep universal maintenance and the single-process deployment boundary during cutover. No downmigration or second live School process is part of recovery. A very delayed older-generation Set-Cookie response across multiple completed rotations remains a bounded delivery race; the short coalesced activity endpoint reduces exposure but does not guarantee arbitrary response ordering.

Run exactly one backend-school process: [the session cache](../backend-school/crates/school-auth/src/session_cache.rs)
and revocation/permission events are process-local. API authentication reuses a successful database
check for up to 60 seconds, bounded by session expiry. Guarded activity publishes final metadata with synchronous session cache fences, so duplicate non-due current-credential pulses without pending rotation reuse the new deadlines without another session read. Pending staging/promotion and explicit security transitions may write separately from the five-minute idle update. Realtime connections
share database validation for up to five minutes per tenant/session/user; their 30-second heartbeat
continues checking cached expiry without extending idle lifetime. Normal authenticated API reads
also seed realtime validation. Logout, password change, and application-driven identity/permission
changes invalidate the relevant cache synchronously; no Redis or additional database is needed.

Direct SQL changes bypass those events and can remain unseen for up to the applicable cache window
(plus the realtime heartbeat interval). Use application operations for immediate revocation. If an
emergency operation changes identity/session state directly, restart the single backend process to
clear caches and terminate old realtime connections. Restarts preserve database-backed sessions and
reload them on demand. Do not scale to multiple workers/replicas or overlap old and
new School processes. The supported architecture stays one School process on one
VPS. A fresh cache can serve during a database outage;
once its validation window ends, authentication fails closed instead of extending stale entries.

Notification/work broadcast overflow emits `reconcile_required` with `{}`. The central
browser store re-reads auth, inbox/unread counts and work state on that signal and
each successful SSE reconnection. This does not replay missing messages; persisted
HTTP state remains authoritative. Session/permission channel loss closes the stream.

Browser tabs hidden for 60 seconds pause notifications SSE and timetable WebSocket. On returning,
the client refreshes authentication and reconciles authoritative state across the disconnected gap.
Web Push subscriptions remain independent of the SSE connection. The application change reduces
session queries; actual database resource savings also depend on other traffic and background jobs.

School authentication uses only opaque sessions. The completed JWT-to-session
bridge and its key-provisioning script are retired. Keep the applied session migration
and stable `SESSION_HMAC_KEY`; recovery uses session-capable releases. Do not deploy
pre-session School images or reintroduce Admin JWT authentication.

## Replacement VPS Migration and DNS Rollback

Use [`scripts/schoolorbit-installer`](../scripts/schoolorbit-installer) from an administrator
machine running Bash 4.4 or newer. The target may be Debian or Ubuntu. Supply credentials only
through environment variables, hidden prompts, or `--secrets-stdin`; the installer never accepts
secret values as command-line arguments. Run the read-only provider and target preflight first:

```bash
./scripts/schoolorbit-installer migrate-vps \
  --repository akephisit/schoolorbit-new \
  --target "$TARGET_IP" \
  --base-domain schoolorbit.app \
  --dry-run
```

Remove `--dry-run` for the real migration. The installer creates a mode-`0600` checkpoint under
`~/.local/state/schoolorbit-installer/`, prints its run ID, and records only non-secret state.
`SCHOOLORBIT_SERVER_PASSWORD` is also required: use a unique value of at least 10 characters for
the `schoolorbit` Linux/Cockpit account. It is installer input, not an application runtime value,
and must remain in `.env.local`, JSON stdin, a hidden prompt, or the operator's secret manager.
After correcting a failure before or during migration, resume the same run without repeating a
verified phase:

```bash
./scripts/schoolorbit-installer migrate-vps --resume RUN_ID
```

The migration bootstraps the target, installs runtime configuration and public ACME certificate material,
dispatches backend-admin, frontend-admin, and one `full` school release, and pins school tenant
discovery and verification to the selected origin until DNS is changed. Menu synchronization uses
the backend-school VPS loopback while public DNS still points at the prior origin. The installer
verifies both APIs directly with `curl --resolve` and public system CA trust, then
prints the DNS diff and
requires the exact phrase `CUTOVER <target-ip>` before one two-record Cloudflare batch sets only both API A records to DNS only (`proxied:false`). Public
verification covers API identity, both frontends, authenticated SSE, and the File Platform. Only
after those checks pass does the migration configure the Cockpit management Tunnel, verify its
connector came from the selected target, publish `server.schoolorbit.app`, and verify the public
Cockpit endpoint before the deployment gates are enabled. A failed post-cutover verification reports recovery commands;
it never performs an automatic rollback.

Rollback restores the complete checkpointed record content, TTL, and proxy state. Confirm that
the current records still represent this run, then execute:

```bash
./scripts/schoolorbit-installer rollback-dns --run-id RUN_ID
```

The command prints the reverse diff and requires the exact phrase `ROLLBACK <original-ip>` before
applying one reverse API-DNS batch. If that migration published management DNS, the same rollback
also restores or removes its management CNAME after revalidating both current states. The
replacement VPS, GitHub configuration, and both Cloudflare Tunnels are retained for diagnosis or a
later retry. Keep the old VPS available until the rollback window has been closed explicitly.

The TLS checkpoint stores `certificate_fingerprint` and `certificate_expiry`, never the private
key or DNS credential. Keep Cloudflare SSL/TLS mode at `Full (strict)` for the unchanged frontend
and management topology.

### Public API certificate lifecycle

[`api-tls.sh`](../scripts/lib/pipeline-remote/api-tls.sh) is the single certificate owner. It
runs the official digest-pinned Certbot DNS Cloudflare container through the existing rootless
Podman namespace. DNS-01 can validate both API names while DNS still reaches a previous VPS;
there is no port-80 challenge routing dependency. Certbot account, renewal state and private logs
live under `/opt/stack/acme`. Its mode-0600 credential is mounted from `/run/user/<uid>` only for
the invocation and is removed on normal/signal exit; reboot clears the runtime filesystem.
The persistent renewal configuration stores only that container credential path.

`API_TLS_CLOUDFLARE_TOKEN` is an Actions secret with Zone DNS Edit access to this zone. Prefer a
dedicated restricted DNS token; the installer uses the existing runtime DNS provisioning token
when configuring this secret. The broad bootstrap and Worker deployment tokens are never stored
for ACME. Never print credentials, copy ACME private state to CI artifacts, or enable a host timer.
The image and server user's existing Podman setup avoid privileged package installation.

The existing [Maintenance workflow](../.github/workflows/maintenance.yml) renews daily under
`schoolorbit-production`; weekly GHCR retention remains separate. To request renewal manually:

```bash
gh workflow run maintenance.yml --ref main -f task=api-tls
```

It requires the latest main Pipeline attempt's planning and selected build gate, trusted accepted
state, no queued runtime changes and no pending application release. Documentation-only main
commits may retain an older accepted application SHA. Issuance stages a validated export outside
Certbot's mutable `live/` lineage. If the exported fingerprint already serves, no maintenance or
reload occurs. Otherwise both APIs enter maintenance; one atomic generation pointer activates the
fullchain and key, then public-trust readiness and authenticated academic smoke must pass before
reopening the exact accepted application release. Nginx keeps its existing certificate filenames;
its complete SSL-directory mount exposes the relative generation links on reload. TLS does not
publish application baselines or change frontend versions.

Both API hosts use publicly trusted certificates and DNS-only A records. Verify direct HTTPS
with the intended hostnames and system CA trust before an origin change. Apex, wildcard, tenant
Workers and `server.schoolorbit.app` remain proxied.

For an existing VPS after the normal Pipeline passes, use the serialized API-only DNS operation:

```bash
gh workflow run operations.yml --ref main -f operation=api-dns-only
```

It refuses queued runtime changes, pending releases, invalid public TLS, ambiguous API records,
drift or A records targeting another origin. Its private rollback journal is
`/opt/stack/deployment/api-dns/<workflow-run-id>-<attempt>.json`. It verifies authenticated full
academic-context smoke directly from the external Actions runner before mutation, proving public
port 443/firewall reachability in addition to the VPS loopback checks. After DNS propagation, public
HTTPS must return the typed maintenance response without a probe token. Loopback authenticated
smoke passes before reopening; public authenticated smoke runs after normal acceptance with the
probe token explicitly absent. A failed public check restores owned maintenance and leaves the
applied journal available for recovery. An already-DNS-only request still performs public
authenticated full-context smoke; successful verification changes neither DNS nor proxy mode.
Rollback deliberately omits that external precondition so it remains available for reachability
recovery. To restore only the complete
two-record DNS snapshot, use the printed journal identity:

```bash
gh workflow run operations.yml --ref main -f operation=rollback-api-dns \
  -f 'options={"rollback_id":"RUN_ID-ATTEMPT"}'
```

Rollback refuses changed cutover records and preserves the accepted application identity. After
its DNS batch, the journal records `rollback_applied`. Repeating the same rollback after failed
acceptance validates the exact restored snapshot and repeats acceptance without another DNS batch.
Drift, a different accepted release or foreign pending ownership still refuses recovery. Rollback
retains certificate generations and management DNS; failure keeps maintenance active.

For a newly journaled DNS-only operation whose public acceptance failed, resume its exact owned
journal on the same accepted application SHA:

```bash
gh workflow run operations.yml --ref main -f operation=api-dns-only \
  -f 'options={"resume_id":"RUN_ID-ATTEMPT"}'
```

The mode-0600 server-user journal must match its schema, owner, request ID, accepted SHA, base
domain, target and exact applied DNS state. Only its private pending-release link permits reuse
of existing maintenance. Resume repeats external public TLS checks and private authenticated
acceptance without another DNS batch; release probe tokens remain loopback only. Invalid resume
IDs fail before remote actions. Source changes still require the normal Pipeline to reconcile
queued runtime work first. Recovery never upgrades a historical journal to a different accepted
SHA. Legacy journals without the ownership marker use normal Pipeline recovery, followed by an
ordinary already-DNS-only verification; their snapshots remain intact.

On failed activation the owner restores the preceding certificate pair and leaves maintenance
active. On later acceptance failure the new valid pair remains with maintenance active. Repeat the
same serialized TLS operation to resume its private journal, bound to the accepted application SHA,
operation mode, staged fingerprint and UUID. Only its owned pending-release link permits that
recovery; an unrelated pending application release must use the normal Pipeline. A retry after
activation still completes public verification and acceptance. Every renewal and recovery check
uses system CA trust.
Existing and reused exported private keys must retain server-user ownership and mode 0600.
DNS-only operational preflight needs zone/DNS access without SSL Settings Read; installer bootstrap
retains its separate Full (strict) settings check. Keep previous private generations
and DNS snapshots until the rollback window is explicitly closed. Certificate recovery must retain
a valid publicly trusted chain for both API hostnames. DNS rollback changes only the snapshotted
record fields and does not authorize restoring an untrusted certificate. Private ACME logs on the
VPS explain issuance failures; workflow logs deliberately contain only bounded status.

## School homepage search indexing

School homepage indexing follows the configured `PUBLIC_BACKEND_URL` hostname, which must use the production topology `school-api.<base-domain>`. The shared policy in `frontend-school/src/lib/school-public/seo.ts` excludes sandbox, local, preview and reserved hosts. `/robots.txt` allows page resources and declares the current production school's `/sitemap.xml`; the sitemap contains only that school's HTTPS homepage. Other HTML responses use `X-Robots-Tag: noindex`. Robots rules and noindex are indexing controls, not replacements for authentication or resource authorization.

After a frontend release, verify initial HTML without JavaScript, tenant-specific metadata and canonical URL, sitemap XML, and sandbox/login noindex as described in [Testing](./TESTING.md). Existing school settings remain the source of truth; do not enter a second name or logo for SEO.

The domain owner completes Google Search Console setup in their own Google account: add the Domain property for the production base domain, copy Google's TXT verification value into that domain's DNS, verify ownership, then submit each real school's `/sitemap.xml` and request indexing of its homepage through URL Inspection. Do not submit sandbox. Google tokens and account credentials must not be stored in repository files. A sitemap or indexing request does not guarantee immediate inclusion or ranking; monitor indexing in Search Console. See [Google's ownership guide](https://support.google.com/webmasters/answer/9008080) and [recrawl guide](https://developers.google.com/search/docs/crawling-indexing/ask-google-to-recrawl).

## Cockpit Management over Cloudflare Tunnel

The supported management path is:

```text
browser -> Cloudflare edge -> Cloudflare Tunnel -> 127.0.0.1:9090 -> Cockpit
```

Cockpit and cloudflared run as host systemd services; they are not Compose services and do not use
the application Nginx container. The host firewall must not allow inbound `9090/tcp`. Cockpit keeps
`root` in `/etc/cockpit/disallowed-users`; log in at `https://server.schoolorbit.app` as
`schoolorbit` so Cockpit Podman sees the same rootless containers as production. A root Cockpit
session would use a different Podman namespace and is intentionally unsupported. Cockpit starts a
new browser profile in Limited access mode by design. Select **Administrative access** and enter the
same `schoolorbit` password when host administration is needed. The installer adds `schoolorbit` to
the standard `sudo` group but does not create a `NOPASSWD` rule. After a group-membership repair,
sign out of Cockpit and sign in again so the new login session receives the group.

Cockpit Podman reaches that namespace through `podman.socket` in the linger-enabled `schoolorbit`
user manager. Its expected API path is `/run/user/<schoolorbit-uid>/podman/podman.sock`; the root
socket under `/run/podman` belongs to a separate rootful namespace and must not be used as a
replacement. Enabling this socket does not stop, recreate, or restart existing containers.

This deployment intentionally has no Cloudflare Access, OTP, or account-member gate. It is a
public login and the login page is therefore publicly reachable. Use a unique strong password, retain SSH key access for recovery,
monitor authentication activity, and treat Cockpit/cloudflared security updates as production
patches. Cloudflare terminates public TLS; Cockpit accepts unencrypted HTTP only on its loopback
listener. The Tunnel token is stored at `/etc/cloudflared/schoolorbit-cockpit.token` as root mode
`0600` and is consumed with `--token-file`; it must never appear in a command argument or checkpoint.

For an already migrated VPS, first ensure the operator environment contains
`SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN` with the account/zone permissions required to manage
Cloudflare Tunnels and DNS plus `SCHOOLORBIT_SERVER_PASSWORD`. Run a read-only check:

```bash
./scripts/schoolorbit-installer configure-cockpit \
  --repository akephisit/schoolorbit-new \
  --target "$TARGET_IP" \
  --base-domain schoolorbit.app \
  --dry-run
```

Remove `--dry-run` to apply. A distinct Tunnel named from the installer run is created or safely
adopted on resume; the management CNAME is published only after Cockpit, the loopback listener, the
fresh SSH verification, and the connector origin IP pass. Resume the same operation with:

```bash
./scripts/schoolorbit-installer configure-cockpit --resume RUN_ID
```

If management DNS was published but validation failed, use the reported run ID:

```bash
./scripts/schoolorbit-installer rollback-cockpit --run-id RUN_ID
```

The command revalidates drift, prints the reverse management diff, and requires the exact phrase
`ROLLBACK COCKPIT server.schoolorbit.app`. It restores an existing snapshotted CNAME or deletes only
the record created by that run. It never deletes either Tunnel or changes application API DNS.

After setup, verify on the target through a privileged SSH session:

```bash
systemctl is-active cockpit.socket schoolorbit-cloudflared.service
ss -ltnH '( sport = :9090 )'
curl -fsS http://127.0.0.1:9090/ping
stat -c '%a %U:%G' /etc/cloudflared/schoolorbit-cockpit.token

server_uid=$(id -u schoolorbit)
server_home=$(getent passwd schoolorbit | awk -F: 'NR == 1 { print $6 }')
runtime_directory="/run/user/$server_uid"
podman_socket="$runtime_directory/podman/podman.sock"
runuser -u schoolorbit -- env \
  HOME="$server_home" \
  XDG_RUNTIME_DIR="$runtime_directory" \
  DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_directory/bus" \
  systemctl --user is-active podman.socket
test -S "$podman_socket"
runuser -u schoolorbit -- env \
  HOME="$server_home" \
  XDG_RUNTIME_DIR="$runtime_directory" \
  podman --remote --url "unix://$podman_socket" info >/dev/null
```

The only listener must be `127.0.0.1:9090`, the ping service must be `cockpit`, and the token file
must be `600 root:root`. The per-user Podman socket and remote API check must both succeed. Confirm
the Cloudflare connector origin matches the target IP, the CNAME is
proxied to the checkpointed Tunnel UUID, direct access to `<target-ip>:9090` fails, and the
`schoolorbit` Cockpit Podman page lists `schoolorbit-backend-admin`, `schoolorbit-backend-school`,
`schoolorbit-clamd`, and `schoolorbit-nginx`. Retain the prior Tunnel/VPS through the rollback window.

## Academic context visibility and date-free identities

For the bounded migration-148 curriculum receipt observation, dispatch **Operations** on
`main` with `operation=diagnostics` and `options={"observe_academic148_curriculum":true}`.
The shared production lock and canonical owned-cleanup interlock protect the uploads and fixed
collector, which uses normal Admin operational
inventory/owner connections and one repeatable-read, read-only connection per tenant. It neither
runs migrations nor changes maintenance or tenant status. Only a complete, validated aggregate
is retained in the three-day `academic148-curriculum-<run>-<attempt>` artifact; refusal produces
no successful evidence artifact. Keep maintenance active while investigating a failed cutover.
The collector validates the trusted migration-001–149 source inventory and binds its fixed query
to the unchanged 001–148 checksum prefix. It remains limited to historical database schemas
147/148: schema 149 is explicitly unsupported and produces no successful evidence artifact.
This diagnostic boundary is not a business fallback or a prerequisite for the approved recovery.
Predicate masks preserve receipt/output multiplicity. Before-050 timing
is correlation rather than proof of a historical client; matching SQLx history verifies the
immutable migration prefix rather than historical ledger authorship. Checkpoint count/hash
matches are independent observations; legitimate later work, actor deletion or historical
session-timezone formatting can change them. Historical checkpoint audits did not record their
session timezone; do not guess one to force a match.
Schema 2 also groups checkpoint observations and divergence totals by verified history version
147 or 148. For receipts whose recorded created count exceeds the current timestamp-matched
count, divergence reports those counts and their difference, distinct current outputs, relative
output-time ordering, current-receipt reuse and retained deletion-item audit correlations. These
are aggregate observations of current facts: timestamp ordering or reuse does not prove causation,
and missing deletion audit evidence does not refute a historical deletion. Output-entry counts
and distinct-output totals use different units and must not be equated. No receipt identities,
timestamps or hashes are published, and the read-only observation still performs no repair.
These counts do not authorize guessed data repair or changes to applied migrations.

Deploy migrations [147](../backend-school/migrations/147_academic_context_visibility_defaults.sql), [148](../backend-school/migrations/148_academic_context_boundary_cleanup.sql) and 149 with the matching backend, generated API and frontend through the existing universal-maintenance and all-tenant migration gates. The exact compiled migration count/version/checksum fence prohibits prior binaries against the new schema. Recovery rolls forward with a compatible release; never recreate academic boundary dates, alter SQLx history or restore an older writer.

Academic Core owns visibility and the school default context independently of lifecycle. New years/terms are hidden. The one-time legacy mapping hides planning years and planning/cancelled terms to preserve the old public endpoint's unpublished intent; management reads retain those records. Other legacy contexts are visible, and the prior unique active/closing context seeds the school default once. Later lifecycle transitions never change visibility/defaults. Default replacement or hiding is one revision-protected transaction. Navigation selection never changes school settings.

Migration148 preserves existing explicit context links and actual dated facts. Only missing scoped Calendar target years are mapped: a homeroom supplies its authoritative year, otherwise the event's original date must identify exactly one year. Global targets remain legitimately yearless. Zero or multiple candidates, room/year conflicts, unknown date-dependent functions/constraints/indexes/views, excessive calendar materialization, changed retained facts or mismatched transformed intent refuse cleanup atomically. Keep maintenance active, investigate the bounded failure and prepare explicit reviewed data remediation before rerunning; never guess a year, retime an event or delete history to force success.

Attendance owns explicit dated counting decisions after cutover. Migration materializes the old weekday/default/override meaning across the old term range and finite persisted Attendance fact dates into `attendance_days`, retaining original override notes and saved rosters. Future counted dates are explicitly saved in the Attendance calendar; saved weekdays are a preset for explicit range generation. A missing date is uncounted, with explicit session overrides retaining precedence and captures remaining independently available. Behavior's old term/year reset intent becomes explicit inclusive cycle-end dates under its own scheduled policy; the following cycle starts on the next date after existing readiness gates. Empty scheduled lists create no invented resets. Promotion retains its intended placement date; new Admission/Promotion/Delivery commands supply actual placement/offering dates. Timetable/publication/membership intervals and official result snapshot dates remain with their existing owners.

Original lifecycle audit payloads, accepted readiness and request checksums remain unchanged. Migration verifies the original ordered request identity before normalizing only the active replay projection. Historical operation days come from each receipt's Bangkok creation day. Promotion's run date is a default for new or pending work; executed items retain the dates of their exact placement receipts, including mixed dates and legitimate decisions with no placement. Approval and execution bind the per-item date.

Curriculum and preparation receipts preserve an accepted explicit offering-date constraint only for new requests that create offerings. Legacy requests predate that input and migrate with a null constraint independently of their created counts, as do new retained-only results; publication can legitimately change an offering's actual date later. The source-confirmed historical draft delete/reapply path could recreate the same offering UUID, so current timestamps do not necessarily prove the original created subset. The cause of the three observed production mismatches remains unknown; a lower timestamp-matched count does not establish that an accepted historical count is wrong. Preparation may also create a separately dated Delivery revision/version. Actual offering/reference dates, links and original creation audits remain intact. Preparation verifies its original normalized module/mapping request and checksum; unrelated malformed, altered or unprovable history still refuses cutover without retiming or rewriting it.

Migration 149 owns a finite correction of that transition premise. On an exact verified, locked
147 cohort, the canonical runner retains ordinary mask-0 receipts. Among violating receipts it
admits only pure mask-1 cases with original count `N` greater than current timestamp-matched `M`,
with `N` no greater than distinct nonnull output cardinality and all other predicates passing.
All receipts participate in preservation. It preflights receipt shape, triggers and dependencies, retains the
original physical count column with its identity/default/checks, and derives a same-name `M`
projection. Before native unchanged 148, complete original receipt JSON must reconstruct exactly
by bidirectional `EXCEPT ALL`, count and SHA proofs. Preparation changes no other domain relation.
One outer transaction and connection span preparation, native SQLx 148 and 149 restoration.
The 148 audit truthfully records the projected phase; immutable 149 evidence links original,
projected, 148-audit and final phases and proves restoration of every original receipt field
except the newly null legacy `starts_on`. Restoration reuses `pg_temp.academic148_snapshot` to
prove that no nonreceipt relation in its transitive graph changed. All staging/helpers disappear
before commit. Existing-148 databases preserve receipt JSON exactly, including legitimate nonnull
explicit dates. Permission/grant reconciliation follows commit and connection release, supporting
a one-connection privileged pool.

For the 147 recovery branch, failure or cancellation rolls the whole transition back; no external
connection may observe a committed projected 148/staging state. A disconnect near commit can
leave 147 or 149. Reconcile fresh exact history and phase evidence before proceeding; never infer
rollback from client loss, replay guessed data repairs, or patch SQLx records. Keep universal
maintenance active and roll forward through the compatible canonical runner. No further
diagnostic-only production cycle is required before this approved correction.

Before dropping any boundary column, migration148 compares row counts and SHA256 summaries for the complete affected foreign-key relation graph, verifies every original nonnull link, compares each newly mapped target and promotion date, and checks exact Attendance calendar and Behavior schedule transformations. It repeats retained-fact and transformed-link checks after cleanup and stores immutable hashes/counts in `academic_context_cutover_audit`. Context status now remains pending below 149. The deployment gate requires context version 149 and exactly six distinct, passed `academicContextCutover` codes for every tenant: the existing five plus `ACADEMIC_CONTEXT_RECEIPT_RECOVERY_EVIDENCE`. The sixth verifies immutable 149 evidence and absence of staging; it does not continually compare original counts against current timestamps. These receipts record atomic phase reconciliation rather than an archive of retired boundaries or a claim that mutable business data never changes after release. Applied migrations replay without transforming data again.

## Tenant Migrations

Active tenant migrations begin at `backend-school/migrations/001_baseline.sql`. Do not modify an applied migration or hide a checksum mismatch.

New tenant provisioning calls the centralized runner in [`backend-school/crates/school-migrations/src/lib.rs`](../backend-school/crates/school-migrations/src/lib.rs), applies every pending active migration, and synchronizes the permission contract before creating the tenant administrator.

The coordinated school release keeps the school API in maintenance mode while it calls
`/internal/migrate-all`. It then verifies `/internal/migration-status` reports every tenant at the
repository's latest migration with no pending, failed, or outdated tenant. The normal proxy opens
only after the authenticated read-only smoke and, for a full release, every staged frontend
promotion succeeds.

The one-time legacy rebaseline is complete and its operational scripts are retired. If a tenant with legacy `_sqlx_migrations` history is discovered, stop the rollout and prepare a new reviewed recovery plan. Never point the current release at that database, copy migration history, or edit SQLx checksum records.

### Budget workspace rollout and recovery

Migration [129](../backend-school/migrations/129_budget_workspace.sql) adds budget settings,
projects/activities/lines, accepted plans, reservations, payments, adjustments and ledger ownership.
Migration [130](../backend-school/migrations/130_budget_history_ownership.sql) preserves immutable
plan-attempt ownership and adjustment revision documents. Its backfill records only the known
current adjustment revision; it cannot recover earlier drafts. Ambiguous or unmappable process
attempt history refuses migration and requires diagnosis rather than guessed ownership.

Deploy matching School backend, generated contracts and frontend through the coordinated release
under Maintenance Mode. Rehearse on disposable tenant copies and retain protected recovery points;
run the central all-tenant migration path and require every tenant at the exact compiled migration
timeline with no pending or failed migration before readiness and authenticated deployed acceptance.
Synchronize permissions/menu through their existing owner. Follow the [budget verification
recipes](./TESTING.md#budget-workspace) and reconcile ledger totals, process transitions and private
evidence before reopening traffic.

Structural additivity of 129 does not permit old binaries: the migration owner's strict compiled
history check rejects a timeline different from its embedded versions/checksums. Once 129 applies,
pre-129 School binaries are prohibited; after 130, only binaries matching that complete timeline
may run. Persisted `budget_evidence` purpose values also make older FilePurpose handling unsafe.
If migration or acceptance fails, retain maintenance and repair forward with a matching release.
Never edit an applied migration, its checksum or SQLx history to force readiness. Restoring a
protected recovery point needs a reviewed procedure accounting for writes since that point; do not
restore an old binary onto the advanced schema.

First configure fiscal years, funding sources, expense categories and approval steps at
`/staff/budget/settings` using an authorized settings account, then create projects at
`/staff/budget/projects`. Organization units/staff come from their existing directories. Fiscal
year is independent of the academic header; fiscal year and responsible organization are fixed
after the first accepted plan. The dashboard is `/staff/budget`, requests/payment work is under
`/staff/budget/requests`, adjustments under `/staff/budget/adjustments`, and exports/reconciliation
under `/staff/budget/reports`. Projects/activities use names without user-entered codes.

The authoritative spend ceiling is each project/activity/funding-source/expense-category
allocation: current budget equals planned plus adjusted, and available equals current minus
reserved minus paid. Approved credits can fund spending above an individual line estimate;
preserve consumed line dimensions and accepted history when amending. Money uses integer satang
strings, quantities permit at most three decimals, and multiplication rounds half up once to
satang. Payment requires a voucher number; evidence attachments are optional and remain private.

### Curriculum editions, educational levels and recovery boundary

Migrations 091/092 replace calendar bounds with a Buddhist revision year and reconcile the verified five-plan secondary-school migration shape. Migration [093](../backend-school/migrations/093_curriculum_edition_levels.sql) establishes one school-owned edition above its educational levels and study programs. It groups only matching known revision year, edition name and publication status; unknown years stay isolated and versionless roots become drafts requiring an explicit year before publication. Existing version UUIDs become level UUIDs. Program, requirement, room, student-year and admission references remain stable; the locked transaction verifies exact requirement content and records `curriculum.edition_hierarchy.reconciled` before committing. Ambiguous duplicate level membership refuses migration.

Migration [094](../backend-school/migrations/094_curriculum_school_permissions.sql) replaces curriculum owner scopes with school read/manage permissions. It preserves each existing curriculum action in role grants, organization-position grants and delegations, including delegation expiry and revocation. Other organization-scoped modules retain their ownership policies. Runtime code has no legacy curriculum root/version API or duplicate publication owner.

Create an edition on the curriculum overview, add educational levels with covered grades, then add programs and their grade/term requirements. Copying selects one published source program and creates independent draft requirements in the destination level, mapping semantic term slots and refusing uncovered grades or stale revisions. Publication validates every level and publishes the entire edition atomically. Room assignment explicitly chooses an edition then a grade-compatible published program; revision year does not limit calendar use, and publication never reassigns existing rooms.

Migration [095](../backend-school/migrations/095_curriculum_publications.sql) captures each currently published edition as immutable publication 1, preserving its complete graph and stable references. Unknown original publisher/date remain unknown. Existing educational-level/program tables become the sole editable draft graph; runtime consumers read the current immutable publication through published views. Opening an amendment creates or reuses one edition-owned draft token. Every mutation requires that exact token and resource row version; publishing validates all levels, seals the complete graph, advances the counter atomically and closes the token. Historical snapshots cannot be edited or appended after their creating transaction. New unpublished editions have no history until publication succeeds.

Migration [097](../backend-school/migrations/097_curriculum_draft_discard.sql) permits amendment cancellation only after the complete workspace matches the current publication. Staff with curriculum manage permission can preview and permanently discard an amendment using its exact token, edition row version and preview content hash. Discard restores all released levels, programs, term slots and requirements in one owner-locked transaction, deletes unpublished additions, closes the token and creates no publication or draft archive. Published history and actual teaching records remain intact. A changed preview or an external reference to a new draft resource refuses cancellation without partial deletion; reload the preview or resolve the reference through its owning workflow. This operation does not delete a never-published edition.

Migration [133](../backend-school/migrations/133_curriculum_programs_without_default.sql) removes study-program defaults from draft and immutable snapshot rows. The central tenant migration runner executes one atomic contraction: it locks the graph and actual-reference tables, records per-table counts and identity/content checksums excluding only the retired flags, drops the dependent published-program view and exact default index with `RESTRICT`, removes both columns, recreates the view, and verifies all evidence plus positional snapshot-capture column order. Its bounded `curriculum.program_default_removed.reconciled` audit includes current-view evidence. An open amendment may intentionally differ from released history; both graphs are preserved independently. The default requirement is removed from publication validation; all other publication guards remain. No business row, revision, timestamp, relationship or bell-schedule default is rewritten.

For 133, run the documented synthetic baseline/predecessor and publication/copy/discard/history checks only in owned disposable test databases. Deploy the matching School backend, generated contracts and School frontend through the normal Pipeline-selected release under universal maintenance; no manual `full` dispatch or unrelated Admin deployment is required. The existing canonical tenant-migration gate requires every active tenant at the exact latest migration, and migration 133 itself verifies surviving graph/reference/view evidence atomically before recording `curriculum.program_default_removed.reconciled`. Require the existing readiness and authenticated read-only smoke acceptance before reopening traffic; production curriculum writes are not a release check. Earlier binaries are incompatible immediately after contraction. If migration fails, its transaction rolls back; repair the cause and retry the canonical runner without editing checksums or SQLx history. If matched-release acceptance fails after migration, keep maintenance active and deploy a compatible forward fix through the same release procedure. Do not restore an earlier binary or recreate the flag; restoring a recovery point follows the existing reviewed recovery procedure accounting for all later writes.

Migration [096](../backend-school/migrations/096_delivery_curriculum_provenance.sql) records publication sources for new curriculum-derived offerings and curriculum preparation runs. Legacy openings without provable publication provenance remain explicitly unknown. Actual offering requirements reference an immutable identity registry so replacing draft requirements cannot delete their provenance. Publication alone never changes existing openings, teaching groups, timetables or delivery snapshots. Staff must review and apply changes through Academic Delivery and manage the resulting timetable separately.

After 095 applies, school binaries that read or mutate the draft tables as published data are prohibited. Deploy 095/096, backend, contracts and frontend together under maintenance. Require the `curriculum.publication_baseline.reconciled` audit, exact snapshot/content preservation, unchanged actual references, native draft/publication tests and a protected-copy rehearsal for every active tenant. Keep maintenance active and repair forward on failure; a restore must account for writes since the recovery point.

Migration [134](../backend-school/migrations/134_curriculum_program_order.sql) adds canonical custom program order to draft and publication rows with constant default zero. Existing rows, identities, revisions, timestamps, references and deliberate draft/history differences are preserved; zero ties retain code/ID fallback without historical backfill or disabled guards. Snapshot capture names program columns explicitly, and the published view appends the field through `CREATE OR REPLACE` without dropping grants. Draft creation/copy appends; complete-set reordering uses the draft token and level revision, leaves an unchanged effective order untouched, and changes only presentation order plus revisions/timestamps. Publication pins order and discard restores it.

Deploy 134 and the matching School backend/contracts/frontend through normal Pipeline selection under universal maintenance and the existing canonical exact-version/readiness/read-only smoke gates. The migration records `curriculum.program_order_added.reconciled` only after atomic preservation assertions. Earlier School writers cannot maintain custom ordering and must not return after this release; keep maintenance enabled and repair forward on failure. Lifecycle and concurrency checks use disposable synthetic databases, never production curriculum writes. This release does not convert catalog metric values or write user drafts to render reports.

For the historical edition/publication cutovers 093–097, rehearse the central runner on disposable copies of every active tenant and preserve fresh recovery evidence using the seven-day procedure under Personnel data cutover. Deploy those cutover migrations, backend, generated contracts and frontend together through a coordinated `full` release under maintenance. Require every active tenant at the exact latest migration version, passing preservation evidence, publication immutability, expected content/reference counts, readiness and authenticated smoke before opening traffic. After 093 replaces curriculum tables, all older school binaries are prohibited; keep maintenance active and repair forward if acceptance fails. Restoring a recovery point requires a reviewed procedure accounting for later writes.

### Academic Core Phase B cleanup and rollback boundary

Migration `045_academic_core_legacy_cleanup.sql` is the destructive Academic Core cleanup boundary.
It runs only through the centralized tenant migration runner while the school API is in maintenance.
The migration locks the affected schema, verifies the Phase A audit and current version-44
reconciliation marker, rechecks retained data and equivalent permission grants, and fails before any
drop when evidence is missing, stale, or inconsistent. It then removes the exact legacy manifest and
records the bounded `academic-core-v1-cleanup` audit. The one-time preflight command and mutable
Phase A reconciliation endpoint are retired and must not be restored.

For the Phase B deployment:

1. Confirm the protected pre-045 database snapshot exists, and do not delete it during deployment.
2. Dispatch the reviewed commit as a `full` school release. `/internal/migrate-all` applies every pending migration,
   including 045; do not invoke tenant migrations through another path.
3. Require `/internal/migration-status` to report every tenant at the repository's latest version
   with no pending, failed, or outdated tenant. Each tenant's `academicCoreCutover` must report
   migration version 45, `cleanupCompleted`, `passed: true`, and only passing bounded checks.
4. Require generated API and permission contracts, `/ready`, and the authenticated read-only smoke
   in the selected `academic_core_smoke_subdomain`. Credentials come only from `SMOKE_USERNAME` and
   `SMOKE_PASSWORD`; the workflow reaches backend-school through VPS loopback and exposes no public
   maintenance bypass.
5. Treat approval to dispatch the reviewed full release as the go/no-go decision. The workflow keeps
   maintenance active on any failure and opens the normal proxy only after every acceptance gate
   passes. Record the first accepted write as the snapshot rollback boundary.

Any migration, cleanup-audit, readiness, contract, or smoke failure keeps maintenance active. Before
the first accepted write, rollback means restoring the protected snapshot and the matching pre-045
release together. After the first write, do not deploy the old app against the new schema; keep
traffic closed and repair forward with a reviewed migration/application artifact. Never edit
`_sqlx_migrations`, an applied migration, cleanup audit, or tenant data to force a green result.

### Personnel data cutover

Historical migrations 081/082 standardized ranks/degrees, references and retired status. The current [083 expansion](../backend-school/migrations/083_staff_personnel_simplification_expand.sql) copies every position into the system-owned catalog with stable UUIDs, status, ordering and timestamps, and moves referenced education names into person-owned text. [084 cleanup](../backend-school/migrations/084_staff_personnel_simplification_cleanup.sql) freshly reconciles the locked source and target against the expansion audit before retiring the generic catalog and reference-ID columns. Custom/inactive current positions remain readable and filterable; new assignments require active selectable standard positions. The former reference CRUD API and management route are retired. Ranks/degrees remain codes; subject groups derive from current active organization memberships.

The one-time cutover completed on all active tenants at version 84, with preservation checks and source/target schema verified before retiring its personnel preflight endpoint, runtime audit readers and deploy helpers. Keep the applied SQL files and durable audit rows unchanged. Their preservation and refusal tests remain under `school-staff` through private test-only readers. Production reads and writes use the canonical owner.

Current releases call `/internal/migrate-all` through the centralized migration runner and verify `/internal/migration-status` against actual SQLx history for every active tenant. The release requires complete tenant coverage at the repository's latest version, no pending/failed/outdated tenants, and the remaining academic and gradebook domain audits. Maintenance stays enabled if verification fails. Readiness, authenticated smoke, menu synchronization and matched Worker promotion remain required before reopening the proxy.

For any future data replacement, retain a protected provider snapshot for at least seven days and rehearse the centralized migration runner on disposable copies of all affected active tenants before promotion. If provider quotas prohibit another protected snapshot or branch, preserve the existing protected snapshot, retain an unmodified no-compute pre-cutover branch for at least seven days, and supplement it with encrypted custom-format dumps of every affected tenant. Store archives and the encryption key outside the repository with restrictive permissions; verify complete decryption and `pg_restore --file=/dev/null` before promotion. Record the quota limitation and recovery branch ID privately; an unprotected branch is not a protected snapshot. Archive recovery requires the retained key and an explicitly reviewed restore procedure. Emit only bounded counts, versions and check codes; keep connection credentials outside arguments and logs.

Migrations 083/084 retain their own locked-source checks, fingerprints, exact preservation evidence and canonical schema checks before replacing FKs or removing retired owners. New tenant provisioning still applies the full immutable migration chain through the central runner. After a tenant applies 084, older binaries expecting the generic catalog or education UUIDs are prohibited. Keep maintenance active on failure and repair forward through a new reviewed migration/application release. Restore a protected snapshot only through a reviewed procedure accounting for later writes. Never edit applied migrations, SQLx history, reconciliation checks or personnel rows to force acceptance.

The initial matched frontend preserved verifiable owner-qualified v2 drafts through a bounded migration-only importer with their original 30-minute expiry. After both target frontends accepted the canonical release and that lifetime elapsed, the verified frontend cleanup retired the importer and v2 schema. The career release writes only owner-qualified v4 drafts. Both target frontends accepted the career release and the full original 30-minute lifetime elapsed before retirement of the v3 importer and schema. Obsolete ownerless and current-owner v2/v3 keys are now removed without reading or importing their contents. Invalid saves preserve the existing canonical draft, and reading never renews its expiry. Focused tests enforce scoped cleanup, canonical career preservation, refusal of retired fields, and absence of migration writes. No legacy API lookup or runtime schema fallback is allowed.

Rank milestone reads use the canonical current history and existing profile scopes. The versioned system policy is owned by [the staff calculator](../backend-school/crates/school-staff/src/rank_milestones.rs); a policy amendment requires a new reviewed version and calendar/state tests. The ordinary date is a planning milestone, and the conditional reduced date never confirms a reduction right or submission eligibility. No schema migration, persisted eligibility, review approval, notification scheduler or new permission is introduced. Deploy the backend before the matched frontend so the independent overview endpoint and history response are available; earlier frontends can ignore the additional history field. Verify the scoped endpoint and both tenant frontends after the matched release.

[Migration 085](../backend-school/migrations/085_staff_career_history.sql) establishes the canonical career-history owner and deferred current-projection guards. Import preserves existing position/rank values, UUID relationships and unrelated staff fields; effective dates, order dates/numbers and import actors remain unknown. Personnel type stays unspecified until explicitly recorded. Current changes and reasoned corrections run through `StaffCareerService` in one transaction with optimistic revision/current-reference checks and audit records. Historical append never advances the current pointer. Public profiles do not expose history or these dates.

After a tenant applies 085, binaries writing position/rank directly into `staff_info` are prohibited. Deploy migration, backend, generated API contracts and frontend as one coordinated full release under maintenance. Rehearse the central runner on every affected active-tenant copy, preserve a current recovery point using the seven-day procedure above, and require actual SQLx-version equality before opening the proxy. Keep maintenance active on failure and repair forward with reviewed artifacts. Never bypass the guards, modify an applied migration or SQLx history, or assume an old binary is a safe rollback. Recovery must account for writes after the retained point. This release records career facts; it does not calculate eligibility or a next submission date.

### Delivery and timetable version cutover

The coordinated school release applies migrations
[`086`](../backend-school/migrations/086_academic_delivery_versions_expand.sql),
[`087`](../backend-school/migrations/087_academic_delivery_versions_backfill.sql), and
[`088`](../backend-school/migrations/088_academic_delivery_timetable_cutover.sql) with the new backend,
generated contract and matching frontend while school-api remains in maintenance.
Rehearse the central runner on protected copies of every affected active tenant before consumer
cutover, retain the current recovery point, and use the disposable fixture recipe in `TESTING.md`.

086 only expands the schema. 087 locks and captures immutable opening graphs, maps exact source IDs,
and records fifteen identity/relationship fingerprints. Consecutive equal opening graphs share a
source; an A/B/A transition retains three dated opening versions. Pure placement drafts remain
independent; mixed revisions retain their pending opening commands and the separately pinned
placement draft. 088 requires fresh passing evidence before removing joint lifecycle columns,
migrates persisted placement coverage, and independently reconciles the canonical result.
Recorded direct inclusions without edit items become deterministic opening commands only when
the journal and draft agree on the published source, retain every original target, and provide
explicit added offering IDs and weekly targets. Early publication captures groups existing by
the recorded Bangkok effective boundary. Later imports require exact active placement IDs and
one dated assignment for every placed instructor, matching teacher ID, group ID and role at the
effective date across all placements. All historical instructors are reconciled against captured
episodes. Missing or overlapping episodes, changed targets without commands, missing journals,
and conflicting sources still stop reconciliation.
Missing, conflicting or ambiguous source evidence stops the migration without partial writes.
Repair evidence through a reviewed operation on protected data; never infer relationships from
names, discard a draft, weaken a guard, edit an applied migration, or change SQLx history.

`/internal/migration-status` reports `deliveryTimetableCutover` using the delivery-owned aggregate
audit reader. The full release keeps maintenance active until every tenant has the exact repository
SQLx version, `migrationVersion: 88`, `status: cutoverCompleted`, `passed: true`, and all thirty
checks from migrations 087/088 passing. It returns bounded codes/counts, without snapshot rows or
teacher/student data. Once 087 captures reconciliation, legacy writes invalidate that evidence;
keep traffic closed until 088 and coordinated backend/frontend acceptance complete. After 088,
older binaries using the joint opening/table lifecycle are prohibited. Recover by rolling forward
with a reviewed migration/application artifact; an old binary rollback is unsafe.

Acceptance verifies existing table/block/resource IDs, pinned historical opening facts, independent
opening publication, editing with the latest published source, review-marked retained placements,
autosave errors, standalone table publication, and deletion limited to an unreferenced draft.
Check read-only and scoped users through the deployed proxy and retain the feature branch and
recovery artifacts until migration, authenticated smoke, menu registration and acceptance pass.

### Selected opening room scope

[Migration 144](../backend-school/migrations/144_delivery_revision_audiences.sql) adds explicit
revision-owned audience parents and allowed rooms. An absent parent preserves legacy registry
semantics; an empty parent intentionally omits the offering and denies new intake after its
published effective date. Removal never deletes established roster memberships or scores.
Retained combined groups keep their identities and settings while the selected graph narrows
coverage. Next drafts inherit both nonempty and empty scope intent. No old removal is guessed
or backfilled.

Deploy the backend, generated contracts and matched frontend together under the ordinary
maintenance/all-tenant migration procedure. Rehearse room removal, reviewed curriculum reopening,
and inherited scope only on owned disposable fixtures or protected copies. Production acceptance
uses the existing read-only schema/provider smoke gates; it does not delete or reopen live courses
or groups. Timetable attendance uses its pinned Delivery source; adopting a newer source requires
the existing explicit timetable reconciliation. No
roster or timetable is automatically published. After any audience intent is written, old binaries
are unsafe because they do not enforce it: keep maintenance active on failure and recover by
rolling forward with 144-compatible providers. Do not erase scope parents, rewrite a published
snapshot, or restore raw registry coverage as recovery.

### Opening draft dates and permanent deletion

[Migration 089](../backend-school/migrations/089_delivery_draft_publication_and_deletion.sql)
separates a draft's internal reference date from its nullable publication date. Published graphs,
identities and effective dates remain unchanged. The matched backend and frontend must deploy
under the coordinated school release, with the centralized all-tenant runner and exact schema-head
checks. Binaries expecting a non-null draft publication date are prohibited after 089. Rehearse on
protected copies of every affected active tenant and retain the current recovery point for seven
days; recover forward after cutover, accounting for subsequent writes.

Ordinary DELETE operations require existing resource-management scopes, current revisions and
confirmed child counts. Drafts and cancelled versions may be deleted together with exclusively owned,
unreferenced children. Published versions and historical references are protected. A foreign-key
conflict rolls back the whole operation; never detach references or disable immutability guards.

The application-owned [cancelled-version cleanup CLI](../backend-school/src/bin/cleanup_cancelled_academic_versions/main.rs)
is a separate, explicitly scoped operation after migration, authenticated smoke and browser acceptance.
It resolves one school through backend-admin, verifies its stable tenant identity and term/year,
requires an active staff actor with both school-wide management capabilities, and preflights every
cancelled opening/table version before deleting anything. Active drafts are outside this operation.
Set `ACADEMIC_CLEANUP_SUBDOMAIN`, `ACADEMIC_CLEANUP_TENANT_ID`,
`ACADEMIC_CLEANUP_ACADEMIC_YEAR_ID`, `ACADEMIC_CLEANUP_ACADEMIC_TERM_ID` and
`ACADEMIC_CLEANUP_ACTOR_ID` from the reviewed scope; supply `BACKEND_ADMIN_URL` and the existing
caller secret through private environment configuration. Do not print connection URLs, secrets or
staff records. Without `ACADEMIC_CLEANUP_PREVIEW_HASH` the CLI preflights and rolls back. Applying
requires the exact returned hash; changed revisions/counts or any protected reference stop the
entire batch. Audit entries and deletes commit together, and the report verifies that published
fingerprints remain unchanged. Verify actual cancelled-row disappearance, active draft preservation,
source selection and the deployed version lists afterward. Do not broaden a failed cleanup scope.

### Gradebook and results cutover

Migration `060_gradebook_results_and_learner_evaluations.sql` is the Release 2 boundary for Gradebook,
learner evaluations, locked results, and result corrections. The deployment gate audits durable
schema, seed, and generated-permission invariants only. Its bounded report may contain check codes,
counts, migration versions, and pass/fail state; it must not expose student identity, scores,
outcomes, credentials, or source rows. Editable school grading configuration is not a cutover
invariant.

For the Release 2 deployment:

1. Create and retain a protected snapshot before dispatching the release.
2. Use one reviewed commit for the migration, backend, contracts, frontend, and deployment gate.
   Run the owned PostgreSQL compatibility suite first; it must use a fresh disposable
   database and its direct non-pooled endpoint to run the migration-060 schema and status-audit tests.
3. Deploy through the centralized runner and apply every pending migration through the repository's
   latest version. Do not apply migration 060 or later files manually.
4. Require `/internal/migration-status` to report repository-version equality for every tenant and
   `gradebookResultsCutover` at migration version 60 with `cutoverCompleted`, `passed: true`, and only
   passing checks. Any missing, failed, pending, or unavailable audit keeps maintenance active.
5. Require generated API and permission contracts, route/menu synchronization, `/ready`, and the
   authenticated read-only smoke to pass. The smoke samples at most two canonical term contexts and
   includes Gradebook subjects, learner-evaluation subjects, and result readiness.
6. Treat the reviewed `full` release dispatch as the explicit go/no-go decision. The workflow opens
   traffic automatically after every gate passes. Record the first accepted write as the
   protected-snapshot rollback boundary.

Before the first accepted write, rollback restores the protected snapshot together with the matching
pre-cutover application. After that boundary, keep traffic closed and repair forward with a reviewed
migration/application artifact. Never alter tenant rows, an applied migration, cutover checks, or
`_sqlx_migrations` to force deployment success.

### School font library cutover

Migration `040_school_font_library.sql` is an intentional empty-state cutover. Before entering the all-tenant migration gate, verify every active tenant has zero legacy certificate font assets, zero `certificate_template_font` staging rows, and zero text elements whose `fontSource.type` is `asset`. The migration enforces the same prerequisite and stops with `legacy certificate template fonts must be empty before migration 040` if any old row remains. Do not silently convert, copy, or delete a non-empty tenant during deployment; stop and use a separately reviewed data-removal or migration procedure.

Deploy migration 040, the backend routes, generated API contract, and the matching frontend together while school-api remains in maintenance mode. If any tenant fails, keep maintenance active, retain the new image, correct the tenant state through an approved operation, and fix forward through the centralized migration gate. Once any tenant has applied migration 040, never deploy an older backend that expects template-owned font columns, edit the applied migration, or alter `_sqlx_migrations` checksums.

## Permission and Menu Synchronization

Permission definitions originate in `contracts/permissions.json` and are materialized into generated registries plus tenant DB data. Deploy the contract artifacts and any new sequential permission migration together.

After permission changes:

1. verify the generated permission contract;
2. apply the new tenant migration;
3. verify expected permission/grant rows;
4. invalidate affected permission caches and confirm `permission_changed` refresh behavior;
5. rebuild tenant frontends when route metadata changed.

Frontend route metadata is synchronized explicitly after each successful tenant frontend
deployment. Provide matching backend and workflow `DEPLOY_KEY` values plus `SUBDOMAIN`.
Synchronization is transactional: frontend route identity is marked `managed_by =
'frontend'`, cleanup is limited to stale frontend-owned rows, and school-owned names,
icons, placement, active state, ordering, and custom menu records remain unchanged. Treat a
failed synchronization step as a failed deployment and fix the scan or backend error before
rerunning it.

## Encryption and Key Rotation

National IDs use application-side AES-256-GCM through `backend-school/src/utils/field_encryption.rs`. Search uses keyed HMAC-SHA256 blind indexes in `*_national_id_hash` columns.

`ENCRYPTION_KEY` and `BLIND_INDEX_KEY` must remain stable after data is written. Rotation requires a dedicated, reviewed job that decrypts with the old key, re-encrypts with the new key, rebuilds blind indexes, verifies counts/samples safely, and provides rollback. Do not switch either key independently without migrating existing data.

Do not use legacy PostgreSQL `pgcrypto`, `ALTER ROLE`, or database session settings for application field encryption. Do not log plaintext values or keys during migration.

## File Storage

Backend-school owns a provider-neutral File Platform. Business modules and frontends store a logical file ID; they never store an R2 key, bucket, provider URL, or signed URL. The platform selects storage from the registered purpose:

- `R2_PUBLIC_BUCKET_NAME` contains only public purposes such as school branding. `R2_PUBLIC_URL` is the delivery base for this bucket.
- `R2_PRIVATE_BUCKET_NAME` contains profiles, achievements, admissions, question-bank images, documents, and `school_font` originals. It must have no public custom domain or `r2.dev` access.
- `R2_PRIVATE_BUCKET_NAME` allows browser delivery only through short-lived signed `GET`/`HEAD` requests from `https://*.schoolorbit.app`. The backend-school deployment applies and verifies this CORS policy without making the bucket public.
- The two bucket names must be present and different. `R2_BUCKET_NAME` and `CDN_URL` are not compatibility fallbacks.

Object keys are immutable and server-generated:

```text
tenants/{tenant_id}/{domain}/{purpose}/{file_id}/v{version}/original.{ext}
tenants/{tenant_id}/{domain}/{purpose}/{file_id}/v{version}/derivatives/{variant}.{ext}
```

The `domain` and `purpose` segments come only from the purpose registry. This looks like folders in R2, but the database remains the source of metadata and lifecycle state. A future document system should reference file IDs and add its authorization relationship; it must not invent another key layout.

Required runtime configuration:

- R2: `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_REGION`, `R2_PUBLIC_BUCKET_NAME`, `R2_PRIVATE_BUCKET_NAME`, and `R2_PUBLIC_URL`.
- Scanner: `CLAMD_ENDPOINT`, `CLAMD_CONNECT_TIMEOUT_MS`, `CLAMD_WRITE_TIMEOUT_MS`, `CLAMD_READ_TIMEOUT_MS`, `CLAMD_MAX_CHUNK_BYTES`, `CLAMD_MAX_RESPONSE_BYTES`, and `CLAMD_MAX_CONCURRENT_SCANS`.
- Lifecycle: `FILE_PRIVATE_GRANT_TTL_SECONDS`, `FILE_RECONCILE_LEASE_SECONDS`, `FILE_RECONCILE_BATCH_SIZE`, `FILE_RECONCILE_MAX_ATTEMPTS`, `FILE_RECONCILE_RETRY_BASE_SECONDS`, and `FILE_RECONCILE_RETRY_MAX_SECONDS`.

Configuration fails closed at startup for missing, placeholder, shared-bucket, or out-of-range values. `/health` remains process liveness. `/ready` requires backend-admin, both R2 buckets, and a clean clamd probe; deployment must gate traffic on `/ready`.

### Bucket and scanner rollout

1. Preserve the existing public bucket and set it as `R2_PUBLIC_BUCKET_NAME`.
2. Check the configured public and private bucket names directly with `HeadBucket`, without requiring account-wide bucket-list access or printing credentials. Create `R2_PRIVATE_BUCKET_NAME` only when that exact private name is absent.
3. Do not attach a public domain, public bucket policy, or `r2.dev` access to the private bucket. Verify `HeadBucket` succeeds for both buckets. Apply the private-bucket CORS policy for `https://*.schoolorbit.app` with `GET` and `HEAD`, then read the policy back before deployment continues.
4. Start the pinned `docker.io/clamav/clamav-debian` runtime. Persist `/var/lib/clamav`, expose no host port, and wait for its healthcheck before backend-school.
5. For a backend-only release, select the `backend` scope and wait for `/ready`. For
   the File Platform contract cutover, the coordinated school release first places
   school-api in maintenance mode, starts the cutover image, waits for
   `/ready`, migrates every active tenant, verifies every tenant reached the
   latest migration, and only then restores the normal proxy.

The school release workflow performs exact-name checks before creation through the pinned AWS CLI image, validates and promotes the canonical production Compose definition, starts `schoolorbit-clamd` when required, and recreates `schoolorbit-backend-school`. It does not restart backend-admin or create a second production topology.

To diagnose private browser delivery, request a fresh typed grant through the authenticated file-download endpoint and keep `data.url` in memory. Fetch that URL separately with the tenant `Origin`, credentials omitted, and referrer disabled. Confirm the R2 response includes a matching `Access-Control-Allow-Origin`; never print, persist, or paste the grant URL because its query string is a temporary bearer credential.

### File Platform contract cutover

Migration `032_file_platform_contract_cutover.sql` is the clean boundary from
the path-based compatibility schema to the final provider-neutral schema. Its
transactional preflight refuses to drop legacy columns when a logical file has
no version, a ready file has no matching current version, or a legacy profile
or achievement path lacks its file-ID replacement.

The coordinated school release workflow performs this cutover while the school
API returns a CORS-safe `503` maintenance response. It starts the new image,
waits for `/ready`, calls the internal all-tenant migration endpoint, and
restores normal traffic only when every active tenant reports the same latest
migration version with no failures. The raw migration response is kept in a
mode-`0600` temporary file and must never be printed because it can contain
tenant-specific failure details.

If readiness, migration, or proxy restoration fails, leave maintenance mode
and the cutover image in place, inspect only safe aggregate/error information,
and fix forward. After any tenant applies migration `032`, never run a
backend-school image older than commit `1bdeb0c5`; those binaries still query
columns that no longer exist.

### Durable lifecycle and recovery

Uploads scan and inspect bytes before reserving public delivery. Originals and derivatives use immutable versions. Delete revokes delivery in metadata first, then removes objects. Provider or metadata failures leave durable operations for the background reconciler; retries use leases, bounded exponential backoff, and a terminal attempt limit.

When reconciliation is unhealthy:

1. keep the backend running only if `/ready` is healthy;
2. inspect safe reconciliation counters and error codes, never raw keys or signed URLs;
3. restore scanner or bucket access before retrying terminal work;
4. compare file/version/derivative rows with object counts using file IDs and tenant/purpose aggregates;
5. do not manually delete metadata rows or reuse object keys.

### Supply catalog rollout and recovery

Deploy the supply catalog through the coordinated School release, including [migration 110](../backend-school/migrations/110_supply_catalog.sql), [migration 111](../backend-school/migrations/111_supply_catalog_navigation.sql), [migration 112](../backend-school/migrations/112_supply_category_normalization.sql), backend and frontend. The additive schema remains compatible with the previous runtime before supply uploads begin. Once supply-purpose files or lifecycle operations exist, recovery must use a supply-aware runtime: earlier File Platform binaries reject those purpose codes. Preserve the expanded schema and repair forward instead of restoring an earlier binary.

Migration 112 aligns raw seed-category keys with application Unicode normalization while preserving canonical application-written keys. An existing collision blocks the transaction without deleting categories or changing their references; reconcile duplicates through an explicitly reviewed correction before retrying.

Proposal approval and image replacement revoke old file delivery inside the business transaction. Object deletion follows through the existing durable reconciler; a committed decision with pending provider work is not a failed approval. Use the lifecycle recovery procedure above for failed physical cleanup, and verify private storage/scanner health with disposable items in an isolated tenant.

### School font ownership and recovery

`school_font` is a private, scanned File Platform purpose. A new upload is staged either for the central school-font manager or for one exact certificate template; attaching a reviewed batch promotes the files into the school-owned library. The promoted font is not owned by a campaign or template, and campaign purge removes only its template references. Delivery remains grant-based and must never expose a bucket name, object key, provider URL, or signed URL in logs or persistent client state.

Deletion is reference-safe. `DELETE /api/school-fonts/{font_id}` returns `409` with the authoritative reference count while any template layout uses the font. Remove or purge those template references through the supported certificate workflow, re-list the library, and retry central deletion only after the count reaches zero. Never bypass this check by deleting `school_fonts`, `certificate_template_font_references`, File Platform metadata, or provider objects manually.

A successful central delete revokes file delivery in metadata before object cleanup. If provider deletion is delayed, leave the durable file operation intact and let the File Platform reconciler retry it with its normal lease, backoff, and terminal-attempt rules. Diagnose with safe file IDs, purpose totals, reconciliation counters, and error codes only. Restore scanner or bucket access before retrying terminal work; do not recreate the font row, reuse its object key, or purge the private bucket.

### Permanent certificate campaign purge

Certificate campaign purge is the only controlled exception that permanently removes File Platform metadata together with business data. It requires either the school-wide delete permission or the exact owner-unit delete permission, an exact campaign-name confirmation, and an unchanged impact snapshot. Starting it moves the campaign to `purging`; normal campaign reads and mutations then remain unavailable until the durable job finishes.

The purge first revokes file delivery and deletes every recorded object through the File Platform. Only after all inventory entries report deletion does the guarded database finalizer remove the campaign, templates, candidates, issue requests, issued and revoked certificates, audit rows, purge inventory, and file metadata in one transaction. Certificate counters are outside this deletion boundary and must never be reduced or reused.

When a purge is delayed or failed:

1. inspect only the API phase, deleted-file count, total-file count, and safe error code;
2. restore provider or database availability before retrying;
3. retry with `POST /api/certificates/campaigns/{campaign_id}/purge/retry` and continue polling the status endpoint;
4. treat status `404` as completion only after the purge was observed or accepted;
5. never delete campaign, purge-job, inventory, file-version, object, or file-metadata rows manually, and never print object keys, signed grants, raw provider errors, or recipient data.

If the campaign remains `purging`, leave the durable records intact so the reconciler can resume safely. Repair forward; do not restore the campaign to an editable status or bypass the finalizer.

The `rollback` image tag is advanced only after readiness and every active
tenant migration succeeds. For releases after the contract cutover, it may be
used only when its source commit is `1bdeb0c5` or newer. Keep migration `032`,
the private bucket, and the scanner volume; never restore a pre-cutover image
or move private objects into the public bucket. Roll frontend-school back
separately if its file-ID API contract is not compatible.

## Focused Troubleshooting

- Process unavailable: check container state and `/health`.
- Process healthy but not serving traffic: inspect `/ready` and its dependency, then service-network DNS/URLs.
- Backend-school cannot resolve tenants: check `BACKEND_ADMIN_URL`, internal secret/caller headers, request origin/subdomain consistency, and backend-admin readiness.
- Menu changes missing: confirm the post-deployment `Synchronize menu routes` step,
  `DEPLOY_KEY`, `SUBDOMAIN`, and backend route-registration response.
- Permission changes stale: verify migration rows, generated registry versions, cache invalidation, and the `permission_changed` client refresh.
- Migration checksum failure: restore the original migration file; add a new migration for the intended change.
- National IDs unreadable or unsearchable: stop writes and verify key/version configuration. Do not guess keys or overwrite ciphertext/blind indexes.
- File Platform not ready: inspect the safe `filePlatform` readiness field, then verify both bucket `HeadBucket` calls and `clamdcheck.sh` inside `schoolorbit-clamd`.
- Upload failure: verify R2 credentials, distinct bucket names, scanner health, purpose limits, and durable reconciliation state.

Use structured logs with correlation context, but redact secrets, cookies, national IDs, request bodies, and raw realtime query strings.

## Date calendar and activity request rollout

Migration [098](../backend-school/migrations/098_calendar_requests_and_date_context.sql) makes events independent of the academic header selection. It preserves existing event facts and timestamps, saves former year/term context in event provenance, and retains audience enrollment scope separately. It also creates private staff activity requests and grants `calendar.request.own` to staff roles and active organization units. Approval requires `calendar.manage.school`; pending and rejected requests never appear in calendar event reads.

Deploy the backend, generated permission/API contracts and frontend together behind the usual maintenance and all-tenant migration gates. Migration 098 removes academic context columns from events, so an older backend cannot be restored against the migrated schema. Recovery requires a compatible forward fix or a reviewed database restore; never alter migration history. Run the calendar database suite before rollout and verify request submission, manager approval and the resulting date-calendar entry on an isolated tenant.

Calendar owns notification meaning, recipient selection and reminder schedules. New requests resolve active staff with both calendar read and manage permissions through the same role, organization and delegation grants as actor authorization. Decisions notify the active requester independently of the event audience option. The [request owner](../backend-school/crates/school-calendar/src/requests.rs) accepts these inboxes through central `create_in_transaction` inside the request/decision transaction; event mutations and reminders likewise accept their whole recipient snapshot atomically. Storage failure rolls back the business save and every related inbox/receipt. The [workflow adapter](../backend-school/crates/school-calendar/src/delivery/request_workflow.rs) only wakes presentation after commit.

The [inbox-first contract](#notification-inbox-first-cutover) governs subsequent presentation. Receipt IDs prove durable acceptance, never browser/device receipt. Source cancellation and missing-inbox guards remain enforced; accepted recipients are not resolved again for transport. Links lead to the pending review queue or the requester's approved/rejected history; rejection reasons remain private request details. Inspect aggregate diagnostics and canonical receipt outcomes when presentation is absent. Never replay approval or create another business event to resend a consumed presentation, and never submit test notifications to real tenant users.

### Attendance sparse-result cutover and scoped reset

Migration [106](../backend-school/migrations/106_attendance_remove_person_notes.sql) removes per-person Attendance notes from records and historical result-save audit snapshots, including snapshots whose sessions were archived or reset. It preserves result facts, audit identity/actor/time/reason, revisions and unrelated audit/calendar notes. Its transactional shape preflight rejects unknown audit layouts with `ATTENDANCE_NOTE_AUDIT_UNMAPPABLE`; preservation reconciliation rejects drift with `ATTENDANCE_NOTE_PRESERVATION_FAILED` before column cleanup. Investigate only aggregate finding codes on a protected copy, repair the identified source through a reviewed operation, and retry the standard migration runner. Never delete audit entries or guess an unknown snapshot mapping.

Deploy the matching backend, generated API contract and frontend together under the coordinated maintenance gate. The new runtime can use schema 105 before cleanup, but once any tenant applies 106 the previous runtime is incompatible because it reads/writes the removed column. Recovery is a compatible forward release while maintenance remains active; do not restore an older Attendance binary or edit an applied migration. This schema change applies to every School tenant receiving the release; it is not a module reset. Old per-person note text is intentionally removed under the user's approved feature change.

Attendance schema expansion and cleanup are owned by [migration 104](../backend-school/migrations/104_attendance_rosters_expand.sql) and [migration 105](../backend-school/migrations/105_attendance_sparse_results_cutover.sql). Deploy through the coordinated School pipeline: enter maintenance, replace the old backend before applying tenant migrations, verify all tenants and the generated frontend contract, then complete authenticated acceptance. Migration 104 is compatible with the previous runtime. Once 105 commits, never restore the previous attendance binary: it writes removed unchecked facts and uses the removed ceremony category/calendar snapshots. Recovery is a repaired forward release under maintenance. A bounded reconciliation finding blocks cleanup; inspect and remediate on a protected copy without emitting tenant rows, then retry through the standard migration runner.

A full Attendance reset is distinct from the closed-term archival operation. It requires explicit operator authorization for the named tenant and both attendance school-management and school-deletion permissions. With the approved tenant session and normal origin/CSRF transport, read `GET /api/attendance/reset`, inspect the resource counts and evidence impact, and post its generation/token with a reason to the same path. Never select a tenant from a request body or run global table deletion. The transaction preserves academic records, users, other module data and the central audit; only attendance-owned central notification IDs are removed. Reconcile an ambiguous response by reading the impact and generation rather than automatically repeating deletion.

The reset requests all Attendance evidence deletions through File Platform, including uncommitted uploads. After commit, it attempts at most 50 prepared object deletions within a ten-second budget; unfinished work stays in the existing durable File Platform queue. Read the impact again to confirm physical cleanup. Durable reset acceptance requires zero attendance resources and zero active evidence files; physical cleanup acceptance additionally requires zero pending file deletions and successful File Platform object/version deletion reconciliation. Preserve deletion manifests until that check succeeds. The new module generation invalidates old previews/scans and queued digests, and virtual unchecked coverage starts on the reset day. Calendar weekday highlights and future navigation remain computed reads, not stored rows. Only perform the production reset for a tenant explicitly authorized by the user; the schema migration preserves other tenants' explicit results and converts their ceremony history to named special rounds.

### Behavior scores and midnight Attendance finalization

Deploy migrations [108](../backend-school/migrations/108_attendance_behavior_statuses.sql) and [109](../backend-school/migrations/109_student_behavior.sql), backend, permission/API contracts and frontend together under the coordinated maintenance gate. The approved rollout has no existing student behavior data and provides no legacy runtime bridge. The shared status contract replaces generic leave with sick/personal leave and adds skipped; migration 108 refuses ambiguous generic-leave facts rather than guessing their meaning. After migration, an older Attendance binary is prohibited. Keep maintenance active on failure and recover with a compatible forward release through the standard tenant migration runner; never alter applied migrations.

Attendance settings own the finalizer activation date. After the next Asia/Bangkok midnight, the background runtime persists absence for unchecked members of expected counted arrival, lesson and special rounds using their dated roster. Cancelled and uncounted rounds do not produce automatic absence. Skipped is explicit. Generation/date progress supports bounded catch-up after downtime. Opening a page never finalizes a day. Monitor bounded `behavior_background_retry` events and queue/progress counts without exposing student data. Term closure waits for finalization through its actual closing date, source-score delivery and pending Behavior cleanup completion.

Behavior scoring is disabled until its settings are enabled and criteria configured. Attendance changes enter a durable monotonic-revision queue; the application adapter applies frozen source policies and acknowledges delivery in one transaction. Corrections reverse and replace the affected effects without duplicate awards. Correct Attendance-derived points at the source round, not in the Behavior manual cancellation flow. Attendance retention preserves Behavior aggregates.

Behavior cleanup requires its own permission, authoritative scoped preview, revision/token and reason. Details-only deletion preserves totals; archive deletion preserves historical totals while ending current ownership; effect deletion removes the selected effects. Independent side resets begin new current cycles while retaining archived totals. Default student summaries span academic terms and years; report filters do not redefine a score cycle. Automatic academic-period resets ignore planning/ready periods that were never activated. Historical totals require a separate audited adjustment permission. These operations delete detailed entries and batch explanations physically, retaining only aggregates and minimal replay/suppression metadata; they cannot reconstruct deleted detail.

Large cleanup/reset operations persist a job before deleting bounded chunks. While a job is pending, score mutations pause and Attendance changes remain queued. The background runtime resumes committed progress after interruption and publishes final totals/cycle changes only at completion. Use the authorized job-status endpoint linked by the cleanup page to distinguish pending from completed; a returned pending response is not cleanup acceptance. Do not repeat the deletion request with a new request ID to accelerate work. Production destructive cleanup remains a separately scoped school/operator decision; feature deployment does not execute a cleanup.

Staff identity and qualification cutover uses migrations 113–114. Migration 113 preserves raw language values, expands the name read projection and backfills the person-owned qualification collection with the original staff-info identity and timestamps. Migration 114 holds the affected tables while reconciling counts, fingerprints, identities, ownership and all source values before removing the old single-qualification columns. Missing or stale evidence raises only `STAFF_PROFILE_RECONCILIATION_REQUIRED`; retain Maintenance Mode and remediate through a reviewed forward recovery. The deployment migration-status report requires all four `staffProfileCutover` checks before promoting the new consumers. After 114, recovery must roll forward with compatible code; the old staff binary cannot read the contracted schema. Do not restore an older binary against a migrated tenant or modify applied migrations. Rehearsal and failure-gate commands are in [testing](TESTING.md).

Migration 115 adds a nullable staff teaching-licence file reference without changing existing licence metadata. Files purpose `teaching_license` requires private scanned PDF/JPEG/PNG/WebP uploads. Before the first such upload, the additive column remains compatible with the previous application; after uploads begin, retain code that recognizes that purpose and recover by rolling forward. Attachment saves promote temporary uploads to standard retention; replaced/removed documents and abandoned drafts use the existing Files retention worker. Never remove an attached object directly from storage.

## Admin customer portal and account cutover

Migration 010 replaces only legacy central `admin_users` accounts under the
approved reset. A transaction compares every school and deployment-history row
before/after and refuses unknown account dependencies. It never opens or changes
a tenant database. School UUIDs, names, subdomains, credentials, DNS metadata,
users, academic data and deployment history remain unchanged. New memberships,
service periods and service requests belong exclusively to the Admin database.

Deploy backend/frontend together through Pipeline and universal maintenance.
After 010, old Admin binaries are incompatible; recover by rolling forward or
through the reviewed backup restore procedure, never by restoring an old binary
against the new schema. Keep a fresh backup and rehearse on an isolated database.
The new runtime accepts only the host-only `__Host-schoolorbit_admin` opaque
cookie. Legacy JWT cookies cannot authenticate. Logout and password resets revoke
persisted sessions. The apex Origin plus session-derived `X-CSRF-Token` is required
for mutations; tenant Origins cannot invoke central account mutations.

Before promoting this cutover, configure server-only `RESEND_API_KEY` and
`RESEND_FROM` in the VPS runtime environment and verify the sender domain in
Resend. Use a dedicated sending subdomain if desired. The installer requires both
values; they never belong in a Worker or `PUBLIC_*` variable. Production must not
set `ADMIN_ALLOWED_DEV_ORIGIN`. Email verification expires after 24 hours; password
reset after one hour. Links store their token in the fragment, require explicit
confirmation and are single use. An undelivered message can be requested again;
only the newest link remains valid. Delivery failure preserves the pending
account and reports a recoverable unavailable response. No real customer email
is used by tests.

For the first release, configure repository **Actions secrets**
`ADMIN_BOOTSTRAP_EMAIL` and `ADMIN_BOOTSTRAP_PASSWORD` for the intended primary
operator. These are temporary owner-job inputs, separate from the VPS runtime
mail settings. Never put a password in workflow inputs, command arguments, logs,
the repository, frontend bindings or `/opt/stack/.env`.

The normal Pipeline owns the entire cutover; no manual `podman pull`, container
replacement or migration is required. Under the existing production lock, release
preflight runs `/app/create_admin --check-deploy-inputs` against the prepared image
before entering maintenance. This read-only check validates mail settings and,
when no verified operator exists, the bootstrap credentials and intended identity.
Missing or invalid inputs fail before stopping services or replacing accounts.
It refuses an existing customer at the bootstrap email or a different pending
operator. A missing account schema alone no longer blocks the first release.

After maintenance begins, Admin stops the old backend, runs the canonical
`/app/backend-admin --migrate-admin`, then `/app/create_admin --prepare-release`.
The latter creates the unverified operator and sends its Resend verification
email. Retry preserves the same account, name and password, and resends a fresh
link without promoting customers or granting verification. A delivery failure
stops release and leaves maintenance active; retry the normal Pipeline after
repairing mail configuration. Post-migration failures recover by rolling forward.

`--check-release-ready` requires a verified operator or the configured pending
operator with an active verification link after successful delivery. Normal
readiness, immutable frontend promotion and proxy/browser acceptance then run
before maintenance opens. Once the portal is available, the primary operator
opens the newest email link and explicitly confirms it on the website. Pending
accounts cannot log in or use operator APIs. This avoids requiring a live
verification page before the first deployment. After verification, remove both
bootstrap Actions secrets; subsequent releases preserve the verified operator
and skip bootstrap without requiring those secrets. Complete dedicated
operator/customer acceptance before public handoff.

The explicit `/app/create_admin --check-ready` remains a stricter diagnostic for
a verified operator. Recovery-only `--bootstrap`, `--resend-verification` and
`--verify-email` modes accept protected one-shot environment values, including
`ADMIN_BOOTSTRAP_TOKEN` for token consumption. They are not ordinary deployment
steps. Never print or pass tokens in command arguments.

Public search discovery uses the apex homepage and `/schools`, with an anonymous,
keyset-paginated `GET /api/v1/public/schools` that selects only active school names
and subdomains. Sandbox/reserved names and inactive schools are omitted. The
central `/robots.txt` advertises `/sitemap.xml`; the sitemap contains only the two
public central pages and does not depend on backend availability. The directory's
server-rendered links discover production school homepages, each with its existing
host-specific canonical, robots and sitemap. Account/private/error pages carry
`noindex`; previews cannot advertise production indexing. No tenant database is
opened and no school/account data is modified by this discovery flow.

Search results use the Admin Orbit PNG as the platform icon. Its vector source is
`frontend-admin/static/schoolorbit.svg`; both frontend deployments carry matching
`schoolorbit.png` assets. A School homepage advertises its stable `/favicon.png`:
the route makes one anonymous request to `/api/school/public/favicon`, selecting
only that tenant's current crest without looking up the school name. The File Platform renders
a 256-pixel square PNG, retaining the whole portrait/landscape image. The original
file is never modified. Installed-app PNGs use `/app-icon/192.png` and
`/app-icon/512.png`, backed by the same public endpoint with a closed size query.
The manifest declares the real dimensions and ordinary icon purpose; Apple touch
and push notifications use the 192-pixel icon. Matching-size platform fallbacks
are rendered from the Admin-owned vector. Existing installed apps may update
icons on their browser/OS schedule; opening the updated app or reinstalling it
refreshes platforms that retain the previous installation icon. Missing/deleted crests fall back to the platform icon;
unavailable services return uncached 503 with Retry-After. Successful icons may be
cached for five minutes. The favicon resolver allows up to fifteen seconds for
a cold PNG response; this anonymous image request does not block homepage HTML. School titles use `ชื่อโรงเรียน - ระบบจัดการโรงเรียนครบวงจร` with matching Open Graph titles. School structured data retains the square `/favicon.png` logo. Social previews use the separate anonymous `/og-image.png` proxy for the current-school 1200×630 PNG banner from `/api/school/public/share-image`; the banner keeps the platform design if no public crest is configured. Homepage image URLs carry a stable public-branding/template version so name or crest changes produce a new advertised URL. The proxy always serves current branding regardless of query parameters, omits cookies, rejects redirects, bounds PNG bytes and allows fifteen seconds independently of HTML rendering. Successful responses cache for five minutes per host; genuine failures return uncached 503 with Retry-After. Bump the frontend template version in `school-public/seo.ts` when the banner design changes. Social platforms control the final card layout and may retain their own cached previews. Google chooses its final
search appearance after recrawling; deploying does not force immediate updates.
Public school identity and the central directory allow up to eight seconds for an anonymous API read before
returning their retryable errors; the independent marketing homepage and sitemap do
not wait for directory data.

After release, use Search Console's domain property to submit the central sitemap
and inspect the previously failing www URLs. Check HTTP and HTTPS www redirect to
the apex and finish at 200 before requesting validation. Report samples and crawl dates identify which historical URLs need validation. Maintenance intentionally returns bounded 503 responses
with Retry-After; do not bypass maintenance or return a success page to crawlers.

The frontend Worker serves the portal on the exact `<base-domain>/*` zone route.
Its exact `www.<base-domain>/*` zone route permanently redirects both HTTP and HTTPS
to the HTTPS apex while preserving path/query; no portal content or API Origin is enabled on www. The former
`admin.<base-domain>` hostname is retired and its Origin is not allowed. Preserve the existing proxied
apex A/AAAA/CNAME record; do not convert it into a Worker custom domain or delete
it to resolve Cloudflare error 100117. This leaves tenant DNS and routes unchanged.
Before maintenance, Pipeline checks the former alias's ownership without mutation.
After promotion, it explicitly detaches the former alias only if its exact
hostname belongs to `schoolorbit-frontend-admin` in production, then confirms its
absence. Wrangler alone does not remove a custom domain when none are configured.
Ambiguous ownership, unavailable lookup or failed removal keeps maintenance active.
The shared wildcard DNS may still resolve the retired name; do not delete that
wildcard because schools use it. Both normal and maintenance origin configurations
return uncached 410 for that hostname, without proxying it. The Admin Worker also
rejects it if a stale mapping reaches the runtime.
Review the exact apex and www routes for competing owners. Keep
Cloudflare Workers Builds disconnected so Pipeline is the sole deployment writer.
The Worker has no internal migration secret; remove any obsolete
`INTERNAL_API_SECRET` binding in the managed Worker configuration at cutover.
Never alter tenant routes or delete school Workers/DNS during this change.

Verified customers submit requests. Operators verify ownership, provision new
schools through the existing pipeline, then approve the matching school and
service period. Existing schools are assigned by verified customer email in the
operator request workspace. Approvals are transactional and serialize against
the school; concurrent decisions cannot grant duplicate periods. Renewal extends
from the later of now or the latest service expiry by one calendar month/year.
There is no automatic payment collection. Expiry is warning-only, independent of
school deployment/active status, and never blocks School login or reads. School
deletion has no public Admin HTTP route in this release. Account deletion,
suspension and school-data cleanup are not automatic subscription actions.

## Organization learning-area separation

Migration 116 separates ordinary learning areas from the administrative hierarchy under
SCHOOL while preserving unit/group IDs, members, grants and catalog owner snapshots.
Run the normal migration/release workflow under universal maintenance. The migration
blocks ambiguous or inconsistent mappings; remediate reported finding codes explicitly
and rerun through the canonical migration runner. It emits only aggregate moved-unit
and tree-grant review counts. Tree grants stay on their existing roots and are not
copied to detached learning areas. Verify the intended users' resource scopes during
authenticated acceptance. AC/ACAD-ACT retains its existing activity affiliation.

After 116, old binaries that create independent registry/unit rows or accept caller-owned
codes are prohibited. Deploy backend and frontend together and recover by rolling forward;
do not restore an older binary against the migrated schema. Confirm one owner per group,
newly created groups in catalog choices, separate member/grant controls and automatic codes
before ending maintenance.

Migration [132](../backend-school/migrations/132_organization_membership_domains.sql) preserves
existing membership identities, dates and chosen primary flags while allowing separate learning-area
and administration primaries. Display aggregation does not create ancestor memberships or grant
ancestor permissions. Overlapping primary intervals within one domain, malformed Workflow unit
selectors or selectors pointing to missing units block the migration with bounded finding codes;
inspect only safe aggregate/error evidence, remediate the affected source explicitly and rerun the
canonical runner. Never guess a missing primary, discard a selector or change applied SQLx history.

Deploy migration 132, backend, generated API contracts and frontend together under universal
maintenance. After any tenant applies 132, retain compatible writers and recover by rolling forward:
old writers assume one global primary and may replace dated membership history. Do not restart an
older binary against the migrated schema. Permanent unit deletion is separate from deactivation;
it refuses the school root, linked learning areas, every foreign-key reference and the functional
Workflow selectors guarded by this migration. Existing history remains authoritative and must not
be purged to make deletion succeed.

## Correspondence processes and signing evidence

Migration 121 is additive and preserves existing workflow windows and work assignments.
Deploy the backend, permission/API contracts and frontend together through the normal tenant
migration and universal maintenance workflow. After feature use begins, retain a compatible
runtime recognizing the private `correspondence_document` and `correspondence_signature`
File Platform purposes; recover with a forward release. Never edit the applied migration or
manually remove attached files, signature images, process snapshots or action/signing history.

Migration 125 adds an explicit directed mode beside fixed-template processes and register-scoped
routing settings. Existing records, template versions, book identities and immutable evidence
remain intact. Deploy its backend, generated API contracts and frontend together. After a directed
book is accepted, recovery requires a runtime supporting directed graphs; use a forward release.
Do not downgrade to a runtime that assumes every process has a template and current sequential step.

The clerk selects one first decision-maker. Without a configured first-recipient allowlist, any
active staff account with correspondence assigned-read and assigned-update permissions is eligible;
process managers can restrict that pool and set defaults separately for incoming, outgoing and
internal registers. There is no mandatory director stage. An explicit acting person can be selected
when their current effective permissions allow the work; position selection uses current dated
organization assignments rather than inventing an acting-position identity. A unit selects its
responsible lead; an ambiguous unit requires a specific person or position.

Each accepted directed instruction freezes actual recipients and creates central work atomically
with its annotations, signatures and history. Required branches wait for explicit acknowledgement
or completion, and then any configured reviewer. Reading is separate. Copies create a notification and read participation, without a My Work assignment; they never block the
book or grant assignment authority. Review defaults off, and a descendant cannot remove an
ancestor's required review. Reviewer work is created only when its required branches finish;
completed no-return branches finish the book automatically. The first decision-maker can also
record a terminal consideration without forwarding. Optional Close remains archival. Copies may still record read/acknowledgement after completion or
archival, without editing the finished document, reopening the book or creating required work.

Existing school/exact-unit correspondence managers can recover a pending required recipient or
activated reviewer through Reassign. This transfers the actual pending slot, preserving its parent,
purpose, onward rights and all review obligations; manager visibility does not grant completion
power. The transaction dismisses/closes old work and cancels its notification source before creating
the replacement work. The first decision-maker replacement must respect the current configured
eligible pool. Accepted old/new names and identities remain in immutable history. Reviewers enter
read participation when accepted, while their work and notification wait until review is ready.

Fixed-template correspondence sends resolve every required recipient once, including future sequential steps.
A missing active required recipient refuses the send transaction without creating work or
notification artifacts. Accepted membership snapshots remain valid across later staffing
changes. Active steps project through canonical My Work and central notification dispatch;
there is no correspondence-specific delivery worker. Source-only previews and final PDFs read
private originals through the File Platform and verify frozen file version/checksum identity.
Final PDFs are composed on the server from persisted revisions and evidence; these electronic
signature records do not claim PKI or certificate-backed digital signing.

Reusable signature assets remain owner-only. Authorized correspondence participants can view
only an image already accepted as signed evidence for that exact correspondence context.
Template changes create versions and never rewrite accepted process snapshots. A returned
record can receive a new content revision and resend under its existing source instance while
retaining prior document, recipient, annotation and signing evidence.

The correspondence usability list derives counts and current-step projections from existing
records and frozen process snapshots; it adds no migration or background job. Deploy its backend,
generated API contract and frontend together under normal maintenance. Saved-draft metadata edits
use the existing revision transaction and stop at the first send; returned records permit body
revisions while retaining their metadata. Registration numbers, register years/kinds, organization
and ownership remain unchanged. A rollback needs matching frontend/backend contracts and must
retain a runtime supporting the correspondence File Platform purposes described above; preserve
all existing summaries, document revisions, process/action history and file/signature evidence.

Migration 128 adds first-read receipts unique to each book/person. It backfills the earliest valid
immutable explicit Read and refuses migration with aggregate `CORRESPONDENCE_READ_BACKFILL_BLOCKED`
findings if historical identities/revisions/timestamps are unmappable or the earliest revision is
ambiguous. Keep maintenance active, preserve the audit, and reconcile through an owned forward
migration; do not delete evidence, skip the gate, or fabricate a later first read. Deploy matching
backend, generated contracts and frontend together. GET/detail/preview do not record reading;
the authorized read POST records the first successfully rendered revision and may update applicable
process/My Work read projections only for the locked current document revision, without acknowledging
or completing work. Historical renders retain honest first-book evidence without marking newer
content read, and newly activated slots receive their own current read timestamp. Repeated reads preserve
the original receipt and create no duplicate history, snapshots, or work-change broadcasts.

Register profiles now store number formatting and reusable text stamps. Numeric reservations and
register years keep their existing owner; new books freeze their formatted label and old books
retain their original labels/numbers. Text stamps freeze editable concrete content and normalized
placement into accepted annotations; bordered text uses the same padded geometry in preview/export.
Creating a resource-bound draft before upload can leave an initial no-file document revision;
attaching an uploaded PDF adds a revision that references its file identity, while sending changes
workflow state without uploading another PDF. Source previews and exports read/verify originals
and compose bytes on demand without persisting a generated PDF. This describes code behavior, not
measured storage or compute usage. After these fields or read receipts are accepted, recover by
rolling forward; older runtimes may drop new JSON fields when rewriting summaries or profiles.

Accepted text annotations without `textLayoutVersion` keep their original height-only fitting;
do not rewrite or infer their layout from UUIDs, dates, or document revisions. New/edited text
requires version 1, which fits both width and height; a bordered text box also requires version 1.
An exactly unchanged accepted legacy base annotation may carry forward into a document revision,
and accepted old mutation requests still replay before new-submission checks. Signature captions
retain their existing layout. Deploy the matching generated contracts/frontend/backend together;
after accepting versioned text, recover forward with a runtime that preserves these semantics.
No SQL migration rewrites historical annotations or signing evidence for this rendering change.

Staff leave uses additive tenant migration `126_staff_leave.sql` after directed routing 125. Configure category profiles, routing and policy source/effective dates before enabling staff submission. There are no seeded legal allowances or approval recipients. Before the first leave policy creates a source-owned process template, the schema is compatible with binaries that ignore the Staff tables; after policy/request activation, do not restore a binary lacking the source-template ownership guard. Preserve request/process/evidence history and recover by rolling forward to a compatible release. This feature adds no departure/return tracking or late-alert worker.

## School appearance loading and recovery

School appearance is persisted per tenant by `/api/school/settings/appearance`; its revision
prevents concurrent editors from overwriting one another. Deploy the matching generated API
contract, backend and frontend together. The public appearance endpoint returns only presentation
settings and uses the request Origin to select the school. Apply additive tenant migration
`127_school_appearance.sql` before enabling these endpoints. It expands existing settings and
preserves logo columns and identities. Its unique singleton index refuses historical duplicate
settings rows; stop and investigate reviewed remediation rather than choosing or deleting a
row or logo automatically. A missing settings row returns 404 for GET/PATCH; it does not create
an invented default save. Earlier logo-only binaries tolerate the expanded schema, but the
new appearance frontend/backend must ship together. Preserve expanded appearance values and
applied migration checksums during recovery. No production migration is part of draft review.

Each document blocks first paint on the same-origin `/school-appearance.css` read. Its private
ETag response revalidates with the backend; failed or timed-out reads return uncached stable
defaults for that document. UI font faces use self-hosted `/fonts/ui/` assets and the existing built-in Sarabun files. The initial font gate
uses an inline classic bootstrap in `app.html`, with no script download that can stall body
parsing. It waits up to three seconds before selecting a fixed system fallback; an independent
CSS deadline reveals readable system text if the inline bootstrap cannot execute. Delayed font completion never
replaces that initial fallback. No shared process cache or SPA-route appearance fetch is used.

After a save, font preparation precedes atomic CSS replacement. A font failure after database
commit is reported as a successful save with a presentation retry, retaining the previous usable
theme. Other documents pick up current settings on their next load, throttled visibility
revalidation, or a same-origin save signal. Personal light/dark preference remains device-owned.

## Curriculum credit criteria

Migration [135](../backend-school/migrations/135_curriculum_credit_criteria.sql) adds empty,
level-owned basic-credit criteria and an independent criteria revision to draft and publication
levels. It preserves existing graph/reference content, revisions and timestamps, appends the
published-level view fields without replacing its identity/grants, and captures levels through
explicit columns. Its atomic preservation audit is `curriculum.credit_criteria_added.reconciled`.
No legacy criteria or suggested preset is inferred. Saving criteria requires the current draft
token, criteria revision and explicitly reviewed covered grades; it leaves requirement, term-slot
and program-order revision baselines unchanged. Historical criteria stay pinned, copy keeps the
destination settings and discard restores saved settings with a monotonic criteria revision.
Criteria are report warnings; they do not enforce graduation or block publication.

Deploy 135 with the matching School backend, generated contracts and frontend through normal
Pipeline selection under existing maintenance, exact-version/readiness and authenticated read-only
smoke gates. Verify preservation through the canonical migration audit; lifecycle rehearsals use
owned synthetic databases. Production curriculum/settings writes are not acceptance checks.
On failure retain maintenance and repair forward through the existing release procedure. Presets
require an explicit staff choice and save; opening or reporting never writes user criteria.

Pre-135 School binaries cannot maintain or restore criteria semantics, and the compiled migration-history gate prohibits their use after migration 135. Keep a compatible 135-or-later writer and repair forward; never restore an older binary after this migration.

## Reviewed owned load fixture cleanup

[Operations](../.github/workflows/operations.yml) exposes `owned-load-cleanup` for the single
[reviewed catalogue](../scripts/lib/owned-load-cleanup/catalogue.json). This owner job uses
`schoolorbit-production` with cancellation disabled; the callable workflow does not reacquire
that group. It performs no load test, school reset, catalog retarget, runtime grant change,
or arbitrary SQL execution.

Stage only the catalogue's eight inert files through the existing authorized backend operator
transport at `/opt/stack/deployment/owned-load-cleanup/inputs/<operationId>`, with owner-only
`0700` directories and `0600` regular files. Staging transports data, never an executable or an
activation request. Verify the local source before staging:

```bash
node scripts/lib/pipeline-owned-load-cleanup.mjs verify-inputs "$PRIVATE_INPUTS"
```

The fixed input SQL and original 64 inputs remain immutable. The SQL's old process timestamp is
a conservative authentication cutoff, not the current PID. A fresh ActualExecutionBinding under
the shared lock verifies the sole rootless backend, exact reviewed registry image digest, public
ready/release identity, accepted state, and unchanged schema-138 timeline. Image configuration
IDs are recorded separately from registry digests; offset timestamps retain nanosecond precision.
A different business image or changed ownership data requires a reviewed catalogue update.

Explicitly confirm the fixed selectors; the generic form's default `sandbox` is rejected:

```bash
gh workflow run operations.yml --ref main \
  -f operation=owned-load-cleanup \
  -f request_id=cleanup-3962938a-54d0fe6f-73f5-49be-908f-4ec14aee486b \
  -f subdomain=snwsb \
  -f school_id=0e297ca4-0809-4aab-a03f-1915045257b8 \
  -f api_url=https://school-api.schoolorbit.app \
  -f options='{"action":"execute"}'
```

The workflow checks exact main, the latest prepared Pipeline gate, trusted accepted state and
absence of related open changes or active production work. It prepares through readonly SQL,
then holds a durable reservation while repeating the GitHub gate immediately before DML. Only
the separate commit phase can create/fsync dispatch intent and execute the reviewed SQL once.
A new main queued during the subsequent readonly observation does not invalidate a still-unchanged
serving process or release.

Before preparation, commit and post-dispatch reconciliation, the canonical
[DNS verification owner](../scripts/lib/pipeline-remote/api-dns-verification.sh) performs an
explicit verify-only path. It checks current provider ownership and exact API records, DNS,
public TLS and authenticated full context without DNS, proxy, maintenance or rollback mutations.
Only the catalogue's exact legacy journal filename and SHA can be classified as historical;
the original failed journal and later successful no-op verification history remain unchanged.
Current owned DNS journals use terminal `accepted` or `rolled_back`; intermediate states and
unknown legacy journals still stop cleanup.

The owner writes separate private verification evidence. It checks fixed TLS/smoke source hashes
and executes verified copies from private tmpfs. Cloudflare and smoke credentials remain confined
to that owner phase and are removed from the cleanup controller's environment. Proof must be
fresh within 60 seconds before DML. The actual binding compares accepted release and stable DNS
facts, while observation timestamps and run identities remain separate, so the 300-second watch
does not falsely expire the proof. Reconciliation classifies a proven pre-dispatch abort before
calling the DNS owner; that abort remains possible during a DNS outage.

The rootless driver resolves active tenant metadata through fixed loopback Admin HTTP, without
redirects or environment proxies, then obtains the existing tenant owner connection without
`include_inactive`. Admin owns decryption. Credentials remain in memory/private tmpfs, with
`0600` pgpass and the canonical CA; pinned PostgreSQL 18.6 verifies the exact owner role/database
marker and TLS. No human database-host SSH identity, new credential, or runtime privilege is used.

Fresh snapshot approval verifies the original 109 watch identities, original business MD5 and
counter floors before accepting the schema-138 baseline. Cleanup locks the queue before deleting
identified inbox rows, jobs and attendance source ledgers. It preserves all real catalog metadata,
the retained inactive actor/password hash, original ten audit rows and all outside fingerprints.
The actual PostgreSQL client exit is durable before parsing output emitted before COMMIT.

After a failure or controller loss, use the same dispatch with `options='{"action":"reconcile"}'`.
Never remove the interlock, change receipts, replay the old load controller, or manually rerun DML.
A proven dead controller plus absent client and no dispatch/exit/commit artifacts permits a
separate aborted-before-dispatch receipt; it is not successful cleanup. A later explicit execution
can prepare again because no dispatch occurred. Unknown or nonzero post-dispatch outcomes remain
blocked. A genuine bound exit zero can recover a missing commit receipt only after fresh owner
identity and exact preserved readback, without DML.

Successful completion requires durable receipt hashes and at least 300 seconds of preserved state
with unchanged serving release, image, PID and start identity, zero resurrected owned data/auth,
and monotonic counters. Interrupted observation may restart the full window but retains every
previous counter floor. Reconciliation can finalize a validated completed window after controller
loss. Journals stay private under `/opt/stack/deployment/owned-load-cleanup`; no raw business rows,
credentials or test-result attestations are uploaded. The common interlock refuses a surviving
fixed-name PostgreSQL client and malformed or unresolved cleanup evidence. The cleanup owner
additionally refuses unresolved release, rollout, TLS and DNS recovery work before starting.

## Assessment coordinators and editing controls

Deploy [migration 146](../backend-school/migrations/146_assessment_coordinators_and_edit_controls.sql)
with the matching School backend, API contracts and frontend under the existing universal
maintenance and all-tenant migration gates. It copies the former editing value into the
new exam-arrangement control for midterm/final, leaves other exam controls disabled, and
preserves existing control versions, plan phases and gradebook controls.

The migration fills missing coordinators on published course offerings in writable academic
contexts, excluding locked course results. It uses the Assessment suggestion rule: exactly
one active primary teacher covers every nonclosed group at the current date clamped to the
term. Ambiguous cases remain unassigned; every existing owner is retained. An existing plan
only gains its coordinator and revision; a newly created plan starts with four zero-score,
no-exam phases. Both publication paths apply the same Assessment-owned operation atomically
for missing owners, including retained targets; an idempotent replay performs no new fill.

`ASSESSMENT_146_PRESERVATION_FAILED` aborts the migration if preserved rows drift. Keep
maintenance active, investigate bounded evidence on an owned copy, and recover through a
compatible forward release. After the split is used, retain a runtime that respects both
controls; an older writer could couple the two decisions again. Never edit applied migrations
or republish existing offerings to trigger the deployment backfill.
