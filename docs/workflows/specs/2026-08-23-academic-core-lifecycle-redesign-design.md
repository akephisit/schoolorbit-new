# Academic Core and Academic Lifecycle Redesign

**Date:** 2026-08-23

**Status:** Approved

**Scope:** `backend-school`, `frontend-school`, tenant database schema and data, permission/API contracts, realtime signals, and production cutover procedures that depend on academic-year or academic-term context

## Context

SchoolOrbit currently models academic years and semesters primarily with mutable rows and global `is_active` flags. Course planning materializes curriculum data into `classroom_courses`; timetable, assessment, exam, activity, supervision, parent, admission, and certificate flows then depend on combinations of academic year, semester, classroom, subject, and study-plan rows. These dependencies do not use one consistent academic context or one consistent versioning rule.

The current enrollment service marks every active class enrollment for a student as moved out before assigning a new classroom, regardless of academic year. This prevents safe preparation of a future year while the current year remains active. Subject and activity generation also resolve versions differently: course generation can retain the exact old subject row while activity generation resolves a later effective version. Assessment plans are shared implicitly by semester and subject rather than through an explicit offering boundary. Several flows infer the current semester independently.

The system does not yet own a complete Gradebook, locked annual result, term-transition workflow, or production promotion workflow. Building promotion directly on the current relationships would make the missing result system and the existing cross-year ambiguity permanent.

This design replaces the academic core, migrates real tenant data during one hard cutover, and establishes one model for curriculum, delivery, results, term transitions, annual closure, and promotion. After cutover, the new model is the only runtime source of truth. There is no legacy read/write compatibility path.

## Goals

- Prepare a future academic year and future terms while the current term and year remain operational.
- Support any configured sequence of regular, summer, remedial, or custom terms without hard-coding two terms per year.
- Separate reusable templates from published operational snapshots so template edits never silently change live teaching data.
- Give subjects, activities, curricula, and study programs stable identities with explicit immutable versions.
- Model courses and student-development activities through one delivery boundary while preserving their different result semantics.
- Make timetable, attendance, teaching logs, assessment, exams, supervision, parent views, and other consumers use one explicit academic context.
- Deliver a complete Gradebook and locked term/year results before promotion uses those results.
- Make term closure, year closure, promotion, and activation reviewable, auditable, idempotent, and recoverable from interrupted batch work.
- Preserve all valid production data through a rehearsed, validated, maintenance-window cutover.
- Remove legacy runtime code, API contracts, and tables after cutover so future development has one model to understand.

## Non-Goals

- Do not rewrite unrelated account, staff, organization, file, session, or notification systems.
- Do not build unrelated missing products such as tuition billing, a full library system, or a new automatic timetable solver.
- Do not copy scores, attendance, exam results, teaching logs, or supervision records from one term into another.
- Do not let promotion infer results directly from raw score rows.
- Do not introduce event sourcing or retain a permanent compatibility layer between old and new academic schemas.
- Do not support multiple concurrent school-wide active academic calendars in the first version. A tenant has at most one active academic year and at most one active academic term, while any number of future years and terms may be in planning.

## Approaches Considered

### Normalize the legacy schema in place

This would preserve more current table and API names, but existing tables combine identity, version, plan, delivery, and operational state. Incremental normalization would require long-lived compatibility branches and leave developers deciding which relationship is authoritative. It was rejected.

### Full event-sourced academic ledger

An event ledger could reproduce every state transition, but it would add projection, replay, operational, and debugging complexity far beyond current requirements. Append-only audit records and immutable published snapshots provide the required traceability without event sourcing. It was rejected.

### Clean Academic Core with one hard cutover — selected

Build normalized replacement boundaries, migrate every affected consumer and real tenant dataset, validate the result during maintenance, and deploy a backend/frontend release that only understands the new model. This has the largest initial release but provides the clearest invariants and the fastest path for later Gradebook and promotion development.

## Architectural Principles

