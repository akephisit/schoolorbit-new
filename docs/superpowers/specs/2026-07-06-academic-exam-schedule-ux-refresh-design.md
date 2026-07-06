# Academic Exam Schedule UX Refresh Design

## Context

The academic exam schedule module now supports exam rounds, exam days, room assignments, timeline placement, publishing, and student/parent published views. The first implementation works, but the staff scheduling workspace needs a more operational layout:

- The readiness card consumes too much horizontal space on the schedule screen.
- Room assignment currently mixes room setup and invigilator assignment in one flow.
- Staff need a later invigilator workflow with workload visibility.
- Timeline drag/drop needs a clearer preview of the actual exam duration.
- Scheduled sessions need a clear way to become unscheduled again.
- Forms should use shadcn-svelte primitives consistently and keep save actions visible.

This design refresh keeps the existing domain model where possible. It focuses on improving the staff workspace without changing the published student/parent schedule contract.

## Goals

- Make `Schedule` the largest and most efficient workspace.
- Move readiness details out of the large right-side card.
- Split invigilator assignment into a separate, later flow.
- Show invigilator workload across the whole round and per day.
- Calculate workload from actual scheduled exam sessions only, excluding gaps.
- Prevent a staff member from supervising overlapping exam sessions in different rooms.
- Add visual drag previews that show the true session duration before drop.
- Let staff remove a scheduled session and return it to the unscheduled tray.
- Use shadcn-svelte primitives consistently in the exam schedule workspace.
- Use semantic button/status colors for scanability.

## Non-Goals

- Do not show invigilators in student or parent published schedules.
- Do not make invigilator assignment a publish blocker.
- Do not redesign unrelated academic modules.
- Do not edit migration `019_academic_exam_schedule.sql`; add new migrations only if schema changes are required.
- Do not introduce a bulk draft save model for timeline placement in this iteration.

## User Decisions

- Invigilators are assigned after rooms and schedule setup.
- Invigilators are assigned per exam day room assignment, not per individual subject session.
- A staff member's workload is counted from actual exam session durations in assigned rooms.
- Workload excludes gaps between sessions.
- Users can view both whole-round workload and per-day workload.
- Unscheduling supports both a dialog action and dragging the block back to the unscheduled tray.
- Timeline placement remains auto-save per action.
- Invigilator assignment does not affect publish readiness.
- A staff member cannot be assigned to overlapping live exam session times across rooms.
- Recommended invigilator count is two per room/day, but it is not a hard requirement.

## Workspace Flow

The staff exam schedule detail page will have four main tabs:

1. `Setup`
   - Manage exam days, day start/end times, grade scope, and blocked windows.
2. `Rooms`
   - Assign each classroom to an exam room for each exam day.
   - Generate seats.
   - Do not assign invigilators in this tab.
3. `Schedule`
   - Main timeline workspace.
   - Drag/drop imported exam items onto day/classroom rows.
   - Move and unschedule sessions.
4. `Invigilators`
   - Assign staff to day room assignments.
   - Show workload summaries and conflicts.

The previous `Review` area becomes a compact readiness/status detail rather than a full workspace tab.

## Schedule-First Layout

The detail page will remove the large right-side readiness card from the main grid. In its place, the top of the workspace will render a compact status bar:

- Round status: draft/published.
- Readiness: ready/not ready.
- Unscheduled item count.
- Room assignment count.
- Invigilator assignment coverage count.
- Conflict or blocker count.
- Button to open readiness details.

Readiness blockers open in the shared shadcn `Sheet` added by this work. This keeps the timeline full-width while still preserving access to publish diagnostics.

The `Schedule` tab should use the full available page width. The unscheduled tray remains near the timeline, but the layout should prioritize the timeline. If the tray consumes too much space on smaller screens, it may become collapsible or move above the timeline.

## Forms And Save Placement

Room and invigilator editing will use right-side shadcn `Sheet` panels. The project does not currently expose a shared `sheet` primitive, so this work must add `frontend-school/src/lib/components/ui/sheet/` following the existing shadcn-svelte component pattern before using sheets in feature code.

- The table/list remains visible in the background.
- The save button is sticky at the bottom of the sheet.
- Cancel/close remains secondary.
- Destructive actions use red styling and confirmation where appropriate.

This avoids the current problem where users need to scroll up/down to find the save action.

## Semantic UI Colors

Use semantic colors consistently in the exam schedule workspace:

- Green: publish, ready, success, generated/completed.
- Blue: primary save/edit/place actions.
- Yellow/amber: warning, incomplete, readiness blockers.
- Red: delete, remove from schedule, destructive actions.
- Gray/outline: refresh, view details, cancel, secondary actions.

All buttons and status indicators should still use the local shadcn-svelte primitives and project variants where available. If a missing primitive or variant is needed, add it to the shared UI layer instead of hand-rolling one-off controls.

## Timeline Placement

Drag/drop placement will show a duration-aware ghost preview:

- The ghost width equals `durationMinutes`.
- The ghost snaps to 15-minute increments.
- The label shows start and end time, for example `08:30-09:30`.
- Valid placement renders in blue/green.
- Invalid placement renders in red and exposes a short reason.
- Invalid reasons include outside day window, blocked window overlap, same-classroom conflict, same-room conflict, wrong row, and off-slot time.

Scheduled sessions remain draggable. During auto-save, only the affected item/session should show a loading state. The rest of the timeline remains usable when safe.

## Unscheduling

Users can remove a scheduled session in two ways:

- Open the session dialog and choose `เอาออกจากตาราง`.
- Drag the scheduled block back to the unscheduled tray.

Unscheduling should call a backend endpoint that deletes the `academic_exam_sessions` row. The returned workspace state should move the item back into `unscheduledItems` after refresh or local patching.

