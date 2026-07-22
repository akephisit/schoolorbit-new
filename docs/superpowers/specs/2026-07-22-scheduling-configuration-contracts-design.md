# Scheduling Configuration Contracts Design

**Date:** 2026-07-22

**Status:** Approved for implementation

**Scope:** Move the Scheduling Constraints configuration surface into the generated Rust/OpenAPI/TypeScript contract, replace six independent mutation routes with one typed atomic patch endpoint, and correct verified patch, target-validation, and frontend save-flow behavior.

## Context

The generated school API currently contains 177 unique operations. Course Planning is the latest completed batch and establishes Rust serde DTOs plus `utoipa` metadata as the source of truth, deterministic OpenAPI and TypeScript generation, standard JSON error envelopes, generated permission constants, thin handlers, and database-backed service tests.

Scheduling Configuration is the next adjacent batch. Six reads and six mutations are currently implemented across `handlers/scheduling_config.rs`, `scheduling_config_service.rs`, and `frontend-school/src/lib/api/scheduling.ts`, but none are in the generated API document. The frontend duplicates every wire DTO. Verified drift includes frontend-only request fields, backend-only fields, response properties that the backend never emits, inability to distinguish omitted properties from explicit `null`, silent success for missing update targets, and inconsistent clearing behavior caused by a mixture of `COALESCE` and unconditional `NULL` writes.

The Scheduling Configuration page presents one **บันทึก** action, implemented as `saveAll()`, but sends independent requests through `Promise.all`. A failure can therefore leave only part of the visible configuration committed. **บันทึกและจัดอัตโนมัติ** calls the same save function before starting a scheduling job, and the current save function catches its own error, so scheduling can continue after a failed or partially successful save.

## Decisions

- Limit this batch to Scheduling Constraints configuration. Auto-scheduling jobs, locked slots, instructor-preference legacy routes, timetable templates, and timetable mutations remain separate batches.
- Keep the six read routes.
- Remove the six existing mutation routes without a compatibility or deprecation period. The controlled frontend in this repository moves in the same change.
- Add one typed `PUT /api/academic/scheduling/configuration` endpoint.
- Send only changed sections and rows. Omitted means unchanged, explicit `null` resets or clears a field, and a concrete value sets it.
- Validate the complete patch before writing and commit all changes in one PostgreSQL transaction.
- Keep the existing `ACADEMIC_COURSE_PLAN_READ_ALL` and `ACADEMIC_COURSE_PLAN_MANAGE_ALL` permissions. No permission or schema migration is required.

## Approaches Considered

### 1. Typed aggregate patch in a Rust service

Define named request, row-patch, view, and result DTOs; validate all references and domain rules in the service; use bulk SQL per section inside one transaction; and generate all frontend wire types from Rust. This is the selected approach because it follows the existing service boundary, keeps compile-time contracts, supports focused tests, and gives the visible save action atomic semantics.

### 2. Compose the existing mutation functions in the handler

Retain the old request DTOs and call the old service functions from one handler. This is initially smaller, but the functions accept `&PgPool`, start their own transactions, and contain inconsistent update semantics. Making the handler coordinate them would either lose atomicity or move business logic into the handler. Rejected.

### 3. PostgreSQL function accepting JSONB

Pass the whole patch to a database function that validates and writes every section. This is atomic but moves domain behavior into an applied migration, weakens Rust/OpenAPI type ownership, and is harder to unit test and evolve. Rejected.

## API Inventory

| Method | Path | Operation ID | Permission |
|---|---|---|---|
| GET | `/api/academic/scheduling/instructors` | `listSchedulingInstructorConstraints` | `academic_course_plan.read.all` |
| GET | `/api/academic/scheduling/subjects` | `listSchedulingSubjectConstraints` | `academic_course_plan.read.all` |
| GET | `/api/academic/scheduling/settings` | `getSchedulingSettings` | `academic_course_plan.read.all` |
| GET | `/api/academic/scheduling/classroom-courses` | `listSchedulingClassroomCourseConstraints` | `academic_course_plan.read.all` |
| GET | `/api/academic/scheduling/classroom-courses/{id}/rooms` | `listSchedulingClassroomCoursePreferredRooms` | `academic_course_plan.read.all` |
| GET | `/api/academic/scheduling/rooms` | `listSchedulingRooms` | `academic_course_plan.read.all` |
| PUT | `/api/academic/scheduling/configuration` | `saveSchedulingConfiguration` | `academic_course_plan.manage.all` |

The following mutation routes are removed:

- `PUT /api/academic/scheduling/instructors/order`
- `PUT /api/academic/scheduling/instructors/{id}`
- `PUT /api/academic/scheduling/subjects/{id}`
- `PUT /api/academic/scheduling/settings`
- `PUT /api/academic/scheduling/classroom-courses/{id}`
- `PUT /api/academic/scheduling/classroom-courses/{id}/rooms`

The checkpoint after this batch is 184 unique operations. SSE, WebSocket, health/readiness, file/binary endpoints, scheduling jobs, locked slots, and timetable mutation routes remain outside this batch.

## Request and Response Contract

`SaveSchedulingConfigurationRequest` is a sparse aggregate patch with these sections:

- `scheduler_settings`: optional global scheduler settings patch;
- `instructor_order`: optional ordered instructor ID list;
- `instructors`: zero or more `InstructorConstraintPatch` rows;
- `subjects`: zero or more `SubjectConstraintPatch` rows;
- `classroom_courses`: zero or more `ClassroomCourseConstraintPatch` rows;
- `preferred_rooms`: zero or more complete room-list replacements for individual classroom courses.

