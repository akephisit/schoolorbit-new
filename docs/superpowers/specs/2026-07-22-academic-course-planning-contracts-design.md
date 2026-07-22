# Academic Course Planning Contracts Design

**Date:** 2026-07-22

**Status:** Approved for implementation

**Scope:** Move the existing Course Planning and Teaching Assignment JSON APIs into the generated Rust/OpenAPI/TypeScript contract while correcting verified authorization, validation, missing-target, and synchronization behavior in the same bounded batch.

## Context

The generated school API currently contains 165 unique operations. Activity Template and Activity Workspace already use Rust serde DTOs and `utoipa` metadata as the source of truth, with deterministic OpenAPI and TypeScript generation plus route drift guards.

Course Planning is the next adjacent workflow. It currently exposes 12 operations from `handlers/course_planning.rs`. The frontend still owns duplicate `ClassroomCourse`, settings, and `CourseInstructor` wire types, and `listClassroomCourses` supports both an old positional signature and a newer object signature. Several service paths also treat missing rows as successful no-ops, accept unbounded instructor roles until PostgreSQL rejects them, silently discard malformed query IDs, or let a direct primary-instructor update drift from the team-teaching junction and existing timetable assignments.

## Approaches Considered

### 1. Contract metadata only

Add `ToSchema`/`utoipa::path`, generate artifacts, and leave runtime behavior unchanged. This is the smallest diff, but it would formalize incorrect `200` responses and unstable string roles. Rejected because generated contracts should describe a reliable API, not preserve verified defects.

### 2. Contract rollout with a bounded correctness audit

Document all 12 routes, add focused service/architecture tests first, correct missing-target and validation behavior, synchronize primary instructor mutations transactionally, and move frontend wire DTOs to generated types. This follows the established Activity Workspace rollout and is the selected approach.

### 3. Merge Course Planning, Scheduling Configuration, and Timetable mutations

Convert the entire scheduling surface in one batch. This could align every related DTO at once, but the resulting route count, asynchronous scheduling jobs, WebSocket side effects, and timetable conflict models would make review and rollback unnecessarily difficult. Deferred to later batches.

## API Inventory

| Method | Path | Operation ID | Permission |
|---|---|---|---|
| GET | `/api/academic/planning/courses` | `listClassroomCourses` | `academic_course_plan.read.all` |
| POST | `/api/academic/planning/courses` | `assignCourses` | `academic_course_plan.manage.all` |
| PUT | `/api/academic/planning/courses/{id}` | `updateClassroomCourse` | `academic_course_plan.manage.all` |
| DELETE | `/api/academic/planning/courses/{id}` | `removeClassroomCourse` | `academic_course_plan.manage.all` |
| POST | `/api/academic/planning/courses/instructors/batch` | `batchListCourseInstructors` | `academic_course_plan.read.all` |
| GET | `/api/academic/planning/courses/instructors` | `batchListCourseInstructorsFromQuery` | `academic_course_plan.read.all` |
| GET | `/api/academic/planning/courses/{id}/instructors` | `listCourseInstructors` | `academic_course_plan.read.all` |
| POST | `/api/academic/planning/courses/{id}/instructors` | `addCourseInstructor` | `academic_course_plan.manage.all` |
| PUT | `/api/academic/planning/courses/{id}/instructors/{uid}` | `updateCourseInstructorRole` | `academic_course_plan.manage.all` |
| DELETE | `/api/academic/planning/courses/{id}/instructors/{uid}` | `removeCourseInstructor` | `academic_course_plan.manage.all` |
| GET | `/api/academic/planning/classrooms/{classroom_id}/activities` | `listClassroomActivities` | `academic_course_plan.read.all` |
| DELETE | `/api/academic/planning/classrooms/{classroom_id}/activities/{slot_id}` | `removeClassroomFromActivitySlot` | `academic_course_plan.manage.all` |

