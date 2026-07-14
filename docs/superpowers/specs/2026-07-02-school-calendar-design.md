# School Calendar Design

## Context

SchoolOrbit needs a school-wide calendar module. The first use case is an academic calendar, but the module should be named and structured as a central `calendar` feature so it can later serve other school operations.

The repository already has:

- academic year, semester, classroom, grade level, and timetable data under the `academic` module.
- notifications with in-app records, SSE delivery, and web push support through `NotificationService`.
- `tokio_cron_scheduler` background jobs in `backend-school/src/main.rs`.
- a `calendar_events` feature toggle seed in the baseline, but no actual calendar event module.

User decisions:

- V1 is manually entered. It does not import events from academic, admission, work, or workflow modules.
- Events publish immediately. There is no draft or approval workflow in V1.
- Events can target staff, students, parents, grade levels, and classrooms.
- Parent visibility is explicit. Parents do not automatically see student-targeted events unless the event also targets parents.
- Events support all-day/multi-day dates and optional start/end times. Recurring events are out of scope.
- Public calendar is supported, but public visibility is never automatic. Staff must mark an event public.
- V1 views are month calendar plus event list.
- Event categories are school-managed with name, color, and order.
- Each event has at most one primary category for calendar color and may have multiple reusable tags.
- Notifications are in scope: notify on create/update and multiple reminders per event.
- Calendar events do not affect timetable, attendance, check-in, or workflow behavior in V1.
- UI must use local shadcn-svelte primitives and semantic color carefully while keeping the interface clean.

## Goals

- Add a central `calendar` backend module and frontend workspace.
- Let authorized staff create, edit, soft-delete, categorize, target, and publish calendar events immediately.
- Let staff, students, and parents see only the authenticated events relevant to them.
- Let unauthenticated visitors see only events explicitly marked public.
- Send in-app notifications when staff choose to notify audiences on create/update.
- Send scheduled reminders, with multiple reminder offsets per event.
- Keep the data model ready for future integration without implementing automatic imports in V1.

## Non-Goals

- Do not build draft, review, approval, or publishing workflows.
- Do not build recurring events.
- Do not sync from academic years, semesters, admission rounds, work items, assessments, or timetable data.
- Do not make events change school-day calculations, timetable generation, attendance, assignments, or workflow deadlines.
- Do not expose private target details on the public calendar.
- Do not use a visual-heavy landing page. This is an operational calendar workspace.

## Architecture

Add a new backend module:

```text
backend-school/src/modules/calendar.rs
backend-school/src/modules/calendar/models.rs
backend-school/src/modules/calendar/services.rs
backend-school/src/modules/calendar/handlers.rs
```

Handlers remain thin:

```text
request context -> permission or self-view resolution -> service call -> API envelope
```

Services own validation, SQL, audience resolution, transactions, notification fan-out, and reminder dispatch helpers. They should receive `&PgPool` and return typed DTOs or outcome structs, not raw `serde_json::Value`.

Register routes in `main.rs` as a central module, not under `/api/academic`.

## Data Model

Add a new migration after the current migration sequence. Do not edit `001_baseline.sql` or any existing applied migration.

### `calendar_categories`

Fields:

- `id uuid primary key`
- `name varchar(120) not null`
- `color varchar(32) not null`
- `order_index integer not null`
- `is_active boolean not null default true`
- `created_at timestamptz not null default now()`
- `updated_at timestamptz not null default now()`

Rules:

- Category names should be unique case-insensitively among active categories.
- Color is a semantic/accent value used by the UI for dots, badges, borders, or light markers.

### `calendar_events`

Fields:

- `id uuid primary key`
- `category_id uuid references calendar_categories(id) on delete set null`
- `title varchar(200) not null`
- `description text`
- `location varchar(200)`
- `start_date date not null`
- `end_date date not null`
- `all_day boolean not null default true`
- `start_time time`
- `end_time time`
- `is_public boolean not null default false`
- `source_type varchar(50)`
- `source_id uuid`
- `created_by uuid references users(id) on delete set null`
- `updated_by uuid references users(id) on delete set null`
- `deleted_at timestamptz`
- `created_at timestamptz not null default now()`
- `updated_at timestamptz not null default now()`

Rules:

- `end_date >= start_date`.
- All-day events do not require `start_time` or `end_time`.
- Timed one-day events require `end_time > start_time`.
- Timed multi-day events may have `start_time` and `end_time`, interpreted as the start time on `start_date` and end time on `end_date`.
- `source_type/source_id` are nullable extension points only. V1-created events use null values.
- Delete uses soft delete through `deleted_at`.

### `calendar_event_targets`

Fields:

- `id uuid primary key`
- `event_id uuid not null references calendar_events(id) on delete cascade`
- `audience_type varchar(20) not null`
- `grade_level_id uuid references grade_levels(id) on delete cascade`
- `class_room_id uuid references class_rooms(id) on delete cascade`

Supported `audience_type` values:

- `all`
- `staff`
- `student`
- `parent`

Rules:

- `all` is for all authenticated user groups and must not include grade/classroom filters.
- `staff` must not include grade/classroom filters in V1.
- `student` and `parent` may include no filter, a grade-level filter, a classroom filter, or multiple target rows for multiple filters.
- A parent sees a class/grade-targeted event only when the event has a `parent` target and at least one of the parent's linked students matches the class/grade target.
- A student-targeted event alone does not make the event visible to parents.

### `calendar_event_reminders`

Fields:

- `id uuid primary key`
- `event_id uuid not null references calendar_events(id) on delete cascade`
- `days_before integer not null`
- `remind_on date not null`
- `sent_at timestamptz`
- `created_at timestamptz not null default now()`

Rules:

- Multiple reminders are allowed per event.
- Reminder offsets are day-based in V1. Examples: 7 days before, 3 days before, 1 day before.
- Reminder offsets must be positive integer day counts.
- `remind_on` is computed as `start_date - days_before`.
- V1 does not support hour/minute-precision reminders.
- Updating event date/time or reminder offsets replaces pending reminders in the same transaction. Already sent reminders remain sent and should not be resent.

### `calendar_tags` and `calendar_event_tags`

Tags extend the original V1 category model without changing its color semantics:

- `calendar_tags` contains a case-insensitively unique `name` (maximum 80 characters).
- `calendar_event_tags` is a many-to-many junction with `(event_id, tag_id)` as its primary key.
- An event keeps zero or one primary `category_id` and can additionally reference any number of tags.
- Deleting a category is a hard delete. The existing `calendar_events.category_id ... ON DELETE SET NULL` constraint preserves every event and changes affected events to “no category”.
- Deleting a tag is a hard delete. `calendar_event_tags.tag_id ... ON DELETE CASCADE` removes only tag associations; events remain unchanged.
- Deleting an event cascades its tag associations.

## API

Use the standard API envelope for all JSON responses.

### Staff Management API

```http
GET    /api/calendar/events
POST   /api/calendar/events
PUT    /api/calendar/events/{id}
DELETE /api/calendar/events/{id}
GET    /api/calendar/categories
POST   /api/calendar/categories
PUT    /api/calendar/categories/{id}
DELETE /api/calendar/categories/{id}
GET    /api/calendar/tags
POST   /api/calendar/tags
PUT    /api/calendar/tags/{id}
DELETE /api/calendar/tags/{id}
```

Authorization:

- `GET /api/calendar/events` and category reads require `calendar.read.school`.
- Mutations require `calendar.manage.school`.

List query:

```text
from=YYYY-MM-DD
to=YYYY-MM-DD
category_id=<uuid>
tag_id=<uuid>
audience=staff|student|parent|all
visibility=public|private
q=<search text>
```

Staff management list returns events in the selected date range and may include target/reminder summaries needed for editing.

Mutation payloads include event fields, target rows, reminder offsets, and notification intent:

```json
{
  "title": "สอบกลางภาค",
  "description": "รายละเอียด",
  "location": "อาคาร 1",
  "categoryId": "category-id",
  "tagIds": ["tag-id-1", "tag-id-2"],
  "startDate": "2026-07-03",
  "endDate": "2026-07-05",
  "allDay": true,
  "startTime": null,
  "endTime": null,
  "isPublic": true,
  "targets": [
    { "audienceType": "student", "gradeLevelId": "grade-id", "classRoomId": null },
    { "audienceType": "parent", "gradeLevelId": "grade-id", "classRoomId": null }
  ],
  "reminderOffsetsDays": [7, 3, 1],
  "notifyAudience": true
}
```

### Authenticated Viewer API

```http
GET /api/me/calendar/events?from=YYYY-MM-DD&to=YYYY-MM-DD&category_id=<uuid>&tag_id=<uuid>
```