1. **Explicit context:** term-scoped reads receive `academicTermId`; year-scoped reads receive `academicYearId`. Active values are UI defaults, not hidden query authority.
2. **Stable identity, immutable version:** a subject, activity, or curriculum keeps one stable identity while effective details live in version rows.
3. **Template versus snapshot:** templates accelerate setup; published offerings, assessment plans, schedules, and results do not follow later template edits automatically.
4. **Preview before apply:** curriculum generation, term carry-forward, roster generation, and promotion first produce a reviewable diff or recommendation.
5. **History is not overwritten:** future preparation creates future records. It never mutates the student's completed year, term, placement, scores, or results.
6. **Locked inputs for consequential decisions:** promotion reads only locked annual results.
7. **Idempotent runs:** setup, calculation, and execution operations tolerate retries and resume without duplicates.
8. **Database-backed integrity:** composite foreign keys, unique indexes, checks, and transaction boundaries prevent cross-year and cross-term relationships.
9. **No silent deletion:** published or closed academic records are archived, superseded, or corrected through an audited workflow.
10. **One runtime model:** after cutover, legacy academic tables and contracts are not read, written, or kept behind feature flags.

## Domain Boundaries

The backend is divided into four cohesive domains. The released code exposes only these new boundaries; transitional implementation code is removed before cutover.

### Academic Core

Owns stable subject/activity identities and versions, curricula and versions, study programs, grade progression rules, academic years, academic terms, bell schedules, homerooms, student-year records, and placement history.

### Learning Delivery

Owns term offerings, course/activity subtype details, learning groups, teachers, planned homeroom coverage, authoritative rosters, timetable integration, activity delivery, and related publishing state.

### Gradebook and Results

Owns assessment templates, offering assessment plans, score sheets, student scores, course/activity results, term results, annual results, approval/locking, and result corrections.

### Academic Lifecycle

Owns term readiness and transitions, annual readiness and closure, promotion policies, promotion runs and decisions, target-year preparation, activation, and lifecycle audit events. It orchestrates domain services; it does not embed direct cross-module SQL.

## Data Model

### Stable catalog and curriculum versions

```text
subjects
└ subject_versions

activities
└ activity_versions

curricula
└ curriculum_versions
   └ study_programs
      └ curriculum_course_requirements
      └ curriculum_activity_requirements
```

`subjects` and `activities` own stable school-level identity and code. Effective name, description, classification, hours, default credit, and other changeable details belong to version rows with non-overlapping effective ranges. Historical offerings retain the exact version they published.

`curriculum_versions` are editable only while draft. Publishing makes the version immutable. A later curriculum revision creates a new version. `study_programs` represent tracks such as normal, science-mathematics, or language programs. Schools without tracks receive one default program.

Requirements describe what a program expects, including grade level, recommended term position or term type, credits/hours, requirement kind, and optional default assessment/activity policy. Requirements are planning input, not live course rows.

### Academic years and configurable terms

```text
academic_years
└ academic_terms
   └ term bell-schedule selection
```

Academic-year states are:

```text
planning -> ready -> active -> closing -> closed -> archived
```

Academic-term states are:

```text
planning -> ready -> active -> closing -> closed
planning -> cancelled
```

An academic term stores `sequence_no`, stable code, display name, `term_type` (`regular`, `summer`, `remedial`, or `custom`), start/end dates, `included_in_year_result`, `blocks_year_closure`, status, and selected bell schedule. Dates must be ordered and fall within the owning academic year. Sequence and code are unique within that year.

The system does not store a separate `number_of_terms` value. The count is derived from term rows. Setup presents presets for two regular terms, two regular terms plus summer, three regular terms, and custom configuration. Presets create ordinary editable planning rows; they do not introduce special runtime branches.

Closing term 2 does not imply closing the year. A year becomes eligible for closing only when every non-cancelled term marked `blocks_year_closure` is closed. A summer/remedial term that can alter annual results must set both `included_in_year_result` and `blocks_year_closure`.

### Students, homerooms, and grade progression

```text
academic_years
├ homerooms
│  └ homeroom_placements
└ student_academic_years

grade_level_progressions
```

