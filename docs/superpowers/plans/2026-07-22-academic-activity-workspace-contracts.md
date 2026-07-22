# Academic Activity Workspace Contract Implementation Plan

**Goal:** Secure and export the complete Activity Workspace so slot, group, instructor, classroom-assignment, member, and self-enrollment behavior uses one backend permission/status contract and generated frontend wire types.

**Architecture:** Rust serde DTOs plus `utoipa` handler metadata remain the wire source of truth. School-wide slot mutations use `activity.manage.all`; timetable cleanup additionally uses `academic.course_plan.manage.all`; group ownership uses `activity_access_policy`; member administration uses `activity.manage_members.all`; self-enrollment remains active-user scoped. SQL stays in `activity_service`, with missing parents or mutation targets returning `AppError::NotFound`, conflict/closed-capacity outcomes returning non-200 errors, and bulk responses reporting actual affected rows.

**Batch size:** 28 operations: 21 mutations and seven dependent reads. This moves the generated checkpoint from 137 to 165 unique operations.

## Operation inventory

| Method | Resolved path | Operation ID | Access |
|---|---|---|---|
| GET | `/api/academic/activity-slots` | `listActivitySlots` | activity list policy |
| PUT | `/api/academic/activity-slots/{id}` | `updateActivitySlot` | `activity.manage.all` |
| DELETE | `/api/academic/activity-slots/{id}` | `deleteActivitySlot` | `activity.manage.all` |
| GET | `/api/academic/activity-slots/{id}/instructors` | `listActivitySlotInstructors` | slot read policy |
| POST | `/api/academic/activity-slots/{id}/instructors` | `addActivitySlotInstructor` | `activity.manage.all` |
| POST | `/api/academic/activity-slots/{id}/instructors/batch` | `addActivitySlotInstructorsBatch` | `activity.manage.all` |
| DELETE | `/api/academic/activity-slots/{id}/instructors/{user_id}` | `removeActivitySlotInstructor` | `activity.manage.all` |
| DELETE | `/api/academic/activity-slots/{id}/instructors/all` | `removeAllActivitySlotInstructors` | `activity.manage.all` |
| DELETE | `/api/academic/activity-slots/{id}/groups` | `deleteAllActivitySlotGroups` | `activity.manage.all` |
| DELETE | `/api/academic/activity-slots/{id}/timetable-entries` | `deleteActivitySlotTimetableEntries` | `academic.course_plan.manage.all` |
| GET | `/api/academic/activity-slots/{id}/classroom-assignments` | `listActivitySlotClassroomAssignments` | slot read policy |
| POST | `/api/academic/activity-slots/{id}/classroom-assignments` | `upsertActivitySlotClassroomAssignments` | `activity.manage.all` |
| DELETE | `/api/academic/activity-slots/{id}/classroom-assignments/all` | `deleteAllActivitySlotClassroomAssignments` | `activity.manage.all` |
| DELETE | `/api/academic/activity-slots/{id}/classroom-assignments/{assignment_id}` | `deleteActivitySlotClassroomAssignment` | `activity.manage.all` |
| GET | `/api/academic/activities` | `listActivityGroups` | activity list policy |
| POST | `/api/academic/activities` | `createActivityGroup` | resource-aware activity manage policy |
| PUT | `/api/academic/activities/{id}` | `updateActivityGroup` | resource-aware activity manage policy |
| DELETE | `/api/academic/activities/{id}` | `deleteActivityGroup` | resource-aware activity manage policy |
| GET | `/api/academic/activities/{id}/members` | `listActivityGroupMembers` | resource-aware activity read policy |
| POST | `/api/academic/activities/{id}/members` | `addActivityGroupMembers` | `activity.manage_members.all` |
| DELETE | `/api/academic/activities/{id}/members/{student_id}` | `removeActivityGroupMember` | `activity.manage_members.all` |
| PUT | `/api/academic/activities/members/{member_id}/result` | `updateActivityGroupMemberResult` | `activity.manage_members.all` |
| GET | `/api/academic/activities/{id}/instructors` | `listActivityGroupInstructors` | resource-aware activity read policy |
| POST | `/api/academic/activities/{id}/instructors` | `addActivityGroupInstructor` | resource-aware activity manage policy |
| DELETE | `/api/academic/activities/{id}/instructors/{instructor_id}` | `removeActivityGroupInstructor` | resource-aware activity manage policy |
| GET | `/api/academic/activities/my-enrollments` | `listMyActivityEnrollments` | authenticated active user |
| POST | `/api/academic/activities/{id}/enroll` | `selfEnrollActivityGroup` | authenticated active student |
| DELETE | `/api/academic/activities/{id}/enroll` | `selfUnenrollActivityGroup` | authenticated active user, own membership |

