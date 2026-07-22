# Auto-Scheduler Removal Design

**Date:** 2026-07-23  
**Status:** Design approved; pending written-spec review  
**Decision owner:** Project owner

## Context

School timetables have many real-world exceptions that are not consistently represented in the system. The current auto-scheduler can therefore produce a technically valid result that is operationally unsuitable, and its direct-write behavior creates a risk that users trust or must repair an unsuitable schedule.

The project will remove the complete auto-scheduling feature instead of hiding it or keeping configuration that has no runtime consumer. Manual timetable editing remains the supported workflow.

## Decision and alternatives

Three approaches were considered:

1. Hide only the frontend. Rejected because callable backend routes, job data, and unused code would remain.
2. Remove jobs and the engine but keep Scheduling Configuration. Rejected because the manual timetable service does not consume those constraints, so the UI would imply behavior that does not exist.
3. Remove the entire auto-scheduler feature. Selected because it leaves one honest timetable workflow, removes dead code and schema, and avoids maintaining an unsafe feature for possible future use.

This is an intentional breaking removal. Old frontend URLs and backend routes will return 404 rather than redirecting or preserving compatibility shims.

## Goals

- Remove Auto Schedule, scheduling jobs, undo, locked-slot automation, and Scheduling Configuration from backend, frontend, API contracts, database schema, tests, and active documentation.
- Preserve every existing row in `academic_timetable_entries`, regardless of whether it was created manually or by an auto-scheduler job.
- Preserve manual timetable viewing and editing, timetable realtime updates, timetable templates, course planning, subjects, instructors, rooms, periods, semesters, and academic years.
- Remove scheduler-only data and constraints so future developers and AI agents do not assume that an auto-scheduler remains supported.
- Keep the repository warning-free and all remaining API, architecture, migration, backend, and frontend checks passing.

## Non-goals

- Replacing Auto Schedule with another algorithm or an AI scheduler.
- Adding a suggestion or preview mode.
- Adding scheduler constraints to the manual timetable validator.
- Changing the manual timetable data model or its permissions.
- Removing `ACADEMIC_COURSE_PLAN_READ_ALL` or `ACADEMIC_COURSE_PLAN_MANAGE_ALL`; other course-planning and timetable features still use them.
- Deleting any existing timetable entry.

## Data-safety invariant

The removal migration must not execute `DELETE`, `TRUNCATE`, or a table replacement against `academic_timetable_entries`.

The migration will remove only the `scheduler_job_id` provenance column from `academic_timetable_entries`. PostgreSQL drops the column, its index, and its foreign-key dependency without deleting the containing timetable rows. Entries that were previously associated with a scheduler job become ordinary timetable entries and remain editable through the manual workflow.

Tests and review must reject any implementation that filters or deletes timetable entries by `scheduler_job_id`.

## Backend removal

### HTTP surface

Remove all routes and handlers for:

- `POST /api/academic/scheduling/auto-schedule`
- `GET /api/academic/scheduling/jobs`
- `GET /api/academic/scheduling/jobs/{id}`
- `POST /api/academic/scheduling/jobs/{id}/undo`
- `POST /api/academic/instructor-preferences`
- `POST /api/academic/instructor-rooms`
- `GET|POST /api/academic/timetable/locked-slots`
- `DELETE /api/academic/timetable/locked-slots/{id}`
- the six Scheduling Configuration read routes under `/api/academic/scheduling/*`
- `PUT /api/academic/scheduling/configuration`

The instructor-preference, instructor-room, and locked-slot routes are included because they are scheduler-only support surfaces. The aggregate Scheduling Configuration API is included because its stored values are consumed only by the removed engine.

### Modules

Delete the scheduler-only handlers, models, services, data loader, algorithms, validators, quality calculator, and focused tests:

- `handlers/scheduling.rs`
- `handlers/scheduling_config.rs`
- `models/scheduling.rs`
- `models/scheduling_config.rs`
- `services/scheduler.rs` and `services/scheduler/`
- `services/scheduler_data.rs`
- `services/scheduling_service.rs`
- `services/scheduling_config_service.rs`
- `services/scheduling_config_service_tests.rs`

Remove their module declarations and re-exports. Do not modify manual timetable, timetable realtime, timetable template, course-planning, or exam-scheduling services except to remove stale imports or references.

## Database migration

Add a new immutable migration, `backend-school/migrations/028_remove_auto_scheduler.sql`. Do not edit `001_baseline.sql` or any existing applied migration.

The migration will:

1. Drop `academic_timetable_entries.scheduler_job_id`. Its scheduler-job index and foreign key disappear with the column.
2. Drop scheduler-only tables:
   - `timetable_scheduling_jobs`
   - `timetable_locked_slots`
   - `scheduler_settings`
   - `classroom_course_preferred_rooms`
   - `instructor_preferences`
   - `instructor_room_assignments`
3. Drop scheduler-only columns from `classroom_courses`:
   - `consecutive_pattern`
   - `same_day_unique`
   - `hard_unavailable_slots`
