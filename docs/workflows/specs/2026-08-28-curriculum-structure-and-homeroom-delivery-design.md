# Curriculum Structure and Homeroom Delivery Workspace

**Date:** 2026-08-28

**Status:** Approved

**Scope:** `backend-school` Academic Core and Learning Delivery services, forward-only tenant data migration and preflight, typed OpenAPI/generated frontend contracts, and the `frontend-school` curriculum and delivery workspaces

## Context

SchoolOrbit already separates published subject and activity catalog versions, curriculum versions, study programs, program requirements, homerooms, term offerings, learning groups, teacher assignments, rosters, physical rooms, and timetable assignments. That separation is necessary: a curriculum describes what a study program requires, an offering makes one published catalog version available in an operational term, and a learning group describes the students who actually learn together.

The current user experience does not expose that model in the way Thai academic staff normally inspect their work. The curriculum workspace shows nested requirement cards and supports mostly one-at-a-time editing. It does not provide the familiar term-by-term curriculum structure with course code, weekly load, credit, total term hours, category subtotals, and plan comparison. The current requirement contract also duplicates ambiguous `credit` and `hours` fields even though the selected immutable catalog version is the authoritative source.

The Learning Delivery landing page is offering-centric. It is useful for managing one subject or activity but makes the more common completeness question difficult: “What does ม.1/1 study this term, and what is still missing?” Staff must open or mentally combine many subject rows. The model already allows a learning group to link one or more homerooms, but the landing page does not make normal, combined, split, or mixed-homeroom delivery easy to understand.

This design provides two connected workspaces:

1. a curriculum structure workspace that reads like a modern, editable Thai curriculum document; and
2. a homeroom-first delivery workspace that prepares and audits term delivery from that published structure.

It preserves the normalized domain chain. It does not collapse offerings, groups, or homerooms into one record and does not restore any legacy academic schema or compatibility path.

## Goals

- Show the complete curriculum structure for one study program by grade and term in the document form familiar to Thai schools.
- Compare all study programs at a grade level without opening each plan separately.
- Add, move, copy, classify as required/elective/optional, reorder, and remove many published courses or activities efficiently.
- Make the referenced published catalog version the only owner of official credits and workload metrics.
- Support any configured curriculum term structure, including regular, summer, remedial, and custom terms, without hardcoded `1`, `2`, or `summer` UI logic.
- Make the default delivery view answer what each homeroom studies in the selected academic term.
- Retain an offering-centric view for advanced subject/activity and group management.
- Represent normal per-homeroom groups, combined homerooms, split groups, electives, and mixed-homeroom activities without duplicated domain records.
- Extend the existing curriculum offering preview/apply workflow to prepare safe draft offerings and default groups without creating duplicates.
- Use typed, set-based workspace APIs instead of frontend request fan-out.
- Cut the frontend and backend over to one clean contract with a reviewed forward migration and no legacy compatibility layer.

## Non-Goals

- Do not automatically assign teachers, preferred physical rooms, timetable slots, or exam schedules.
- Do not publish learning-group rosters automatically.
- Do not infer elective student choices or place students into mixed elective groups automatically.
- Do not activate or close an academic term or year, lock results, promote students, or graduate students.
- Do not edit an applied migration or revive a legacy academic table, endpoint, or form.
- Do not build a generic spreadsheet engine or a universal metadata-driven form builder.
- Do not reproduce a scanned government document pixel for pixel.
- Do not add printing or spreadsheet/PDF export in the first curriculum workspace release. The read model must keep those future outputs possible.
- Do not make a physical classroom the owner of delivery. Physical rooms remain timetable resources, while a homeroom is the administrative student cohort.

## Approaches Considered

### Reorganize only the existing frontend responses

The frontend could call the current curriculum, offering, group, homeroom, teacher, roster, and timetable endpoints and regroup the results locally. This would create request fan-out, duplicate domain joining in Svelte, weak concurrency behavior, and inconsistent status rules. It is rejected.

### Replace offerings and groups with direct homeroom-course assignments

