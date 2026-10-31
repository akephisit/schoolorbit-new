# Academic context visibility and defaults

Approved scope: the user's 2026-10-30 instruction removes academic year and term boundary dates from persistence and HTTP contracts. Visibility, school defaults and operational lifecycle are separate decisions. The Senior accepted the contracts and consumer direction below in task state; implementation does not require another user approval.

## Owners and contracts

Academic Core owns year/term identities, `isVisible`, context options and the tenant-local `academic_context_settings` singleton. Academic HTTP owns its endpoints and OpenAPI declarations. Existing year and term management permissions jointly authorize `PUT /api/academic/context/settings`; lifecycle authorization and transitions remain independent. No new crate or permission registry owner is needed.

Year and term create/update DTOs and responses remove `startDate`, `endDate`, `plannedEndDate` and `closedOn` as applicable. New identities start hidden. A one-time migration keeps legacy planning years and planning/cancelled terms hidden, preserving the former public endpoint's unpublished intent; other legacy records become visible. This deliberately hides planning contexts from normal staff discovery while management reads retain them. Operational defaults remain visible. Normal school, public, student and parent context options include only visible years and visible terms of visible parents, retaining each endpoint's existing enrollment/relationship scope. Setup and explicit management reads retain hidden records.

`AcademicContextOptions` exposes nullable `defaultAcademicYearId` and `defaultAcademicTermId` instead of lifecycle-derived active IDs. Defaults are returned only when included in the caller's options. A null default does not authorize date/status inference. URL and user selection are local navigation state and never update these settings.

The settings command is `{rowVersion, defaultAcademicYearId, defaultAcademicTermId, years: [{id, isVisible}], terms: [{id, isVisible}]}`. It locks and checks the settings revision, applies visibility changes and validates the resulting defaults atomically. A default year must be visible; a default term requires its visible parent to equal the default year and must itself be visible. A year-only default and no default are valid. Hiding the current default requires a replacement or explicit clearing in the same command. The response contains authoritative `settings`, changed `years` and `terms` with their new row versions, and `contextOptions`.

## Real dates and explicit relationships

Academic identities do not define civil intervals. Existing event dates, publication effective dates, placement/membership intervals, exam dates, official result snapshot facts and transition audit timestamps retain their meaning and identities.

- Calendar events and requests keep their actual date ranges. Global targets may remain yearless. Grade/homeroom targets supply their enrollment year explicitly; room/year mismatches are rejected. No event-date inference remains in writes, visibility, notifications or options.
- Attendance owns dated counting decisions in `attendance_days`. Legacy weekday/default/override meaning is materialized into explicit dates before boundary removal. New calendars are explicitly saved; reports, runtime digests and finalization use those dates instead of generating a term interval. Capture remains independently available on uncounted dates.
- Behavior owns explicit reset schedules and their dated intent. Monthly/manual behavior remains available; academic-boundary reset intent is reconciled into dated Behavior scheduling, without retaining a second academic interval owner.
- Admission and promotion use explicit placement effective dates where the command creates future placements. Accepted operational events may use their actual Bangkok operation date when that is the event's meaning. Existing placements are never retimed.
- Delivery, roster and timetable owners retain actual `startsOn`, `endsOn` and `effectiveFrom` facts. Commands require meaningful dates when they create those facts; no omitted input falls back to a deleted academic boundary. Open-ended availability remains open-ended.
- Lifecycle ordering uses academic year numbers and term sequences. Activation/closure retain receipts and actual operation timestamps; no closure boundary date is accepted. Dated readiness evaluates actual published facts and operation dates. Operational write guards remain enforced independently of visibility/defaults.
- Assessment and Results use actual requested operation/as-of dates and frozen official snapshot facts, without clamping to year or term dates.

Accepted lifecycle audits/readiness/checksums remain immutable; only active replay projections become date-free. Promotion preserves per-item execution dates and nullable no-placement decisions, with a run default for new/pending work. New curriculum/preparation requests bind the accepted explicit offering date only when they create offerings. Legacy requests had no such input and migrate with a null constraint, as do new retained-only results; mutable output dates never reconstruct accepted intent. Actual offering dates and preparation's separately created Delivery reference dates remain independent facts; a null constraint does not imply a no-op run. Verify original normalized preparation identities, output IDs/counts, exact creation transactions and version/audit links before cleanup.

## Migration and recovery

Add forward migrations after 145; never alter applied migrations. Inspect the complete 001–146 timeline and current dependent functions/views before DDL. Retain existing context foreign keys and composite context constraints. Map a missing scoped relationship only when its original semantic date identifies exactly one existing year/term; refuse zero or multiple matches atomically. Calendar's deliberately global targets are not missing relationships.

Before destructive cleanup, snapshot stable IDs, retained facts, relationship identities and aggregate digests. Materialize domain-specific dated intent using old dates only as migration input. Reconcile row counts, original facts, every pre-existing nonnull link and all transformed intent. Drop/rebuild only explicitly identified date-dependent functions/views/constraints; an unexpected dependency refuses cleanup. Remove all retired columns and runtime fields only after reconciliation succeeds in the same migration transaction. Replaying an applied migration is a no-op through the canonical runner.

Deploy the matching backend, generated contracts and frontend under the existing maintenance/all-tenant migration gates. Older binaries require the removed columns and are prohibited after migration. Recovery rolls forward with a compatible release; it never recreates boundary dates or guesses context. Production execution is outside the current agent assignment.

## Acceptance

Verify hidden/new contexts, visible-parent filtering, independent lifecycle states/defaults, scoped options, stale and concurrent settings writes, denied permissions, atomic hiding/default replacement, stable IDs/links, materialized calendars, unique historical mappings, every known refusal state and safe replay. Guards reject removed boundary columns/DTO fields and runtime date inference. Use disposable PostgreSQL fixtures and one coordinated Cargo target/build owner. Regenerate OpenAPI and TypeScript from the owning Rust contracts and run affected consumer checks. Independent review and QA assess the complete frozen candidate.