`student_academic_years` is the authoritative record of a student's grade level, study program, and academic status for one year. There is one row per student and academic year. Statuses are `planned`, `active`, `completed`, `withdrawn`, and `graduated`.

`homeroom_placements` records placement history against a student-year row. At most one placement is current for a student-year, but ended placements remain historical. Preparing a future placement never moves the current-year enrollment. A mid-year room transfer closes the old placement and opens a new placement through an explicit transfer service.

`grade_level_progressions` replaces the assumption that every grade has one unconditional `next_grade_level_id`. It defines permitted transitions by school/curriculum context and supports promotion, repetition, completion, and approved exceptional movement.

### Learning offerings and groups

```text
learning_offerings
├ course_offering_details
└ activity_offering_details

learning_groups
├ learning_group_homerooms
├ learning_group_teachers
└ learning_group_students
```

Every `learning_offering` belongs to one academic term, has kind `course` or `activity`, records its source curriculum requirement when applicable, and moves through `draft`, `published`, and `closed`. Publishing creates an operational snapshot that later curriculum edits cannot mutate.

Every offering has exactly one subtype row matching its kind. Database constraints reject a course without course details, an activity without activity details, or an offering carrying both detail types.

Course details record subject version, target grade/program, credits, hours, grading policy, and assessment-plan relationship. Activity details record activity version/type, assignment or self-registration behavior, capacity, attendance requirements, and pass criteria.

`learning_groups` represent actual teaching groups. A group may cover one or more homerooms. `learning_group_homerooms` records planning coverage; `learning_group_students` is the authoritative roster consumed by Gradebook, attendance, exams, and parent/student views after roster publication. This supports electives and individual overrides without changing homeroom placement. `learning_group_teachers` owns instructor assignment and role.

An offering may have multiple groups that share one assessment plan explicitly. If groups require different assessment structures, planning creates separate offerings. Timetable, attendance, daily teaching, and supervision observations reference `learning_group_id`, not a mutable classroom-course bundle.

### Assessments, scores, and results

```text
assessment_templates
-> course_assessment_plans
-> score_sheets
-> student_assessment_scores
-> learning_results
-> student_term_results
-> student_year_results
```

Assessment templates are reusable setup aids. Applying a template creates an independent `course_assessment_plan` for one offering. Plan states are `draft`, `published`, and `locked`. Once score entry begins, structural changes require an audited revision/correction path rather than deletion or silent reweighting.

A `score_sheet` belongs to one learning group and moves through `draft`, `submitted`, `returned`, `approved`, and `locked`. Student score rows store decimal values and a distinct status: `scored`, `missing`, `absent`, `exempt`, or `pending`. Missing work is never represented as numeric zero unless the grading policy explicitly awards zero.

`learning_results` provides a common result header. Course results store numeric/letter grade, credits attempted/earned, and GPA contribution. Activity results store attendance or completion evidence and pass/fail outcome; they do not contribute to GPA unless a future explicit policy says otherwise.

`student_term_results` aggregate one student's locked term outcomes. `student_year_results` aggregate included closed terms and move through `draft`, `calculated`, `reviewed`, `approved`, and `locked`. Promotion can reference only a locked annual result.

Locked corrections create `result_corrections` with before/after values, reason, requester, approver, timestamps, and recalculation outcome. A correction that affects an unexecuted promotion invalidates its recommendation and requires recalculation. After promotion execution, a correction never silently moves the student; it raises a lifecycle impact requiring a separately approved placement/promotion adjustment.

All score, credit, weight, and GPA values use PostgreSQL `NUMERIC` with explicit precision and scale. Application DTOs avoid binary floating-point for authoritative calculations.

## Academic-Term Lifecycle

### Prepare the next term

While term 1 remains active, term 2 or summer may remain in planning. The transition workspace can prepare:

- curriculum-derived course and activity offerings;
- learning groups, teachers, and draft rosters;
- a new timetable or a validated draft copied from the previous term;
- independent assessment plans with blank score sheets;
- exam rounds and schedules;
- activity registration/assignment;
- term-scoped supervision cycles;
- calendar dates, exceptions, and bell-schedule selection.

