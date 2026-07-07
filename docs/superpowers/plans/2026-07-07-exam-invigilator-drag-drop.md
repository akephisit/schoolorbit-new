# Exam Invigilator Drag Drop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current checkbox-sheet invigilator workflow with a teacher-to-room drag/drop workspace that shows teacher workload and enforces one room per teacher per exam day.

**Architecture:** Add staff-level invigilator assign/remove endpoints that return a refreshed `ExamInvigilatorWorkspace`, keeping the existing full-list endpoint for compatibility. The frontend keeps the exam schedule detail page as the data owner, while `ExamInvigilatorPanel` coordinates day/search/pending state and delegates display to focused teacher-list and room-board components.

**Tech Stack:** Rust + Axum + sqlx backend, SvelteKit 5 + TypeScript frontend, local shadcn-svelte primitives, Node static tests, Rust unit/static tests.

---

## File Map

- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`
  - Add staff-level invigilator assign/remove services.
  - Add day-level move helpers and published-round guard.
  - Add focused service tests.
- Modify: `backend-school/src/modules/academic/handlers/exam_schedule.rs`
  - Add thin handlers for assign/remove staff invigilator actions.
- Modify: `backend-school/src/modules/academic.rs`
  - Register `PUT` and `DELETE` staff-level invigilator routes.
- Modify: `frontend-school/src/lib/api/examSchedule.ts`
  - Add typed client functions for assign/remove invigilator actions.
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`
  - Add static contract tests for new API functions and drag/drop UI shape.
- Create: `frontend-school/src/lib/components/academic/exam-schedule/invigilatorDrag.ts`
  - Share drag payload constants, card view types, and minute formatting.
- Create: `frontend-school/src/lib/components/academic/exam-schedule/InvigilatorStaffList.svelte`
  - Render draggable teacher cards with selected-day and round workload.
- Create: `frontend-school/src/lib/components/academic/exam-schedule/InvigilatorRoomCard.svelte`
  - Render one room drop target with invigilator chips and remove buttons.
- Create: `frontend-school/src/lib/components/academic/exam-schedule/InvigilatorRoomBoard.svelte`
  - Render selected-day room cards and empty state.
- Modify: `frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte`
  - Replace table/sheet workflow with the drag/drop layout.
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
  - Wire new API callbacks into `ExamInvigilatorPanel`.

---

### Task 1: Backend Staff-Level Invigilator Services

**Files:**
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`

- [ ] **Step 1: Add failing service tests**

Append these tests inside the existing `#[cfg(test)] mod tests` in `backend-school/src/modules/academic/services/exam_schedule_service.rs` near the existing invigilator tests:

```rust
#[test]
fn assign_invigilator_to_assignment_uses_day_level_move_semantics() {
    let source = include_str!("exam_schedule_service.rs");
    let start = source
        .find("pub async fn assign_invigilator_to_assignment")
        .expect("assign service should exist");
    let body = &source[start..source[start..]
        .find("pub async fn remove_invigilator_from_assignment")
        .map(|index| start + index)
        .unwrap_or(source.len())];

    let lock_position = body
        .find("lock_exam_invigilator_staff_conflict_scope")
        .expect("assign service should lock staff/day scope");
    let validate_position = body
        .find("validate_active_staff_users")
        .expect("assign service should validate active staff");
    let delete_position = body
        .find("delete_staff_invigilator_from_other_day_assignments_in_tx")
        .expect("assign service should remove staff from other rooms on the same day");
    let insert_position = body
        .find("insert_staff_invigilator_if_missing_in_tx")
        .expect("assign service should insert target staff");

    assert!(lock_position < validate_position);
    assert!(validate_position < delete_position);
    assert!(delete_position < insert_position);
    assert!(body.contains("ensure_exam_round_is_mutable"));
    assert!(body.contains("get_invigilator_workspace(pool, round_id)"));
}

#[test]
fn remove_invigilator_from_assignment_only_deletes_target_assignment() {
    let source = include_str!("exam_schedule_service.rs");
    let start = source
        .find("pub async fn remove_invigilator_from_assignment")
        .expect("remove service should exist");
    let body = &source[start..];

    assert!(body.contains("delete_staff_invigilator_from_assignment_in_tx"));
    assert!(!body.contains("delete_staff_invigilator_from_other_day_assignments_in_tx"));
    assert!(body.contains("ensure_exam_round_is_mutable"));
    assert!(body.contains("get_invigilator_workspace(pool, round_id)"));
}

#[test]
fn exam_round_mutation_guard_rejects_published_rounds() {
    assert!(ensure_exam_round_is_mutable("draft").is_ok());
    assert!(ensure_exam_round_is_mutable("published").is_err());
}
```

- [ ] **Step 2: Run backend tests to verify they fail**

Run:

```bash
cd backend-school
cargo test modules::academic::services::exam_schedule_service::tests::assign_invigilator_to_assignment_uses_day_level_move_semantics --bin backend-school
```

Expected: FAIL because `assign_invigilator_to_assignment` does not exist yet.

- [ ] **Step 3: Add mutation context and guard**

In `backend-school/src/modules/academic/services/exam_schedule_service.rs`, add this struct near `SeatAssignmentContext`:

```rust
#[derive(Debug, sqlx::FromRow)]
struct InvigilatorAssignmentMutationContext {
    assignment_id: Uuid,
    exam_day_id: Uuid,
    exam_round_id: Uuid,
    round_status: String,
}
```

Add this helper near `validate_unique_invigilator_staff_ids`:

```rust
fn ensure_exam_round_is_mutable(status: &str) -> Result<(), AppError> {
    if status == "published" {
        return Err(AppError::BadRequest(
            "Published exam rounds cannot be changed".to_string(),
        ));
    }

    Ok(())
}
```

Add this query helper near `fetch_seat_assignment_context`:

```rust
async fn fetch_invigilator_assignment_mutation_context_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    assignment_id: Uuid,
) -> Result<InvigilatorAssignmentMutationContext, AppError> {
    sqlx::query_as::<_, InvigilatorAssignmentMutationContext>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               exam_day.exam_round_id,
               exam_round.status AS round_status
        FROM academic_exam_day_room_assignments assignment
        JOIN academic_exam_days exam_day ON exam_day.id = assignment.exam_day_id
        JOIN academic_exam_rounds exam_round ON exam_round.id = exam_day.exam_round_id
        WHERE assignment.id = $1
        FOR UPDATE OF assignment, exam_round
        "#,
    )
    .bind(assignment_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam room assignment not found".to_string()))
}
```

- [ ] **Step 4: Add row-level mutation helpers**

Add these helpers near `replace_assignment_invigilators_in_tx`:

```rust
async fn delete_staff_invigilator_from_other_day_assignments_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    exam_day_id: Uuid,
    target_assignment_id: Uuid,
    staff_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"
        DELETE FROM academic_exam_day_invigilators invigilator
        USING academic_exam_day_room_assignments assignment
        WHERE assignment.id = invigilator.day_room_assignment_id
          AND assignment.exam_day_id = invigilator.exam_day_id
          AND assignment.exam_day_id = $1
          AND invigilator.staff_id = $2
          AND invigilator.day_room_assignment_id <> $3
        "#,
    )
    .bind(exam_day_id)
    .bind(staff_id)
    .bind(target_assignment_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

async fn insert_staff_invigilator_if_missing_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    exam_day_id: Uuid,
    assignment_id: Uuid,
    staff_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"
        INSERT INTO academic_exam_day_invigilators (
            exam_day_id,
            day_room_assignment_id,
            staff_id
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (day_room_assignment_id, staff_id) DO NOTHING
        "#,
    )
    .bind(exam_day_id)
    .bind(assignment_id)
    .bind(staff_id)
    .execute(&mut **tx)
    .await
    .map_err(map_day_room_assignment_write_error)?;

    Ok(result.rows_affected())
}

async fn delete_staff_invigilator_from_assignment_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    assignment_id: Uuid,
    staff_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"
        DELETE FROM academic_exam_day_invigilators
        WHERE day_room_assignment_id = $1
          AND staff_id = $2
        "#,
    )
    .bind(assignment_id)
    .bind(staff_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}
```

- [ ] **Step 5: Add assign/remove services**

Add these public service functions after `update_assignment_invigilators`:

```rust
pub async fn assign_invigilator_to_assignment(
    pool: &PgPool,
    assignment_id: Uuid,
    staff_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ExamInvigilatorWorkspace, AppError> {
    let mut tx = pool.begin().await?;
    let context =
        fetch_invigilator_assignment_mutation_context_for_update(&mut tx, assignment_id).await?;
    ensure_exam_round_is_mutable(&context.round_status)?;

    let staff_ids = vec![staff_id];
    lock_exam_invigilator_staff_conflict_scope(&mut tx, context.exam_day_id, &staff_ids).await?;
    validate_active_staff_users(&mut tx, &staff_ids).await?;

    let removed_count = delete_staff_invigilator_from_other_day_assignments_in_tx(
        &mut tx,
        context.exam_day_id,
        context.assignment_id,
        staff_id,
    )
    .await?;
    let inserted_count = insert_staff_invigilator_if_missing_in_tx(
        &mut tx,
        context.exam_day_id,
        context.assignment_id,
        staff_id,
    )
    .await?;

    let round_id = context.exam_round_id;
    if removed_count > 0 || inserted_count > 0 {
        mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    }
    tx.commit().await?;

    get_invigilator_workspace(pool, round_id).await
}

pub async fn remove_invigilator_from_assignment(
    pool: &PgPool,
    assignment_id: Uuid,
    staff_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ExamInvigilatorWorkspace, AppError> {
    let mut tx = pool.begin().await?;
    let context =
        fetch_invigilator_assignment_mutation_context_for_update(&mut tx, assignment_id).await?;
    ensure_exam_round_is_mutable(&context.round_status)?;

    let deleted_count =
        delete_staff_invigilator_from_assignment_in_tx(&mut tx, context.assignment_id, staff_id)
            .await?;

    let round_id = context.exam_round_id;
    if deleted_count > 0 {
        mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    }
    tx.commit().await?;

    get_invigilator_workspace(pool, round_id).await
}
```

- [ ] **Step 6: Run backend tests**

Run:

```bash
cd backend-school
cargo test modules::academic::services::exam_schedule_service::tests::assign_invigilator_to_assignment_uses_day_level_move_semantics --bin backend-school
cargo test modules::academic::services::exam_schedule_service::tests::remove_invigilator_from_assignment_only_deletes_target_assignment --bin backend-school
cargo test modules::academic::services::exam_schedule_service::tests::exam_round_mutation_guard_rejects_published_rounds --bin backend-school
```

Expected: all selected tests pass.

- [ ] **Step 7: Commit backend service work**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs
git commit -m "feat: add invigilator staff move service"
```

---

### Task 2: Backend Handlers And Routes

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/exam_schedule.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`

- [ ] **Step 1: Add failing route/source tests**

Append these tests inside `#[cfg(test)] mod tests` in `backend-school/src/modules/academic/services/exam_schedule_service.rs`:

```rust
#[test]
fn academic_routes_expose_staff_level_invigilator_actions() {
    let source = include_str!("../../academic.rs");

    assert!(source.contains(
        "/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}"
    ));
    assert!(source.contains("assign_assignment_invigilator"));
    assert!(source.contains("remove_assignment_invigilator"));
}

#[test]
fn exam_schedule_handler_uses_staff_level_invigilator_services() {
    let source = include_str!("../handlers/exam_schedule.rs");

    assert!(source.contains("pub async fn assign_assignment_invigilator"));
    assert!(source.contains("pub async fn remove_assignment_invigilator"));
    assert!(source.contains("exam_schedule_service::assign_invigilator_to_assignment"));
    assert!(source.contains("exam_schedule_service::remove_invigilator_from_assignment"));
    assert!(source.contains("Path((assignment_id, staff_id)): Path<(Uuid, Uuid)>"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd backend-school
cargo test modules::academic::services::exam_schedule_service::tests::academic_routes_expose_staff_level_invigilator_actions --bin backend-school
```