Every collection defaults to empty when omitted. An entirely empty request is a successful no-op.

Patchable fields use a tri-state representation at the Rust boundary:

- missing property: `Unchanged`;
- explicit JSON `null`: `Clear` or reset to the documented domain default;
- concrete JSON value: `Set(value)`.

The generated OpenAPI and TypeScript schemas expose these fields as optional and nullable. The service normalizes clearing consistently:

- unavailable/preferred slot arrays become `[]`;
- an assigned instructor room is deleted;
- optional subject restrictions and a classroom-course consecutive pattern become SQL `NULL`;
- instructor priority resets to `100`;
- `default_max_consecutive` resets to `4`;
- non-null booleans reset to their documented default;
- a preferred-room replacement with `rooms: []` removes every preferred room for that classroom course.

`InstructorConstraintPatch` does not carry a second per-row priority value. Ordering is owned only by `instructor_order`, avoiding two conflicting priority sources in the same request.

The endpoint returns `ApiResponse<SchedulingConfigurationSaveResult>`. The result contains a top-level `changed` flag and per-section affected counts for scheduler settings, instructor order, instructor constraints, subject constraints, classroom-course constraints, and preferred-room sets. Counts represent state changes, not merely payload length. This gives the frontend a typed outcome without requiring a broad reload.

## Backend Design

### Handler boundary

All seven handlers resolve exactly one `actor_tenant_context`. Reads require `ACADEMIC_COURSE_PLAN_READ_ALL`; the aggregate mutation requires `ACADEMIC_COURSE_PLAN_MANAGE_ALL`. JSON rejection from the aggregate body is mapped into the standard `400` error envelope. Handlers own no SQL, database rows, validation, or transactions.

Read response DTOs and query DTOs move to the academic scheduling model boundary and derive `Serialize`, `Deserialize` where needed, and `ToSchema`. DB-facing row structs remain private to the service.

### Validation and locking

The aggregate service opens a transaction, resolves the active academic year, and locks that year row before validating or writing. Every aggregate save therefore serializes against other aggregate saves for the active year.

Validation happens before the first mutation:

- reject duplicate instructor, subject, classroom-course, room, or rank keys within a section;
- every instructor must be an active staff user;
- every subject and room must be active;
- every classroom course must belong to the active academic year;
- every period ID must belong to the active academic year;
- day codes must be one of `MON` through `SUN`;
- numeric limits, priorities, pattern members, and ranks must stay within documented bounds;
- each consecutive pattern must sum to the effective periods per week, including the same fallback calculation used by the read query;
- references missing from any section produce `404` before any write occurs.

Malformed JSON, duplicates, invalid ranges, invalid day codes, and inconsistent patterns return `400`. Missing active year or references return `404`. Expected integrity or concurrent-write conflicts return `409`. Unexpected failures return `500` without exposing SQL, credentials, or raw request bodies.

### Writes

After validation, each same-kind section uses one bulk update, insert, upsert, or delete where practical. Preferred-room replacements delete all rows for the affected classroom-course IDs in one statement and insert all replacement rows in one bulk statement. Instructor-room replacements are likewise normalized and written inside the same transaction. Statements avoid rewriting identical values where PostgreSQL `IS DISTINCT FROM` can express the comparison, allowing affected counts to reflect actual state changes.

Any error rolls back settings, order, instructor, subject, classroom-course, and room changes together. No schema migration is required; this change only uses the current tables and constraints.

## Frontend Design

`frontend-school/src/lib/api/scheduling.ts` aliases the generated scheduling view, request, and result schemas. The six old mutation wrappers and handwritten wire DTOs are removed. One `saveSchedulingConfiguration` wrapper calls the new aggregate endpoint.

The Scheduling Configuration page continues to keep UI edit state locally. `saveAll()` constructs a sparse request from dirty sections and sends exactly one mutation request. It applies saved local state and clears dirty flags only after the response succeeds. A no-op response is treated as success.

Save failures propagate to the caller instead of being swallowed. **บันทึกและจัดอัตโนมัติ** starts an auto-scheduling job only after the aggregate save transaction commits successfully. The page layout and visible controls do not change.

## Testing

Development follows red-green-refactor:

1. Add failing static tests for the seven route/permission mappings, absence of the six removed mutation routes, generated DTO ownership, and the 184-operation checkpoint.
2. Add request-deserialization tests for missing/null/value patch behavior and malformed JSON mapping.
3. Add database-backed service tests for all-or-nothing rollback, missing targets, active-year scoping, duplicate keys, reference validation, day/period validation, range validation, clearing defaults, effective periods-per-week validation, and accurate affected counts.
4. Add frontend static tests that reject the six old mutation wrappers and handwritten scheduling wire DTOs, require one aggregate request, and require auto-scheduling to stop after a save failure.
5. Generate OpenAPI and TypeScript only after the contract tests fail for missing operations.
6. Run Rust format, Clippy, backend static architecture tests, full database-backed backend tests, generated API and permission checks, frontend static tests, the Svelte autofixer for the changed page, and `svelte-check`.

Final audit confirms 184 unique operation IDs, no applied migration changes, no raw permission literals in production code, no plaintext PII/secrets in logs, a clean diff, and matching frontend/backend permissions.

## Out of Scope

- Auto-scheduling job, job-history, and undo contracts
- Timetable locked-slot contracts
- Legacy direct instructor-preference and instructor-room-assignment contracts
- Timetable template and timetable mutation contracts
- Scheduler algorithm changes
- Database schema or migration changes
- New permissions or organization scopes
- UI redesign
