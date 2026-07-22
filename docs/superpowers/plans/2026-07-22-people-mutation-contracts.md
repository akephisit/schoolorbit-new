# People Mutation Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add reviewed generated contracts for staff, student, parent-link, and achievement mutations while closing the inactive-account permission and student mutation correctness gaps discovered during inventory.

**Architecture:** Rust serde DTOs and `utoipa` handler metadata remain the wire source of truth. Behavior fixes live in permission/student services, handlers own cache/realtime effects, `SchoolApiDoc` owns the exported operation registry, and frontend wrappers consume generated schemas through stable aliases. This batch adds 13 operations (12 mutations plus the dependent achievement list read), moving the checkpoint from 68 to 81 operations.

**Tech Stack:** Rust, Axum, sqlx, PostgreSQL, utoipa, SvelteKit 5, TypeScript, Node static tests, generated OpenAPI contracts.

## Global Constraints

- Follow `.rules` and `docs/superpowers/specs/2026-07-22-backend-school-mutation-contract-rollout-design.md`.
- Use `TEST_DATABASE_URL` and isolated test schemas for database behavior tests.
- Write failing tests before production changes and observe the expected failure.
- Do not edit applied migrations; this batch requires no schema change.
- Keep handlers thin and database behavior in services.
- Use generated permission constants on both backend and frontend.
- Never log or commit plaintext national IDs, passwords, tokens, request bodies, or real credentials.
- `national_id` remains AES-256-GCM encrypted and HMAC blind-indexed through `field_encryption.rs`.
- Generate `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/school-api.ts`; never edit them manually.
- Run `svelte-autofixer` after touching a `.svelte` file.
- Merge and push only after all focused and global verification passes.

---

### Task 1: Make inactive accounts ineffective and student deletion observable

