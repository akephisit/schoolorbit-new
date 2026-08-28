# Academic Foundations Setup Clarity

**Date:** 2026-08-28

**Status:** Proposed

**Scope:** `backend-school` Academic Core and closely related foundation services, `frontend-school` staff academic foundation pages, typed OpenAPI/generated frontend contracts, focused data repair when deterministic, and verification for the planning-only workflow

## Context

SchoolOrbit now runs on the normalized Academic Core and Learning Delivery model. Academic years and configurable terms, bell schedules, periods, catalog identities and versions, curricula, student-year records, homerooms, offerings, groups, and rosters are separate authoritative concepts. The Topbar selects an explicit year or term context without activating it.

The model is sound, but several foundation forms still expose storage-oriented fields as if they were school decisions. For example, an academic year asks for both `2571` and an unrelated free-text display name; a term asks for sequence, code, name, type, and two lifecycle flags at the same time; a bell schedule asks users to invent `DEFAULT`; and period weekdays are entered as comma-separated English codes. Similar duplication appears in homeroom names/codes and several older academic editors. A valid request can therefore preserve internally consistent foreign keys while still recording contradictory or confusing school meaning.

This design improves the planning experience on the new schema. It does not restore a legacy model or introduce a compatibility path. It also does not activate or close an academic year or term. Operational lifecycle work remains owned by `SCH-002` after Gradebook and locked results exist.

This is a UX and invariant release named **Academic Foundations Setup Clarity**. It must not be confused with the already completed Academic Core cutover release or the future term-lifecycle release.

## Goals

- Ask staff only for meaningful school decisions and derive storage-oriented values.
- Prevent direct API clients as well as the UI from saving contradictory year, term, schedule, period, homeroom, or delivery relationships.
- Turn Academic Core setup into one understandable, planning-only path: year, bell schedule, periods, then terms.
- Keep official school/domain identifiers, such as subject codes and activity codes, visible and editable.
- Replace internal IDs, English enum codes, comma-separated machine values, and unconstrained role text with human-readable controls.
- Keep every ordinary catalog or registry page list-first and let each page check only its own direct dependencies.
- Preserve school-authored custom labels while repairing only deterministic legacy-standard mismatches.
- Remove confirmed unused raw-ID editors so there is one supported foundation workflow.
- Keep API contracts generated from typed Rust DTOs and run all verification serially.

## Non-Goals

- Do not activate, close, reopen, archive, or cancel an academic year or term.
- Do not implement Gradebook, score entry, result locking, term carry-forward, annual closure, promotion, graduation, or academic documents.
- Do not add a global academic readiness center or require all academic work before a term starts.
- Do not build a universal metadata-driven form engine.
- Do not automatically generate timetables, learning groups, rosters, or assessment structures.
- Do not hide official school codes merely because they are called `code` in storage.
- Do not edit any applied migration or maintain a parallel legacy UI/API compatibility path.
- Do not include admission PII changes in this release; admission privacy and predictable-credential work requires its own security-focused design.

## Approaches Considered

### Build a universal form engine first

A schema-driven form engine could centralize labels, derived fields, and selectors, but it would have to encode different rules for catalogs, curricula, schedules, terms, homerooms, and delivery. That abstraction would be harder to understand than the current domain services and would slow down future changes. It is rejected.

### Clean up only the frontend

Hiding duplicated inputs and mapping labels in Svelte would improve appearance, but direct API clients could still save contradictions and future pages could reintroduce them. It is rejected.

### Improve one domain release at a time with matching backend invariants — selected

Academic Foundations is the first bounded release. It combines a focused guided setup page, list-first registry pages, typed API changes, service validation, deterministic repair, and removal of confirmed dead editors. Later Assessment and Scheduling, Supervision, and Admission and Privacy releases can use the same interaction rules without a universal form engine.

## Design Principles

1. **One source for one meaning:** when a display value is the standard rendering of another field, the system derives it.
2. **Official versus internal codes:** official school codes remain school input; database/workflow codes are generated and hidden.
3. **Human labels at the UI boundary:** users select named years, terms, grades, programs, teachers, rooms, subjects, and activities, never UUIDs.
4. **Advanced only when meaningful:** a custom label or lifecycle-related term option is available behind an explicit advanced section with plain Thai explanation.
5. **Backend owns invariants:** the service derives or validates the same relationships; frontend convenience is not a correctness boundary.
6. **Planning is not activation:** creating and selecting a planning context never changes operational status.
7. **Local dependencies:** the guided sequence exists only inside Academic Core because those four records directly depend on one another. Other pages block only the action whose direct dependency is missing.
8. **History is intentional:** normalize only values proven to be old standard defaults. Preserve true school customization.
9. **No opaque fallbacks:** an unresolved relationship is an integrity error with a recovery action, not a UUID rendered as a label.

