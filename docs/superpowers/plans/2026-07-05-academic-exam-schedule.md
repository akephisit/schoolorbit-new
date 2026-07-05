# Academic Exam Schedule Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a school academic exam scheduling system where staff create exam rounds, import only assessment categories marked `in_timetable`, assign classroom-level exam rooms and all-day invigilators per exam day, drag exam blocks onto day timelines using each assessment category's duration, publish valid schedules, and let students/parents view published exam schedules with room and seat numbers.

**Architecture:** Backend owns persistence, permission checks, validation, conflict detection, readiness, publishing, and self/parent filtering. Frontend owns staff workflow and drag/drop interaction but mirrors backend validation for immediate feedback. The source of truth for exam items is `academic_assessment_categories.exam_mode = 'in_timetable'` joined through subject plans to active classroom courses in the selected semester. Exam rooms and invigilators are assigned once per `exam_day + classroom`; scheduled sessions resolve room, invigilators, and generated seats from that assignment.

**Tech Stack:** Rust + Axum + sqlx + PostgreSQL backend; SvelteKit 5 + TypeScript + Tailwind/shadcn frontend; existing `apiClient`/`requireApiData`; existing permission registry pattern; sqlx migrations only through new migration files.

---

## Task 1: Add Schema and Permission Registry Entries

**Files**

- `backend-school/migrations/019_academic_exam_schedule.sql`
- `backend-school/src/permissions/registry.rs`
- `frontend-school/src/lib/permissions/registry.ts`

**Tests First**

- [ ] Before adding code, run:

```bash
cd backend-school
cargo test permissions::registry --lib
```

- [ ] Expect either existing pass or no matching tests. Record existing failures if unrelated.

**Implementation**

- [ ] Add migration `019_academic_exam_schedule.sql`; do not edit existing migrations.
- [ ] Create the tables below in this order:

```sql
-- Academic exam scheduling

INSERT INTO permissions (code, description)
VALUES
  ('academic_exam_schedule.read.school', 'Read academic exam schedules for the school'),
  ('academic_exam_schedule.manage.school', 'Create and manage academic exam schedules for the school'),
  ('academic_exam_schedule.publish.school', 'Publish academic exam schedules for the school')
ON CONFLICT (code) DO NOTHING;

WITH inserted_permissions AS (
  SELECT id, code
  FROM permissions
  WHERE code IN (
    'academic_exam_schedule.read.school',
    'academic_exam_schedule.manage.school',
    'academic_exam_schedule.publish.school'
  )
),
admin_roles AS (
  SELECT id
  FROM roles
  WHERE lower(name) IN ('admin', 'administrator', 'super admin', 'school admin')
)
INSERT INTO role_permissions (role_id, permission_id)
SELECT admin_roles.id, inserted_permissions.id
FROM admin_roles
CROSS JOIN inserted_permissions
ON CONFLICT DO NOTHING;

CREATE TABLE academic_exam_rounds (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  academic_semester_id UUID NOT NULL REFERENCES academic_semesters(id) ON DELETE RESTRICT,
  name TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published')),
  published_at TIMESTAMPTZ,
  published_by UUID REFERENCES users(id) ON DELETE SET NULL,
  created_by UUID REFERENCES users(id) ON DELETE SET NULL,
  updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_rounds_name_not_blank CHECK (btrim(name) <> ''),
  CONSTRAINT academic_exam_rounds_published_fields CHECK (
    (status = 'draft' AND published_at IS NULL)
    OR (status = 'published' AND published_at IS NOT NULL)
  ),
  UNIQUE (academic_semester_id, name)
);

CREATE TABLE academic_exam_days (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_round_id UUID NOT NULL REFERENCES academic_exam_rounds(id) ON DELETE CASCADE,
  exam_date DATE NOT NULL,
  label TEXT,
  start_time TIME NOT NULL,
  end_time TIME NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_days_time_order CHECK (start_time < end_time),
  UNIQUE (exam_round_id, exam_date),
  UNIQUE (exam_round_id, sort_order)
);

CREATE TABLE academic_exam_day_grade_levels (
  exam_day_id UUID NOT NULL REFERENCES academic_exam_days(id) ON DELETE CASCADE,
  grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,
  PRIMARY KEY (exam_day_id, grade_level_id)
);

CREATE TABLE academic_exam_day_blocked_windows (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_day_id UUID NOT NULL REFERENCES academic_exam_days(id) ON DELETE CASCADE,
  label TEXT NOT NULL,
  start_time TIME NOT NULL,
  end_time TIME NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_day_blocked_windows_label_not_blank CHECK (btrim(label) <> ''),
  CONSTRAINT academic_exam_day_blocked_windows_time_order CHECK (start_time < end_time)
);

CREATE TABLE academic_exam_day_room_assignments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_day_id UUID NOT NULL REFERENCES academic_exam_days(id) ON DELETE CASCADE,
  classroom_id UUID NOT NULL REFERENCES class_rooms(id) ON DELETE RESTRICT,
  room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE RESTRICT,
  capacity_override INTEGER CHECK (capacity_override IS NULL OR capacity_override > 0),
  created_by UUID REFERENCES users(id) ON DELETE SET NULL,
  updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (exam_day_id, classroom_id),
  UNIQUE (exam_day_id, room_id)
);

CREATE TABLE academic_exam_day_invigilators (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_day_id UUID NOT NULL REFERENCES academic_exam_days(id) ON DELETE CASCADE,
  day_room_assignment_id UUID NOT NULL REFERENCES academic_exam_day_room_assignments(id) ON DELETE CASCADE,
  staff_id UUID NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
  role_label TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (day_room_assignment_id, staff_id),
  UNIQUE (exam_day_id, staff_id)
);

CREATE TABLE academic_exam_schedule_items (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_round_id UUID NOT NULL REFERENCES academic_exam_rounds(id) ON DELETE CASCADE,
  assessment_category_id UUID NOT NULL REFERENCES academic_assessment_categories(id) ON DELETE RESTRICT,
  classroom_course_id UUID NOT NULL REFERENCES classroom_courses(id) ON DELETE RESTRICT,
  classroom_id UUID NOT NULL REFERENCES class_rooms(id) ON DELETE RESTRICT,
  subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE RESTRICT,
  grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,
  duration_minutes INTEGER NOT NULL CHECK (duration_minutes > 0),
  imported_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (exam_round_id, assessment_category_id, classroom_id)
);

CREATE TABLE academic_exam_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_schedule_item_id UUID NOT NULL REFERENCES academic_exam_schedule_items(id) ON DELETE CASCADE,
  exam_day_id UUID NOT NULL REFERENCES academic_exam_days(id) ON DELETE CASCADE,
  starts_at TIME NOT NULL,
  ends_at TIME NOT NULL,
  created_by UUID REFERENCES users(id) ON DELETE SET NULL,
  updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_sessions_time_order CHECK (starts_at < ends_at),
  UNIQUE (exam_schedule_item_id)
);

CREATE TABLE academic_exam_seat_assignments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  day_room_assignment_id UUID NOT NULL REFERENCES academic_exam_day_room_assignments(id) ON DELETE CASCADE,
  student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  seat_number TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_seat_assignments_seat_not_blank CHECK (btrim(seat_number) <> ''),
  UNIQUE (day_room_assignment_id, student_id),
  UNIQUE (day_room_assignment_id, seat_number)
);

CREATE INDEX idx_academic_exam_rounds_semester_status
  ON academic_exam_rounds (academic_semester_id, status);
CREATE INDEX idx_academic_exam_days_round_date
  ON academic_exam_days (exam_round_id, exam_date);
CREATE INDEX idx_academic_exam_schedule_items_round_classroom
  ON academic_exam_schedule_items (exam_round_id, classroom_id);
CREATE INDEX idx_academic_exam_sessions_day_time
  ON academic_exam_sessions (exam_day_id, starts_at, ends_at);
CREATE INDEX idx_academic_exam_seat_assignments_student
  ON academic_exam_seat_assignments (student_id);
```