Expected: FAIL because the route is not registered yet.

- [ ] **Step 3: Add handlers**

In `backend-school/src/modules/academic/handlers/exam_schedule.rs`, add these handlers after `update_assignment_invigilators`:

```rust
/// PUT /api/academic/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}
pub async fn assign_assignment_invigilator(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((assignment_id, staff_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let workspace = exam_schedule_service::assign_invigilator_to_assignment(
        &pool,
        assignment_id,
        staff_id,
        actor.user_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(workspace)).into_response())
}

/// DELETE /api/academic/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}
pub async fn remove_assignment_invigilator(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((assignment_id, staff_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let workspace = exam_schedule_service::remove_invigilator_from_assignment(
        &pool,
        assignment_id,
        staff_id,
        actor.user_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(workspace)).into_response())
}
```

- [ ] **Step 4: Register route**

In `backend-school/src/modules/academic.rs`, add this route before the existing full-list invigilator route:

```rust
.route(
    "/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}",
    put(handlers::exam_schedule::assign_assignment_invigilator)
        .delete(handlers::exam_schedule::remove_assignment_invigilator),
)
```

Keep the existing route below it:

```rust
.route(
    "/exam-schedules/room-assignments/{assignment_id}/invigilators",
    put(handlers::exam_schedule::update_assignment_invigilators),
)
```

- [ ] **Step 5: Run tests and cargo check**

Run:

```bash
cd backend-school
cargo test modules::academic::services::exam_schedule_service::tests::academic_routes_expose_staff_level_invigilator_actions --bin backend-school
cargo test modules::academic::services::exam_schedule_service::tests::exam_schedule_handler_uses_staff_level_invigilator_services --bin backend-school
cargo check
```

Expected: selected tests pass and `cargo check` exits with code 0.

- [ ] **Step 6: Commit backend route work**

```bash
git add backend-school/src/modules/academic.rs backend-school/src/modules/academic/handlers/exam_schedule.rs backend-school/src/modules/academic/services/exam_schedule_service.rs
git commit -m "feat: expose invigilator drag actions"
```

---

### Task 3: Frontend API Client Contract

**Files:**
- Modify: `frontend-school/src/lib/api/examSchedule.ts`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing frontend static test**

Add this test near the existing invigilator API tests in `frontend-school/tests/static/academic-exam-schedule.test.mjs`:

```js
test('exam schedule API exposes staff-level invigilator drag actions', () => {
	const api = readFileSync(projectPath('src/lib/api/examSchedule.ts'), 'utf8');

	assert.match(api, /export async function assignExamAssignmentInvigilator/);
	assert.match(api, /export async function removeExamAssignmentInvigilator/);
	assert.match(
		api,
		/room-assignments\/\$\{assignmentId\}\/invigilators\/\$\{staffId\}/
	);
	assert.match(api, /apiClient\.put<ExamInvigilatorWorkspace>/);
	assert.match(api, /apiClient\.delete<ExamInvigilatorWorkspace>/);
});
```

- [ ] **Step 2: Run static test to verify it fails**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: FAIL because the two API functions do not exist yet.

- [ ] **Step 3: Add API functions**

In `frontend-school/src/lib/api/examSchedule.ts`, add these functions after `updateExamAssignmentInvigilators`:

```ts
export async function assignExamAssignmentInvigilator(
	assignmentId: string,
	staffId: string
): Promise<ExamInvigilatorWorkspace> {
	const response = await apiClient.put<ExamInvigilatorWorkspace>(
		`/api/academic/exam-schedules/room-assignments/${assignmentId}/invigilators/${staffId}`
	);
	return apiData(response, 'ไม่สามารถบันทึกกรรมการคุมสอบได้');
}

export async function removeExamAssignmentInvigilator(
	assignmentId: string,
	staffId: string
): Promise<ExamInvigilatorWorkspace> {
	const response = await apiClient.delete<ExamInvigilatorWorkspace>(
		`/api/academic/exam-schedules/room-assignments/${assignmentId}/invigilators/${staffId}`
	);
	return apiData(response, 'ไม่สามารถลบกรรมการคุมสอบได้');
}
```

- [ ] **Step 4: Run static test**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: all tests in the file pass.

- [ ] **Step 5: Commit frontend API contract**

```bash
git add frontend-school/src/lib/api/examSchedule.ts frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: add invigilator drag API client"
```

---

### Task 4: Drag Types And Static UI Guards

**Files:**
- Create: `frontend-school/src/lib/components/academic/exam-schedule/invigilatorDrag.ts`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing static test for drag UI structure**

Add this test near the existing invigilator panel tests:

```js
test('exam invigilator drag workflow uses teacher cards and room drop targets', () => {
	const panel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte'),
		'utf8'
	);
	const staffListPath =
		'src/lib/components/academic/exam-schedule/InvigilatorStaffList.svelte';
	const roomBoardPath =
		'src/lib/components/academic/exam-schedule/InvigilatorRoomBoard.svelte';
	const roomCardPath =
		'src/lib/components/academic/exam-schedule/InvigilatorRoomCard.svelte';
	const dragHelperPath = 'src/lib/components/academic/exam-schedule/invigilatorDrag.ts';

	for (const file of [staffListPath, roomBoardPath, roomCardPath, dragHelperPath]) {
		assert.equal(existsSync(projectPath(file)), true, `${file} should exist`);
	}

	const staffList = readFileSync(projectPath(staffListPath), 'utf8');
	const roomBoard = readFileSync(projectPath(roomBoardPath), 'utf8');
	const roomCard = readFileSync(projectPath(roomCardPath), 'utf8');
	const dragHelper = readFileSync(projectPath(dragHelperPath), 'utf8');

	assert.match(panel, /<InvigilatorStaffList/);
	assert.match(panel, /<InvigilatorRoomBoard/);
	assert.match(staffList, /draggable=/);
	assert.match(staffList, /วันนี้/);
	assert.match(staffList, /รวมรอบนี้/);
	assert.match(roomBoard, /InvigilatorRoomCard/);
	assert.match(roomCard, /ondrop=/);
	assert.match(roomCard, /กรรมการ \{assignment\.invigilators\.length\} คน/);
	assert.match(roomCard, /onRemoveInvigilator/);
	assert.match(dragHelper, /INVIGILATOR_STAFF_DRAG_TYPE/);
	assert.doesNotMatch(panel + staffList + roomBoard + roomCard, /แนะนำ 2 คน|2\/2/);
});
```

- [ ] **Step 2: Run static test to verify it fails**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: FAIL because the new files do not exist yet.

- [ ] **Step 3: Create drag helper**

Create `frontend-school/src/lib/components/academic/exam-schedule/invigilatorDrag.ts`:

```ts
import type {
	ExamInvigilatorAssignmentSummary,
	ExamInvigilatorStaffOption,
	ExamInvigilatorStaffWorkload
} from '$lib/api/examSchedule';

export const INVIGILATOR_STAFF_DRAG_TYPE =
	'application/x-schoolorbit-exam-invigilator-staff-id';

export type InvigilatorStaffCardView = {
	staffId: string;
	displayName: string;
	selectedDayMinutes: number;
	totalMinutes: number;
	assignedAssignment: ExamInvigilatorAssignmentSummary | null;
};

export function formatInvigilatorMinutes(minutes: number): string {
	const hours = Math.floor(minutes / 60);
	const remainder = minutes % 60;
	if (hours === 0) return `${remainder} นาที`;
	if (remainder === 0) return `${hours} ชม.`;
	return `${hours} ชม. ${remainder} นาที`;
}

export function workloadStaffName(workload: ExamInvigilatorStaffWorkload): string {
	return workload.staffName || workload.staffId;
}

export function staffOptionName(staff: ExamInvigilatorStaffOption): string {
	return staff.displayName || staff.staffId;
}
```

- [ ] **Step 4: Run static test**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: still FAIL because the Svelte components and panel wiring are not created yet.

- [ ] **Step 5: Commit helper and failing UI guard only after Task 5 green**

Do not commit this task by itself while the static suite is failing. Keep these changes staged together with Task 5.

---

### Task 5: Drag/Drop Invigilator Components

**Files:**
- Create: `frontend-school/src/lib/components/academic/exam-schedule/InvigilatorStaffList.svelte`
- Create: `frontend-school/src/lib/components/academic/exam-schedule/InvigilatorRoomCard.svelte`
- Create: `frontend-school/src/lib/components/academic/exam-schedule/InvigilatorRoomBoard.svelte`

- [ ] **Step 1: Create teacher list component**

Create `frontend-school/src/lib/components/academic/exam-schedule/InvigilatorStaffList.svelte`:

```svelte
<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { PageState } from '$lib/components/app-state';
	import {
		formatInvigilatorMinutes,
		INVIGILATOR_STAFF_DRAG_TYPE,
		type InvigilatorStaffCardView
	} from './invigilatorDrag';

	let {
		staffCards = [],
		search = '',
		showAvailableOnly = false,
		readonly = false,
		pendingStaffIds = [],
		onSearchChange,
		onShowAvailableOnlyChange
	}: {
		staffCards: InvigilatorStaffCardView[];
		search?: string;
		showAvailableOnly?: boolean;
		readonly?: boolean;
		pendingStaffIds?: string[];
		onSearchChange?: (value: string) => void;
		onShowAvailableOnlyChange?: (value: boolean) => void;
	} = $props();

	function handleDragStart(event: DragEvent, staffId: string) {
		if (readonly || pendingStaffIds.includes(staffId)) {
			event.preventDefault();
			return;
		}

		event.dataTransfer?.setData(INVIGILATOR_STAFF_DRAG_TYPE, staffId);
		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = 'move';
		}
	}
</script>

<section class="flex min-h-0 flex-col rounded-md border bg-background">
	<div class="space-y-3 border-b p-3">
		<div>
			<h3 class="text-sm font-semibold">ครู</h3>
			<p class="text-xs text-muted-foreground">{staffCards.length} คนในรายการ</p>
		</div>
		<div class="grid gap-2">
			<Label for="exam-invigilator-search">ค้นหาครู</Label>
			<Input
				id="exam-invigilator-search"
				type="search"
				value={search}
				placeholder="ชื่อครู"
				oninput={(event) => onSearchChange?.(event.currentTarget.value)}
			/>
		</div>
		<label class="flex items-center gap-2 text-sm">
			<Checkbox
				checked={showAvailableOnly}
				onCheckedChange={(checked) => onShowAvailableOnlyChange?.(checked === true)}
			/>
			<span>แสดงเฉพาะครูว่างวันนี้</span>
		</label>
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto p-3">
		{#if staffCards.length === 0}
			<PageState title="ไม่พบรายชื่อครู" description="ลองค้นหาด้วยคำอื่น" />
		{:else}
			<div class="space-y-2">
				{#each staffCards as staff (staff.staffId)}
					<article
						class="rounded-md border p-3 text-sm transition hover:border-primary/60 {pendingStaffIds.includes(
							staff.staffId
						)
							? 'opacity-60'
							: ''}"
						draggable={!readonly && !pendingStaffIds.includes(staff.staffId)}
						ondragstart={(event) => handleDragStart(event, staff.staffId)}
					>
						<div class="flex items-start justify-between gap-3">
							<div class="min-w-0">
								<p class="truncate font-medium">{staff.displayName}</p>
								<p class="mt-1 text-xs text-muted-foreground">
									วันนี้ {formatInvigilatorMinutes(staff.selectedDayMinutes)} · รวมรอบนี้
									{formatInvigilatorMinutes(staff.totalMinutes)}
								</p>
							</div>
							{#if staff.assignedAssignment}
								<Badge variant="outline" class="shrink-0">
									{staff.assignedAssignment.classroomName}
								</Badge>
							{:else}
								<Badge variant="secondary" class="shrink-0">ว่างวันนี้</Badge>
							{/if}
						</div>
					</article>
				{/each}
			</div>
		{/if}
	</div>
</section>
```

