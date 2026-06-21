# Daily Teaching Overview Design

## Context

SchoolOrbit already has two timetable surfaces:

- `/staff/timetable` — self-service timetable for the logged-in teacher.
- `/staff/academic/timetable` — academic course-plan timetable workspace for planning and editing.

The requested feature is a read-oriented daily overview that answers: "Today, which teachers are teaching which periods, classes, and subjects across the whole school?"

This should not be another edit surface. It should be a fast operational view for checking the school's teaching activity on a selected day.

User decisions:

- Use the two-level access model.
- Teachers see the whole-school daily teaching table as read-only.
- Academic staff get extra filters, export/print, and links back to the existing timetable planning page.
- Mobile must still use a table view, not a stacked per-teacher list.

## Goals

- Add a daily whole-school teacher timetable overview.
- Show one selected date at a time, defaulting to today.
- Show each teacher's lessons by period: subject, classroom, and room where available.
- Keep the UI table-based on desktop and mobile.
- Make the page useful for ordinary teachers without granting timetable management access.
- Give academic staff stronger filtering and operational tools without duplicating the timetable editor.

## Non-Goals

- Do not add timetable editing in this page.
- Do not replace `/staff/academic/timetable`.
- Do not expose staff PII, student lists, student names, national IDs, contact data, or private profile data.
- Do not add attendance, substitution, absence, or cover-teacher workflows in the first iteration.
- Do not add a weekly/monthly report in the first iteration.

## Menu And Route

Add a new real menu route:

```text
วิชาการ > ตารางสอนวันนี้
```

Recommended route:

```text
/staff/academic/timetable/today
```

Reasoning:

- It belongs near the existing timetable planning workspace.
- It is clearly different from `/staff/timetable`, which remains "my timetable".
- It can share timetable terminology, period models, semester selectors, and future navigation from the existing academic timetable area.

The route should have menu metadata and be visible to staff users who can read the daily overview. It should not appear as a child route of `/staff/timetable`.

## Permission Model

Use a two-level access model.

### Teacher Read-Only Access

Teachers can open the daily overview and see the whole-school table. They cannot edit entries, export, print, or jump directly into edit actions unless they already have academic timetable permissions.

Add a read-only permission using the canonical permission shape:

```text
academic_timetable_today.read.school
```

Frontend constant name:

```text
ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL
```

Add a new permission module for route/menu discovery:

```text
academic_timetable_today
```

Frontend module constant:

```text
ACADEMIC_TIMETABLE_TODAY
```

The menu route should use this module permission so teachers can see the page without receiving broader `academic_course_plan` access. The route still lives visually under the academic workspace/group.

Ordinary teacher access is view-only. Export, print, and edit navigation are academic enhanced actions.

### Academic Enhanced Access

Academic staff with existing timetable/course-plan permissions get extra capabilities:

- `academic_course_plan.read.all`: advanced filters and links to the planning workspace.
- `academic_course_plan.manage.all`: edit-link affordances that navigate to `/staff/academic/timetable`; actual edits still happen only on the existing editor and remain backend-authorized there.

No frontend-only gate should be trusted for backend access. The daily overview endpoint must independently authorize teacher read-only access and academic read-all access.

## Backend API

Add a dedicated daily overview endpoint instead of opening the existing academic timetable list endpoint to every teacher.

Recommended endpoint:

```http
GET /api/academic/timetable/daily-teaching?date=YYYY-MM-DD&academic_semester_id=<uuid>
```

Query behavior:

- `date` defaults to the tenant-local current date when omitted.
- `academic_semester_id` is optional only if the backend can resolve the current active semester. If no active semester can be resolved, return a clear bad-request or empty-state payload the frontend can display.
- The backend converts `date` to the timetable `day_of_week`.

The endpoint should return a typed API envelope:

```json
{
  "success": true,
  "data": {
    "date": "2026-06-22",
    "dayOfWeek": "MON",
    "academicSemesterId": "00000000-0000-0000-0000-000000000000",
    "periods": [
      {
        "id": "period-id",
        "name": "คาบ 1",
        "startTime": "08:30:00",
        "endTime": "09:20:00",
        "orderIndex": 1
      }
    ],
    "teachers": [
      {
        "id": "teacher-id",
        "displayName": "ครูสมชาย ใจดี",
        "organizationUnitNames": ["กลุ่มสาระคณิตศาสตร์"],
        "periods": [
          {
            "periodId": "period-id",
            "entries": [
              {
                "entryId": "entry-id",
                "entryType": "COURSE",
                "subjectCode": "ค21101",
                "subjectName": "คณิตศาสตร์",
                "classroomName": "ม.1/1",
                "roomCode": "321",
                "title": null,
                "note": null,
                "isTeamTeaching": false
              }
            ]
          }
        ]
      }
    ],
    "summary": {
      "totalTeacherCount": 84,
      "displayedTeacherCount": 61,
      "teachersTeachingCount": 61,
      "lessonCount": 244,
      "emptyTeacherCount": 23
    }
  }
}
```

The final implementation can choose a flatter row shape if it keeps the UI efficient, but the API must remain typed and must not return raw JSON values.

### Data Rules

- Default rows include active staff teachers who have a timetable entry on the selected day.
- Summary counts include total active teachers, teachers with lessons, displayed teachers, lesson count, and teachers without lessons.
- Academic enhanced users can toggle `include_empty_teachers=true` to include teachers with no teaching periods that day.
- Include team-teaching entries for every teacher attached through `timetable_entry_instructors`.
- Include `COURSE`, `ACTIVITY`, `HOMEROOM`, `ACADEMIC`, and `BREAK` only when they are associated with a teacher. Teacher-less school-wide breaks should not create rows for every teacher.
- Sort periods by `academic_periods.order_index`, then start time.
- Sort teachers by organization unit display order if available, then Thai display name.

