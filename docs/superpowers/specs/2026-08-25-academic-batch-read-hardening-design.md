# Academic Batch Read Hardening

**Date:** 2026-08-25

**Status:** Approved for written review

**Scope:** `backend-school`, `frontend-school`, generated School API contracts, academic and
supervision read paths, request cancellation, focused performance guards, and deployment smoke
coverage

## Context

The Academic Core hard cutover replaced legacy academic relationships with explicit academic-year
and academic-term context. The replacement data model is correct, but several Release 1 workspaces
reconstruct collection relationships by calling a resource-detail endpoint once per parent row.
The timetable workspace is the visible production failure: after loading all term offerings, it
calls `/api/academic/offerings/{id}/groups` sequentially for every offering. A term with hundreds of
offerings therefore produces hundreds of browser requests and leaves the page skeleton visible for
tens of seconds.

The same pattern exists in student-year placements, homeroom advisors, curriculum program
requirements, study-program option discovery, and the all-years Academic Core setup page. The
timetable request chain is not cancelled when the component unmounts, so navigating away does not
stop the remaining requests. Some frontend collection calls already use one HTTP request but the
backend then hydrates every response row with additional SQL queries. Learning groups and
supervision templates/observations have this backend N+1 behavior.

Other bounded fan-out paths were confirmed in timetable occupancy and move validation, curriculum
offering preview/apply signaling, and question-bank Word export. These paths must be repaired before
Gradebook and Results adds further group, student-year, offering, and assessment volume.

This release hardens every confirmed read/hydration N+1 path from the audit. It does not classify
intentional per-file uploads, external file operations, notification delivery, or independently
audited per-item mutations as collection-read defects.

## Goals

- Make the number of HTTP requests used to open an affected workspace independent of the number of
  offerings, groups, homerooms, students, programs, requirements, templates, or observations.
- Make list-service SQL query counts independent of response row count by bulk-loading related rows.
- Cancel superseded or unmounted academic workspace reads so stale chains do not continue after
  navigation or context changes.
- Preserve explicit `academicYearId` and `academicTermId` authority and the current union of
  assigned, organization-unit, organization-tree, and school access.
- Keep Rust DTOs and OpenAPI as the wire-contract source of truth and remove manual supervision wire
  DTO ownership from the frontend.
- Preserve current response semantics, ordering, realtime signals, stale-response protection, and
  mutation behavior.
- Add behavior and architecture coverage that fails if a collection page or list hydrator returns
  to one request/query per row.

## Non-Goals

- No database schema, migration, seed, or permission-contract change.
- No change to academic lifecycle states, Gradebook, result calculation, term closure, or promotion.
- No generic GraphQL, data-loader framework, repository-wide ORM, or universal workspace endpoint.
- No batching of file uploads/deletes, malware scanning, outbound notifications, calendar reminder
  delivery, or other operations whose items have separate external side effects.
- No removal of resource-detail endpoints that remain valid for focused detail and mutation flows.
- No speculative optimization of endpoints that have no confirmed fan-out or row-dependent query
  count.

## Confirmed Defects and Required Outcomes

| Flow | Current fan-out | Required outcome |
|---|---|---|
| Timetable workspace | one group request per offering; group hydration adds three SQL queries per group | one term-scoped group request and constant-count bulk hydration |
| Student-year workspace | one placement request per student-year | one year-scoped placement request |
| Homeroom workspace | one advisor request per homeroom | one year-scoped advisor-assignment request |
| Study-program options | years, curricula, versions, and programs traversed through nested requests | one year-scoped option request |
| Curriculum editor | one requirements request per study program | one version-scoped programs-with-requirements request |
| Academic Core setup | terms and bell schedules requested once per year | one setup read model request |
| Supervision templates | list query followed by four queries per template | constant-count template/section/item/step hydration |
| Supervision observations | list query followed by evaluators, actions, and rating queries per observation | constant-count observation hydration |
| Curriculum offering preview | existing offering lookup once per requirement | one bulk existing-offering lookup |
| Curriculum offering apply signaling | full offering reload once per signaled offering | one bulk signal-descriptor query |
| Timetable occupancy/conflicts | related homeroom/instructor reads once per entry and repeated slot scans | bulk relationship loads and one bounded candidate scan |
| Timetable batch responses | one full entry reload per created/deactivated/template-applied entry | one bulk entry reload/hydration |
| Question-bank Word export | one detail request per selected question | one authorized export-data request preserving requested order |

## Approaches Considered

### Run existing detail requests concurrently