The action is auto-saved immediately, matching current placement behavior.

## Invigilator Workflow

The `Invigilators` tab uses a room-first workflow:

- Select an exam day.
- Show a table of room assignments for that day.
- Each row shows classroom, room, assigned invigilators, invigilator count, and total supervision time for that room/day.
- `จัดกรรมการ` opens a right-side sheet.
- The sheet uses a searchable staff picker with checkboxes or a combobox-style multi-select.
- The UI recommends two invigilators per room/day but does not block lower or higher counts.

Invigilators are assigned to the existing day room assignment. The assignment applies to all scheduled sessions for that classroom on that exam day.

## Workload Calculation

Workload is calculated from actual scheduled sessions:

- For each day room assignment, collect scheduled sessions for the same exam day and classroom.
- Sum `endsAt - startsAt` for each session.
- Exclude gaps between sessions.
- Attribute that total duration to each invigilator assigned to the day room assignment.

Example:

- 08:30-09:30 plus 10:00-11:30 equals 2 hours 30 minutes.
- The 09:30-10:00 gap is not counted.

The workspace exposes:

- Whole-round workload per staff member: total minutes, assigned days, assigned room-day count.
- Per-day workload per staff member: total minutes and room-day count for the selected day.
- Conflict details when a staff member is assigned to overlapping live session ranges.

## Conflict Validation

Invigilator conflicts are based on actual scheduled session ranges:

- A staff member may supervise multiple room assignments on the same day if their scheduled session times do not overlap.
- A staff member cannot supervise two rooms with overlapping scheduled sessions.
- Validation occurs on save.
- The UI should pre-warn or disable conflicting staff options when it can compute the conflict from current workspace data.
- The backend remains authoritative and returns a clear bad request if a conflict is detected.

Because invigilator assignment is not a publish blocker, missing invigilators show as informational coverage in the status bar and invigilator tab, not as publish readiness blockers.

## Backend Shape

Existing tables already support day room assignments and invigilator rows. The preferred backend design is to reuse them unless database-level conflict prevention requires an additional migration.

Service/API changes:

- Add a dedicated invigilator assignment API or split service method from room assignment.
- Stop requiring the room assignment form to provide `invigilatorStaffIds`.
- Keep old request compatibility only if useful for deployed clients, but the new UI should not send invigilators through room assignment.
- Add invigilator workload data to the workspace response or expose it through a dedicated endpoint.
- Add an unschedule endpoint for scheduled sessions.
- Add backend validation for overlapping invigilator session ranges.

Potential endpoints:

- `PUT /api/academic/exam-schedules/day-room-assignments/{assignmentId}/invigilators`
- `GET /api/academic/exam-schedules/{roundId}/invigilators`
- `DELETE /api/academic/exam-schedules/sessions/{sessionId}`

Exact route names can follow the existing handler style if a shorter route fits better.

## Frontend Components

Expected new or changed components:

- `CompactExamScheduleStatus.svelte`
  - Replaces the large readiness card in the primary layout.
  - Opens readiness details in a sheet.
- `ExamRoomAssignmentPanel.svelte`
  - Removes invigilator picker.
  - Uses a sheet for create/edit.
  - Keeps seat generation actions.
- `ExamInvigilatorPanel.svelte`
  - New room-first day tab.
  - Shows day assignments and workload.
  - Opens a sheet for staff selection.
- `ExamScheduleTimeline.svelte`
  - Adds duration ghost preview.
  - Adds unschedule drop target.
  - Adds remove action in session dialog.
- `ExamItemTray.svelte`
  - Accepts scheduled session drops for unscheduling.
  - Shows count and loading state for affected item/session.

All controls should use local shadcn-svelte components where available: `Button`, `Badge`, `Alert`, `Sheet`, `Dialog`, `Table`, `Input`, `Select`, `Checkbox`, `Popover`, `Command`, and shared app-state components.

Because `Sheet` is not currently present in `frontend-school/src/lib/components/ui/`, implementation must add it as a shared UI primitive instead of hand-rolling a feature-local drawer.

## Testing

Backend focused tests:

- Workload helper sums session durations and excludes gaps.
- Workload helper groups by whole round and selected day.
- Invigilator conflict helper rejects overlapping ranges.
- Invigilator conflict helper allows non-overlapping same-day assignments.
- Unschedule service deletes sessions and returns not found for missing ids.
- Room assignment update no longer requires invigilators.

Frontend/static tests:

- Staff exam schedule detail has tabs for setup, rooms, schedule, invigilators.
- Readiness card is not rendered as a large right-side aside in the schedule-first layout.
- Room assignment panel no longer contains staff search/checkbox invigilator controls.
- Invigilator panel exposes workload summary and assignment sheet.
- Timeline drag preview uses duration width and valid/invalid states.
- Scheduled sessions can be removed through dialog and tray drop contract.
- Exam schedule UI uses shadcn primitives for touched controls.

Manual verification:

- Create an exam round, days, rooms, and sessions.
- Move sessions and observe preview width/time.
- Remove a scheduled session through both paths.
- Assign invigilators after scheduling.
- Verify workload totals and conflict blocking.
- Publish remains possible without invigilators when other readiness conditions are met.

## Rollout Notes

This is a UX and workflow refresh on top of the existing exam schedule feature. It should be implemented in small commits:

1. Backend service/API for invigilators, workload, and unschedule.
2. Compact status/readiness layout.
3. Room panel cleanup and sheet editing.
4. Invigilator panel and workload summary.
5. Timeline ghost preview and unschedule interactions.
6. Static and focused regression coverage.

The branch can be merged only after backend focused tests, frontend static tests, `cargo check`, `svelte-check`, and `git diff --check` pass.
