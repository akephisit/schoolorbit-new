# Academic Exam Schedule UX Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh the academic exam schedule staff workspace so scheduling gets more space, invigilators are assigned later with workload visibility, drag/drop shows duration previews, and scheduled sessions can be unscheduled.

**Architecture:** Reuse the existing exam schedule database tables and service module. Add dedicated invigilator service APIs/workload DTOs, keep room assignment independent from invigilators, then refactor the Svelte workspace into schedule-first layout with compact status, sheet-based editors, invigilator tab, duration-aware drag preview, and unschedule actions.

**Tech Stack:** Rust + Axum + sqlx backend, SvelteKit 5 + TypeScript frontend, Tailwind, local shadcn-svelte primitives, static Node tests, Rust service unit tests.

---

## File Structure

Backend files:

- Modify `backend-school/src/modules/academic/models/exam_schedule.rs`
  - Add invigilator request/response DTOs.
  - Make room assignment invigilators optional for backward-compatible room updates.
- Modify `backend-school/src/modules/academic/services/exam_schedule_service.rs`
  - Extract invigilator update logic from room assignment updates.
  - Add workload and conflict pure helpers.
  - Add `get_invigilator_workspace()` and `update_assignment_invigilators()`.
  - Preserve existing invigilators when saving rooms without invigilator payload.
- Modify `backend-school/src/modules/academic/handlers/exam_schedule.rs`
  - Add handlers for invigilator workspace and assignment update.
- Modify `backend-school/src/modules/academic.rs`
  - Register invigilator routes.
- Modify `backend-school/tests/static_architecture.rs`
  - Extend route/permission static coverage.

Frontend files:

- Create `frontend-school/src/lib/components/ui/sheet/`
  - Shared shadcn-svelte sheet primitive built on Bits UI Dialog.
- Modify `frontend-school/src/lib/api/examSchedule.ts`
  - Add invigilator workload types/functions.
  - Remove invigilators from room assignment input contract.
- Create `frontend-school/src/lib/components/academic/exam-schedule/CompactExamScheduleStatus.svelte`
  - Compact status bar and readiness sheet.
- Modify `frontend-school/src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte`
  - Room/seat only, sheet editor, sticky save.
- Create `frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte`
  - Room-first invigilator assignment and workload summary.
- Modify `frontend-school/src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte`
  - Duration ghost preview and unschedule callbacks.
- Modify `frontend-school/src/lib/components/academic/exam-schedule/ExamItemTray.svelte`
  - Accept scheduled-session drops for unscheduling.
- Modify `frontend-school/src/lib/components/academic/exam-schedule/ExamSessionBlock.svelte`
  - Loading/removing state and better semantic styling.
- Modify `frontend-school/src/lib/utils/examScheduleTime.ts`
  - Add `buildTimelineDragPreview` and `formatExamMinutes` helpers used by the timeline, status bar, and invigilator workload components.
- Modify `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
  - Schedule-first layout, new tabs, invigilator data loading, unschedule wiring.
- Modify `frontend-school/tests/static/academic-exam-schedule.test.mjs`
  - Static UX/API contract tests.
- Modify `frontend-school/tests/static/exam-schedule-time.test.mjs`
  - Time helper tests for preview/formatting.

---

## Task 1: Backend Invigilator Domain Helpers

**Files:**
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`

- [ ] **Step 1: Add failing service tests for workload and conflict helpers**

Append these tests inside `#[cfg(test)] mod tests` in `exam_schedule_service.rs`:

```rust
#[test]
fn invigilator_workload_sums_session_minutes_without_gaps() {
    let assignment_id = Uuid::from_u128(1);
    let staff_id = Uuid::from_u128(2);
    let windows = vec![
        InvigilatorSessionWindow {
            assignment_id,
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("08:30"),
            ends_at: t("09:30"),
        },
        InvigilatorSessionWindow {
            assignment_id,
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("10:00"),
            ends_at: t("11:30"),
        },
    ];

    let minutes = invigilator_workload_minutes(&windows);

    assert_eq!(minutes, 150);
}

#[test]
fn invigilator_conflict_rejects_overlapping_live_session_ranges() {
    let staff_id = Uuid::from_u128(7);
    let candidate = vec![InvigilatorSessionWindow {
        assignment_id: Uuid::from_u128(1),
        exam_day_id: Uuid::from_u128(10),
        staff_id,
        starts_at: t("08:30"),
        ends_at: t("09:30"),
    }];
    let existing = vec![InvigilatorSessionWindow {
        assignment_id: Uuid::from_u128(2),
        exam_day_id: Uuid::from_u128(10),
        staff_id,
        starts_at: t("09:00"),
        ends_at: t("10:00"),
    }];

    assert!(has_invigilator_time_conflict(Uuid::from_u128(1), &candidate, &existing));
}

#[test]
fn invigilator_conflict_allows_non_overlapping_same_day_assignments() {
    let staff_id = Uuid::from_u128(7);
    let candidate = vec![InvigilatorSessionWindow {
        assignment_id: Uuid::from_u128(1),
        exam_day_id: Uuid::from_u128(10),
        staff_id,
        starts_at: t("08:30"),
        ends_at: t("09:30"),
    }];
    let existing = vec![InvigilatorSessionWindow {
        assignment_id: Uuid::from_u128(2),
        exam_day_id: Uuid::from_u128(10),
        staff_id,
        starts_at: t("09:30"),
        ends_at: t("10:30"),
    }];

    assert!(!has_invigilator_time_conflict(Uuid::from_u128(1), &candidate, &existing));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
```

Expected: compile fails because `InvigilatorSessionWindow`, `invigilator_workload_minutes`, and `has_invigilator_time_conflict` do not exist.

- [ ] **Step 3: Add helper structs and pure functions**

Add near the other pure helper structs in `exam_schedule_service.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvigilatorSessionWindow {
    pub assignment_id: Uuid,
    pub exam_day_id: Uuid,
    pub staff_id: Uuid,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
}

pub fn invigilator_workload_minutes(windows: &[InvigilatorSessionWindow]) -> i32 {
    windows
        .iter()
        .map(|window| minutes_between_times(window.starts_at, window.ends_at))
        .sum()
}

fn minutes_between_times(starts_at: NaiveTime, ends_at: NaiveTime) -> i32 {
    let start_minutes = starts_at.num_seconds_from_midnight() / 60;
    let end_minutes = ends_at.num_seconds_from_midnight() / 60;
    end_minutes.saturating_sub(start_minutes) as i32
}

pub fn has_invigilator_time_conflict(
    candidate_assignment_id: Uuid,
    candidate_windows: &[InvigilatorSessionWindow],
    existing_windows: &[InvigilatorSessionWindow],
) -> bool {
    candidate_windows.iter().any(|candidate| {
        existing_windows.iter().any(|existing| {
            existing.assignment_id != candidate_assignment_id
                && existing.staff_id == candidate.staff_id
                && existing.exam_day_id == candidate.exam_day_id
                && time_ranges_overlap(
                    candidate.starts_at,
                    candidate.ends_at,
                    existing.starts_at,
                    existing.ends_at,
                )
        })
    })
}
```

- [ ] **Step 4: Run tests to verify pass**

Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
```

Expected: all `exam_schedule_service` tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs
git commit -m "test: cover exam invigilator workload helpers"
```

---

## Task 2: Backend Invigilator DTOs And Room Payload Split

**Files:**
- Modify: `backend-school/src/modules/academic/models/exam_schedule.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`

- [ ] **Step 1: Add failing tests for room save preserving invigilators when payload omits them**

Add this static/pure test in `exam_schedule_service.rs` tests:

```rust
#[test]
fn room_assignment_payload_without_invigilators_preserves_existing_staff() {
    let request = serde_json::json!({
        "classroomId": Uuid::from_u128(1),
        "roomId": Uuid::from_u128(2),
        "capacityOverride": null
    });

    let parsed: UpsertDayRoomAssignmentRequest = serde_json::from_value(request).unwrap();

    assert_eq!(parsed.invigilator_staff_ids, None);
}

#[test]
fn room_assignment_payload_with_invigilators_remains_backwards_compatible() {
    let staff_id = Uuid::from_u128(3);
    let request = serde_json::json!({
        "classroomId": Uuid::from_u128(1),
        "roomId": Uuid::from_u128(2),
        "capacityOverride": null,
        "invigilatorStaffIds": [staff_id]
    });

    let parsed: UpsertDayRoomAssignmentRequest = serde_json::from_value(request).unwrap();

    assert_eq!(parsed.invigilator_staff_ids, Some(vec![staff_id]));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
```

