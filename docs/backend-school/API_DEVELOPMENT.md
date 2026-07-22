# Backend School - API Development Guide

## Workflow: Adding a New Endpoint

To add a new API endpoint, follow these steps to ensure consistency:

### 1. Define the Protocol
Decide on the HTTP Method (GET, POST, PUT, DELETE) and the URL path.
*   *Example:* `GET /api/staff/:id`

### 2. Create the Request/Response Models
If the endpoint expects a JSON body or returns a JSON response, define the structs.
*   **Location:** `src/models/` or inside the handler file if specific to that handler.
*   **Derive Macros:** `#[derive(Serialize, Deserialize, Validate)]`

### 3. Implement the Repository Method
If you need new data, write the SQL query in the repository layer.
*   **File:** e.g., `src/repositories/staff_repo.rs`

### 4. Implement the Service Logic
Call the repository and apply any business rules.
*   **File:** e.g., `src/services/staff_service.rs`

### 5. Create the Handler
Extract data from the Request, call the Service, and handle errors.
*   **File:** e.g., `src/handlers/staff.rs`
*   **Return Type:** Use `AxumResult<Json<YourResponse>>` (or standard `Result`).

### 6. Register the Route
Add the new handler to the router configuration.
*   **File:** `src/routes.rs` (or `main.rs` depending on setup).
*   **Permission:** Load the actor context in the handler and call `actor.require_*` if the endpoint is protected.

## Generated API contracts

Rust request/response DTOs and OpenAPI handler metadata are the source of truth.
`contracts/openapi/school-api.json` and files under
`frontend-school/src/lib/api/generated/` are generated files; do not edit them
directly.

After changing a documented DTO or endpoint:

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Commit Rust source, OpenAPI, generated TypeScript, and focused tests together.
Frontend API modules import generated wire DTOs and may map them to separate
domain/view models. Generation must not require database credentials or start
the backend server.

The current checkpoint contains 165 unique operations: 32 auth/authorization
operations, 36 read-oriented JSON operations from the prior checkpoint, the
completed Phase 1 people batch (12 mutations plus the dependent achievement
list read), and the first Phase 2 academic structure batch (15 mutations plus
four dependent reads), the Phase 2 curriculum core batch (15 mutations plus
nine dependent reads), the Phase 2 activity-template batch (10 mutations plus
three dependent reads), and the Phase 2 activity workspace batch (21 mutations
plus seven dependent reads). The people operations are:

- staff: `createStaff`, `updateStaff`, `deleteStaff`
- student/parent-link: `updateStudentProfile`, `createStudent`, `updateStudent`,
  `deleteStudent`, `addStudentParent`, `removeStudentParent`
- achievement: `listAchievements`, `createAchievement`, `updateAchievement`,
  `deleteAchievement`

The academic structure operations are `getAcademicStructure`,
`createGradeLevel`, `deleteGradeLevel`, `createAcademicYear`,
`updateAcademicYear`, `setActiveAcademicYear`, `getAcademicYearLevels`,
`updateAcademicYearLevels`, `createSemester`, `updateSemester`,
`deleteSemester`, `listClassrooms`, `createClassroom`, `updateClassroom`,
`enrollStudents`, `listClassEnrollments`, `removeEnrollment`,
`updateEnrollmentNumber`, and `autoAssignClassNumbers`.

The curriculum core operations are `listSubjectGroups`,
`batchListSubjectDefaultInstructors`, `listSubjects`, `createSubject`,
`updateSubject`, `deleteSubject`, `listSubjectDefaultInstructors`,
`addSubjectDefaultInstructor`, `removeSubjectDefaultInstructor`,
`updateSubjectDefaultInstructorRole`, `listStudyPlans`, `createStudyPlan`,
`getStudyPlan`, `updateStudyPlan`, `deleteStudyPlan`, `listStudyPlanVersions`,
`createStudyPlanVersion`, `getStudyPlanVersion`, `updateStudyPlanVersion`,
`deleteStudyPlanVersion`, `listStudyPlanSubjects`,
`addSubjectsToStudyPlanVersion`, `deleteStudyPlanSubject`, and
`generateCoursesFromStudyPlan`.