This endpoint is for authenticated self-view and does not require calendar management permission.

Behavior:

- Staff users receive events targeted to `all` or `staff`.
- Student users receive events targeted to `all` or `student`, filtered by their active enrollment when grade/classroom targets are present.
- Parent users use the parent child route for child-specific views in V1. An aggregate parent calendar across all linked children is out of scope for V1.

### Parent Child API

```http
GET /api/parent/students/{student_id}/calendar/events?from=YYYY-MM-DD&to=YYYY-MM-DD&category_id=<uuid>&tag_id=<uuid>
```

Behavior:

- Verify the parent-child link.
- Return events targeted to `all` or `parent`.
- If the event has grade/classroom filters, the selected child must match at least one filter.
- Do not return student-only events unless parent was explicitly targeted.

### Public API

```http
GET /api/public/calendar/events?from=YYYY-MM-DD&to=YYYY-MM-DD&category_id=<uuid>&tag_id=<uuid>
```

Behavior:

- No authentication required.
- Return only non-deleted events with `is_public = true`.
- Do not expose internal target rows or private reminder data.
- Public visibility is explicit and independent from authenticated audience targeting.

## Permissions

Add backend permission constants and definitions:

```text
calendar.read.school
calendar.manage.school
```

Add frontend registry constants:

```ts
PERMISSION_MODULES.CALENDAR = 'calendar'
PERMISSIONS.CALENDAR_READ_SCHOOL = 'calendar.read.school'
PERMISSIONS.CALENDAR_MANAGE_SCHOOL = 'calendar.manage.school'
```

Expected use:

- Staff calendar menu route uses `PERMISSION_MODULES.CALENDAR`.
- Staff action controls use `PERMISSIONS.CALENDAR_MANAGE_SCHOOL`.
- Student and parent routes use user-type route metadata. Backend target filtering is the authority.
- Public routes do not use permission checks.

## Notification And Reminder Behavior

Use the existing `NotificationService` for in-app, SSE, and push delivery.

### Notify On Create/Update

When `notifyAudience = true`:

- Resolve target users after the event transaction commits.
- Send each target user a notification with a link to the appropriate calendar route.
- Staff targets link to `/staff/calendar`.
- Student targets link to `/student/calendar`.
- Parent targets link to `/parent/student/{id}/calendar` when the notification is child-specific.
- Deduplicate user IDs before sending.

If `notifyAudience = false`, the event is still published and visible in calendar views.

### Reminders

Add a calendar reminder scheduled job using the existing scheduler infrastructure.

Behavior:

- Run once per day in V1, at 07:00 tenant-local time.
- Iterate active tenant databases through `AdminClient::list_active_schools()` and `PoolManager::get_pool()`, following the existing file-cleaner job pattern.
- Find due reminders where `remind_on <= tenant_current_date` and `sent_at is null`.
- Resolve target users from the reminder's event and current event targets.
- Send notifications through `NotificationService`.
- Mark `sent_at` only after dispatch is attempted, inside a transaction or with idempotent locking to avoid duplicate sends.
- After creating or updating an event, process due reminders for that event immediately if any `remind_on` date is today or in the past. This avoids missing a reminder when staff create or edit an event after the daily reminder job has already run.

The implementation plan should choose the exact idempotent locking query, but duplicate reminders must be avoided.

### Future Reminder Precision

V1 intentionally uses day-based reminders to keep Neon tenant databases idle most of the day. The implementation must keep reminder calculation isolated in service helpers so a future self-hosted deployment can add more precise reminders without rewriting calendar events.

Future migration path:

- Add `offset_minutes integer`.
- Add `scheduled_at timestamptz`.
- Add `precision varchar(20)` with values such as `daily` and `datetime`.
- Keep existing `days_before` and `remind_on` for V1 daily reminders.
- Extend reminder query logic to process `precision = 'daily'` from `remind_on` and `precision = 'datetime'` from `scheduled_at`.

Frontend copy should use the neutral label "เตือนล่วงหน้า" while V1 controls expose day-based choices only.

## Frontend Routes

Staff management:

```text
/staff/calendar
```

Student self-view:

```text
/student/calendar
```

Parent child view:

```text
/parent/student/[id]/calendar
```

Public calendar:

```text
/calendar
```

All authenticated calendar pages use route-backed pages, not only in-memory tabs.

## Frontend UX

### Staff Calendar Workspace