### Backend Structure

Follow existing backend patterns:

- Handler in the academic timetable module stays thin.
- Service owns SQL and grouping logic.
- Use typed DTOs with `#[serde(rename_all = "camelCase")]`.
- Use request context helpers.
- Add a policy/helper that authorizes either the new read-only permission or existing academic course-plan read permission.
- Add service-level unit tests for pure grouping helpers, including team teaching and empty periods.

## Frontend API

Add a typed API wrapper in the timetable API client, for example:

```ts
getDailyTeachingOverview(filters): Promise<LoadedApiResponse<DailyTeachingOverview>>
```

The frontend type names should reflect the API shape:

- `DailyTeachingOverview`
- `DailyTeachingPeriod`
- `DailyTeachingTeacher`
- `DailyTeachingPeriodCell`
- `DailyTeachingSummary`

The page should not call `listStaff()` or generic staff profile APIs. Teacher names and organization labels must come from the dedicated daily overview endpoint.

## Page Layout

### Header And Controls

Use `PageShell` with title:

```text
ตารางสอนวันนี้
```

Header controls:

- date picker defaulting to today
- previous day / next day buttons
- academic semester selector
- refresh button

Read-only teacher controls:

- search teacher name

Academic enhanced controls:

- organization unit / subject group filter
- grade/classroom filter
- toggle "รวมครูไม่มีคาบ"
- print/export button
- link to `/staff/academic/timetable`

### Summary Row

Show compact cards above the table:

- ครูที่มีสอนวันนี้
- จำนวนคาบสอนรวม
- ครูไม่มีคาบวันนี้
- ภาคเรียน / วันที่เลือก

The summary is operational and should not become an analytics dashboard.

### Main Table

Desktop and mobile both use a table.

Structure:

- Rows: teachers.
- Columns: periods.
- Sticky first column: teacher name and organization unit.
- Sticky header row: period name and time.
- Cells: one or more teaching entries.

Cell content:

- subject or event title
- classroom name
- room code
- subtle badge for team teaching or non-course entry type

Empty cells:

- show a quiet dash or blank state, not a large card.

Overflow:

- If more than one entry appears in a teacher/period cell, show stacked compact chips inside the cell.
- If cell content is long, truncate with tooltip/detail dialog.

### Mobile Table

Mobile remains table-based.

Required behavior:

- Horizontal scroll for period columns.
- Sticky teacher column on the left.
- Sticky period header at the top when scrolling vertically.
- Cell chips remain compact and readable.
- Tapping a cell opens a bottom sheet or dialog with full details.

The mobile table may use smaller typography and fixed column widths, but it should not switch to a card list by teacher.

## Empty, Loading, And Error States

Use shared app-state components:

- `PageSkeleton` for initial loading.
- `PageState` for no active semester, no timetable for the date, permission blocked, and load error states.

Examples:

- No active semester: "ยังไม่มีภาคเรียนที่ใช้งานสำหรับวันที่เลือก"
- No lessons: "ไม่พบตารางสอนของครูในวันนี้"
- Permission blocked: route guard handles true no-access; page-level blocked states only apply to optional enhanced actions.

## Realtime And Freshness

The first iteration can use manual refresh and normal API loading.

If the implementation can reuse the existing timetable WebSocket safely, it may refresh the daily overview when timetable entries for the selected semester change. This is optional for the first iteration and should not block delivery.

## Privacy And Security

The endpoint returns only operational timetable fields:

- teacher display name
- organization unit display label
- subject/title
- classroom
- room
- period/time

It must not return:

- national IDs
- usernames unless needed for a technical identifier display, which is not needed here
- phone, email, address, line ID, emergency contacts
- student names or roster data

The page must remain read-only for ordinary teachers.

## Testing

Backend:

- Static guard that the route is registered and uses typed API DTOs.
- Unit tests for grouping timetable entries by teacher and period.
- Unit tests for team-teaching entries appearing under each assigned teacher.
- Authorization tests or static checks proving the endpoint accepts the new read permission or academic read permission, without requiring manage permission.
- `cargo test --test static_architecture`.
- `cargo check`.

Frontend:

- Static contract test that the route metadata uses the permission registry, not hardcoded strings.
- Static/API contract test for `getDailyTeachingOverview()`.
- Static page test that the page uses `PageShell`, `PageSkeleton`, `PageState`, and does not call staff/student list APIs.
- `npm run test:static`.
- `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`.

Repository:

- `git diff --check`.
- `git status --short`.

## Acceptance Criteria

- Staff users with the daily overview read permission can open `/staff/academic/timetable/today`.
- The page defaults to today's date and shows the correct timetable day.
- Teachers are shown in rows and periods are shown in columns on both desktop and mobile.
- Mobile uses a horizontally scrollable table with sticky teacher column and sticky period header.
- Each occupied cell shows subject/title, classroom, and room where available.
- Team-teaching entries appear for every teacher assigned to the entry.
- Ordinary teachers cannot edit timetable data from this page.
- Academic staff see enhanced filters and navigation to the existing timetable planner when their permissions allow it.
- The endpoint does not expose PII or student roster data.
- Static and type checks pass.

## Open Decisions

No product decisions remain open for the first design. The first implementation should default to showing only teachers with lessons, and only academic enhanced users should see the "รวมครูไม่มีคาบ" toggle.
