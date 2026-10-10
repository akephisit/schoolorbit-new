# Independent delivery and timetable versions

Approved by the user's implementation request on 2026-10-04. This replaces the
shared operational/timetable change-set design discussed earlier in this chat.

## Ownership and behavior

Delivery owns an immutable, dated published version of the term's offerings,
weekly targets, groups, homeroom coverage and instructor assignments. Existing
operational change items are the delivery revision's edit journal, not the owner
of a timetable. Timetable owns drafts, blocks, placement validation, publication
and deletion. One published delivery version can support any number of timetable
versions. Keep these capabilities in their existing crates; timetable already
depends on delivery, so no additional crate or reverse dependency is warranted.

The timetable page is read-first. Editing copies the displayed published
timetable or resumes an explicitly selected draft, binds it to the latest
published delivery version, preserves placements by stable IDs, and reports
incompatible offerings/groups/instructors without deleting lessons. New demands
remain unplaced. Autosave remains immediate. Finishing editing is not rollback.
New delivery publications during editing offer an explicit source update.

Delivery publication checks its own graph and does not require scheduled periods
or publish a timetable. Student roster publication remains a separate existing
lifecycle: publishing an opening marks its offering/group registries published
but does not approve or publish student memberships. Published timetables retain their pinned delivery graph;
new delivery publications signal an outdated source rather than rewriting them.
Timetable publication chooses its effective date then checks date/source validity,
unresolved source issues, target counts and collisions, under row-version and
preview-hash guards. A semantic content comparison includes the delivery source
but excludes generated block IDs, provenance, row revisions and audit fields.

Draft timetable deletion is permanent and school-manage-only. Delete its owned
children atomically; reject published/cancelled versions, stale revisions and
external history references. Preserve the delivery version and source timetable.
Write a bounded deletion audit in the same transaction and broadcast a change.

## Canonical storage and interfaces

Introduce `academic_delivery_versions` with term/year context, source version,
status, effective date, row revision, publication metadata and a typed snapshot.
The snapshot is one canonical published graph, expressed by named Rust models
and stored as validated JSONB. It contains offering/catalog snapshot fields,
weekly targets, group/coverage data and instructor IDs/roles, not student PII.
Stable offering and group identities remain the existing academic resource IDs.
Timetable reads this immutable graph through delivery's public service, rather
than deriving its source from mutable live assignment rows or timetable targets.

Operational change-set base/target references become delivery-version references.
Published snapshots cannot be changed. Timetable versions acquire a required
same-term delivery reference and nullable draft effective date. Published
timetables require both a published delivery source and a real effective date.
Draft blocks may retain incompatible stable IDs solely for explicit reconciliation;
publication rejects unresolved membership/instructor issues.

Group placements retain their actual homeroom coverage as a timetable-owned
placement field. Source upgrades compare it with the delivery group's intended
coverage, rather than silently moving historical or unresolved lessons into a
different homeroom. This is placement evidence, not a second delivery graph.

Expose typed delivery-version list/detail operations and independent timetable
clone, source-update, readiness, publish and delete operations. Existing delivery
revision operations remain delivery-only and use delivery IDs. Regenerate OpenAPI
and TypeScript, then update all clients together; do not introduce endpoint aliases
or a second runtime read path. Preserve existing permission codes and resource
scopes; whole-version deletion needs `academic_timetable.manage.school` and
publication needs `academic_timetable.publish.school`.

## Migration and cutover

Use new forward migrations: expand/backfill/reconcile, then cleanup after an
executable preservation gate. Reconstruct dated delivery graphs only from stable
IDs, existing dated assignments, targets and recorded change items. Map historical
timetables to published snapshots; identical graphs may share a delivery version.
Operational drafts with items become delivery drafts. Their existing timetable
placements survive independently against the published base delivery graph until
the new delivery graph is published and explicitly adopted. Timetable-only drafts
do not create delivery drafts. Preserve original revision reason/actor/history in
canonical audit/provenance, including receipt relationships.

The retired direct-inclusion path also left additive target IDs without journal
items. Convert these only when the draft and its journal agree on the exact
published source and all existing source targets are unchanged. Preserve the
explicit added IDs/weekly targets as deterministic opening inclusion commands;
retain the original journal/date and table placements. Removed or altered targets
without commands, missing journals and conflicting sources still stop migration.

Group creation/import timestamps are not teaching effective dates. A later-created group may
enter a historical graph only through its explicit active placement IDs and exactly one dated
assignment for each placed instructor, matching teacher, group and role at the effective date
across all placements. Missing or overlapping episodes and mismatched roles stop mapping.
Unplaced later groups are excluded; no current teacher is substituted for an old instructor.

Gate mapping by counts, identities, relationships and hashes. Missing/ambiguous
sources and external immutable references stop cleanup; never infer matches by
names. Test legacy-shaped fixtures before switching any consumer. Remove the
mandatory joint publication path and obsolete target ownership after verified
conversion. Existing teacher handoff receipts are history and must not be dropped
to permit deleting a draft.

The coordinated full-release maintenance workflow owns schema migration and
frontend/backend promotion. Old binaries are forbidden after consumer cutover;
recovery is a tested roll-forward. No parallel runtime compatibility layer.

## Verification

Run preservation and failure fixtures in disposable rootless PostgreSQL, focused
delivery/timetable/lifecycle/supervision tests, API/permission checks, static
architecture and workspace Rust checks. Browser fixtures cover read-first editing,
1:N sources, explicit source upgrades, invalid lesson retention, autosave failures,
date selection, publication races, permanent deletion and denied permissions.
Inspect desktop/mobile and light/dark with sanitized fixtures. Run the complete
change matrix in `.rules`, coordinated deployment and acceptance before cleanup.
