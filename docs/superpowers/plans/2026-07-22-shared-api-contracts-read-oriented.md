# Read-Oriented Backend-School API Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. The approved execution mode for this rollout is inline/sequential; do not delegate implementation.

**Goal:** Extend the backend-owned OpenAPI contract from 30 auth/authorization operations to 66 operations by documenting the 36 implemented read-oriented backend-school JSON endpoints and replacing matching handwritten frontend wire DTOs with generated aliases.

**Architecture:** Rust serde DTOs and Axum handlers remain authoritative. Each batch first proves its route/schema inventory is absent, then adds exact `utoipa` metadata, regenerates `school-api.json` and TypeScript, and migrates only frontend transport types that exactly match the wire. Domain/view models and explicit mappers remain handwritten.

**Constraints:** No route, permission, status, response, or database behavior changes. Do not document SSE, WebSocket, health, file/binary, or mutation operations in this phase. Error responses must be listed only when the current handler/service can emit that status through `AppError`. Optional-versus-nullable fields must follow actual serde serialization.

## Inventory (36 operations)

### Batch A — lookup, user menu, and administration reads (15)

- `GET /api/menu/user`
- `GET /api/admin/features`
- `GET /api/admin/features/{id}`
- `GET /api/admin/menu/groups`
- `GET /api/admin/menu/items`
- `GET /api/lookup/staff`
- `GET /api/lookup/students`
- `GET /api/lookup/rooms`
- `GET /api/lookup/roles`
- `GET /api/lookup/organization-units`
- `GET /api/lookup/organization-units/{id}`
- `GET /api/lookup/grade-levels`
- `GET /api/lookup/classrooms`
- `GET /api/lookup/academic-years`
- `GET /api/lookup/subjects`

### Batch B — staff and self-service reads (14)

- `GET /api/staff`
- `GET /api/staff/dashboard`
- `GET /api/staff/{id}`
- `GET /api/staff/{id}/public-profile`
- `GET /api/student/profile`
- `GET /api/parent/profile`
- `GET /api/parent/students/{student_id}`
- `GET /api/parent/students/{student_id}/timetable`
- `GET /api/parent/students/{student_id}/exam-schedules`
- `GET /api/parent/students/{student_id}/calendar/events`
- `GET /api/me/timetable`
- `GET /api/me/exam-schedules`
- `GET /api/staff/exam-schedules`
- `GET /api/me/calendar/events`

### Batch C — calendar, school, and notification reads (7)

- `GET /api/public/calendar/events`
- `GET /api/calendar/events`
- `GET /api/calendar/categories`
- `GET /api/calendar/tags`
- `GET /api/school/public`
- `GET /api/school/settings`
- `GET /api/notifications`

## Task 1 — Strengthen router coverage for phased expansion

**Files:** `backend-school/tests/static_architecture.rs`, `backend-school/src/api_contract.rs`

1. Add a failing fixture test showing the router scanner finds nested and direct read-oriented registrations and reports a handler omitted from `#[openapi(paths(...))]`.
2. Generalize the existing auth/authorization scanner into a phased JSON handler scanner without requiring a hard-coded handler list.
3. Keep protocol exclusions explicit; do not interpret SSE/WebSocket/binary handlers as JSON operations.
4. Verify the current 30-operation contract remains green before adding Batch A.

## Task 2 — Batch A contracts and frontend aliases

**Files:** lookup/menu/system models and handlers, `backend-school/src/api_contract.rs`, matching frontend API modules, contract tests.

1. Add RED assertions for all 15 method/path/operationId triples, UUID/query parameters, envelope bodies, required fields, and serde omission/nullability.
2. Derive `ToSchema` on the exact request/query/response DTOs. Introduce a named response DTO only where a handler currently returns a private/local struct or anonymous JSON shape; do not change serialization.
3. Add path metadata and register all handlers.
4. Regenerate artifacts and replace matching lookup/menu/system handwritten wire DTOs with generated schema aliases.
5. Run focused Rust contract tests, router guard, generator tests/check, frontend static tests, and `svelte-check`.
6. Commit the green batch.

## Task 3 — Batch B contracts and frontend aliases

**Files:** staff/student/parent/academic/calendar handlers and models, `backend-school/src/api_contract.rs`, matching frontend API modules, contract tests.

1. Add RED inventory and transport-shape tests for all 14 operations.
2. Verify every handler's actual success envelope and service error paths before annotating statuses. Treat paginated staff responses and self/parent timetable variants as distinct schemas where their wire shapes differ.
3. Preserve PDPA rules: encrypted `national_id` storage is unchanged and plaintext identifiers are never added to logs or newly exposed responses.
4. Register the operations and exact DTO schemas, regenerate artifacts, and migrate exact frontend wire duplicates.
5. Run focused backend/frontend verification and commit the green batch.

## Task 4 — Batch C contracts and frontend aliases

**Files:** calendar/school/notification models and handlers, `backend-school/src/api_contract.rs`, matching frontend API modules, contract tests.

1. Add RED assertions for all seven operations, including calendar query parameters and list response item types.
2. Document JSON list operations only. Keep `/api/notifications/stream` excluded as SSE and keep all calendar/school/notification mutations for Phase 4.
3. Derive/register exact schemas and statuses, regenerate artifacts, and migrate exact frontend wire duplicates.
4. Run focused verification and commit the green batch.

## Task 5 — Phase inventory, documentation, and review

1. Assert exactly 66 unique operations in `school-api.json` (30 existing + 36 new) and verify all 36 read-oriented registrations are covered from router sources.
2. Update `.rules`, `docs/TESTING.md`, `docs/backend-school/API_DEVELOPMENT.md`, and `IMPROVEMENT_PLAN.md` with the 66-operation checkpoint and remaining Phase 4 scope.
3. Run full backend formatting/check/clippy/static/unit tests, deterministic generation/check/tests, frontend type/static/build checks, and environment-backed smoke tests only if their variables are available.
4. Request an independent read-only code review; fix every Critical/Important finding with regression tests and repeat verification.

## Completion evidence

- OpenAPI and generated TypeScript contain 66 unique operations.
- All tracked generated files are deterministic and clean under `check:api-contracts`.
- Frontend consumers no longer use handwritten duplicates for any migrated wire DTO.
- SSE/WebSocket/binary/mutation exclusions remain absent and documented.
- The router-derived drift guard fails on a fixture with an undocumented phased JSON handler.
- Worktree is clean after phase commits and review fixes.
