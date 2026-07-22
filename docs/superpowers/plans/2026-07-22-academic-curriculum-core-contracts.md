# Academic Curriculum Core Contracts Implementation Plan

> **Execution:** Follow `superpowers:executing-plans` task-by-task in an isolated worktree. Apply `superpowers:test-driven-development` to permission and missing-target corrections.

**Goal:** Secure and export the Subjects and Core Study Plans workflow so backend authorization, frontend permission gates, Rust DTOs, OpenAPI, and generated TypeScript describe the same behavior.

**Architecture:** Rust serde DTOs plus `utoipa` handler metadata remain the wire source of truth. Subject handlers keep the existing resource-aware `curriculum_access_policy`; study-plan handlers load one actor/tenant context and use the matching curriculum permission family. Course generation uses `ACADEMIC_COURSE_PLAN_MANAGE_ALL` because it mutates classroom courses. SQL stays in `subject_service` and `study_plan_service`, with missing targets returning `AppError::NotFound` and bulk counts reporting real inserts.

**Batch size:** 24 operations: 15 mutations and nine dependent reads. This moves the generated checkpoint from 100 to 124 unique operations.

## Operation inventory

| Method | Resolved path | Handler | Permission / policy | Success data |
|---|---|---|---|---|
| GET | `/api/academic/subjects/groups` | `list_subject_groups` | curriculum read policy | `Vec<SubjectGroup>` |
| GET | `/api/academic/subjects/default-instructors` | `batch_list_subject_default_instructors` | curriculum read policy | instructor map |
| GET | `/api/academic/subjects` | `list_subjects` | curriculum read policy | `Vec<Subject>` |
| POST | `/api/academic/subjects` | `create_subject` | curriculum create policy | `Subject` (201) |
| PUT | `/api/academic/subjects/{id}` | `update_subject` | curriculum update policy | `Subject` |
| DELETE | `/api/academic/subjects/{id}` | `delete_subject` | curriculum delete policy | `EmptyData` |
| GET | `/api/academic/subjects/{id}/default-instructors` | `list_subject_default_instructors` | curriculum update policy for target | `Vec<SubjectDefaultInstructor>` |
| POST | `/api/academic/subjects/{id}/default-instructors` | `add_subject_default_instructor` | curriculum update policy for target | `EmptyData` |
| DELETE | `/api/academic/subjects/{id}/default-instructors/{uid}` | `remove_subject_default_instructor` | curriculum update policy for target | `EmptyData` |
| PUT | `/api/academic/subjects/{id}/default-instructors/{uid}` | `update_subject_default_instructor_role` | curriculum update policy for target | `EmptyData` |
| GET | `/api/academic/study-plans` | `list_study_plans` | curriculum read policy | `Vec<StudyPlan>` |
| POST | `/api/academic/study-plans` | `create_study_plan` | curriculum create policy | `StudyPlan` (201) |
| GET | `/api/academic/study-plans/{id}` | `get_study_plan` | curriculum read policy | `StudyPlan` |
| PUT | `/api/academic/study-plans/{id}` | `update_study_plan` | curriculum update policy | `StudyPlan` |
| DELETE | `/api/academic/study-plans/{id}` | `delete_study_plan` | curriculum delete policy | `EmptyData` |
| GET | `/api/academic/study-plan-versions` | `list_study_plan_versions` | curriculum read policy | `Vec<StudyPlanVersion>` |
| POST | `/api/academic/study-plan-versions` | `create_study_plan_version` | curriculum create policy | `StudyPlanVersion` (201) |
| GET | `/api/academic/study-plan-versions/{id}` | `get_study_plan_version` | curriculum read policy | `StudyPlanVersion` |
| PUT | `/api/academic/study-plan-versions/{id}` | `update_study_plan_version` | curriculum update policy | `StudyPlanVersion` |
| DELETE | `/api/academic/study-plan-versions/{id}` | `delete_study_plan_version` | curriculum delete policy | `EmptyData` |
| GET | `/api/academic/study-plan-versions/{id}/subjects` | `list_study_plan_subjects` | curriculum read policy | `Vec<StudyPlanSubject>` |
| POST | `/api/academic/study-plan-versions/{id}/subjects` | `add_subjects_to_version` | curriculum update policy | `CountData` |
| DELETE | `/api/academic/study-plan-subjects/{id}` | `delete_study_plan_subject` | curriculum delete policy | `EmptyData` |
| POST | `/api/academic/planning/generate-from-plan` | `generate_courses_from_plan` | `ACADEMIC_COURSE_PLAN_MANAGE_ALL` | `GenerateCoursesData` |