`Promise.all` or a bounded client queue would reduce elapsed time but would retain request volume,
backend authorization work, connection pressure, and per-row SQL hydration. It would also continue
work after navigation unless every request were separately cancelled. This treats latency rather
than ownership and was rejected.

### Return one universal academic workspace document

A single endpoint containing every academic collection would minimize HTTP calls but couple
unrelated pages, overfetch sensitive data, make permission behavior ambiguous, and become a second
application API inside one response. It conflicts with route-specific loading and was rejected.

### Add context-scoped collection read models and bulk hydrators — selected

Each affected route receives the smallest collection contract needed to replace its per-parent
detail calls. Services fetch parent and related rows in bounded SQL queries, then group them by
stable IDs. Existing detail endpoints remain for focused edits. Frontend loaders issue a fixed set of
typed requests and pass a shared abort signal. This keeps ownership explicit and scales with row
count without creating a universal payload.

## API Design

All query parameters use generated camelCase names. Collection reads use `GET` unless the selected
ID set can exceed a safe URL length. Every JSON response uses `ApiResponse<T>`.

### Term learning groups

```text
GET /api/academic/learning-groups?academicTermId={termId}
```

The response is `Vec<LearningGroup>`. The service joins each group to its offering and applies the
same `AcademicResourceListFilter` used by `GET /api/academic/offerings`. Assigned access preserves
current offering-level semantics: assignment to any group makes the accessible offering's groups
visible, matching the existing nested endpoint behavior. Teacher assignments, homeroom coverage,
and preferred rooms are fetched once each for all returned group IDs.

`GET /api/academic/offerings/{id}/groups` remains available for the delivery editor and reuses the
same bulk hydrator for its filtered subset.

### Year placement and advisor collections

```text
GET /api/academic/placements?academicYearId={yearId}
GET /api/academic/homeroom-advisors?academicYearId={yearId}
```

Placements return the existing `HomeroomPlacement` shape because it already contains
`studentAcademicYearId`. Advisor collection rows use a new `HomeroomAdvisorAssignment` DTO containing
`id`, `homeroomId`, `userId`, and `role`. Both endpoints validate the requested academic year and
reuse the corresponding student-year or homeroom read permission. They return only relationships
owned by the selected year.

The existing nested endpoints remain mutation/detail boundaries. Their implementation may call the
same collection helpers with a single parent ID.

### Study-program options for an academic year

```text
GET /api/academic/study-program-options?academicYearId={yearId}
```

The response contains only lookup fields required by homeroom, student-year, and admission forms:
`id`, `code`, `name`, `curriculumId`, and `curriculumName`. One SQL query selects published programs
whose published curriculum version covers the selected academic year. Authorization is the union of
the same curriculum read scopes used by the catalog; the endpoint does not expose unpublished
programs or unrelated curriculum details.

This replaces the frontend helper that downloads all years and traverses every curriculum/version.

### Curriculum version program workspace

```text
GET /api/academic/curriculum-versions/{versionId}/program-workspace
```

The response is:

```text
CurriculumProgramWorkspace {
  programs: Vec<StudyProgram>,
  requirements: Vec<StudyProgramRequirement>
}

StudyProgramRequirement {
  studyProgramId,
  requirement: ProgramRequirement fields
}
```

Programs and course/activity requirements are loaded in constant-count queries. Including
`studyProgramId` lets the frontend group requirements without one request per program. Existing
program and requirement mutation endpoints remain authoritative and their returned resources patch
the local workspace state.

### Academic setup workspace

```text
GET /api/academic/setup/workspace
```

This route is intentionally context-free because it is the system page that administers all years.
It returns full `AcademicYear`, `AcademicTerm`, and `BellSchedule` rows needed by the existing editor,
not the reduced Topbar context options. The service executes one bounded query per collection and
sorts with the same rules as the current page. Access requires the existing read capability for each
included collection; it grants no new authority.

Period details stay schedule-scoped and load only when the user opens or selects a schedule. The
setup response does not embed all bell-schedule periods.

### Question-bank export data

```text
POST /api/academic/question-bank/questions/export-data
QuestionBankExportDataRequest { questionIds: Vec<Uuid> }
```

POST is used because an ordered export selection can exceed safe query-string length. This is a
read-only application command: it creates no record and emits no event. The request accepts 1–200
unique IDs, applies the existing question-bank resource policy to the complete set, and returns full
`QuestionDetail` rows in exactly the requested order. Summaries, choices, and file metadata are
loaded with set-based queries.

### Supervision contract registration

Every currently routed supervision endpoint and stable DTO used by `frontend-school` is registered
in the School API OpenAPI document. Existing Rust request/response types gain `ToSchema` and query
types gain `IntoParams` where required. Generated TypeScript operation and schema types replace the
manual wire DTO declarations in `frontend-school/src/lib/api/supervision.ts`.