Carry-forward uses `preview -> apply`. It may copy configuration and assignments as draft but never copies scores, attendance, exam results, teaching logs, or supervision records.

### Close the current term

The source term moves from `active` to `closing`. Each owning module returns readiness findings with severity `ready`, `warning`, or `blocking` and an actionable route. Required checks include submitted/approved score sheets, completed course/activity results, attendance summaries when present, resolved enrollment changes, and locked term results.

Warnings require explicit acknowledgement. Blocking findings prevent closure. Once closed, ordinary writes for term-owned operational data fail with an actionable domain error. Corrections use the dedicated audited path.

### Activate the next term

Activation requires the target term to be `ready`, its owning year to be active, the source active term to be closed, and all configured target readiness gates to pass. Activation changes statuses and the default context transactionally; it does not generate a large body of data at activation time. Activating the first term of a new year occurs in the same lifecycle transaction as target-year activation. A tenant may have a period with no active term, but never two active terms.

## Academic-Year Closure and Promotion

### Prepare a future year

A future year may be `planning` while the current year is active. The school may create terms, calendars, programs, homerooms, advisors, draft offerings/groups, and draft timetables without changing current student records.

### Close the source year

After all blocking terms close, the year moves to `closing`. The system calculates, reviews, approves, and locks annual results. Only when every required student annual result is locked or placed on an explicitly approved hold may the year become `closed`.

### Promotion runs and decisions

```text
promotion_runs
└ promotion_decisions
```

A promotion run stores source year, target year, immutable policy version, input snapshot, creator/approver/executor, timestamps, and status:

```text
draft -> calculated -> reviewed -> approved -> executing -> completed
                                                    -> failed
```

An explicitly retried failed run returns to `executing` and resumes only decisions that do not already have a completed execution marker.

Each decision stores the source student-year, locked source annual result, recommended outcome, final outcome, target grade/program when applicable, override reason, execution status, and created target student-year ID.

Supported outcomes are:

- `promote`: create a planned target student-year at an allowed next grade;
- `repeat`: create a planned target student-year at the same grade;
- `graduate`: create no target student-year and complete the source record;
- `transfer_out`: create no target student-year;
- `hold`: create no target student-year until resolved;
- `conditional`: create a planned target student-year linked to an explicit approved condition.

Calculation produces recommendations only. A human with the required permission reviews and approves final decisions. Overrides require a reason. Execution is resumable and idempotent per decision: unique constraints prevent duplicate target student-year or placement records, and completed decisions are not repeated.

### Activate the target year

The activation workspace checks target year/term state, curriculum selection, homerooms/advisors, promotion decisions, planned student-year rows, placements, published offerings/groups, rosters, and configured timetable/calendar gates. Activation changes the target year and eligible planned student-year/placement records to active. It opens prepared data; it does not copy or regenerate it.

## Academic Context and Topbar

Staff academic pages use one route-aware Academic Context Switcher in the Topbar. On desktop it presents linked year and term controls; on mobile it collapses to one button such as `2570 · เทอม 2`. Status labels distinguish active, planning, ready, and closed contexts.

Selecting a context changes only what the user is viewing or editing. It never activates a year or term. Actual transitions remain in permission-protected lifecycle workspaces with readiness checks.

Each route declares one context requirement:

- `none`
- `year_required`
- `term_required`
- `term_optional`

Term-optional pages offer `ทั้งปี`. Non-academic routes hide the selector. Parent/student pages use a simpler page-local history selector rather than a staff-wide management context.

The selected IDs are encoded in the URL so refresh, browser history, deep links, and bookmarks preserve context. Client state may remember a convenience default, but URL/request values are authoritative. Changing context with unsaved edits triggers a navigation guard.

List endpoints require their relevant context ID. Resource mutation endpoints derive context from the authoritative resource and reject duplicated payload context that disagrees. The backend verifies term ownership and cross-resource consistency; it never trusts the Topbar alone.

## Affected Consumers