**Files:**
- Modify: `backend-school/src/modules/staff/services/status_tests.rs`
- Modify: `backend-school/src/middleware/permission.rs`
- Modify: `backend-school/src/modules/staff/services/user_role_service.rs`
- Modify: `backend-school/src/modules/students/handlers.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: `get_cached_user_permissions(tenant, user_id, pool, cache)` and `PermissionCache::invalidate_user`.
- Produces: effective permissions are empty when `users.status != 'active'`; `delete_student` invalidates the tenant/user cache and emits `permission_changed`.

- [ ] **Step 1: Write the inactive-account database regression test**

Append to `status_tests.rs` a test that creates a user and active role permission, proves the permission is present, sets the user status to `inactive`, invalidates the user cache, and proves the permission is absent:

```rust
#[tokio::test]
async fn inactive_user_accounts_have_no_effective_permissions() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let fixture = Uuid::new_v4().simple().to_string();
    let user_id = create_test_user(
        &pool,
        &format!("inactive-user-{}@example.test", &fixture[..8]),
        "Test1234!",
    )
    .await
    .expect("test user should be created");
    let role_id: Uuid = sqlx::query_scalar(
        "INSERT INTO roles (code, name, user_type, level)
         VALUES ($1, $2, 'staff', 1) RETURNING id",
    )
    .bind(format!("TACTIVEUSER{}", &fixture[..8]))
    .bind("inactive user test role")
    .fetch_one(&pool)
    .await
    .expect("test role should be created");
    let permission_id = permission_id(&pool, "roles.read.all").await;
    sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)")
        .bind(role_id)
        .bind(permission_id)
        .execute(&pool)
        .await
        .expect("role permission should be assigned");
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
        .bind(user_id)
        .bind(role_id)
        .execute(&pool)
        .await
        .expect("role should be assigned");

    let tenant = format!("inactive-user-{}", &fixture[..8]);
    let cache = PermissionCache::new();
    let active = get_cached_user_permissions(&tenant, user_id, &pool, &cache)
        .await
        .expect("active permissions should load");
    assert_has_permission(&active, "roles.read.all");

    sqlx::query("UPDATE users SET status = 'inactive' WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("test account should deactivate");
    cache.invalidate_user(&tenant, user_id);

    let inactive = get_cached_user_permissions(&tenant, user_id, &pool, &cache)
        .await
        .expect("inactive permissions should load");
    assert_lacks_permission(&inactive, "roles.read.all");
}
```

- [ ] **Step 2: Add a failing static side-effect guard for student deletion**

Add `student_deactivation_invalidates_effective_permissions` to `static_architecture.rs`. Read `src/modules/students/handlers.rs`, isolate `delete_student`, and require these exact calls:

```rust
assert!(delete_handler.contains("permission_cache.invalidate_user(&tenant, student_id)"));
assert!(delete_handler.contains("notify_permission_changed(&tenant, student_id)"));
```

- [ ] **Step 3: Run the tests and observe RED**

Run:

```bash
cd backend-school
test -n "$TEST_DATABASE_URL"
cargo test inactive_user_accounts_have_no_effective_permissions --bin backend-school -- --nocapture
cargo test student_deactivation_invalidates_effective_permissions --test static_architecture
```

Expected: the database test finds `roles.read.all` after status becomes inactive, and the static test reports missing cache/realtime calls.

- [ ] **Step 4: Filter effective permissions through the active user**

In `fetch_user_permissions`, keep the three permission branches unchanged and add this outer predicate before `ORDER BY code`:

```sql
WHERE EXISTS (
    SELECT 1
    FROM users active_user
    WHERE active_user.id = $1
      AND active_user.status = 'active'
)
```

Apply the same active-user condition to `user_role_service::get_user_permissions` so the administrative effective-permission read matches runtime authorization.

- [ ] **Step 5: Invalidate student permissions after deactivation**

In `delete_student`, retain the tenant before moving the pool, then add:

```rust
let tenant = context.tenant.subdomain.clone();
// ... service call ...
state.permission_cache.invalidate_user(&tenant, student_id);
state.notify_permission_changed(&tenant, student_id);
```

- [ ] **Step 6: Run focused and neighboring authorization tests**

Run:

```bash
cd backend-school
test -n "$TEST_DATABASE_URL"
cargo test inactive_user_accounts_have_no_effective_permissions --bin backend-school -- --nocapture
cargo test middleware::permission::tests --bin backend-school
cargo test student_deactivation_invalidates_effective_permissions --test static_architecture
```

Expected: PASS.

- [ ] **Step 7: Commit the security correction**

```bash
git add backend-school/src/middleware/permission.rs \
  backend-school/src/modules/staff/services/user_role_service.rs \
  backend-school/src/modules/staff/services/status_tests.rs \
  backend-school/src/modules/students/handlers.rs \
  backend-school/tests/static_architecture.rs