## Field Ownership Rules

Foundation fields fall into four explicit classes.

| Class | Examples | UI and API behavior |
|---|---|---|
| School-authored identity | subject code, activity code, curriculum code, study-program code | Required or optional according to the domain; shown with examples and existing uniqueness validation |
| Derived display value | standard academic-year name, standard term name, homeroom standard name | Previewed in the UI and derived by the backend; custom override is explicit where the domain allows it |
| Internal technical value | UUID, row version, bell-schedule internal code, generated ordering key | Never ordinary editable content; resource IDs come from selections/routes and concurrency versions stay inside typed requests |
| Constrained school choice | term type, grade, subject/activity type, advisor role, weekdays, default schedule | Rendered as shadcn-svelte selection, checkbox, switch, or command control with Thai labels |

An implementation audit must classify every editable field on the scoped pages before changing it. A field named `code` is not hidden until its domain ownership is confirmed.

## Academic Core Guided Setup

The route `/staff/academic/core` remains context-independent because it owns creation of academic contexts. The Topbar selector remains hidden on this route. The page keeps an in-page selected planning year and presents an **academic setup path** with four directly dependent steps.

This is not a mandatory global wizard. Existing years remain readable in a compact list/summary, completed steps may be reopened, and staff may leave and return without an in-memory-only draft.

### Step 1 — Academic year

Staff enter:

- year in Buddhist Era;
- start and end dates;
- ordinary school weekdays through named Thai weekday controls.

The standard display name is `ปีการศึกษา {year}`. The UI previews that name instead of asking for a second unrelated value. An advanced `ใช้ชื่อแสดงผลอื่น` control permits a deliberate custom label. Changing the year updates the standard label but never overwrites an existing explicit custom label without confirmation.

Validation requires a supported positive year, ordered dates, at least one school day, a unique year, and no duplicate standard identity. Error copy names the conflicting value in Thai. Creation remains `planning`.

### Step 2 — Bell schedule

The chosen planning year is inherited from step 1 and is not reselected. Staff enter a meaningful schedule name such as `ตารางเวลาปกติ` or `ตารางวันศุกร์`; they do not enter `DEFAULT` or another internal code.

The backend generates a stable unique internal code. The first schedule in a year becomes the default. Additional schedules can be made default through a named action, which transactionally clears the previous default. The UI explains that a default is only the schedule preselected for new terms; it does not activate the year.

### Step 3 — Periods

Periods are edited as readable rows under the selected schedule:

- order;
- optional display name;
- start time and end time;
- applicable school days;
- active state.

Rows default to the academic year's school days. `ใช้ทุกวันเรียน` is the ordinary choice; expanding a row exposes weekday checkboxes when the period applies only on selected days. Users never type `MON,TUE,...`.

The backend normalizes weekday ordering, rejects unknown or duplicate weekdays, rejects days outside the owning year's configured school days, requires positive unique order, requires start time before end time, and rejects overlapping active periods on the same applicable day. A replace operation remains atomic and concurrency-protected.

### Step 4 — Terms

Staff choose a term preset:

- regular term;
- summer term;
- remedial term;
- custom term.

The system proposes the next available sequence, stable internal code, and Thai display name from the preset and existing terms. Staff ordinarily enter only type, dates, and bell schedule. A custom label is an explicit advanced override.

The advanced section explains, in school language, whether the term contributes to the annual result and whether it must close before the year can close. Presets provide defaults, but staff may override them because summer and remedial policy differs by school. These settings are planning metadata only; this release provides no close-year operation.

Validation requires dates inside the owning year, ordered dates, a bell schedule owned by that year, unique sequence/code, and no silently contradictory preset-derived values. The number of terms remains derived from term rows; no separate `number_of_terms` field is introduced.

### Step and summary behavior