This is contract ownership alignment, not a route rename or compatibility layer. Existing paths and
JSON shapes remain unless generation exposes an actual mismatch, in which case Rust serialization
and the frontend consumer are corrected together with a focused contract test.

## Backend Bulk Hydration

### Reusable bulk hydrators

List and detail services share one bulk implementation:

```text
rows -> collect parent IDs -> fetch each child relation with ANY($1)
     -> group child rows in HashMap/BTreeMap -> assemble ordered DTOs
```

An empty parent set returns immediately without issuing `ANY('{}')` queries. Detail reads call the
bulk hydrator with one row and require exactly one result. Missing required subtype data remains an
internal consistency error rather than producing a partial DTO.

The required conversions are:

- learning groups: teachers, homerooms, preferred rooms;
- supervision templates: sections, items, workflow steps;
- supervision observations: evaluators, actions, average ratings;
- timetable entries/occupancy: instructors and effective homerooms;
- question-bank export: choices and authorized file metadata.

### Curriculum offering preview and signaling

Preview loads all existing course and activity offerings for the requested term and candidate
catalog-version IDs before iterating requirements. A keyed map resolves create/retain/conflict
actions without SQL inside the loop.

Apply keeps its transactional, idempotent write behavior. After commit, one service query returns
`learningOfferingId`, `academicTermId`, and `rowVersion` for every result ID. The handler emits the
same realtime change signal per offering from those descriptors without fully hydrating each
offering.

### Timetable validation and batch responses

Timetable batch create, deactivate, and template application load all result rows in one query and
pass them through `hydrate_rows`; they do not call `get_entry` in a loop.

Occupancy loads all effective group homerooms and instructors for the selected term in bulk.
Move-validation loads the selected term's relevant active entries once, bulk-loads relationships,
indexes entries by `(dayOfWeek, bellSchedulePeriodId)`, and evaluates each candidate cell against
the in-memory index. Slot-level create/update conflict checks continue to lock the relevant rows but
bulk-load relationships for the locked set rather than querying per conflicting entry. Database
locks and conflict semantics remain unchanged.

## Frontend Loading and Cancellation

Affected wrappers accept `ApiRequestOptions` or `AbortSignal` and pass the signal to the central API
client. A workspace load owns one `AbortController`:

1. Abort the previous controller before a context reload.
2. Create one controller and pass its signal to every request belonging to that revision.
3. Ignore `AbortError` without showing a toast or error state.
4. Commit response state only when the controller is current and not aborted.
5. Abort on component cleanup.

Revision guards remain as defense against non-fetch asynchronous work; they are not treated as
cancellation.

The affected pages are Academic Core setup, curricula, homerooms, student-years, timetable, and the
admission round flow that consumes study-program options. Supervision workspace loading adopts the
same cancellation pattern while moving to generated types. Question-bank export is user-triggered;
closing the export workflow or leaving the page aborts its single export-data request.

Normal workspace request counts are fixed by route requirements rather than row count. A page may
still issue several independent collection requests when their permissions or refresh lifecycles
differ; no request may be generated by iterating response rows.

## Authorization and Data Isolation

- New academic collection endpoints reuse existing capability families and resource policies. No
  permission code or role grant changes.
- Term learning groups use the learning-offering list filter and preserve union behavior.
- Year-scoped placement/advisor endpoints validate that every returned row belongs to the requested
  year.
- Study-program options expose only published, effective curriculum data authorized for the actor.
- Question-bank export applies access to every requested ID and fails the whole request if any item
  is missing or unauthorized; it never reveals which unauthorized ID exists.
- Supervision response batching does not broaden the rows selected by the existing list-access
  query. It only changes how child rows for already-authorized observations are hydrated.
- No national IDs, blind indexes, credentials, or new PII fields enter responses or logs.

## Error and Limit Semantics

- Missing or malformed context remains HTTP 400 through typed query deserialization.
- A context/resource ownership mismatch remains a validation or not-found error consistent with the
  existing handler.
- Batch endpoints define explicit maximum result/request sizes based on their existing page limits;
  they fail with an actionable error rather than silently truncating a required workspace.
- Aborted browser requests do not clear already-rendered data and do not display failure messages.
- A non-abort failure uses the current Thai page error state and leaves stale responses unable to
  overwrite a newer revision.
- Bulk hydration fails closed when a required child/snapshot is missing. Optional child collections
  become empty arrays.

## Performance Invariants

