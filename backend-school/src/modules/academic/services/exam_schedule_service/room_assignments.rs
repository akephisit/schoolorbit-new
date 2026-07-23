use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    DayRoomAssignmentView, GenerateSeatsRequest, InvigilatorView, SeatAssignmentView,
    UpsertDayRoomAssignmentRequest,
};

use super::invigilation::{
    fetch_invigilator_views_by_assignment_ids, lock_exam_invigilator_staff_conflict_scope,
    replace_assignment_invigilators_in_tx, validate_invigilator_time_conflicts,
    validate_unique_invigilator_staff_ids,
};
use super::rounds_and_days::{fetch_exam_day_context_for_update, mark_round_draft_after_mutation};
use super::sessions_and_conflicts::validate_day_allows_grade_level;

#[derive(Debug, sqlx::FromRow)]
struct DayRoomAssignmentViewRow {
    id: Uuid,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    classroom_name: String,
    room_id: Uuid,
    room_name: String,
    building_name: Option<String>,
    room_capacity: Option<i32>,
    capacity_override: Option<i32>,
    seats_generated: bool,
}
#[derive(Debug, sqlx::FromRow)]
struct ClassroomAssignmentContext {
    classroom_id: Uuid,
    classroom_name: String,
    grade_level_id: Uuid,
    is_active: Option<bool>,
}
#[derive(Debug, sqlx::FromRow)]
struct RoomAssignmentContext {
    room_id: Uuid,
    capacity: i32,
    status: String,
}
#[derive(Debug, sqlx::FromRow)]
pub(super) struct SeatAssignmentContext {
    assignment_id: Uuid,
    pub(super) exam_round_id: Uuid,
    classroom_id: Uuid,
    capacity_override: Option<i32>,
    room_capacity: i32,
}
impl DayRoomAssignmentViewRow {
    fn into_view(self, invigilators: Vec<InvigilatorView>) -> DayRoomAssignmentView {
        DayRoomAssignmentView {
            id: self.id,
            exam_day_id: self.exam_day_id,
            classroom_id: self.classroom_id,
            classroom_name: self.classroom_name,
            room_id: self.room_id,
            room_name: self.room_name,
            building_name: self.building_name,
            room_capacity: self.room_capacity,
            capacity_override: self.capacity_override,
            invigilators,
            seats_generated: self.seats_generated,
        }
    }
}
#[derive(Debug, Clone, sqlx::FromRow)]
pub(super) struct SeatStudent {
    pub student_id: Uuid,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SeatAssignmentDraft {
    pub student_id: Uuid,
    pub seat_number: String,
}
pub(super) fn build_default_seat_assignments(students: &[SeatStudent]) -> Vec<SeatAssignmentDraft> {
    students
        .iter()
        .enumerate()
        .map(|(index, student)| SeatAssignmentDraft {
            student_id: student.student_id,
            seat_number: format!("{:02}", index + 1),
        })
        .collect()
}
pub(super) fn validate_seat_generation_capacity(
    active_student_count: usize,
    effective_capacity: i32,
) -> Result<(), AppError> {
    if effective_capacity <= 0 {
        return Err(AppError::BadRequest(
            "Room capacity must be greater than zero".to_string(),
        ));
    }
    if active_student_count > effective_capacity as usize {
        return Err(AppError::BadRequest(format!(
            "Classroom has {active_student_count} active student(s), which exceeds the room capacity of {effective_capacity}"
        )));
    }
    Ok(())
}
pub async fn list_day_room_assignments(
    pool: &PgPool,
    exam_day_id: Uuid,
) -> Result<Vec<DayRoomAssignmentView>, AppError> {
    let day_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM academic_exam_days
            WHERE id = $1
        )
        "#,
    )
    .bind(exam_day_id)
    .fetch_one(pool)
    .await?;

    if !day_exists {
        return Err(AppError::NotFound("Exam day not found".to_string()));
    }

    fetch_day_room_assignment_views_for_day(pool, exam_day_id).await
}
pub async fn upsert_day_room_assignment(
    pool: &PgPool,
    exam_day_id: Uuid,
    request: UpsertDayRoomAssignmentRequest,
    actor_user_id: Uuid,
) -> Result<DayRoomAssignmentView, AppError> {
    let invigilator_staff_ids = request
        .invigilator_staff_ids
        .as_ref()
        .map(|ids| validate_unique_invigilator_staff_ids(ids.clone()))
        .transpose()?;
    let capacity_override = validate_capacity_override(request.capacity_override)?;

    let mut tx = pool.begin().await?;
    let day_context = fetch_exam_day_context_for_update(&mut tx, exam_day_id).await?;
    let classroom = fetch_classroom_assignment_context(&mut tx, request.classroom_id).await?;
    if classroom.is_active != Some(true) {
        return Err(AppError::BadRequest(
            "Classroom must be active before assigning an exam room".to_string(),
        ));
    }
    validate_day_allows_grade_level(&mut tx, exam_day_id, classroom.grade_level_id).await?;

    let room = fetch_room_assignment_context(&mut tx, request.room_id).await?;
    if room.status != "ACTIVE" {
        return Err(AppError::BadRequest(
            "Room must be ACTIVE before assigning it to an exam day".to_string(),
        ));
    }

    let effective_capacity = capacity_override.unwrap_or(room.capacity);
    let active_student_count =
        count_active_classroom_students(&mut tx, request.classroom_id).await?;
    if active_student_count > i64::from(effective_capacity) {
        return Err(AppError::BadRequest(format!(
            "Classroom has {active_student_count} active student(s), which exceeds the room capacity of {effective_capacity}"
        )));
    }

    let assignment_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO academic_exam_day_room_assignments (
            exam_day_id,
            classroom_id,
            room_id,
            capacity_override,
            created_by,
            updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $5)
        ON CONFLICT (exam_day_id, classroom_id)
        DO UPDATE SET
            room_id = EXCLUDED.room_id,
            capacity_override = EXCLUDED.capacity_override,
            updated_by = EXCLUDED.updated_by,
            updated_at = now()
        RETURNING id
        "#,
    )
    .bind(exam_day_id)
    .bind(request.classroom_id)
    .bind(request.room_id)
    .bind(capacity_override)
    .bind(actor_user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_day_room_assignment_write_error)?;

    if let Some(invigilator_staff_ids) = invigilator_staff_ids {
        lock_exam_invigilator_staff_conflict_scope(&mut tx, exam_day_id, &invigilator_staff_ids)
            .await?;
        validate_invigilator_time_conflicts(
            &mut tx,
            day_context.exam_round_id,
            assignment_id,
            &invigilator_staff_ids,
        )
        .await?;
        replace_assignment_invigilators_in_tx(
            &mut tx,
            day_context.exam_round_id,
            exam_day_id,
            assignment_id,
            &invigilator_staff_ids,
        )
        .await?;
    }

    mark_round_draft_after_mutation(&mut tx, day_context.exam_round_id, Some(actor_user_id))
        .await?;
    tx.commit().await?;

    fetch_day_room_assignment_view(pool, assignment_id).await
}
pub async fn generate_seats_for_assignment(
    pool: &PgPool,
    assignment_id: Uuid,
    request: GenerateSeatsRequest,
    actor_user_id: Uuid,
) -> Result<Vec<SeatAssignmentView>, AppError> {
    let mut tx = pool.begin().await?;
    let assignment_context = fetch_seat_assignment_context(&mut tx, assignment_id).await?;

    let existing_seats = fetch_seat_assignments_for_assignment(&mut tx, assignment_id).await?;
    if !request.regenerate && !existing_seats.is_empty() {
        tx.commit().await?;
        return Ok(existing_seats);
    }

    let students = fetch_ordered_seat_students(&mut tx, assignment_context.classroom_id).await?;
    let effective_capacity = assignment_context
        .capacity_override
        .unwrap_or(assignment_context.room_capacity);
    validate_seat_generation_capacity(students.len(), effective_capacity)?;

    let mut wrote_seats = false;
    if request.regenerate {
        sqlx::query(
            r#"
            DELETE FROM academic_exam_seat_assignments
            WHERE day_room_assignment_id = $1
            "#,
        )
        .bind(assignment_id)
        .execute(&mut *tx)
        .await?;
        wrote_seats = true;
    }

    let seat_drafts = build_default_seat_assignments(&students);

    if !seat_drafts.is_empty() {
        let student_ids: Vec<Uuid> = seat_drafts
            .iter()
            .map(|assignment| assignment.student_id)
            .collect();
        let seat_numbers: Vec<String> = seat_drafts
            .iter()
            .map(|assignment| assignment.seat_number.clone())
            .collect();

        sqlx::query(
            r#"
            INSERT INTO academic_exam_seat_assignments (
                day_room_assignment_id,
                student_id,
                seat_number
            )
            SELECT $1, student_id, seat_number
            FROM unnest($2::uuid[], $3::text[]) AS seat(student_id, seat_number)
            "#,
        )
        .bind(assignment_context.assignment_id)
        .bind(&student_ids)
        .bind(&seat_numbers)
        .execute(&mut *tx)
        .await?;
        wrote_seats = true;
    }

    if wrote_seats {
        mark_round_draft_after_mutation(
            &mut tx,
            assignment_context.exam_round_id,
            Some(actor_user_id),
        )
        .await?;
    }

    let seats = fetch_seat_assignments_for_assignment(&mut tx, assignment_id).await?;
    tx.commit().await?;

    Ok(seats)
}
fn validate_capacity_override(capacity_override: Option<i32>) -> Result<Option<i32>, AppError> {
    if matches!(capacity_override, Some(value) if value <= 0) {
        return Err(AppError::BadRequest(
            "Capacity override must be greater than zero".to_string(),
        ));
    }
    Ok(capacity_override)
}
async fn fetch_classroom_assignment_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    classroom_id: Uuid,
) -> Result<ClassroomAssignmentContext, AppError> {
    sqlx::query_as::<_, ClassroomAssignmentContext>(
        r#"
        SELECT id AS classroom_id,
               name AS classroom_name,
               grade_level_id,
               is_active
        FROM class_rooms
        WHERE id = $1
        "#,
    )
    .bind(classroom_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Classroom not found".to_string()))
}
async fn fetch_room_assignment_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    room_id: Uuid,
) -> Result<RoomAssignmentContext, AppError> {
    sqlx::query_as::<_, RoomAssignmentContext>(
        r#"
        SELECT id AS room_id,
               capacity,
               status
        FROM rooms
        WHERE id = $1
        "#,
    )
    .bind(room_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Room not found".to_string()))
}
async fn count_active_classroom_students(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    classroom_id: Uuid,
) -> Result<i64, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM student_class_enrollments enrollment
        WHERE enrollment.class_room_id = $1
          AND enrollment.status = 'active'
        "#,
    )
    .bind(classroom_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::from)
}
async fn fetch_day_room_assignment_views_for_day(
    pool: &PgPool,
    exam_day_id: Uuid,
) -> Result<Vec<DayRoomAssignmentView>, AppError> {
    let rows = sqlx::query_as::<_, DayRoomAssignmentViewRow>(
        r#"
        SELECT assignment.id,
               assignment.exam_day_id,
               assignment.classroom_id,
               classroom.name AS classroom_name,
               assignment.room_id,
               room.name_th AS room_name,
               building.name_th AS building_name,
               room.capacity AS room_capacity,
               assignment.capacity_override,
               EXISTS (
                   SELECT 1
                   FROM academic_exam_seat_assignments seat
                   WHERE seat.day_room_assignment_id = assignment.id
               ) AS seats_generated
        FROM academic_exam_day_room_assignments assignment
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE assignment.exam_day_id = $1
        ORDER BY classroom.name, room.name_th, assignment.id
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(pool)
    .await?;

    hydrate_day_room_assignment_views(pool, rows).await
}
pub(super) async fn fetch_day_room_assignment_view(
    pool: &PgPool,
    assignment_id: Uuid,
) -> Result<DayRoomAssignmentView, AppError> {
    let rows = sqlx::query_as::<_, DayRoomAssignmentViewRow>(
        r#"
        SELECT assignment.id,
               assignment.exam_day_id,
               assignment.classroom_id,
               classroom.name AS classroom_name,
               assignment.room_id,
               room.name_th AS room_name,
               building.name_th AS building_name,
               room.capacity AS room_capacity,
               assignment.capacity_override,
               EXISTS (
                   SELECT 1
                   FROM academic_exam_seat_assignments seat
                   WHERE seat.day_room_assignment_id = assignment.id
               ) AS seats_generated
        FROM academic_exam_day_room_assignments assignment
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE assignment.id = $1
        "#,
    )
    .bind(assignment_id)
    .fetch_all(pool)
    .await?;

    let mut views = hydrate_day_room_assignment_views(pool, rows).await?;
    views
        .pop()
        .ok_or_else(|| AppError::NotFound("Exam room assignment not found".to_string()))
}
async fn hydrate_day_room_assignment_views(
    pool: &PgPool,
    rows: Vec<DayRoomAssignmentViewRow>,
) -> Result<Vec<DayRoomAssignmentView>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let assignment_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let mut invigilators_by_assignment =
        fetch_invigilator_views_by_assignment_ids(pool, &assignment_ids).await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let invigilators = invigilators_by_assignment
                .remove(&row.id)
                .unwrap_or_default();
            row.into_view(invigilators)
        })
        .collect())
}
pub(super) async fn fetch_seat_assignment_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    assignment_id: Uuid,
) -> Result<SeatAssignmentContext, AppError> {
    sqlx::query_as::<_, SeatAssignmentContext>(
        r#"
        SELECT assignment.id AS assignment_id,
               exam_day.exam_round_id,
               assignment.classroom_id,
               assignment.capacity_override,
               room.capacity AS room_capacity
        FROM academic_exam_day_room_assignments assignment
        JOIN academic_exam_days exam_day ON exam_day.id = assignment.exam_day_id
        JOIN rooms room ON room.id = assignment.room_id
        WHERE assignment.id = $1
        FOR UPDATE OF assignment
        "#,
    )
    .bind(assignment_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam room assignment not found".to_string()))
}
async fn fetch_seat_assignments_for_assignment(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    assignment_id: Uuid,
) -> Result<Vec<SeatAssignmentView>, AppError> {
    sqlx::query_as::<_, SeatAssignmentView>(
        r#"
        SELECT seat.id,
               seat.day_room_assignment_id,
               seat.student_id,
               concat_ws(
                   ' ',
                   NULLIF(
                       concat_ws('', NULLIF(TRIM(user_account.title), ''), NULLIF(TRIM(user_account.first_name), '')),
                       ''
                   ),
                   NULLIF(TRIM(user_account.last_name), '')
               )
                   AS student_name,
               seat.seat_number
        FROM academic_exam_seat_assignments seat
        JOIN users user_account ON user_account.id = seat.student_id
        WHERE seat.day_room_assignment_id = $1
        ORDER BY length(seat.seat_number), seat.seat_number, seat.id
        "#,
    )
    .bind(assignment_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}
async fn fetch_ordered_seat_students(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    classroom_id: Uuid,
) -> Result<Vec<SeatStudent>, AppError> {
    sqlx::query_as::<_, SeatStudent>(
        r#"
        SELECT user_account.id AS student_id
        FROM student_class_enrollments enrollment
        JOIN users user_account
          ON user_account.id = enrollment.student_id
         AND user_account.user_type = 'student'
         AND user_account.status = 'active'
        LEFT JOIN student_info ON student_info.user_id = user_account.id
        WHERE enrollment.class_room_id = $1
          AND enrollment.status = 'active'
        ORDER BY enrollment.class_number ASC NULLS LAST,
                 student_info.student_id ASC NULLS LAST,
                 user_account.id ASC
        "#,
    )
    .bind(classroom_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}
pub(super) fn map_day_room_assignment_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_error) = &error {
        let code = db_error.code().unwrap_or_default();
        if code == "23505" {
            let constraint = db_error.constraint().unwrap_or_default();
            if constraint.contains("exam_day_id_room_id") {
                return AppError::BadRequest(
                    "Room is already assigned to another classroom on this exam day".to_string(),
                );
            }
            if constraint.contains("day_room_assignment_id_staff_id") {
                return AppError::BadRequest(
                    "Duplicate invigilator for this room assignment".to_string(),
                );
            }
            return AppError::BadRequest(
                "Exam room assignment conflicts with existing schedule data".to_string(),
            );
        }
    }
    AppError::from(error)
}