Expected: the first test fails to deserialize because `invigilatorStaffIds` is currently required, or compile fails because the field is still `Vec<Uuid>`.

- [ ] **Step 3: Update DTOs**

In `models/exam_schedule.rs`, change `UpsertDayRoomAssignmentRequest` and add the new invigilator request:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertDayRoomAssignmentRequest {
    pub classroom_id: Uuid,
    pub room_id: Uuid,
    pub capacity_override: Option<i32>,
    #[serde(default)]
    pub invigilator_staff_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExamInvigilatorsRequest {
    pub invigilator_staff_ids: Vec<Uuid>,
}
```

- [ ] **Step 4: Update room assignment service to preserve invigilators unless explicitly provided**

In `upsert_day_room_assignment`, replace the first invigilator line with:

```rust
    let invigilator_staff_ids = request
        .invigilator_staff_ids
        .as_ref()
        .map(|ids| validate_unique_invigilator_staff_ids(ids.clone()))
        .transpose()?;
```

Replace the unconditional delete/insert block with:

```rust
    if let Some(invigilator_staff_ids) = invigilator_staff_ids {
        replace_assignment_invigilators_in_tx(
            &mut tx,
            day_context.exam_round_id,
            exam_day_id,
            assignment_id,
            &invigilator_staff_ids,
        )
        .await?;
    }
```

Move the existing delete/insert logic into this compatibility helper; conflict validation is added in Task 3:

```rust
async fn replace_assignment_invigilators_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    _round_id: Uuid,
    exam_day_id: Uuid,
    assignment_id: Uuid,
    invigilator_staff_ids: &[Uuid],
) -> Result<(), AppError> {
    validate_active_staff_users(tx, invigilator_staff_ids).await?;

    sqlx::query(
        r#"
        DELETE FROM academic_exam_day_invigilators
        WHERE day_room_assignment_id = $1
        "#,
    )
    .bind(assignment_id)
    .execute(&mut **tx)
    .await?;

    if invigilator_staff_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO academic_exam_day_invigilators (
            exam_day_id,
            day_room_assignment_id,
            staff_id
        )
        SELECT $1, $2, staff_id
        FROM unnest($3::uuid[]) AS staff_id
        "#,
    )
    .bind(exam_day_id)
    .bind(assignment_id)
    .bind(invigilator_staff_ids)
    .execute(&mut **tx)
    .await
    .map_err(map_day_room_assignment_write_error)?;

    Ok(())
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
cargo check
```

Expected: tests and check pass.

- [ ] **Step 6: Commit**

```bash
git add backend-school/src/modules/academic/models/exam_schedule.rs backend-school/src/modules/academic/services/exam_schedule_service.rs
git commit -m "feat: split exam room and invigilator payloads"
```

---

## Task 3: Backend Invigilator Workspace And Conflict Validation

**Files:**
- Modify: `backend-school/src/modules/academic/models/exam_schedule.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`
- Modify: `backend-school/src/modules/academic/handlers/exam_schedule.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/tests/static_architecture.rs`

- [ ] **Step 1: Add failing route/static tests**

In `backend-school/tests/static_architecture.rs`, extend `academic_exam_schedule_routes_are_registered_and_authorized()` with:

```rust
    assert!(academic_routes.contains("/exam-schedules/{round_id}/invigilators"));
    assert!(academic_routes.contains("/exam-schedules/room-assignments/{assignment_id}/invigilators"));
    assert!(exam_handler.contains("get_invigilator_workspace"));
    assert!(exam_handler.contains("update_assignment_invigilators"));
    assert_handler_permission(
        &exam_handler,
        "get_invigilator_workspace",
        "ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL",
    );
    assert_handler_permission(
        &exam_handler,
        "update_assignment_invigilators",
        "ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL",
    );
```

- [ ] **Step 2: Run static test to verify failure**

Run:

```bash
cd backend-school
cargo test --test static_architecture academic_exam_schedule_routes_are_registered_and_authorized
```

Expected: fails because invigilator routes/handlers do not exist.

- [ ] **Step 3: Add response DTOs**

In `models/exam_schedule.rs`, add:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorAssignmentSummary {
    pub assignment_id: Uuid,
    pub exam_day_id: Uuid,
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub room_id: Uuid,
    pub room_name: String,
    pub session_minutes: i32,
    pub invigilators: Vec<InvigilatorView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorDayWorkload {
    pub exam_day_id: Uuid,
    pub minutes: i32,
    pub assignment_count: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorStaffWorkload {
    pub staff_id: Uuid,
    pub staff_name: String,
    pub total_minutes: i32,
    pub assigned_day_count: i32,
    pub assignment_count: i32,
    pub days: Vec<ExamInvigilatorDayWorkload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorWorkspace {
    pub round_id: Uuid,
    pub assignments: Vec<ExamInvigilatorAssignmentSummary>,
    pub staff_workloads: Vec<ExamInvigilatorStaffWorkload>,
}
```

- [ ] **Step 4: Add service methods and SQL row structs**

In `exam_schedule_service.rs`, import the new DTOs and add row structs:

```rust
#[derive(Debug, sqlx::FromRow)]
struct InvigilatorAssignmentSummaryRow {
    assignment_id: Uuid,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    classroom_name: String,
    room_id: Uuid,
    room_name: String,
    session_minutes: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct InvigilatorSessionWindowRow {
    assignment_id: Uuid,
    exam_day_id: Uuid,
    staff_id: Uuid,
    staff_name: String,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
}
```

Add:

```rust
pub async fn get_invigilator_workspace(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ExamInvigilatorWorkspace, AppError> {
    let assignments = fetch_invigilator_assignment_summaries(pool, round_id).await?;
    let assignment_ids: Vec<Uuid> = assignments.iter().map(|item| item.assignment_id).collect();
    let mut invigilators_by_assignment =
        fetch_invigilator_views_by_assignment_ids(pool, &assignment_ids).await?;
    let staff_workloads = fetch_invigilator_staff_workloads(pool, round_id).await?;

    Ok(ExamInvigilatorWorkspace {
        round_id,
        assignments: assignments
            .into_iter()
            .map(|row| ExamInvigilatorAssignmentSummary {
                assignment_id: row.assignment_id,
                exam_day_id: row.exam_day_id,
                classroom_id: row.classroom_id,
                classroom_name: row.classroom_name,
                room_id: row.room_id,
                room_name: row.room_name,
                session_minutes: row.session_minutes,
                invigilators: invigilators_by_assignment
                    .remove(&row.assignment_id)
                    .unwrap_or_default(),
            })
            .collect(),
        staff_workloads,
    })
}
```

Add query helper:

```rust
async fn fetch_invigilator_assignment_summaries(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<InvigilatorAssignmentSummaryRow>, AppError> {
    sqlx::query_as::<_, InvigilatorAssignmentSummaryRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               day.id AS exam_day_id,
               assignment.classroom_id,
               classroom.name AS classroom_name,
               assignment.room_id,
               room.name_th AS room_name,
               COALESCE(SUM(EXTRACT(EPOCH FROM (session.ends_at - session.starts_at)) / 60), 0)::INT
                   AS session_minutes
        FROM academic_exam_day_room_assignments assignment
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        LEFT JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE day.exam_round_id = $1
        GROUP BY assignment.id, day.id, assignment.classroom_id, classroom.name, assignment.room_id, room.name_th
        ORDER BY day.sort_order, classroom.name, room.name_th, assignment.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}
```

Add `BTreeMap` and `BTreeSet` beside the existing collection imports:

```rust
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
```

Implement `fetch_invigilator_staff_workloads()` by querying session windows and passing the rows to a deterministic Rust grouping helper:

```rust
async fn fetch_invigilator_staff_workloads(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamInvigilatorStaffWorkload>, AppError> {
    let rows = sqlx::query_as::<_, InvigilatorSessionWindowRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               invigilator.staff_id,
               concat_ws(' ', user_account.title, user_account.first_name, user_account.last_name)
                   AS staff_name,
               session.starts_at,
               session.ends_at
        FROM academic_exam_day_invigilators invigilator
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.id = invigilator.day_room_assignment_id
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN users user_account ON user_account.id = invigilator.staff_id
        JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE day.exam_round_id = $1
        ORDER BY staff_name, day.sort_order, session.starts_at, assignment.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    Ok(build_invigilator_staff_workloads(rows))
}
```

Add the pure grouping helper:

```rust
#[derive(Debug, Default)]
struct StaffWorkloadAccumulator {
    staff_name: String,
    day_minutes: BTreeMap<Uuid, i32>,
    day_assignments: BTreeMap<Uuid, BTreeSet<Uuid>>,
    assignments: BTreeSet<Uuid>,
}

fn build_invigilator_staff_workloads(
    rows: Vec<InvigilatorSessionWindowRow>,
) -> Vec<ExamInvigilatorStaffWorkload> {
    let mut by_staff: BTreeMap<Uuid, StaffWorkloadAccumulator> = BTreeMap::new();

    for row in rows {
        let minutes = minutes_between_times(row.starts_at, row.ends_at);
        let accumulator = by_staff
            .entry(row.staff_id)
            .or_insert_with(|| StaffWorkloadAccumulator {
                staff_name: row.staff_name.clone(),
                ..Default::default()
            });

        *accumulator.day_minutes.entry(row.exam_day_id).or_insert(0) += minutes;
        accumulator
            .day_assignments
            .entry(row.exam_day_id)
            .or_default()
            .insert(row.assignment_id);
        accumulator.assignments.insert(row.assignment_id);
    }

    by_staff
        .into_iter()
        .map(|(staff_id, accumulator)| {
            let days = accumulator
                .day_minutes
                .iter()
                .map(|(exam_day_id, minutes)| ExamInvigilatorDayWorkload {
                    exam_day_id: *exam_day_id,
                    minutes: *minutes,
                    assignment_count: accumulator
                        .day_assignments
                        .get(exam_day_id)
                        .map(|assignment_ids| assignment_ids.len() as i32)
                        .unwrap_or(0),
                })
                .collect::<Vec<_>>();

            ExamInvigilatorStaffWorkload {
                staff_id,
                staff_name: accumulator.staff_name,
                total_minutes: days.iter().map(|day| day.minutes).sum(),
                assigned_day_count: days.len() as i32,
                assignment_count: accumulator.assignments.len() as i32,
                days,
            }
        })
        .collect()
}
```

- [ ] **Step 5: Add update service with conflict validation**

Add:

```rust
pub async fn update_assignment_invigilators(
    pool: &PgPool,
    assignment_id: Uuid,
    request: UpdateExamInvigilatorsRequest,
    actor_user_id: Uuid,
) -> Result<DayRoomAssignmentView, AppError> {
    let invigilator_staff_ids = validate_unique_invigilator_staff_ids(request.invigilator_staff_ids)?;
    let mut tx = pool.begin().await?;
    let context = fetch_seat_assignment_context(&mut tx, assignment_id).await?;
    let exam_day_id: Uuid = sqlx::query_scalar(
        "SELECT exam_day_id FROM academic_exam_day_room_assignments WHERE id = $1",
    )
    .bind(assignment_id)
    .fetch_one(&mut *tx)
    .await?;

    validate_invigilator_time_conflicts(&mut tx, context.exam_round_id, assignment_id, &invigilator_staff_ids)
        .await?;
    replace_assignment_invigilators_in_tx(
        &mut tx,
        context.exam_round_id,
        exam_day_id,
        assignment_id,
        &invigilator_staff_ids,
    )
    .await?;
    mark_round_draft_after_mutation(&mut tx, context.exam_round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    fetch_day_room_assignment_view(pool, assignment_id).await
}
```

Add `fetch_assignment_session_windows()`:

```rust
async fn fetch_assignment_session_windows(
    tx: &mut Transaction<'_, Postgres>,
    assignment_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<Vec<InvigilatorSessionWindow>, AppError> {
    if staff_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, InvigilatorSessionWindowRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               staff.staff_id,
               '' AS staff_name,
               session.starts_at,
               session.ends_at
        FROM academic_exam_day_room_assignments assignment
        JOIN unnest($2::uuid[]) AS staff(staff_id) ON TRUE
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE assignment.id = $1
        ORDER BY session.starts_at, staff.staff_id
        "#,
    )
    .bind(assignment_id)
    .bind(staff_ids)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| InvigilatorSessionWindow {
            assignment_id: row.assignment_id,
            exam_day_id: row.exam_day_id,
            staff_id: row.staff_id,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
        })
        .collect())
}
```

Add `fetch_existing_invigilator_session_windows()`:

```rust
async fn fetch_existing_invigilator_session_windows(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<Vec<InvigilatorSessionWindow>, AppError> {
    if staff_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, InvigilatorSessionWindowRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               invigilator.staff_id,
               '' AS staff_name,
               session.starts_at,
               session.ends_at
        FROM academic_exam_day_invigilators invigilator
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.id = invigilator.day_room_assignment_id
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE day.exam_round_id = $1
          AND invigilator.staff_id = ANY($2)
        ORDER BY assignment.exam_day_id, session.starts_at, invigilator.staff_id
        "#,
    )
    .bind(round_id)
    .bind(staff_ids)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| InvigilatorSessionWindow {
            assignment_id: row.assignment_id,
            exam_day_id: row.exam_day_id,
            staff_id: row.staff_id,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
        })
        .collect())
}
```

Add `validate_invigilator_time_conflicts()`:

```rust
async fn validate_invigilator_time_conflicts(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
    assignment_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<(), AppError> {
    if staff_ids.is_empty() {
        return Ok(());
    }

    let candidate_windows = fetch_assignment_session_windows(tx, assignment_id, staff_ids).await?;
    if candidate_windows.is_empty() {
        return Ok(());
    }

    let existing_windows = fetch_existing_invigilator_session_windows(tx, round_id, staff_ids).await?;
    if has_invigilator_time_conflict(assignment_id, &candidate_windows, &existing_windows) {
        return Err(AppError::BadRequest(
            "Invigilator has an overlapping exam supervision assignment".to_string(),
        ));
    }

    Ok(())
}
```

- [ ] **Step 6: Add handlers and routes**

In `handlers/exam_schedule.rs`, import `UpdateExamInvigilatorsRequest` and add:

```rust
pub async fn get_invigilator_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL)?;

    let workspace = exam_schedule_service::get_invigilator_workspace(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(workspace)).into_response())
}

pub async fn update_assignment_invigilators(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(assignment_id): Path<Uuid>,
    Json(payload): Json<UpdateExamInvigilatorsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let assignment = exam_schedule_service::update_assignment_invigilators(
        &pool,
        assignment_id,
        payload,
        actor.user_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(assignment)).into_response())
}
```

In `academic.rs`, register before publish route:

```rust
        .route(
            "/exam-schedules/{round_id}/invigilators",
            get(handlers::exam_schedule::get_invigilator_workspace),
        )
        .route(
            "/exam-schedules/room-assignments/{assignment_id}/invigilators",
            put(handlers::exam_schedule::update_assignment_invigilators),
        )
```

- [ ] **Step 7: Run backend tests**

Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
cargo test --test static_architecture academic_exam_schedule_routes_are_registered_and_authorized
cargo check
```

Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add backend-school/src/modules/academic/models/exam_schedule.rs backend-school/src/modules/academic/services/exam_schedule_service.rs backend-school/src/modules/academic/handlers/exam_schedule.rs backend-school/src/modules/academic.rs backend-school/tests/static_architecture.rs
git commit -m "feat: add exam invigilator workload api"
```

---

## Task 4: Frontend API Contracts And Static Guards