git commit -m "fix(authz): revoke inactive account permissions"
```

### Task 2: Return not-found for missing student mutation targets

**Files:**
- Modify: `backend-school/src/modules/students/services.rs`
- Modify: `backend-school/src/modules/students.rs`
- Create: `backend-school/src/modules/students/mutation_tests.rs`

**Interfaces:**
- Consumes: `update_student`, `delete_student`, and `add_parent_to_student`.
- Produces: missing student mutation targets return `AppError::NotFound`; no orphan parent is created for a missing student.

- [ ] **Step 1: Register a database test module**

Add to `backend-school/src/modules/students.rs`:

```rust
#[cfg(test)]
mod mutation_tests;
```

- [ ] **Step 2: Write missing-target database tests**

Create `mutation_tests.rs` using `create_test_pool()` and `run_test_migrations()`. Add three tests:

```rust
#[tokio::test]
async fn update_missing_student_returns_not_found() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    let result = services::update_student(
        &pool,
        Uuid::new_v4(),
        UpdateStudentRequest {
            email: None,
            first_name: Some("Missing".to_string()),
            last_name: None,
            phone: None,
            address: None,
            student_number: None,
        },
    )
    .await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn delete_missing_student_returns_not_found() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    let result = services::delete_student(&pool, Uuid::new_v4()).await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn add_parent_to_missing_student_returns_not_found_without_creating_parent() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    let phone = format!("09{}", &Uuid::new_v4().simple().to_string()[..8]);
    let result = services::add_parent_to_student(
        &pool,
        Uuid::new_v4(),
        CreateParentRequest {
            title: None,
            first_name: "Missing".to_string(),
            last_name: "Parent".to_string(),
            phone: phone.clone(),
            relationship: "parent".to_string(),
            national_id: None,
            email: None,
        },
    )
    .await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
    let created: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
        .bind(phone)
        .fetch_one(&pool)
        .await
        .expect("parent existence should load");
    assert!(!created);
}
```

- [ ] **Step 3: Run the tests and observe RED**

Run:

```bash
cd backend-school
test -n "$TEST_DATABASE_URL"
cargo test modules::students::mutation_tests --bin backend-school -- --nocapture
```

Expected: update/delete return `Ok(())`; add-parent returns an internal database error instead of `NotFound`.

- [ ] **Step 4: Check affected rows for update and delete**

Capture the `UPDATE users` results in both services. Before subsequent writes or commit, return:

```rust
if result.rows_affected() == 0 {
    return Err(AppError::NotFound("ไม่พบนักเรียน".to_string()));
}
```

The open transaction rolls back automatically on return.

- [ ] **Step 5: Validate the student before creating a parent**

At the start of the add-parent transaction, query:

```sql
SELECT EXISTS(
    SELECT 1 FROM users
    WHERE id = $1 AND user_type = 'student'
)
```

Return `AppError::NotFound("ไม่พบนักเรียน".to_string())` before `get_or_create_parent_user` when false.

- [ ] **Step 6: Run focused tests**

Run the command from Step 3 again. Expected: all three tests PASS.

- [ ] **Step 7: Commit the student mutation fix**

```bash
git add backend-school/src/modules/students.rs \
  backend-school/src/modules/students/services.rs \
  backend-school/src/modules/students/mutation_tests.rs
git commit -m "fix(student): reject missing mutation targets"
```

### Task 3: Export staff mutation contracts

**Files:**
- Modify: `backend-school/src/modules/staff/models.rs`
- Modify: `backend-school/src/modules/staff/handlers/staff.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Produces operations `createStaff`, `updateStaff`, `deleteStaff` and generated schemas `CreateStaffRequest`, `UpdateStaffRequest`, `CreateStaffInfoRequest`, `OrganizationAssignment`.

- [ ] **Step 1: Add failing contract assertions**

In `api_contract.rs`, add a test asserting:

```rust
assert_operations(
    &document,
    &[
        ("/api/staff", "post", "createStaff"),
        ("/api/staff/{id}", "put", "updateStaff"),
        ("/api/staff/{id}", "delete", "deleteStaff"),
    ],
);
```

Also assert create returns 201 with `ApiResponse_UuidIdData`, update/delete return 200 with `ApiResponse_EmptyData`, path IDs use UUID format, create documents 400/401/403, update documents 400/401/403/404, and delete documents 401/403/404 through `ApiErrorResponse`.

- [ ] **Step 2: Run the contract test and observe RED**

Run `cargo test people_staff_mutation_contracts --bin backend-school`. Expected: missing `/api/staff` POST metadata.

- [ ] **Step 3: Derive request schemas**

Add `ToSchema` to:

```rust
CreateStaffInfoRequest
CreateStaffRequest
OrganizationAssignment
UpdateStaffRequest
```

- [ ] **Step 4: Annotate staff handlers**

Add `#[utoipa::path]` with the approved operation IDs, exact path parameters, `ApiResponse<UuidIdData>` or `ApiResponse<EmptyData>`, and current 400/401/403/404 behavior. Do not change runtime envelopes.

- [ ] **Step 5: Register staff paths and schemas**

