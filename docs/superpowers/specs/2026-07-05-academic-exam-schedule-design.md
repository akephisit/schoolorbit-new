# Academic Exam Schedule Design

## Goal

Build an academic exam scheduling workflow for current students. The system should create midterm/final exam rounds from existing assessment plans, let staff schedule exams on custom day timelines with drag and drop, assign each classroom to a fixed exam room and invigilators for a full exam day, and publish the finished timetable to students and parents.

## Scope

This design covers the first implementation of the exam schedule module:

- Staff create exam rounds for an academic semester, such as "Midterm 1/2569".
- Staff define exam days, operating hours, blocked windows, and grade-level scope per day.
- The system imports only `academic_assessment_categories` where `exam_mode = 'in_timetable'`.
- Imported exams use `exam_duration_minutes` from the assessment category as the scheduling block duration.
- Staff assign each classroom to one exam room for the full exam day.
- Staff assign invigilators to that classroom-room assignment for the full exam day.
- Staff generate default seat numbers for each classroom-room assignment from classroom enrollment order.
- Staff drag unscheduled exam items onto a day timeline; the block length is derived from duration.
- The UI and backend prevent invalid placements.
- Staff publish a valid schedule so students and parents can see their personal exam timetable.

Out of scope for the first version:

- Auto-generating the full timetable.
- Per-subject room or invigilator overrides.
- Seating plans that split one classroom across multiple rooms.
- Importing `outside_timetable` assessment categories into the exam timetable.
- Reusing regular timetable periods.

## Existing System Fit

The workflow lives under `modules/academic` as a new `exam_schedule` area, separate from regular academic timetable code. This avoids forcing exam scheduling into `academic_timetable_entries`, because exams use variable durations, exam-day windows, blocked time ranges, and full-day classroom room assignments instead of regular timetable periods.

The module integrates with:

- `academic_assessment_plans` and `academic_assessment_categories` for `in_timetable` exam sources and `exam_duration_minutes`.
- `class_rooms` / classrooms for the student group being scheduled.
- `rooms` and `buildings` for exam room selection, room labels, and default capacity.
- student enrollments to determine the students in each classroom.
- staff records for invigilator assignment.
- standard request context, permission, API envelope, and service-layer patterns.

## Core Entities

### Exam Round

Represents a named exam cycle for one semester.

Important fields:

- `id`
- `academic_semester_id`
- `name`
- `exam_type`, for example `midterm` or `final`
- `status`, with `draft` and `published`
- `published_at`
- `created_by`
- timestamps

Any edit after publish should return the round to `draft` so student and parent visibility stays intentional.

### Exam Day

Represents one usable day inside an exam round.

Important fields:

- `id`
- `exam_round_id`
- `exam_date`
- `start_time`
- `end_time`
- `display_order`
- optional notes

Each day has one or more grade-level scopes. Example: day 1 and 3 allow Mathayom lower secondary, day 2 and 4 allow Mathayom upper secondary.

### Exam Day Blocked Window

Represents a time range where staff cannot place exams, such as lunch.

Important fields:

- `id`
- `exam_day_id`
- `start_time`
- `end_time`
- `label`

### Exam Day Classroom Room Assignment

Represents the full-day mapping from a classroom to its exam room.

Example:

```text
Exam day 1
M.1/1 -> room 313 -> capacity 40
M.1/2 -> room 314 -> capacity 40
```

Important fields:

- `id`
- `exam_day_id`
- `classroom_id`
- `room_id`
- `capacity`

The initial capacity is copied from `rooms.capacity` when staff select the room. Staff may adjust capacity for that exam day. Later changes to master room capacity do not mutate an already planned exam day.

### Exam Day Invigilator

Represents staff assigned to a classroom-room assignment for the whole day.

Important fields:

- `id`
- `day_room_assignment_id`
- `staff_id`
- `role`, optional, for example `primary` or `assistant`

### Exam Schedule Item

Represents one imported exam that needs placement on the timeline.

Important fields:

- `id`
- `exam_round_id`
- source `assessment_category_id`
- `subject_id`
- `classroom_id`
- `duration_minutes`
- `status`, such as `unscheduled` or `scheduled`

Only assessment categories with `exam_mode = 'in_timetable'` are imported. `outside_timetable`, `none`, and `practical` are excluded from the timetable.

Import expands subject-level assessment categories into classroom-level schedule items for the semester. For example, if the M.1 Mathematics assessment category is `in_timetable` and M.1/1, M.1/2, and M.1/3 all have that subject, the import creates one schedule item per classroom.

### Exam Session

Represents a scheduled placement of one exam item.

Important fields:

- `id`
- `exam_schedule_item_id`
- `exam_day_id`
- `start_time`
- `duration_minutes`

The effective end time is computed as `start_time + duration_minutes`. There is no `period_id`.

The session resolves its room and invigilators through `exam_day_id + classroom_id` using the full-day room assignment.

### Exam Seat Assignment

Represents the default seating list for one classroom-room assignment.

Important fields:

- `id`
- `day_room_assignment_id`
- `student_id`
- `seat_number`

The first version keeps the whole classroom in one exam room for the day. Seat numbers default from classroom enrollment number or the existing classroom ordering. Staff can regenerate the list after enrollment changes. Splitting one classroom across multiple exam rooms is intentionally out of scope.

## Staff Workflow

1. Staff create an exam round, for example `Midterm 1/2569`, for an academic semester.
2. Staff add exam days.
3. For each day, staff configure:
   - day start and end time,
   - blocked windows such as lunch,
   - grade-level scope allowed on that day.