- [ ] Add backend permission constants:

```rust
pub const ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL: &str = "academic_exam_schedule.read.school";
pub const ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL: &str = "academic_exam_schedule.manage.school";
pub const ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL: &str = "academic_exam_schedule.publish.school";
```

- [ ] Add three `PermissionDefinition` entries to `ALL_PERMISSIONS`:

```rust
PermissionDefinition {
    code: ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL,
    module: "academic_exam_schedule",
    action: "read",
    scope: "school",
    description: "Read academic exam schedules for the school",
},
PermissionDefinition {
    code: ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL,
    module: "academic_exam_schedule",
    action: "manage",
    scope: "school",
    description: "Create and manage academic exam schedules for the school",
},
PermissionDefinition {
    code: ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL,
    module: "academic_exam_schedule",
    action: "publish",
    scope: "school",
    description: "Publish academic exam schedules for the school",
},
```

- [ ] Mirror the constants in `frontend-school/src/lib/permissions/registry.ts`:

```ts
ACADEMIC_EXAM_SCHEDULE: "academic_exam_schedule",
```

```ts
ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL: "academic_exam_schedule.read.school",
ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL: "academic_exam_schedule.manage.school",
ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL: "academic_exam_schedule.publish.school",
```

**Verification**

- [ ] Run:

```bash
cd backend-school
cargo test permissions::registry --lib
```

- [ ] Run:

```bash
cd frontend-school
npm run check
```

- [ ] Commit:

```bash
git add backend-school/migrations/019_academic_exam_schedule.sql backend-school/src/permissions/registry.rs frontend-school/src/lib/permissions/registry.ts
git commit -m "feat: add academic exam schedule schema"
```

---

## Task 2: Add Backend Models and Pure Validation Helpers

**Files**

- `backend-school/src/modules/academic/models.rs`
- `backend-school/src/modules/academic/models/exam_schedule.rs`
- `backend-school/src/modules/academic/services.rs`
- `backend-school/src/modules/academic/services/exam_schedule_service.rs`

**Tests First**

- [ ] Create `exam_schedule_service.rs` with test module first and failing tests for pure behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(value: &str) -> NaiveTime {
        NaiveTime::parse_from_str(value, "%H:%M").unwrap()
    }

    #[test]
    fn computes_end_time_from_duration() {
        assert_eq!(add_minutes(t("08:30"), 90).unwrap(), t("10:00"));
    }

    #[test]
    fn detects_half_open_time_overlap() {
        assert!(time_ranges_overlap(t("08:30"), t("10:00"), t("09:59"), t("11:00")));
        assert!(!time_ranges_overlap(t("08:30"), t("10:00"), t("10:00"), t("11:00")));
    }

    #[test]
    fn rejects_placement_outside_day_window() {
        let outcome = validate_session_window(
            t("08:00"),
            120,
            t("08:30"),
            t("16:00"),
            &[BlockedWindow {
                id: None,
                label: "Lunch".to_string(),
                start_time: t("12:00"),
                end_time: t("13:00"),
            }],
        );
        assert!(matches!(outcome, Err(SessionValidationError::BeforeDayStart)));
    }

    #[test]
    fn rejects_placement_across_blocked_window() {
        let outcome = validate_session_window(
            t("11:30"),
            90,
            t("08:30"),
            t("16:00"),
            &[BlockedWindow {
                id: None,
                label: "Lunch".to_string(),
                start_time: t("12:00"),
                end_time: t("13:00"),
            }],
        );
        assert!(matches!(outcome, Err(SessionValidationError::BlockedWindow(_))));
    }
}
```

- [ ] Run the focused test and confirm it fails because helpers do not exist:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
```

**Implementation**

- [ ] Add module exports:

```rust
// backend-school/src/modules/academic/models.rs
pub mod exam_schedule;
```

```rust
// backend-school/src/modules/academic/services.rs
pub mod exam_schedule_service;
```

- [ ] Add DTOs in `models/exam_schedule.rs`. Use existing model style in nearby academic modules. Include these core shapes:

```rust
use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ExamRound {
    pub id: Uuid,
    pub academic_semester_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExamRoundRequest {
    pub academic_semester_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExamRoundRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ExamDay {
    pub id: Uuid,
    pub exam_round_id: Uuid,
    pub exam_date: NaiveDate,
    pub label: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpsertExamDayRequest {
    pub exam_date: NaiveDate,
    pub label: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub sort_order: i32,
    pub grade_level_ids: Vec<Uuid>,
    pub blocked_windows: Vec<BlockedWindowInput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockedWindow {
    pub id: Option<Uuid>,
    pub label: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockedWindowInput {
    pub label: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
}

#[derive(Debug, Deserialize)]
pub struct UpsertDayRoomAssignmentRequest {
    pub classroom_id: Uuid,
    pub room_id: Uuid,
    pub capacity_override: Option<i32>,
    pub invigilator_staff_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PlaceExamSessionRequest {
    pub exam_schedule_item_id: Uuid,
    pub exam_day_id: Uuid,
    pub starts_at: NaiveTime,
}

#[derive(Debug, Serialize)]
pub struct ExamScheduleWorkspace {
    pub round: ExamRound,
    pub days: Vec<ExamDayDetail>,
    pub unscheduled_items: Vec<ExamScheduleItemView>,
    pub scheduled_sessions: Vec<ExamSessionView>,
    pub readiness: ExamScheduleReadiness,
}

#[derive(Debug, Serialize)]
pub struct ExamScheduleReadiness {
    pub can_publish: bool,
    pub blockers: Vec<String>,
}
```

- [ ] Add service helper types and helpers in `services/exam_schedule_service.rs`:

```rust
use chrono::{Duration, NaiveTime};

use crate::errors::AppError;
use crate::modules::academic::models::exam_schedule::BlockedWindow;

#[derive(Debug, PartialEq, Eq)]
pub enum SessionValidationError {
    InvalidDuration,
    EndTimeOverflow,
    BeforeDayStart,
    AfterDayEnd,
    BlockedWindow(String),
}

pub fn add_minutes(start: NaiveTime, minutes: i32) -> Result<NaiveTime, SessionValidationError> {
    if minutes <= 0 {
        return Err(SessionValidationError::InvalidDuration);
    }
    start
        .checked_add_signed(Duration::minutes(i64::from(minutes)))
        .ok_or(SessionValidationError::EndTimeOverflow)
}

pub fn time_ranges_overlap(
    left_start: NaiveTime,
    left_end: NaiveTime,
    right_start: NaiveTime,
    right_end: NaiveTime,
) -> bool {
    left_start < right_end && right_start < left_end
}

pub fn validate_session_window(
    starts_at: NaiveTime,
    duration_minutes: i32,
    day_start: NaiveTime,
    day_end: NaiveTime,
    blocked_windows: &[BlockedWindow],
) -> Result<NaiveTime, SessionValidationError> {
    let ends_at = add_minutes(starts_at, duration_minutes)?;
    if starts_at < day_start {
        return Err(SessionValidationError::BeforeDayStart);
    }
    if ends_at > day_end {
        return Err(SessionValidationError::AfterDayEnd);
    }
    for blocked in blocked_windows {
        if time_ranges_overlap(starts_at, ends_at, blocked.start_time, blocked.end_time) {
            return Err(SessionValidationError::BlockedWindow(blocked.label.clone()));
        }
    }
    Ok(ends_at)
}

fn validation_error_to_app_error(error: SessionValidationError) -> AppError {
    match error {
        SessionValidationError::InvalidDuration => AppError::BadRequest("Exam duration must be greater than zero".into()),
        SessionValidationError::EndTimeOverflow => AppError::BadRequest("Exam end time is outside the valid day range".into()),
        SessionValidationError::BeforeDayStart => AppError::BadRequest("Exam starts before the exam day begins".into()),
        SessionValidationError::AfterDayEnd => AppError::BadRequest("Exam ends after the exam day ends".into()),
        SessionValidationError::BlockedWindow(label) => AppError::BadRequest(format!("Exam overlaps blocked window: {label}")),
    }
}
```

**Verification**

