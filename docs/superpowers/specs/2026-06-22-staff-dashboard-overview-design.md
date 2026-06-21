# Staff Dashboard Overview Design

## Context

The `/staff` route is the staff landing dashboard. It is not a replacement for `/staff/manage`, which remains the permission-gated staff directory and management workspace.

The current `/staff` page shows placeholder counts (`totalStaff`, `totalStudents`, `activeClasses`) and static dashboard blocks. The new design should make those numbers real and improve the page layout while keeping the page accessible to every authenticated staff user.

User decision: every teacher/staff user may see school-wide aggregate counts such as total staff and total students. This permission does not imply access to staff or student lists, profile detail, PII, contact information, or management actions.

## Goals

- Show real school-wide aggregate counts on `/staff`.
- Keep `/staff` available to all authenticated staff users.
- Preserve existing list/profile permissions for `/api/staff`, `/api/students`, and management routes.
- Improve the page as an operational overview with clear shortcuts and readable state handling.
- Avoid exposing personal data beyond aggregate counts.

## Non-Goals

- Do not replace `/staff/manage` with a directory-first page.
- Do not open staff or student list APIs to every staff user.
- Do not expose names, national IDs, contact details, class rosters, or per-person records through the dashboard endpoint.
- Do not add analytics charts or recent activity feeds unless backed by an existing safe data contract.

## Approach

Add a dedicated aggregate endpoint for the staff dashboard:

```http
GET /api/staff/dashboard
```

The endpoint is authenticated and tenant-aware. It verifies the current user is an active staff user, then returns only aggregate school counts.

Initial response shape:

```json
{
  "success": true,
  "data": {
    "totalStaff": 84,
    "totalStudents": 1248,
    "activeClassrooms": 42
  }
}
```

The endpoint uses camelCase at the API boundary and should be consumed through a typed frontend API function.

## Backend Design

Add a small staff dashboard service under the staff module, for example `backend-school/src/modules/staff/services/dashboard_service.rs`.

The service owns the SQL and returns a typed response struct. Add a small pure mapper/helper for the DB count row so the new service file has meaningful service-level unit coverage. It counts:

- active staff users: `users.user_type = 'staff' AND users.status = 'active'`
- active student users: `users.user_type = 'student' AND users.status = 'active'`
- active classrooms: `class_rooms.is_active = true`

The handler should follow the existing request context pattern:

1. Load authenticated actor tenant context.
2. Verify the actor is a staff user or otherwise reject the request.
3. Call the service.
4. Return `ApiResponse::ok(data)`.

No raw `serde_json::Value` should be used for the known response shape. No PII should be selected or logged.

## Frontend Design

Add a typed dashboard API function in the existing staff API client.

The `/staff` page should prefer route loading for the dashboard data if the authenticated backend cookie/header path works reliably in this route. If the current app auth pattern makes server-side route loading unreliable, use the existing client API pattern with a localized stats loading state. The dashboard endpoint is available to every active staff user, so this request does not depend on action permissions. The page should:

- show a loading state for the stats area while the dashboard request is pending
- show `PageState` or a compact error state if the dashboard request fails
- show the three real aggregate cards when loaded
- keep quick actions filtered by permissions using the existing `can` store
- keep account/settings and self timetable shortcuts visible where appropriate

The visual direction is "Operational Overview": a clean dashboard with a stat row, grouped quick actions, and a small system/workspace panel. It should use existing `PageShell`, shadcn-svelte UI primitives, and app-state components where they fit.

## Permission And Privacy Model

School-wide counts are intentionally visible to all authenticated staff users. This is a separate data contract from list/read permissions.

The dashboard endpoint must not reuse list endpoints to calculate counts in the frontend, because that would either:

- expose list records to users who only need aggregate counts, or
- require hidden admin-only calls that make the page fail for ordinary teachers.

Existing staff/student profile and list permissions remain unchanged.

## Error Handling

Backend errors should return the standard API envelope. The handler should not panic or unwrap database results.

Frontend errors should not break the whole staff landing page. If the dashboard stats fail to load, the rest of the shortcut workspace remains usable and the stats area shows a clear retry/error state.

## Testing

Backend:

- Add focused service-level unit coverage for the pure helper that maps dashboard count rows into the API response.
- Add or run a focused backend check for the new staff dashboard module.
- Run `cargo check` for backend changes.

Frontend:

- Add or update a focused frontend test if the project already has a suitable pattern for page/API behavior.
- Run frontend static/type checks for touched TypeScript/Svelte code.

Repository-level:

- Run `git diff --check`.
- Keep `git status --short` limited to the intended implementation files before final handoff.

## Open Decisions

No unresolved product decisions remain for the first iteration. Additional metrics such as attendance, pending approvals, or recent activity require separate safe data contracts and should not be added in this change.