- Only the current incomplete step opens as the primary form; completed steps collapse to summaries with `แก้ไข` actions.
- A missing direct dependency disables only the next action and names what to complete.
- Existing years, schedules, periods, and terms remain readable even without manage permission.
- Read-only users see summaries without empty management forms.
- Planning status is always visible as `ฉบับเตรียมการ`; copy does not say `กำลังใช้งาน` unless the record is actually active.
- The page explicitly states that activation, closure, and promotion are separate protected workflows not included here.

## Catalog, Curriculum, and Delivery Foundation Pages

The following academic foundation surfaces are audited in this release: subject groups, subjects, activities, curricula and study programs, homerooms, student-year records and placements, offerings, learning groups, teacher assignments, and rosters.

### List-first interaction

Ordinary registries use readable desktop tables and equivalent mobile cards. Creation and editing use a dialog or sheet rather than keeping several unrelated forms open at once. Filters and summaries remain readable without management permissions.

Tables prioritize the fields staff use to recognize a record: official code, Thai name, type, grade/range, effective status/version, owning curriculum or term, and operational status. Internal IDs and row versions are not columns.

### Human-readable controls

- Grade, subject/activity type, curriculum version, study program, year, term, offering, homeroom, room, teacher, and student references use typed named options.
- Large option sets use searchable Combobox/Command behavior and load only after the exact action and permission require them.
- Advisor role is a constrained Thai selection for the backend-supported `primary` and `secondary` values, not free text.
- Homeroom standard code/name is derived from grade plus room number. A deliberate school-specific display-name override is advanced; the internal relationship still uses grade and room identifiers.
- Catalog and curriculum official codes remain editable and receive domain-specific examples and uniqueness errors.
- Delivery pages continue to use `รายการเปิดสอน`, `กลุ่มเรียน`, and `รายชื่อนักเรียนในกลุ่ม` as distinct concepts.

Page-local dependency guidance from the approved Academic Work Organization design remains authoritative. This release must not turn curriculum, delivery, timetable, assessment, or supervision into later steps of the Academic Core setup path.

## Backend and API Design

Typed Rust request DTOs distinguish ordinary school input from server-owned derived values. Where compatibility is not required, the supported create/update contract removes ordinary client ownership of internal fields instead of accepting and ignoring them.

Service-layer helpers own:

- standard year-name derivation and explicit custom-name handling;
- bell-schedule code generation and default selection;
- term preset proposals and validation;
- homeroom standard code/name derivation;
- weekday normalization;
- cross-year, cross-term, and cross-resource ownership checks;
- conflict and concurrency outcomes with actionable messages.

Handlers remain limited to context, generated permission checks, resource policy, service calls, typed envelopes, and required realtime signals. JSON changes are registered in OpenAPI and consumed through regenerated TypeScript DTOs. The frontend does not cast around transitional shapes.

No new broad permission is introduced merely to simplify the UI. Existing read/manage boundaries remain authoritative, and option endpoints return only the minimum fields needed for a human-readable selection.

## Data Repair and Cutover

There is one supported runtime path after this release. Confirmed unused editors that accept raw catalog-version, owner, program, grade, or other UUID text are removed only after following re-exports and all call sites.

Before any data repair, implementation produces a deterministic audit of scoped records:

- standard academic-year names that disagree with their numeric year;
- generated-looking bell-schedule or homeroom names/codes that disagree with authoritative component fields;
- invalid weekday codes or weekday sets outside the owning academic year;
- term preset-looking names/codes that disagree with their type and sequence;
- broken cross-year references.

Automatic repair is limited to rows that match a recognized old system-generated pattern and have no evidence of a custom school label. Ambiguous rows are reported and preserved for explicit review. Cross-resource violations that cannot be repaired deterministically block deployment instead of being hidden.

If persisted data changes are needed, they use a new forward-only sequential tenant migration after the full migration timeline is inspected. Applied migrations are never edited. If the audit finds no persisted inconsistency, no migration is added merely to satisfy the design.

The frontend and API cut over together to the new supported inputs. There is no legacy form, dual-write path, fallback UUID input, or permanent compatibility adapter.

## Error, Empty, and Concurrency Behavior

- Field errors appear beside the meaningful school field, not beside a hidden derived value.
- Cross-field errors identify both values and the owning record, for example a term date outside its academic year.
- Missing context, valid empty data, missing action prerequisite, permission denial, and request failure remain separate page states.
- Failed named-option resolution is shown as a data-integrity problem with a recovery route when one exists; no UUID is rendered as replacement copy.
- Stale row versions preserve the user's draft and offer reload/review rather than silently overwriting a newer change.
- Atomic period replacement and default-schedule changes either complete fully or leave the previous state unchanged.
- Existing readable data remains visible when one create/edit action fails.

