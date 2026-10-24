# Deferred browser uploads and durable physical cleanup

Approved extension to Draft PR 171 on 2026-10-24. Existing shared database/runtime/query changes remain in place. The user authorized implementation after reviewing the browser save/upload and cleanup flows. Merge, deployment, production operations and load testing remain excluded. Senior owns Git and acceptance; separate writers own backend/data/pipeline and frontend paths.

## Browser and server boundary

Selecting a new file, cropping/editing/layout changes and preview remain in the browser. The browser uploads the final local file once after explicit Save/Submit/Confirm; Save Draft also counts as a commit. Cancel or replace before confirmation disposes only the local File/Blob URL and never issues a storage DELETE. Selecting an existing stored/library file never uploads it again.

The server continues to own inspection, authorization, relationship attachment, official signatures, permissions, audit and final rendering. A local preview of an unsaved file cannot substitute for server validation. If upload succeeds but the business save fails, retry reuses that uploaded file ID. An ambiguous save reconciles the existing mutation intent before cleanup; request timeout alone does not authorize deleting an accepted file. Accepted/referenced/history files are never cleanup candidates merely because they are old.

Delete/purge validates current domain references, commits revocation in PostgreSQL, then performs provider deletion. Existing synchronous operations and durable retries remain. Backend recovery applies independently of browser behavior. This design adds no public HTTP contract or generic queue/crate.

## Verified recovery gap

File Platform reserves original and derivative locators before provider PUT. Derivative retries reuse one immutable file/version/recipe key and verify expected size and SHA-256 before writing. Current exact owner/attempt/expiry fences protect database effects; an in-memory write-start flag can request cleanup only when fenced finish executes.

Migration 039 defines the current finalize_certificate_campaign_purge function. It deletes operations, derivative/version locators and files, and purge inventory cascades away. No later migration through 135 replaces that function. Therefore scanning remaining file metadata alone cannot recover a PUT which completes after hard purge. Already-purged unknown locators are not reconstructable from current rows.

The R2 adapter uses conditional If-None-Match:* PUT and ordinary key DELETE. It returns no object-version/ETag/settlement receipt. SDK retry ambiguity and dropped client futures do not establish server-side quiescence. HEAD404 or successful DELETE is an observation at that time, not proof a previous PUT cannot later complete.

## Additive migrations 136/137 and immutable physical identity

Introduce file_storage_cleanup_watches, owned by File Platform, independent of deletable domain rows. Suggested columns:

- id UUID primary key; immutable file_id, target_kind (version/derivative), target_id; no FK to files, operations, versions, derivatives, campaigns or purge inventory;
- provider_code, storage_class, object_key and content_type; a unique (provider_code, storage_class, object_key) groups all writes to the same logical physical target and forbids retargeting that key to another namespace or target;
- namespace_fingerprint nullable, with a fixed validated fingerprint format; NULL means legacy/unverified and forbids provider cleanup;
- may_write_since and write_generation as conservative durable write-permission markers, a monotonic row_revision incremented on every update, not assertions that network IO actually began;
- revoked_at as irreversible cleanup authority; a revoked key cannot be reused or rebound;
- next_probe_at; cleanup lease_owner, lease_generation, leased_at and lease_expires_at with consistent-null/positive checks;
- last_probe_at, last_absent_at, last_error_code restricted to safe codes, namespace_bound_at/binding_evidence_digest for explicit attestation, created_at and updated_at.

Do not retain payload, user names, original filename, credentials or signed URLs. Do not automatically delete watch rows or mark uncertain work permanently complete. All permitted writes are conservatively treated as potentially outstanding; a later successful generation cannot erase uncertainty from an earlier one.

New upload reservation registers each validated bound locator in the same transaction before metadata/network work is committed. Synchronous reservation may record permission for its immediate original/derivative PUTs; asynchronous claim preparation records permission under the exact existing operation claim. Only owned permit/operation data leaves the DB phase.

Migration hooks preserve locators for every metadata insert and before physical metadata deletion, and activate watches when file revocation commits. Insert hooks for unaware/legacy writers create unverified locators, never guess a binding. BEFORE DELETE hooks on derivative/version rows preserve and revoke the exact locator before migration039's finalizer removes it. Applied migrations remain unchanged. Registering an existing key must compare immutable target identity, namespace and revocation, not silently ON CONFLICT-overwrite them.