4. For each day, staff assign classrooms to exam rooms:
   - M.1/1 to room 313,
   - M.1/2 to room 314,
   - each assignment copies and can override room capacity.
5. For each classroom-room assignment, staff choose full-day invigilators.
6. Staff generate or refresh seat numbers for each classroom-room assignment.
7. Staff import exam items from assessment categories where `exam_mode = 'in_timetable'`; import expands each subject/category into the matching classrooms for the semester.
8. The scheduling page shows unscheduled items in a side panel and exam days as timeline tabs.
9. Staff drag an item onto a timeline. The block length is determined by `duration_minutes`.
10. The UI refuses invalid drops.
11. Staff run readiness validation.
12. Staff publish once all blocking issues are resolved.
13. Students and parents see the published personal timetable.

## Scheduling UX

The staff detail page should be a single operational workspace with three regions:

- Left setup/queue panel:
  - exam day setup,
  - blocked windows,
  - classroom room assignments,
  - invigilators,
  - unscheduled imported exams.
- Center timeline:
  - day tabs,
  - classroom tracks for the selected day,
  - drag-and-drop exam blocks,
  - blocked windows rendered as unavailable bands.
- Right readiness panel:
  - publish status,
  - validation results,
  - student preview for a selected classroom/student.

Timeline behavior:

- Blocks snap to a configured time unit such as 5 or 10 minutes.
- Block height/width reflects `duration_minutes`.
- Drops outside day time, across blocked windows, or into invalid scopes are rejected.
- Drops that create classroom, room, or invigilator conflicts are rejected.
- The UI should explain why a drop was rejected.

## Student And Parent View

Student and parent routes show only published exam rounds.

For each student, the timetable is derived from their current classroom and scheduled exam sessions. The response should include:

- exam round name,
- date,
- start time,
- computed end time,
- subject,
- exam room,
- optional building,
- seat number.

Invigilators do not need to be shown to students or parents in the first version.

## Permissions

Add a new permission module `academic_exam_schedule`.

Recommended permissions:

- `academic_exam_schedule.read.school`
- `academic_exam_schedule.manage.school`
- `academic_exam_schedule.publish.school`

Frontend route access should be read-first:

- users with read permission can enter the workspace and inspect schedules,
- create/update/delete controls require manage permission,
- publish controls require publish permission.

Use backend permission registry constants and mirror them in the frontend permission registry.

## API Shape

All JSON responses use the standard `{ success, data, message?, error? }` envelope.

Representative staff endpoints:

```http
GET    /api/academic/exam-schedules
POST   /api/academic/exam-schedules
GET    /api/academic/exam-schedules/{round_id}
PUT    /api/academic/exam-schedules/{round_id}
POST   /api/academic/exam-schedules/{round_id}/import-assessments
POST   /api/academic/exam-schedules/{round_id}/publish
POST   /api/academic/exam-schedules/{round_id}/validate

POST   /api/academic/exam-schedules/{round_id}/days
PUT    /api/academic/exam-schedules/days/{day_id}
DELETE /api/academic/exam-schedules/days/{day_id}

PUT    /api/academic/exam-schedules/days/{day_id}/grade-levels
PUT    /api/academic/exam-schedules/days/{day_id}/blocked-windows
PUT    /api/academic/exam-schedules/days/{day_id}/room-assignments
PUT    /api/academic/exam-schedules/day-room-assignments/{id}/invigilators
POST   /api/academic/exam-schedules/day-room-assignments/{id}/generate-seats

POST   /api/academic/exam-schedules/{round_id}/sessions
PUT    /api/academic/exam-schedules/sessions/{session_id}
DELETE /api/academic/exam-schedules/sessions/{session_id}
```

Representative self-view endpoints:

```http
GET /api/me/exam-schedule
GET /api/parent/students/{student_id}/exam-schedule
```

## Validation Rules

Blocking validation applies in frontend drag/drop and backend save/publish:

- only `in_timetable` categories may be imported into schedule items,
- every scheduled item must be placed on an exam day whose grade-level scope allows the classroom,
- `start_time + duration_minutes` must stay inside the exam day's operating hours,
- sessions must not overlap blocked windows,
- the same classroom must not have overlapping sessions,
- one room must not be assigned to multiple classrooms on the same exam day,
- one invigilator must not be assigned to multiple classroom-room assignments on the same exam day,
- classroom enrollment count must not exceed the assigned exam room capacity,
- every published classroom-room assignment must have seat assignments for the enrolled students,
- publish requires all active imported schedule items to be scheduled,
- publish requires every scheduled classroom/day to have a room assignment and required invigilators.

Backend validation remains authoritative even if the UI prevents invalid drops.

## Error Handling

Handlers stay thin:

1. resolve actor tenant context,
2. require permission,
3. call service,
4. return typed API response.

Services return `Result<T, AppError>` and typed validation outcomes. Validation errors should be user-facing Thai messages where they affect the UI.

No plaintext PII is required for this workflow. Student self/parent views should return only schedule data needed by the exam workflow.

## Testing

Backend service unit tests:

- import filters only `in_timetable`,
- duration is copied from assessment categories,
- time overlap detection,
- blocked-window detection,
- grade-level scope validation,
- room capacity validation,
- duplicate room assignment validation,
- duplicate invigilator validation,
- publish readiness.

Frontend checks:

- route metadata and permission constants stay aligned,
- API client contracts use typed responses,
- Svelte check for the new pages,
- drag/drop pure helpers for time math and validation where practical.

Repository checks:

```bash
cd backend-school && cargo test exam_schedule_service
cd backend-school && cargo check
cd frontend-school && npm run test:static
cd frontend-school && npm run check
git diff --check
```
