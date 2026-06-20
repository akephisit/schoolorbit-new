# Supervision Observation Detail Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a dedicated teaching supervision observation detail page with safe edit, evaluator management, and cancellation actions.

**Architecture:** Backend adds manager-only observation endpoints that reuse supervision policy helpers and service-layer validation. Frontend adds typed API functions, a guard-only SvelteKit detail route, and links from the parent workspace while keeping mutation state local to the affected observation.

**Tech Stack:** Rust + Axum + sqlx, SvelteKit 5 + TypeScript, local shadcn-svelte primitives, Node static tests, Rust unit/static tests.

---

### Task 1: Backend Contract And Service Rules

**Files:**
- Modify: `backend-school/src/modules/supervision/models.rs`
- Modify: `backend-school/src/modules/supervision/services.rs`
- Modify: `backend-school/src/modules/supervision/handlers.rs`
- Test: `backend-school/src/modules/supervision/services.rs`
- Test: `backend-school/tests/static_architecture.rs`

- [ ] **Step 1: Write failing backend static and unit tests**

Add tests that require these symbols and route strings:

```rust
assert!(handler.contains("patch(update_observation)"));
assert!(handler.contains("put(replace_observation_evaluators)"));
assert!(handler.contains("post(cancel_observation)"));
assert!(models.contains("UpdateSupervisionObservationRequest"));
assert!(models.contains("ReplaceObservationEvaluatorsRequest"));
assert!(models.contains("CancelObservationRequest"));
assert!(service.contains("manager_can_edit_observation"));
assert!(service.contains("replace_observation_evaluators"));
assert!(service.contains("cancel_observation"));
```

Add focused service tests:

```rust
#[test]
fn manager_can_edit_only_manageable_observation_statuses() {
    assert!(manager_can_edit_observation(SupervisionObservationStatus::Requested));
    assert!(manager_can_edit_observation(SupervisionObservationStatus::Planned));
    assert!(manager_can_edit_observation(SupervisionObservationStatus::Returned));
    assert!(!manager_can_edit_observation(SupervisionObservationStatus::UnderReview));
    assert!(!manager_can_edit_observation(SupervisionObservationStatus::Approved));
    assert!(!manager_can_edit_observation(SupervisionObservationStatus::Published));
    assert!(!manager_can_edit_observation(SupervisionObservationStatus::Completed));
    assert!(!manager_can_edit_observation(SupervisionObservationStatus::Cancelled));
}

#[test]
fn evaluator_replacement_keeps_submitted_evaluators() {
    let submitted = Uuid::new_v4();
    let retained = normalize_evaluator_replacement(
        &[EvaluatorReplacementState {
            evaluator_user_id: submitted,
            submitted: true,
        }],
        vec![EvaluatorAssignmentInput {
            evaluator_user_id: Uuid::new_v4(),
            role_label: None,
            is_required: Some(true),
        }],
    )
    .expect("replacement");

    assert!(retained.iter().any(|item| item.evaluator_user_id == submitted));
}
```

- [ ] **Step 2: Verify RED**

Run:

```bash
cd backend-school
cargo test modules::supervision::services::tests::manager_can_edit_only_manageable_observation_statuses --bin backend-school
cargo test --test static_architecture teaching_supervision_observation_detail_actions_are_registered
```

Expected: fail because helpers/routes/DTOs do not exist.

- [ ] **Step 3: Implement backend DTOs and services**