- [ ] **Step 2: Create room card component**

Create `frontend-school/src/lib/components/academic/exam-schedule/InvigilatorRoomCard.svelte`:

```svelte
<script lang="ts">
	import type { ExamInvigilatorAssignmentSummary } from '$lib/api/examSchedule';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { X } from 'lucide-svelte';
	import { INVIGILATOR_STAFF_DRAG_TYPE } from './invigilatorDrag';

	let {
		assignment,
		readonly = false,
		pendingAssignmentIds = [],
		pendingStaffIds = [],
		onAssignInvigilator,
		onRemoveInvigilator
	}: {
		assignment: ExamInvigilatorAssignmentSummary;
		readonly?: boolean;
		pendingAssignmentIds?: string[];
		pendingStaffIds?: string[];
		onAssignInvigilator?: (assignmentId: string, staffId: string) => void;
		onRemoveInvigilator?: (assignmentId: string, staffId: string) => void;
	} = $props();

	let dragOver = $state(false);

	const isSaving = $derived(pendingAssignmentIds.includes(assignment.assignmentId));

	function staffIdFromDrag(event: DragEvent): string {
		return event.dataTransfer?.getData(INVIGILATOR_STAFF_DRAG_TYPE) ?? '';
	}

	function handleDragOver(event: DragEvent) {
		if (readonly) return;
		if (!staffIdFromDrag(event)) return;

		event.preventDefault();
		dragOver = true;
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move';
		}
	}

	function handleDrop(event: DragEvent) {
		if (readonly) return;
		event.preventDefault();
		dragOver = false;

		const staffId = staffIdFromDrag(event);
		if (!staffId) return;
		if (assignment.invigilators.some((invigilator) => invigilator.staffId === staffId)) return;

		onAssignInvigilator?.(assignment.assignmentId, staffId);
	}
</script>

<article
	class="min-h-36 rounded-md border bg-background p-3 transition {dragOver
		? 'border-primary ring-2 ring-primary/20'
		: ''} {isSaving ? 'opacity-70' : ''}"
	ondragenter={(event) => handleDragOver(event)}
	ondragover={handleDragOver}
	ondragleave={() => (dragOver = false)}
	ondrop={handleDrop}
>
	<div class="flex items-start justify-between gap-3">
		<div class="min-w-0">
			<h3 class="truncate text-sm font-semibold">{assignment.classroomName || '-'}</h3>
			<p class="truncate text-xs text-muted-foreground">{assignment.roomName || '-'}</p>
		</div>
		<Badge variant="outline">กรรมการ {assignment.invigilators.length} คน</Badge>
	</div>

	<div class="mt-3 flex min-h-16 flex-wrap content-start gap-2 rounded-md border border-dashed p-2">
		{#if assignment.invigilators.length === 0}
			<p class="text-xs text-muted-foreground">ลากครูมาวางตรงนี้</p>
		{:else}
			{#each assignment.invigilators as invigilator (invigilator.staffId)}
				<Badge variant="secondary" class="gap-1 pr-1">
					<span>{invigilator.displayName}</span>
					{#if !readonly}
						<Button
							type="button"
							variant="ghost"
							size="icon"
							class="h-5 w-5"
							disabled={pendingStaffIds.includes(invigilator.staffId)}
							onclick={() =>
								onRemoveInvigilator?.(assignment.assignmentId, invigilator.staffId)}
							aria-label={`เอา ${invigilator.displayName} ออกจากห้อง ${assignment.classroomName}`}
						>
							<X class="h-3 w-3" />
						</Button>
					{/if}
				</Badge>
			{/each}
		{/if}
	</div>
</article>
```

- [ ] **Step 3: Create room board component**

Create `frontend-school/src/lib/components/academic/exam-schedule/InvigilatorRoomBoard.svelte`:

```svelte
<script lang="ts">
	import type { ExamInvigilatorAssignmentSummary } from '$lib/api/examSchedule';
	import { PageState } from '$lib/components/app-state';
	import InvigilatorRoomCard from './InvigilatorRoomCard.svelte';

	let {
		assignments = [],
		readonly = false,
		pendingAssignmentIds = [],
		pendingStaffIds = [],
		onAssignInvigilator,
		onRemoveInvigilator
	}: {
		assignments: ExamInvigilatorAssignmentSummary[];
		readonly?: boolean;
		pendingAssignmentIds?: string[];
		pendingStaffIds?: string[];
		onAssignInvigilator?: (assignmentId: string, staffId: string) => void;
		onRemoveInvigilator?: (assignmentId: string, staffId: string) => void;
	} = $props();
</script>

<section class="min-h-0 rounded-md border bg-background">
	<div class="border-b p-3">
		<h3 class="text-sm font-semibold">ห้องสอบ</h3>
		<p class="text-xs text-muted-foreground">{assignments.length} ห้องในวันที่เลือก</p>
	</div>

	<div class="min-h-0 overflow-y-auto p-3">
		{#if assignments.length === 0}
			<PageState
				title="ยังไม่มีห้องสอบในวันนี้"
				description="กำหนดห้องสอบในแท็บห้องสอบก่อนจัดกรรมการ"
			/>
		{:else}
			<div class="grid gap-3 md:grid-cols-2 2xl:grid-cols-3">
				{#each assignments as assignment (assignment.assignmentId)}
					<InvigilatorRoomCard
						{assignment}
						{readonly}
						{pendingAssignmentIds}
						{pendingStaffIds}
						{onAssignInvigilator}
						{onRemoveInvigilator}
					/>
				{/each}
			</div>
		{/if}
	</div>
</section>
```