## Task 1: Lock authorization and HTTP behavior

**Files:**

- Modify: `backend-school/src/policies/activity_access_policy.rs`
- Modify: `backend-school/src/modules/academic/handlers/activity.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `frontend-school/tests/static/permission-actions.test.mjs`

1. Add failing static tests for all 28 handler permission/context decisions.
2. Add resource-aware slot read access so `activity.manage.own` users can consume nested data for slots visible to them.
3. Make group deletion use the same owner/all resource policy as group editing and the frontend UI.
4. Convert closed slot, invalid instructor, duplicate enrollment, capacity, and classroom-scope outcomes from HTTP 200 error envelopes to typed 4xx errors.
5. Keep self views permission-free but require an authenticated active user; self-enrollment additionally requires an active student enrollment.

## Task 2: Reject missing targets and make bulk behavior truthful

**Files:**

- Modify: `backend-school/src/modules/academic/services/activity_service.rs`
- Modify: `backend-school/src/modules/academic/services/activity_service_tests.rs` or add focused tests under the existing module

1. Add failing database-backed tests for missing slots, groups, members, instructors, classroom assignments, users, and classrooms.
2. Add explicit existence checks/row-count checks so missing parents and mutation targets return not-found instead of empty success or internal errors.
3. Validate bounded role/result values before writes.
4. Deduplicate bulk inputs and report actual inserts/processed assignments.
5. Lock group enrollment capacity checks and perform capacity-sensitive writes transactionally.

## Task 3: Export the 165-operation OpenAPI contract

**Files:**

- Modify: `backend-school/src/modules/academic/handlers/activity.rs`
- Modify: `backend-school/src/modules/academic/models/activity.rs`
- Modify: `backend-school/src/modules/academic/services/activity_service.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/tests/static_architecture.rs`

1. Add failing assertions for all method/path/operation-ID pairs and the 165-operation total.
2. Derive/register exact request, query, response, enum, and count DTO schemas.
3. Document only real success/error statuses and standard envelopes.
4. Regenerate the tracked OpenAPI and TypeScript artifacts and verify unique operation IDs.

## Task 4: Consume generated frontend wire types

**Files:**

- Modify: `frontend-school/src/lib/api/academic.ts`
- Modify as required: Activity staff/student Svelte pages
- Add: `frontend-school/tests/static/academic-activity-workspace-contract.test.mjs`

1. Add failing assertions that the frontend owns no duplicate handwritten Activity Workspace DTOs.
2. Alias generated request/response/filter schemas in `academic.ts` and type each wrapper body.
3. Keep existing UI/domain normalization explicit where nullable wire fields require fallbacks.
4. Run the Svelte autofixer for every changed `.svelte` file and require `svelte-check` to remain clean.

## Task 5: Record, verify, merge, and push

**Files:**

- Modify: `.rules`
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/backend-school/API_DEVELOPMENT.md`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`

1. Record the 165-operation checkpoint, permission/status corrections, missing-target rules, and the next Academic mutation batch.
2. Run Rust format, Clippy, API contract tests, static architecture tests, and all database-backed backend tests.
3. Run frontend generated-contract/permission checks, all static tests, Svelte autofixer, and `svelte-check`.
4. Audit migration immutability, secrets/PII logging, operation uniqueness, route mappings, and duplicate handwritten DTOs.
5. Review the complete diff, fast-forward merge to `main`, push `origin/main`, verify `main...origin/main` is `0 0`, and remove only this batch's worktree/branch.