- [ ] Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
```

- [ ] Commit:

```bash
git add backend-school/src/modules/academic/models.rs backend-school/src/modules/academic/models/exam_schedule.rs backend-school/src/modules/academic/services.rs backend-school/src/modules/academic/services/exam_schedule_service.rs
git commit -m "feat: add exam schedule domain helpers"
```

---

## Task 3: Implement Round, Day, and Workspace Backend Service

**Files**

- `backend-school/src/modules/academic/services/exam_schedule_service.rs`
- `backend-school/src/modules/academic/models/exam_schedule.rs`

**Tests First**

- [ ] Add tests for pure readiness composition before database calls:

```rust
#[test]
fn readiness_requires_days_items_rooms_and_sessions() {
    let readiness = build_readiness(WorkspaceCounts {
        day_count: 0,
        item_count: 4,
        unscheduled_count: 4,
        missing_room_assignment_count: 2,
        missing_seat_assignment_count: 2,
    });
    assert!(!readiness.can_publish);
    assert!(readiness.blockers.iter().any(|value| value.contains("exam day")));
    assert!(readiness.blockers.iter().any(|value| value.contains("unscheduled")));
}
```

- [ ] Run focused tests and confirm failure:

```bash
cd backend-school
cargo test readiness_requires_days_items_rooms_and_sessions --bin backend-school
```

**Implementation**

- [ ] Add service functions with this public surface:

```rust
pub async fn list_rounds(
    pool: &PgPool,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<ExamRound>, AppError>;

pub async fn create_round(
    pool: &PgPool,
    request: CreateExamRoundRequest,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError>;

pub async fn update_round(
    pool: &PgPool,
    round_id: Uuid,
    request: UpdateExamRoundRequest,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError>;

pub async fn upsert_exam_day(
    pool: &PgPool,
    round_id: Uuid,
    request: UpsertExamDayRequest,
) -> Result<ExamDayDetail, AppError>;

pub async fn delete_exam_day(
    pool: &PgPool,
    round_id: Uuid,
    exam_day_id: Uuid,
) -> Result<(), AppError>;

pub async fn get_workspace(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ExamScheduleWorkspace, AppError>;
```

- [ ] `create_round` trims `name`; reject blank names with `AppError::BadRequest`.
- [ ] `update_round`, `upsert_exam_day`, `delete_exam_day` call `mark_round_draft_after_mutation` so edits after publish return the round to `draft`:

```rust
async fn mark_round_draft_after_mutation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    round_id: Uuid,
    actor_user_id: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE academic_exam_rounds
        SET status = 'draft',
            published_at = NULL,
            published_by = NULL,
            updated_by = COALESCE($2, updated_by),
            updated_at = now()
        WHERE id = $1
        "#,
        round_id,
        actor_user_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
```

- [ ] `upsert_exam_day` runs in one transaction:
  - validate `start_time < end_time`.
  - validate all `blocked_windows` have `start_time < end_time`.
  - insert or update day by `(exam_round_id, exam_date)`.
  - delete and replace grade-level scope rows for the day.
  - delete and replace blocked windows for the day.
  - mark round draft.
- [ ] `delete_exam_day` removes the day by `(round_id, exam_day_id)`; cascading deletes sessions, room assignments, invigilators, and seats for the day.
- [ ] `get_workspace` returns:
  - round metadata.
  - day details with grade levels and blocked windows.
  - unscheduled imported items.
  - scheduled sessions with subject, assessment category, classroom, room, invigilators.
  - readiness blockers from `build_readiness`.
- [ ] Add `WorkspaceCounts` and `build_readiness`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct WorkspaceCounts {
    pub day_count: i64,
    pub item_count: i64,
    pub unscheduled_count: i64,
    pub missing_room_assignment_count: i64,
    pub missing_seat_assignment_count: i64,
}

pub fn build_readiness(counts: WorkspaceCounts) -> ExamScheduleReadiness {
    let mut blockers = Vec::new();
    if counts.day_count == 0 {
        blockers.push("Add at least one exam day".to_string());
    }
    if counts.item_count == 0 {
        blockers.push("Import in-timetable assessment categories".to_string());
    }
    if counts.unscheduled_count > 0 {
        blockers.push(format!("Schedule {} remaining exam item(s)", counts.unscheduled_count));
    }
    if counts.missing_room_assignment_count > 0 {
        blockers.push(format!("Assign rooms for {} classroom-day group(s)", counts.missing_room_assignment_count));
    }
    if counts.missing_seat_assignment_count > 0 {
        blockers.push(format!("Generate seats for {} classroom-day group(s)", counts.missing_seat_assignment_count));
    }
    ExamScheduleReadiness {
        can_publish: blockers.is_empty(),
        blockers,
    }
}
```

**Verification**

- [ ] Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
cargo check
```

- [ ] Commit:

```bash
git add backend-school/src/modules/academic/models/exam_schedule.rs backend-school/src/modules/academic/services/exam_schedule_service.rs
git commit -m "feat: manage academic exam rounds"
```

---

## Task 4: Implement Import, Room Assignment, Invigilator, and Seat Service

**Files**

- `backend-school/src/modules/academic/services/exam_schedule_service.rs`
- `backend-school/src/modules/academic/models/exam_schedule.rs`

**Tests First**

- [ ] Add pure seat-number test:

```rust
#[test]
fn generates_padded_seat_numbers_in_student_order() {
    let students = vec![
        SeatStudent { student_id: Uuid::nil(), order_key: "02".to_string() },
        SeatStudent { student_id: Uuid::max(), order_key: "10".to_string() },
    ];
    let seats = build_default_seat_assignments(&students);
    assert_eq!(seats[0].seat_number, "01");
    assert_eq!(seats[1].seat_number, "02");
}
```

- [ ] Run focused test and confirm failure:

```bash
cd backend-school
cargo test generates_padded_seat_numbers_in_student_order --bin backend-school
```

**Implementation**

- [ ] Add request/response DTOs:

```rust
#[derive(Debug, Deserialize)]
pub struct ImportExamItemsRequest {
    pub grade_level_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Serialize)]
pub struct ImportExamItemsResult {
    pub inserted_count: i64,
    pub skipped_existing_count: i64,
    pub skipped_missing_duration_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DayRoomAssignmentView {
    pub id: Uuid,
    pub exam_day_id: Uuid,
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub room_id: Uuid,
    pub room_name: String,
    pub building_name: Option<String>,
    pub room_capacity: Option<i32>,
    pub capacity_override: Option<i32>,
    pub invigilators: Vec<InvigilatorView>,
    pub seats_generated: bool,
}

#[derive(Debug, Serialize)]
pub struct InvigilatorView {
    pub staff_id: Uuid,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateSeatsRequest {
    pub regenerate: bool,
}
```

- [ ] Add service functions:

```rust
pub async fn import_exam_items(
    pool: &PgPool,
    round_id: Uuid,
    request: ImportExamItemsRequest,
    actor_user_id: Uuid,
) -> Result<ImportExamItemsResult, AppError>;

pub async fn list_day_room_assignments(
    pool: &PgPool,
    exam_day_id: Uuid,
) -> Result<Vec<DayRoomAssignmentView>, AppError>;

pub async fn upsert_day_room_assignment(
    pool: &PgPool,
    exam_day_id: Uuid,
    request: UpsertDayRoomAssignmentRequest,
    actor_user_id: Uuid,
) -> Result<DayRoomAssignmentView, AppError>;

pub async fn generate_seats_for_assignment(
    pool: &PgPool,
    assignment_id: Uuid,
    request: GenerateSeatsRequest,
) -> Result<Vec<SeatAssignmentView>, AppError>;
```

- [ ] `import_exam_items` expands assessment categories through subject plans and classroom courses:

```sql
WITH round_context AS (
  SELECT id AS exam_round_id, academic_semester_id
  FROM academic_exam_rounds
  WHERE id = $1
),
source_items AS (
  SELECT
    rc.exam_round_id,
    c.id AS assessment_category_id,
    cc.id AS classroom_course_id,
    cr.id AS classroom_id,
    ap.subject_id,
    cr.grade_level_id,
    c.exam_duration_minutes AS duration_minutes
  FROM round_context rc
  JOIN academic_assessment_plans ap
    ON ap.academic_semester_id = rc.academic_semester_id
  JOIN academic_assessment_categories c
    ON c.assessment_plan_id = ap.id
  JOIN classroom_courses cc
    ON cc.academic_semester_id = rc.academic_semester_id
   AND cc.subject_id = ap.subject_id
  JOIN class_rooms cr
    ON cr.id = cc.classroom_id
  WHERE c.exam_mode = 'in_timetable'
    AND c.exam_duration_minutes IS NOT NULL
    AND cr.is_active = true
    AND ($2::uuid[] IS NULL OR cr.grade_level_id = ANY($2))
)
INSERT INTO academic_exam_schedule_items (
  exam_round_id,
  assessment_category_id,
  classroom_course_id,
  classroom_id,
  subject_id,
  grade_level_id,
  duration_minutes
)
SELECT
  exam_round_id,
  assessment_category_id,
  classroom_course_id,
  classroom_id,
  subject_id,
  grade_level_id,
  duration_minutes
FROM source_items
ON CONFLICT (exam_round_id, assessment_category_id, classroom_id) DO NOTHING;
```

- [ ] Count skipped existing and skipped missing duration with separate `SELECT COUNT(*)` queries using the same source joins.
- [ ] `upsert_day_room_assignment`:
  - validates the selected classroom grade level is allowed by `academic_exam_day_grade_levels`.
  - validates room exists and uses `rooms.capacity` when `capacity_override` is `NULL`.
  - rejects assignment when classroom enrolled student count exceeds effective capacity.
  - upserts `academic_exam_day_room_assignments` by `(exam_day_id, classroom_id)`.
  - replaces invigilators for the assignment.
  - relies on DB unique constraints to prevent the same room or staff being reused on the same day; convert unique violations to `AppError::BadRequest` with a Thai-usable English message.
  - marks round draft.
- [ ] `build_default_seat_assignments` sorts by classroom number when available, then student code, then student id:

```rust
#[derive(Debug, Clone)]
pub struct SeatStudent {
    pub student_id: Uuid,
    pub order_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatAssignmentDraft {
    pub student_id: Uuid,
    pub seat_number: String,
}

pub fn build_default_seat_assignments(students: &[SeatStudent]) -> Vec<SeatAssignmentDraft> {
    students
        .iter()
        .enumerate()
        .map(|(index, student)| SeatAssignmentDraft {
            student_id: student.student_id,
            seat_number: format!("{:02}", index + 1),
        })
        .collect()
}
```

- [ ] `generate_seats_for_assignment` deletes existing seats only when `regenerate = true`; otherwise it returns existing seats if present.
- [ ] Use existing room/building data. Do not duplicate building names into exam tables.

**Verification**

- [ ] Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
cargo check
```

- [ ] Commit:

```bash
git add backend-school/src/modules/academic/models/exam_schedule.rs backend-school/src/modules/academic/services/exam_schedule_service.rs
git commit -m "feat: import exam items and assign exam rooms"
```

---

## Task 5: Implement Session Placement, Conflict Validation, Publish, and Personal Views

**Files**

- `backend-school/src/modules/academic/services/exam_schedule_service.rs`
- `backend-school/src/modules/academic/models/exam_schedule.rs`

**Tests First**

- [ ] Add pure conflict tests:

```rust
#[test]
fn detects_classroom_time_conflict() {
    let candidate = CandidateSession {
        session_id: None,
        classroom_id: Uuid::nil(),
        exam_day_id: Uuid::nil(),
        starts_at: t("09:00"),
        ends_at: t("10:00"),
    };
    let existing = vec![CandidateSession {
        session_id: Some(Uuid::max()),
        classroom_id: Uuid::nil(),
        exam_day_id: Uuid::nil(),
        starts_at: t("09:30"),
        ends_at: t("10:30"),
    }];
    assert!(has_same_classroom_conflict(&candidate, &existing));
}
```

- [ ] Run focused test and confirm failure:

```bash
cd backend-school
cargo test detects_classroom_time_conflict --bin backend-school
```

**Implementation**

- [ ] Add service functions:

```rust
pub async fn place_exam_session(
    pool: &PgPool,
    request: PlaceExamSessionRequest,
    actor_user_id: Uuid,
) -> Result<ExamSessionView, AppError>;

pub async fn delete_exam_session(
    pool: &PgPool,
    session_id: Uuid,
    actor_user_id: Uuid,
) -> Result<(), AppError>;

pub async fn publish_round(
    pool: &PgPool,
    round_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError>;

pub async fn list_my_published_exam_schedule(
    pool: &PgPool,
    user_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError>;

pub async fn list_child_published_exam_schedule(
    pool: &PgPool,
    parent_user_id: Uuid,
    student_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError>;
```

- [ ] `place_exam_session` performs all validation in a transaction:
  - load schedule item and round.
  - load target day.
  - load day grade-level scope; reject if item grade level is not included.
  - load blocked windows; compute `ends_at` from item duration via `validate_session_window`.
  - require `academic_exam_day_room_assignments` exists for `(exam_day_id, classroom_id)`.
  - reject same classroom overlap on the target day.
  - reject same room overlap on the target day by joining existing sessions through room assignment.
  - insert or update one session by `exam_schedule_item_id`.
  - mark round draft.
- [ ] Use half-open time ranges for conflicts: `[starts_at, ends_at)`.
- [ ] Add pure conflict helper:

```rust
#[derive(Debug, Clone)]
pub struct CandidateSession {
    pub session_id: Option<Uuid>,
    pub classroom_id: Uuid,
    pub exam_day_id: Uuid,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
}

pub fn has_same_classroom_conflict(
    candidate: &CandidateSession,
    existing: &[CandidateSession],
) -> bool {
    existing.iter().any(|item| {
        item.exam_day_id == candidate.exam_day_id
            && item.classroom_id == candidate.classroom_id
            && item.session_id != candidate.session_id
            && time_ranges_overlap(candidate.starts_at, candidate.ends_at, item.starts_at, item.ends_at)
    })
}
```

- [ ] `publish_round` calls `get_workspace`, checks `readiness.can_publish`, and updates:

```sql
UPDATE academic_exam_rounds
SET status = 'published',
    published_at = now(),
    published_by = $2,
    updated_by = $2,
    updated_at = now()
WHERE id = $1
RETURNING *
```

- [ ] `list_my_published_exam_schedule` resolves the current student from `user_id`, includes only `academic_exam_rounds.status = 'published'`, and returns only that student's classroom sessions and seat number.
- [ ] `list_child_published_exam_schedule` first verifies the parent-child link using the same relationship rules as `parents::services::get_child_timetable`, then returns the same personal schedule shape.
- [ ] Personal view shape:

```rust
#[derive(Debug, Serialize)]
pub struct PersonalExamScheduleRound {
    pub round_id: Uuid,
    pub round_name: String,
    pub academic_semester_id: Uuid,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub sessions: Vec<PersonalExamSessionView>,
}

#[derive(Debug, Serialize)]
pub struct PersonalExamSessionView {
    pub exam_date: chrono::NaiveDate,
    pub starts_at: chrono::NaiveTime,
    pub ends_at: chrono::NaiveTime,
    pub subject_name: String,
    pub assessment_category_name: String,
    pub classroom_name: String,
    pub room_name: String,
    pub building_name: Option<String>,
    pub seat_number: Option<String>,
}
```

**Verification**

- [ ] Run:

```bash
cd backend-school
cargo test exam_schedule_service --bin backend-school
cargo check
```

- [ ] Commit:

```bash
git add backend-school/src/modules/academic/models/exam_schedule.rs backend-school/src/modules/academic/services/exam_schedule_service.rs
git commit -m "feat: schedule and publish academic exams"
```

---

## Task 6: Add Backend Handlers and Routes

**Files**

- `backend-school/src/modules/academic/handlers.rs`
- `backend-school/src/modules/academic/handlers/exam_schedule.rs`
- `backend-school/src/modules/academic.rs`
- `backend-school/src/main.rs`
- `backend-school/src/modules/parents/handlers.rs`
- `backend-school/src/modules/parents/services.rs`

**Tests First**

- [ ] Search existing route tests before adding new ones:

```bash
rg "academic/timetable|parent/students/.*/timetable|/api/me/timetable" backend-school/tests backend-school/src
```

- [ ] If route smoke tests exist, add failing route coverage for:
  - admin route requires read/manage/publish permission.
  - self route does not require academic permission.
  - parent route verifies parent-child link.
- [ ] If no route tests exist, add a static architecture test in `backend-school/tests/static_architecture.rs` that checks the handler file and route strings exist.

**Implementation**

- [ ] Add handler module export:

```rust
// backend-school/src/modules/academic/handlers.rs
pub mod exam_schedule;
```

- [ ] Handler functions should be thin: permission check, service call, response.
- [ ] Create `handlers/exam_schedule.rs` with these endpoints:

```rust
pub async fn list_rounds(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Query(query): Query<ListExamRoundsQuery>,
) -> Result<Json<Vec<ExamRound>>, AppError>;

pub async fn create_round(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(request): Json<CreateExamRoundRequest>,
) -> Result<(StatusCode, Json<ExamRound>), AppError>;

pub async fn get_workspace(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(round_id): Path<Uuid>,
) -> Result<Json<ExamScheduleWorkspace>, AppError>;

pub async fn import_items(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(round_id): Path<Uuid>,
    Json(request): Json<ImportExamItemsRequest>,
) -> Result<Json<ImportExamItemsResult>, AppError>;

pub async fn upsert_day(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(round_id): Path<Uuid>,
    Json(request): Json<UpsertExamDayRequest>,
) -> Result<Json<ExamDayDetail>, AppError>;

pub async fn upsert_day_room_assignment(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(exam_day_id): Path<Uuid>,
    Json(request): Json<UpsertDayRoomAssignmentRequest>,
) -> Result<Json<DayRoomAssignmentView>, AppError>;

pub async fn generate_seats(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(assignment_id): Path<Uuid>,
    Json(request): Json<GenerateSeatsRequest>,
) -> Result<Json<Vec<SeatAssignmentView>>, AppError>;

pub async fn place_session(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(request): Json<PlaceExamSessionRequest>,
) -> Result<Json<ExamSessionView>, AppError>;

pub async fn delete_session(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, AppError>;

pub async fn publish_round(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(round_id): Path<Uuid>,
) -> Result<Json<ExamRound>, AppError>;
```

- [ ] Permission mapping:
  - list/get workspace: `ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL`
  - create/update/day/import/room/session/seat: `ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL`
  - publish: `ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL`
- [ ] Register routes in `backend-school/src/modules/academic.rs` under `/api/academic/exam-schedules`:

```rust
.route("/exam-schedules", get(exam_schedule::list_rounds).post(exam_schedule::create_round))
.route("/exam-schedules/{round_id}", get(exam_schedule::get_workspace).patch(exam_schedule::update_round))
.route("/exam-schedules/{round_id}/import-items", post(exam_schedule::import_items))
.route("/exam-schedules/{round_id}/days", post(exam_schedule::upsert_day))
.route("/exam-schedules/days/{exam_day_id}", delete(exam_schedule::delete_day))
.route("/exam-schedules/days/{exam_day_id}/room-assignments", post(exam_schedule::upsert_day_room_assignment))
.route("/exam-schedules/room-assignments/{assignment_id}/seats", post(exam_schedule::generate_seats))
.route("/exam-schedules/sessions", post(exam_schedule::place_session))
.route("/exam-schedules/sessions/{session_id}", delete(exam_schedule::delete_session))
.route("/exam-schedules/{round_id}/publish", post(exam_schedule::publish_round))
```

- [ ] Add self route in `main.rs` near `/api/me/timetable`:

```rust
.route("/api/me/exam-schedules", get(academic_exam_schedule_handlers::list_my_exam_schedule))
```

- [ ] Add parent route near parent timetable route:

```rust
.route(
    "/api/parent/students/{student_id}/exam-schedules",
    get(parent_handlers::get_child_exam_schedule),
)
```

- [ ] Add parent service wrapper calling `exam_schedule_service::list_child_published_exam_schedule`.

**Verification**

- [ ] Run:

```bash
cd backend-school
cargo test --all-targets
cargo check
```

- [ ] Commit:

```bash
git add backend-school/src/modules/academic/handlers.rs backend-school/src/modules/academic/handlers/exam_schedule.rs backend-school/src/modules/academic.rs backend-school/src/main.rs backend-school/src/modules/parents/handlers.rs backend-school/src/modules/parents/services.rs backend-school/tests/static_architecture.rs
git commit -m "feat: expose academic exam schedule api"
```

---

## Task 7: Add Frontend API Client and Route Metadata

**Files**

- `frontend-school/src/lib/api/examSchedule.ts`
- `frontend-school/src/routes/(app)/staff/academic/exam-schedules/_meta.ts`
- `frontend-school/src/routes/(app)/staff/academic/exam-schedules/+page.ts`
- `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.ts`
- `frontend-school/src/routes/(app)/student/exams/_meta.ts`
- `frontend-school/src/routes/(app)/student/exams/+page.ts`
- `frontend-school/src/routes/(app)/parent/student/[id]/exams/_meta.ts`
- `frontend-school/src/routes/(app)/parent/student/[id]/exams/+page.ts`
- `frontend-school/tests/static/academic-exam-schedule.test.mjs`

**Tests First**

- [ ] Add a static test that fails until route and permission metadata exist:

```js
import { describe, expect, it } from "vitest";
import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();

describe("academic exam schedule routes", () => {
  it("registers staff, student, and parent exam routes", () => {
    expect(existsSync(join(root, "src/routes/(app)/staff/academic/exam-schedules/+page.svelte"))).toBe(true);
    expect(existsSync(join(root, "src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte"))).toBe(true);
    expect(existsSync(join(root, "src/routes/(app)/student/exams/+page.svelte"))).toBe(true);
    expect(existsSync(join(root, "src/routes/(app)/parent/student/[id]/exams/+page.svelte"))).toBe(true);
  });

  it("guards staff route with exam schedule permissions", () => {
    const meta = readFileSync(join(root, "src/routes/(app)/staff/academic/exam-schedules/_meta.ts"), "utf8");
    expect(meta).toContain("ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL");
  });
});
```

- [ ] Run the static test and confirm it fails:

```bash
cd frontend-school
npm test -- academic-exam-schedule
```

**Implementation**

- [ ] Add API types and methods:

```ts
import { apiClient, requireApiData } from "$lib/api/client";

export type ExamRoundStatus = "draft" | "published";

export interface ExamRound {
  id: string;
  academic_semester_id: string;
  name: string;
  description?: string | null;
  status: ExamRoundStatus;
  published_at?: string | null;
  created_at: string;
  updated_at: string;
}

export interface ExamDay {
  id: string;
  exam_round_id: string;
  exam_date: string;
  label?: string | null;
  start_time: string;
  end_time: string;
  sort_order: number;
}

export interface BlockedWindow {
  id?: string | null;
  label: string;
  start_time: string;
  end_time: string;
}

export interface ExamScheduleItem {
  id: string;
  classroom_id: string;
  classroom_name: string;
  subject_id: string;
  subject_name: string;
  assessment_category_id: string;
  assessment_category_name: string;
  grade_level_id: string;
  grade_level_name: string;
  duration_minutes: number;
}

export interface ExamSession {
  id: string;
  exam_schedule_item_id: string;
  exam_day_id: string;
  starts_at: string;
  ends_at: string;
  classroom_id: string;
  classroom_name: string;
  subject_name: string;
  assessment_category_name: string;
  room_name: string;
  building_name?: string | null;
  invigilators: { staff_id: string; display_name: string }[];
}

export interface ExamScheduleWorkspace {
  round: ExamRound;
  days: Array<ExamDay & { grade_level_ids: string[]; blocked_windows: BlockedWindow[] }>;
  unscheduled_items: ExamScheduleItem[];
  scheduled_sessions: ExamSession[];
  readiness: { can_publish: boolean; blockers: string[] };
}

export async function listExamRounds(params: { academic_semester_id?: string } = {}) {
  return requireApiData<ExamRound[]>(
    apiClient.get("/academic/exam-schedules", { params }),
    "ไม่สามารถโหลดรอบสอบได้",
  );
}

export async function getExamScheduleWorkspace(roundId: string) {
  return requireApiData<ExamScheduleWorkspace>(
    apiClient.get(`/academic/exam-schedules/${roundId}`),
    "ไม่สามารถโหลดตารางสอบได้",
  );
}

export async function placeExamSession(input: {
  exam_schedule_item_id: string;
  exam_day_id: string;
  starts_at: string;
}) {
  return requireApiData<ExamSession>(
    apiClient.post("/academic/exam-schedules/sessions", input),
    "ไม่สามารถวางเวลาสอบได้",
  );
}

export async function listMyExamSchedules(params: { academic_semester_id?: string } = {}) {
  return requireApiData<PersonalExamScheduleRound[]>(
    apiClient.get("/me/exam-schedules", { params }),
    "ไม่สามารถโหลดตารางสอบได้",
  );
}

export async function listChildExamSchedules(studentId: string, params: { academic_semester_id?: string } = {}) {
  return requireApiData<PersonalExamScheduleRound[]>(
    apiClient.get(`/parent/students/${studentId}/exam-schedules`, { params }),
    "ไม่สามารถโหลดตารางสอบของนักเรียนได้",
  );
}
```

- [ ] Add staff route metadata with read permission; the page can conditionally show manage/publish buttons based on permission store.
- [ ] Add student/parent route metadata using the same pattern as existing student/parent timetable pages.
- [ ] Load staff detail page through `+page.ts` using route param `id`.

**Verification**

- [ ] Run:

```bash
cd frontend-school
npm test -- academic-exam-schedule
npm run check
```

- [ ] Commit:

```bash
git add frontend-school/src/lib/api/examSchedule.ts frontend-school/src/routes/\(app\)/staff/academic/exam-schedules frontend-school/src/routes/\(app\)/student/exams frontend-school/src/routes/\(app\)/parent/student/\[id\]/exams frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: add exam schedule frontend api"
```

---

## Task 8: Build Staff Exam Schedule List and Setup Flow

**Files**

- `frontend-school/src/routes/(app)/staff/academic/exam-schedules/+page.svelte`
- `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
- `frontend-school/src/lib/components/academic/exam-schedule/ExamRoundDialog.svelte`
- `frontend-school/src/lib/components/academic/exam-schedule/ExamDaySetupPanel.svelte`
- `frontend-school/src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte`
- `frontend-school/src/lib/components/academic/exam-schedule/ReadinessPanel.svelte`

**Tests First**

- [ ] Add static test assertions that staff pages include the expected API functions and permission constants:

```js
it("staff workspace wires setup, import, room assignment, and publish actions", () => {
  const page = readFileSync(join(root, "src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte"), "utf8");
  expect(page).toContain("getExamScheduleWorkspace");
  expect(page).toContain("importExamItems");
  expect(page).toContain("publishExamRound");
  expect(page).toContain("ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL");
  expect(page).toContain("ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL");
});
```

- [ ] Run:

```bash
cd frontend-school
npm test -- academic-exam-schedule
```

**Implementation**

- [ ] List page:
  - use `PageShell`, `PageSkeleton`, `PageState`, existing `Button`, `Badge`, `Table` patterns.
  - filters by semester when existing semester selector helper is available in the academic area.
  - create round dialog with `name`, `academic_semester_id`, `description`.
  - show status badge `draft`/`published`.
  - navigate to `/staff/academic/exam-schedules/{id}`.
- [ ] Detail page layout:
  - top toolbar: round name, status, import button, publish button.
  - tabs or segmented state for `Setup`, `Schedule`, `Review`.
  - setup tab includes exam days, grade-level scope, blocked windows.
  - room tab assigns one room and all-day invigilators per `exam_day + classroom`.
  - readiness panel always visible in a right side panel on desktop and below timeline on mobile.
- [ ] The setup flow must not ask for per-subject start/end time. It only captures day start/end and blocked windows.
- [ ] Add API methods omitted in Task 7:

```ts
export async function createExamRound(input: {
  academic_semester_id: string;
  name: string;
  description?: string | null;
}) {
  return requireApiData<ExamRound>(
    apiClient.post("/academic/exam-schedules", input),
    "ไม่สามารถสร้างรอบสอบได้",
  );
}

export async function upsertExamDay(roundId: string, input: {
  exam_date: string;
  label?: string | null;
  start_time: string;
  end_time: string;
  sort_order: number;
  grade_level_ids: string[];
  blocked_windows: Array<{ label: string; start_time: string; end_time: string }>;
}) {
  return requireApiData<ExamDay>(
    apiClient.post(`/academic/exam-schedules/${roundId}/days`, input),
    "ไม่สามารถบันทึกวันสอบได้",
  );
}

export async function importExamItems(roundId: string, input: { grade_level_ids?: string[] | null }) {
  return requireApiData<ImportExamItemsResult>(
    apiClient.post(`/academic/exam-schedules/${roundId}/import-items`, input),
    "ไม่สามารถนำเข้ารายการสอบได้",
  );
}

export async function publishExamRound(roundId: string) {
  return requireApiData<ExamRound>(
    apiClient.post(`/academic/exam-schedules/${roundId}/publish`, {}),
    "ไม่สามารถเผยแพร่ตารางสอบได้",
  );
}
```

- [ ] Use real room options from the existing facility room API. Show room label as `building / room / capacity`.
- [ ] Use existing staff search/list API for invigilator selection. If the current staff API supports pagination, keep the selector server-driven and searchable.
- [ ] When a staff user lacks manage permission, render read-only setup and room assignment panels.

**Verification**

- [ ] Run:

```bash
cd frontend-school
npm test -- academic-exam-schedule
npm run check
```

- [ ] Commit:

```bash
git add frontend-school/src/routes/\(app\)/staff/academic/exam-schedules frontend-school/src/lib/components/academic/exam-schedule frontend-school/src/lib/api/examSchedule.ts frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: build exam schedule setup ui"
```

---

## Task 9: Build Drag/Drop Timeline Scheduling UI

**Files**

- `frontend-school/src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte`
- `frontend-school/src/lib/components/academic/exam-schedule/ExamItemTray.svelte`
- `frontend-school/src/lib/components/academic/exam-schedule/ExamSessionBlock.svelte`
- `frontend-school/src/lib/utils/examScheduleTime.ts`
- `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`

**Tests First**

- [ ] Add TypeScript/Vitest tests for time helpers:

```ts
import { describe, expect, it } from "vitest";
import { addMinutes, minutesBetween, timeToMinutes, validateTimelinePlacement } from "$lib/utils/examScheduleTime";

describe("exam schedule time helpers", () => {
  it("computes end time from duration", () => {
    expect(addMinutes("08:30", 90)).toBe("10:00");
  });

  it("rejects blocked window overlap", () => {
    expect(
      validateTimelinePlacement({
        startsAt: "11:30",
        durationMinutes: 90,
        dayStart: "08:30",
        dayEnd: "16:00",
        blockedWindows: [{ label: "พักกลางวัน", start_time: "12:00", end_time: "13:00" }],
      }).ok,
    ).toBe(false);
  });

  it("uses half-open ranges", () => {
    expect(minutesBetween("08:30", "10:00")).toBe(90);
    expect(timeToMinutes("10:00")).toBe(600);
  });
});
```

- [ ] Run and confirm failure:

```bash
cd frontend-school
npm test -- examScheduleTime
```

**Implementation**

- [ ] Implement `examScheduleTime.ts`:

```ts
export function timeToMinutes(value: string): number {
  const [hours, minutes] = value.split(":").map(Number);
  return hours * 60 + minutes;
}

export function minutesToTime(value: number): string {
  const hours = Math.floor(value / 60);
  const minutes = value % 60;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

export function addMinutes(start: string, durationMinutes: number): string {
  return minutesToTime(timeToMinutes(start) + durationMinutes);
}

export function minutesBetween(start: string, end: string): number {
  return timeToMinutes(end) - timeToMinutes(start);
}

export function rangesOverlap(leftStart: string, leftEnd: string, rightStart: string, rightEnd: string): boolean {
  return timeToMinutes(leftStart) < timeToMinutes(rightEnd) && timeToMinutes(rightStart) < timeToMinutes(leftEnd);
}
```

- [ ] Timeline UX:
  - rows are grouped by day, then classroom.
  - visible columns are generated from day `start_time` to `end_time` in 15-minute increments.
  - blocked windows render as unavailable bands.
  - unscheduled tray contains imported `in_timetable` items only.
  - dragged block width equals `duration_minutes`.
  - drop snaps to 15-minute increments.
  - frontend blocks drops that fail local validation; backend remains final authority.
  - moving an existing block calls the same `placeExamSession`.
- [ ] Ensure timeline dimensions are stable:

```css
.timeline-grid {
  --slot-width: 24px;
  grid-auto-columns: var(--slot-width);
}

.session-block {
  min-height: 2.25rem;
  overflow: hidden;
}
```

- [ ] Add accessible non-drag fallback:
  - selecting a block opens a small dialog with day and start time.
  - submit calls `placeExamSession`.
- [ ] After successful placement, reload workspace or update local state from returned session.
- [ ] Do not expose invigilators on student/parent pages; showing them in staff review is acceptable.

**Verification**

- [ ] Run:

```bash
cd frontend-school
npm test -- examScheduleTime
npm test -- academic-exam-schedule
npm run check
```

- [ ] Start the app if no dev server is already running:

```bash
cd frontend-school
npm run dev -- --host 0.0.0.0
```

- [ ] Use Playwright or browser inspection to verify:
  - desktop staff workspace timeline is nonblank.
  - mobile layout stacks without overlapping text.
  - dragged block width changes according to duration.
  - invalid drop over lunch is blocked.

- [ ] Commit:

```bash
git add frontend-school/src/lib/components/academic/exam-schedule frontend-school/src/lib/utils/examScheduleTime.ts frontend-school/src/routes/\(app\)/staff/academic/exam-schedules/\[id\]/+page.svelte
git commit -m "feat: add exam schedule timeline"
```

---

## Task 10: Build Student and Parent Published Exam Views

**Files**

- `frontend-school/src/routes/(app)/student/exams/+page.svelte`
- `frontend-school/src/routes/(app)/parent/student/[id]/exams/+page.svelte`
- `frontend-school/src/lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte`
- `frontend-school/src/lib/api/examSchedule.ts`

**Tests First**

- [ ] Extend the static test:

```js
it("personal exam views do not expose invigilators", () => {
  const studentPage = readFileSync(join(root, "src/routes/(app)/student/exams/+page.svelte"), "utf8");
  const parentPage = readFileSync(join(root, "src/routes/(app)/parent/student/[id]/exams/+page.svelte"), "utf8");
  expect(studentPage).not.toContain("invigilator");
  expect(parentPage).not.toContain("invigilator");
});
```

- [ ] Run:

```bash
cd frontend-school
npm test -- academic-exam-schedule
```

**Implementation**

- [ ] Student view calls `listMyExamSchedules`.
- [ ] Parent view calls `listChildExamSchedules(studentId)`.
- [ ] Shared component groups sessions by round and date.
- [ ] Display per session:
  - date.
  - start/end time.
  - subject.
  - assessment category.
  - classroom.
  - building and room.
  - seat number.
- [ ] Empty state says no published exam schedule is available.
- [ ] Loading and error states use existing `PageSkeleton`/`PageState` components.

**Verification**

- [ ] Run:

```bash
cd frontend-school
npm test -- academic-exam-schedule
npm run check
```

- [ ] Commit:

```bash
git add frontend-school/src/routes/\(app\)/student/exams frontend-school/src/routes/\(app\)/parent/student/\[id\]/exams frontend-school/src/lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte frontend-school/src/lib/api/examSchedule.ts frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat: show published exam schedules"
```

---

## Task 11: End-to-End Verification and Cleanup

**Files**

- Any files changed by prior tasks.
- `docs/superpowers/specs/2026-07-05-academic-exam-schedule-design.md`
- `docs/superpowers/plans/2026-07-05-academic-exam-schedule.md`

**Verification**

- [ ] Run backend verification:

```bash
cd backend-school
cargo test --all-targets
cargo check
```

- [ ] Run frontend verification:

```bash
cd frontend-school
npm test -- academic-exam-schedule
npm test -- examScheduleTime
npm run check
```

- [ ] Run repository hygiene:

```bash
git diff --check
git status --short
```

- [ ] If environment variables for smoke tests are configured, run:

```bash
scripts/smoke_test.sh
```

- [ ] Manual browser smoke path:
  - staff creates exam round.
  - staff adds four exam days.
  - staff scopes lower-secondary grade levels to days 1 and 3, upper-secondary grade levels to days 2 and 4.
  - staff imports exam items.
  - staff assigns classroom `ม.1/1` to room `313` for a day and assigns two invigilators.
  - staff generates seats.
  - staff drags a 90-minute exam item onto timeline and confirms end time is computed.
  - staff attempts lunch overlap and sees the drop blocked.
  - staff publishes when readiness is green.
  - student sees published sessions with room and seat.
  - parent sees the same for linked child.

**Final Commit**

- [ ] If any verification-only or cleanup changes remain:

```bash
git add backend-school frontend-school docs/superpowers/plans/2026-07-05-academic-exam-schedule.md
git commit -m "test: verify academic exam schedule workflow"
```

---

## Design Constraints to Preserve

- [ ] Import only `academic_assessment_categories.exam_mode = 'in_timetable'`.
- [ ] Use `exam_duration_minutes` as the block duration; never ask staff to enter subject-specific end time.
- [ ] Day start/end and blocked windows are configured at exam-day level.
- [ ] Exam day grade-level scope controls which grades can be scheduled that day.
- [ ] Room assignment is one classroom to one room for the whole day.
- [ ] Invigilators are all-day per classroom-room-day assignment.
- [ ] Room capacity comes from `rooms.capacity`, with optional per-assignment override.
- [ ] Student and parent views show only published rounds.
- [ ] Student and parent views include seat number but not invigilators.
- [ ] Any edit after publish returns the round to `draft`.
- [ ] Backend validation is final even when frontend blocks invalid drops early.

## Self-Review Notes

- The plan uses a new migration and does not edit existing migrations.
- Backend logic is concentrated in `exam_schedule_service.rs`; handlers remain thin.
- The schema references existing academic, classroom, room, staff, student, and user tables instead of duplicating master data.
- Personal routes follow the existing `/api/me/` and `/api/parent/students/{id}/...` split.
- Frontend route and API naming match the existing staff/student/parent route structure.
- No plaintext sensitive personal identifiers are introduced or logged.