All operations require authentication and tenant resolution. This batch has no migration, file, SSE, WebSocket, or PII-encryption change.

## Task 1: Lock down study-plan authorization

**Files:**

- Modify: `backend-school/src/modules/academic/handlers/study_plans.rs`
- Modify: `backend-school/src/policies/curriculum_access_policy.rs`
- Modify: `backend-school/tests/static_architecture.rs`

1. Add a failing static regression proving the 14 core study-plan handlers load `actor_tenant_context` and enforce the permission family shown above.
2. Add policy helpers for read/create/update/delete decisions so organization-scoped permissions accepted by the frontend remain accepted by the backend.
3. Replace `tenant_pool` and optional header identity with one authenticated actor/tenant context; pass `actor.user_id` to course generation.
4. Run focused static/policy tests, format, and Clippy.
5. Commit as `fix(academic): enforce curriculum plan permissions`.

## Task 2: Correct subject and study-plan target behavior

**Files:**

- Modify: `backend-school/src/modules/academic/services/subject_service.rs`
- Modify: `backend-school/src/modules/academic/services/study_plan_service.rs`
- Create or modify focused database-backed service tests

1. Add failing tests for missing subjects, instructor assignments, study plans, versions, and plan-subject rows.
2. Ensure an invalid instructor-role update cannot demote an existing primary assignment before discovering the target is absent.
3. Make list queries propagate database errors instead of silently returning empty arrays.
4. Validate parent plan/version targets before child mutation and report actual inserted count for conflict-skipped plan subjects.
5. Preserve transactions and bulk helpers; do not edit migrations.
6. Run focused database-backed tests and commit as `fix(academic): reject missing curriculum targets`.

## Task 3: Export all 24 backend operations

**Files:**

- Modify: `backend-school/src/modules/academic/handlers/subjects.rs`
- Modify: `backend-school/src/modules/academic/handlers/study_plans.rs`
- Modify: `backend-school/src/modules/academic/models/curriculum.rs`
- Modify: `backend-school/src/modules/academic/models/study_plans.rs`
- Modify: `backend-school/src/api_contract.rs`

1. Add failing contract assertions for all method/path pairs, unique operation IDs, request schemas, typed success envelopes, and the 124-operation total.
2. Derive `ToSchema` for stable DTOs and define schema-only enums where wire values are bounded.
3. Add `utoipa::path` metadata with real status codes and standard error envelopes.
4. Register paths and schemas in `SchoolApiDoc` and regenerate the contract.
5. Run focused contract tests, format, and Clippy.
6. Commit as `feat(api): export curriculum core contracts`.

## Task 4: Generate frontend types and remove duplicate wire DTOs

**Files:**

- Regenerate: `contracts/openapi/school-api.json`
- Regenerate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/academic.ts`
- Create: `frontend-school/tests/static/academic-curriculum-core-contract.test.mjs`
- Modify: contract checkpoint tests and relevant Svelte consumers only when generated optionality reveals a real mismatch

1. Add failing static assertions for the 24 generated operations and exact wrapper paths/methods.
2. Replace handwritten subject/study-plan wire DTOs and request shapes with generated schema aliases while preserving public API names.
3. Regenerate contracts and run focused Node tests, API staleness checks, TypeScript/Svelte checks, and Svelte autofixer for every edited `.svelte` file.
4. Commit as `feat(frontend): type curriculum core mutations`.

## Task 5: Verify, document, merge, and push

**Files:**

- Modify: `.rules`
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `docs/backend-school/API_DEVELOPMENT.md`
- Modify: `docs/TESTING.md`

1. Record the 124-operation checkpoint, permission correction, missing-target behavior, and the next Academic batch (plan activities and activity catalog).
2. Run backend format, Clippy, contract, static architecture, and full database-backed tests.
3. Run frontend generated-contract/permission checks, all static tests, and `svelte-check`.
4. Audit migrations, sensitive fields/secrets, generated staleness, operation-ID uniqueness, route mappings, and duplicate handwritten DTOs.
5. Review against this plan, rerun affected checks, fast-forward merge to `main`, push `origin/main`, verify `main...origin/main` is `0 0`, and remove the owned worktree/branch.