- [ ] **Step 4: Run static test**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: still FAIL until `ExamInvigilatorPanel.svelte` imports and renders the components.

---

### Task 6: Replace Invigilator Panel Layout

**Files:**
- Modify: `frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte`

- [ ] **Step 1: Update panel imports and props**

Replace the old sheet/table-specific imports with these imports:

```svelte
<script lang="ts">
	import type {
		ExamDayDetail,
		ExamInvigilatorAssignmentSummary,
		ExamInvigilatorStaffOption,
		ExamInvigilatorWorkspace
	} from '$lib/api/examSchedule';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import * as Select from '$lib/components/ui/select';
	import { compareExamDaysByDate } from '$lib/utils/examScheduleDayOrder';
	import { RefreshCw } from 'lucide-svelte';
	import InvigilatorRoomBoard from './InvigilatorRoomBoard.svelte';
	import InvigilatorStaffList from './InvigilatorStaffList.svelte';
	import {
		staffOptionName,
		workloadStaffName,
		type InvigilatorStaffCardView
	} from './invigilatorDrag';
```

Update props to remove the old sheet save/search props and add drag actions:

```ts
	let {
		days = [],
		workspace,
		staff = [],
		loading = false,
		loadError = '',
		readonly = false,
		onAssignInvigilator,
		onRemoveInvigilator,
		onRetry
	}: {
		days: ExamDayDetail[];
		workspace: ExamInvigilatorWorkspace | null;
		staff: ExamInvigilatorStaffOption[];
		loading?: boolean;
		loadError?: string;
		readonly?: boolean;
		onAssignInvigilator?: (
			assignmentId: string,
			staffId: string
		) => Promise<ExamInvigilatorWorkspace>;
		onRemoveInvigilator?: (
			assignmentId: string,
			staffId: string
		) => Promise<ExamInvigilatorWorkspace>;
		onRetry?: () => Promise<void> | void;
	} = $props();
```

- [ ] **Step 2: Add panel state and derived data**

Use this state and derived data inside the script:

```ts
	let selectedDayId = $state('');
	let staffSearch = $state('');
	let showAvailableOnly = $state(false);
	let localWorkspace = $state<ExamInvigilatorWorkspace | null>(workspace);
	let pendingStaffIds = $state<string[]>([]);
	let pendingAssignmentIds = $state<string[]>([]);

	const sortedDays = $derived([...days].sort(compareExamDaysByDate));
	const selectedDay = $derived(
		days.find((day) => day.id === selectedDayId) ?? sortedDays[0] ?? null
	);
	const selectedDayAssignments = $derived(
		[...(localWorkspace?.assignments ?? [])]
			.filter((assignment) => assignment.examDayId === (selectedDay?.id ?? selectedDayId))
			.sort((a, b) => {
				const classroomCompare = a.classroomName.localeCompare(b.classroomName, 'th');
				return classroomCompare === 0
					? a.roomName.localeCompare(b.roomName, 'th')
					: classroomCompare;
			})
	);
	const dayLabel = $derived(
		selectedDay ? formatDayDate(selectedDay.examDate, selectedDay.label) : 'เลือกวันสอบ'
	);
	const staffCards = $derived(buildStaffCards());
	const displayedStaffCards = $derived(filterStaffCards(staffCards));
```

Add helpers:

```ts
	function formatDayDate(value: string, label?: string | null): string {
		const dateLabel = new Date(value).toLocaleDateString('th-TH', {
			weekday: 'short',
			month: 'short',
			day: 'numeric'
		});
		return label ? `${label} · ${dateLabel}` : dateLabel;
	}

	function selectedDayMinutes(staffId: string): number {
		const workload = localWorkspace?.staffWorkloads.find((item) => item.staffId === staffId);
		return workload?.days.find((day) => day.examDayId === selectedDayId)?.minutes ?? 0;
	}

	function selectedDayAssignment(staffId: string): ExamInvigilatorAssignmentSummary | null {
		return (
			selectedDayAssignments.find((assignment) =>
				assignment.invigilators.some((invigilator) => invigilator.staffId === staffId)
			) ?? null
		);
	}

	function buildStaffCards(): InvigilatorStaffCardView[] {
		const cards = new Map<string, InvigilatorStaffCardView>();

		for (const option of staff) {
			cards.set(option.staffId, {
				staffId: option.staffId,
				displayName: staffOptionName(option),
				selectedDayMinutes: selectedDayMinutes(option.staffId),
				totalMinutes:
					localWorkspace?.staffWorkloads.find((workload) => workload.staffId === option.staffId)
						?.totalMinutes ?? 0,
				assignedAssignment: selectedDayAssignment(option.staffId)
			});
		}

		for (const workload of localWorkspace?.staffWorkloads ?? []) {
			if (!cards.has(workload.staffId)) {
				cards.set(workload.staffId, {
					staffId: workload.staffId,
					displayName: workloadStaffName(workload),
					selectedDayMinutes: selectedDayMinutes(workload.staffId),
					totalMinutes: workload.totalMinutes,
					assignedAssignment: selectedDayAssignment(workload.staffId)
				});
			}
		}

		return [...cards.values()].sort((a, b) => a.displayName.localeCompare(b.displayName, 'th'));
	}

	function filterStaffCards(cards: InvigilatorStaffCardView[]): InvigilatorStaffCardView[] {
		const search = staffSearch.trim().toLowerCase();
		return cards.filter((card) => {
			if (showAvailableOnly && card.assignedAssignment) return false;
			if (!search) return true;
			return card.displayName.toLowerCase().includes(search);
		});
	}
```

