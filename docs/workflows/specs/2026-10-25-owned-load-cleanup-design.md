# Owned load fixture cleanup through Operations

Status: Senior approved the privileged owner job, fixed bundle and shared interlock direction;
this artifact records the approved execution contract.

## Outcome and scope

Add an explicit Operations executor for the reviewed synthetic fixture belonging to load run
`3962938a-e006-4b05-8bb0-20bcc3cd6bef`. The user chose the normal shared production lock,
not an exception. The operation performs no new load test, school reset, migration, release,
catalog retargeting, or blanket prefix deletion.

The immutable original manifest identifies the deletion closure. Preserve all outside business
rows, the real published activity and its exact historical catalog context, the retained inactive
synthetic actor and password hash, the original ten audit rows, and the original 109 file-watch
identities and business projection. Existing operational input hashes and commit receipts are
operational evidence; they are not uploaded local-test receipts or a CI attestation system.

## Authority and transport

Use a fixed `owned-load-cleanup` Operations choice and a callable main-only workflow under
`schoolorbit-production`, with `cancel-in-progress: false`. Allow only an allowlisted operation
identifier and an `execute` or `reconcile` action. Reject arbitrary SQL, commands, paths, URLs,
tenant selectors, and executable bundle contents.

The existing backend-host SSH identity runs a rootless remote driver. That driver asks the
canonical Admin migration-connection provider for the fixed tenant's owner credential using the
existing backend-school internal caller identity. Admin remains the only credential decryptor.
Use the pinned PostgreSQL 18.6 client, the existing verify-full CA mount, and remote-only private
temporary credential storage; never emit credentials, URLs, cookies, or raw business rows.

The current runtime role has CONNECT and DML grants but no database TEMP grant. The reviewed SQL
uses temporary tables and functions, so the approved direction is an explicit privileged owner
job rather than a runtime grant change. Existing privileged storage-attestation operations in
`backend-school/src/operations/tenant_rollout.rs` provide an owner-job precedent. Verify the exact
active tenant through ordinary Admin metadata, rejecting a pending directory mutation, then call
the owner endpoint without `include_inactive`. Verify the exact database, current role, database
owner, `schoolorbit:<tenant UUID>:migration` role marker, and full migration checksums before use.
Document this explicit owner-job boundary in `.rules` and Operations; ordinary runtime authority
remains DML-only.

## Reviewed input boundary

The repository owns the fixed operation catalogue and trusted executor. A private bundle carries
only the reviewed ownership data, SQL, original preservation contract, and typed operation
metadata. Validate exact hashes, ownership, restrictive modes, regular files, no symlinks or path
traversal, a bounded file list, and byte limits. Never run a supplied generator or executable.
Stage inputs to a derived fixed private path; dispatch cannot choose a filesystem location.
Staging only transports inert operation input. The shared-lock executor validates it again before
creating an intent or using a credential. Staging details must preserve this trust boundary.

The prepared private candidate is immutable evidence: its commit SQL SHA256 is
`913dac04605eb990fa9ec4907049f7ca81d61654e649b7aaaac16146be7d34d5` and its hash index is
`9649a38dd08f91bcd1fbcb899a5fda28e7f0689cb0f06c55f77040b3177b33ae`. It embeds the old
accepted release and process identity as its original reviewed SQL binding. Preserve those SQL
bytes and hashes. The old `backendStartedAt` remains a conservative authentication cutoff; it is
not a claim that the old literal PID is still running.

Inside the shared lock, capture a separate immutable `ActualExecutionBinding` containing the
fresh accepted state, public release, sole backend container/PID/start/image and trusted source
fingerprints. Require the actual process start to be at or after the reviewed cutoff, and all
owned sessions to have been revoked before that cutoff. Require the reviewed backend image
`sha256:f983b8e4593e2a7b480b13643c9778e7b79a55c6fa7eb72f6987a8562fd89043`, with the full
schema-138 timeline unchanged. A different image requires an explicit reviewed catalogue update. An Operations-only
deployment can reuse that image even when its public release identity or PID changes.

The execution journal links the fixed reviewed payload hashes to this fresh actual binding;
no caller supplies SQL or performs source substitution. The approved capture is generated only
from fresh validated reads. Do not rewrite the old candidate, original 64 inputs, historical
reports, or preservation fingerprints. A new business image, schema, ownership input, or source
outside the reviewed compatibility boundary stops execution for a new review.

## Cancellation and recovery

GitHub concurrency does not protect a remote command after controller loss. Before dispatching
SQL, atomically create and fsync a durable cleanup interlock and once-only journal on the backend
host. Every production mutation owner must refuse this interlock: release and maintenance entry,
Admin/tenant provisioning, DNS and certificate operations, frontend recreation, and retention.
Do not borrow `pending-release`, whose existing semantics belong to maintenance/release recovery.

The journal records input identity and intent before DML, then commit outcome, reconciliation,
watch progress, and completion. A cancellation, timeout, lost connection, malformed receipt, or
unknown commit leaves the interlock intact. No automatic DML retry or replay is permitted.
The reconcile action runs under the same GitHub group, performs only reviewed reads and the
remaining observation, and cannot dispatch cleanup DML. An inconclusive result remains blocked.

## Database and preservation protocol

Fresh preflight binds the accepted release, sole backend container/PID/start/image, exact tenant,
schema 138 and checksum timeline, all 3,001 owned users, and the 418 unchanged revoked sessions.
All owned session creation, last-seen and revocation timestamps must precede that process start;
there must be zero raw unrevoked sessions and no intervening owned login. Lock all owned users
first to fence final login issuance. Recheck process/source continuity through completion.

Validate exact manifest relationships, closed default controls (4 assessment, 4 gradebook,
2 learner evaluation), grants, absence of PII/guardian/push extensions, source-to-job mappings,
and incoming foreign keys. Lock delete parents and dispatch jobs with NOWAIT; any contention or
outside reference aborts the complete transaction. Keep the central owner order: lock queue jobs, delete identified inbox rows, delete jobs, then
delete attendance source-ledger rows.

The same fresh snapshot returns the original eleven-field watch projection's row count and
MD5(sorted MD5(projected row)), alongside watch IDs and monotonic counter values. Approval must
compare it to the immutable original 109-watch evidence before creating the approved snapshot.
Within the atomic cleanup, compare the approved snapshot, delete only the exact closure, set
only the retained actor's status inactive, and reconcile outside fingerprints, original audit rows,
all session hashes, catalog facts, context, and watch preservation before commit.

After commit, use a separate connection for reconciliation and observe for at least 300 seconds.
Require zero resurrected owned data and raw unrevoked sessions, unchanged process/release/source,
exact preserved business facts and watch IDs, and nondecreasing declared counters. Keep workers
running. Only a durable successful receipt and completed watch can release the active interlock.

## Approval gates

Before implementation, Senior and Reviewer confirm the separate actual binding, inert staging
boundary, and complete production-mutator interlock coverage. Independent Review and QA accept
the frozen implementation and disposable rehearsal before Senior performs production operations.
No production execution is authorized by this design artifact alone.