A direct homeroom-course table would make the simplest case look smaller, but it cannot correctly model one offering shared by several programs, combined homerooms, a homeroom split into multiple groups, mixed electives, or activities spanning grade levels. It would also couple curriculum intent to timetable delivery. It is rejected.

### Preserve the normalized domain and add purpose-built workspaces — selected

Catalog version, curriculum requirement, homeroom, offering, learning group, and roster remain separate authoritative concepts. Backend services provide curriculum-structure and homeroom-delivery read models plus typed bulk mutations. The current offering detail route remains the advanced editing surface. This approach improves staff comprehension without weakening the domain.

## Authoritative Domain Chain

The system keeps this ownership order:

```text
published subject/activity catalog version
    -> published curriculum version
        -> complete study-program structure
            -> homeroom assigned to that study program in an academic year
                -> offering in one operational academic term
                    -> one or more learning groups
                        -> homeroom sources and/or explicit roster members
```

Each concept answers a different question:

- **Catalog version:** What is this course or activity, and what are its official metrics?
- **Curriculum requirement:** At which grade and curriculum term slot does a study program require it, and is it required, elective, or optional?
- **Homeroom:** Which administrative cohort follows which study program in this academic year?
- **Offering:** Is this exact catalog version available in this operational term?
- **Learning group:** Which students learn it together, with which teachers and preferred rooms?
- **Timetable assignment:** When and in which physical room does that group learn?

An offering is not a room-course row. For one term and one exact catalog version, a single offering can carry multiple grade/study-program targets and own several learning groups. A normal subject can have one group per homeroom; a combined subject can have one group linked to several homerooms; a split subject can have several groups linked to the same homeroom.

## Curriculum Term Slots

A curriculum version is effective across academic years and therefore must not reference a concrete operational `academic_term_id`. It instead owns an ordered set of **curriculum term slots**. Each slot has a stable ID, order, term type, occurrence within that type, and school-facing label. Requirements reference the slot ID rather than a free-form `recommended_term_code`.

Examples include first regular term, second regular term, summer term, or a school-defined special term. The UI renders the slots returned by the API and never hardcodes two terms. A curriculum version with no summer slot has no summer requirements; adding an operational summer term does not invent curriculum requirements.

When preparing delivery, the backend resolves the selected operational academic term to exactly one curriculum term slot using the term type and its occurrence/order semantics. No name-string comparison is allowed. A missing or ambiguous mapping is an explicit prerequisite error with a route back to curriculum or academic-term setup.

Curriculum term slots become immutable when their curriculum version is published. Changing the term structure requires a new draft curriculum version.

## Catalog Metric Ownership

The selected immutable published catalog version is the source of official workload data. Curriculum requirements do not accept editable copies of `credit` or `hours`.

The typed read model distinguishes the real meanings needed by the curriculum document:

- weekly periods or weekly hours;
- academic credit, where the resource carries credit;
- total instructional hours for the curriculum term;
- the unit used for the weekly value; and
- course/activity kind and category metadata.

Subject catalog versions already carry credit, periods per week, and hours per semester. Any activity catalog version selected into a curriculum must expose an explicit total-hours value; credit remains absent for non-credit activities. New publication validation requires the official metrics appropriate to each resource kind, while a migrated published version with missing metrics is blocked from curriculum publication until repaired. The UI can present both resource kinds in one table while retaining their typed meaning in the API.

The visible curriculum section and the plan-specific requirement rule are separate meanings. `รายวิชาพื้นฐาน`, `รายวิชาเพิ่มเติม`, and `กิจกรรมพัฒนาผู้เรียน` come from the published catalog resource kind and official catalog classification. `required`, `elective`, and `optional` remain requirement rules owned by the study program. The editor must not let a requirement relabel an activity as a course or turn a basic course into an additional course while leaving its catalog version unchanged.

Existing ambiguous `program_requirements.credit` and `program_requirements.hours` values are not retained as runtime overrides. The migration preflight compares them with their referenced catalog versions, classifies deterministic matches, and reports mismatches. Missing activity total hours are never guessed from a calendar date range or an assumed 20-week term. They must be populated from a deterministic existing source or explicitly resolved before the destructive cleanup proceeds.

