# Staff Exam Schedule Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `/staff/exams` with a responsive published-schedule dashboard that separates lower and upper secondary schedules, presents merged day/time tables, shows school-wide invigilator assignments, and highlights the signed-in staff member's own duties.

**Architecture:** Keep student and parent personal-schedule DTOs unchanged and change only `GET /api/staff/exam-schedules` to a staff-specific nested published-round DTO. Build one deterministic frontend view utility for flattening, filtering, sorting, row-span calculation, and current-user summaries; feed its typed results into focused Svelte presentation components and a tabbed dashboard coordinator.

**Tech Stack:** Rust 2024, Axum, sqlx/PostgreSQL, utoipa/OpenAPI 3.1, SvelteKit 2, Svelte 5 runes, TypeScript, Tailwind CSS 4, local shadcn-svelte components, bits-ui, Node test runner.

## Global Constraints

- Read and follow `.rules`, especially typed API boundaries, shared page layout/state UI, generated API contracts, and read-first staff UX.
- Return only rounds where `academic_exam_rounds.status = 'published'`.
- Keep `/api/me/exam-schedules` and `/api/parent/students/{student_id}/exam-schedules` on `PersonalExamScheduleRound`; do not expose invigilator data there.
- Expose only stable ids and display labels required by the staff workflow; do not expose usernames, email addresses, phone numbers, addresses, national ids, or profile details.
- Do not create or edit a database migration.
- Keep `/staff/exams` read-only and available to active staff without adding an academic-management permission.
- Use generated TypeScript wire DTOs from `Schemas`; do not add handwritten transport DTOs.
- Use local shadcn-svelte primitives and shared `PageShell`, `PageSkeleton`, and `PageState`.
- Use Svelte 5 runes and typed `$props`; run the Svelte autofixer on every changed Svelte file.
- Implement every behavior change test-first and commit after each independently passing task.

## File Map

### Backend domain and service

- Modify `backend-school/src/modules/academic/models/exam_schedule.rs`
  - Own the new staff-only published round/day/session/room-assignment/invigilator response types.
- Modify `backend-school/src/modules/academic/services/exam_schedule_service/published_views.rs`
  - Query and group published staff schedule data while preserving existing student/parent logic.
- Modify `backend-school/src/modules/academic/services/exam_schedule_service_tests.rs`
  - Prove the published integration returns grade metadata, rooms, workload bounds, and invigilators.
- Modify `backend-school/src/modules/academic/handlers/exam_schedule.rs`
  - Document the staff endpoint with the new response type.
- Modify `backend-school/src/api_contract.rs`
  - Register and assert the new OpenAPI schemas.

### Generated contract and API client

- Regenerate `contracts/openapi/school-api.json`.
- Regenerate `frontend-school/src/lib/api/generated/school-api.ts`.
- Modify `frontend-school/src/lib/api/examSchedule.ts`
  - Export generated staff DTO aliases and type `listStaffExamSchedules()` with them.

### Frontend view logic

- Create `frontend-school/src/lib/utils/staff-exam-schedule-view.ts`
  - Own flattening, natural sorting, composed filtering, row spans, mobile day groups, round summaries, and current-user duty summaries.
- Create `frontend-school/tests/static/staff-exam-schedule-view.test.mjs`
  - Execute the TypeScript utility with representative multi-day, multi-level data.

### Shared UI primitive

- Create `frontend-school/src/lib/components/ui/collapsible/collapsible.svelte`.
- Create `frontend-school/src/lib/components/ui/collapsible/collapsible-trigger.svelte`.
- Create `frontend-school/src/lib/components/ui/collapsible/collapsible-content.svelte`.
- Create `frontend-school/src/lib/components/ui/collapsible/index.ts`.
  - Wrap bits-ui Collapsible using the local shadcn-svelte conventions.

### Feature components and route

- Create `frontend-school/src/lib/components/academic/exam-schedule/StaffExamScheduleTable.svelte`
  - Render the desktop merged schedule table and mobile day cards.
- Create `frontend-school/src/lib/components/academic/exam-schedule/StaffExamInvigilatorTable.svelte`
  - Render the desktop merged invigilator report and mobile room cards.
- Create `frontend-school/src/lib/components/academic/exam-schedule/MyExamInvigilationView.svelte`
  - Render current-user totals, upcoming/completed assignments, and the no-assignment state.
- Create `frontend-school/src/lib/components/academic/exam-schedule/StaffExamScheduleDashboard.svelte`
  - Own round/filter/tab state, overview cards, and composition of the three focused views.
- Modify `frontend-school/src/routes/(app)/staff/exams/+page.svelte`
  - Load the staff DTO, read the current staff id, and render the dashboard.
- Modify `frontend-school/tests/static/academic-exam-schedule.test.mjs`
  - Guard the route, dashboard, semantic tables, responsive cards, privacy boundary, and unchanged personal pages.

---

### Task 1: Add staff-only published schedule types and pure grouping