Add typed request DTOs:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSupervisionObservationRequest {
    pub template_id: Option<Uuid>,
    pub timetable_entry_id: Option<Uuid>,
    pub observed_at: Option<DateTime<Utc>>,
    pub manual_lesson: Option<ManualLessonInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceObservationEvaluatorsRequest {
    #[serde(default)]
    pub evaluators: Vec<EvaluatorAssignmentInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelObservationRequest {
    pub reason: Option<String>,
}
```

Add service functions:

```rust
pub fn manager_can_edit_observation(status: SupervisionObservationStatus) -> bool {
    matches!(
        status,
        SupervisionObservationStatus::Requested
            | SupervisionObservationStatus::Planned
            | SupervisionObservationStatus::Returned
    )
}
```

`update_observation` validates manageable status, resolves lesson input with the observation cycle, updates lesson/template fields, inserts an `updated` action, and returns `get_observation`.

`replace_observation_evaluators` validates manageable status, rejects assigning the observed teacher, preserves submitted evaluator rows, deletes only non-submitted evaluator rows, bulk inserts missing requested evaluators, inserts `evaluators_updated`, and returns `get_observation`.

`cancel_observation` validates manager access and status transition to `cancelled`, inserts a `cancelled` action with reason, and returns the updated observation.

- [ ] **Step 4: Wire handlers and routes**

Add handler functions using `actor_tenant_context`, `get_observation`, and `require_observation_management_access`, then route:

```rust
.route(
    "/observations/{id}",
    get(get_observation).patch(update_observation),
)
.route(
    "/observations/{id}/evaluators",
    put(replace_observation_evaluators),
)
.route("/observations/{id}/cancel", post(cancel_observation))
```

- [ ] **Step 5: Verify backend GREEN**

Run:

```bash
cd backend-school
cargo test modules::supervision::services::tests --bin backend-school
cargo test --test static_architecture teaching_supervision_observation_detail_actions_are_registered
cargo check
```

Expected: pass.

### Task 2: Frontend API And Detail Route

**Files:**
- Modify: `frontend-school/src/lib/api/supervision.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte`
- Test: `frontend-school/tests/static/supervision-booking.test.mjs`

- [ ] **Step 1: Write failing frontend static test**

Add assertions that require:

```js
assert.match(supervisionApi, /updateSupervisionObservation/);
assert.match(supervisionApi, /replaceSupervisionObservationEvaluators/);
assert.match(supervisionApi, /cancelSupervisionObservation/);
assert.match(detailRoute, /_meta\s*=\s*\{\s*access:/);
assert.doesNotMatch(detailRoute, /menu:/);
assert.match(detailPage, /getSupervisionObservation/);
assert.match(detailPage, /updateSupervisionObservation/);
assert.match(detailPage, /replaceSupervisionObservationEvaluators/);
assert.match(detailPage, /cancelSupervisionObservation/);
assert.match(parentPage, /href=\{`\\/staff\\/academic\\/supervision\\/\\$\\{observation\\.id\\}`\}/);
```

- [ ] **Step 2: Verify RED**

Run:

```bash
cd frontend-school
npm run test:static -- supervision-booking.test.mjs
```

Expected: fail because the detail route/API functions do not exist.

- [ ] **Step 3: Implement typed API functions**

Add typed request interfaces and functions in `src/lib/api/supervision.ts`:

```ts
export type UpdateSupervisionObservationRequest = Partial<
    Pick<RequestSupervisionObservationRequest, 'timetableEntryId' | 'observedAt' | 'manualLesson'> & {
        templateId: string;
    }
>;

export interface ReplaceObservationEvaluatorsRequest {
    evaluators: EvaluatorAssignmentInput[];
}

export interface CancelObservationRequest {
    reason?: string | null;
}
```

Functions call the new backend endpoints and return `ApiResponse<SupervisionObservation>`.

- [ ] **Step 4: Implement guard-only route metadata**

`+page.ts` should use `_meta.access.permission = PERMISSION_MODULES.SUPERVISION` and load the `id` param.

- [ ] **Step 5: Implement detail page**

Use `PageShell`, `PageState`, `LoadingButton`, `Card`, `Dialog`, `Button`, `Badge`, `Input`, `Textarea`, and `Command/Popover` for evaluator selection. Keep observation state local:

```ts
let observation = $state<SupervisionObservation | null>(null);
function replaceObservation(updated: SupervisionObservation) {
    observation = updated;
}
```

Use action-specific loading state and patch `observation` from mutation responses.

- [ ] **Step 6: Link parent workspace to detail route**

Add clear "รายละเอียด" buttons/links from the existing observation cards and progress table.

- [ ] **Step 7: Verify frontend GREEN**

Run:

```bash
cd frontend-school
npm run test:static -- supervision-booking.test.mjs
npm run check
npm run lint
```

Expected: pass.

### Task 3: Final Verification And Publish

**Files:**
- All modified files from Tasks 1-2.

- [ ] **Step 1: Repository checks**

Run:

```bash
git diff --check
git status -sb
```

Expected: only intended supervision/detail plan files are changed.

- [ ] **Step 2: Commit and push**

Commit backend/frontend implementation and push `main`.

```bash
git add backend-school/src/modules/supervision backend-school/tests/static_architecture.rs frontend-school/src/lib/api/supervision.ts frontend-school/src/routes/'(app)'/staff/academic/supervision frontend-school/tests/static/supervision-booking.test.mjs docs/superpowers/plans/2026-06-21-supervision-observation-detail.md
git commit -m "Add supervision observation detail workflow"
git push origin main
```
