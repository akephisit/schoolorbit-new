# Timetable drafts use the latest opening publication

Approved working agreement, 2026-11-01. This changes the earlier explicit draft-source selection policy. Published timetable source pins remain immutable.

## Problem and outcome

Selecting the dated applicable opening can move a draft from a newer opening to an older graph and produce misleading teacher mismatches. Draft workflows must use the highest Delivery publication number in the exact term, without exposing an older-source choice. Alignment preserves every stored placement, teacher, child identity and row revision; genuine mismatches remain visible for review.

“Latest” governs active workflows: newly created or resumed drafts, successful draft content writes and new publications. Dormant or read-only drafts may retain an older persisted pointer and must truthfully show pending alignment. GET requests never mutate, and Delivery never writes Timetable rows on publication.

## Owners and contract

- `school-academic-delivery::services::versions` remains the publication selector. Highest publication number selects the planning source, including future publications; dated operational selection remains separate.
- `school-academic-timetable` owns alignment, draft mutation admission, validation and source issues. `school-academic-http` retains transport, central session validation and resource-policy enforcement.
- Keep `PUT /api/academic/timetable-versions/{version_id}/delivery-source`. Its closed request contains only `rowVersion`; the owner resolves the latest published source under the exclusive term lock. A client-supplied `deliveryVersionId` is rejected.
- Add `TimetableAction::AlignDeliverySource`, authorized by existing school-wide Manage **or** Publish. Scoped Manage and read-only grants cannot align the whole draft. This grants no other editing, cloning, deletion or publication permission.
- Add the typed publication finding `delivery_source_outdated` and placement conflict code `TIMETABLE_DELIVERY_SOURCE_OUTDATED`. Retain pinned/latest publication contexts; remove `TimetablePublicationPreview.applicableDeliveryPublication` and its older-source action.
- Definitive pre-write latest-source guards return HTTP 409 with optional `data: TimetableDraftSourceConflict`, whose `code: TimetableDraftSourceConflictCode` has the sole value `TIMETABLE_DELIVERY_SOURCE_OUTDATED`. The generic AppError/HTTP owners transport a producer-owned code without domain dependencies. Ordinary conflicts omit this data; status or human text never proves that a write was rejected before commit.
- Timetable workspace date rules may compose the latest source start with Delivery's existing active-term lower bound. Delivery's own date-rule meaning stays unchanged.

## Persistence and concurrency

Fresh creation and cloning select the latest source in the acceptance transaction. Cloning retains the original published timetable in `source_version_id` and preserves the existing child-copy/provenance rules. Existing draft resumption, including the initial-create endpoint's existing-target branch, uses an Existing CommitPermit rather than mutating under a CreationPermit. Term-preparation imports retain their exact target-context mappings and latest target source.

Alignment uses the existing accepted task, tenant lease, draft gate, post-gate session/authorization revalidation, WholeDraft exclusion, exclusive term lock and optimistic row revision. It updates only the draft source pointer, version revision/time and existing source-update audit. It never reassigns teachers, removes lessons or rewrites the origin link. Conflicting leases refuse alignment; an individual block lease cannot authorize WholeDraft changes.

A confirmed no-op changes neither audit/revision nor room epoch. Changed alignment invalidates the room after commit and requires an authoritative full workspace/new epoch; a partial block delta cannot carry replacement publication contexts. Unknown commit outcomes retain existing invalidation and reconciliation behavior. Accepted writes survive caller cancellation; there are no generic automatic mutation retries.

After a successful business commit, unavailable cached version metadata or failed delta publication invalidates the room and preserves the accepted success. It cannot escape as a rejected-write conflict. Frontend retains failed captured edit/placement/removal intent for explicit review. A typed server source rejection (`source_rejected`) or locally proven failure before child HTTP dispatch (`not_submitted`) permits latest alignment after the batch drains in the current context, followed by review and a fresh explicit Save/lease. Only the exact code certifies server pre-write refusal; local proof does not claim a server rejection. Neither class replays the child mutation. Unknown outcomes after dispatch remain fenced until explicit reconciliation/discard, and GET never clears every save error automatically.

All draft content mutation families revalidate latest-source membership under the term lock before writing. Read-only placement preview cannot authorize placement against an obsolete source. Publication preview reports an obsolete source; new publication rechecks it along with the preview hash. Accepted publication and handoff receipts replay before new-source checks, preserving accepted identities and historical facts.

## Workflow and dates

Frontend aligns automatically through the existing lifecycle operation on Edit, Resume, Open Publication and coalesced Delivery reconciliation while actively editing. It fences captured context/session/version, drains captured autosaves and releases its own idle leases, retains unsaved form input and reacquires a valid lease after full reconciliation. Other editors' leases are never force-released. Read-only navigation performs no alignment request; scoped actors see an explicit blocked stale state.

Initial publication date is the maximum of Bangkok today, the latest source start and the existing minimum. Preserve later user date choices. A future latest opening is available for planning but cannot make a timetable operational before that opening's start. Source-date eligibility, active-term no-retroactivity and consumed-history guards remain authoritative; there is no dated fallback or early activation.

No schema change is planned. Applied migrations through 151, publication numbers, raw episode/effectivity provenance, historical pins and stored receipts are untouched.

## Acceptance

Prove old UUID input rejection; byte-identical draft children after alignment; latest selection independent of dates; preserved published/origin pins; both creation/resumption permit paths; stale-source denial across mutation families and between preview/commit; no-op, conflicting leases, queued revocation, cancellation and room invalidation behavior; school Publisher-only alignment with unchanged other permissions; exact accepted replay; future-date/history refusal and term isolation. Independent Review and QA assess the frozen candidate using the owning checks in the implementation plan.