**Files:**
- Modify: `backend-school/src/modules/academic/models/exam_schedule.rs:359-384`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service/published_views.rs:1-322`

**Interfaces:**
- Consumes: existing `PersonalExamScheduleRound`, `PersonalExamSessionView`, and `shared::minutes_between_times(NaiveTime, NaiveTime) -> i32`.
- Produces:
  - `StaffPublishedExamScheduleRound`
  - `StaffPublishedExamDay`
  - `StaffPublishedExamSession`
  - `StaffPublishedExamRoomAssignment`
  - `StaffPublishedExamInvigilator`
  - `group_staff_published_exam_rows(assignment_rows, session_rows) -> Vec<StaffPublishedExamScheduleRound>`

- [ ] **Step 1: Write a failing pure service test for nested grouping**

Add `#[cfg(test)] mod tests` at the bottom of `published_views.rs`. Construct one published
round with one exam day, one room assignment, two sessions, and two invigilators. Assert the
assignment receives both invigilators, `session_minutes` is the sum of actual session windows, and
session order is preserved.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

    fn t(value: &str) -> NaiveTime {
        NaiveTime::parse_from_str(value, "%H:%M").expect("test time must be valid")
    }

    #[allow(clippy::too_many_arguments)]
    fn staff_session_row(
        round_id: Uuid,
        day_id: Uuid,
        session_id: Uuid,
        assignment_id: Uuid,
        classroom_id: Uuid,
        room_id: Uuid,
        published_at: DateTime<Utc>,
        starts_at: NaiveTime,
        ends_at: NaiveTime,
    ) -> StaffPublishedExamSessionRow {
        StaffPublishedExamSessionRow {
            round_id,
            round_name: "กลางภาค 1/2569".to_string(),
            academic_semester_id: Uuid::from_u128(6),
            published_at: Some(published_at),
            exam_day_id: day_id,
            day_label: Some("วันแรก".to_string()),
            exam_date: NaiveDate::from_ymd_opt(2026, 8, 3).expect("date must be valid"),
            session_id,
            starts_at,
            ends_at,
            duration_minutes: minutes_between_times(starts_at, ends_at),
            subject_id: Uuid::from_u128(9),
            subject_code: "ค21101".to_string(),
            subject_name: "คณิตศาสตร์".to_string(),
            assessment_category_name: "กลางภาค".to_string(),
            grade_level_id: Uuid::from_u128(10),
            grade_level_name: "มัธยมศึกษาปีที่ 1".to_string(),
            grade_level_type: "secondary".to_string(),
            grade_level_year: 1,
            classroom_id,
            classroom_name: "ม.1/1".to_string(),
            day_room_assignment_id: assignment_id,
            room_id,
            room_name: "313".to_string(),
            building_name: Some("อาคาร 3".to_string()),
        }
    }

    #[test]
    fn staff_rows_group_by_round_day_and_assignment_with_actual_minutes() {
        let round_id = Uuid::from_u128(1);
        let day_id = Uuid::from_u128(2);
        let assignment_id = Uuid::from_u128(3);
        let classroom_id = Uuid::from_u128(4);
        let room_id = Uuid::from_u128(5);
        let published_at = Utc::now();

        let assignment_rows = vec![
            StaffPublishedExamAssignmentRow {
                round_id,
                round_name: "กลางภาค 1/2569".to_string(),
                academic_semester_id: Uuid::from_u128(6),
                published_at: Some(published_at),
                exam_day_id: day_id,
                day_label: Some("วันแรก".to_string()),
                exam_date: NaiveDate::from_ymd_opt(2026, 8, 3).expect("date must be valid"),
                assignment_id,
                classroom_id,
                classroom_name: "ม.1/1".to_string(),
                room_id,
                room_name: "313".to_string(),
                building_name: Some("อาคาร 3".to_string()),
                staff_id: Some(Uuid::from_u128(7)),
                display_name: Some("ครู ก".to_string()),
            },
            StaffPublishedExamAssignmentRow {
                round_id,
                round_name: "กลางภาค 1/2569".to_string(),
                academic_semester_id: Uuid::from_u128(6),
                published_at: Some(published_at),
                exam_day_id: day_id,
                day_label: Some("วันแรก".to_string()),
                exam_date: NaiveDate::from_ymd_opt(2026, 8, 3).expect("date must be valid"),
                assignment_id,
                classroom_id,
                classroom_name: "ม.1/1".to_string(),
                room_id,
                room_name: "313".to_string(),
                building_name: Some("อาคาร 3".to_string()),
                staff_id: Some(Uuid::from_u128(8)),
                display_name: Some("ครู ข".to_string()),
            },
        ];

        let session_rows = vec![
            staff_session_row(
                round_id,
                day_id,
                Uuid::from_u128(11),
                assignment_id,
                classroom_id,
                room_id,
                published_at,
                t("08:30"),
                t("09:30"),
            ),
            staff_session_row(
                round_id,
                day_id,
                Uuid::from_u128(12),
                assignment_id,
                classroom_id,
                room_id,
                published_at,
                t("10:00"),
                t("11:30"),
            ),
        ];

        let rounds = group_staff_published_exam_rows(assignment_rows, session_rows);
        let day = &rounds[0].days[0];
        let assignment = &day.room_assignments[0];

        assert_eq!(day.sessions.len(), 2);
        assert_eq!(
            day.sessions
                .iter()
                .map(|session| session.starts_at)
                .collect::<Vec<_>>(),
            vec![t("08:30"), t("10:00")]
        );
        assert_eq!(assignment.invigilators.len(), 2);
        assert_eq!(assignment.session_minutes, 150);
        assert_eq!(assignment.earliest_starts_at, Some(t("08:30")));
        assert_eq!(assignment.latest_ends_at, Some(t("11:30")));
    }
}
```

- [ ] **Step 2: Run the pure test and verify RED**

Run:

```bash
cd backend-school
cargo test modules::academic::services::exam_schedule_service::published_views::tests --bin backend-school
```

Expected: compilation fails because the staff DTOs, row structs, and grouping function do not exist.

- [ ] **Step 3: Add the exact staff response models**

Add these serde/utoipa models after `PersonalExamSessionView`:

```rust
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffPublishedExamScheduleRound {
    pub round_id: Uuid,
    pub round_name: String,
    pub academic_semester_id: Uuid,
    #[schema(required = true)]
    pub published_at: Option<DateTime<Utc>>,
    pub days: Vec<StaffPublishedExamDay>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffPublishedExamDay {
    pub exam_day_id: Uuid,
    #[schema(required = true)]
    pub label: Option<String>,
    pub exam_date: NaiveDate,
    pub sessions: Vec<StaffPublishedExamSession>,
    pub room_assignments: Vec<StaffPublishedExamRoomAssignment>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffPublishedExamSession {
    pub session_id: Uuid,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
    pub duration_minutes: i32,
    pub subject_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    pub assessment_category_name: String,
    pub grade_level_id: Uuid,
    pub grade_level_name: String,
    pub grade_level_type: String,
    pub grade_level_year: i32,
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub day_room_assignment_id: Uuid,
    pub room_id: Uuid,
    pub room_name: String,
    #[schema(required = true)]
    pub building_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffPublishedExamRoomAssignment {
    pub assignment_id: Uuid,
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub room_id: Uuid,
    pub room_name: String,
    #[schema(required = true)]
    pub building_name: Option<String>,
    pub session_minutes: i32,
    #[schema(required = true)]
    pub earliest_starts_at: Option<NaiveTime>,
    #[schema(required = true)]
    pub latest_ends_at: Option<NaiveTime>,
    pub invigilators: Vec<StaffPublishedExamInvigilator>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffPublishedExamInvigilator {
    pub staff_id: Uuid,
    pub display_name: String,
}
```

- [ ] **Step 4: Implement private row types and deterministic grouping**

In `published_views.rs`, add:

```rust
#[derive(Debug, sqlx::FromRow)]
struct StaffPublishedExamAssignmentRow {
    round_id: Uuid,
    round_name: String,
    academic_semester_id: Uuid,
    published_at: Option<DateTime<Utc>>,
    exam_day_id: Uuid,
    day_label: Option<String>,
    exam_date: NaiveDate,
    assignment_id: Uuid,
    classroom_id: Uuid,
    classroom_name: String,
    room_id: Uuid,
    room_name: String,
    building_name: Option<String>,
    staff_id: Option<Uuid>,
    display_name: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct StaffPublishedExamSessionRow {
    round_id: Uuid,
    round_name: String,
    academic_semester_id: Uuid,
    published_at: Option<DateTime<Utc>>,
    exam_day_id: Uuid,
    day_label: Option<String>,
    exam_date: NaiveDate,
    session_id: Uuid,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    duration_minutes: i32,
    subject_id: Uuid,
    subject_code: String,
    subject_name: String,
    assessment_category_name: String,
    grade_level_id: Uuid,
    grade_level_name: String,
    grade_level_type: String,
    grade_level_year: i32,
    classroom_id: Uuid,
    classroom_name: String,
    day_room_assignment_id: Uuid,
    room_id: Uuid,
    room_name: String,
    building_name: Option<String>,
}
```

Implement `group_staff_published_exam_rows` by processing assignment rows first. Preserve first-seen round/day/assignment order with index maps. Append an invigilator only when both optional fields are present. Then process session rows, append each session to its day, and update the matching assignment:

```rust
assignment.session_minutes += minutes_between_times(row.starts_at, row.ends_at);
assignment.earliest_starts_at = Some(
    assignment.earliest_starts_at.map_or(row.starts_at, |value| value.min(row.starts_at)),
);
assignment.latest_ends_at = Some(
    assignment.latest_ends_at.map_or(row.ends_at, |value| value.max(row.ends_at)),
);
```

Use `HashMap<Uuid, usize>` for round indexes, `HashMap<(Uuid, Uuid), usize>` for day indexes, and `HashMap<Uuid, (usize, usize, usize)>` for assignment locations. Map every session field explicitly into `StaffPublishedExamSession`.

- [ ] **Step 5: Run the pure test and verify GREEN**

Run:

```bash
cd backend-school
cargo fmt --check
cargo test modules::academic::services::exam_schedule_service::published_views::tests --bin backend-school
```

Expected: the grouping test passes and formatting is clean.

- [ ] **Step 6: Commit**

```bash
git add backend-school/src/modules/academic/models/exam_schedule.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/published_views.rs
git commit -m "feat(exams): model staff published schedules"
```

### Task 2: Query published sessions, rooms, and invigilators for staff

**Files:**
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service/published_views.rs:54-60,235-292`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service_tests.rs:829-940`

**Interfaces:**
- Consumes: Task 1 staff DTOs and `group_staff_published_exam_rows`.
- Produces: `list_staff_published_exam_schedule(...) -> Result<Vec<StaffPublishedExamScheduleRound>, AppError>` populated from published database rows.

- [ ] **Step 1: Change the database-backed test to require staff nested data**

Update `publish_exposes_the_same_session_to_student_staff_and_linked_parent` so the student and parent assertions stay unchanged while staff assertions use `days`.

```rust
let staff_round = &staff_rounds[0];
assert_eq!(staff_round.round_id, round_id);
assert_eq!(staff_round.days.len(), 1);

let staff_day = &staff_round.days[0];
assert_eq!(staff_day.sessions.len(), 3);
assert_eq!(staff_day.room_assignments.len(), 2);

let first_assignment = staff_day
    .room_assignments
    .iter()
    .find(|assignment| assignment.assignment_id == first_assignment_id)
    .expect("first room assignment should be published");
assert_eq!(first_assignment.invigilators.len(), 1);
assert_eq!(first_assignment.invigilators[0].staff_id, fixture.staff_user_id);
assert_eq!(first_assignment.session_minutes, 120);
assert_eq!(
    first_assignment.earliest_starts_at,
    NaiveTime::from_hms_opt(8, 0, 0)
);
assert_eq!(
    first_assignment.latest_ends_at,
    NaiveTime::from_hms_opt(10, 0, 0)
);
assert!(staff_day.sessions.iter().all(|session| {
    session.grade_level_type == "secondary"
        && session.grade_level_year >= 1
        && !session.grade_level_name.is_empty()
}));

let second_assignment = staff_day
    .room_assignments
    .iter()
    .find(|assignment| assignment.assignment_id == second_assignment_id)
    .expect("second room assignment should be published");
assert!(second_assignment.invigilators.is_empty());
```

- [ ] **Step 2: Run the integration test and verify RED**

Run with the configured isolated test database:

```bash
cd backend-school
cargo test publish_exposes_the_same_session_to_student_staff_and_linked_parent --bin backend-school
```

Expected: compilation fails because the staff service still returns `PersonalExamScheduleRound`.

- [ ] **Step 3: Replace the staff query with two typed published queries**

Change the staff function return type:

```rust
pub async fn list_staff_published_exam_schedule(
    pool: &PgPool,
    user_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<StaffPublishedExamScheduleRound>, AppError>
```

Change the private staff loader to the same response type and replace its existing personal-session
query with these two typed queries:

```rust
async fn list_published_exam_schedule_for_staff(
    pool: &PgPool,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<StaffPublishedExamScheduleRound>, AppError> {
    let assignment_rows = sqlx::query_as::<_, StaffPublishedExamAssignmentRow>(
        r#"
        SELECT round.id AS round_id,
               round.name AS round_name,
               round.academic_semester_id,
               round.published_at,
               day.id AS exam_day_id,
               day.label AS day_label,
               day.exam_date,
               assignment.id AS assignment_id,
               assignment.classroom_id,
               classroom.name AS classroom_name,
               assignment.room_id,
               room.name_th AS room_name,
               building.name_th AS building_name,
               invigilator.staff_id,
               CASE
                   WHEN invigilator.staff_id IS NULL THEN NULL
                   ELSE concat_ws(
                       ' ',
                       NULLIF(
                           concat_ws(
                               '',
                               NULLIF(TRIM(user_account.title), ''),
                               NULLIF(TRIM(user_account.first_name), '')
                           ),
                           ''
                       ),
                       NULLIF(TRIM(user_account.last_name), '')
                   )
               END AS display_name
        FROM academic_exam_day_room_assignments assignment
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN academic_exam_rounds round ON round.id = day.exam_round_id
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        LEFT JOIN academic_exam_day_invigilators invigilator
          ON invigilator.day_room_assignment_id = assignment.id
         AND invigilator.exam_day_id = day.id
        LEFT JOIN users user_account ON user_account.id = invigilator.staff_id
        WHERE round.status = 'published'
          AND ($1::uuid IS NULL OR round.academic_semester_id = $1)
        ORDER BY round.published_at DESC NULLS LAST,
                 round.name,
                 round.id,
                 day.exam_date,
                 classroom.name,
                 room.name_th,
                 user_account.first_name NULLS LAST,
                 user_account.last_name NULLS LAST,
                 invigilator.staff_id NULLS LAST
        "#,
    )
    .bind(academic_semester_id)
    .fetch_all(pool)
    .await?;

    let session_rows = sqlx::query_as::<_, StaffPublishedExamSessionRow>(
        r#"
        SELECT round.id AS round_id,
               round.name AS round_name,
               round.academic_semester_id,
               round.published_at,
               day.id AS exam_day_id,
               day.label AS day_label,
               day.exam_date,
               session.id AS session_id,
               session.starts_at,
               session.ends_at,
               item.duration_minutes,
               subject.id AS subject_id,
               subject.code AS subject_code,
               COALESCE(
                   NULLIF(subject.name_th, ''),
                   NULLIF(subject.name_en, ''),
                   subject.code
               ) AS subject_name,
               category.name AS assessment_category_name,
               grade_level.id AS grade_level_id,
               CASE grade_level.level_type
                   WHEN 'kindergarten' THEN CONCAT('อ.', grade_level.year)
                   WHEN 'primary' THEN CONCAT('ป.', grade_level.year)
                   WHEN 'secondary' THEN CONCAT('ม.', grade_level.year)
                   ELSE CONCAT('?.', grade_level.year)
               END AS grade_level_name,
               grade_level.level_type AS grade_level_type,
               grade_level.year AS grade_level_year,
               classroom.id AS classroom_id,
               classroom.name AS classroom_name,
               assignment.id AS day_room_assignment_id,
               room.id AS room_id,
               room.name_th AS room_name,
               building.name_th AS building_name
        FROM academic_exam_sessions session
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.exam_round_id = session.exam_round_id
        JOIN academic_exam_rounds round
          ON round.id = item.exam_round_id
         AND round.academic_semester_id = item.academic_semester_id
        JOIN academic_exam_days day
          ON day.id = session.exam_day_id
         AND day.exam_round_id = session.exam_round_id
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN grade_levels grade_level ON grade_level.id = item.grade_level_id
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE round.status = 'published'
          AND ($1::uuid IS NULL OR round.academic_semester_id = $1)
        ORDER BY round.published_at DESC NULLS LAST,
                 round.name,
                 round.id,
                 day.exam_date,
                 session.starts_at,
                 session.ends_at,
                 CASE grade_level.level_type
                     WHEN 'kindergarten' THEN 1
                     WHEN 'primary' THEN 2
                     WHEN 'secondary' THEN 3
                     ELSE 4
                 END,
                 grade_level.year,
                 classroom.name,
                 subject.code,
                 category.display_order,
                 category.name,
                 session.id
        "#,
    )
    .bind(academic_semester_id)
    .fetch_all(pool)
    .await?;

    Ok(group_staff_published_exam_rows(assignment_rows, session_rows))
}
```

- [ ] **Step 4: Run focused backend tests and verify GREEN**

Run:

```bash
cd backend-school
cargo fmt --check
cargo test modules::academic::services::exam_schedule_service::published_views::tests --bin backend-school
cargo test publish_exposes_the_same_session_to_student_staff_and_linked_parent --bin backend-school
cargo check
```

Expected: all commands pass.

- [ ] **Step 5: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service/published_views.rs \
  backend-school/src/modules/academic/services/exam_schedule_service_tests.rs
git commit -m "feat(exams): load staff published invigilation"
```

### Task 3: Publish and consume the generated staff API contract

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/exam_schedule.rs:417-443`
- Modify: `backend-school/src/api_contract.rs:29-31,551-553,2339-2436`
- Regenerate: `contracts/openapi/school-api.json`
- Regenerate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/examSchedule.ts:213-214,485-491`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs:240-280,1424-1526`

**Interfaces:**
- Consumes: Task 1 Rust staff DTOs and Task 2 service return type.
- Produces generated aliases:
  - `StaffPublishedExamScheduleRound`
  - `StaffPublishedExamDay`
  - `StaffPublishedExamSession`
  - `StaffPublishedExamRoomAssignment`
  - `StaffPublishedExamInvigilator`
  - `listStaffExamSchedules(filters?) -> Promise<StaffPublishedExamScheduleRound[]>`

- [ ] **Step 1: Write failing API contract assertions**

In `documents_self_service_timetable_exam_and_calendar_reads`, require the staff endpoint to use its
own envelope:

```rust
assert_eq!(
    document["paths"]["/api/staff/exam-schedules"]["get"]["responses"]["200"]["content"]
        ["application/json"]["schema"]["$ref"],
    "#/components/schemas/ApiResponse_Vec_StaffPublishedExamScheduleRound"
);

let staff_round = &schemas["StaffPublishedExamScheduleRound"];
assert!(required(staff_round).contains(&"publishedAt"));
assert!(contains_null(&staff_round["properties"]["publishedAt"]));
assert!(required(staff_round).contains(&"days"));

let staff_day = &schemas["StaffPublishedExamDay"];
for field in ["sessions", "roomAssignments"] {
    assert!(required(staff_day).contains(&field));
}

let staff_assignment = &schemas["StaffPublishedExamRoomAssignment"];
for field in ["buildingName", "earliestStartsAt", "latestEndsAt"] {
    assert!(required(staff_assignment).contains(&field));
    assert!(contains_null(&staff_assignment["properties"][field]));
}

let staff_invigilator = &schemas["StaffPublishedExamInvigilator"];
for forbidden in ["username", "email", "phone", "nationalId", "national_id"] {
    assert!(staff_invigilator["properties"].get(forbidden).is_none());
}
```

In `academic-exam-schedule.test.mjs`, add the staff transport boundary assertions before changing
the frontend API module:

```js
const examScheduleApi = readFileSync(projectPath('src/lib/api/examSchedule.ts'), 'utf8');
assert.match(examScheduleApi, /export type StaffPublishedExamScheduleRound = Schemas\['StaffPublishedExamScheduleRound'\]/);
assert.match(examScheduleApi, /listStaffExamSchedules[\s\S]*Promise<StaffPublishedExamScheduleRound\[\]>/);
assert.match(examScheduleApi, /listMyExamSchedules[\s\S]*Promise<PersonalExamScheduleRound\[\]>/);
assert.match(examScheduleApi, /listChildExamSchedules[\s\S]*Promise<PersonalExamScheduleRound\[\]>/);
```

- [ ] **Step 2: Run the contract test and verify RED**

Run:

```bash
cd backend-school
cargo test api_contract::tests::documents_self_service_timetable_exam_and_calendar_reads --bin backend-school
cd ../frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: the backend response schema still points to
`ApiResponse_Vec_PersonalExamScheduleRound`, and the frontend static test cannot find the staff
alias or return type.

- [ ] **Step 3: Register the new response in the handler and OpenAPI document**

Change the staff handler response annotation to:

```rust
(status = 200, description = "Published school exam schedules for staff", body = ApiResponse<Vec<crate::modules::academic::models::exam_schedule::StaffPublishedExamScheduleRound>>)
```

Import and register all five staff schemas plus
`ApiResponse<Vec<StaffPublishedExamScheduleRound>>` in `api_contract.rs`. Keep the personal schemas
registered for student and parent endpoints.

- [ ] **Step 4: Run the backend contract test and verify GREEN**

Run:

```bash
cd backend-school
cargo fmt --check
cargo test api_contract::tests::documents_self_service_timetable_exam_and_calendar_reads --bin backend-school
```

Expected: PASS.

- [ ] **Step 5: Regenerate tracked API artifacts**

Run:

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Expected: `school-api.json` and generated `school-api.ts` contain all five staff DTOs and the staff
operation references `ApiResponse_Vec_StaffPublishedExamScheduleRound`.

- [ ] **Step 6: Type the frontend API with generated schemas**

Add aliases next to the personal aliases:

```ts
export type StaffPublishedExamScheduleRound = Schemas['StaffPublishedExamScheduleRound'];
export type StaffPublishedExamDay = Schemas['StaffPublishedExamDay'];
export type StaffPublishedExamSession = Schemas['StaffPublishedExamSession'];
export type StaffPublishedExamRoomAssignment = Schemas['StaffPublishedExamRoomAssignment'];
export type StaffPublishedExamInvigilator = Schemas['StaffPublishedExamInvigilator'];
```

Change only the staff request:

```ts
export async function listStaffExamSchedules(
	filters: ExamScheduleFilters = {}
): Promise<StaffPublishedExamScheduleRound[]> {
	const response = await apiClient.get<StaffPublishedExamScheduleRound[]>(
		`/api/staff/exam-schedules${examScheduleQuery(filters)}`
	);
	return apiData(response, 'ไม่สามารถโหลดตารางสอบสำหรับครูได้');
}
```

- [ ] **Step 7: Run focused contract/static tests**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
npm run check:api-contracts
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add backend-school/src/modules/academic/handlers/exam_schedule.rs \
  backend-school/src/api_contract.rs \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/examSchedule.ts \
  frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat(exams): publish staff schedule contract"
```

### Task 4: Build and test the staff schedule view model

**Files:**
- Create: `frontend-school/src/lib/utils/staff-exam-schedule-view.ts`
- Create: `frontend-school/tests/static/staff-exam-schedule-view.test.mjs`

**Interfaces:**
- Consumes: generated staff DTO aliases from Task 3.
- Produces:

```ts
export type StaffExamScheduleLevelFilter = 'all' | 'lower_secondary' | 'upper_secondary';

export interface StaffExamScheduleFilters {
	dayId: string;
	level: StaffExamScheduleLevelFilter;
	classroomId: string;
	query: string;
}

export interface StaffExamScheduleSessionRecord extends StaffPublishedExamSession {
	examDayId: string;
	examDate: string;
	dayLabel: string | null;
	invigilators: StaffPublishedExamInvigilator[];
}

export interface StaffExamRoomAssignmentRecord extends StaffPublishedExamRoomAssignment {
	examDayId: string;
	examDate: string;
	dayLabel: string | null;
	sessions: StaffPublishedExamSession[];
}

export interface StaffExamScheduleRenderRow {
	session: StaffExamScheduleSessionRecord;
	showDayCell: boolean;
	dayRowSpan: number;
	showTimeCell: boolean;
	timeRowSpan: number;
	dayGroupIndex: number;
}

export interface StaffExamInvigilatorRenderRow {
	assignment: StaffExamRoomAssignmentRecord;
	showDayCell: boolean;
	dayRowSpan: number;
	dayGroupIndex: number;
	isCurrentUser: boolean;
}

export interface MyExamInvigilationSummary {
	items: Array<StaffExamRoomAssignmentRecord & { status: 'upcoming' | 'completed' }>;
	assignedDayCount: number;
	assignmentCount: number;
	totalMinutes: number;
}
```

- [ ] **Step 1: Write failing behavior tests**

Create a two-day round fixture with:

- day 1: `ม.1/1` at `08:30-09:30` and `ม.1/2` at the same time;
- day 1: `ม.4/1` at `10:00-11:30`;
- day 2: one `ม.1/1` session at `09:00-10:00`;
- staff A assigned to day 1 `ม.1/1` and day 2 `ม.1/1`;
- staff B assigned to day 1 `ม.4/1`.

Write separate tests:

```js
it('filters lower and upper secondary by grade year', () => {
	const lower = filterStaffExamScheduleRound(round, {
		dayId: 'all',
		level: 'lower_secondary',
		classroomId: 'all',
		query: ''
	});
	assert.deepEqual(lower.sessions.map((session) => session.gradeLevelYear), [1, 1, 1]);

	const upper = filterStaffExamScheduleRound(round, {
		dayId: 'all',
		level: 'upper_secondary',
		classroomId: 'all',
		query: ''
	});
	assert.deepEqual(upper.sessions.map((session) => session.gradeLevelYear), [4]);
});

it('composes day classroom and search filters', () => {
	const filtered = filterStaffExamScheduleRound(round, {
		dayId: 'day-1',
		level: 'lower_secondary',
		classroomId: 'class-m1-1',
		query: 'ครู ก'
	});
	assert.deepEqual(filtered.sessions.map((session) => session.sessionId), ['session-m1-1']);
	assert.deepEqual(filtered.assignments.map((assignment) => assignment.assignmentId), ['assignment-m1-1']);
});

it('never merges day or time spans across group boundaries', () => {
	const rows = buildStaffExamScheduleRenderRows(flattenStaffExamScheduleRound(round).sessions);
	assert.deepEqual(rows.map((row) => row.dayRowSpan), [3, 0, 0, 1]);
	assert.deepEqual(rows.map((row) => row.timeRowSpan), [2, 0, 1, 1]);
	assert.deepEqual(rows.map((row) => row.dayGroupIndex), [0, 0, 0, 1]);
});

it('recomputes invigilator day spans after filtering', () => {
	const filtered = filterStaffExamScheduleRound(round, {
		dayId: 'day-1',
		level: 'all',
		classroomId: 'all',
		query: ''
	});
	const rows = buildStaffExamInvigilatorRenderRows(filtered.assignments, 'staff-a');
	assert.deepEqual(rows.map((row) => row.dayRowSpan), [3, 0, 0]);
	assert.deepEqual(rows.map((row) => row.isCurrentUser), [true, false, false]);
});

it('summarizes only the current staff assignments', () => {
	const summary = buildMyExamInvigilationSummary(
		flattenStaffExamScheduleRound(round).assignments,
		'staff-a',
		new Date('2026-08-03T07:00:00')
	);
	assert.equal(summary.assignedDayCount, 2);
	assert.equal(summary.assignmentCount, 2);
	assert.equal(summary.totalMinutes, 120);
	assert.equal(summary.items.every((item) => item.invigilators.some((staff) => staff.staffId === 'staff-a')), true);
});

it('keeps invalid display dates and times readable', () => {
	assert.equal(formatStaffExamDate('not-a-date'), 'not-a-date');
	assert.equal(formatStaffExamTime('not-a-time'), 'not-a-time');
	assert.equal(formatStaffExamTime('08:30:00'), '08:30');
});
```

- [ ] **Step 2: Run the helper tests and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/staff-exam-schedule-view.test.mjs
```

Expected: module-not-found failure for `staff-exam-schedule-view.ts`.

- [ ] **Step 3: Implement flattening, sorting, and composed filtering**

Export these functions:

```ts
export function flattenStaffExamScheduleRound(round: StaffPublishedExamScheduleRound): {
	sessions: StaffExamScheduleSessionRecord[];
	assignments: StaffExamRoomAssignmentRecord[];
};

export function filterStaffExamScheduleRound(
	round: StaffPublishedExamScheduleRound,
	filters: StaffExamScheduleFilters
): {
	sessions: StaffExamScheduleSessionRecord[];
	assignments: StaffExamRoomAssignmentRecord[];
};
```

Build an assignment map by `assignmentId`. Enrich sessions with their assignment's invigilators.
Use `Intl.Collator('th', { numeric: true, sensitivity: 'base' })` for classroom, subject, and room
ordering. Apply filters as an intersection:

```ts
const levelMatches =
	filters.level === 'all' ||
	(filters.level === 'lower_secondary' &&
		session.gradeLevelType === 'secondary' &&
		session.gradeLevelYear >= 1 &&
		session.gradeLevelYear <= 3) ||
	(filters.level === 'upper_secondary' &&
		session.gradeLevelType === 'secondary' &&
		session.gradeLevelYear >= 4 &&
		session.gradeLevelYear <= 6);
```

Normalize search with `trim().toLocaleLowerCase('th-TH')`. Match subject name/code, assessment
category, classroom, building, room, and assignment invigilator display names. An assignment's
searchable text includes the subject/category fields from its linked sessions, so subject search
keeps the matching room assignment visible. Keep an assignment only when day, classroom, level,
and search all match.

Sort schedule records by exam date, start time, grade-level year, natural classroom order, natural
subject order, assessment category, and session id. Sort assignments by exam date, natural
classroom order, natural room order, and assignment id.

- [ ] **Step 4: Implement row spans, day groups, summaries, and current-user ordering**

Export:

```ts
export function buildStaffExamScheduleRenderRows(
	sessions: StaffExamScheduleSessionRecord[]
): StaffExamScheduleRenderRow[];

export function buildStaffExamInvigilatorRenderRows(
	assignments: StaffExamRoomAssignmentRecord[],
	currentStaffId: string
): StaffExamInvigilatorRenderRow[];

export function groupStaffScheduleRowsByDay(
	rows: StaffExamScheduleRenderRow[]
): Array<{ examDate: string; dayLabel: string | null; rows: StaffExamScheduleRenderRow[] }>;

export function groupStaffInvigilatorRowsByDay(
	rows: StaffExamInvigilatorRenderRow[]
): Array<{ examDate: string; dayLabel: string | null; rows: StaffExamInvigilatorRenderRow[] }>;

export function buildMyExamInvigilationSummary(
	assignments: StaffExamRoomAssignmentRecord[],
	currentStaffId: string,
	now: Date
): MyExamInvigilationSummary;
```

Calculate a span only across consecutive equal keys. Day key is `examDate`; time key is
`examDate|startsAt|endsAt`. Set the first row's span to the group length and continuation rows to
zero. Assign a zero-based `dayGroupIndex` to every schedule and invigilator row so the table can
alternate day-group backgrounds without inferring adjacency in markup. For personal ordering,
compare `new Date(`${examDate}T${latestEndsAt ?? '23:59:59'}`)` with `now`; sort upcoming ascending
and completed descending.

Also export a round summary with stable selected-round counts:

```ts
export interface StaffExamRoundSummary {
	examDayCount: number;
	examRoomCount: number;
	invigilatorCount: number;
	nextPersonalAssignment: StaffExamRoomAssignmentRecord | null;
}

export function buildStaffExamRoundSummary(
	round: StaffPublishedExamScheduleRound,
	currentStaffId: string,
	now: Date
): StaffExamRoundSummary;

export function formatStaffExamDate(value: string): string;
export function formatStaffExamTime(value: string): string;
```

`formatStaffExamDate` uses a local-midnight `Date` and Thai `Intl.DateTimeFormat`; it returns the
source string (or `-` for empty input) when parsing fails. `formatStaffExamTime` returns `HH:mm`
from a valid API time and returns the source string (or `-`) when parsing fails.

- [ ] **Step 5: Run helper tests and verify GREEN**

Run:

```bash
cd frontend-school
node --test tests/static/staff-exam-schedule-view.test.mjs
```

Expected: all helper tests pass.

- [ ] **Step 6: Commit**

```bash
git add frontend-school/src/lib/utils/staff-exam-schedule-view.ts \
  frontend-school/tests/static/staff-exam-schedule-view.test.mjs
git commit -m "feat(exams): derive staff schedule views"
```

### Task 5: Add the shared Collapsible primitive

**Files:**
- Create: `frontend-school/src/lib/components/ui/collapsible/collapsible.svelte`
- Create: `frontend-school/src/lib/components/ui/collapsible/collapsible-trigger.svelte`
- Create: `frontend-school/src/lib/components/ui/collapsible/collapsible-content.svelte`
- Create: `frontend-school/src/lib/components/ui/collapsible/index.ts`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

**Interfaces:**
- Consumes: `bits-ui` `Collapsible` and `$lib/utils.cn`.
- Produces `$lib/components/ui/collapsible` exports `Root`, `Trigger`, `Content` and aliases
  `Collapsible`, `CollapsibleTrigger`, `CollapsibleContent`.

- [ ] **Step 1: Write a failing shared-primitive contract test**

```js
test('staff exam mobile cards use the shared collapsible primitive', () => {
	for (const relativePath of [
		'src/lib/components/ui/collapsible/collapsible.svelte',
		'src/lib/components/ui/collapsible/collapsible-trigger.svelte',
		'src/lib/components/ui/collapsible/collapsible-content.svelte',
		'src/lib/components/ui/collapsible/index.ts'
	]) {
		assert.equal(existsSync(projectPath(relativePath)), true, `${relativePath} should exist`);
	}

	const root = readFileSync(projectPath('src/lib/components/ui/collapsible/collapsible.svelte'), 'utf8');
	const index = readFileSync(projectPath('src/lib/components/ui/collapsible/index.ts'), 'utf8');
	assert.match(root, /Collapsible as CollapsiblePrimitive/);
	assert.match(root, /bind:open/);
	assert.match(index, /Root as Collapsible/);
});
```

- [ ] **Step 2: Run the static test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: assertion failure because the four shared files do not exist.

- [ ] **Step 3: Implement the shadcn-style wrappers**

`collapsible.svelte`:

```svelte
<script lang="ts">
	import { Collapsible as CollapsiblePrimitive } from 'bits-ui';

	let { open = $bindable(false), ...restProps }: CollapsiblePrimitive.RootProps = $props();
</script>

<CollapsiblePrimitive.Root bind:open data-slot="collapsible" {...restProps} />
```

`collapsible-trigger.svelte`:

```svelte
<script lang="ts">
	import { Collapsible as CollapsiblePrimitive } from 'bits-ui';

	let { ref = $bindable(null), ...restProps }: CollapsiblePrimitive.TriggerProps = $props();
</script>

<CollapsiblePrimitive.Trigger bind:ref data-slot="collapsible-trigger" {...restProps} />
```

`collapsible-content.svelte`:

```svelte
<script lang="ts">
	import { Collapsible as CollapsiblePrimitive } from 'bits-ui';
	import { cn } from '$lib/utils.js';

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: CollapsiblePrimitive.ContentProps = $props();
</script>

<CollapsiblePrimitive.Content
	bind:ref
	data-slot="collapsible-content"
	class={cn('overflow-hidden', className)}
	{...restProps}
/>
```

`index.ts`:

```ts
import Root from './collapsible.svelte';
import Trigger from './collapsible-trigger.svelte';
import Content from './collapsible-content.svelte';

export {
	Root,
	Trigger,
	Content,
	Root as Collapsible,
	Trigger as CollapsibleTrigger,
	Content as CollapsibleContent
};
```

- [ ] **Step 4: Run the Svelte autofixer and static test**

Call `mcp__svelte.svelte_autofixer` for the three Svelte files with
`desired_svelte_version: 5`. Apply every reported issue and rerun the autofixer until each returns
no issues or suggestions.

Then run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add frontend-school/src/lib/components/ui/collapsible \
  frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat(ui): add collapsible primitive"
```

### Task 6: Render merged schedule and invigilator reports

**Files:**
- Create: `frontend-school/src/lib/components/academic/exam-schedule/StaffExamScheduleTable.svelte`
- Create: `frontend-school/src/lib/components/academic/exam-schedule/StaffExamInvigilatorTable.svelte`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

**Interfaces:**
- Consumes:
  - `StaffExamScheduleRenderRow[]`
  - `StaffExamInvigilatorRenderRow[]`
  - `groupStaffScheduleRowsByDay`
  - `groupStaffInvigilatorRowsByDay`
  - `formatStaffExamDate`
  - `formatStaffExamTime`
  - `$lib/utils.cn`
  - shared Collapsible from Task 5.
- Produces:

```ts
// StaffExamScheduleTable.svelte props
{ rows: StaffExamScheduleRenderRow[] }

// StaffExamInvigilatorTable.svelte props
{ rows: StaffExamInvigilatorRenderRow[]; currentStaffId: string }
```

- [ ] **Step 1: Write failing semantic/responsive component assertions**

Add assertions that the two files exist and require:

```js
for (const source of [scheduleTable, invigilatorTable]) {
	assert.match(source, /hidden md:block/);
	assert.match(source, /md:hidden/);
	assert.match(source, /Collapsible\.Root/);
	assert.match(source, /scope="col"/);
}
assert.match(scheduleTable, /rowspan=\{row\.dayRowSpan\}/);
assert.match(scheduleTable, /rowspan=\{row\.timeRowSpan\}/);
assert.match(invigilatorTable, /rowspan=\{row\.dayRowSpan\}/);
assert.match(invigilatorTable, /ฉัน/);
```

- [ ] **Step 2: Run the static test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: failure because the report components do not exist.

- [ ] **Step 3: Implement the desktop schedule table**

Use local Table components with `class="min-w-[1040px]"`. Render:

```svelte
{#each rows as row (row.session.sessionId)}
	<TableRow
		class={cn(
			row.dayGroupIndex % 2 === 1 && 'bg-muted/15',
			row.showDayCell && 'border-t-2'
		)}
	>
		{#if row.showDayCell}
			<TableCell rowspan={row.dayRowSpan} class="bg-muted/30 text-center align-top font-medium">
				{formatStaffExamDate(row.session.examDate)}
			</TableCell>
		{/if}
		{#if row.showTimeCell}
			<TableCell rowspan={row.timeRowSpan} class="text-center font-mono align-top">
				{formatStaffExamTime(row.session.startsAt)}–{formatStaffExamTime(row.session.endsAt)}
			</TableCell>
		{/if}
		<TableCell class="whitespace-normal">
			<div class="font-medium">{row.session.subjectName}</div>
			<div class="text-xs text-muted-foreground">{row.session.subjectCode}</div>
		</TableCell>
		<TableCell class="whitespace-normal">
			<Badge variant="secondary">{row.session.assessmentCategoryName}</Badge>
		</TableCell>
		<TableCell>
			<Badge variant="outline">{row.session.gradeLevelName}</Badge>
			<div>{row.session.classroomName}</div>
		</TableCell>
		<TableCell class="text-center whitespace-normal">{roomLabel(row.session)}</TableCell>
		<TableCell class="whitespace-normal">{invigilatorLabel(row.session.invigilators)}</TableCell>
	</TableRow>
{/each}
```

The desktop wrapper is `hidden md:block`; set sticky header classes on every `TableHead`. Use
`scope="col"`. Wrap only the table in `overflow-x-auto`, keep the page container at `min-w-0`, and
do not introduce page-level horizontal overflow.

- [ ] **Step 4: Implement schedule mobile day cards**

Group rows by day. Use `md:hidden`, one `Collapsible.Root open={index === 0}` per day, a full-width
trigger with the formatted date and session count, and a chevron icon marked `aria-hidden="true"`.
Inside `Collapsible.Content`, render each session as labeled time, subject, classroom, room, and
invigilator values. Do not render a wide table in the mobile branch.

```svelte
<div class="space-y-3 md:hidden">
	{#each groupStaffScheduleRowsByDay(rows) as group, index (group.examDate)}
		<Collapsible.Root open={index === 0} class="rounded-xl border bg-card">
			<Collapsible.Trigger
				class="flex w-full items-center justify-between gap-3 p-4 text-left font-medium"
			>
				<span>{formatStaffExamDate(group.examDate)}</span>
				<span class="flex items-center gap-2 text-sm text-muted-foreground">
					{group.rows.length} รายการ
					<ChevronDown class="size-4" aria-hidden="true" />
				</span>
			</Collapsible.Trigger>
			<Collapsible.Content class="divide-y border-t">
				{#each group.rows as row (row.session.sessionId)}
					<div class="grid gap-2 p-4 text-sm">
						<div>
							<span class="text-muted-foreground">เวลา:</span>
							{formatStaffExamTime(row.session.startsAt)}–{formatStaffExamTime(
								row.session.endsAt
							)}
						</div>
						<div>
							<span class="text-muted-foreground">วิชา:</span>
							{row.session.subjectName} ({row.session.subjectCode})
						</div>
						<div>
							<span class="text-muted-foreground">ชั้นเรียน:</span>
							{row.session.classroomName}
						</div>
						<div>
							<span class="text-muted-foreground">ห้องสอบ:</span>
							{roomLabel(row.session)}
						</div>
						<div>
							<span class="text-muted-foreground">กรรมการ:</span>
							{invigilatorLabel(row.session.invigilators)}
						</div>
					</div>
				{/each}
			</Collapsible.Content>
		</Collapsible.Root>
	{/each}
</div>
```

- [ ] **Step 5: Implement the invigilator desktop and mobile views**

Desktop columns are day, classroom, room, actual time ranges, and invigilators. Build actual ranges
from `assignment.sessions`, sorted by start time and formatted as a comma-separated list. Render
each invigilator with a keyed each block:

```svelte
{#each row.assignment.invigilators as invigilator (invigilator.staffId)}
	<Badge variant={invigilator.staffId === currentStaffId ? 'default' : 'outline'}>
		{invigilator.displayName}
		{#if invigilator.staffId === currentStaffId}
			<span class="sr-only">ผู้ใช้ปัจจุบัน</span>
			<span aria-hidden="true"> · ฉัน</span>
		{/if}
	</Badge>
{/each}
```

Use `row.isCurrentUser` for room-row emphasis and `currentStaffId` for the per-name badges:

```ts
interface Props {
	rows: StaffExamInvigilatorRenderRow[];
	currentStaffId: string;
}
```

Apply the same `dayGroupIndex` alternating day background used by the schedule table and primary
tint when `row.isCurrentUser` is true. Center day and room cells while keeping invigilator names
left-aligned. Mobile uses one collapsible day card and one room block per assignment:

```svelte
<div class="space-y-3 md:hidden">
	{#each groupStaffInvigilatorRowsByDay(rows) as group, index (group.examDate)}
		<Collapsible.Root open={index === 0} class="rounded-xl border bg-card">
			<Collapsible.Trigger
				class="flex w-full items-center justify-between gap-3 p-4 text-left font-medium"
			>
				<span>{formatStaffExamDate(group.examDate)}</span>
				<span>{group.rows.length} ห้อง <ChevronDown class="size-4" aria-hidden="true" /></span>
			</Collapsible.Trigger>
			<Collapsible.Content class="divide-y border-t">
				{#each group.rows as row (row.assignment.assignmentId)}
					<div class={cn('grid gap-2 p-4 text-sm', row.isCurrentUser && 'bg-primary/5')}>
						<div class="font-medium">{row.assignment.classroomName}</div>
						<div>ห้องสอบ: {roomLabel(row.assignment)}</div>
						<div>เวลา: {assignmentTimeRanges(row.assignment.sessions)}</div>
						<div class="flex flex-wrap gap-1">
							{#each row.assignment.invigilators as invigilator (invigilator.staffId)}
								<Badge variant={invigilator.staffId === currentStaffId ? 'default' : 'outline'}>
									{invigilator.displayName}
									{#if invigilator.staffId === currentStaffId}
										<span aria-hidden="true"> · ฉัน</span>
									{/if}
								</Badge>
							{/each}
						</div>
					</div>
				{/each}
			</Collapsible.Content>
		</Collapsible.Root>
	{/each}
</div>
```

- [ ] **Step 6: Run autofix and focused tests**

Call the Svelte autofixer on both components until clean, then run:

```bash
cd frontend-school
node --test tests/static/staff-exam-schedule-view.test.mjs
node --test tests/static/academic-exam-schedule.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all commands pass.

- [ ] **Step 7: Commit**

```bash
git add frontend-school/src/lib/components/academic/exam-schedule/StaffExamScheduleTable.svelte \
  frontend-school/src/lib/components/academic/exam-schedule/StaffExamInvigilatorTable.svelte \
  frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat(exams): render staff schedule reports"
```

### Task 7: Build the tabbed dashboard and integrate the route

**Files:**
- Create: `frontend-school/src/lib/components/academic/exam-schedule/MyExamInvigilationView.svelte`
- Create: `frontend-school/src/lib/components/academic/exam-schedule/StaffExamScheduleDashboard.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/exams/+page.svelte:1-50`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs:1424-1526`

**Interfaces:**
- Consumes Task 3 `StaffPublishedExamScheduleRound[]`, Task 4 view functions, and Task 6 report components.
- Produces:

```ts
// StaffExamScheduleDashboard.svelte props
{
	rounds: StaffPublishedExamScheduleRound[];
	currentStaffId: string;
}

// MyExamInvigilationView.svelte props
{
	summary: MyExamInvigilationSummary;
}
```

- [ ] **Step 1: Write failing dashboard and route assertions**

Change the old staff-page assertions so the staff route must use the dashboard and must not use the
personal view:

```js
assert.match(staffPage, /type StaffPublishedExamScheduleRound/);
assert.match(staffPage, /StaffExamScheduleDashboard/);
assert.match(staffPage, /authStore/);
assert.doesNotMatch(staffPage, /PersonalExamScheduleView/);
assert.doesNotMatch(staffPage, /showSeatNumber/);
```

Keep the personal privacy test, but build its source string from only
`PersonalExamScheduleView.svelte`, the student page, and the parent page. Add dashboard assertions:

```js
for (const label of ['ภาพรวม', 'ตารางสอบ', 'กรรมการคุมสอบ', 'งานคุมของฉัน']) {
	assert.match(dashboard, new RegExp(label));
}
assert.match(dashboard, /Tabs\.Root/);
assert.match(dashboard, /Select\.Root/);
assert.match(dashboard, /StaffExamScheduleTable/);
assert.match(dashboard, /StaffExamInvigilatorTable/);
assert.match(dashboard, /MyExamInvigilationView/);
assert.match(dashboard, /ล้างตัวกรอง/);
assert.match(dashboard, /rounds\[0\]\?\.roundId/);
assert.match(dashboard, /activeTab = 'schedule'/);
assert.match(dashboard, /selectedDayId = examDayId/);
assert.match(dashboard, /aria-label=/);

for (const forbidden of ['nationalId', 'national_id', 'username', 'phone', 'email']) {
	assert.doesNotMatch([staffPage, dashboard, scheduleTable, invigilatorTable, myView].join('\n'), new RegExp(forbidden));
}
```

- [ ] **Step 2: Run the static test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/academic-exam-schedule.test.mjs
```

Expected: failure because the dashboard and personal-duty components do not exist and the route
still renders `PersonalExamScheduleView`.

- [ ] **Step 3: Implement the personal-duty view**

Render three compact totals with local Card components: assigned days, room-day assignments, and
formatted supervision minutes. When `summary.items` is empty, render:

```svelte
<PageState
	title="ยังไม่มีงานคุมสอบของฉัน"
	description="ไม่พบชื่อของคุณในกรรมการคุมสอบของรอบนี้"
/>
```

Otherwise render upcoming items first and completed items in a muted section. Each item shows the
date, earliest/latest bounds, actual minutes, classroom, room, and unique subject names.

- [ ] **Step 4: Implement dashboard state and derived data**

Use typed props and Svelte 5 runes:

```ts
let { rounds, currentStaffId }: Props = $props();
let selectedRoundId = $state(rounds[0]?.roundId ?? '');
let selectedDayId = $state('all');
let selectedLevel = $state<StaffExamScheduleLevelFilter>('all');
let selectedClassroomId = $state('all');
let query = $state('');
let activeTab = $state('overview');
let now = new Date();

let selectedRound = $derived(rounds.find((round) => round.roundId === selectedRoundId) ?? rounds[0] ?? null);
let filtered = $derived(
	selectedRound
		? filterStaffExamScheduleRound(selectedRound, {
				dayId: selectedDayId,
				level: selectedLevel,
				classroomId: selectedClassroomId,
				query
			})
		: { sessions: [], assignments: [] }
);
let scheduleRows = $derived(buildStaffExamScheduleRenderRows(filtered.sessions));
let invigilatorRows = $derived(
	buildStaffExamInvigilatorRenderRows(filtered.assignments, currentStaffId)
);
let mySummary = $derived(
	buildMyExamInvigilationSummary(
		selectedRound ? flattenStaffExamScheduleRound(selectedRound).assignments : [],
		currentStaffId,
		now
	)
);
```

Round changes call a handler that resets day, level, classroom, and query filters. Level changes
reset only the classroom filter. The clear action resets all four tab-level filters. Derive day and
classroom options from the selected round; classroom options respect the selected level.

```ts
function clearFilters() {
	selectedDayId = 'all';
	selectedLevel = 'all';
	selectedClassroomId = 'all';
	query = '';
}

function selectRound(roundId: string) {
	selectedRoundId = roundId;
	clearFilters();
}

function selectLevel(level: StaffExamScheduleLevelFilter) {
	selectedLevel = level;
	selectedClassroomId = 'all';
}
```

- [ ] **Step 5: Implement controls, stable summary cards, and four tabs**

Use shadcn `Select`, `Input`, `Button`, `Card`, `Badge`, and `Tabs`. Summary cards use
`buildStaffExamRoundSummary(selectedRound, currentStaffId, now)` and do not change with tab-level
filters.

The overview shows the selected round's date range, next exam day, lower/upper session counts, and
next personal assignment. Render a `เผยแพร่แล้ว` badge because this endpoint contains published
rounds only. Each upcoming-day button calls a handler that assigns its `examDayId` to
`selectedDayId` and sets `activeTab = 'schedule'`. The other tabs render the Task 6 tables and
personal view.

```ts
function openExamDay(examDayId: string) {
	selectedDayId = examDayId;
	activeTab = 'schedule';
}
```

When `selectedRound` is null, render the page-level `PageState` title
`ยังไม่มีตารางสอบที่เผยแพร่`. When the selected round has no sessions, render a round-specific
empty state. When filters produce no rows, show inline `PageState` with
`actionLabel="ล้างตัวกรอง"`.

Use `grid gap-3 md:grid-cols-*` classes so filters and summary cards stack on narrow screens. Give
every `Select.Trigger` and search `Input` a visible label or `aria-label`. Put the Tabs list in an
`overflow-x-auto` wrapper and preserve visible focus styling supplied by the shared primitives.

- [ ] **Step 6: Integrate the route and current user**

Replace the staff page's personal type/component imports:

```svelte
<script lang="ts">
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		listStaffExamSchedules,
		type StaffPublishedExamScheduleRound
	} from '$lib/api/examSchedule';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import StaffExamScheduleDashboard from '$lib/components/academic/exam-schedule/StaffExamScheduleDashboard.svelte';
	import { authStore } from '$lib/stores/auth';

	let { data }: PageProps = $props();
	let loading = $state(true);
	let error = $state('');
	let rounds = $state<StaffPublishedExamScheduleRound[]>([]);
	let currentStaffId = $derived($authStore.user?.id ?? '');
```

Keep `loadSchedules()` and retry behavior. Render:

```svelte
<PageShell title={data.title} description="ภาพรวมตารางสอบและการคุมสอบที่ประกาศแล้ว">
	{#if loading}
		<PageSkeleton variant="table" rows={7} columns={7} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadSchedules}
		/>
	{:else}
		<StaffExamScheduleDashboard {rounds} {currentStaffId} />
	{/if}
</PageShell>
```

- [ ] **Step 7: Run autofix and focused tests**

Call the Svelte autofixer on:

- `MyExamInvigilationView.svelte`
- `StaffExamScheduleDashboard.svelte`
- `src/routes/(app)/staff/exams/+page.svelte`
- the two Task 6 table components after any integration correction

Repeat until every file is clean. Then run:

```bash
cd frontend-school
node --test tests/static/staff-exam-schedule-view.test.mjs
node --test tests/static/academic-exam-schedule.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all commands pass with no Svelte or TypeScript diagnostics.

- [ ] **Step 8: Commit**

```bash
git add frontend-school/src/lib/components/academic/exam-schedule/MyExamInvigilationView.svelte \
  frontend-school/src/lib/components/academic/exam-schedule/StaffExamScheduleDashboard.svelte \
  frontend-school/src/routes/'(app)'/staff/exams/+page.svelte \
  frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat(exams): add staff schedule dashboard"
```

### Task 8: Run the complete verification gate

**Files:**
- Verify all files listed in Tasks 1-7.
- Modify only a failing file whose behavior is already specified in this plan.

**Interfaces:**
- Consumes the complete backend contract, generated API, view utility, shared primitive, dashboard components, and route.
- Produces a clean working tree whose test evidence covers backend, API generation, frontend behavior, Svelte diagnostics, formatting, and repository diff safety.

- [ ] **Step 1: Run backend verification**

```bash
cd backend-school
cargo fmt --check
cargo test modules::academic::services::exam_schedule_service::published_views::tests --bin backend-school
cargo test publish_exposes_the_same_session_to_student_staff_and_linked_parent --bin backend-school
cargo test api_contract::tests::documents_self_service_timetable_exam_and_calendar_reads --bin backend-school
cargo check
```

Expected: all commands pass.

- [ ] **Step 2: Run generated-contract verification**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
```

Expected: both commands pass without changing tracked files.

- [ ] **Step 3: Run frontend behavior and static verification**

```bash
cd frontend-school
node --test tests/static/staff-exam-schedule-view.test.mjs
node --test tests/static/academic-exam-schedule.test.mjs
npm run test:static
```

Expected: all tests pass.

- [ ] **Step 4: Run Svelte and formatting verification**

Run the Svelte autofixer one final time on every changed `.svelte` file and confirm zero issues or
suggestions, then run:

```bash
cd frontend-school
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npx prettier --check \
  src/lib/utils/staff-exam-schedule-view.ts \
  src/lib/components/ui/collapsible \
  src/lib/components/academic/exam-schedule/StaffExamScheduleTable.svelte \
  src/lib/components/academic/exam-schedule/StaffExamInvigilatorTable.svelte \
  src/lib/components/academic/exam-schedule/MyExamInvigilationView.svelte \
  src/lib/components/academic/exam-schedule/StaffExamScheduleDashboard.svelte \
  'src/routes/(app)/staff/exams/+page.svelte' \
  tests/static/staff-exam-schedule-view.test.mjs \
  tests/static/academic-exam-schedule.test.mjs
```

Expected: all commands pass.

- [ ] **Step 5: Inspect repository safety**

```bash
cd ..
git diff --check
git status --short
git log -8 --oneline
```

The expected final state is clean. If any tracked change remains, return to the task that owns that
file, repeat its focused RED/GREEN verification, and use that task's literal `git add` and commit
commands. Do not create an empty verification commit.