The hard cutover includes every existing consumer that directly or indirectly relies on academic years, semesters, subjects, study plans, classrooms, enrollments, classroom courses, or activities.

| Consumer | Required redesign |
|---|---|
| Academic planning | Plan through curriculum versions, programs, requirements, offerings, and groups |
| Classrooms/enrollment | Use student-year records and placement history; future preparation must not move current students |
| Timetable/daily teaching | Reference term learning groups and explicit context; copied schedules remain drafts |
| Assessment | Replace implicit semester/subject sharing with offering plans and group score sheets |
| Question bank | Reference stable subjects so reusable questions survive subject-version changes |
| Exam scheduling | Reference term offerings/groups and their assessment plans with same-context constraints |
| Activities | Use activity offerings/groups and term rosters while preserving pass/fail semantics |
| Supervision | Require academic year and allow an optional term for year-long cycles |
| Admission | Target an academic year/program and create planned student-year/placement records at enrollment handoff |
| Parent/student views | Query explicit context and expose only authorized learner data |
| Certificates/reports | Resolve year/term from locked source records; unrelated certificate campaigns remain unchanged |
| Calendar | Remain date-owned; optionally attach academic context for instructional events |
| Lookup/search | Return stable identities and the explicit effective version needed by the caller |

Systems that do not currently exist are not built merely because they may later consume the core. Their integration contract is prepared: attendance and teaching use `learning_group_id`; annual student services use `student_academic_year_id`; term services use `academic_term_id`.

## Backend, API, and Realtime Boundaries

Handlers remain thin: canonical tenant/request context, permission and resource policy, typed request validation, domain service call, standard `ApiResponse<T>`, and a realtime signal when required. Business rules, transactions, calculations, and SQL belong in focused services.

The Academic Lifecycle service requests typed readiness results from Gradebook, Learning Delivery, timetable, exam, activity, and supervision services. It does not update their tables directly. Adding a future module requires a readiness provider rather than editing one giant transition query.

Rust DTOs and OpenAPI registration remain the wire-contract source of truth. Frontend DTOs are generated and mapped to UI view models only when presentation semantics differ. Known responses never use `unknown`, ad-hoc JSON, or response casts.

Realtime events are change signals, not authorization or data truth. Signals include context status changes, offering/group publication, score-sheet status changes, result locks, and promotion-run progress. Receiving clients re-read the authoritative typed HTTP resource.

## Authorization and Audit

Permission families separate read, manage, enter, review, approve, lock/correct, transition, and execute capabilities. Resource policies apply existing scopes:

- teachers enter/read scores for `assigned` learning groups;
- subject/organization reviewers use `organization_unit` or `organization_tree` scope;
- school academic staff manage school-wide planning and approval through `school` scope;
- term/year transitions and promotion execution require explicit school-scoped capabilities.

Target capability families include `academic_year`, `academic_term`, `learning_offering`, `gradebook`, `academic_result`, and `academic_promotion`. Exact entries follow `module.action.scope` and are generated from `contracts/permissions.json`. Obsolete `all`-scoped or coarse promotion permissions are removed rather than retained as aliases.

Permission migration preserves equivalent access without escalation. Existing comparable school-wide manage/execute grants may map to the corresponding new capability. New correction, approval, or execution authority is not inferred from an unrelated read or assigned-level grant. Preflight reports role mappings before cutover.

State transitions and consequential changes append audit records containing resource/run ID, before/after status or value, actor, reason when required, timestamp, and request/idempotency identifier. Audit payloads and logs exclude plaintext national IDs, secrets, raw request bodies, and unnecessary PII.

## Concurrency and Error Semantics

- Mutable drafts carry a version or updated-at precondition. A stale update returns a conflict with instructions to reload rather than overwriting another user's work.
- Closed/locked resource mutations return a domain error identifying the required correction workflow.
- Readiness responses contain stable machine codes, severity, Thai user-facing guidance, affected count, and a route/context for resolution.
- Batch runs store per-item status and a run-level summary. Retrying resumes pending/failed-safe items and skips completed items.
- Preview/apply operations bind the preview to a source-version hash. Apply fails if the source or target changed after preview.
- Database uniqueness and composite foreign keys are the final protection against duplicate or cross-context writes.

