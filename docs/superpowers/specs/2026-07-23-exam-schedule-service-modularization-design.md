# Exam Schedule Service Modularization Design

**Date:** 2026-07-23  
**Status:** Approved  
**Scope:** Structural refactor of `backend-school` exam-schedule service only

## Context

`backend-school/src/modules/academic/services/exam_schedule_service.rs` currently contains about
4,876 lines. It owns round and day management, workspace hydration, item import, room and seat
assignment, invigilator management, session placement and conflict locking, publishing, and
published student/staff/parent views. These are cohesive exam-schedule capabilities, but keeping
all of them in one file makes feature work and review unnecessarily expensive.

The service already exposes a useful public boundary: handlers and parent services call
`exam_schedule_service::...`. The refactor will preserve that boundary and reorganize only the
implementation behind it.

## Goals

1. Split exam-schedule behavior into use-case-oriented modules that can be understood and tested
   independently.
2. Preserve every public Rust service path used by handlers, policies, and parent services.
3. Preserve HTTP behavior, generated OpenAPI, SQL semantics, transaction boundaries, and advisory
   lock ordering.
4. Move focused tests next to the behavior they cover and add database characterization tests for
   critical workflows before moving high-risk code.
5. Establish a repeatable compatibility-facade pattern for later supervision, calendar, and
   timetable refactors.

## Non-goals

- No API, DTO, permission, frontend, WebSocket, or response-contract changes.
- No schema or migration changes.
- No query optimization, index changes, caching, observability service, or behavior rewrite.
- No repository/domain abstraction layer and no mocking framework.
- No simultaneous refactor of supervision, calendar, or timetable.

## Selected approach: compatibility facade

`exam_schedule_service.rs` remains the public entry point. It declares private child modules and
re-exports their existing public functions and types. Existing callers continue to use paths such
as:

```rust
exam_schedule_service::create_round(...)
exam_schedule_service::place_exam_session(...)
exam_schedule_service::publish_round(...)
```

The facade contains module declarations and public re-exports only. It does not retain SQL or
business workflows. Rust resolves child files under `services/exam_schedule_service/`; no `mod.rs`
file is introduced.

This approach avoids broad handler churn and has no runtime dispatch or network overhead.

## Target structure

```text
backend-school/src/modules/academic/services/
├── exam_schedule_service.rs
└── exam_schedule_service/
    ├── shared.rs
    ├── rounds_and_days.rs
    ├── workspace.rs
    ├── room_assignments.rs
    ├── invigilation.rs
    ├── sessions_and_conflicts.rs
    ├── publishing.rs
    └── published_views.rs
```

### `exam_schedule_service.rs`

- Declares child modules.
- Re-exports the public functions and public outcome/input helper types currently consumed outside
  the service.
- Contains a facade-surface test or static guard that protects the caller-facing API.
- Contains no SQL, transaction, or business-rule implementation.

### `shared.rs`

- Owns only low-level pure types and helpers used by at least two sibling modules.
- Suitable examples include half-open time-range logic, deterministic conflict-lock keys, common
  session-window primitives, and narrowly shared validation errors.
- Does not become a miscellaneous helper file and does not own an end-to-end SQL workflow.

### `rounds_and_days.rs`

- Lists, creates, and updates exam rounds.
- Creates, updates, and deletes exam days and their configuration.
- Owns round mutability checks, round-kind normalization, day-window and blocked-window
  normalization, and the internal transition that marks a round draft after a mutation.

### `workspace.rs`

- Loads the administrative exam workspace and readiness counts.
- Imports exam items and clears mismatched items.
- Hydrates scheduled and unscheduled items for workspace responses.
- Exposes only narrow `pub(super)` readiness/count helpers needed by publishing.

### `room_assignments.rs`

- Lists and writes day-room assignments.
- Owns room capacity validation, seat generation, ordered student loading, seat hydration, and
  room-assignment write-error mapping.
- Exposes narrow internal lookup/context functions needed for session placement.

### `invigilation.rs`

- Loads the invigilator workspace and staff options.
- Adds, replaces, moves, and removes invigilator assignments.
- Owns workload calculation, live-session conflict validation, staff lock ordering, and
  invigilator view hydration.
- Exposes only the conflict/context operations required by room assignment and session placement.

### `sessions_and_conflicts.rs`

- Places and deletes exam sessions.
- Owns classroom, room, time-window, blocked-window, and grade-scope conflict checks.
- Preserves the existing advisory-lock scope and lock-before-read ordering.
- Calls the round-state transition in `rounds_and_days` after successful mutations.

### `publishing.rs`

