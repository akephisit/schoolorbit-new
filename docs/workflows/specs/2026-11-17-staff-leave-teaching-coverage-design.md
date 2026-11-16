# Staff leave and exit teaching coverage

The approved change adds date-specific teaching coverage to both leave and planned exit requests. Creation and editing use dedicated full-page routes and one shared three-step workflow: request details, affected lessons grouped by actual date, then review and send. Existing request identities, policy/evidence snapshots, audit history, signatures, optimistic revision checks, retry identities and cancellation behavior remain authoritative.

## Ownership and contracts

`school-staff-leave` owns request coverage persistence, completeness, recipient consent, conflict detection and emergency submission. Timetable/Academic Delivery remain the authoritative providers of allocations, actual student groups, teaching offerings, grade levels and availability. Attendance and daily timetable consume accepted date-specific duties through the backend provider; weekly schedules remain unchanged. The central school API client owns transport and generated Rust/OpenAPI contracts own wire types. Frontend routes own initial reads, domain components own interaction state, and existing auth/permission stores own session identity.

Senior owns the shared Git index and integration. Backend owns Rust, new migration 160, permission/API source and generated outputs. Frontend owns non-generated school UI, focused tests and these workflow artifacts. Contract-dependent frontend operations begin only after the backend publishes the actual generated contract. No parallel handwritten wire interfaces or generated-file edits are allowed.

## Request and academic context

Request details retain explicit academic year and term selections, visibly initialized from the school's valid defaults; dates never infer academic context. Half-day coverage uses a nullable school-managed boundary and never silently assumes noon. Missing boundary or schedule is actionable configuration state, distinct from a ready timetable with zero affected lessons. Existing requests with unverifiable historical schedules are visibly identified rather than silently declared covered.

A request draft may be saved while teaching assignments or consents are incomplete. Details and evidence retain current validation and policy behavior. Saving changed request dates or context requires a fresh authoritative coverage plan and digest; stale coverage cannot overwrite newer source data. Navigation between workflow steps preserves the current draft, and deep links/reloads resume the saved server request. Return links retain the originating leave/exit list's filters and paging and reject external destinations. Identity changes and exact permission loss dispose drafts and pending reads.

## Lesson planning

Every affected allocation appears under its actual date, with period/time, original subject or offering, student group/homerooms and room. Each day and the complete plan show separate selected and confirmed counts. A selected teacher covers either a substitution or a reciprocal exchange. Assignments record the offering actually taught or a named teaching/enrichment activity; they may use the replacement teacher's own subject. The UI must not imply that the original subject was taught when another offering/activity was selected.

Teacher candidates are searchable by name and filterable by subject group and grades they actually teach. Cross-subject-group candidates are allowed by default; same-subject-group and same-grade choices are optional quick filters. Batch assignment offers only teachers free for every selected slot. Candidate selection appears in a desktop side panel and an inline bounded mobile surface, with searchable choices, native page scrolling and keyboard access; large nested dialogs are excluded.

Reciprocal exchange requires the actual return date and lesson for the same student group. Backend validation checks both teachers, both slots, collisions and the real return lesson identity. Date and teacher changes supersede candidate/return reads. Unknown or stale reads do not silently become empty candidate lists.

## Consent, approval and emergency

The assigned-task route exposes only the recipient's own teaching rows, dates, teaching activity and exchange facts needed for consent. It omits private leave reasons, evidence and policy/personnel details. Recipients can select multiple own rows and confirm or decline them in one explicit action. Changed assignments reset affected consent. Administrative approval waits for complete assignments and required confirmations.

A requester can explicitly submit incomplete coverage with a nonempty emergency reason. The review surface makes the incomplete state and emergency reason clear; authorized managers can complete the plan. Ordinary submit never silently bypasses coverage requirements. Backend resource authorization remains the authority for requester, recipient and manager actions; route metadata and exact UI permissions guard visibility and optional reads.

## Acceptance and verification

Verify creation and editing for leave and exit; all three steps and back navigation; saved draft deep links and list return context; date/context changes and source/revision conflicts; unchanged retry identities; evidence preservation; session/permission disposal; multiple dates and daily/overall counts; cross-group search, real grade filters and batch all-slot availability; substitution with actual teaching activity; reciprocal same-group return validation; missing half-day boundary, missing timetable, valid zero lessons and read failures; emergency submission and ordinary approval gates; recipient-only redaction and multi-row consent. Daily timetable/Attendance must reflect accepted duties without changing weekly schedules.

Use disposable synthetic fixtures. Run focused helper and browser cases, school Svelte/TypeScript checks, scoped formatter/lint, route-data inventory/policy and documentation policy. Inspect mobile and desktop in light/dark themes, keyboard focus, bounded candidate scrolling, loading/error/retry and pending states. Backend separately verifies migration preservation, tenant/resource authorization, transactions, concurrency, availability and generated contracts. Independent Review and QA assess the complete frozen candidate before integration.