## Data Migration and Hard Cutover

### Mapping

The migration performs these conceptual transformations:

| Legacy owner | New owner |
|---|---|
| `academic_semesters` | `academic_terms` |
| version-like `subjects` rows | stable `subjects` plus `subject_versions` |
| `study_plans`, versions, subjects, activities | curricula, versions, programs, and requirements |
| `class_rooms` | `homerooms` |
| `student_class_enrollments` | `student_academic_years` and `homeroom_placements` |
| `classroom_courses` | course `learning_offerings` and `learning_groups` |
| activity slots/groups/members | activity offerings/groups/rosters |
| assessment plans/items | offering assessment plans/items |
| timetable/exam/supervision references | new term, offering, group, and stable subject references |

Existing UUIDs are preserved where source and target represent the same identity. Split/merged entities receive deterministic target IDs so rehearsal and production mapping are reproducible. No source row is silently discarded; every row must map, be explicitly classified as obsolete empty draft data, or block cutover.

Legacy `is_active = true` years and terms map to `active`. An inactive row ending before the cutover date maps to `closed`; a row starting after the cutover date maps to `planning`. An inactive row whose dates include the cutover date, overlapping active rows, or dates that cannot determine one unambiguous state block preflight and require an explicit reviewed mapping before migration. Student-year, placement, offering, and group states derive from these resolved year/term states plus their source status; conflicting source statuses also block preflight.

The legacy product has no complete locked term/year result system. Migration does not fabricate scores or results for historical closed years. Those rows receive auditable migration provenance, remain readable as historical academic structure, and cannot be used as promotion input. The locked-result closure invariant applies to lifecycle transitions performed by the new result system. Promotion becomes available only for a source year whose complete `student_year_results` were calculated and locked under that system.

### Preflight

A read-only preflight command uses the same mapping rules as migration and runs against every tenant before any cutover migration is applied. It reports duplicate/conflicting subject identities, cross-year relationships, overlapping active enrollments, invalid statuses, orphan schedules/plans/activities, date violations, permission mappings, and source/target cardinality expectations.

All blocking findings are resolved through reviewed data repair before release. Because applied migrations are immutable across tenants, the cutover migration is not edited after one tenant applies it. An unexpected later issue requires a new sequential repair migration.

### Rehearsal

The full migration and new release run repeatedly against protected snapshots/clones of representative real tenant data. Rehearsal verifies row counts, relationship counts, numeric totals, representative user workflows, migration duration, and application performance. No production secret or plaintext sensitive value enters fixtures or logs.

### Maintenance cutover

1. Put the school application into global maintenance mode and stop writes, workers, and academic realtime mutations.
2. Confirm source counts are stable and take a recoverable database snapshot under the production retention policy.
3. Apply the new schema and backfill migrations through the centralized migration runner. Each migration is transactional where PostgreSQL and the migration operation support it. A failed validation stops that tenant's sequence, does not mark the failing migration applied, and keeps the platform in maintenance.
4. Run reconciliation: semesters, student-year/placement coverage, offerings/groups, rosters, timetable entries, activities, assessment items, exam references, and permission grants.
5. After every tenant passes reconciliation, apply the final sequential cleanup migration through the same runner to drop superseded academic tables and run final schema guards.
6. Deploy the new backend/frontend release and generated API/permission contracts while traffic remains closed.
7. Run authenticated smoke tests through the deployed proxy against selected tenant contexts.
8. Open traffic. The new Academic Core becomes the only source of truth.

The application contains no dual-read, dual-write, legacy feature flag, or per-tenant compatibility branch. The database snapshot, not live legacy tables, is the rollback source.

### Rollback boundary

Before accepting a new-system write, rollback restores the snapshot and deploys the previous release. After opening traffic, the old application cannot be deployed alone against the new schema. Emergency recovery requires maintenance and either a reviewed forward fix or snapshot restoration with explicit acceptance/reconciliation of post-cutover writes. The go-live decision therefore occurs only after migration reconciliation and smoke tests pass while writes remain disabled.

