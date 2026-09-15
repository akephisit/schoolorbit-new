# Backend School Satellite Domain Extraction Plan

> **Execution:** Implement owner group by owner group under the approved crate-architecture
> specification and `.rules`. Each group must be independently buildable and reviewable.

**Goal:** Complete checkpoint 11 by extracting every remaining backend-school domain that meets
the crate-admission rule, while retaining genuinely application-level aggregation and runtime
side effects in the root package.

**Architecture:** The root application keeps Axum handlers/routes, full `AppState`, tenant/session
resolution, realtime and notification publication, scheduler iteration, OpenAPI composition, and
cross-domain view composition. Each admitted crate owns its models, validation, policies that can
be expressed with foundation APIs, SQL services, and focused tests. Cross-domain actions use
public operations, typed outcomes, or narrow application adapters; no crate imports root source.

## Admission Decision

| Candidate | Decision | Evidence and boundary |
|---|---|---|
| Admission | Extract `school-admission` | ~8,482 Rust lines, cohesive round/application/exam/score/selection lifecycle, 28 focused tests; file deletion remains a handler-side outcome. |
| Supervision | Extract `school-supervision` | ~8,228 lines, cohesive template/cycle/observation/evaluation/review lifecycle, 31 focused tests; depends one-way on Academic Core/Timetable and exposes Lifecycle preparation operations. |
| Staff and organization | Extract `school-staff` | ~5,950 lines, one identity/role/unit/membership/delegation owner, 26 focused tests; auth cache/realtime invalidation remains application composition. |
| Calendar | Extract `school-calendar` | ~4,389 lines and 53 focused tests; the crate owns calendar persistence/visibility while push/SSE delivery and all-tenant scheduling remain root adapters. |
| Question bank | Extract `school-question-bank` | ~2,761 lines and 13 focused tests; owns questions, rich content, subject/resource authorization, and file references while physical file deletion stays at the application boundary. |
| Work and workflow | Extract one `school-workflow` | The ~839-line work item service consumes the ~696-line workflow-window state machine directly; one crate preserves their shared lifecycle and eight focused tests without an artificial inter-crate edge. |
| Students | Extract `school-students` | ~1,600 lines and nine focused tests; owns student/parent records, encrypted field persistence, and student query shapes. Handler policy maps into a narrow crate-owned list-access enum. |
| Parents | Keep in root composition | ~788 lines and five tests; its primary behavior aggregates Student profiles with Calendar and Exam views. A separate crate would require multiple reverse ports and provide little local compile value. |
| School, facility, consent, achievement, lookup, menu, notification, system | Keep in root application | Each is small or dominated by application/runtime composition; none currently provides enough cohesive implementation plus isolated compile benefit to justify another package. Re-evaluate when material domain logic is added. |

## Dependency Direction

```text
foundation (errors, permissions, authorization, crypto, tenancy, file platform)
  ├── admission
  ├── staff
  ├── calendar
  ├── question-bank
  ├── workflow
  └── students

academic-core + academic-timetable -> supervision
supervision -> application Lifecycle provider adapter

all satellite crates -> backend-school application/composition
```

No satellite crate may depend on `backend-school`, full `AppState`, another crate's private source
path, or notification/session/realtime runtime state. Satellite-to-satellite edges require an
explicit owner relationship; none is required by this checkpoint.

## Task 1: Guard the Approved Satellite Graph

- [x] Add RED static architecture coverage for every admitted workspace member, centralized
  dependencies/lints, forbidden root imports, singular production ownership, and the exact allowed
  dependency edges.
- [x] Guard the retained Parents/application modules so the evaluation is explicit rather than
  implying that every folder must become a crate.
- [x] Record package tests and representative persistence coverage required before each owner move.

## Task 2: Extract Admission and Supervision

- [x] Move Admission models/services/PII validation and focused tests to `school-admission`; keep
  handlers, request context, and File Platform deletion orchestration in the application.
- [x] Move Supervision models/services/policy-safe operations to `school-supervision`; replace the
  root timezone import with the canonical Bangkok value owned by the crate and preserve Timetable
  and Lifecycle transaction scope.
- [x] Update handlers, OpenAPI imports, Lifecycle provider adapter, and source-path guards without
  changing routes, DTO serialization, SQL, permissions, or migrations.

## Task 3: Extract Staff/Organization and Students

- [x] Move staff, roles, permissions, organization units, memberships, delegations, audit writes,
  and models to `school-staff`; move service-returned DTOs out of handler ownership.
- [x] Keep cache invalidation, auth side effects, realtime signals, and request metadata in root
  handlers/adapters; map root resource scopes into a narrow crate-owned list access contract.
- [x] Move student models/services/encrypted persistence to `school-students`; preserve national-ID
  encryption/blind-index behavior and never expose or log plaintext identifiers.
- [x] Keep Parent cross-domain views in root and consume Student public models/operations.

## Task 4: Extract Calendar, Question Bank, and Work/Workflow

- [x] Move Calendar models, validation, event/category/tag/reminder persistence, visibility, and
  focused tests to `school-calendar`; keep NotificationService, broadcast channels, AdminClient
  iteration, and scheduled runtime wiring as application adapters.
- [x] Move Question Bank models/services/access policy to `school-question-bank`; use only public
  authorization and File Platform contracts and keep deletion orchestration in handlers.
- [x] Move workflow-window and work-item models/services/policy to one `school-workflow` crate;
  retain HTTP/realtime publication in the application.

## Task 5: Preserve Contracts and Runtime Behavior

- [x] Add at least one canonical-migration persistence test per admitted crate using dev-only
  `school-test-db`, or retain an existing stronger root integration test when the behavior is
  deliberately cross-domain.
- [x] Run every new package test, focused root adapters/integrations, backend static architecture,
  API/permission generators and tests, and prove tracked contracts and migrations byte unchanged.
- [x] Remove old production owners; root facades may re-export crate APIs only when they contain no
  SQL, validation, business rule, or duplicate model definition.

## Task 6: Verify Compile Scope and Checkpoint Candidate

- [x] Warm the single main target, then measure three representative comment-only workspace checks
  for each coherent owner group (or one representative per tightly related extraction batch).
- [x] Confirm Cargo rebuilds only the edited crate and actual consumers, while unrelated academic
  and satellite crates remain fresh; compare medians with the 59.445-second checkpoint-1 baseline.
- [x] Update the master status and measurements only after the exact candidate passes. The complete
  disposable root database suite and release/build gates may be consolidated with checkpoint 12,
  but must pass before the full program is declared complete.

## Completion Gate

Checkpoint 11 reaches candidate status only when all seven admitted owners have singular crates and
narrow public APIs; Parents and every other retained module has a written owner decision; Cargo
enforces an acyclic graph; root keeps only application composition/adapters for the moved domains;
API, permission, migration, PII, transaction, audit, cache, notification, and realtime behavior are
unchanged; focused tests pass; and representative edits demonstrate reduced compile scope.