**Files:**
- Modify: `frontend-school/src/lib/api/examSchedule.ts`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing static API tests**

In `academic-exam-schedule.test.mjs`, add:

```js
test('exam schedule API exposes invigilator workspace and updates separately from room assignment', () => {
  const api = readFile('frontend-school/src/lib/api/examSchedule.ts');

  assert.match(api, /export interface ExamInvigilatorWorkspace/);
  assert.match(api, /export async function getExamInvigilatorWorkspace/);
  assert.match(api, /export async function updateExamAssignmentInvigilators/);
  assert.match(api, /room-assignments\\/\\$\\{assignmentId\\}\\/invigilators/);

  const roomInputStart = api.indexOf('export interface UpsertDayRoomAssignmentInput');
  const roomInputEnd = api.indexOf('export interface GenerateSeatsInput');
  const roomInput = api.slice(roomInputStart, roomInputEnd);
  assert.doesNotMatch(roomInput, /invigilatorStaffIds/);
});
```

- [ ] **Step 2: Run static test to verify failure**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
```

Expected: fails because the API types/functions do not exist and room input still contains invigilators.

- [ ] **Step 3: Update TypeScript API contracts**

In `examSchedule.ts`, remove `invigilatorStaffIds` from `UpsertDayRoomAssignmentInput`:

```ts
export interface UpsertDayRoomAssignmentInput {
	classroomId: string;
	roomId: string;
	capacityOverride?: number | null;
}
```

Add:

```ts
export interface ExamInvigilatorAssignmentSummary {
	assignmentId: string;
	examDayId: string;
	classroomId: string;
	classroomName: string;
	roomId: string;
	roomName: string;
	sessionMinutes: number;
	invigilators: InvigilatorView[];
}

export interface ExamInvigilatorDayWorkload {
	examDayId: string;
	minutes: number;
	assignmentCount: number;
}

export interface ExamInvigilatorStaffWorkload {
	staffId: string;
	staffName: string;
	totalMinutes: number;
	assignedDayCount: number;
	assignmentCount: number;
	days: ExamInvigilatorDayWorkload[];
}

export interface ExamInvigilatorWorkspace {
	roundId: string;
	assignments: ExamInvigilatorAssignmentSummary[];
	staffWorkloads: ExamInvigilatorStaffWorkload[];
}

export interface UpdateExamInvigilatorsInput {
	invigilatorStaffIds: string[];
}
```

Add:

```ts
export async function getExamInvigilatorWorkspace(
	roundId: string
): Promise<ExamInvigilatorWorkspace> {
	const response = await apiClient.get<ExamInvigilatorWorkspace>(
		`/api/academic/exam-schedules/${roundId}/invigilators`
	);
	return apiData(response, 'ไม่สามารถโหลดข้อมูลกรรมการคุมสอบได้');
}

export async function updateExamAssignmentInvigilators(
	assignmentId: string,
	input: UpdateExamInvigilatorsInput
): Promise<DayRoomAssignmentView> {
	const response = await apiClient.put<DayRoomAssignmentView>(
		`/api/academic/exam-schedules/room-assignments/${assignmentId}/invigilators`,
		input
	);
	return apiData(response, 'ไม่สามารถบันทึกกรรมการคุมสอบได้');
}
```

- [ ] **Step 4: Run frontend static tests**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
```

Expected: all static tests pass.

- [ ] **Step 5: Commit**

```bash
git add frontend-school/src/lib/api/examSchedule.ts frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: add exam invigilator frontend api"
```

---

## Task 5: Shared shadcn Sheet Primitive

**Files:**
- Create: `frontend-school/src/lib/components/ui/sheet/sheet.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/sheet-portal.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/sheet-overlay.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/sheet-content.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/sheet-header.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/sheet-footer.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/sheet-title.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/sheet-description.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/sheet-close.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/sheet-trigger.svelte`
- Create: `frontend-school/src/lib/components/ui/sheet/index.ts`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing static test for shared sheet primitive**

Add:

```js
test('exam schedule refresh uses shared shadcn sheet primitive instead of feature-local drawers', () => {
  assertFileExists('frontend-school/src/lib/components/ui/sheet/index.ts');
  const index = readFile('frontend-school/src/lib/components/ui/sheet/index.ts');
  assert.match(index, /Content as SheetContent/);
  assert.match(index, /Header as SheetHeader/);
  assert.match(index, /Footer as SheetFooter/);
});
```

- [ ] **Step 2: Run static test to verify failure**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
```

Expected: fails because `ui/sheet` files do not exist.

- [ ] **Step 3: Implement sheet primitive**

Use the existing `dialog` primitive pattern. `sheet.svelte`:

```svelte
<script lang="ts">
	import { Dialog as SheetPrimitive } from 'bits-ui';
	import type { WithoutChildrenOrChild } from '$lib/utils.js';

	let { ref = $bindable(null), ...restProps }: WithoutChildrenOrChild<SheetPrimitive.RootProps> = $props();
</script>

<SheetPrimitive.Root bind:ref {...restProps} />
```

`sheet-portal.svelte`:

```svelte
<script lang="ts">
	import { Dialog as SheetPrimitive } from 'bits-ui';
	import type { WithoutChildrenOrChild } from '$lib/utils.js';

	let { ...restProps }: WithoutChildrenOrChild<SheetPrimitive.PortalProps> = $props();
</script>

<SheetPrimitive.Portal {...restProps} />
```

`sheet-overlay.svelte`:

```svelte
<script lang="ts">
	import { Dialog as SheetPrimitive } from 'bits-ui';
	import { cn, type WithoutChildrenOrChild } from '$lib/utils.js';

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: WithoutChildrenOrChild<SheetPrimitive.OverlayProps> = $props();
</script>

<SheetPrimitive.Overlay
	bind:ref
	data-slot="sheet-overlay"
	class={cn(
		'fixed inset-0 z-50 bg-black/50 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
		className
	)}
	{...restProps}
/>
```

`sheet-content.svelte`:

```svelte
<script lang="ts">
	import { Dialog as SheetPrimitive } from 'bits-ui';
	import XIcon from '@lucide/svelte/icons/x';
	import type { ComponentProps, Snippet } from 'svelte';
	import { cn, type WithoutChildrenOrChild } from '$lib/utils.js';
	import SheetPortal from './sheet-portal.svelte';
	import * as Sheet from './index.js';

	type Side = 'top' | 'right' | 'bottom' | 'left';

	let {
		ref = $bindable(null),
		class: className,
		side = 'right',
		portalProps,
		children,
		showCloseButton = true,
		...restProps
	}: WithoutChildrenOrChild<SheetPrimitive.ContentProps> & {
		side?: Side;
		portalProps?: WithoutChildrenOrChild<ComponentProps<typeof SheetPortal>>;
		children: Snippet;
		showCloseButton?: boolean;
	} = $props();

	const sideClasses: Record<Side, string> = {
		top: 'inset-x-0 top-0 h-auto border-b data-[state=closed]:slide-out-to-top data-[state=open]:slide-in-from-top',
		right:
			'inset-y-0 right-0 h-full w-3/4 border-l data-[state=closed]:slide-out-to-right data-[state=open]:slide-in-from-right sm:max-w-md',
		bottom:
			'inset-x-0 bottom-0 h-auto border-t data-[state=closed]:slide-out-to-bottom data-[state=open]:slide-in-from-bottom',
		left: 'inset-y-0 left-0 h-full w-3/4 border-r data-[state=closed]:slide-out-to-left data-[state=open]:slide-in-from-left sm:max-w-md'
	};
</script>

