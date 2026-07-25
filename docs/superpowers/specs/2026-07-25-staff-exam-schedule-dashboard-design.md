# Staff Exam Schedule Dashboard Design

## Context

The staff route `/staff/exams` currently shows every published school exam session in a shared
table component that is also used by student and parent pages. The staff view hides seat numbers,
but otherwise remains a wide, minimally grouped table. It does not expose invigilator assignments,
does not distinguish lower and upper secondary schedules, and cannot answer a staff member's most
common operational questions:

- What is being examined on each day and in each classroom?
- Who supervises each classroom and exam room?
- Which assignments belong to the signed-in staff member?

The exam schedule export already provides useful report structures, including merged day and time
groups and a dedicated invigilator summary. The staff web page should adopt the same information
hierarchy while remaining responsive and interactive.

## Goals

- Keep a whole-school view of published exam schedules.
- Separate lower-secondary and upper-secondary schedule views.
- Let staff narrow the schedule to an individual classroom.
- Show invigilators by exam day, classroom, and exam room.
- Give the signed-in staff member a focused view of their own invigilation assignments.
- Group repeated day, time, classroom, and room values with table row spans on desktop.
- Use a responsive day-card presentation on mobile instead of forcing a wide table.
- Reuse the existing shared page layout, state components, and shadcn-svelte primitives.
- Keep student and parent published-schedule contracts unchanged and free of invigilator data.

## Non-Goals

- Do not let staff create, edit, publish, or assign exam schedules from `/staff/exams`.
- Do not expose draft exam rounds.
- Do not change the academic exam-schedule management workspace or its XLSX export.
- Do not add another download workflow to the staff page in this iteration.
- Do not add real-time updates or WebSocket events.
- Do not expose usernames, email addresses, phone numbers, national IDs, or other staff PII.
- Do not add or edit database migrations.

## User Decisions

- The page remains a whole-school published schedule.
- The primary layout is a tabbed dashboard rather than one long report or multiple routes.
- The schedule provides lower-secondary, upper-secondary, and classroom-specific views.
- The page includes an invigilator table organized by day and room.
- The page includes a signed-in user's invigilation view.
- The desktop presentation should resemble the downloaded workbook, including merged repeated
  cells, while the mobile presentation may use cards.

## Information Architecture

The page keeps the existing `PageShell` title and uses the following top-level controls:

- Published exam-round selector, defaulting to the first round returned by the backend. The backend
  continues ordering rounds by most recent publication first.
- Optional exam-day filter.
- Search across subject name, subject code, classroom name, room name, and invigilator display name.
- A clear-filters action that returns day, level, classroom, and search filters to their defaults.

Summary cards appear below the controls:

- Number of exam days in the selected round.
- Number of distinct exam rooms used.
- Number of distinct assigned invigilators.
- The signed-in user's next invigilation assignment, or an explicit no-assignment state.

These four cards summarize the selected round and do not change with the tab-level day, level,
classroom, or search filters.

The workspace has four shadcn-svelte tabs.

### Overview

The overview shows:

- The selected round's published status and date range.
- The next exam day and its session count.
- A concise lower-secondary and upper-secondary session count.
- The signed-in user's next assignment with date, time range, classroom, and exam room.
- A compact list of upcoming exam days that links through tab state to the schedule view.

The overview is a scanning surface, not a duplicate of the full tables.

### Exam Schedule

The schedule tab has a secondary level selector:

- All levels.
- Lower secondary, defined by grade-level year `1` through `3`.
- Upper secondary, defined by grade-level year `4` through `6`.

A classroom filter defaults to all classrooms and is populated from the selected level. Selecting
a classroom narrows the same table; it does not navigate to another route.

Desktop columns are:

1. Exam day and date.
2. Start and end time.
3. Subject and optional subject code.
4. Assessment category.
5. Classroom.
6. Building and exam room.
7. Invigilators.

Rows are ordered by exam date, start time, grade-level year, classroom natural order, subject, and
assessment category. The table header remains sticky inside a horizontally scrollable table
container.

### Invigilators

The invigilator tab follows the workbook's `กรรมการคุมสอบ` hierarchy:

1. Exam day and date.
2. Classroom.
3. Exam room.
4. Actual scheduled time ranges for that room and day.
5. Invigilator names.

Assignments are ordered by exam date, classroom natural order, and room name. Invigilator names
use one readable list inside the final cell rather than assuming exactly two people. The signed-in
user receives a visible `ฉัน` badge and primary-tinted emphasis without changing the displayed
name.

### My Invigilation

This tab filters room assignments whose invigilator list contains the signed-in user's `staffId`.
Each assignment shows:

