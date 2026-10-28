# Current result obligations after Delivery room removal

## Authorized outcome and limits

This separate follow-up builds on deployed Delivery room scope. A currently included M6 group must be able to complete normal course/activity/learner-evaluation locking when an explicitly omitted, unused M4 group remains in the shared registry. Any established learner obligation retains that group's existing closing requirements, even when the current Delivery graph omits it. Keep teaching identities, settings, membership episodes, criteria, scores, evaluations, results, locks and published history intact.

The verified gap is raw nonclosed-group selection in Results `services/readiness.rs` and `services/locking.rs`, plus learner-evaluation current lock selection in Assessment. Change only current readiness/lock selection. General subject/workspace/search/history readers, Exam sources, assessment criteria and established assessment obligations remain unchanged. This is not a grading reset, replacement-instance feature, migration or new permission/API contract.

## One authoritative classifier

Results owns the batched public operation:

`current_obligation_group_ids_on_connection(connection, academic_term_id, academic_year_id, date, candidate_group_ids) -> Result<Vec<Uuid>, AppError>`.

Validate every distinct candidate's group/offering/term/year context. Unknown or mismatched IDs fail closed. Capture one Bangkok calendar date per operation, clamp it once to the selected term bounds, and use that same date for classification and current readiness/lock checks. Resolve the applicable effective published Delivery source through its public batched audience provider; future restrictions do not activate early.

Retain a candidate when Delivery returns None (unchanged registry semantics), the explicit applicable source includes the offering/group, or any established learner history/obligation exists. Exclude only Some explicit omitted offering/group plus zero established evidence. Presence is sufficient regardless of active/removed/ended status or invalidation. Do not delete or rewrite evidence to make a group disappear.

Use the correct authoritative evidence owners:

- Delivery: any membership episode/history, including removed or ended membership.
- Assessment: leaf group grading items, student scores, phase confirmations and learner-evaluation facts/history, including invalidated confirmations and established evaluation sources.
- Results: course overrides, group result confirmations, activity evaluations/outcomes, earned results, result locks and their recorded source/history facts.

Results composes narrow public batched Delivery/Assessment providers with its own evidence. Do not introduce foreign private-ledger SQL in Results or HTTP. Assessment's evidence is a leaf and must not call the Results port. Shared offering assessment plans, teachers, targets, curriculum preparation receipts and timetable configuration alone are not learner obligations for every registry group. Preserve all such facts even when they do not establish closing obligations.

Classify before existing normal eligible-group limits. Use a separate named hard raw-candidate scan budget and one extra row to detect overflow; fail explicitly instead of truncating. Omitted unused groups must not consume the normal output limit. Preserve normal limits after classification and existing permission filters.

## Dependency and application boundaries

Results adds only the verified acyclic local dependency on Delivery, with api-docs forwarding and lockfile changes as needed. Results already depends on Assessment. Assessment must not depend directly on Results.

Extend the existing Assessment `ports::ResultLockPort` with a narrow batched current-obligation operation carrying the caller's transaction, term/year, captured date and candidate IDs. `src/adapters/academic_result_locks.rs` delegates to Results. Academic HTTP passes its existing injected port only into learner-evaluation current `read_lock_readiness` and `lock_subject`. This reuses the Results classifier without adding a runtime call cycle or a second policy.

Results course readiness and `lock_course_subject` use that classifier; activity readiness, bulk locking and direct group locking agree. Assessment learner-evaluation readiness/subject locking uses the port. Lock operations retain offering-then-group ordering and classify/recheck evidence under those locks. Existing domain/subject locks stay conservative. Established facts committed before a waiting classifier obtains the offering/group locks are retained. When the result lock wins first, a Gradebook first-history writer rechecks the existing Results lock and refuses under its current guard. A lock-first backdated Delivery roster-membership request retains the existing admission policy: if accepted, it remains a fact included by later classification while immutable old snapshots stay unchanged. This change introduces no new post-lock roster prohibition.

Direct locking of an explicitly omitted, unused activity is refused; it must not create an empty activity lock. An empty-only subject must not receive a fabricated lock. Included-empty and legacy groups keep existing validation requirements. A ready M6 fixture must have valid grading prerequisites; a preexisting missing-plan failure is not proof of correct filtering.

## Private Delivery context reuse

In `school-academic-delivery/src/services/offerings.rs`, scoped preparation returns a private owned preview plus its optional already locked `RevisionContext`. The preview endpoint returns the existing wire preview. Apply validates its hash/choices and reuses that exact context for application and cleanup, avoiding the redundant full graph read/decode in the same transaction.

The builder only reads/locks while creating the preview, and revision/version locks remain held by that transaction. Preserve lock acquisition, exact hash inputs, request/receipt identity, replay/stale behavior and the unscoped Lifecycle path. No schema, wire, generator or policy change is needed for this private correction.

## Verification and recovery

Required families: actual valid M6 course-lock success with unused omitted M4; isolated evidence categories retained with exact history bytes; legacy/included-empty/future-boundary behavior; activity readiness/bulk/direct agreement; learner readiness/lock agreement with criteria and existing locks intact. Add real-owner concurrent first-history serialization, unknown/wrong-context refusal, raw-budget refusal and normal limits applied after filtering. Retain existing scoped preparation/replay, projection, inheritance and dated/pinned attendance checks.

Rehearse mutations only in owned disposable fixtures or protected copies. Production acceptance remains the existing read-only schema/provider smoke and maintenance gates. No backfill is authorized; migration144's compatible roll-forward requirement after audience intent exists is unchanged. This follow-up has its own draft PR and reviewed inputs. Record its completed outcome and actual checks there before removing these temporary artifacts.
