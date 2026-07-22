# Academic Structure Mutation Contracts Implementation Plan

> **Execution:** Follow `superpowers:executing-plans` task-by-task in an isolated worktree. Apply `superpowers:test-driven-development` to every behavior correction and contract guard.

**Goal:** Secure and export the first Academic workflow batch: structure, years, semesters, classrooms, year-level configuration, and enrollment administration.

**Architecture:** Rust serde DTOs plus `utoipa` handler metadata remain the wire source of truth. Academic handlers load one actor/tenant context, enforce the same generated permission codes used by the frontend, delegate SQL to `academic_structure_service`, and return `ApiResponse<T>`. The generated TypeScript contract replaces handwritten transport DTOs while Svelte pages keep explicit local UI state.

**Batch size:** 19 operations: 15 mutations and four dependent reads. This moves the generated checkpoint from 81 to 100 unique operations.

## Operation inventory

| Method | Resolved path | Handler | Permission | Success data |
|---|---|---|---|---|
| GET | `/api/academic/structure` | `list_academic_structure` | `ACADEMIC_STRUCTURE_READ_ALL` | `AcademicStructure` |
| POST | `/api/academic/levels` | `create_grade_level` | `ACADEMIC_STRUCTURE_MANAGE_ALL` | `GradeLevelResponse` (201) |
| DELETE | `/api/academic/levels/{id}` | `delete_grade_level` | `ACADEMIC_STRUCTURE_MANAGE_ALL` | `EmptyData` |
| POST | `/api/academic/years` | `create_academic_year` | `ACADEMIC_STRUCTURE_MANAGE_ALL` | `AcademicYear` (201) |
| PUT | `/api/academic/years/{id}` | `update_academic_year` | `ACADEMIC_STRUCTURE_MANAGE_ALL` | `AcademicYear` |
| PUT | `/api/academic/years/{id}/active` | `toggle_active_year` | `ACADEMIC_STRUCTURE_MANAGE_ALL` | `EmptyData` |
| GET | `/api/academic/years/{id}/levels` | `get_year_levels` | `ACADEMIC_STRUCTURE_READ_ALL` | `Vec<Uuid>` |
| PUT | `/api/academic/years/{id}/levels` | `update_year_levels` | `ACADEMIC_STRUCTURE_MANAGE_ALL` | `EmptyData` |
| POST | `/api/academic/semesters` | `create_semester` | `ACADEMIC_STRUCTURE_MANAGE_ALL` | `Semester` (201) |
| PUT | `/api/academic/semesters/{id}` | `update_semester` | `ACADEMIC_STRUCTURE_MANAGE_ALL` | `Semester` |
| DELETE | `/api/academic/semesters/{id}` | `delete_semester` | `ACADEMIC_STRUCTURE_MANAGE_ALL` | `EmptyData` |
| GET | `/api/academic/classrooms` | `list_classrooms` | `ACADEMIC_CLASSROOM_READ_ALL` | `Vec<Classroom>` |
| POST | `/api/academic/classrooms` | `create_classroom` | `ACADEMIC_CLASSROOM_CREATE_ALL` | `Classroom` (201) |
| PUT | `/api/academic/classrooms/{id}` | `update_classroom` | `ACADEMIC_CLASSROOM_UPDATE_ALL` | `Classroom` |
| POST | `/api/academic/enrollments` | `enroll_students` | `ACADEMIC_ENROLLMENT_UPDATE_ALL` | `EmptyData` |
| GET | `/api/academic/enrollments/class/{id}` | `get_class_enrollments` | `ACADEMIC_ENROLLMENT_READ_ALL` | `Vec<StudentEnrollment>` |
| DELETE | `/api/academic/enrollments/{id}` | `remove_enrollment` | `ACADEMIC_ENROLLMENT_UPDATE_ALL` | `EmptyData` |
| PUT | `/api/academic/enrollments/{id}/number` | `update_enrollment_number` | `ACADEMIC_ENROLLMENT_UPDATE_ALL` | `EmptyData` |
| POST | `/api/academic/enrollments/class/{id}/auto-number` | `auto_assign_class_numbers` | `ACADEMIC_ENROLLMENT_UPDATE_ALL` | `EmptyData` |

