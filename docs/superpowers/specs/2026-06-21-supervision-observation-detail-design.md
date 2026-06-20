# Supervision Observation Detail Design

## Goal

Add a dedicated teaching supervision observation detail page so staff can inspect one observation, edit safe fields, manage evaluators, and cancel an observation without overloading the main supervision workspace.

The main workspace at `/staff/academic/supervision` remains the entry point for booking, request approval, evaluation, cycles, templates, and reports. The new detail page handles one observation at a time.

## Route And Navigation

- Add `/staff/academic/supervision/[id]` as a guard-only route.
- Do not create a sidebar menu record for the detail route.
- Link to the detail page from observation cards/tables in:
  - `รายการของฉัน`
  - `คำขอจองที่รออนุมัติ`
  - `รายการที่ได้รับมอบหมายให้ประเมิน`
  - `รายงานความคืบหน้ารอบนิเทศ`
- The detail page must use the same module/read permissions as the parent workspace, while action controls are shown only when exact capability permissions pass.

## Detail Page Layout

The page should use `PageShell` and local shadcn-svelte primitives.

Sections:

1. Header summary
   - observed teacher
   - current status
   - cycle
   - observed date and time
   - primary actions allowed for the current actor

2. Lesson details
   - subject
   - period
   - classroom/level
   - room
   - timetable entry or manual lesson source

3. Evaluators
   - all assigned evaluators
   - evaluator status and submitted state when available
   - edit evaluator action for users with `supervision.manage.*`

4. Evaluation and result
   - template title
   - score summary
   - evaluator submission state
   - link/open evaluation form when current user is an assigned evaluator

5. Timeline
   - requested, updated, planned, returned, evaluated, submitted, approved, published, acknowledged, cancelled actions when present
   - actor and timestamp when available

## Edit Rules

Editing is status-aware.

- The observed teacher may edit or cancel their own observation only while status is `requested`.
- Managers with `supervision.manage.organization_unit`, `supervision.manage.organization_tree`, or `supervision.manage.school` may edit lesson fields and evaluators while status is `requested`, `planned`, or `returned`.
- Managers may cancel observations that have already entered workflow, but cancellation must be a status transition to `cancelled`, not a hard delete.
- Hard delete is not part of the user-facing workflow.
- If any evaluator has submitted responses, those submitted responses must not be deleted silently.
- Adding evaluators after approval is allowed when status is still manageable.
- Removing evaluators is allowed only for evaluators who have not submitted responses. Submitted evaluator rows should be kept for audit/result integrity.

## Backend API Changes

Existing endpoints already support observed-teacher request edits:

- `PATCH /api/supervision/observations/{id}/request`
- `DELETE /api/supervision/observations/{id}/request`

Add manager endpoints for detail-page actions:

- `PATCH /api/supervision/observations/{id}`
  - Edits lesson/date/time/template fields when current status allows it.
  - Returns `ApiResponse<SupervisionObservation>`.

- `PUT /api/supervision/observations/{id}/evaluators`
  - Bulk validates and upserts evaluator assignments.
  - Keeps submitted evaluator rows.
  - Returns `ApiResponse<SupervisionObservation>`.

- `POST /api/supervision/observations/{id}/cancel`
  - Cancels with an optional reason.
  - Records an action timeline entry.
  - Returns `ApiResponse<SupervisionObservation>`.

These endpoints must use `actor_tenant_context`, supervision policy helpers, typed DTOs, and service-layer logic. Do not add database logic to handlers.

## Frontend State And UX

- The detail page loads the observation by id using `getSupervisionObservation`.
- Mutations return the updated `SupervisionObservation` and update only local page state.
- The parent workspace may optimistically patch local state when a detail action is performed there, but it should not reload the whole workspace for normal observation edits.
- Use `LoadingButton` or action-specific loading state for every mutation.
- Use dialogs/drawers for edit forms:
  - edit lesson/date/time
  - edit evaluators
  - cancel observation
- Destructive cancellation requires a confirmation dialog and reason textarea.

## Testing

Frontend static tests should cover:

- detail route has guard-only access metadata and no sidebar menu metadata
- parent workspace links observation rows/cards to `/staff/academic/supervision/[id]`
- detail page uses shared layout/state components and shadcn-svelte primitives
- mutation handlers patch local observation state instead of broad reloads

Backend tests should cover:

- manager edit is allowed for manageable statuses
- manager edit is rejected after submitted/approved/published/completed states
- observed teacher can edit/cancel only their own requested observation
- evaluator replacement preserves submitted evaluator rows
- cancel endpoint transitions to `cancelled` and records an action

## Rollout Order

1. Backend DTOs, services, policy checks, routes, and focused tests.
2. Frontend API client functions and types.
3. Frontend detail route and parent workspace links.
4. Static tests and focused checks.
5. Manual sandbox verification for edit evaluator, edit lesson/date, cancel, and read-only views.