- Locks and publishes an exam round.
- Uses workspace readiness/count helpers without duplicating readiness SQL.
- Preserves the existing rule that readiness is checked while holding the required round lock.

### `published_views.rs`

- Lists published schedules for the current student, staff member, and a parent's linked child.
- Owns active-user and parent-child validation for these read paths.
- Groups published exam-session rows into the existing response models.

## Dependency and visibility rules

The facade is the only supported entry point for callers outside the service. Child modules may
use sibling functionality through deliberately small `pub(super)` functions and types. Private
database row structs stay with the module that executes their query.

Dependencies should follow use cases rather than form a new generic layer:

```text
external callers -> facade -> public use-case functions
use-case modules  -> shared pure primitives
workspace         -> room/invigilator hydration helpers
publishing        -> workspace readiness helpers
sessions          -> room/invigilator conflict helpers + round draft transition
```

`shared.rs` must not depend on a use-case module. Any emerging circular dependency is resolved by
moving the smallest truly shared primitive downward or by keeping orchestration with the use case
that owns the transaction; it is not resolved by making whole modules public.

`backend-school/src/modules/academic/models/exam_schedule.rs` remains the owner of API/domain
models. The refactor does not move stable DTOs into service implementation files.

## Behavioral compatibility

The implementation is moved in complete behavioral slices:

- SQL text and bind order remain unchanged during extraction.
- A transaction begins and commits or rolls back at the same service boundary as before.
- Advisory locks are acquired in the same sorted order and before the same reads/writes.
- Error variants, HTTP statuses, and user-visible messages remain unchanged.
- Handler permission checks and resource policy calls remain unchanged.
- Public service functions and types are re-exported under their existing paths.
- OpenAPI operation inventory and generated TypeScript artifacts remain byte-for-byte unchanged
  unless a generator adds non-semantic ordering; any semantic diff blocks completion.

Refactor commits must not contain opportunistic query rewrites or business-rule changes. A defect
discovered during extraction is reproduced separately and fixed in a focused follow-up change.

## Testing strategy

### Characterization before extraction

The current file has broad pure/static coverage but limited database-level service coverage. Before
moving high-risk workflows, add focused PostgreSQL characterization tests using `TEST_DATABASE_URL`
and the repository's isolated-schema test helpers for:

- round and exam-day lifecycle;
- room capacity and generated seats;
- invigilator live-session conflicts and staff move semantics;
- concurrent session placement and advisory-lock behavior;
- publish readiness and publish locking;
- student, staff, and parent published views.

Tests assert observable outcomes, persisted rows, rollback behavior, and relevant error variants;
they do not duplicate implementation order unless lock order is the contract under test.

### Tests alongside extracted logic

- Move pure tests into the child module that owns the function.
- Update source-order/lock guards to read the owning child file directly.
- Keep handler/router contract guards at the architecture-test boundary.
- Each logic-bearing child module includes meaningful focused tests.
- The facade has a compile/static surface guard for every public function consumed by current
  handlers and parent services.

### Final verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- Full `backend-school` test suite against PostgreSQL 17 with required extensions in `public`
- Offline OpenAPI export and generated-contract drift checks
- `git diff --check`
- Direct audit that no migration or frontend path changed
- Final code review focused on transactions, lock ordering, facade completeness, and accidental
  visibility expansion

## Implementation sequence

1. Record baseline and add missing database characterization tests.
2. Create the facade structure and extract shared pure primitives.
3. Extract published views first because they are read-oriented and independent.
4. Extract rounds/days and workspace behavior.
5. Extract room assignments and seat generation.
6. Extract invigilation behavior and its conflict/lock helpers.
7. Extract session placement/deletion and conflict orchestration.
8. Extract publishing and remove all remaining business logic from the facade.
9. Tighten imports/visibility and add facade/module static guards.
10. Run full verification and independent code review.

Each extraction is a focused commit and must compile and pass its affected tests before the next
slice starts.

## Definition of done

- `exam_schedule_service.rs` is a compatibility facade with no SQL or business workflow.
- Public caller paths remain valid and handlers do not import child implementation modules.
- No replacement child file becomes another 4,000-line monolith; target files remain cohesive and
  generally below about 1,000 lines without artificial micro-files.
- Critical database workflows have characterization coverage.
- API, permissions, migrations, frontend behavior, and generated contracts are unchanged.
- Strict linting and the full PostgreSQL-backed backend suite pass from the reviewed final commit.

## Follow-on order

After this subproject is reviewed and integrated, separate design/plan/implementation cycles will
apply the same compatibility-first principles to:

1. supervision;
2. calendar;
3. timetable.

Timetable remains last because it is a core dependency of parent and supervision flows and first
needs stronger characterization coverage.
