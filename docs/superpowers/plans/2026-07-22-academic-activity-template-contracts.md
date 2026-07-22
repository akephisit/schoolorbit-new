# Academic Activity Template Contracts Implementation Plan

> **Execution:** Follow `superpowers:executing-plans` task-by-task in an isolated worktree. Apply `superpowers:test-driven-development` to permission and missing-target corrections.

**Goal:** Secure and export the Study Plan Activities and Activity Catalog workflow so backend authorization, frontend permission gates, Rust DTOs, OpenAPI, and generated TypeScript describe the same behavior.

**Architecture:** Rust serde DTOs plus `utoipa` handler metadata remain the wire source of truth. Plan-activity and catalog handlers load one `actor_tenant_context` and use the generated curriculum permission family already consumed by the Subjects and Study Plans UI. Generating semester activity slots additionally requires `activity.manage.all`, because it mutates the school-wide activity workspace, while reading the plan template follows curriculum read policy. SQL remains in `study_plan_service`; missing parents and mutation targets return `AppError::NotFound`, and instructor-role transitions validate the target before changing the current primary assignment.

**Batch size:** 13 operations: ten mutations and three dependent reads. This moves the generated checkpoint from 124 to 137 unique operations.

## Operation inventory

| Method | Resolved path | Handler | Permission / policy | Success data |
|---|---|---|---|---|
| GET | `/api/academic/study-plan-versions/{id}/activities` | `list_plan_activities` | curriculum read policy | `Vec<StudyPlanVersionActivity>` |
| POST | `/api/academic/study-plan-versions/{id}/activities` | `add_plan_activity` | curriculum update policy | `StudyPlanVersionActivity` (201) |
| PUT | `/api/academic/study-plan-activities/{id}` | `update_plan_activity` | curriculum update policy | `StudyPlanVersionActivity` |
| DELETE | `/api/academic/study-plan-activities/{id}` | `delete_plan_activity` | curriculum delete policy | `EmptyData` |
| POST | `/api/academic/activities/generate-from-plan` | `generate_activities_from_plan` | curriculum read policy + `ACTIVITY_MANAGE_ALL` | `GenerateActivitiesFromPlanOutcome` |
| GET | `/api/academic/activity-catalog` | `list_activity_catalog` | curriculum read policy | `Vec<ActivityCatalog>` |
| POST | `/api/academic/activity-catalog` | `create_activity_catalog` | curriculum create policy | `ActivityCatalog` (201) |
| PUT | `/api/academic/activity-catalog/{id}` | `update_activity_catalog` | curriculum update policy | `ActivityCatalog` |
| DELETE | `/api/academic/activity-catalog/{id}` | `delete_activity_catalog` | curriculum delete policy | `EmptyData` |
| GET | `/api/academic/activity-catalog/{id}/default-instructors` | `list_catalog_default_instructors` | curriculum read policy | `Vec<CatalogDefaultInstructor>` |
| POST | `/api/academic/activity-catalog/{id}/default-instructors` | `add_catalog_default_instructor` | curriculum update policy | `EmptyData` |
| DELETE | `/api/academic/activity-catalog/{id}/default-instructors/{uid}` | `remove_catalog_default_instructor` | curriculum update policy | `EmptyData` |
| PUT | `/api/academic/activity-catalog/{id}/default-instructors/{uid}` | `update_catalog_default_instructor_role` | curriculum update policy | `EmptyData` |

All operations require authentication and tenant resolution. This batch has no migration, file, SSE, WebSocket, or PII-encryption change.

## Task 1: Lock down activity-template authorization

**Files:**

- Modify: `backend-school/src/modules/academic/handlers/study_plans.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `frontend-school/src/routes/(app)/staff/academic/activities/+page.svelte` only if its generate gate does not match the backend policy

1. Add a failing static regression proving all 13 handlers load `actor_tenant_context` and enforce the permission policy shown above.
2. Replace `tenant_pool` and optional header identity with one authenticated actor/tenant context; pass `actor.user_id` to activity generation.
3. Make the frontend Generate action require both activity management and curriculum read, matching the backend and the plan-version loader.
4. Run focused static/policy tests, the Svelte autofixer for edited components, format, Clippy, and `svelte-check`.
5. Commit as `fix(academic): enforce activity template permissions`.

## Task 2: Correct activity-template target behavior

**Files:**

- Modify: `backend-school/src/modules/academic/services/study_plan_service.rs`
- Modify: `backend-school/src/modules/academic/services/study_plan_service_tests.rs`

1. Add failing database-backed tests for missing plan versions, plan activities, catalogs, catalog instructor assignments, semester targets, and referenced catalog/grade/instructor rows.
2. Make list operations distinguish an existing empty parent from a missing parent.
3. Make update/delete operations return not-found when no row is affected and preserve internal database errors.
4. Validate the catalog instructor assignment before demoting an existing primary instructor.
5. Keep dependent cleanup transactional and preserve existing conflict behavior; do not edit migrations.
6. Run focused database-backed tests and commit as `fix(academic): reject missing activity template targets`.

## Task 3: Export all 13 backend operations

**Files:**

- Modify: `backend-school/src/modules/academic/handlers/study_plans.rs`
- Modify: `backend-school/src/modules/academic/models/study_plans.rs`
- Modify: `backend-school/src/modules/academic/models/activity.rs`
- Modify: `backend-school/src/modules/academic/services/activity_service.rs`
- Modify: `backend-school/src/modules/academic/services/study_plan_service.rs`
- Modify: `backend-school/src/api_contract.rs`

1. Add failing contract assertions for all method/path pairs, unique operation IDs, request schemas, typed success envelopes, and the 137-operation total.
2. Derive `ToSchema` for stable DTOs and define schema-only enums where wire values are bounded.
3. Add `utoipa::path` metadata with real status codes and standard error envelopes.
4. Register paths and schemas in `SchoolApiDoc` and regenerate the contract.
5. Run focused contract tests, format, and Clippy.
6. Commit as `feat(api): export activity template contracts`.

## Task 4: Generate frontend types and remove duplicate wire DTOs

**Files:**

- Regenerate: `contracts/openapi/school-api.json`
- Regenerate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/academic.ts`
- Create: `frontend-school/tests/static/academic-activity-template-contract.test.mjs`
- Modify: contract checkpoint tests and relevant Svelte consumers only when generated optionality reveals a real mismatch

1. Add failing static assertions for the 13 generated operations and exact wrapper paths/methods.
2. Replace handwritten activity-template/catalog wire DTOs and request shapes with generated schema aliases while preserving public API names.
3. Regenerate contracts and run focused Node tests, API staleness checks, TypeScript/Svelte checks, and Svelte autofixer for every edited `.svelte` file.
4. Commit as `feat(frontend): type activity template mutations`.

## Task 5: Verify, document, merge, and push

**Files:**

- Modify: `.rules`
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `docs/backend-school/API_DEVELOPMENT.md`
- Modify: `docs/TESTING.md`

1. Record the 137-operation checkpoint, permission correction, missing-target behavior, and the next Academic mutation batch.
2. Run backend format, Clippy, contract, static architecture, and full database-backed tests.
3. Run frontend generated-contract/permission checks, all static tests, and `svelte-check`.
4. Audit migrations, sensitive fields/secrets, generated staleness, operation-ID uniqueness, route mappings, and duplicate handwritten DTOs.
5. Review against this plan, rerun affected checks, fast-forward merge to `main`, push `origin/main`, verify `main...origin/main` is `0 0`, and remove the owned worktree/branch.
