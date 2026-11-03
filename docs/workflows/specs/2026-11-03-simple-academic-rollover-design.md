# Simple academic rollover

Status: user-approved design, recorded for implementation on 2026-11-03.

School staff close a period and prepare the next roster without needing locked course results, annual credit calculations, SGS-style promotion policies, or a separate approval ceremony. Preserve student identities and lightweight year, grade and room history. The existing year lifecycle page becomes the unified year/term setup, next-term and next-year workspace.

## Ownership and behavior

Academic Core owns years, terms, annual enrollment and dated homeroom placement. Academic Lifecycle owns closure, roster preview/confirmation, replay receipts and retention policy. Students owns administrative registration assertions; Auth owns account access. Application adapters compose Attendance, Behavior and Files through their public provider operations.

Term closure preserves the existing annual roster. Preparing the next term does not duplicate annual enrollment or promote grades. Year rollover shows every source student and a proposed destination; staff choose promote, repeat, graduate, transfer out or hold, review a complete preview and confirm once. The effective placement date is explicitly supplied. The destination is prepared, never implicitly activated. Opening the destination remains an independent authorized operation.

Retain existing promotion run/item/approval/execution receipts and their identities. The canonical manual flow reuses the existing ledger with nullable policy and recommendation evidence where manual intent has no such prerequisite. Do not invent result, credit or policy evidence. Preserve all previously accepted evidence, historical conditional outcomes and replay semantics. Reopening remains blocked by completed/executing promotion and successor dependencies. Legacy in-progress work must be detected and recovered explicitly. Retiring old command paths must not delete historical facts.

Graduation and transfer update enrollment through Core and, when needed, administrative registration through a Students-owned transactional operation. They do not silently disable accounts, revoke sessions or delete student/guardian identity. Results remain available as reports and warnings; they do not authorize or block manual roster choices or closure.

## Detail retention

Retention is optional and distinct from score reset. Existing schools start disabled. An explicit enable sets a server-owned effective timestamp; automatic eligibility includes only subsequently closed periods. No migration, page read, setting save or next-year preparation deletes existing details.

Lifecycle owns a versioned policy with primary and secondary minimum retain-through boundaries (`term` or `year`) and a configurable grace period, initially 90 days. Suggested boundaries are year for primary and term for secondary. Policy activation requires the relevant academic-management, Attendance-delete and Behavior-cleanup authority.

Current providers archive whole terms. Use the latest required boundary among all education levels in the annual roster, including members with virtual Attendance coverage. A mixed primary/secondary term waits until year closure if primary requires year retention. Unknown classification blocks automatic cleanup. The UI explicitly explains that shared term details are removed together and mixed levels wait for the latest boundary. Do not pretend that primary and secondary details can currently expire independently.

Use actual accepted close receipt timestamps: migration 148 removed academic calendar boundary columns. Reopen/reclose invalidates the old deadline; an archived period cannot reopen as writable detail. Record a durable per-term job and policy revision before deletion. Bounded background work resumes accepted progress, does not depend on a user opening a page, and never makes next-period opening wait for purge.

Attendance's provider freezes and reconciles school and teacher summaries, checks source-score/notification delivery and requests evidence deletion transactionally through Files. Behavior's provider deletes details only, preserving current/history totals, cycle ownership and minimal replay/suppression evidence. Reuse its durable chunk jobs. File metadata deletion and physical object/version reconciliation are separate: keep manifests and report pending physical cleanup honestly. Never delete bucket objects directly.

Keep identities, annual/room history, aggregate summaries, existing results, central audit and immutable replay evidence. This release's automatic cleanup covers Attendance detail/evidence and Behavior detail; unrelated modules and course-result history are excluded. No actual existing-tenant or production cleanup is authorized by implementation or deployment.

## Migration and acceptance

Use new sequential migrations after 151, with Backend owning 152 and retention owning 153. Applied migrations and the narrow canonical 149 recovery are immutable. Additive policy/job schema defaults off and does not enqueue historical cleanup. Any semantic ledger changes reconcile counts, identities, accepted receipt content and references before consumer cutover. Fail closed on unknown layouts and retain bounded aggregate diagnostics.

Acceptance covers stale preview rejection, actor/tenant authorization, exact replay identity, concurrent confirmation, all five manual outcomes, prepared destination, unchanged UUID/history, closure without Results, same-year roster continuity, mixed-level deadlines, disabled/future-only retention, reopen invalidation, queue refusal, aggregate preservation, restart recovery and pending Files reconciliation. Generated API and permission contracts follow their source owners. Production rollout uses the canonical migration and maintenance gates and compatible forward recovery.
