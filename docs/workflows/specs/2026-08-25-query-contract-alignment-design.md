# Query Contract Alignment Design

**Date:** 2026-08-25  
**Status:** Proposed  
**Scope:** Backend runtime query DTOs, generated OpenAPI, all frontend API query wrappers, and academic-context selection for the routes exposed by the Phase B cutover

## Problem

Phase B made `academicYearId` explicit on academic-year-scoped backend handlers. Several frontend wrappers still build query strings manually with legacy snake_case keys or omit the required year entirely. Production therefore rejects otherwise valid requests during Axum query deserialization, for example:

- `academic_year_id` is rejected where runtime accepts `academicYearId`;
- student and parent profile requests omit required `academicYearId`;
- the student list sends `page_size` and legacy filters that `ListStudentsQuery` rejects.

The current `check:api-contracts` task only proves that committed OpenAPI/generated artifacts match their exporters. It does not prove that:

1. a handler's runtime `Query<T>` is documented by that operation;
2. every runtime GET operation exists in OpenAPI;
3. a frontend wrapper emits the generated operation's query names.

This is why regenerating the contract can pass while production still returns HTTP 400.

## Goals

- Make the backend query DTO the single source of truth for runtime parsing and OpenAPI query parameters.
- Make every frontend API wrapper with query parameters consume generated operation query types instead of handwritten query-key strings.
- Give each user role a clear, authorized way to select the academic year used by profile/list requests.
- Add semantic contract tests that fail before a camelCase/snake_case or missing-required-query regression reaches production.
- Remove the affected legacy query behavior outright; do not add compatibility aliases.

## Non-goals

- Renaming every existing response field from snake_case to camelCase in this release.
- Moving student or parent pages into the staff global Academic Context.
- Changing academic-year data, promotion rules, permissions, or database schema.
- Auditing every API response body in the application; this initiative closes the application-wide query-contract gap and the directly exposed student-list response mismatch.

## Design decisions

### 1. Runtime DTOs own query contracts

Every affected Axum query struct derives `Deserialize` and `utoipa::IntoParams`, uses `#[serde(rename_all = "camelCase", deny_unknown_fields)]`, and is referenced directly from the handler's `#[utoipa::path(params(...))]` annotation.

The release will:

- document `StudentAcademicYearQuery` on student and parent profile operations;
- document `ListStudentsQuery` on `GET /api/students`;
- document `CalendarEventQuery` on all four calendar-list operations;
- retain explicit path parameters only where an operation also has a path ID;
- register the missing `GET /api/students` and `GET /api/students/{id}` operations and their schemas in the OpenAPI exporter;
- remove stale manual calendar query declarations such as `category_id` and `tag_id`.

There will be no `academic_year_id` alias and no dual parsing. The public query name is `academicYearId`.

### 2. Frontend serializes typed query objects centrally

`apiClient.get` will accept an optional query object and serialize defined scalar values through one shared implementation. Callers will stop concatenating affected query strings manually.

Each wrapper will type its query from the generated `operations` contract, for example the generated query type for `lookupGradeLevels` or `listStudents`. TypeScript must therefore reject a wrapper that tries to send `academic_year_id`, `page_size`, or a removed filter.

The wrapper remains the UI-facing boundary: pages pass selected IDs to a named function, while the wrapper maps those values into the generated query type and delegates serialization to `apiClient`.

The first implementation checkpoint migrates the Phase B paths that are producing production errors. The second checkpoint inventories and migrates every remaining query-bearing wrapper under `frontend-school/src/lib/api`. Browser navigation query strings in Svelte routes are not API requests and remain owned by route-context code.

If a migrated wrapper's operation or query parameters are missing from OpenAPI, the backend annotation/exporter must be corrected before the wrapper is converted. A handwritten local substitute for a missing generated query type is not an acceptable endpoint state.

### 3. OpenAPI response types match the student list wire shape

`StudentListItem` and `StudentListResponse` will be registered as OpenAPI schemas so the frontend no longer carries a handwritten list-item type that expects `class_room` while the backend sends `homeroom`.

This release preserves the existing snake_case response shape (`first_name`, `page_size`, and related fields) and documents it exactly. Query names are standardized independently. Pagination semantics beyond the backend's current response are not expanded in this hotfix; the UI must not invent `total` or `total_pages` fields that are absent from the contract.

### 4. Academic context follows authorization boundaries

The staff Topbar Academic Context remains staff-only. It must not become a global selector for learner or parent routes.

- Staff student-list/detail/edit routes declare `year_required` and read the selected year from the existing staff Academic Context.
- Student home/profile routes load authorized choices from `/api/me/academic-context/options`, keep the selected `academicYearId` in the URL, and pass it to profile requests.
- Parent child detail/timetable routes load choices from `/api/parent/students/{studentId}/academic-context/options`, keep the selected year in the URL, and pass it to child-profile requests.
- Parent home needs choices across linked children. Add `/api/parent/academic-context/options`, returning the union of years/terms for the signed-in parent's active linked students. The page selects one authorized year and passes it to `/api/parent/profile`.