Because catalog versions are immutable after publication, a curriculum that references an older version remains historically stable even after a newer catalog version is published.

## Complete Study-Program Structures

Every study program owns a complete independent structure for every supported grade. A “general,” “science–mathematics,” or “intensive English” program is not stored as a base plan plus hidden overrides.

Independence makes published history, totals, comparison, and later versioning unambiguous. To avoid repetitive staff work, the draft editor provides explicit copy operations:

- copy a whole grade from another study program;
- copy one curriculum term slot;
- copy selected requirements; and
- replace or merge after a preview of conflicts.

A copied requirement still references the exact published catalog version chosen at copy time. The operation never silently upgrades it to a newer version.

## Curriculum Workspace

The curriculum routes remain context-independent. They define versions that can span multiple years, so they do not take the Topbar academic year/term as an implicit owner. The page uses explicit curriculum, version, grade, and study-program controls.

### Curriculum overview

`/staff/academic/curricula` remains a list-first registry of curricula and versions. Selecting a curriculum opens its structure workspace. The header provides curriculum, version, grade, view, and status controls without exposing UUIDs or storage codes.

### Compare-all-programs view

The default comparison surface for a selected grade places course/activity rows against study-program columns. It groups rows by school-facing curriculum category and indicates each program's assigned term slots and credits/workload. Shared rows remain visually quiet; differences use a restrained highlight. Category and whole-grade totals let staff find omissions and divergent plans quickly.

Selecting a cell opens the corresponding program/term detail rather than editing a dense matrix inline.

### Single-program document view

The single-program surface places curriculum term panels side by side on wide screens and stacks them on narrow screens. Each panel uses the familiar columns:

- code;
- Thai course or activity name;
- weekly periods/hours;
- credit where applicable; and
- total curriculum-term hours.

Rows are grouped into the school's curriculum categories, including basic courses, additional courses, and student-development activities. The system renders category subtotals and term totals. It also shows validation notices when official metrics are missing or a configured curriculum rule is not satisfied.

The visual direction is a modern “editable curriculum document”: crisp table rules, calm SchoolOrbit blue/neutral tokens, readable Thai typography, and tabular numeric alignment. It does not imitate scan artifacts or compress the workspace to paper width.

### Draft edit mode

Published curriculum versions are read-only. A draft version opens a two-pane editing workspace:

- the catalog pane provides searchable, typed published subject/activity versions; and
- the structure pane shows the selected program, grade, and term slots.

Staff can select multiple catalog entries, add them to a term slot, choose their plan-specific requirement rule, move or reorder selected requirements, copy from another program or slot, and remove selected requirements. The official curriculum section remains catalog-derived. All staged operations are undoable until save. The UI previews additions, removals, moves, duplicates, and catalog-version conflicts before one atomic save.

Official catalog metrics are visible but never editable in this workspace. Large option sets load only when edit permission and the relevant action require them. Draft saves use a workspace row version and return the updated typed workspace so the frontend patches local state without a broad reload.

## Homeroom-First Delivery Workspace

`/staff/academic/delivery` continues to use the Topbar-selected `academicYearId` and `academicTermId`. Its default view changes to **by homeroom**, with **by subject/activity** as a secondary view.

### By-homeroom view

Homerooms are grouped by grade and study program. Each summary shows the count of expected curriculum items and the count whose operational delivery is ready. Expanding a homeroom shows one row per expected curriculum requirement with concise, independently computed statuses for:

- offering missing, draft, or published;
- no group, normal group, combined group, or split groups;
- primary teacher missing or assigned;
- roster draft or published; and
- timetable unscheduled, partly scheduled, or scheduled when timetable data is available.

The workspace is an audit/read model, not a second owner of these states. Row actions navigate to or open the existing authoritative offering/group workflow.

Combined groups appear in every linked homeroom with a badge such as `เรียนรวม ม.1/1, ม.1/2` and the same group ID. Split groups appear as distinct group A/B rows under the same expected requirement. An unlinked offering or group must remain visible in a separate `ยังไม่ผูกห้องประจำชั้น` queue so the room-first view never hides operational records.