4. Drop scheduler-only columns from `subjects`:
   - `min_consecutive_periods`
   - `max_consecutive_periods`
   - `allow_single_period`
   - `allowed_period_ids`
   - `allowed_days`
5. Drop the now-unused `validate_allowed_days(jsonb)` function after its subject constraint is removed with the column.
6. Drop the now-unused `scheduling_algorithm` and `scheduling_status` enum types after the job table is gone.

Core scheduling inputs such as `subjects.periods_per_week`, academic periods, rooms, instructor/course assignments, and timetable entries remain.

The migration is intentionally destructive for scheduler configuration and job history. A tenant database backup is required before deployment because rolling back the code alone cannot restore removed scheduler metadata. Manual timetable entries do not require restoration because the migration preserves them.

## API contract

Remove the seven Scheduling Configuration operations and their scheduler-only schemas from the Rust `utoipa` source, generated OpenAPI JSON, and generated TypeScript definitions.

The generated contract moves from 184 to 177 unique operation IDs. Auto-scheduling job routes were never in the generated OpenAPI checkpoint, so they do not reduce this count further.

Contract generation remains offline and deterministic. Router-derived architecture guards must verify that all remaining documented operations still match registered handlers.

## Frontend removal

Delete:

- `src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte`
- `src/routes/(app)/staff/academic/timetable/scheduling-config/+page.ts`
- `src/routes/(app)/staff/academic/timetable/scheduling/jobs/+page.svelte`
- `src/routes/(app)/staff/academic/timetable/scheduling/jobs/[jobId]/+page.svelte`

Remove the Auto Schedule navigation/button from the manual timetable page. Remove scheduler job, undo, configuration, instructor-preference, room-assignment, and locked-slot types and API wrappers from `src/lib/api/scheduling.ts`. Preserve any timetable-template functions still used from that module.

Remove route metadata, static-test inventories, loading-state expectations, mobile drag-and-drop opt-ins, and layout expectations that name the deleted pages. No hidden button, direct URL, polling timer, or client API call may remain.

## Documentation cleanup

- Remove `docs/plans/AUTO_SCHEDULER_REDESIGN.md`.
- Remove the superseded Scheduling Configuration design and implementation-plan documents dated 2026-07-22; Git history remains the historical record.
- Update `.rules`, `IMPROVEMENT_PLAN.md`, `docs/TESTING.md`, and `docs/backend-school/API_DEVELOPMENT.md` to record the removal and the 177-operation checkpoint.
- Ensure active documentation identifies manual timetable editing as the only supported timetable construction workflow.

## Verification

### Migration and data safety

- Add an architecture/migration guard that reads migration 028 and rejects `DELETE FROM academic_timetable_entries`, `TRUNCATE academic_timetable_entries`, or equivalent destructive replacement.
- Apply all migrations to a clean sandbox database.
- Verify `academic_timetable_entries` remains present and no longer has `scheduler_job_id`.
- Insert and read a manual timetable entry after migration 028.
- For a pre-migration fixture or database copy, record the timetable-entry count before migration 028 and assert the same count afterward.
- Verify scheduler-only tables, columns, functions, and enum types no longer exist.

### Backend and contracts

- Assert the removed routes and module declarations are absent.
- Assert the OpenAPI document contains 177 unique operation IDs and no scheduler-configuration paths or schemas.
- Run formatting, Clippy with warnings denied, all backend targets, database-backed tests, and static architecture tests.

### Frontend

- Add a static removal guard covering deleted URLs, links, API wrappers, types, and polling/undo behavior.
- Regenerate API types and run contract checks.
- Run all frontend static tests, the Svelte autofixer on any modified surviving component, and `svelte-check` with required public environment variables.

### Repository

- Confirm existing migration files are byte-for-byte unchanged and only migration 028 is added.
- Run `git diff --check` and verify the worktree is clean after commits.

## Deployment and rollback

Before deployment, back up every tenant database whose scheduler configuration or job history might need archival. Ensure no auto-scheduler job is expected to survive the deployment.

The new backend no longer queries scheduler schema, so it can start after migration 028 applies. Because this project currently operates a single backend instance, the code and migration can ship together. If deployment changes to overlapping old/new replicas, first deploy a release that removes route access and stops jobs, drain old replicas, and only then deploy migration 028.

Rollback after migration 028 is a database restore or a forward migration that recreates the schema; rolling back only the application binary is unsupported. This limitation affects scheduler metadata, not preserved timetable entries.

## Acceptance criteria

- Manual timetable pages and APIs still work with all pre-existing timetable entries.
- No Auto Schedule, Scheduling Configuration, job history, progress, or undo UI remains.
- Removed backend routes return 404.
- Scheduler engine and support modules are absent from the compiled backend.
- Scheduler-only database objects are absent after migration 028.
- `academic_timetable_entries` contains the same rows across migration 028 and remains manually editable.
- OpenAPI contains exactly 177 unique operations and generated TypeScript is current.
- Backend, frontend, contract, architecture, and migration verification all pass without warnings.