All operations require the existing authentication middleware and tenant resolution. They may return the standard 401/403/500 envelope; typed path operations also document 400 and 404 where the service can emit them. This batch has no file, SSE, WebSocket, migration, or PII-encryption change.

## Task 1: Lock down academic structure authorization

**Files:**

- Modify: `backend-school/src/modules/academic/handlers.rs`
- Modify: `backend-school/tests/static_architecture.rs`

1. Add a failing static regression test proving all 19 handlers load `actor_tenant_context` and require the exact permission shown above.
2. Replace direct `tenant_pool` access with a single actor/tenant context per handler.
3. Enforce read/manage/create/update permissions before every service call.
4. Run the focused static test, formatter, and Clippy.
5. Commit as `fix(academic): enforce structure mutation permissions`.

## Task 2: Correct missing-target behavior

**Files:**

- Modify: `backend-school/src/modules/academic/services/academic_structure_service.rs`
- Create: `backend-school/src/modules/academic/services/academic_structure_service_tests.rs`
- Modify: `backend-school/src/modules/academic/services.rs`

1. Add database-backed failing tests for toggling a missing academic year, deleting a missing semester/grade level/enrollment, updating a missing enrollment number, and replacing levels for a missing year.
2. Return `AppError::NotFound` without changing other rows when the target is absent.
3. Preserve transactional behavior and bulk helpers; do not edit migrations.
4. Run focused tests with `TEST_DATABASE_URL` and commit as `fix(academic): reject missing structure targets`.

## Task 3: Export the 19 backend operations

**Files:**

- Modify: `backend-school/src/modules/academic/handlers.rs`
- Modify: `backend-school/src/modules/academic/models.rs`
- Modify: `backend-school/src/modules/academic/services/academic_structure_service.rs`
- Modify: `backend-school/src/api_contract.rs`

1. Add failing `api_contract::tests` assertions for all method/path pairs, unique operation IDs, request schemas, typed success envelopes, and the 100-operation total.
2. Derive `ToSchema` for stable request/response DTOs. Represent `Classroom.advisors` as `Vec<ClassroomAdvisor>` at the OpenAPI boundary and expose handler-local request DTOs.
3. Add `utoipa::path` metadata with actual response codes and security requirements.
4. Register all paths and component schemas in `SchoolApiDoc`.
5. Run focused contract tests, format, Clippy, and commit as `feat(api): export academic structure contracts`.

## Task 4: Generate frontend types and remove duplicate wire DTOs

**Files:**

- Regenerate: `contracts/school-api.json`
- Regenerate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/academic.ts`
- Create: `frontend-school/tests/static/academic-structure-mutation-contract.test.mjs`
- Modify: relevant static API response tests when needed

1. Add failing static assertions for the 19 generated operations and exact API wrapper paths/methods.
2. Import `Schemas` and replace handwritten academic structure, classroom, enrollment, request, and `Record<string, never>` empty transport types with stable generated aliases.
3. Preserve public API names used by pages; use explicit mappers only if a UI view differs from the wire type.
4. Regenerate contracts, run focused Node tests, API staleness checks, and TypeScript/Svelte checks.
5. Commit as `feat(frontend): type academic structure mutations`.

## Task 5: Verify, review, document, merge, and push

**Files:**

- Modify: `.rules`
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `docs/API_DEVELOPMENT.md`
- Modify: `docs/TESTING.md`

1. Record the 100-operation checkpoint, permission correction, missing-target behavior, and the next Academic sub-batch (curriculum and study plans).
2. Run the backend gate: format, Clippy, contract tests, static architecture, and full database-backed unit suite.
3. Run the frontend gate: generated contract/permission checks, API/static tests, and `svelte-check` with required public env placeholders.
4. Audit the diff for migrations, plaintext sensitive data, duplicate DTOs, route/operation mismatches, and operation-ID uniqueness.
5. Review against this plan, address verified findings, rerun affected checks, fast-forward merge to `main`, push `origin/main`, and verify `main...origin/main` is `0 0`.