- [ ] **Step 3: Add optimistic assign/remove methods**

Add these methods:

```ts
	function markPending(assignmentId: string, staffId: string) {
		if (!pendingAssignmentIds.includes(assignmentId)) {
			pendingAssignmentIds = [...pendingAssignmentIds, assignmentId];
		}
		if (!pendingStaffIds.includes(staffId)) {
			pendingStaffIds = [...pendingStaffIds, staffId];
		}
	}

	function clearPending(assignmentId: string, staffId: string) {
		pendingAssignmentIds = pendingAssignmentIds.filter((id) => id !== assignmentId);
		pendingStaffIds = pendingStaffIds.filter((id) => id !== staffId);
	}

	function applyWorkspace(workspaceData: ExamInvigilatorWorkspace) {
		localWorkspace = workspaceData;
	}

	async function assignInvigilator(assignmentId: string, staffId: string) {
		if (readonly || !onAssignInvigilator || pendingStaffIds.includes(staffId)) return;

		markPending(assignmentId, staffId);
		try {
			const updatedWorkspace = await onAssignInvigilator(assignmentId, staffId);
			applyWorkspace(updatedWorkspace);
		} finally {
			clearPending(assignmentId, staffId);
		}
	}

	async function removeInvigilator(assignmentId: string, staffId: string) {
		if (readonly || !onRemoveInvigilator || pendingStaffIds.includes(staffId)) return;

		markPending(assignmentId, staffId);
		try {
			const updatedWorkspace = await onRemoveInvigilator(assignmentId, staffId);
			applyWorkspace(updatedWorkspace);
		} finally {
			clearPending(assignmentId, staffId);
		}
	}
```

This first implementation uses action-scoped pending state and server-returned workspace replacement. It does not need manual optimistic patching because the API returns the canonical refreshed workspace quickly and keeps rollback simple.

- [ ] **Step 4: Add effects**

Replace the old sheet/search effects with:

```ts
	$effect(() => {
		localWorkspace = workspace;
	});

	$effect(() => {
		if (!selectedDayId && sortedDays[0]) {
			selectedDayId = sortedDays[0].id;
		}
		if (selectedDayId && !days.some((day) => day.id === selectedDayId)) {
			selectedDayId = sortedDays[0]?.id ?? '';
			pendingStaffIds = [];
			pendingAssignmentIds = [];
		}
	});
```

- [ ] **Step 5: Replace markup**

Replace the current table/aside/sheet markup with:

```svelte
{#if loadError}
	<section class="rounded-md border bg-background">
		<PageState
			variant="error"
			title="โหลดข้อมูลกรรมการคุมสอบไม่สำเร็จ"
			description={loadError}
			actionLabel="ลองอีกครั้ง"
			onaction={onRetry}
		/>
	</section>
{:else if localWorkspace === null}
	<section class="rounded-md border bg-background">
		<PageState
			title={loading ? 'กำลังโหลดข้อมูลกรรมการคุมสอบ' : 'ยังไม่มีข้อมูลกรรมการคุมสอบ'}
			description="ข้อมูลอ้างอิงจากห้องสอบที่กำหนดไว้ในรอบนี้"
		/>
	</section>
{:else}
	<section class="min-h-[calc(100vh-16rem)] overflow-hidden rounded-md border bg-muted/20">
		<div
			class="flex flex-col gap-3 border-b bg-background px-4 py-4 lg:flex-row lg:items-center lg:justify-between"
		>
			<div>
				<h2 class="font-semibold">จัดกรรมการคุมสอบ</h2>
				<p class="text-sm text-muted-foreground">
					ลากครูไปวางในห้องสอบของวันที่เลือก
				</p>
			</div>
			<div class="flex flex-wrap items-center gap-2">
				<Select.Root type="single" bind:value={selectedDayId}>
					<Select.Trigger class="w-full sm:w-64">{dayLabel}</Select.Trigger>
					<Select.Content>
						{#each sortedDays as day (day.id)}
							<Select.Item value={day.id}>{formatDayDate(day.examDate, day.label)}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				{#if onRetry}
					<LoadingButton
						variant="outline"
						size="sm"
						loading={loading}
						loadingLabel="กำลังโหลด..."
						onclick={onRetry}
					>
						<RefreshCw class="h-4 w-4" />
						รีเฟรช
					</LoadingButton>
				{/if}
			</div>
		</div>

		{#if !selectedDay}
			<PageState title="ยังไม่มีวันสอบ" description="ต้องมีวันสอบก่อนจัดกรรมการคุมสอบ" />
		{:else}
			<div class="grid min-h-0 gap-3 p-3 xl:grid-cols-[20rem_minmax(0,1fr)]">
				<InvigilatorStaffList
					staffCards={displayedStaffCards}
					search={staffSearch}
					{showAvailableOnly}
					{readonly}
					{pendingStaffIds}
					onSearchChange={(value) => (staffSearch = value)}
					onShowAvailableOnlyChange={(value) => (showAvailableOnly = value)}
				/>
				<InvigilatorRoomBoard
					assignments={selectedDayAssignments}
					{readonly}
					{pendingAssignmentIds}
					{pendingStaffIds}
					onAssignInvigilator={assignInvigilator}
					onRemoveInvigilator={removeInvigilator}
				/>
			</div>
		{/if}
	</section>
{/if}
```

- [ ] **Step 6: Run static and Svelte checks**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: static tests pass and `svelte-check` reports 0 errors and 0 warnings. If `svelte-check` finds Svelte syntax issues, fix the exact diagnostics and rerun the same command.

- [ ] **Step 7: Commit component and panel work**

```bash
git add frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte \
  frontend-school/src/lib/components/academic/exam-schedule/InvigilatorStaffList.svelte \
  frontend-school/src/lib/components/academic/exam-schedule/InvigilatorRoomBoard.svelte \
  frontend-school/src/lib/components/academic/exam-schedule/InvigilatorRoomCard.svelte \
  frontend-school/src/lib/components/academic/exam-schedule/invigilatorDrag.ts \
  frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: add invigilator drag board"
```