The activity-template operations are `listStudyPlanActivities`,
`addStudyPlanActivity`, `updateStudyPlanActivity`, `deleteStudyPlanActivity`,
`generateActivitiesFromStudyPlan`, `listActivityCatalog`,
`createActivityCatalog`, `updateActivityCatalog`, `deleteActivityCatalog`,
`listActivityCatalogDefaultInstructors`,
`addActivityCatalogDefaultInstructor`,
`removeActivityCatalogDefaultInstructor`, and
`updateActivityCatalogDefaultInstructorRole`.

The activity workspace operations are `listActivitySlots`, `updateActivitySlot`,
`deleteActivitySlot`, `listActivitySlotInstructors`, `addActivitySlotInstructor`,
`addActivitySlotInstructorsBatch`, `removeActivitySlotInstructor`,
`removeAllActivitySlotInstructors`, `deleteAllActivitySlotGroups`,
`deleteActivitySlotTimetableEntries`, `listActivitySlotClassroomAssignments`,
`upsertActivitySlotClassroomAssignments`,
`deleteAllActivitySlotClassroomAssignments`,
`deleteActivitySlotClassroomAssignment`, `listActivityGroups`,
`createActivityGroup`, `updateActivityGroup`, `deleteActivityGroup`,
`listActivityGroupMembers`, `addActivityGroupMembers`,
`removeActivityGroupMember`, `updateActivityGroupMemberResult`,
`listActivityGroupInstructors`, `addActivityGroupInstructor`,
`removeActivityGroupInstructor`, `listMyActivityEnrollments`,
`selfEnrollActivityGroup`, and `selfUnenrollActivityGroup`.

Activity workspace handlers use school-wide permissions for slot administration,
resource-aware policies for group ownership/read access, member-management
permission for roster changes, and authenticated self-service for enrollment.
Missing parents and mutation targets return not-found, bounded role/result values
are validated, bulk counts report actual writes, and capacity-sensitive writes
are transactional.

This is the current Phase 4 mutation-contract rollout checkpoint. The next
Phase 2 batch is course planning, teaching assignments, and scheduling
configuration. The OpenAPI document describes implemented backend
routes only; a frontend helper or UI call is not evidence that a backend route
exists. SSE, WebSocket, health/readiness, and file/binary endpoints remain
explicitly outside this OpenAPI contract.

Effective authorization is conditional on the account itself: when
`users.status != 'active'`, permission resolution returns no effective
permissions even if active assignments remain. A user-status mutation must
invalidate the per-user permission cache and emit `permission_changed`; student
soft deletion follows this rule. Student update/delete/add-parent services return
not-found for a missing student before any dependent mutation or parent creation.

Academic structure, classroom, and enrollment handlers load one
`actor_tenant_context` and enforce the matching generated permission before the
service call. Missing academic years, semesters, grade levels, classrooms, and
enrollments return not-found. Setting a missing academic year active leaves the
current active year unchanged.

Curriculum subject and study-plan handlers enforce the same generated permission
policy consumed by the frontend. Study-plan CRUD permits the matching all-scope
or organization-scope curriculum permission, while course generation requires
`ACADEMIC_COURSE_PLAN_MANAGE_ALL`. Missing subjects, instructor assignments,
study plans, versions, and plan-subject rows return not-found. A path
`version_id` is authoritative for plan-subject listing, the add-subject response
counts rows actually inserted, and an update to a missing instructor assignment
cannot demote the existing primary instructor.

Study-plan activity, activity-catalog, and default-instructor handlers load one
`actor_tenant_context` and enforce the matching curriculum read/create/update/
delete policy. Activity generation additionally requires `ACTIVITY_MANAGE_ALL`
plus curriculum read access. Missing study-plan versions, semesters, catalogs,
grade levels, instructors, plan-activity rows, and default-instructor
assignments return not-found. Updating a missing default-instructor assignment
cannot demote the existing primary instructor.

For a new documented endpoint:

1. Derive/register the exact Rust serde/`ToSchema` request and response DTOs.
2. Add `utoipa::path` to the implemented handler with only statuses it can emit.
3. Register the handler and schemas in `backend-school/src/api_contract.rs`.
4. Run the generator and commit both tracked generated files.
5. Import the generated wire DTO in the frontend API module; add an explicit
   mapper only when the UI needs a different view/domain shape.

### Reversible access-record deactivation

- `DELETE /api/roles/{id}` and `DELETE /api/organization/units/{id}` are
  implemented backend routes that set `is_active = false`; neither route performs
  a physical delete.
- Both DELETE routes require `roles.delete.all`. A PUT with
  `is_active: false` requires `roles.update.all` plus `roles.delete.all`;
  reactivation with `is_active: true` requires `roles.update.all`.
- Existing role assignments, organization memberships, grants, delegations, and
  history remain stored. Inactive sources do not contribute effective permissions;
  valid relationships take effect again after reactivation.
- Every real status transition writes an audit row in the same transaction.
  Idempotent requests do not add audit rows or trigger permission-cache/realtime
  invalidation.
- `is_system` is read-only and migration/provisioning-owned. It protects the
  `ADMIN` role and `SCHOOL` organization unit and must not be added to create or
  update request DTOs.
- Organization units enforce hierarchy safety: active children block parent
  deactivation, and an inactive parent blocks active create, move, or reactivation.
- `GET /api/roles` and `GET /api/organization/units` accept the optional
  `include_inactive=true` query for management screens. The default stays
  active-only for assignment consumers.

## Error Handling
*   Use the custom `AppError` type (if available) to map errors to HTTP status codes.
*   Internal DB errors should generally result in `500 Internal Server Error`, while validation failures result in `400 Bad Request`.

## Authentication & Permissions

### System Overview
The system uses a **Permission-Based Access Control (PBAC)** model. Users are assigned Roles, and Roles have Permissions.
*   **Authentication:** Handled by `auth_middleware` (validates JWT/Cookie).
*   **Authorization:** Handled explicitly within each handler using `utils::request_context` helpers and `ActorContext` methods.

### Implementing Permission Checks
To enforce that a user must have a specific permission (e.g., `staff.create.all`) to use an endpoint, follow this pattern inside your handler function:

```rust
use axum::{extract::State, http::HeaderMap, Json};
use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

pub async fn my_protected_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<MyResponse>>, AppError> {
    // 1. Resolve tenant + actor through the central request context helper.
    let context = actor_tenant_context(&state, &headers).await?;

    // 2. Enforce permission through ActorContext.
    context.actor.require_permission(codes::MY_FEATURE_READ_ALL)?;

    // 3. Proceed with business logic
    // Use context.tenant.pool for service calls and context.actor.user_id when needed.
    let result = my_service::load(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(result)))
}
```

Use `tenant_context(&state, &headers).await?` or `tenant_pool(&state, &headers).await?` for public tenant routes that do not need an actor. Do not call `resolve_tenant_pool`, `load_actor_context`, `pool_manager.get_pool`, or local `get_pool` helpers from feature handlers; those lower-level APIs are wrapped by `utils::request_context`.

### Defining New Permissions
1.  **Source Registry:** Add the permission to `contracts/permissions.json` using the canonical `module.action.scope` shape.
2.  **Generate Registries:** From `frontend-school`, run `npm run generate:permissions`, `npm run check:permissions`, and `npm run test:permissions`.
3.  **Commit Generated Artifacts:** Commit the permission contract, lock file, backend registry, and frontend registry together. Never edit `backend-school/src/permissions/registry_generated.rs` or `frontend-school/src/lib/permissions/registry.generated.ts` directly.
4.  **Database Migration:** Add new database permission rows through a new sequential migration after `001_baseline.sql`; do not edit an already-applied migration.
5.  **Usage:** load `ActorContext` once through `actor_tenant_context(...)`, then call `actor.require_permission(codes::MY_FEATURE_READ_ALL)` or `actor.require_any_permission(&[...])`.

## Logging

Runtime code should use `tracing::debug!`, `tracing::info!`, `tracing::warn!`, or `tracing::error!`. Avoid `println!` and `eprintln!` outside intentional CLI/bin output. Do not log plaintext PII, national IDs, credentials, tokens, database URLs, or raw request bodies.