- Exam day and date.
- Earliest scheduled start and latest scheduled end for that room assignment.
- Actual scheduled supervision minutes, excluding gaps between sessions.
- Classroom and exam room.
- Subjects scheduled in the assigned classroom on that day.

Assignments whose exam end time is still in the future appear chronologically before completed
assignments; completed assignments remain visible in reverse chronological order. The comparison
uses the browser's local date and time. The tab includes totals for assigned days, room-day
assignments, and actual scheduled supervision time. If the user has no assignments in the selected
round, the tab shows a focused empty state rather than an empty table.

## Merged-Cell Semantics

Desktop tables use semantic HTML `rowspan` values computed from already sorted visible rows.

- Schedule day cells span all visible rows for the same exam date.
- Schedule time cells span consecutive rows only when exam date, start time, and end time match.
- Invigilator day cells span all visible room-assignment rows for the same exam date.
- If one room assignment needs more than one visual row, its classroom and room cells span only
  those rows.
- A merge group never crosses an exam day, time slot, selected round, or filtered result boundary.
- Applying a filter rebuilds groups and spans from the filtered rows.
- A single-row group renders with `rowspan=1`.

The grouping helper returns explicit render rows with flags such as `showDayCell` and span values.
The Svelte component renders those instructions without calculating adjacent row state in markup.

Mobile layouts do not use row spans. They group records into cards headed by exam day and show
individual time or room rows inside each card.

## Responsive And Visual Design

Desktop uses report-like tables with:

- Sticky neutral table headers.
- Subtle alternating backgrounds by exam day group.
- Centered day, time, and room cells.
- Left-aligned subject and invigilator cells.
- Compact badges for level, assessment category, and the current user.
- Borders that preserve the visual grouping created by row spans.

At widths below the existing mobile breakpoint:

- Summary cards become a compact grid.
- Filters stack vertically.
- Tabs remain horizontally scrollable through the local Tabs primitive.
- Schedule and invigilator tables become day-grouped cards built from a shared shadcn-style
  Collapsible primitive backed by the existing `bits-ui` dependency.
- The first visible day starts expanded; other day cards start collapsed and can be toggled with
  keyboard-accessible triggers.
- Cards expose time, subject, classroom, room, and invigilators as labeled values.
- No page-level horizontal overflow is introduced.

Color is supplementary. Day groups, current-user emphasis, and status text remain understandable
without relying on color alone.

## Backend Contract

The existing `GET /api/staff/exam-schedules` route remains the staff read endpoint, but it receives
a staff-specific typed response. Student and parent endpoints continue returning
`PersonalExamScheduleRound`.

The staff response is composed of published rounds with nested exam days:

```text
StaffPublishedExamScheduleRound
  roundId
  roundName
  academicSemesterId
  publishedAt
  days[]

StaffPublishedExamDay
  examDayId
  label
  examDate
  sessions[]
  roomAssignments[]

StaffPublishedExamSession
  sessionId
  startsAt
  endsAt
  durationMinutes
  subjectId
  subjectCode
  subjectName
  assessmentCategoryName
  gradeLevelId
  gradeLevelName
  gradeLevelType
  gradeLevelYear
  classroomId
  classroomName
  dayRoomAssignmentId
  roomId
  roomName
  buildingName

StaffPublishedExamRoomAssignment
  assignmentId
  classroomId
  classroomName
  roomId
  roomName
  buildingName
  sessionMinutes
  earliestStartsAt
  latestEndsAt
  invigilators[]

StaffPublishedExamInvigilator
  staffId
  displayName
```

`sessionMinutes` is the sum of scheduled session durations for the assignment and excludes gaps.
`earliestStartsAt` and `latestEndsAt` are display bounds, not workload duration.

The handler continues resolving the current user through the request-context helper and the service
continues requiring an active staff account. The service queries only rounds with
`status = 'published'`. It returns typed models and groups session and assignment rows in the
service layer.

The OpenAPI handler response, Rust schemas, tracked OpenAPI artifact, and generated TypeScript
types are updated together. The frontend API module consumes the generated staff DTO and does not
introduce a parallel wire interface.

## Frontend Architecture

The route remains `frontend-school/src/routes/(app)/staff/exams/+page.svelte`. It keeps the existing
client-side credentialed API load because the school API cookie belongs to the cross-origin API
domain and the current app authentication flow already uses the custom browser API client.

Expected components under `frontend-school/src/lib/components/academic/exam-schedule/`:

- `StaffExamScheduleDashboard.svelte`
  - Owns round, day, level, classroom, search, and tab state.
  - Derives summary counts and filtered view data.
