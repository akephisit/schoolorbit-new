use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    PersonalExamScheduleRound, PersonalExamSessionView,
};

#[derive(Debug, sqlx::FromRow)]
struct PersonalExamSessionRow {
    round_id: Uuid,
    round_name: String,
    academic_semester_id: Uuid,
    published_at: Option<DateTime<Utc>>,
    exam_date: NaiveDate,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    subject_name: String,
    assessment_category_name: String,
    classroom_name: String,
    room_name: String,
    building_name: Option<String>,
    seat_number: Option<String>,
}

impl PersonalExamSessionRow {
    fn into_session_view(self) -> PersonalExamSessionView {
        PersonalExamSessionView {
            exam_date: self.exam_date,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            subject_name: self.subject_name,
            assessment_category_name: self.assessment_category_name,
            classroom_name: self.classroom_name,
            room_name: self.room_name,
            building_name: self.building_name,
            seat_number: self.seat_number,
        }
    }
}

pub async fn list_my_published_exam_schedule(
    pool: &PgPool,
    user_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    ensure_active_student_user(pool, user_id).await?;
    list_published_exam_schedule_for_student(pool, user_id, academic_semester_id).await
}

pub async fn list_staff_published_exam_schedule(
    pool: &PgPool,
    user_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    ensure_active_staff_user_for_exam_schedule(pool, user_id).await?;
    list_published_exam_schedule_for_staff(pool, academic_semester_id).await
}

pub async fn list_child_published_exam_schedule(
    pool: &PgPool,
    parent_user_id: Uuid,
    student_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    ensure_parent_user_for_exam_schedule(pool, parent_user_id).await?;
    ensure_parent_student_link_for_exam_schedule(pool, parent_user_id, student_id).await?;
    list_published_exam_schedule_for_student(pool, student_id, academic_semester_id).await
}

async fn ensure_active_student_user(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let user_row: Option<(String, String)> =
        sqlx::query_as("SELECT user_type, status FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    match user_row
        .as_ref()
        .map(|(user_type, status)| (user_type.as_str(), status.as_str()))
    {
        Some(("student", "active")) => Ok(()),
        Some(_) => Err(AppError::Forbidden(
            "Only active students can view personal exam schedules".to_string(),
        )),
        None => Err(AppError::AuthError("Please sign in".to_string())),
    }
}

async fn ensure_active_staff_user_for_exam_schedule(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<(), AppError> {
    let user_row: Option<(String, String)> =
        sqlx::query_as("SELECT user_type, status FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    match user_row
        .as_ref()
        .map(|(user_type, status)| (user_type.as_str(), status.as_str()))
    {
        Some(("staff", "active")) => Ok(()),
        Some(_) => Err(AppError::Forbidden(
            "Only active staff can view published exam schedules".to_string(),
        )),
        None => Err(AppError::AuthError("Please sign in".to_string())),
    }
}

async fn ensure_parent_user_for_exam_schedule(
    pool: &PgPool,
    parent_user_id: Uuid,
) -> Result<(), AppError> {
    let user_type: Option<String> = sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(parent_user_id)
        .fetch_optional(pool)
        .await?;

    match user_type.as_deref() {
        Some("parent") => Ok(()),
        Some(_) => Err(AppError::Forbidden("เฉพาะผู้ปกครองเท่านั้น".to_string())),
        None => Err(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string())),
    }
}

async fn ensure_parent_student_link_for_exam_schedule(
    pool: &PgPool,
    parent_user_id: Uuid,
    student_id: Uuid,
) -> Result<(), AppError> {
    let is_linked: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM student_parents
            JOIN users user_account ON user_account.id = student_parents.student_user_id
            WHERE student_parents.parent_user_id = $1
              AND student_parents.student_user_id = $2
              AND user_account.user_type = 'student'
              AND user_account.status = 'active'
        )
        "#,
    )
    .bind(parent_user_id)
    .bind(student_id)
    .fetch_one(pool)
    .await?;

    if !is_linked {
        return Err(AppError::Forbidden(
            "คุณไม่มีสิทธิ์เข้าถึงข้อมูลนักเรียนคนนี้".to_string(),
        ));
    }

    Ok(())
}