---

### Task 7: Page Wiring For New Actions

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing static test for page wiring**

Add this test near the existing staff workspace invigilator tests:

```js
test('staff workspace wires staff-level invigilator drag actions', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);

	assert.match(page, /assignExamAssignmentInvigilator/);
	assert.match(page, /removeExamAssignmentInvigilator/);
	assert.match(page, /async function handleAssignInvigilator/);
	assert.match(page, /async function handleRemoveInvigilator/);
	assert.match(page, /onAssignInvigilator=\{handleAssignInvigilator\}/);
	assert.match(page, /onRemoveInvigilator=\{handleRemoveInvigilator\}/);
});
```

- [ ] **Step 2: Run static test to verify it fails**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: FAIL because the page is not wired to the new API functions.

- [ ] **Step 3: Import new API functions**

In `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`, add imports from `$lib/api/examSchedule`:

```ts
		assignExamAssignmentInvigilator,
		removeExamAssignmentInvigilator,
```

- [ ] **Step 4: Add page callbacks**

Add these functions near `handleSaveInvigilators`:

```ts
	async function handleAssignInvigilator(assignmentId: string, staffId: string) {
		try {
			const updatedWorkspace = await assignExamAssignmentInvigilator(assignmentId, staffId);
			invigilatorWorkspace = updatedWorkspace;
			toast.success('บันทึกกรรมการคุมสอบแล้ว');
			return updatedWorkspace;
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'บันทึกกรรมการคุมสอบไม่สำเร็จ');
			throw saveError;
		}
	}

	async function handleRemoveInvigilator(assignmentId: string, staffId: string) {
		try {
			const updatedWorkspace = await removeExamAssignmentInvigilator(assignmentId, staffId);
			invigilatorWorkspace = updatedWorkspace;
			toast.success('ลบกรรมการคุมสอบแล้ว');
			return updatedWorkspace;
		} catch (saveError) {
			toast.error(saveError instanceof Error ? saveError.message : 'ลบกรรมการคุมสอบไม่สำเร็จ');
			throw saveError;
		}
	}
```

Keep `handleSaveInvigilators` until the old full-list endpoint is intentionally removed. It will no longer be passed to the new panel.

- [ ] **Step 5: Update component props**

In the `<ExamInvigilatorPanel>` call, remove:

```svelte
savingAssignmentId={savingInvigilatorAssignmentId}
onSaveInvigilators={handleSaveInvigilators}
onSearchStaff={searchStaffOptions}
```

Add:

```svelte
onAssignInvigilator={handleAssignInvigilator}
onRemoveInvigilator={handleRemoveInvigilator}
```

The final component call should include:

```svelte
<ExamInvigilatorPanel
	days={workspace.days}
	workspace={invigilatorWorkspace}
	{staff}
	loading={loadingInvigilators}
	loadError={invigilatorLoadError}
	readonly={!canManageExamSchedules || workspace.round.status === 'published'}
	onAssignInvigilator={handleAssignInvigilator}
	onRemoveInvigilator={handleRemoveInvigilator}
	onRetry={() => loadInvigilators()}
/>
```

- [ ] **Step 6: Run frontend checks**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: static tests pass and `svelte-check` reports 0 errors and 0 warnings.

- [ ] **Step 7: Commit page wiring**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte' \
  frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: wire invigilator drag actions"
```

---

### Task 8: Final Verification

**Files:**
- No code changes unless a verification command finds a defect.

- [ ] **Step 1: Run backend focused tests**

Run:

```bash
cd backend-school
cargo test modules::academic::services::exam_schedule_service::tests::assign_invigilator_to_assignment_uses_day_level_move_semantics --bin backend-school
cargo test modules::academic::services::exam_schedule_service::tests::remove_invigilator_from_assignment_only_deletes_target_assignment --bin backend-school
cargo test modules::academic::services::exam_schedule_service::tests::exam_round_mutation_guard_rejects_published_rounds --bin backend-school
cargo test modules::academic::services::exam_schedule_service::tests::academic_routes_expose_staff_level_invigilator_actions --bin backend-school
cargo test modules::academic::services::exam_schedule_service::tests::exam_schedule_handler_uses_staff_level_invigilator_services --bin backend-school
```

Expected: all selected tests pass.

- [ ] **Step 2: Run backend compile check**

Run:

```bash
cd backend-school
cargo check
```

Expected: exits with code 0.

- [ ] **Step 3: Run frontend static tests**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: all tests in the file pass.

- [ ] **Step 4: Run Svelte diagnostics**

Run:

```bash
cd frontend-school
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: `svelte-check found 0 errors and 0 warnings`.

- [ ] **Step 5: Run diff checks**

Run:

```bash
git diff --check
git status --short
```

Expected: `git diff --check` has no output. `git status --short` shows only intentional files if there are uncommitted verification fixes.

- [ ] **Step 6: Confirm no uncommitted verification fixes remain**

Run:

```bash
git status --short
```

Expected: no output. If output appears, inspect the files, run the relevant verification command from this task again after fixing them, and commit the exact files that changed before marking the plan complete.

---

## Self-Review Notes

- Spec coverage: The plan covers day-first layout, teacher-to-room drag/drop, immediate save, one room per teacher per day, move-on-drop behavior, no target count, workload display, backend authority, staff-level API actions, focused components, error handling through scoped pending state, and verification.
- Scope: This is one cohesive feature across backend action endpoints and the frontend invigilator tab. It does not change unrelated exam tabs or student/parent published views.
- Type consistency: Backend action endpoints return `ExamInvigilatorWorkspace`; frontend API callbacks and panel props use the same type. Staff ids and assignment ids remain strings on the frontend and `Uuid` in Rust handlers/services.