- `StaffExamScheduleTable.svelte`
  - Renders the desktop schedule table and mobile schedule cards.
- `StaffExamInvigilatorTable.svelte`
  - Renders the desktop room/invigilator report and mobile room cards.
- `MyExamInvigilationView.svelte`
  - Renders current-user totals, upcoming assignments, and empty state.

Small presentation helpers remain within these components. Deterministic sorting, filtering,
grouping, duration aggregation, and row-span calculation live in one focused utility module so
they can be tested independently.

The student and parent routes continue using `PersonalExamScheduleView.svelte`. The staff route no
longer passes staff data through that personal view.

## Data Flow

1. The route loads staff published rounds through `listStaffExamSchedules()`.
2. The page defaults to the most recently published round returned by the backend.
3. The dashboard derives available days, levels, classrooms, subjects, rooms, and invigilator
   search values from the selected round.
4. Filter changes recompute visible records locally and do not issue additional API requests.
5. The schedule and invigilator tabs build grouped render rows from filtered records.
6. The current user's tab filters by the authenticated user's id from the existing auth store.
7. Retry performs the same single API request and retains no stale failed data.

No mutation is performed from this page.

## Loading, Empty, And Error States

- Initial loading uses the shared `PageSkeleton`.
- Initial failure uses `PageState` with a retry action and a toast containing the same useful
  message.
- No published rounds uses a page-level empty state.
- A selected round with no sessions uses a round-specific empty state.
- Filters with no matches use an inline empty state and clear-filters action.
- No personal assignment uses a dedicated `งานคุมของฉัน` empty state.
- Missing optional labels fall back to `-` without hiding the rest of a record.
- Invalid date or time strings fall back to their source text rather than throwing during render.

## Authorization And Privacy

- The route remains available to active staff accounts without an academic management permission.
- The backend remains authoritative and returns published data only.
- The staff response exposes stable ids and display labels required by the workflow.
- The response does not include usernames, email addresses, phone numbers, addresses, national
  ids, profile details, or other PII.
- Student and parent responses do not gain invigilator names or ids.

## Accessibility

- Use local shadcn-svelte `Tabs`, `Select`, `Input`, `Badge`, `Table`, and app-state components.
- Desktop tables use real table markup.
- Column headers use the appropriate header scope.
- Row-spanned day and time cells remain table cells rather than visual overlays.
- Search and filter controls have visible labels or accessible names.
- The current-user badge supplements the full display name.
- Keyboard focus remains visible.
- Mobile card labels preserve the meaning supplied by desktop column headers.
- The page keeps a unique descriptive `<title>`.

## Testing

### Backend

- Staff published schedule returns only published rounds.
- Session rows include grade-level metadata required for lower/upper-secondary filtering.
- Room assignments contain the correct invigilators for the same exam day and classroom.
- Assignment `sessionMinutes` sums actual session durations and excludes gaps.
- Empty invigilator assignments remain visible with an empty invigilator list.
- Active-staff validation remains enforced.
- Student and parent response models remain free of staff invigilator fields.

### Frontend

- Round selection defaults to the most recently published round.
- Lower-secondary filtering includes grade years 1-3 and excludes 4-6.
- Upper-secondary filtering includes grade years 4-6 and excludes 1-3.
- Classroom, day, and search filters compose as an intersection.
- Schedule day spans never cross dates.
- Schedule time spans never cross dates or different start/end times.
- Invigilator day spans are recomputed after filtering.
- My-invigilation filtering matches only the current user's `staffId`.
- Current-user names render with the `ฉัน` badge.
- Empty states and clear-filters behavior are present.
- Mobile presentation does not depend on desktop row spans.

### Verification

- Regenerate and check the OpenAPI and TypeScript contracts.
- Run focused backend exam-schedule service tests and `cargo check`.
- Run the focused frontend academic exam-schedule static tests.
- Run `svelte-check`.
- Run the Svelte autofixer on every changed Svelte component until no issues remain.
- Verify desktop and mobile browser viewports with representative multi-day, multi-level data.
- Run `git diff --check` and inspect the final working tree.

## Rollout

Implement in small verified stages:

1. Add the staff-specific published schedule DTO and service query/grouping behavior.
2. Update OpenAPI and generated frontend contracts.
3. Add deterministic frontend view helpers with focused tests.
4. Replace the staff personal table with the tabbed dashboard.
5. Add responsive cards, merged desktop cells, and current-user emphasis.
6. Run contract, backend, frontend, Svelte, and browser verification.

The deployment requires no database migration and does not alter existing draft schedule management
or student/parent published schedule behavior.