Add all three handlers and four request/nested schemas to `SchoolApiDoc`. Reuse the existing `UuidIdData`, `EmptyData`, and error schemas.

- [ ] **Step 6: Run focused contract and compile checks**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test people_staff_mutation_contracts --bin backend-school
cargo check --bin backend-school
```

Expected: PASS.

- [ ] **Step 7: Commit staff contracts**

```bash
git add backend-school/src/modules/staff/models.rs \
  backend-school/src/modules/staff/handlers/staff.rs \
  backend-school/src/api_contract.rs
git commit -m "feat(api): export staff mutation contracts"
```

### Task 4: Export student and parent-link mutation contracts

**Files:**
- Modify: `backend-school/src/modules/students/models.rs`
- Modify: `backend-school/src/modules/students/handlers.rs`
- Modify: `backend-school/src/modules/students/handlers_parents.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Produces operations `updateStudentProfile`, `createStudent`, `updateStudent`, `deleteStudent`, `addStudentParent`, and `removeStudentParent`.

- [ ] **Step 1: Add failing operation and schema assertions**

Add a contract test for:

```rust
[
    ("/api/student/profile", "put", "updateStudentProfile"),
    ("/api/students", "post", "createStudent"),
    ("/api/students/{id}", "put", "updateStudent"),
    ("/api/students/{id}", "delete", "deleteStudent"),
    ("/api/students/{id}/parents", "post", "addStudentParent"),
    ("/api/students/{id}/parents/{parent_id}", "delete", "removeStudentParent"),
]
```

Assert `CreateStudentRequest.parents` is an optional array of `CreateParentRequest`, `password` is required only on create, no national-ID example/default appears, create returns `ApiResponse_CreateStudentResponse`, and the other operations return `ApiResponse_EmptyData`.

- [ ] **Step 2: Run the test and observe RED**

Run `cargo test people_student_mutation_contracts --bin backend-school`. Expected: missing student PUT/POST paths.

- [ ] **Step 3: Derive request/response schemas**

Add `ToSchema` to:

```rust
UpdateOwnProfileRequest
CreateStudentRequest
CreateParentRequest
UpdateStudentRequest
CreateStudentResponse
```

- [ ] **Step 4: Annotate six student handlers**

Document exact UUID path parameters and current/corrected 400/401/403/404 responses. Use 201 for create and 200 for empty mutations.

- [ ] **Step 5: Register paths and schemas**

Add the six handlers and five schemas to `SchoolApiDoc`.

- [ ] **Step 6: Run focused checks**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test people_student_mutation_contracts --bin backend-school
test -n "$TEST_DATABASE_URL"
cargo test modules::students::mutation_tests --bin backend-school -- --nocapture
cargo check --bin backend-school
```

Expected: PASS.

- [ ] **Step 7: Commit student contracts**

```bash
git add backend-school/src/modules/students/models.rs \
  backend-school/src/modules/students/handlers.rs \
  backend-school/src/modules/students/handlers_parents.rs \
  backend-school/src/api_contract.rs
git commit -m "feat(api): export student mutation contracts"
```

### Task 5: Export achievement contracts

**Files:**
- Modify: `backend-school/src/modules/achievement/models.rs`
- Modify: `backend-school/src/modules/achievement/handlers.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Produces `listAchievements`, `createAchievement`, `updateAchievement`, and `deleteAchievement`; schemas `Achievement`, `AchievementListFilter`, `CreateAchievementRequest`, and `UpdateAchievementRequest`.

- [ ] **Step 1: Write the failing contract test**

Assert the four path/method/operation-ID tuples, query date formats, UUID IDs, achievement response envelope, 201 create, empty delete, and 401/403/404 errors.

- [ ] **Step 2: Observe RED**

Run `cargo test people_achievement_contracts --bin backend-school`. Expected: `/api/achievements` is absent.

- [ ] **Step 3: Derive achievement schemas and query parameters**