Use `PageShell`.

Primary layout:

- Header with title, current month, and create button.
- Filter row: search, month selector, category, audience, public/private.
- Month grid.
- Event list for the selected month or selected day.
- Mobile stacks grid and list vertically.

Staff with `calendar.read.school` can inspect the workspace. Staff with `calendar.manage.school` see create/edit/delete/category controls.

### Create/Edit Event

Use shadcn-svelte `Dialog` for V1 create/edit. Do not build a custom drawer. A future pass can move the form to a shadcn-style sheet if the project adds that primitive.

Fields:

- title
- description
- location
- category
- date range
- all-day toggle
- optional start/end time
- audience selector: all/staff/student/parent
- grade level and classroom selectors for student/parent targets
- public checkbox
- notify audience checkbox
- reminder offset editor with multiple day-based offsets

The save action is "บันทึกและเผยแพร่" because V1 publishes immediately.

### Category Management

Use a dialog with:

- category list
- edit controls for name, color, order, active state
- destructive action for delete/deactivate

Color selection should be constrained to a clean palette suitable for event accents.

### Student, Parent, And Public Views

Use month grid plus event list.

Rules:

- No management controls.
- Student route calls `/api/me/calendar/events`.
- Parent child route calls `/api/parent/students/{student_id}/calendar/events`.
- Public route calls `/api/public/calendar/events`.
- Public route shows only public information: title, category, date/time, location, and description if public-safe.

## UI System And Color

Use local shadcn-svelte components as the default primitives:

- `Button`
- `Dialog`
- `Select`
- `Popover`
- `Checkbox`
- `Badge`
- `Table`
- `DatePicker`
- `PageShell`
- `PageState`
- `LoadingButton`

Do not hand-roll primitive controls with raw HTML/Tailwind unless a needed primitive is missing. If a primitive is missing, add or extend it under `frontend-school/src/lib/components/ui/` following the local shadcn-svelte pattern.

Color rules:

- Use primary color for create/save/confirm actions.
- Use destructive color for delete actions.
- Use secondary/outline/ghost variants for cancel, filter, and low-emphasis actions.
- Use event category colors as accents: dot, side border, badge, or light marker.
- Avoid large saturated color blocks.
- Use semantic colors for state feedback: success, warning, destructive, info.
- Keep the interface clean, operational, and readable. The color should communicate meaning, not decorate the page.

## Backend Implementation Notes

- Add `calendar` to module routing without creating `mod.rs`.
- Use `actor_tenant_context` for protected staff routes.
- Use `current_user_tenant_context_from_headers` or equivalent request context helpers for self-view routes.
- Use `tenant_context` for public tenant calendar routes when resolving the tenant from Origin/Referer or `X-School-Subdomain`.
- Add backend static architecture checks for the new module's handler/service boundaries.
- Services should include pure helper tests for:
  - date/time validation
  - target validation
  - reminder date calculation
  - audience user deduplication
  - parent visibility matching

## Frontend Implementation Notes

- Add a typed API client at `frontend-school/src/lib/api/calendar.ts`.
- Use concrete API response types, not `unknown` or raw `Record<string, unknown>` contracts.
- Update `frontend-school/src/lib/permissions/registry.ts`.
- Use route metadata:
  - `/staff/calendar`: real menu route with `PERMISSION_MODULES.CALENDAR`
  - `/student/calendar`: student user type route
  - `/parent/student/[id]/calendar`: parent user type route
  - `/calendar`: public route
- Initial route loads should fetch only data needed for the current page.
- Avoid full-page reloads after mutations; patch local event/category state from typed mutation responses.

## Testing

Backend:

- Service unit tests for validation and audience/reminder helper logic.
- Handler/service integration tests if project test utilities support tenant DB setup for the new tables.
- Static architecture tests for calendar handler/service boundaries and permission constants.
- `cargo check` for backend changes.

Frontend:

- Typecheck and static tests for permission registry/menu metadata alignment.
- Component/page tests where existing patterns support them.
- Playwright smoke path for login -> staff calendar -> create event -> visible in list can be added after the core route exists.

Repository-level:

- `git diff --check`
- `git status --short`

## Locked Implementation Decisions

- Create/edit uses `Dialog` in V1.
- The reminder scheduler runs once per day at 07:00 tenant-local time in V1.
- Category delete deactivates the category by setting `is_active = false`. Existing events keep their category reference for historical display.