Backfill every surviving locator including already-deleted rows, preserving known identity and marking unknown namespaces unverified. Activation and backfill must be transactionally safe against concurrent registration/deletion. Deployment requires draining old writers; no deployment is performed by this task.

## Storage namespace binding

StorageProvider gains a required narrow namespace(storage_class) capability returning an opaque validated StorageNamespace. There is no production fallback. R2 derives its fingerprint using SHA-256 over unambiguous length-delimited provider identity, canonical endpoint/account and the actual selected bucket. Credentials, region and CDN/public URL are excluded: credential rotation in the same physical bucket remains valid, whereas endpoint/account/bucket changes mismatch the persisted binding.

Every PUT and DELETE compares the persisted bound namespace with the current provider before IO. Worker source reads used for derivative generation also validate their source binding. New uploads record the current explicit provider namespace during reservation. Migration136 cannot attest where old objects were stored: backfilled NULL bindings remain blocked rather than inferred from current configuration. Legacy remediation requires independently verified historical namespace ownership and an explicit narrowly scoped operator binding of enumerated records; an unprovable record stays blocked. No automatic binding or provider bucket discovery is introduced.

Implement a narrow offline file_storage_namespace CLI using existing privileged bootstrap and explicit DbSession/transaction boundaries. Its manifest enumerates watch UUIDs, expected immutable-locator SHA-256 digests, expected row_revision (covering write generation, revocation, binding and lease state) and attested namespace fingerprints; it never contains file payload, credentials or signed URLs. A dry-run validates all identities, provider namespace and absence of active cleanup claims. Apply additionally requires explicit historical-namespace attestation, locks all manifest rows in stable UUID order before any mutation and rechecks every expected identity/revision in one guarded transaction and refuses changed rows, bound-to-different namespace, revocation mismatch or active claims. Output contains aggregate counts/safe codes only. A HEAD404 is never namespace evidence. Custom GUCs are not privilege barriers: the NULL-to-bound trigger independently requires the actual watch-table owner or equivalent verified owner-role authority, and apply verifies that authority. Runtime DML roles cannot bind legacy rows even when setting the evidence GUC. New bound INSERTs remain available to normal reservation writers. No broad SECURITY DEFINER helper is exposed.

Dry-run/check use an explicitly nonmigrating read-only database path, never permission sync or claim mutation. A read-only check mode inspects all retained watches in the selected tenant database, including revoked/purged targets, and exits nonzero while any namespace remains unresolved or mismatched. This is an explicit deployment gate, not a health-endpoint or timeout-policy change: Maintenance Mode and drain old writers, apply canonical migration, independently attest and bind verified legacy locators, require a clean check, then enable matching writers. This task performs no production execution. Unprovable namespace ownership keeps rows blocked and prevents declaring rollout ready. A changed storage namespace never silently rebinds an existing watch. Same-name bucket destruction/recreation cannot be detected from the current provider API; continuity and verified operator procedures remain explicit assumptions.

## Proposed typed backend entries

- StorageNamespace is opaque, cloneable/equatable, without Debug/Serialize or raw provider credential access. A trusted provider-locator constructor computes the versioned fingerprint; persisted parsing is crate-private.
- StorageProvider::namespace(StorageClass) -> Result<StorageNamespace, StorageError> is required for production implementations. No default or fallback binds unknown legacy objects.
- NewUpload carries its explicit namespace. FileRepository::reserve_upload(&NewUpload) -> Result<UploadWritePermits, RepositoryError> commits before returning original and derivative WritePermit values containing watch identity, exact StoredObject and bound namespace. FilePlatform uploads only these permitted objects.
- DerivativeWork carries persisted source/target bindings; DeleteWork carries an optional persisted namespace so an unknown legacy binding fails closed before provider IO. Existing OperationClaim remains authoritative for ordinary worker effects.
- FileRepository::claim_cleanup_watch(worker, lease, available_namespaces) -> Result<Option<LeasedCleanupWatch>, RepositoryError> returns only owned locator, exact cleanup claim and bound namespace. Namespace filtering prevents blocked legacy records starving runnable work.
- FileRepository::finish_cleanup_watch(&LeasedCleanupWatch, CleanupObservation) -> Result<(), RepositoryError> validates exact owner/generation/wall-clock expiry before bookkeeping. CleanupObservation contains Absent, Deleted, or Failed with a fixed safe error code. All observations reschedule rather than certifying permanent settlement.
- The watch provider performer is network-only, accepts an owned claim/locator plus FilePlatform, and never receives a pool or repository. Root Cleaner owns phase composition, tenant proofs, limits and fairness. The offline binding command owns only explicit namespace attestation, not new migration/queue/HTTP machinery.