Add `ToSchema` to response/request structs and `IntoParams` to `AchievementListFilter`. Mark response `Option` fields as required nullable because serde emits them as `null`; leave request `Option` fields optional/nullable.

- [ ] **Step 4: Annotate and register achievement handlers**

Add `utoipa::path` for list/create/update/delete, register paths/schemas, and add the `achievement` tag.

- [ ] **Step 5: Run focused checks**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test people_achievement_contracts --bin backend-school
cargo test modules::achievement --bin backend-school
cargo check --bin backend-school
```

Expected: PASS.

- [ ] **Step 6: Commit achievement contracts**

```bash
git add backend-school/src/modules/achievement/models.rs \
  backend-school/src/modules/achievement/handlers.rs \
  backend-school/src/api_contract.rs
git commit -m "feat(api): export achievement contracts"
```

### Task 6: Generate frontend contracts and remove duplicate wire DTOs

**Files:**
- Modify: `contracts/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/src/lib/api/staff.ts`
- Modify: `frontend-school/src/lib/api/students.ts`
- Modify: `frontend-school/src/lib/api/achievement.ts`
- Modify: `frontend-school/src/lib/types/achievement.ts`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
- Create: `frontend-school/tests/static/people-mutation-contract.test.mjs`

**Interfaces:**
- Consumes: generated schemas from Tasks 3–5.
- Produces: stable generated aliases for every migrated request/result and static drift coverage.

- [ ] **Step 1: Write failing frontend static tests**

Create a test that requires:

- all 13 operation IDs in generated TypeScript;
- staff/student/achievement wrappers to use `Schemas[...]` aliases;
- no handwritten `interface CreateStaffRequest`, `interface UpdateStaffRequest`, `interface CreateStudentRequest`, `interface UpdateStudentRequest`, `interface UpdateOwnProfileRequest`, `interface CreateParentRequest`, `interface Achievement`, `interface CreateAchievementRequest`, or `interface UpdateAchievementRequest`;
- mutation wrapper methods and paths remain exact;
- generated contract operation count is 81.

Add assertions to `api-response-contract.test.mjs` that empty mutations use `EmptyData` instead of `Record<string, never>`.

- [ ] **Step 2: Run the tests and observe RED**

Run:

```bash
cd frontend-school
node --test tests/static/people-mutation-contract.test.mjs \
  tests/static/api-response-contract.test.mjs
```

Expected: generated operation IDs and aliases are absent.

- [ ] **Step 3: Regenerate tracked artifacts**

Run `npm run generate:api-contracts`. Do not hand-edit either generated file.

- [ ] **Step 4: Replace handwritten staff/student request types**

Use aliases such as:

```ts
export type CreateStaffRequest = Schemas['CreateStaffRequest'];
export type UpdateStaffRequest = Schemas['UpdateStaffRequest'];
export type CreateStudentRequest = Schemas['CreateStudentRequest'];
export type UpdateStudentRequest = Schemas['UpdateStudentRequest'];
export type UpdateOwnProfileRequest = Schemas['UpdateOwnProfileRequest'];
export type CreateParentRequest = Schemas['CreateParentRequest'];
type EmptyData = Schemas['EmptyData'];
type CreateStudentResponse = Schemas['CreateStudentResponse'];
```

Keep existing view return shapes where pages depend on them, but type the transport payload through the generated aliases.

- [ ] **Step 5: Replace achievement wire interfaces**

Make `types/achievement.ts` re-export generated aliases and keep only UI-only types if one exists. Update the API wrapper to use `EmptyData` for delete.

- [ ] **Step 6: Run generated and focused frontend checks**

Run:

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/people-mutation-contract.test.mjs \
  tests/static/api-response-contract.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: PASS with 81 operations and no handwritten duplicate DTOs.

- [ ] **Step 7: Commit generated/frontend contracts**

```bash
git add contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/staff.ts \
  frontend-school/src/lib/api/students.ts \
  frontend-school/src/lib/api/achievement.ts \
  frontend-school/src/lib/types/achievement.ts \
  frontend-school/tests/static/people-mutation-contract.test.mjs \
  frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "feat(frontend): type people mutation APIs"