async fn list_published_exam_schedule_for_student(
    pool: &PgPool,
    student_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    let rows = sqlx::query_as::<_, PersonalExamSessionRow>(
        r#"
        SELECT round.id AS round_id,
               round.name AS round_name,
               round.academic_semester_id,
               round.published_at,
               day.exam_date,
               session.starts_at,
               session.ends_at,
               COALESCE(NULLIF(subject.name_th, ''), NULLIF(subject.name_en, ''), subject.code)
                   AS subject_name,
               category.name AS assessment_category_name,
               classroom.name AS classroom_name,
               room.name_th AS room_name,
               building.name_th AS building_name,
               seat.seat_number
        FROM student_class_enrollments enrollment
        JOIN users student_user
          ON student_user.id = enrollment.student_id
         AND student_user.user_type = 'student'
         AND student_user.status = 'active'
        JOIN academic_exam_schedule_items item
          ON item.classroom_id = enrollment.class_room_id
        JOIN academic_exam_rounds round
          ON round.id = item.exam_round_id
         AND round.academic_semester_id = item.academic_semester_id
        JOIN academic_exam_sessions session
          ON session.exam_schedule_item_id = item.id
         AND session.exam_round_id = item.exam_round_id
        JOIN academic_exam_days day
          ON day.id = session.exam_day_id
         AND day.exam_round_id = session.exam_round_id
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        LEFT JOIN academic_exam_seat_assignments seat
          ON seat.day_room_assignment_id = assignment.id
         AND seat.student_id = enrollment.student_id
        WHERE enrollment.student_id = $1
          AND enrollment.status = 'active'
          AND round.status = 'published'
          AND ($2::uuid IS NULL OR round.academic_semester_id = $2)
        ORDER BY round.published_at DESC NULLS LAST,
                 round.name,
                 day.exam_date,
                 session.starts_at,
                 classroom.name,
                 subject.code,
                 category.display_order,
                 category.name,
                 session.id
        "#,
    )
    .bind(student_id)
    .bind(academic_semester_id)
    .fetch_all(pool)
    .await?;

    Ok(group_personal_exam_schedule_rows(rows))
}

async fn list_published_exam_schedule_for_staff(
    pool: &PgPool,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    let rows = sqlx::query_as::<_, PersonalExamSessionRow>(
        r#"
        SELECT round.id AS round_id,
               round.name AS round_name,
               round.academic_semester_id,
               round.published_at,
               day.exam_date,
               session.starts_at,
               session.ends_at,
               COALESCE(NULLIF(subject.name_th, ''), NULLIF(subject.name_en, ''), subject.code)
                   AS subject_name,
               category.name AS assessment_category_name,
               classroom.name AS classroom_name,
               room.name_th AS room_name,
               building.name_th AS building_name,
               NULL::text AS seat_number
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
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE round.status = 'published'
          AND ($1::uuid IS NULL OR round.academic_semester_id = $1)
        ORDER BY round.published_at DESC NULLS LAST,
                 round.name,
                 day.exam_date,
                 session.starts_at,
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

    Ok(group_personal_exam_schedule_rows(rows))
}

fn group_personal_exam_schedule_rows(
    rows: Vec<PersonalExamSessionRow>,
) -> Vec<PersonalExamScheduleRound> {
    let mut rounds = Vec::new();
    let mut round_indexes = HashMap::new();

    for row in rows {
        let round_id = row.round_id;
        let round_index = match round_indexes.get(&round_id) {
            Some(index) => *index,
            None => {
                let index = rounds.len();
                rounds.push(PersonalExamScheduleRound {
                    round_id,
                    round_name: row.round_name.clone(),
                    academic_semester_id: row.academic_semester_id,
                    published_at: row.published_at,
                    sessions: Vec::new(),
                });
                round_indexes.insert(round_id, index);
                index
            }
        };

        rounds[round_index].sessions.push(row.into_session_view());
    }

    rounds
}
