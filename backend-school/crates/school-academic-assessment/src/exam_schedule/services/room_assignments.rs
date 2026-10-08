use std::collections::{HashMap, HashSet};

use sqlx::PgPool;
use uuid::Uuid;

use crate::exam_schedule::models::{
    DayRoomAssignmentView, GenerateSeatsRequest, InvigilatorView, SeatAssignmentView,
    UpsertDayRoomAssignmentRequest,
};
use school_errors::AppError;

use super::invigilation::{
    fetch_invigilator_views_by_assignment_ids, lock_exam_invigilator_staff_conflict_scope,
    replace_assignment_invigilators_in_tx, validate_invigilator_time_conflicts,
    validate_unique_invigilator_staff_ids,
};
use super::rounds_and_days::{fetch_exam_day_context_for_update, mark_round_draft_after_mutation};
use super::sessions_and_conflicts::validate_day_allows_grade_level;
use super::shared::{require_exam_write, ExamWriteTarget};

#[derive(Debug, sqlx::FromRow)]
struct DayRoomAssignmentViewRow {
    id: Uuid,
    exam_day_id: Uuid,
    homeroom_id: Uuid,
    homeroom_name: String,
    room_id: Uuid,
    room_name: String,
    building_name: Option<String>,
    room_capacity: Option<i32>,
    capacity_override: Option<i32>,
    seats_generated: bool,
}
#[derive(Debug, sqlx::FromRow)]
struct ClassroomAssignmentContext {
    grade_level_id: Uuid,
    is_active: Option<bool>,
}
#[derive(Debug, sqlx::FromRow)]
struct RoomAssignmentContext {
    capacity: i32,
    status: String,
}
#[derive(Debug, sqlx::FromRow)]
pub(super) struct SeatAssignmentContext {
    assignment_id: Uuid,
    pub(super) exam_round_id: Uuid,
    capacity_override: Option<i32>,
    room_capacity: i32,
}
impl DayRoomAssignmentViewRow {
    fn into_view(self, invigilators: Vec<InvigilatorView>) -> DayRoomAssignmentView {
        DayRoomAssignmentView {
            id: self.id,
            exam_day_id: self.exam_day_id,
            homeroom_id: self.homeroom_id,
            homeroom_name: self.homeroom_name,
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
    pub class_number: Option<i32>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SeatAssignmentDraft {
    pub student_id: Uuid,
    pub seat_number: String,
}
pub(super) fn build_default_seat_assignments(
    students: &[SeatStudent],
) -> Result<Vec<SeatAssignmentDraft>, AppError> {
    let mut numbers = HashSet::new();
    students
        .iter()
        .map(|student| {
            let number = student
                .class_number
                .filter(|number| *number > 0)
                .ok_or_else(|| {
                    AppError::BadRequest(
                        "กรุณากำหนดเลขที่นักเรียนในห้องเรียนให้ครบก่อนกำหนดที่นั่งสอบ".to_string(),
                    )
                })?;
            if !numbers.insert(number) {
                return Err(AppError::BadRequest(
                    "เลขที่นักเรียนในห้องเรียนซ้ำกัน กรุณาแก้ไขก่อนกำหนดที่นั่งสอบ".to_string(),
                ));
            }
            Ok(SeatAssignmentDraft {
                student_id: student.student_id,
                seat_number: number.to_string(),
            })
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
    require_exam_write(&mut tx, ExamWriteTarget::Day(exam_day_id)).await?;
    let day_context = fetch_exam_day_context_for_update(&mut tx, exam_day_id).await?;
    let classroom = fetch_classroom_assignment_context(&mut tx, request.homeroom_id).await?;
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
        count_active_classroom_students(&mut tx, request.homeroom_id).await?;
    if active_student_count > i64::from(effective_capacity) {
        return Err(AppError::BadRequest(format!(
            "Classroom has {active_student_count} active student(s), which exceeds the room capacity of {effective_capacity}"
        )));
    }

    let assignment_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO academic_exam_day_room_assignments (
            exam_day_id,
            academic_term_id,
            academic_year_id,
            homeroom_id,
            room_id,
            capacity_override,
            created_by,
            updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
        ON CONFLICT (exam_day_id, homeroom_id)
        DO UPDATE SET
            room_id = EXCLUDED.room_id,
            capacity_override = EXCLUDED.capacity_override,
            updated_by = EXCLUDED.updated_by,
            updated_at = now()
        RETURNING id
        "#,
    )
    .bind(exam_day_id)
    .bind(day_context.academic_term_id)
    .bind(day_context.academic_year_id)
    .bind(request.homeroom_id)
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

    synchronize_assignment_seats_in_tx(&mut tx, &[assignment_id]).await?;
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
    require_exam_write(&mut tx, ExamWriteTarget::Assignment(assignment_id)).await?;
    let assignment_context = fetch_seat_assignment_context(&mut tx, assignment_id).await?;

    let existing_seats = fetch_seat_assignments_for_assignment(&mut tx, assignment_id).await?;
    if !request.regenerate && !existing_seats.is_empty() {
        tx.commit().await?;
        return Ok(existing_seats);
    }

    synchronize_assignment_seats_in_tx(&mut tx, &[assignment_id]).await?;
    mark_round_draft_after_mutation(
        &mut tx,
        assignment_context.exam_round_id,
        Some(actor_user_id),
    )
    .await?;

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
    homeroom_id: Uuid,
) -> Result<ClassroomAssignmentContext, AppError> {
    sqlx::query_as::<_, ClassroomAssignmentContext>(
        r#"
        SELECT grade_level_id,
               is_active
        FROM homerooms
        WHERE id = $1
        "#,
    )
    .bind(homeroom_id)
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
        SELECT capacity,
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
    homeroom_id: Uuid,
) -> Result<i64, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM homeroom_placements placement
        JOIN student_academic_years student_year
          ON student_year.id = placement.student_academic_year_id
        WHERE placement.homeroom_id = $1
          AND placement.status = 'current'
          AND student_year.status = 'active'
        "#,
    )
    .bind(homeroom_id)
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
               assignment.homeroom_id,
               classroom.name AS homeroom_name,
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
        JOIN homerooms classroom ON classroom.id = assignment.homeroom_id
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
               assignment.homeroom_id,
               classroom.name AS homeroom_name,
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
        JOIN homerooms classroom ON classroom.id = assignment.homeroom_id
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
               assignment.homeroom_id,
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
#[derive(sqlx::FromRow)]
struct AssignmentSeatStudent {
    assignment_id: Uuid,
    student_id: Uuid,
    class_number: Option<i32>,
}

pub(super) async fn synchronize_assignment_seats_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    assignment_ids: &[Uuid],
) -> Result<(), AppError> {
    if assignment_ids.is_empty() {
        return Ok(());
    }
    let contexts = sqlx::query_as::<_, SeatAssignmentContext>(
        r#"
        SELECT assignment.id AS assignment_id, day.exam_round_id,
               assignment.homeroom_id, assignment.capacity_override,
               room.capacity AS room_capacity
        FROM academic_exam_day_room_assignments assignment
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN rooms room ON room.id = assignment.room_id
        WHERE assignment.id = ANY($1)
        ORDER BY assignment.id
        FOR UPDATE OF assignment
        "#,
    )
    .bind(assignment_ids)
    .fetch_all(&mut **tx)
    .await?;
    let rows = sqlx::query_as::<_, AssignmentSeatStudent>(
        r#"
        SELECT assignment.id AS assignment_id, student_year.student_id,
               placement.class_number
        FROM academic_exam_day_room_assignments assignment
        JOIN homeroom_placements placement
          ON placement.homeroom_id = assignment.homeroom_id
         AND placement.status = 'current'
        JOIN student_academic_years student_year
          ON student_year.id = placement.student_academic_year_id
         AND student_year.academic_year_id = assignment.academic_year_id
         AND student_year.status = 'active'
        JOIN users student ON student.id = student_year.student_id
         AND student.user_type = 'student' AND student.status = 'active'
        WHERE assignment.id = ANY($1)
        ORDER BY assignment.id, placement.class_number, student_year.student_id
        FOR SHARE OF placement, student_year, student
        "#,
    )
    .bind(assignment_ids)
    .fetch_all(&mut **tx)
    .await?;
    let mut students_by_assignment: HashMap<Uuid, Vec<SeatStudent>> = HashMap::new();
    for row in rows {
        students_by_assignment
            .entry(row.assignment_id)
            .or_default()
            .push(SeatStudent {
                student_id: row.student_id,
                class_number: row.class_number,
            });
    }
    let mut draft_assignment_ids = Vec::new();
    let mut student_ids = Vec::new();
    let mut seat_numbers = Vec::new();
    for context in contexts {
        let students = students_by_assignment
            .remove(&context.assignment_id)
            .unwrap_or_default();
        validate_seat_generation_capacity(
            students.len(),
            context.capacity_override.unwrap_or(context.room_capacity),
        )?;
        for draft in build_default_seat_assignments(&students)? {
            draft_assignment_ids.push(context.assignment_id);
            student_ids.push(draft.student_id);
            seat_numbers.push(draft.seat_number);
        }
    }
    // Remove changed or departed seats first so number swaps cannot hit the unique constraint.
    // Unchanged seats retain their identities and timestamps.
    sqlx::query(
        r#"
        DELETE FROM academic_exam_seat_assignments seat
        WHERE seat.day_room_assignment_id = ANY($1)
          AND NOT EXISTS (
              SELECT 1 FROM unnest($2::uuid[], $3::uuid[], $4::text[])
                  AS desired(assignment_id, student_id, seat_number)
              WHERE desired.assignment_id = seat.day_room_assignment_id
                AND desired.student_id = seat.student_id
                AND desired.seat_number = seat.seat_number
          )
        "#,
    )
    .bind(assignment_ids)
    .bind(&draft_assignment_ids)
    .bind(&student_ids)
    .bind(&seat_numbers)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO academic_exam_seat_assignments (day_room_assignment_id, student_id, seat_number)
        SELECT * FROM unnest($1::uuid[], $2::uuid[], $3::text[])
        ON CONFLICT (day_room_assignment_id, student_id) DO NOTHING
        "#,
    )
    .bind(&draft_assignment_ids)
    .bind(&student_ids)
    .bind(&seat_numbers)
    .execute(&mut **tx)
    .await?;
    Ok(())
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