## Uploaded files after failed business saves

The frontend owner's initial audit identified 24 upload calls across 21 files, five early-selection flow groups and retry gaps; existing correct flows remain unchanged. Backend must audit domain-owned staging, expires_at, reference checks and promotion for Budget, licenses, Supply, signatures, Question Bank and other affected uploads. A successfully uploaded ID is reused on save retry; a confirmed abandonment is eligible only under the existing domain-owned deletion/staging policy. Expiry must never delete an accepted/referenced/history file merely for age. Fix missing promotion or relationship guards at their authoritative owners if the audit proves a gap; do not add a blanket all-file age sweep. This backend audit remains an implementation prerequisite, not a claimed completed result.

## Recovery phases and fencing

Extend the existing FileReconciliation tenant turn and bounded continuation, without new permits, pool limits or timeout policy. Alternate bounded opportunities for ordinary operations and recurring watches within the SAME total batch/permit/120-second budget. Either queue can use an empty peer slot; a continuously backlogged queue cannot consume the other queue's next opportunity. Probes count against bounded per-turn work. Persist due time and order by next_probe_at/id so prefix failures and restarts do not starve later targets. Repeated absence is rescheduled with capped backoff. Migration137 adds a per-watch prefer_watch_next bit, atomically flipped on the oldest runnable verified target when a NEW sweep chooses its first queue. This survives lost local continuations and process restarts; watch and ordinary failures cannot reset every new sweep to ordinary-first. Claiming a watch also advances its due timestamp before IO, so repeated lost finishes do not monopolize the oldest-key prefix. Check/attestation requires both136 and137, without modifying136 after its committed/disposable application.

1. Resolve current tenant work proof outside DB admission. Under the existing phase deadline and before/after pool proof validation, claim one due revoked watch using SKIP LOCKED and an exact owner/generation/expiry claim. Return owned locator/namespace data, then release DB resources.
2. Check provider namespace, HEAD and when present DELETE, using a bounded external phase. Do not hold a connection, transaction, TenantPool or shared DB permit during provider IO. No automatic PUT retry is added.
3. Re-resolve the same immutable tenant UUID, acquire an admitted DB phase, and update only the matching unexpired cleanup claim using wall time after any lock wait. Cancellation or stale claims leave durable work for a future pass.
4. Record absence/delete/error observations and schedule another probe. Existing file/operation metadata and accepted generations are untouched, including when the original domain rows no longer exist.

Cleanup authority comes from irreversible physical-key revocation, never from an expired write claim. Active accepted targets must not be deleted because an earlier worker was reclaimed. Old and new attempts share a target key; they share one watch. Existing synchronous provider work and failure retry paths remain immediate/owned, while watches catch interruptions beyond their completion path.

## Guarantees, remaining limits and verification

The forward guarantee is eventual recovery of recorded revoked keys, conditional on eventual provider completion, retained tenant database/storage namespace and fair successful recovery progress. There is no atomic database/storage commit or bounded orphan-absence guarantee. With no current provider settlement proof, uncertain watches remain scheduled indefinitely; document persistent row and provider-probe costs. Versioned storage would require version-aware deletion before asserting all versions erased.

Migration cannot reconstruct previously purged locators or prove past namespace ownership. Operator inventory/backup reconciliation for those historical unknowns is a separate reviewed activity; do not guess keys, scan/delete an unreviewed bucket, or claim historical recovery completed.

Required tests include independent remote PUT completion after client cancellation, DELETE, HEAD404, restart and the real039 finalizer; watch survival; repeated absence/reappearance; exact cleanup claim steal/expiry; stale write generations versus accepted targets; same-key grouping; namespace rotation/credential-only rotation/legacy-unbound refusal; fair bounded progress and DB-resource release during provider IO; synchronous upload/delete compatibility and existing tenant proof checks. Browser tests must cover no upload before commit, final-file single upload, local cancellation/replacement, library reuse and uploaded-ID reuse after save failure. No skip or weakened assertion substitutes for these cases.

Checks are scheduled through owning runners with pinned toolchains and one Cargo owner. No Cargo or database fixtures run until Senior grants a serialized slot. Independent Reviewer and QA assess each coherent checkpoint and the final integrated candidate. Remove these workflow inputs only after PR171 records the completed extension; canonical rules/testing/operations retain durable contracts and limitations.