- Timetable page group HTTP calls: exactly one per selected term, independent of offering count.
- Placement and advisor HTTP calls: exactly one per selected year, independent of parent count.
- Study-program option HTTP calls: exactly one per selected year.
- Curriculum program workspace HTTP calls: exactly one per selected curriculum version.
- No list service may call its detail hydrator once per row.
- Learning-group, supervision-template, and supervision-observation list query counts are constant
  with respect to returned row count.
- Timetable candidate validation does not execute one database query per candidate cell or existing
  entry.
- Question-bank export uses one application request for the selected set.

These are behavioral boundaries. Integration tests exercise services with more than one parent and
related child rows; frontend tests call real wrapper/workspace loader code with a controlled
transport and assert emitted requests and abort behavior. Static guards supplement these tests by
rejecting known per-row hydrator shapes, but do not replace behavior tests.

## Realtime and Cache Behavior

This release does not change realtime event names or payloads. Applying curriculum offerings still
emits one change signal per affected offering. The signal descriptors come from one bulk read after
commit. Receiving clients continue to re-read authoritative HTTP state.

No new long-lived cross-user frontend cache is introduced. Page state remains scoped to its route
and academic context. Academic Topbar context options retain their existing session cache and are
not duplicated by the setup workspace.

## Delivery Sequence

Release 1.1 is implemented in independently reviewed checkpoints but deployed as one coherent
backend/frontend contract release:

1. Add failing request-count, cancellation, service hydration, authorization, and OpenAPI tests.
2. Add academic batch collection DTOs/services/handlers/routes.
3. Bulk-hydrate learning groups and convert affected academic pages to fixed-count cancellable
   loaders.
4. Bulk-load curriculum offering preview/signals and timetable validation/batch responses.
5. Bulk-hydrate supervision, register its OpenAPI contract, regenerate DTOs, and convert the
   frontend wrapper.
6. Add question-bank export-data batching and convert Word export.
7. Regenerate the School API contract and run the full backend/frontend verification matrix.
8. Push/deploy only after explicit user approval, then verify readiness and authenticated
   representative workflows.

No migration or maintenance window is required. Backend and frontend must deploy through the normal
coordinated `main` workflow because the frontend will consume newly documented endpoints.

## Verification Strategy

### Focused backend tests

- learning-group term list preserves school, organization, tree, and assigned unions;
- term group list returns teachers, homerooms, and preferred rooms for multiple groups;
- year placement/advisor lists reject cross-year leakage;
- study-program options include only published versions effective for the requested year;
- curriculum program workspace groups requirements by the correct program;
- supervision list hydration returns complete multi-parent child collections;
- curriculum preview produces identical actions using a bulk existing-offering map;
- timetable occupancy/move validation preserves conflict output for multi-entry fixtures;
- question-bank export preserves requested order and fails closed for unauthorized IDs;
- OpenAPI documents every new query and all supervision routes with camelCase names.

Database-backed academic and authorization tests run through
`./scripts/test_backend_school.sh ... -- --test-threads=1` so PostgreSQL work remains serial.

### Focused frontend tests

- academic wrappers send generated camelCase query objects;
- timetable workspace loading emits one term group request, not one per offering;
- student-year and homeroom workspace loaders emit one relationship collection request;
- study-program options use the year-scoped endpoint;
- a newer load aborts the previous signal and unmount cleanup aborts the active load;
- abort does not show an error while a genuine request failure does;
- supervision wrapper consumes generated operations/schemas;
- question-bank export sends one ordered batch request.

Each modified Svelte component passes the Svelte 5 analyzer.

### Required matrix

From `backend-school`:

```text
cargo fmt --all -- --check
cargo test --test static_architecture
cargo test api_contract::tests -- --nocapture
cargo check
```

From `frontend-school`:

```text
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

From the repository root, finish with `git diff --check`, final diff review, and
`git status --short`. Commands run serially.

## Acceptance Criteria

- Opening timetable with hundreds of offerings produces one learning-group request and no request
  chain continues after leaving the page.
- Opening student-years or homerooms produces no placement/advisor request per displayed row.
- Study-program options require one year-scoped request in every consuming workflow.
- Curriculum and Academic Core setup request counts do not grow with program or year count.
- Learning-group and supervision list SQL query counts do not grow with response row count.
- Curriculum preview/apply and timetable validation preserve current business results without
  per-item read queries.
- Question-bank Word export retrieves its selected question set in one authorized request.
- New and existing affected APIs are documented by Rust/OpenAPI and consumed through generated
  TypeScript types, including supervision.
- All authorization, PII minimization, realtime, and explicit academic-context invariants remain
  intact.
- The full applicable verification matrix passes before any push or deployment approval request.