Valid empty data, missing Topbar context, a homeroom without a study program, no applicable published curriculum, a curriculum-term mapping failure, permission denial, and request failure remain distinct states with direct recovery actions.

### By-subject/activity view

The existing offering overview remains available for staff who need to answer which rooms or groups take one subject. It continues to summarize targets, groups, teachers, and roster publication. The offering detail route remains the advanced surface for group metadata, linked homerooms, teacher assignments, preferred physical rooms, and roster workflows.

The two views are projections over the same offering and group records. They never dual-write or maintain separate delivery copies.

## Prepare-Term Preview and Apply

The existing typed `preview-from-curriculum` and `apply-from-curriculum` service flow already provides source hashing, idempotency, and offering reuse. This design extends that workflow rather than introducing a competing preparation path.

### Preview inputs and derivation

For the selected academic term, the service loads in set-based queries:

- its academic year and resolved curriculum term slot;
- active homerooms and their study programs;
- the applicable published curriculum version;
- expected program requirements;
- existing offerings and targets;
- existing groups and linked homerooms; and
- the minimum teacher, roster, and timetable aggregates needed for statuses.

The preview produces a room-by-room matrix plus a normalized operation plan. Offerings are deduplicated by operational term, resource kind, and exact catalog version while accumulating applicable grade/program/requirement targets.

For an ordinary required course or activity, the default proposal is one draft group per homeroom under the shared offering. Elective or optional requirements create or retain the draft offering but do not assume student grouping. They enter a `ต้องจัดกลุ่ม` queue unless staff explicitly chooses a supported grouping in the preview.

Before apply, staff may:

- keep the default per-homeroom groups;
- combine selected homerooms into one group;
- split one homeroom into named groups;
- skip a proposal; or
- leave an elective/optional item for later grouping.

The preview never auto-selects students, teachers, preferred physical rooms, or timetable slots.

### Apply guarantees

Apply accepts the reviewed normalized choices, source hash, and idempotency key. The backend recomputes authoritative source data and rejects a stale preview. A transaction then:

- reuses matching offerings and existing compatible groups;
- creates only missing draft offerings, targets, and requested draft groups;
- never overwrites a non-default or manually customized group;
- records the source curriculum version and requirement relationships needed for traceability; and
- returns created, retained, skipped, and conflict outcomes grouped by homeroom.

Retrying the same idempotency key with the same request returns the original result. Reusing it with a different request is rejected. Any database error rolls back the entire operation.

## Backend and API Design

Typed Rust workspace DTOs own the wire contract. At minimum, the contract provides:

- a curriculum structure workspace containing term slots, programs, grade sections, catalog metrics, requirements, totals, validation notices, and concurrency versions;
- a typed bulk draft mutation for requirement additions, moves, copies, removals, and ordering;
- a homeroom delivery workspace containing expected requirements, linked offering/group summaries, status aggregates, and unlinked records; and
- an extended preparation preview/apply request and result with normalized grouping choices, source hash, idempotency, and conflict outcomes.

Exact route names are finalized in the implementation plan after all existing route registrations and generated client call sites are enumerated. Existing routes may be replaced in place where their purpose remains the same; obsolete wire fields are removed rather than accepted and ignored.

Handlers remain thin: authenticated tenant context, generated permission checks, resource policy, service call, typed `ApiResponse`, and a relevant realtime change signal after mutation. SQL, deduplication, totals, term-slot resolution, grouping, conflict detection, and idempotency remain in services and pure helpers with focused tests.

Services use bounded, set-based queries. The homeroom workspace must not query once per room, offering, group, teacher, or roster. Known JSON shapes use named Rust types, and all changed paths and schemas are registered with `utoipa`, regenerated into `contracts/openapi/school-api.json`, and consumed through generated TypeScript DTOs.

## Permission, Privacy, and Realtime Boundaries

Existing curriculum read/manage and delivery read/manage permission boundaries remain authoritative unless implementation analysis proves a narrower existing action is required. No broad permission is added merely to make the workspace convenient.