The parent endpoint prevents two confusing alternatives: exposing staff-wide/public years that have no linked child data, or silently guessing the active year. Empty options render an explicit no-academic-record state rather than issuing an invalid profile request.

All pages that can change context use the existing request-revision/abort pattern so a slower response from the previous year cannot overwrite the current selection.

### 5. Semantic contract guards

The release adds checks at both boundaries:

1. Backend OpenAPI tests inspect the exported document and assert the exact required/optional query parameters for the affected operations. Because handler annotations reference the query DTO via `IntoParams`, runtime parsing and documentation share the same Rust type.
2. Frontend wrapper tests run the real wrappers against a request recorder and assert the emitted method, path, and query names. They cover required years, optional terms/filters, omitted undefined values, and camelCase pagination.
3. A frontend contract test rejects new manual API query construction under `src/lib/api` after the migration is complete. Explicit non-backend URL construction, if any, needs a narrow reviewed allow-list entry.
4. Generated contract checks continue to verify that OpenAPI and TypeScript artifacts are committed and current.

Behavioral tests focus on executable wrappers. The static rule has only one small responsibility: preventing raw query-string construction from bypassing those typed wrappers again. It does not attempt to infer endpoint behavior from arbitrary source text.

## Affected routes

The production paths directly covered by this release are:

- `/staff/academic/homerooms`
- `/staff/academic/exam-schedules/[id]`
- `/staff/academic/student-years`
- `/staff/calendar`
- `/staff/students`
- `/staff/students/[id]`
- `/staff/students/[id]/edit`
- `/student`
- `/student/profile`
- `/parent`
- `/parent/student/[id]`
- `/parent/student/[id]/timetable`

Calendar profile/list wrappers used by these pages are included even when a page has not yet reproduced the same exact error, because they share the incorrect contract boundary.

The application-wide closure checkpoint also covers the remaining query builders in `frontend-school/src/lib/api`, including facility, supervision, achievement, staff, student activity, role, file, timetable, certificate, question-bank, work, admission, and other wrapper modules found by the committed inventory test. This broader migration prevents the same defect class from surviving behind a route that has not yet been opened in production.

## Request flow

1. The route resolves an authorized academic context for its role.
2. The URL stores the selected `academicYearId` and optional `academicTermId` where applicable.
3. The page calls a domain wrapper with the selected IDs.
4. The wrapper constructs a generated operation query type.
5. `apiClient` serializes defined fields using their generated camelCase names.
6. Axum deserializes the same names into the DTO documented by OpenAPI.

## Failure behavior

- Missing authorized academic contexts produce a user-readable empty state, not a request with a missing required query.
- A URL containing an unauthorized or unavailable year is normalized to the role-scoped default and replaced in the URL.
- Unknown frontend query keys fail TypeScript during development.
- Contract-name drift fails backend/frontend tests and generated-artifact checks in CI.
- API errors continue through the existing `ApiResponse`/`ApiClientError` behavior; no silent fallback to a legacy query name is allowed.

## Delivery and compatibility

No database migration or data move is required. The runtime backend already accepts the intended camelCase query names; most backend work makes that runtime truth visible in OpenAPI and adds the parent-scoped options endpoint.

Implementation is delivered in two reviewable checkpoints:

1. **Phase B repair:** correct OpenAPI coverage, add typed query serialization, fix the listed production routes, add role-scoped academic selection, and deploy after focused verification.
2. **Application-wide closure:** migrate the remaining API query wrappers, correct any OpenAPI omissions they expose, enable the no-manual-query guard, regenerate contracts, and deploy after the full contract suite.

The defect class is considered closed only after checkpoint 2. Each checkpoint remains deployable; backend and frontend changes within a checkpoint can ship in one push because:

- the corrected frontend is compatible with the current backend runtime;
- the documented backend query behavior does not invalidate already-correct clients;
- the legacy broken requests remain intentionally unsupported.

Deployment verification must exercise at least one staff, student, and parent session and confirm that changing the selected year updates the URL and resulting data without HTTP 400 responses.

## Verification matrix

Per `.rules`, this change requires serial execution of the relevant backend, frontend, contract, and documentation checks. At minimum:

- focused Rust unit/OpenAPI tests for the changed handlers and parent authorization;
- focused frontend API-wrapper and academic-context route tests;
- frontend type checking and affected Svelte checks;
- OpenAPI export plus generated frontend contract verification;
- repository documentation/link checks;
- production smoke verification after deployment.

Commands must be run one at a time on this workspace to avoid resource contention.