## Delivery Sequence

This architecture is one coherent target but is too large for one implementation plan. Delivery uses four independently reviewed releases. The next implementation plan covers Release 1 only.

### Release 1 — Academic Core Cutover

- New core, delivery, year/term, student-year, placement, offering, and group schema.
- The minimum new offering assessment-plan schema required to migrate and preserve the existing assessment-planning feature; student score entry and result calculation remain Release 2.
- Explicit context APIs and route-aware Topbar switcher.
- Migration of real data and permission mappings.
- Conversion of all current affected consumers to the new model.
- One production hard cutover and removal of legacy runtime paths.

### Release 2 — Gradebook and Results

- Assessment-template authoring and the full assessment-plan lifecycle, score sheets, typed score states, course/activity results, term/year aggregation, approval/locking, and corrections.
- A useful complete score/result system without requiring promotion.

### Release 3 — Term Lifecycle

- Term presets, preparation and carry-forward preview/apply, modular readiness, closing/activation, summer/remedial behavior, and transition audit.

### Release 4 — Annual Lifecycle and Promotion

- Annual closure, versioned promotion policies, recommendations, review/approval/override, idempotent execution, graduation/repetition/transfer/hold/conditional outcomes, target-year placement, and activation readiness.

Each later release builds only on the new core. It does not reintroduce or temporarily revive legacy schema support.

## Verification Strategy

### Domain and service tests

- Version-range and published-snapshot immutability.
- Term/year state transition matrices.
- Curriculum preview/apply source-hash conflicts and retry behavior.
- Score status, decimal calculation, aggregation, locking, and correction impact.
- Summer inclusion/blocking policies.
- Promotion recommendations, overrides, outcomes, resumability, and duplicate prevention.

### Database integration tests

- One student-year per student/year and one current placement per student-year.
- At most one active year and term.
- Same-year and same-term composite foreign-key rejection.
- Offering/group/roster and assessment/group consistency.
- Closed/locked data mutation constraints where database enforcement applies.
- Migration from representative legacy fixtures, including known inconsistent edge cases.

### Authorization and contract tests

- Allowed, denied, and union behavior for assigned, organization, tree, and school scopes.
- No capability escalation during permission migration.
- Generated permission registries and OpenAPI/TypeScript DTOs remain synchronized.
- Parent/student responses exclude unrelated learners and unnecessary PII.

### Frontend and end-to-end tests

- Topbar context persists in URL, changes dependent term options, respects route scope, and does not activate a term.
- Unsaved edits prevent accidental context loss.
- Preparing term 2 does not change active term 1 data.
- Blocking readiness items link to the correct context and resolution page.
- A locked term/result rejects ordinary edits and offers the correction path.
- End-to-end flows cover future-year preparation, term transition, Gradebook approval, year closure, promotion retry, and target-year activation as each release is delivered.

### Cutover verification

- Preflight passes every tenant before migrations begin.
- Rehearsal and production compare counts and relationship totals, not only migration exit status.
- `git diff --check`, generated contract checks, backend/frontend verification, migration/schema compatibility tests, and deployed smoke tests follow the change-type matrix in `.rules` and the operational procedures in `docs/OPERATIONS.md`.

## Acceptance Criteria

- Current and future academic-year student records coexist without altering one another.
- A school can configure two, three, summer, remedial, or custom terms through term rows and presets.
- Every affected runtime consumer uses explicit new academic context and no legacy academic table.
- Curriculum/template changes cannot silently alter published offerings or result structures.
- Course and activity delivery share offering/group infrastructure while keeping distinct result rules.
- Term activation is a readiness-gated status change, not hidden bulk generation.
- Annual closure requires configured terms and locked annual results.
- Promotion uses only locked annual results, requires human approval, and is idempotent.
- Real tenant data passes preflight, rehearsal, migration reconciliation, and smoke tests before writes reopen.
- Rollback uses the captured snapshot; no legacy compatibility code or live legacy tables remain after successful cutover.