<SheetPortal {...portalProps}>
	<Sheet.Overlay />
	<SheetPrimitive.Content
		bind:ref
		data-slot="sheet-content"
		class={cn(
			'bg-background fixed z-50 flex flex-col gap-4 shadow-lg transition ease-in-out data-[state=closed]:duration-300 data-[state=open]:duration-500 data-[state=open]:animate-in data-[state=closed]:animate-out',
			sideClasses[side],
			className
		)}
		{...restProps}
	>
		{@render children?.()}
		{#if showCloseButton}
			<SheetPrimitive.Close class="ring-offset-background focus:ring-ring absolute right-4 top-4 rounded-xs opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-offset-2 focus:outline-hidden">
				<XIcon class="size-4" />
				<span class="sr-only">Close</span>
			</SheetPrimitive.Close>
		{/if}
	</SheetPrimitive.Content>
</SheetPortal>
```

Create header and footer:

```svelte
<!-- sheet-header.svelte -->
<script lang="ts">
	import { cn, type WithElementRef } from '$lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	let { ref = $bindable(null), class: className, ...restProps }: WithElementRef<HTMLAttributes<HTMLDivElement>> = $props();
</script>

<div bind:this={ref} data-slot="sheet-header" class={cn('flex flex-col gap-1.5 p-6', className)} {...restProps} />
```

```svelte
<!-- sheet-footer.svelte -->
<script lang="ts">
	import { cn, type WithElementRef } from '$lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	let { ref = $bindable(null), class: className, ...restProps }: WithElementRef<HTMLAttributes<HTMLDivElement>> = $props();
</script>

<div bind:this={ref} data-slot="sheet-footer" class={cn('mt-auto flex flex-col-reverse gap-2 border-t p-4 sm:flex-row sm:justify-end', className)} {...restProps} />
```

Create title, description, close, and trigger:

```svelte
<!-- sheet-title.svelte -->
<script lang="ts">
	import { Dialog as SheetPrimitive } from 'bits-ui';

	let { ref = $bindable(null), ...restProps }: SheetPrimitive.TitleProps = $props();
</script>

<SheetPrimitive.Title bind:ref data-slot="sheet-title" {...restProps} />
```

```svelte
<!-- sheet-description.svelte -->
<script lang="ts">
	import { Dialog as SheetPrimitive } from 'bits-ui';

	let { ref = $bindable(null), ...restProps }: SheetPrimitive.DescriptionProps = $props();
</script>

<SheetPrimitive.Description bind:ref data-slot="sheet-description" {...restProps} />
```

```svelte
<!-- sheet-close.svelte -->
<script lang="ts">
	import { Dialog as SheetPrimitive } from 'bits-ui';

	let { ref = $bindable(null), ...restProps }: SheetPrimitive.CloseProps = $props();
</script>

<SheetPrimitive.Close bind:ref data-slot="sheet-close" {...restProps} />
```

```svelte
<!-- sheet-trigger.svelte -->
<script lang="ts">
	import { Dialog as SheetPrimitive } from 'bits-ui';

	let { ref = $bindable(null), ...restProps }: SheetPrimitive.TriggerProps = $props();
</script>

<SheetPrimitive.Trigger bind:ref data-slot="sheet-trigger" {...restProps} />
```

`index.ts`:

```ts
import Root from './sheet.svelte';
import Portal from './sheet-portal.svelte';
import Overlay from './sheet-overlay.svelte';
import Content from './sheet-content.svelte';
import Header from './sheet-header.svelte';
import Footer from './sheet-footer.svelte';
import Title from './sheet-title.svelte';
import Description from './sheet-description.svelte';
import Close from './sheet-close.svelte';
import Trigger from './sheet-trigger.svelte';

export {
	Root,
	Portal,
	Overlay,
	Content,
	Header,
	Footer,
	Title,
	Description,
	Close,
	Trigger,
	Root as Sheet,
	Portal as SheetPortal,
	Overlay as SheetOverlay,
	Content as SheetContent,
	Header as SheetHeader,
	Footer as SheetFooter,
	Title as SheetTitle,
	Description as SheetDescription,
	Close as SheetClose,
	Trigger as SheetTrigger
};
```

- [ ] **Step 4: Run frontend verification**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: static tests pass and `svelte-check` has 0 errors/warnings.

- [ ] **Step 5: Commit**

```bash
git add frontend-school/src/lib/components/ui/sheet frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: add shared sheet ui primitive"
```

---

## Task 6: Compact Status Bar And Schedule-First Layout

**Files:**
- Create: `frontend-school/src/lib/components/academic/exam-schedule/CompactExamScheduleStatus.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing static layout test**

Add:

```js
test('staff exam schedule detail uses compact status and removes large readiness aside', () => {
  const page = readFile('frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte');

  assert.match(page, /CompactExamScheduleStatus/);
  assert.doesNotMatch(page, /<aside class="min-w-0 xl:sticky/);
  assert.doesNotMatch(page, /xl:grid-cols-\\[minmax\\(0,1fr\\)_22rem\\]/);
  assert.match(page, /value="invigilators"/);
});
```

- [ ] **Step 2: Run static test to verify failure**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
```

Expected: fails because status component and invigilator tab are not wired.

- [ ] **Step 3: Create compact status component**

Create `CompactExamScheduleStatus.svelte`:

```svelte
<script lang="ts">
	import type {
		ExamDayDetail,
		ExamRoundStatus,
		ExamScheduleItem,
		ExamScheduleReadiness,
		ExamSession
	} from '$lib/api/examSchedule';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Sheet from '$lib/components/ui/sheet';
	import { AlertTriangle, CheckCircle2, Eye } from 'lucide-svelte';

	let {
		status = 'draft',
		readiness,
		days = [],
		unscheduledItems = [],
		scheduledSessions = [],
		invigilatorAssignedCount = 0,
		invigilatorAssignmentCount = 0
	}: {
		status?: ExamRoundStatus;
		readiness: ExamScheduleReadiness;
		days: ExamDayDetail[];
		unscheduledItems: ExamScheduleItem[];
		scheduledSessions: ExamSession[];
		invigilatorAssignedCount?: number;
		invigilatorAssignmentCount?: number;
	} = $props();

	const roomAssignmentCount = $derived(
		days.reduce((total, day) => total + day.roomAssignments.length, 0)
	);
	const totalItems = $derived(unscheduledItems.length + scheduledSessions.length);
</script>

<section class="flex flex-wrap items-center gap-2 rounded-md border bg-background px-3 py-2">
	<Badge variant={status === 'published' ? 'default' : 'secondary'}>
		{status === 'published' ? 'เผยแพร่แล้ว' : 'ฉบับร่าง'}
	</Badge>
	<Badge variant={readiness.canPublish ? 'default' : 'secondary'} class={readiness.canPublish ? 'bg-emerald-600 text-white' : ''}>
		{#if readiness.canPublish}
			<CheckCircle2 class="h-3.5 w-3.5" />
			พร้อมเผยแพร่
		{:else}
			<AlertTriangle class="h-3.5 w-3.5" />
			ยังไม่พร้อม
		{/if}
	</Badge>
	<Badge variant="outline">ยังไม่จัด {unscheduledItems.length}/{totalItems}</Badge>
	<Badge variant="outline">ห้องสอบ {roomAssignmentCount}</Badge>
	<Badge variant="outline">กรรมการ {invigilatorAssignedCount}/{invigilatorAssignmentCount}</Badge>

	<Sheet.Root>
		<Sheet.Trigger>
			{#snippet child({ props })}
				<Button {...props} type="button" variant="outline" size="sm" class="ml-auto">
					<Eye class="h-4 w-4" />
					ดูความพร้อม
				</Button>
			{/snippet}
		</Sheet.Trigger>
		<Sheet.Content side="right" class="sm:max-w-lg">
			<Sheet.Header>
				<Sheet.Title>ความพร้อมก่อนเผยแพร่</Sheet.Title>
				<Sheet.Description>รายการตรวจสอบของรอบตารางสอบนี้</Sheet.Description>
			</Sheet.Header>
			<div class="flex-1 space-y-3 overflow-y-auto px-6 pb-6">
				{#if readiness.blockers.length === 0}
					<div class="rounded-md border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-900">
						ไม่มีรายการติดขัด
					</div>
				{:else}
					{#each readiness.blockers as blocker, index (`${index}-${blocker}`)}
						<div class="rounded-md border border-amber-200 bg-amber-50 p-3 text-sm text-amber-950">
							{blocker}
						</div>
					{/each}
				{/if}
			</div>
		</Sheet.Content>
	</Sheet.Root>
</section>
```

- [ ] **Step 4: Integrate layout in page**

In `+page.svelte`:

- Replace `ReadinessPanel` import with `CompactExamScheduleStatus`.
- Do not import `ExamInvigilatorPanel` in this task. For this task, add the tab trigger and temporary `PageState` content; Task 8 replaces that content with the real panel.
- Replace the outer two-column grid with a single full-width workspace:

```svelte
<div class="space-y-4">
	<CompactExamScheduleStatus
		status={workspace.round.status}
		readiness={workspace.readiness}
		days={workspace.days}
		unscheduledItems={workspace.unscheduledItems}
		scheduledSessions={workspace.scheduledSessions}
	/>

	<Tabs.Root bind:value={activeTab} class="gap-4">
		<Tabs.List class="grid w-full grid-cols-4 md:w-fit">
			<Tabs.Trigger value="setup">Setup</Tabs.Trigger>
			<Tabs.Trigger value="rooms">Rooms</Tabs.Trigger>
			<Tabs.Trigger value="schedule">Schedule</Tabs.Trigger>
			<Tabs.Trigger value="invigilators">Invigilators</Tabs.Trigger>
		</Tabs.List>
		<Tabs.Content value="setup">
			<!-- Move the existing setup tab content here unchanged. -->
		</Tabs.Content>
		<Tabs.Content value="rooms">
			<!-- Move the existing rooms tab content here unchanged. -->
		</Tabs.Content>
		<Tabs.Content value="schedule">
			<!-- Move the existing schedule tab content here unchanged. -->
		</Tabs.Content>
		<Tabs.Content value="invigilators">
			<PageState title="ยังไม่ได้เปิดหน้าจัดกรรมการ" description="จะเพิ่มในขั้นถัดไป" />
		</Tabs.Content>
	</Tabs.Root>
</div>
```

Remove the old `<aside>` with `ReadinessPanel` and remove the `review` tab.

- [ ] **Step 5: Run tests**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: static tests pass and `svelte-check` has 0 errors/warnings.

- [ ] **Step 6: Commit**

```bash
git add frontend-school/src/lib/components/academic/exam-schedule/CompactExamScheduleStatus.svelte frontend-school/src/routes/\(app\)/staff/academic/exam-schedules/\[id\]/+page.svelte frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: compact exam schedule readiness status"
```

---

## Task 7: Room Assignment Panel Cleanup And Sheet Editor

**Files:**
- Modify: `frontend-school/src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing static test**

Add:

```js
test('exam room assignment panel is room and seat only with sheet editing', () => {
  const panel = readFile('frontend-school/src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte');

  assert.match(panel, /\\$lib\\/components\\/ui\\/sheet/);
  assert.doesNotMatch(panel, /staffSearch/);
  assert.doesNotMatch(panel, /selectedInvigilatorIds/);
  assert.doesNotMatch(panel, /invigilatorStaffIds/);
  assert.match(panel, /บันทึกห้องสอบ/);
  assert.match(panel, /sticky|mt-auto/);
});
```

- [ ] **Step 2: Run static test to verify failure**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
```

Expected: fails because the panel still contains staff search and invigilator form state.

- [ ] **Step 3: Refactor panel state**

Remove:

- `StaffListItem` import.
- `Checkbox` import.
- `Search`, `Users` imports.
- `InvigilatorOption` type.
- `staff`, `onSearchStaff`, invigilator props.
- `selectedInvigilatorIds`, `selectedInvigilatorOptions`, `staffOptions`, `staffSearch`, `staffSearching`, `staffSearchError`, `lastStaffSearch`, `staffSearchRequestToken`.
- `toggleInvigilator`, staff search `$effect`, and `invigilatorNames`.

Add sheet state:

```ts
let editorOpen = $state(false);
let editingAssignmentId = $state<string | null>(null);
```

Change `loadAssignment()` to open the sheet:

```ts
function loadAssignment(assignment: ExamDayRoomAssignmentView) {
	selectedDayId = assignment.examDayId;
	classroomId = assignment.classroomId;
	roomId = assignment.roomId;
	capacityOverride = assignment.capacityOverride ? String(assignment.capacityOverride) : '';
	editingAssignmentId = assignment.id;
	editorOpen = true;
}
```

Change `resetForm()`:

```ts
function resetForm() {
	classroomId = '';
	roomId = '';
	capacityOverride = '';
	editingAssignmentId = null;
}
```

Change `submitForm()` input:

```ts
const saved = await onSaveAssignment?.(dayId, {
	classroomId,
	roomId,
	capacityOverride: capacityOverride ? Number(capacityOverride) : null
});
if (saved) {
	resetForm();
	editorOpen = false;
}
```

- [ ] **Step 4: Replace right-side inline form with Sheet**

Use:

```svelte
<Sheet.Root bind:open={editorOpen}>
	<Sheet.Content side="right" class="sm:max-w-md">
		<Sheet.Header>
			<Sheet.Title>{editingAssignmentId ? 'แก้ไขห้องสอบ' : 'เพิ่มห้องสอบ'}</Sheet.Title>
			<Sheet.Description>กำหนดห้องสอบและความจุสำหรับห้องเรียนในวันที่เลือก</Sheet.Description>
		</Sheet.Header>
		<form
			class="flex min-h-0 flex-1 flex-col"
			onsubmit={(event) => {
				event.preventDefault();
				submitForm();
			}}
		>
			<div class="flex-1 space-y-4 overflow-y-auto px-6 py-2">
				<!-- existing classroom, room, capacity controls -->
			</div>
			<Sheet.Footer class="sticky bottom-0 bg-background">
				<Button type="button" variant="outline" onclick={() => (editorOpen = false)}>ยกเลิก</Button>
				<LoadingButton type="submit" loading={saving} loadingLabel="กำลังบันทึก..." disabled={!selectedDay || !classroomId || !roomId}>
					บันทึกห้องสอบ
				</LoadingButton>
			</Sheet.Footer>
		</form>
	</Sheet.Content>
</Sheet.Root>
```

Keep the table and seat generation button. Replace the old form area with an “เพิ่มห้องสอบ” button that opens the sheet:

```svelte
<Button type="button" onclick={() => { resetForm(); editorOpen = true; }}>เพิ่มห้องสอบ</Button>
```

- [ ] **Step 5: Run frontend checks**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: static tests pass and `svelte-check` has 0 errors/warnings.

- [ ] **Step 6: Commit**

```bash
git add frontend-school/src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: simplify exam room assignment panel"
```

---

## Task 8: Invigilator Panel And Workload Summary

**Files:**
- Create: `frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing static test**

Add:

```js
test('exam invigilator panel exposes room-first workflow and workload summary', () => {
  assertFileExists('frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte');
  const panel = readFile('frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte');
  const page = readFile('frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte');

  assert.match(panel, /staffWorkloads/);
  assert.match(panel, /sessionMinutes/);
  assert.match(panel, /จัดกรรมการ/);
  assert.match(panel, /แนะนำ 2 คน/);
  assert.match(panel, /updateExamAssignmentInvigilators|onSaveInvigilators/);
  assert.match(page, /getExamInvigilatorWorkspace/);
  assert.match(page, /<ExamInvigilatorPanel/);
});
```

- [ ] **Step 2: Run static test to verify failure**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
```

Expected: fails because panel/page wiring does not exist.

- [ ] **Step 3: Create `ExamInvigilatorPanel.svelte`**

Implement props:

```ts
let {
	days = [],
	workspace,
	staff = [],
	readonly = false,
	savingAssignmentId = null,
	onSaveInvigilators,
	onSearchStaff
}: {
	days: ExamDayDetail[];
	workspace: ExamInvigilatorWorkspace | null;
	staff: StaffListItem[];
	readonly?: boolean;
	savingAssignmentId?: string | null;
	onSaveInvigilators?: (assignmentId: string, staffIds: string[]) => Promise<boolean> | boolean;
	onSearchStaff?: (search: string) => Promise<StaffListItem[]>;
} = $props();
```

Use state:

```ts
let selectedDayId = $state('');
let editorOpen = $state(false);
let selectedAssignmentId = $state<string | null>(null);
let selectedStaffIds = $state<string[]>([]);
let staffSearch = $state('');
let staffOptions = $state<StaffListItem[]>([]);
```

Render:

- Top day `Select`.
- Left/main `Table` filtered by selected day.
- Right/inline workload summary cards:
  - Whole round list from `workspace.staffWorkloads`.
  - Selected day list using `workload.days.find(day => day.examDayId === selectedDayId)`.
- `Sheet` editor with staff search and checkbox list.
- Badge “แนะนำ 2 คน” when selected count is below 2, informational only.

Add helpers:

```ts
function formatMinutes(minutes: number): string {
	const hours = Math.floor(minutes / 60);
	const remainder = minutes % 60;
	if (hours === 0) return `${remainder} นาที`;
	if (remainder === 0) return `${hours} ชม.`;
	return `${hours} ชม. ${remainder} นาที`;
}
```

- [ ] **Step 4: Wire page data loading**

In `+page.svelte` import:

```ts
import {
	getExamInvigilatorWorkspace,
	updateExamAssignmentInvigilators,
	type ExamInvigilatorWorkspace
} from '$lib/api/examSchedule';
import ExamInvigilatorPanel from '$lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte';
```

Add state:

```ts
let invigilatorWorkspace = $state<ExamInvigilatorWorkspace | null>(null);
let loadingInvigilators = $state(false);
let savingInvigilatorAssignmentId = $state<string | null>(null);
```

Add:

```ts
async function loadInvigilators(roundId = workspace?.round.id ?? loadedRoundId) {
	if (!roundId) return;
	loadingInvigilators = true;
	try {
		invigilatorWorkspace = await getExamInvigilatorWorkspace(roundId);
	} catch (loadError) {
		toast.error(loadError instanceof Error ? loadError.message : 'โหลดข้อมูลกรรมการคุมสอบไม่สำเร็จ');
	} finally {
		loadingInvigilators = false;
	}
}

async function handleSaveInvigilators(assignmentId: string, staffIds: string[]): Promise<boolean> {
	savingInvigilatorAssignmentId = assignmentId;
	try {
		await updateExamAssignmentInvigilators(assignmentId, { invigilatorStaffIds: staffIds });
		toast.success('บันทึกกรรมการคุมสอบแล้ว');
		await Promise.all([refreshWorkspace(), loadInvigilators()]);
		return true;
	} catch (saveError) {
		toast.error(saveError instanceof Error ? saveError.message : 'บันทึกกรรมการคุมสอบไม่สำเร็จ');
		return false;
	} finally {
		savingInvigilatorAssignmentId = null;
	}
}
```

In the invigilators tab:

```svelte
<ExamInvigilatorPanel
	days={workspace.days}
	workspace={invigilatorWorkspace}
	staff={staff}
	readonly={!canManageExamSchedules || workspace.round.status === 'published'}
	savingAssignmentId={savingInvigilatorAssignmentId}
	onSaveInvigilators={handleSaveInvigilators}
	onSearchStaff={searchStaffOptions}
/>
```

When active tab becomes `invigilators` and `invigilatorWorkspace === null`, call `loadInvigilators()`.

- [ ] **Step 5: Update compact status coverage counts**

Pass:

```svelte
invigilatorAssignedCount={invigilatorWorkspace?.assignments.filter((assignment) => assignment.invigilators.length > 0).length ?? 0}
invigilatorAssignmentCount={invigilatorWorkspace?.assignments.length ?? 0}
```

- [ ] **Step 6: Run checks**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: static tests pass and `svelte-check` has 0 errors/warnings.

- [ ] **Step 7: Commit**

```bash
git add frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte frontend-school/src/routes/\(app\)/staff/academic/exam-schedules/\[id\]/+page.svelte frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: add exam invigilator workspace"
```

---

## Task 9: Timeline Drag Preview

**Files:**
- Modify: `frontend-school/src/lib/utils/examScheduleTime.ts`
- Modify: `frontend-school/tests/static/exam-schedule-time.test.mjs`
- Modify: `frontend-school/src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing time helper tests**

In `exam-schedule-time.test.mjs`, add:

```js
test('timeline drag preview reports snapped start end left width and validity', async () => {
  const module = await import('../../src/lib/utils/examScheduleTime.ts');
  const day = {
    id: 'day-1',
    startTime: '08:30:00',
    endTime: '12:00:00',
    gradeLevelIds: [],
    blockedWindows: []
  };

  const preview = module.buildTimelineDragPreview({
    day,
    clientX: 156,
    trackLeft: 100,
    dragOffsetPx: 0,
    slotWidthPx: 24,
    durationMinutes: 60,
    candidate: {
      examScheduleItemId: 'item-1',
      classroomId: 'classroom-1',
      gradeLevelId: 'grade-1'
    },
    scheduledSessions: []
  });

  assert.equal(preview.startTime, '09:00');
  assert.equal(preview.endTime, '10:00');
  assert.equal(preview.widthPx, 96);
  assert.equal(preview.valid, true);
});
```

- [ ] **Step 2: Run helper test to verify failure**

Run:

```bash
cd frontend-school
node --test tests/static/exam-schedule-time.test.mjs
```

Expected: fails because `buildTimelineDragPreview` does not exist.

- [ ] **Step 3: Implement helper**

In `examScheduleTime.ts`, add:

```ts
export interface TimelineDragPreviewInput {
	day: ExamDayLike;
	clientX: number;
	trackLeft: number;
	dragOffsetPx: number;
	slotWidthPx: number;
	durationMinutes: number;
	candidate: {
		examScheduleItemId: string;
		classroomId: string;
		gradeLevelId: string;
		sourceSessionId?: string;
	};
	scheduledSessions: ExamSessionLike[];
}

export interface TimelineDragPreview {
	startTime: string;
	endTime: string;
	leftPx: number;
	widthPx: number;
	valid: boolean;
	reason?: string;
}

export function buildTimelineDragPreview(input: TimelineDragPreviewInput): TimelineDragPreview {
	const startTime = clientXToTimelineStartTime({
		clientX: input.clientX,
		trackLeft: input.trackLeft,
		dragOffsetPx: input.dragOffsetPx,
		dayStartTime: input.day.startTime,
		slotWidthPx: input.slotWidthPx
	});
	const validation = validateExamSessionPlacement({
		day: input.day,
		candidate: {
			...input.candidate,
			startTime,
			durationMinutes: input.durationMinutes
		},
		scheduledSessions: input.scheduledSessions
	});

	return {
		startTime,
		endTime: addMinutes(startTime, input.durationMinutes),
		leftPx: (minutesBetween(input.day.startTime, startTime) / TIMELINE_SLOT_MINUTES) * input.slotWidthPx,
		widthPx: Math.max(input.slotWidthPx, (input.durationMinutes / TIMELINE_SLOT_MINUTES) * input.slotWidthPx),
		valid: validation.ok,
		reason: validation.reason
	};
}
```

- [ ] **Step 4: Add failing static timeline test**

In `academic-exam-schedule.test.mjs`, add:

```js
test('staff timeline renders duration-aware drag preview states', () => {
  const timeline = readFile('frontend-school/src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte');

  assert.match(timeline, /buildTimelineDragPreview/);
  assert.match(timeline, /dragPreview/);
  assert.match(timeline, /preview\\.valid/);
  assert.match(timeline, /preview\\.startTime/);
  assert.match(timeline, /preview\\.endTime/);
});
```

- [ ] **Step 5: Implement timeline preview state**

In `ExamScheduleTimeline.svelte`:

- Import `buildTimelineDragPreview`.
- Add state:

```ts
let dragPreview = $state<{
	dayId: string;
	classroomId: string;
	leftPx: number;
	widthPx: number;
	startTime: string;
	endTime: string;
	valid: boolean;
	reason?: string;
} | null>(null);
```

- Add `ondragleave` and `ondragend` cleanup.
- In `handleDragOver(event, day, assignmentClassroomId)`, parse payload and update preview using the helper.
- Render preview inside the row before session blocks:

```svelte
{#if dragPreview?.dayId === day.id && dragPreview.classroomId === assignment.classroomId}
	<div
		class="pointer-events-none absolute top-1 rounded border-2 px-2 py-1 text-xs shadow-sm"
		class:border-blue-500={dragPreview.valid}
		class:bg-blue-50={dragPreview.valid}
		class:text-blue-900={dragPreview.valid}
		class:border-destructive={ !dragPreview.valid }
		class:bg-destructive/10={ !dragPreview.valid }
		class:text-destructive={ !dragPreview.valid }
		style:left={`${dragPreview.leftPx}px`}
		style:width={`${dragPreview.widthPx}px`}
	>
		<div class="truncate font-mono">{dragPreview.startTime}-{dragPreview.endTime}</div>
		{#if dragPreview.reason}
			<div class="truncate text-[10px]">{dragPreview.reason}</div>
		{/if}
	</div>
{/if}
```

- [ ] **Step 6: Run frontend checks**

Run:

```bash
cd frontend-school
node --test tests/static/exam-schedule-time.test.mjs
npm run test:static -- academic-exam-schedule
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add frontend-school/src/lib/utils/examScheduleTime.ts frontend-school/tests/static/exam-schedule-time.test.mjs frontend-school/src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: preview exam session duration while dragging"
```

---

## Task 10: Unschedule Interaction

**Files:**
- Modify: `frontend-school/src/lib/components/academic/exam-schedule/ExamItemTray.svelte`
- Modify: `frontend-school/src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] **Step 1: Add failing static test**

Add:

```js
test('scheduled exam sessions can be removed through dialog and tray drop', () => {
  const tray = readFile('frontend-school/src/lib/components/academic/exam-schedule/ExamItemTray.svelte');
  const timeline = readFile('frontend-school/src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte');
  const page = readFile('frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte');

  assert.match(tray, /onUnscheduleSession/);
  assert.match(tray, /ondrop/);
  assert.match(timeline, /เอาออกจากตาราง/);
  assert.match(timeline, /onUnscheduleSession/);
  assert.match(page, /deleteExamSession/);
  assert.match(page, /handleUnscheduleExamSession/);
});
```

- [ ] **Step 2: Run static test to verify failure**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
```

Expected: fails because unschedule props and handler wiring do not exist.

- [ ] **Step 3: Update `ExamItemTray` drop target**

Add prop:

```ts
onUnscheduleSession?: (sessionId: string) => Promise<boolean> | boolean;
```

Add drag payload parser for scheduled sessions:

```ts
type DragPayload = {
	examScheduleItemId: string;
	classroomId: string;
	gradeLevelId: string;
	durationMinutes: number;
	sourceSessionId?: string;
};

function dragPayload(event: DragEvent): DragPayload | null {
	const payload = event.dataTransfer?.getData('application/x-exam-schedule-item');
	if (!payload) return null;
	try {
		return JSON.parse(payload) as DragPayload;
	} catch {
		return null;
	}
}

async function handleDrop(event: DragEvent) {
	if (readonly) return;
	event.preventDefault();
	const payload = dragPayload(event);
	if (!payload?.sourceSessionId) return;
	await onUnscheduleSession?.(payload.sourceSessionId);
}
```

Add `ondragover`/`ondrop` to the tray container and visual copy: `ลากรายการที่จัดแล้วกลับมาที่นี่เพื่อเอาออกจากตาราง`.

- [ ] **Step 4: Update timeline dialog**

Add prop:

```ts
onUnscheduleSession?: (sessionId: string) => Promise<boolean> | boolean;
```

Add dialog action:

```ts
async function removeSelectedSession() {
	if (!selectedSession) return;
	const removed = await onUnscheduleSession?.(selectedSession.id);
	if (removed) dialogOpen = false;
}
```

Add red button in `Dialog.Footer`:

```svelte
<LoadingButton
	variant="destructive"
	loading={placingItemId === selectedSession?.examScheduleItemId}
	loadingLabel="กำลังเอาออก..."
	onclick={removeSelectedSession}
	disabled={!selectedSession || placementDisabled}
>
	เอาออกจากตาราง
</LoadingButton>
```

Pass `onUnscheduleSession` to `ExamItemTray`.

- [ ] **Step 5: Wire page handler**

In `+page.svelte`, `deleteExamSession` already exists in API. Add:

```ts
async function handleUnscheduleExamSession(sessionId: string): Promise<boolean> {
	const session = workspace?.scheduledSessions.find((item) => item.id === sessionId);
	if (!session) return false;
	placingItemId = session.examScheduleItemId;
	try {
		await deleteExamSession(sessionId);
		toast.success('เอารายการสอบออกจากตารางแล้ว');
		await refreshWorkspace();
		return true;
	} catch (deleteError) {
		toast.error(deleteError instanceof Error ? deleteError.message : 'เอารายการสอบออกจากตารางไม่สำเร็จ');
		return false;
	} finally {
		placingItemId = null;
	}
}
```

Pass to timeline:

```svelte
onUnscheduleSession={handleUnscheduleExamSession}
```

- [ ] **Step 6: Run checks**

Run:

```bash
cd frontend-school
npm run test:static -- academic-exam-schedule
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add frontend-school/src/lib/components/academic/exam-schedule/ExamItemTray.svelte frontend-school/src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte frontend-school/src/routes/\(app\)/staff/academic/exam-schedules/\[id\]/+page.svelte frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: allow unscheduling exam sessions"
```

---

## Task 11: Final Integration, Review, And Verification

**Files:**
- Review all changed exam schedule files.

- [ ] **Step 1: Run full backend verification**

Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
cargo check
cargo test --all-targets -- --skip modules::auth::tests::auth_tests::test_login_success --skip modules::auth::tests::auth_tests::test_login_invalid_credentials
```

Expected:

- `exam_schedule_service` tests pass.
- `cargo check` passes.
- all-target tests pass with only the two auth DB tests filtered.

- [ ] **Step 2: Run full frontend verification**

Run:

```bash
cd frontend-school
node --test tests/static/exam-schedule-time.test.mjs
npm run test:static -- academic-exam-schedule
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected:

- exam schedule time helper tests pass.
- static tests pass.
- `svelte-check` reports 0 errors and 0 warnings.

- [ ] **Step 3: Run diff and status checks**

Run:

```bash
git diff --check
git status --short
```

Expected:

- `git diff --check` exits 0.
- `git status --short` shows no uncommitted changes after all task commits.

- [ ] **Step 4: Request final code review**

Use `superpowers:requesting-code-review`. Ask reviewer to focus on:

- Invigilator conflict validation correctness.
- Workload calculation excluding gaps.
- Room assignment no longer deleting invigilators accidentally.
- Schedule-first layout preserving publish readiness behavior.
- Drag preview and unschedule interactions.
- shadcn-svelte primitive consistency.

- [ ] **Step 5: Address review feedback**

If reviewer finds Critical or Important issues, fix them with failing tests first, re-run relevant verification, and commit fixes. If only Minor issues remain, decide whether to fix immediately based on risk.

- [ ] **Step 6: Final branch handling**

Use `superpowers:finishing-a-development-branch` after review approval and verification. Present merge/push options unless the user has already given an explicit integration instruction.

---

## Self-Review Checklist

- Spec coverage:
  - Schedule-first layout: Tasks 5-6.
  - Split invigilators from rooms: Tasks 2, 7, 8.
  - Workload summary and per-day drilldown: Tasks 1, 3, 8.
  - Conflict blocking: Tasks 1, 3.
  - Non-publish-blocking invigilators: Tasks 3, 6, 8.
  - Duration drag preview: Task 9.
  - Unschedule by dialog and tray drop: Task 10.
  - shadcn-svelte usage and sheet primitive: Tasks 5-8.
  - Semantic status/button colors: Tasks 6-10.
- Red-flag scan: no unresolved implementation markers or open choices remain in this plan.
- Type consistency:
  - Backend request field is `invigilator_staff_ids`; frontend field is `invigilatorStaffIds`.
  - Assignment ids use `assignmentId` in frontend and `assignment_id` in backend.
  - Existing delete session API is reused as `deleteExamSession(sessionId)`.