- Readers can inspect structures and delivery status without loading management-only option sets.
- Curriculum mutations require the existing curriculum management capability and published versions remain service-enforced read-only.
- Preparation apply and offering/group mutations require the existing delivery management capability.
- Detail actions continue to enforce resource policy, not only route visibility.

The homeroom overview returns homeroom labels, course/activity labels, group labels, and aggregate status/counts. It does not return student identities, national IDs, contact data, or full roster membership. Existing roster detail permissions remain the boundary for student-level data.

After a successful mutation, the backend emits the existing appropriate academic change signal. Realtime is only an invalidation hint; clients re-read the authoritative affected workspace and do not treat an event payload as permission or data truth.

## Data Migration and Contract Cutover

Implementation first reads the complete tenant migration timeline and adds only the next sequential forward migration. No applied migration is edited.

The preflight inventories:

- every curriculum requirement and its referenced catalog version;
- duplicated requirement credits/hours that match or disagree with catalog values;
- requirements with invalid or unmappable term codes;
- published curriculum versions whose term structure cannot be made explicit;
- activity versions missing an official total-hours value where one is required;
- homerooms whose study program is outside the applicable curriculum version; and
- existing offerings/groups whose source references or targets conflict with the normalized chain.

Deterministic rows are migrated to explicit curriculum term slots and catalog-owned metrics. Ambiguous rows are reported with their IDs and school-facing labels but are not guessed. The destructive part of the cutover runs only when preflight has no unresolved blockers.

The final schema removes requirement-level editable `credit`, `hours`, and free-form term-code ownership once their replacements are populated and verified. Activity metric changes use explicit typed catalog fields and publication validation. Required indexes and constraints enforce uniqueness and ownership relationships that services depend on.

Backend, generated contract, and frontend cut over as one release unit. There is no dual-write, fallback parser, old response adapter, hidden legacy form, or runtime compatibility layer. A Neon snapshot is required immediately before the migration on `sandbox` and any later production tenant.

## Error and Concurrency Behavior

- A stale draft save preserves the user's staged changes and asks them to reload and review differences.
- A published curriculum edit returns a domain conflict even if called outside the UI.
- A duplicate exact catalog version in the same program, grade, and curriculum term slot is rejected or normalized according to the preview, never silently duplicated.
- A missing official metric identifies the catalog item and links to its catalog page.
- A term-slot mismatch names the selected operational term and the applicable curriculum version.
- A stale preparation preview requires a new preview and performs no writes.
- Existing customized groups are conflicts requiring an explicit decision, not overwrite targets.
- Failed apply leaves offerings, targets, and groups unchanged.
- One failing row does not erase already loaded read-only workspace data.

## Visual and Interaction Direction

The workspace uses the existing SchoolOrbit PageShell, semantic color tokens, typography, density, and local shadcn-svelte primitives. Its distinctive visual reference is a Thai curriculum folio: clear term panels, restrained table rules, grouped curriculum categories, and strong numeric totals.

Desktop tables use sticky headers and readable fixed numeric columns with horizontal scrolling instead of excessive shrinking. Mobile stacks term panels and converts dense comparisons into program cards without changing information order. Multi-selection is keyboard accessible, focus is visible, destructive removal requires a reviewed staged state, and drag-and-drop is never the only way to move rows.

Loading uses skeletons matching the real table structure. Empty and prerequisite states explain the specific missing relationship. Motion is limited to brief state transitions and respects reduced-motion preferences. Dark mode and Thai/Buddhist Era display behavior remain consistent with the application.

## Release Sequence

### Release 1 — Curriculum Structure Workspace

- add explicit curriculum term slots and catalog metric ownership;
- implement migration preflight and the forward cleanup migration;
- replace ambiguous curriculum requirement contracts;
- add the structure workspace read model and atomic bulk draft mutation;
- implement compare-all-programs and single-program document views; and
- implement multi-select, copy, move, reorder, remove, undo, and preview behavior.

Release 1 is deployed and exercised on `sandbox` before Release 2 starts because Delivery depends on its canonical published structure.

### Release 2 — Homeroom Delivery Workspace

