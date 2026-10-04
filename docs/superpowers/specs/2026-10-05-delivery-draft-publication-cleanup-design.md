# Delivery draft publication and permanent deletion

Approved by the user on 2026-10-05. Delivery and timetable remain separate existing crate owners; no new crate, permission codes, or general page-view API is warranted.

Delivery drafts have a nullable effective date and a separate stable reference date for editing. Existing draft dates become reference dates; published dates/IDs/snapshots remain intact. Candidate-date preview is read-only; publication recomputes the candidate graph with exact episode IDs, locks term/revisions/resources, compares date-bound hash and row versions, then materializes it atomically. General preview is explicitly preliminary. New drafts choose a bounded Bangkok-date reference from the term/latest published source. Published consumers reject a missing date.

Typed semantic changes compare source and target snapshots by IDs (offerings, groups, teachers, roles, targets and weekly periods). Change-set detail carries names and before/after changes so the visible region is independent of the lazy offering tab. Findings have collision-free render identities and named correction links.

Hard deletion permits only draft/cancelled versions. Delivery deletes the journal and exclusive unreferenced draft resources; timetable deletes owned placements. Both enforce optimistic revision/counts, scoped authorization, reference checks, atomic audit, and FK rollback. Published versions are immutable. Cancel endpoints are retired. Deletion propagates existing academic change signals and open clients reconcile deleted IDs.

The one-time cancelled cleanup targets SNWSB term 1d4731b6-48a5-408b-8e5b-1afcf6b01766 only. Discover IDs/revisions/counts and preflight all references; if any target is blocked, delete none. Invoke the same domain deletion operations inside one transaction, never raw deletion or trigger bypass. Keep backups and audit according to normal operations policy, without a parallel product archive.

New forward migration only, after applied 088. Published fingerprints and reference identities are reconciled on disposable copies before cutover. After nullable draft dates, old binaries are prohibited. Deploy matching backend/frontend under existing maintenance/release gates. Recovery is roll-forward. Retain a current recovery point before real cleanup.