```

### Task 7: Patch achievement UI state from typed mutation results

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/manage/[id]/+page.svelte`
- Modify: `frontend-school/tests/static/people-mutation-contract.test.mjs`

**Interfaces:**
- Consumes: `ApiResponse<Achievement>` from create/update and `ApiResponse<EmptyData>` from delete.
- Produces: create prepends the returned achievement, update replaces the matching row, and delete removes the row without calling `loadAchievements()`.

- [ ] **Step 1: Add failing local-state assertions**

Require the page to contain:

```ts
achievements = [res.data, ...achievements];
achievements = achievements.map((item) => (item.id === res.data.id ? res.data : item));
achievements = achievements.filter((item) => item.id !== deleteId);
```

and reject `loadAchievements()` inside the successful create/update/delete branches.

- [ ] **Step 2: Run the focused test and observe RED**

Run `node --test tests/static/people-mutation-contract.test.mjs`. Expected: the page still reloads achievements.

- [ ] **Step 3: Patch local state**

Use the typed returned achievement to insert/replace. Capture the deleted ID before clearing it, then filter local state after success. Keep initial/manual loading unchanged.

- [ ] **Step 4: Run Svelte verification**

Run:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/manage/[id]/+page.svelte' --svelte-version 5
node --test tests/static/people-mutation-contract.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: no autofixer issues and all tests PASS. Do not mechanically rewrite unrelated existing `$effect` suggestions.

- [ ] **Step 5: Commit the UI update**

```bash
git add 'frontend-school/src/routes/(app)/staff/manage/[id]/+page.svelte' \
  frontend-school/tests/static/people-mutation-contract.test.mjs
git commit -m "perf(frontend): patch achievement mutation state"
```

### Task 8: Document, verify, review, merge, and push Phase 1

**Files:**
- Modify: `.rules`
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/backend-school/API_DEVELOPMENT.md`

**Interfaces:**
- Produces: the 81-operation checkpoint and Phase 1 completion record.

- [ ] **Step 1: Update documentation**

Record 81 operations, list the 13 new operations by domain, document inactive-account permission suppression and student cache invalidation, and state that Phase 2 academic mutations remain next. Keep protocol/binary exclusions unchanged.

- [ ] **Step 2: Run backend verification**

```bash
cd backend-school
cargo fmt --all -- --check
cargo clippy --bin backend-school -- -D warnings
cargo test api_contract::tests --bin backend-school
cargo test --test static_architecture
test -n "$TEST_DATABASE_URL"
cargo test --bin backend-school
```

- [ ] **Step 3: Run frontend verification**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run check:permissions
npm run test:permissions
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Run Prettier only across changed frontend files and report unrelated repository-wide lint failures without bulk formatting them.

- [ ] **Step 4: Inspect sensitive and generated diffs**

```bash
git diff --check
git diff main...HEAD -- backend-school/migrations
git diff main...HEAD -- contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts
git diff main...HEAD | rg -ni 'national_id|password|authorization|bearer|secret' || true
git status --short
```

Confirm no migration changed and all credential-looking values are synthetic test fixtures only.

- [ ] **Step 5: Commit documentation**

```bash
git add .rules IMPROVEMENT_PLAN.md docs/TESTING.md docs/backend-school/API_DEVELOPMENT.md
git commit -m "docs(api): record people mutation contracts"
```

- [ ] **Step 6: Review and integrate**

Use `superpowers:requesting-code-review`, address verified findings, rerun affected checks, then use `superpowers:finishing-a-development-branch`. With the already-approved integration policy, fast-forward merge the feature branch to `main`, push `origin/main`, and verify `main...origin/main` is `0 0` before starting the Phase 2 worktree.
