# Exam Invigilator Drag Drop Design

## Context

The academic exam schedule workspace already separates exam day setup, room assignment, schedule placement, and invigilator assignment. The current `กรรมการ` tab supports selecting a day, viewing room assignments, opening a sheet, and choosing invigilators with checkboxes. It also exposes workload data from the backend:

- `assignments`: room-day assignments with classroom, room, `sessionMinutes`, and current invigilators.
- `staffWorkloads`: total workload per staff member and per-day workload in minutes.
- staff option search for active staff users.

The next UX step is to make invigilator assignment faster and more visible by changing the tab from a table-plus-sheet picker into a drag/drop workspace.

## Goals

- Let staff assign invigilators by dragging a teacher card into an exam room card.
- Show each teacher's workload while assigning: selected-day minutes and whole-round minutes.
- Show how many invigilators each room currently has.
- Keep the workspace focused on one selected exam day at a time.
- Save each drag/drop or remove action immediately.
- Enforce the rule that one teacher can supervise only one room per exam day.
- Move a teacher automatically when dropped into another room on the same day.
- Keep the page usable while one assignment is saving; do not block the whole tab.

## Non-Goals

- Do not add a target invigilator count such as `2/2`.
- Do not make invigilator coverage a publish blocker.
- Do not assign invigilators per subject session in this iteration.
- Do not redesign the setup, room, or schedule tabs.
- Do not add a database migration unless implementation discovers a required schema/index change.
- Do not edit existing applied migrations.

## User Decisions

- The user drags a teacher into an exam room.
- A drop saves immediately.
- A teacher can supervise only one room per exam day.
- Dropping a teacher into a new room on the same day moves the teacher from the old room to the new room.
- Rooms do not have a required invigilator target count.
- The tab works by selecting an exam day first, then assigning invigilators for that day.

## Layout

The `กรรมการ` tab should use a two-pane operational layout.

Top controls:

- Exam day select.
- Staff search input.
- Optional checkbox or switch: show only teachers available on the selected day.
- Refresh action if needed.

Left pane: teacher list.

- Each teacher is a draggable card.
- Each card shows:
  - display name,
  - selected-day workload, formatted as hours/minutes,
  - whole-round workload,
  - selected-day room if already assigned.
- Teachers currently saving show a small pending state on their own card only.

Right pane: room board.

- Shows only room assignments for the selected exam day.
- Each room is a drop target card.
- Each card shows:
  - classroom name, for example `ม.1/1`,
  - building/room display if available,
  - `กรรมการ X คน`,
  - invigilator chips with remove buttons.
- Empty rooms show a clear drop area.
- During drag hover, the room card shows a visible drop border and a preview chip.

The workload summary should not be a separate large side panel because the teacher cards already carry the workload context. This keeps the room board as the dominant work area.

## Interaction

Assigning a teacher:

1. User selects an exam day.
2. User drags a teacher card from the left pane.
3. User drops it onto a room card on the right pane.
4. The UI optimistically adds the teacher to the target room.
5. If the teacher was assigned to another room on the same day, the UI optimistically removes the teacher from that previous room.
6. The backend saves the move in one transaction.
7. On success, workloads and room assignments update.
8. On failure, the UI rolls back the affected teacher and room cards and shows an error toast.

Removing a teacher:

1. User clicks the remove button on the teacher chip in a room card.
2. The UI optimistically removes the chip.
3. The backend removes that teacher from that room assignment.
4. On success, workloads update.
5. On failure, the chip is restored and an error toast is shown.

No-op cases:

- Dropping the same teacher into the same room should do nothing beyond ending the drag state.
- Dropping onto an invalid area should not call the API.

## Rules And Validation

The backend must remain authoritative.

- A teacher can have at most one room assignment per exam day.
- A room can have any number of invigilators.
- Assigning a teacher to a target room removes that teacher from any other room assignment on the same exam day.
- Removing a teacher affects only the target room assignment.
- Published rounds are read-only.
- Missing room assignments produce an empty state that points users back to the `ห้องสอบ` tab.
- Room-day workload is calculated from actual scheduled exam session duration for that classroom and day.
- If a room has no scheduled sessions, its `sessionMinutes` can be `0`, so assigned teachers can show `0 นาที` for that day.

Existing backend logic currently validates overlapping live session ranges. This design changes the intended UX rule to one room per teacher per day, so the new move action should enforce the stricter day-level rule in service logic. A database unique constraint can be considered later after existing tenant data is audited, but it is not required for the first implementation.

## Backend API

The current API replaces the full invigilator list for a room assignment. That is still useful for compatibility, but drag/drop needs staff-level actions so moves are atomic.

Add assign/move action:

```text
PUT /api/academic/exam-schedules/room-assignments/{assignmentId}/invigilators/{staffId}
```

Behavior:

- Resolve the assignment's exam day and exam round.
- Reject if the round is published.
- Validate that `staffId` is an active staff user.
- Lock the selected staff/day conflict scope.
- Delete that staff member from other room assignments on the same exam day.
- Insert the staff member into the target assignment if not already present.
- Mark the round draft after mutation.
- Return enough updated data for the frontend to patch the board, preferably the refreshed invigilator workspace or a typed mutation result containing affected assignments and workloads.

Add remove action:

```text
DELETE /api/academic/exam-schedules/room-assignments/{assignmentId}/invigilators/{staffId}
```

Behavior:

- Resolve the assignment's exam day and exam round.
- Reject if the round is published.
- Delete only that staff member from the target assignment.
- Mark the round draft after mutation if a row changed.
- Return updated data for the frontend to patch or refresh the invigilator workspace.

The final return shape should follow the existing API envelope. A pragmatic first implementation may return the refreshed `ExamInvigilatorWorkspace` after each action because the workspace is not large and it keeps workload patches simple.

## Frontend Components

Keep `ExamInvigilatorPanel.svelte` as the tab coordinator and split drag/drop UI into smaller components.

- `ExamInvigilatorPanel.svelte`
  - selected day state,
  - search/filter state,
  - pending staff/assignment state,
  - optimistic patch and rollback,
  - API callbacks.
- `InvigilatorStaffList.svelte`
  - teacher cards,
  - workload display,
  - drag source behavior,
  - selected-day availability state.
- `InvigilatorRoomBoard.svelte`
  - selected-day room assignment list,
  - empty state,
  - board-level drop coordination.
- `InvigilatorRoomCard.svelte`
  - single room card,
  - drop target behavior,
  - hover preview,
  - invigilator chips and remove actions.

Use local shadcn-svelte primitives for controls and states: `Button`, `Badge`, `Input`, `Select`, `Checkbox` or `Switch`, `Card` if available, `Tooltip`, and shared app-state components.

## Visual States

Teacher card states:

- normal,
- assigned today with room label,
- available today,
- filtered out by search or availability,
- saving,
- drag active.

Room card states:

- empty invigilator list,
- has invigilators,
- valid drop hover,
- saving affected assignment,
- remove pending for a chip,
- readonly when the round is published.

Status colors should be semantic but not target-count based:

- neutral for empty rooms,
- standard foreground for assigned rooms,
- accent/primary border during valid drag hover,
- muted/pending styling for optimistic saves,
- destructive styling only for remove/error actions.

## Data Flow

Initial tab load:

1. Route page loads the main exam schedule workspace.
2. When the `กรรมการ` tab becomes active, it loads the invigilator workspace and staff options.
3. The panel derives selected-day assignments and selected-day workloads from the workspace.

Assign/move:

1. Frontend builds an optimistic rollback snapshot for the staff member and affected room assignments.
2. Frontend updates the local invigilator workspace immediately.
3. Frontend calls the assign/move endpoint.
4. On success, frontend replaces the invigilator workspace with the returned workspace or patches affected assignments/workloads.
5. On error, frontend restores the rollback snapshot.

Remove:

1. Frontend removes the chip optimistically.
2. Frontend calls the remove endpoint.
3. On success, frontend replaces or patches workspace data.
4. On error, frontend restores the chip.

## Error Handling

- If invigilator workspace loading fails, keep the current retry state.
- Assignment failures should show a toast with the backend message.
- Optimistic failures should restore only the affected teacher and room cards.
- A stale response from a previous round or previous search must not overwrite current state.
- Published-round errors should leave the UI read-only after refresh.

## Testing

Backend tests:

- Assigning a staff member to a new room removes that staff member from other rooms on the same exam day.
- Assigning a staff member to the same room is idempotent.
- Removing a staff member deletes only the target room assignment row.
- Assign/move rejects published rounds.
- Assign/move validates active staff users.
- The service locks staff/day scope before mutating rows.

Frontend/static tests:

- `ExamInvigilatorPanel` renders a teacher-to-room drag/drop workflow.
- Teacher cards show selected-day workload and whole-round workload.
- Room cards show `กรรมการ X คน`.
- The UI does not render target-count text such as `2/2`.
- Dropping assigned staff into another selected-day room calls the move action rather than replacing the full room list.
- Removing a teacher chip calls the remove action.
- Pending assignment state is scoped to affected teacher/room, not the whole tab.

Verification commands for implementation:

- Focused backend service tests for exam schedule invigilator logic.
- `cargo check` for backend changes.
- `node --test tests/static/academic-exam-schedule.test.mjs` for frontend static coverage.
- `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check` for frontend type/Svelte diagnostics.
- `git diff --check`.

## Open Implementation Notes

- Prefer returning the refreshed invigilator workspace from assign/remove endpoints unless performance becomes a real issue.
- Keep the old full-list update endpoint until all existing UI paths are migrated or compatibility is intentionally removed.
- If tenant data later needs hard database enforcement, add a new migration after auditing rows where one staff member appears in multiple room assignments on the same exam day.