- add the set-based homeroom delivery read model;
- make by-homeroom the default and retain by-subject/activity as the secondary view;
- show normal, combined, split, elective, unlinked, roster, teacher, and timetable states;
- extend curriculum preparation preview/apply to include reviewed default grouping; and
- keep existing offering and group detail workflows as the advanced editing surface.

Each release has its own approved implementation plan, focused verification checkpoint, commit, push, automated deployment, and `sandbox` smoke test. Commands run one at a time because the local environment has known resource contention when Rust and frontend work run concurrently.

## Testing and Verification

Implementation follows test-driven development and runs checks serially.

Backend coverage proves:

- curriculum term-slot uniqueness, ordering, immutability, and operational-term resolution;
- catalog metric ownership and publication validation for courses and activities;
- complete independent program structures and copy semantics;
- atomic bulk replacement, ordering, and stale row-version behavior;
- set-based homeroom workspace grouping and status derivation;
- one offering reused across compatible program/grade targets;
- normal, combined, split, elective/optional, unlinked, and customized-group cases;
- stale source hashes, idempotent retries, changed idempotency requests, and full rollback;
- allowed and denied permission/resource-policy paths;
- migration preflight classification and idempotent deterministic migration; and
- OpenAPI registration of all changed typed DTOs.

Frontend coverage proves:

- comparison and single-program totals are derived from the typed workspace;
- dynamic term slots render without hardcoded two-term assumptions;
- published structures are read-only and draft bulk actions preserve staged changes;
- official credits/hours are visible but not editable in curriculum requirements;
- Topbar context is not misapplied to curriculum definition and is required for delivery;
- homeroom rows render normal, combined, split, elective, unlinked, and missing-dependency states;
- read-only users do not request management-only data;
- preview/apply clearly separates proposed work from committed work;
- no per-row request fan-out is introduced; and
- keyboard, responsive, focus, loading, empty, and error states remain usable.

Every analyzed or edited Svelte file is checked with the project Svelte tooling and all reported issues are resolved. Applicable `.rules` checks include focused Rust tests; migration compatibility through the explicit disposable database gate; `cargo fmt --all -- --check`; static architecture tests; `cargo check`; API contract generation/check/tests; frontend lint, Svelte check, static tests, and focused Playwright workflow coverage; `git diff --check`; final diff review; and `git status --short`.

## Deployment and Rollback

Release 1 is a coordinated backend/contract/frontend cutover because the old ambiguous curriculum contract is intentionally unsupported afterward. Preflight runs against a disposable copy first, then against `sandbox`; a Neon snapshot is taken immediately before the real tenant migration. Deployment must not continue while an ambiguous requirement, term mapping, or required activity metric remains unresolved.

Release 2 is deployed only after staff validate curriculum structures and totals in `sandbox`. Browser smoke coverage verifies both views and preview/apply without publishing rosters or assigning timetables.

Applied migrations are never reversed or edited. A corrective schema/data change uses a new reviewed forward migration. The previous application cannot be treated as a valid rollback after the incompatible Release 1 cutover; emergency database restoration from the snapshot is reserved for an immediate failed cutover before new authoritative writes, otherwise recovery moves forward.

## Success Criteria

- Staff can inspect a selected program in the familiar grade-and-term curriculum table with correct category and whole-term totals.
- Staff can compare all plans at one grade and identify their differences without opening each plan.
- Staff can add, move, copy, reorder, or remove many courses and activities while official catalog metrics remain non-editable.
- No supported curriculum requirement API stores an editable duplicate credit, ambiguous hours value, or free-form term code.
- A curriculum can define regular, summer, remedial, or custom term slots without frontend hardcoding.
- Opening Delivery shows what each homeroom should study and what operational setup is still missing.
- The same learning group is shown consistently under every homeroom it combines, while split groups remain distinct.
- Staff can preview and atomically create missing draft offerings and ordinary required groups without duplicate offerings or groups.
- Elective/optional grouping, teachers, physical rooms, timetables, and roster publication remain deliberate separate decisions.
- Curriculum and Delivery load through bounded typed workspace requests with no request-per-row pattern.
- Migration preflight, generated API contracts, permissions, frontend checks, backend checks, and `sandbox` smoke tests pass serially before each release is pushed.