The checkpoint after this batch is 177 unique operations. SSE, WebSocket, health/readiness, and file/binary routes remain outside the OpenAPI document.

## Backend Design

### Typed API boundary

`models/course_planning.rs` owns query, request, response, and bounded role schemas. Existing snake_case JSON remains unchanged. Flexible `classroom_courses.settings` stays a JSON object because the stored settings are intentionally open-ended; it is the exception permitted for flexible configuration, not a new known-shape JSONB contract.

The primary instructor patch must distinguish an omitted property from explicit JSON `null`:

- omitted: leave the current primary instructor unchanged;
- UUID: assign that instructor as primary;
- `null`: remove the current primary assignment.

The OpenAPI schema documents the field as optional and nullable. Instructor roles are limited to `primary` and `secondary` at the API boundary and validated again in the service before SQL.

### Service behavior

Handlers remain limited to request context, permission enforcement, service calls, envelopes, and WebSocket notification. Services own all existence checks, row-count decisions, validation, transactions, and DB-facing rows.

- Course assignment validates the classroom, semester, and every distinct subject before writing. Duplicate subject IDs are normalized. The returned count is the number of newly inserted classroom courses.
- Course update/delete return `404` when the course does not exist.
- Explicit primary-instructor changes validate the user and update the course team plus existing timetable-entry instructors in one transaction. Clearing the primary removes that primary assignment and its derived timetable-entry assignments; omitting the field leaves it unchanged.
- Instructor list on a missing course returns `404`. Add validates course, instructor, and role. Remove/update return `404` for a missing assignment, and updating a missing assignment cannot demote the current primary.
- The comma-separated GET batch query rejects any malformed UUID with `400` rather than silently dropping it. Empty batch input returns an empty map.
- Classroom activity listing validates both classroom and semester before returning an empty list. Removing a missing classroom-slot assignment returns `404`.

Database failures remain `500` without exposing raw SQL or credentials. Expected client errors are `400`, `401`, `403`, `404`, and `409` only where a real domain conflict can occur.

### Authorization

The existing frontend and backend permission model already agrees:

- all reads require `ACADEMIC_COURSE_PLAN_READ_ALL`;
- all mutations require `ACADEMIC_COURSE_PLAN_MANAGE_ALL`.

Each handler resolves exactly one `actor_tenant_context`. No new permission code or migration is required. Static architecture tests lock these mappings and keep SQL out of handlers.

## Frontend Design

`frontend-school/src/lib/api/academic.ts` aliases generated request/response schemas instead of redeclaring wire DTOs. API wrapper names and URLs remain stable. `listClassroomCourses` uses one object parameter; the remaining positional call site is migrated so the compatibility union can be removed.

UI-specific fallback handling remains in Svelte components where generated nullable fields need presentation defaults. No visual redesign is included. Any changed `.svelte` file must be processed by the Svelte autofixer and pass `svelte-check`.

## Testing

Development follows red-green-refactor:

1. Add failing static tests for the 12 route/permission mappings and the 177-operation checkpoint.
2. Add focused service tests for role parsing, batch/query normalization, missing parents/targets, assignment counts, and primary-instructor synchronization.
3. Add a failing frontend contract test that rejects handwritten Course Planning wire DTOs and requires generated aliases.
4. Generate OpenAPI and TypeScript only after backend metadata tests are red for missing operations.
5. Run Rust format, Clippy, static architecture tests, full database-backed backend tests, generated contract/permission checks, all frontend static tests, Svelte autofixer for changed components, and `svelte-check`.

Final audit confirms 177 unique operation IDs, no applied migration changes, no plaintext PII/secrets in logs, a clean diff, and matching frontend/backend permissions.

## Out of Scope

- Scheduling configuration and scheduler-job contracts
- Timetable mutation/conflict contracts
- WebSocket payload generation
- Database schema or migration changes
- New permissions or organization scopes
- UI redesign
