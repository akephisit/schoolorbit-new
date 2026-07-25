use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    PersonalExamScheduleRound, PersonalExamSessionView, StaffPublishedExamDay,
    StaffPublishedExamInvigilator, StaffPublishedExamRoomAssignment,
    StaffPublishedExamScheduleRound, StaffPublishedExamSession,
};

use super::shared::minutes_between_times;

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

fn group_staff_published_exam_rows(
    assignment_rows: Vec<StaffPublishedExamAssignmentRow>,
    session_rows: Vec<StaffPublishedExamSessionRow>,
) -> Vec<StaffPublishedExamScheduleRound> {
    let mut rounds = Vec::new();
    let mut round_indexes = HashMap::new();
    let mut day_indexes = HashMap::new();
    let mut assignment_locations = HashMap::new();

    for row in assignment_rows {
        let round_index = match round_indexes.get(&row.round_id) {
            Some(index) => *index,
            None => {
                let index = rounds.len();
                rounds.push(StaffPublishedExamScheduleRound {
                    round_id: row.round_id,
                    round_name: row.round_name.clone(),
                    academic_semester_id: row.academic_semester_id,
                    published_at: row.published_at,
                    days: Vec::new(),
                });
                round_indexes.insert(row.round_id, index);
                index
            }
        };

        let day_key = (row.round_id, row.exam_day_id);
        let day_index = match day_indexes.get(&day_key) {
            Some(index) => *index,
            None => {
                let index = rounds[round_index].days.len();
                rounds[round_index].days.push(StaffPublishedExamDay {
                    exam_day_id: row.exam_day_id,
                    label: row.day_label.clone(),
                    exam_date: row.exam_date,
                    sessions: Vec::new(),
                    room_assignments: Vec::new(),
                });
                day_indexes.insert(day_key, index);
                index
            }
        };

        let assignment_index = match assignment_locations.get(&row.assignment_id) {
            Some((_, _, index)) => *index,
            None => {
                let index = rounds[round_index].days[day_index].room_assignments.len();
                rounds[round_index].days[day_index].room_assignments.push(
                    StaffPublishedExamRoomAssignment {
                        assignment_id: row.assignment_id,
                        classroom_id: row.classroom_id,
                        classroom_name: row.classroom_name,
                        room_id: row.room_id,
                        room_name: row.room_name,
                        building_name: row.building_name,
                        session_minutes: 0,
                        earliest_starts_at: None,
                        latest_ends_at: None,
                        invigilators: Vec::new(),
                    },
                );
                assignment_locations.insert(row.assignment_id, (round_index, day_index, index));
                index
            }
        };

        if let (Some(staff_id), Some(display_name)) = (row.staff_id, row.display_name) {
            let invigilators = &mut rounds[round_index].days[day_index].room_assignments
                [assignment_index]
                .invigilators;
            if !invigilators
                .iter()
                .any(|invigilator| invigilator.staff_id == staff_id)
            {
                invigilators.push(StaffPublishedExamInvigilator {
                    staff_id,
                    display_name,
                });
            }
        }
    }

    for row in session_rows {
        let Some(&round_index) = round_indexes.get(&row.round_id) else {
            continue;
        };
        let Some(&day_index) = day_indexes.get(&(row.round_id, row.exam_day_id)) else {
            continue;
        };

        rounds[round_index].days[day_index]
            .sessions
            .push(StaffPublishedExamSession {
                session_id: row.session_id,
                starts_at: row.starts_at,
                ends_at: row.ends_at,
                duration_minutes: row.duration_minutes,
                subject_id: row.subject_id,
                subject_code: row.subject_code,
                subject_name: row.subject_name,
                assessment_category_name: row.assessment_category_name,
                grade_level_id: row.grade_level_id,
                grade_level_name: row.grade_level_name,
                grade_level_type: row.grade_level_type,
                grade_level_year: row.grade_level_year,
                classroom_id: row.classroom_id,
                classroom_name: row.classroom_name,
                day_room_assignment_id: row.day_room_assignment_id,
                room_id: row.room_id,
                room_name: row.room_name,
                building_name: row.building_name,
            });

        if let Some(&(assignment_round_index, assignment_day_index, assignment_index)) =
            assignment_locations.get(&row.day_room_assignment_id)
        {
            let assignment = &mut rounds[assignment_round_index].days[assignment_day_index]
                .room_assignments[assignment_index];
            assignment.session_minutes += minutes_between_times(row.starts_at, row.ends_at);
            assignment.earliest_starts_at = Some(
                assignment
                    .earliest_starts_at
                    .map_or(row.starts_at, |value| value.min(row.starts_at)),
            );
            assignment.latest_ends_at = Some(
                assignment
                    .latest_ends_at
                    .map_or(row.ends_at, |value| value.max(row.ends_at)),
            );
        }
    }

    rounds
}

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
            grade_level_name: "ม.1".to_string(),
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