## Visual and Interaction Direction

The UI continues the established SchoolOrbit blue/neutral design system, density, typography, PageShell, and shadcn-svelte primitives. The distinctive element is the real dependency path inside Academic Core, not a decorative progress dashboard.

The path uses compact numbered steps, completed summaries, plain Thai descriptions, and one emphasized next action. It avoids global completion percentages, celebratory states, and software terms such as context ID, row version, enum, or default code. Desktop uses full-width readable tables and a bounded editing surface; mobile keeps the same information order in cards/sheets. Keyboard navigation, visible focus, dark mode, reduced motion, and Thai/Buddhist Era date behavior remain required.

## Release Boundary and Follow-up

This design is the first of four serial clarity releases:

1. **Academic Foundations Setup Clarity** — this design.
2. **Assessment and Scheduling Clarity** — assessment structures, question bank, timetable, exam rounds, rooms, and invigilation.
3. **Supervision Workflow Clarity** — split the monolithic workspace by real supervision workflow and constrain internal states.
4. **Admission and Privacy Clarity** — admission workflow, national-ID permission/minimization audit, masking, exports, and removal of predictable initial credentials.

Each release receives a separate approved design, implementation plan, verification checkpoint, and deployment. Term lifecycle, Gradebook/results, and promotion remain under `SCH-002`, not release 2 of this clarity series.

## Testing and Verification

Implementation follows test-driven development and runs commands serially.

Backend coverage proves:

- standard and custom year names cannot contradict their ownership rules;
- bell-schedule codes/defaults are derived and unique per year;
- period weekday, ordering, time, overlap, and atomic replacement rules;
- term preset derivation plus explicit advanced overrides;
- homeroom derivation and same-year/resource ownership;
- allowed and denied permission/resource-policy paths;
- stale concurrency conflicts preserve authoritative data;
- any repair migration changes only deterministic recognized rows and is idempotent;
- OpenAPI registers every changed typed request and response.

Frontend coverage proves:

- Academic Core exposes the four-step planning path without activation actions;
- derived/internal fields are not ordinary editable controls;
- advanced overrides are discoverable, explained, and do not silently reset;
- weekday and advisor-role controls serialize only supported backend values;
- scoped foundation pages contain no editable UUID fallback;
- read-only users do not request management-only option data;
- each page preserves the approved missing-context, empty, prerequisite, permission, and error distinctions;
- responsive, keyboard, focus, date-picker, deep-link, and unsaved-change behavior;
- no per-row request fan-out is introduced.

Every edited Svelte component is checked with the project Svelte analyzer/autofixer. Applicable `.rules` checks include focused Rust tests; `cargo fmt --all -- --check`; backend static architecture tests and `cargo check`; API contract generation/check/tests; frontend lint, Svelte check, and static tests; focused Playwright coverage when the environment is available; `git diff --check`; final diff review; and `git status --short`. Commands run one at a time to avoid the known local resource contention.

## Deployment and Rollback

The release is implemented and verified against `sandbox` first. Additive or replacement backend contracts and any forward migration deploy before or together with the frontend that consumes them. Browser smoke coverage confirms creation and editing through human-readable controls.

Rollback may restore the previous application version only while its contract remains accepted by the deployed backend. A forward data repair is not reversed by editing or deleting its migration; any required corrective action uses a new reviewed forward migration. No production activation, closure, result, or promotion state changes during this release.

## Success Criteria

- A staff member can create a planning year, schedule, periods, and terms without inventing a duplicate name, machine code, UUID, or English weekday list.
- Entering year `2571` produces the standard name `ปีการศึกษา 2571` unless the user deliberately enables a custom label.
- Ordinary term creation requires type, dates, and schedule while preset-derived details remain understandable and reviewable.
- Foundation registries show enough school-facing detail to identify records and use constrained named selections for relationships.
- Direct API requests cannot save the contradictions the UI prevents.
- Existing custom school labels remain intact, and only deterministic old standard mismatches are repaired.
- No active foundation workflow retains an editable raw-ID component or a UUID label fallback.
- Planning contexts remain selectable for work without being treated as operationally active.
- Gradebook, closure, activation, and promotion remain unavailable until their own readiness design is implemented.
