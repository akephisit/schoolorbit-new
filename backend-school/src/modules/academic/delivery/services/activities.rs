use std::collections::HashMap;

use chrono::Utc;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

use super::super::models::{
    ActivityRegistrationType, ActivityResult, LearningOfferingStatus, RosterStatus,
    StudentActivityGroupOption, StudentActivityOfferingOption, StudentActivityRegistrationQuery,
    StudentActivityRegistrationResult,
};
use super::{append_audit, require_writable_term, TermContext};

pub async fn list_registration_options(
    pool: &PgPool,
    student_id: Uuid,
    query: StudentActivityRegistrationQuery,
) -> Result<Vec<StudentActivityOfferingOption>, AppError> {
    let rows: Vec<StudentActivityRegistrationRow> = sqlx::query_as(
        r#"WITH student_context AS (
               SELECT student_year.id AS student_academic_year_id,
                      student_year.grade_level_id,
                      student_year.study_program_id,
                      placement.homeroom_id
               FROM student_academic_years student_year
               JOIN academic_terms selected_term
                 ON selected_term.id = $1
                AND selected_term.academic_year_id = student_year.academic_year_id
               LEFT JOIN LATERAL (
                   SELECT candidate.homeroom_id
                   FROM homeroom_placements candidate
                   WHERE candidate.student_academic_year_id = student_year.id
                     AND candidate.status IN ('current', 'planned')
                     AND candidate.start_date <= selected_term.end_date
                     AND (candidate.end_date IS NULL
                          OR candidate.end_date >= selected_term.start_date)
                   ORDER BY (candidate.status = 'current') DESC,
                            candidate.start_date DESC,
                            candidate.id
                   LIMIT 1
               ) placement ON true
               WHERE student_year.student_id = $2
                 AND student_year.status IN ('planned', 'active')
               LIMIT 1
           )
           SELECT offering.id AS learning_offering_id,
                  offering.academic_term_id,
                  offering.academic_year_id,
                  offering.code_snapshot AS offering_code,
                  offering.name_snapshot AS offering_name,
                  activity.activity_type,
                  learning_group.id AS learning_group_id,
                  learning_group.code AS group_code,
                  learning_group.name AS group_name,
                  learning_group.description AS group_description,
                  CASE
                      WHEN learning_group.capacity IS NULL THEN detail.capacity
                      WHEN detail.capacity IS NULL THEN learning_group.capacity
                      ELSE LEAST(learning_group.capacity, detail.capacity)
                  END AS capacity,
                  (
                      SELECT count(*)::bigint
                      FROM learning_group_students member
                      WHERE member.learning_group_id = learning_group.id
                        AND member.membership_status = 'active'
                  ) AS member_count,
                  ARRAY(
                      SELECT concat_ws(
                                 ' ',
                                 NULLIF(btrim(teacher.title), ''),
                                 NULLIF(btrim(teacher.first_name), ''),
                                 NULLIF(btrim(teacher.last_name), '')
                             )
                      FROM learning_group_teachers assignment
                      JOIN users teacher ON teacher.id = assignment.teacher_id
                      WHERE assignment.learning_group_id = learning_group.id
                        AND teacher.status = 'active'
                      ORDER BY assignment.role, assignment.created_at, assignment.id
                  ) AS teacher_names,
                  EXISTS (
                      SELECT 1
                      FROM learning_group_students member
                      WHERE member.learning_group_id = learning_group.id
                        AND member.student_id = $2
                        AND member.membership_status = 'active'
                  ) AS enrolled,
                  selected_term.status NOT IN ('closing', 'closed', 'cancelled')
                      AND offering.status = 'published'
                      AND learning_group.status = 'published'
                      AND learning_group.roster_status = 'draft' AS registration_open
           FROM student_context learner
           JOIN academic_terms selected_term ON selected_term.id = $1
           JOIN learning_offerings offering
             ON offering.academic_term_id = selected_term.id
            AND offering.academic_year_id = selected_term.academic_year_id
           JOIN activity_offering_details detail
             ON detail.learning_offering_id = offering.id
            AND detail.registration_type = 'self'
           JOIN activities activity ON activity.id = detail.activity_id
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = offering.id
           WHERE offering.status IN ('published', 'closed')
             AND (
                 learning_group.status <> 'closed'
                 OR EXISTS (
                     SELECT 1
                     FROM learning_group_students member
                     WHERE member.learning_group_id = learning_group.id
                       AND member.student_id = $2
                       AND member.membership_status = 'active'
                 )
             )
             AND EXISTS (
                 SELECT 1
                 FROM learning_offering_targets target
                 WHERE target.learning_offering_id = offering.id
                   AND target.grade_level_id = learner.grade_level_id
                   AND target.study_program_id = learner.study_program_id
                   AND (
                       target.target_kind = 'grade_program'
                       OR (target.target_kind = 'homeroom'
                           AND target.homeroom_id = learner.homeroom_id)
                   )
             )
             AND (
                 NOT EXISTS (
                     SELECT 1
                     FROM learning_group_homerooms coverage
                     WHERE coverage.learning_group_id = learning_group.id
                 )
                 OR EXISTS (
                     SELECT 1
                     FROM learning_group_homerooms coverage
                     WHERE coverage.learning_group_id = learning_group.id
                       AND coverage.homeroom_id = learner.homeroom_id
                 )
             )
           ORDER BY offering.code_snapshot, offering.id, learning_group.code, learning_group.id"#,
    )
    .bind(query.academic_term_id)
    .bind(student_id)
    .fetch_all(pool)
    .await?;

    let mut offerings = Vec::<StudentActivityOfferingOption>::new();
    let mut offering_indexes = HashMap::<Uuid, usize>::new();
    for row in rows {
        let offering_index = match offering_indexes.get(&row.learning_offering_id) {
            Some(index) => *index,
            None => {
                let index = offerings.len();
                offering_indexes.insert(row.learning_offering_id, index);
                offerings.push(StudentActivityOfferingOption {
                    id: row.learning_offering_id,
                    academic_term_id: row.academic_term_id,
                    academic_year_id: row.academic_year_id,
                    code: row.offering_code,
                    name: row.offering_name,
                    activity_type: row.activity_type,
                    enrolled_group_id: None,
                    groups: Vec::new(),
                });
                index
            }
        };
        let offering = &mut offerings[offering_index];
        if row.enrolled {
            offering.enrolled_group_id = Some(row.learning_group_id);
        }
        offering.groups.push(StudentActivityGroupOption {
            id: row.learning_group_id,
            code: row.group_code,
            name: row.group_name,
            description: row.group_description,
            capacity: row.capacity,
            member_count: row.member_count,
            teacher_names: row.teacher_names,
            enrolled: row.enrolled,
            registration_open: row.registration_open,
        });
    }
    Ok(offerings)
}

pub async fn enroll(
    pool: &PgPool,
    student_id: Uuid,
    group_id: Uuid,
    query: StudentActivityRegistrationQuery,
) -> Result<StudentActivityRegistrationResult, AppError> {
    let mut transaction = pool.begin().await?;
    let context = lock_registration_context(&mut transaction, group_id).await?;
    let term = require_registration_window(&mut transaction, &context, &query).await?;
    let learner = require_eligible_student(&mut transaction, student_id, &context, &term).await?;

    let existing: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"SELECT member.learning_group_id, member.student_academic_year_id
           FROM learning_group_students member
           JOIN learning_groups sibling ON sibling.id = member.learning_group_id
           WHERE sibling.learning_offering_id = $1
             AND member.student_id = $2
             AND member.membership_status = 'active'
           ORDER BY member.created_at, member.id
           LIMIT 1
           FOR UPDATE OF member"#,
    )
    .bind(context.learning_offering_id)
    .bind(student_id)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some((existing_group_id, student_academic_year_id)) = existing {
        if existing_group_id == group_id {
            transaction.commit().await?;
            return Ok(StudentActivityRegistrationResult {
                learning_offering_id: context.learning_offering_id,
                learning_group_id: group_id,
                student_academic_year_id,
                enrolled: true,
                revision: context.revision,
            });
        }
        return Err(AppError::Conflict(
            "นักเรียนลงทะเบียนกลุ่มอื่นของกิจกรรมนี้แล้ว กรุณายกเลิกกลุ่มเดิมก่อน".to_string(),
        ));
    }

    if let Some(capacity) = context.effective_capacity() {
        let member_count: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM learning_group_students \
             WHERE learning_group_id = $1 AND membership_status = 'active'",
        )
        .bind(group_id)
        .fetch_one(&mut *transaction)
        .await?;
        if member_count >= i64::from(capacity) {
            return Err(AppError::Conflict("กลุ่มกิจกรรมนี้เต็มแล้ว".to_string()));
        }
    }

    let membership_id = Uuid::new_v4();
    let joined_at = registration_date(&term);
    sqlx::query(
        r#"INSERT INTO learning_group_students (
               id, learning_group_id, academic_term_id, academic_year_id,
               student_academic_year_id, student_id, membership_status,
               roster_source, joined_at
           ) VALUES ($1, $2, $3, $4, $5, $6, 'active', 'self_registration', $7)"#,
    )
    .bind(membership_id)
    .bind(group_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(learner.student_academic_year_id)
    .bind(student_id)
    .bind(joined_at)
    .execute(&mut *transaction)
    .await?;
    let revision = increment_group_revision(&mut transaction, group_id).await?;
    transaction.commit().await?;
    append_audit(
        pool,
        "activity.self_registered",
        "learning_group",
        group_id,
        context.academic_year_id,
        context.academic_term_id,
        student_id,
        serde_json::json!({
            "learningOfferingId": context.learning_offering_id,
            "membershipId": membership_id
        }),
    )
    .await?;

    Ok(StudentActivityRegistrationResult {
        learning_offering_id: context.learning_offering_id,
        learning_group_id: group_id,
        student_academic_year_id: learner.student_academic_year_id,
        enrolled: true,
        revision,
    })
}

pub async fn unenroll(
    pool: &PgPool,
    student_id: Uuid,
    group_id: Uuid,
    query: StudentActivityRegistrationQuery,
) -> Result<StudentActivityRegistrationResult, AppError> {
    let mut transaction = pool.begin().await?;
    let context = lock_registration_context(&mut transaction, group_id).await?;
    let term = require_registration_window(&mut transaction, &context, &query).await?;
    let membership: (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT id, student_academic_year_id
           FROM learning_group_students
           WHERE learning_group_id = $1
             AND student_id = $2
             AND membership_status = 'active'
           FOR UPDATE"#,
    )
    .bind(group_id)
    .bind(student_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบการลงทะเบียนกิจกรรมที่ต้องการยกเลิก".to_string()))?;
    let left_at = registration_date(&term);
    sqlx::query(
        r#"UPDATE learning_group_students
           SET membership_status = 'removed',
               left_at = GREATEST(joined_at, $1),
               row_version = row_version + 1,
               updated_at = now()
           WHERE id = $2"#,
    )
    .bind(left_at)
    .bind(membership.0)
    .execute(&mut *transaction)
    .await?;
    let revision = increment_group_revision(&mut transaction, group_id).await?;
    transaction.commit().await?;
    append_audit(
        pool,
        "activity.self_unregistered",
        "learning_group",
        group_id,
        context.academic_year_id,
        context.academic_term_id,
        student_id,
        serde_json::json!({
            "learningOfferingId": context.learning_offering_id,
            "membershipId": membership.0
        }),
    )
    .await?;

    Ok(StudentActivityRegistrationResult {
        learning_offering_id: context.learning_offering_id,
        learning_group_id: group_id,
        student_academic_year_id: membership.1,
        enrolled: false,
        revision,
    })
}

pub async fn get_result(
    pool: &PgPool,
    learning_group_id: Uuid,
    student_academic_year_id: Uuid,
) -> Result<Option<ActivityResult>, AppError> {
    Ok(sqlx::query_as::<_, ActivityResultRow>(
        r#"SELECT result.id AS learning_result_id,
                  member.id AS learning_group_student_id,
                  detail.outcome,
                  result.updated_at AS finalized_at
           FROM learning_group_students member
           JOIN learning_results result
             ON result.learning_group_id = member.learning_group_id
            AND result.student_academic_year_id = member.student_academic_year_id
            AND result.kind = 'activity' AND result.status = 'recorded'
           JOIN activity_result_details detail ON detail.learning_result_id = result.id
           WHERE member.learning_group_id = $1
             AND member.student_academic_year_id = $2
           ORDER BY result.updated_at DESC, result.id
           LIMIT 1"#,
    )
    .bind(learning_group_id)
    .bind(student_academic_year_id)
    .fetch_optional(pool)
    .await?
    .map(Into::into))
}

#[derive(sqlx::FromRow)]
struct ActivityResultRow {
    learning_result_id: Uuid,
    learning_group_student_id: Uuid,
    outcome: String,
    finalized_at: chrono::DateTime<chrono::Utc>,
}

impl From<ActivityResultRow> for ActivityResult {
    fn from(row: ActivityResultRow) -> Self {
        Self {
            learning_result_id: row.learning_result_id,
            learning_group_student_id: row.learning_group_student_id,
            outcome: Some(row.outcome),
            attendance_percent: None,
            teacher_comment: None,
            finalized_at: Some(row.finalized_at),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct StudentActivityRegistrationRow {
    learning_offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    offering_code: String,
    offering_name: String,
    activity_type: String,
    learning_group_id: Uuid,
    group_code: String,
    group_name: String,
    group_description: Option<String>,
    capacity: Option<i32>,
    member_count: i64,
    teacher_names: Vec<String>,
    enrolled: bool,
    registration_open: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct RegistrationLockRow {
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    offering_status: LearningOfferingStatus,
    group_status: LearningOfferingStatus,
    roster_status: RosterStatus,
    registration_type: ActivityRegistrationType,
    group_capacity: Option<i32>,
    activity_capacity: Option<i32>,
    revision: i64,
}

impl RegistrationLockRow {
    fn effective_capacity(&self) -> Option<i32> {
        match (self.group_capacity, self.activity_capacity) {
            (Some(group), Some(activity)) => Some(group.min(activity)),
            (Some(group), None) => Some(group),
            (None, Some(activity)) => Some(activity),
            (None, None) => None,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct EligibleStudentRow {
    student_academic_year_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
    homeroom_id: Option<Uuid>,
}

async fn lock_registration_context(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
) -> Result<RegistrationLockRow, AppError> {
    sqlx::query_as(
        r#"SELECT learning_group.id AS learning_group_id,
                  offering.id AS learning_offering_id,
                  offering.academic_term_id,
                  offering.academic_year_id,
                  offering.status AS offering_status,
                  learning_group.status AS group_status,
                  learning_group.roster_status,
                  detail.registration_type,
                  learning_group.capacity AS group_capacity,
                  detail.capacity AS activity_capacity
                  , learning_group.row_version AS revision
           FROM learning_groups learning_group
           JOIN learning_offerings offering
             ON offering.id = learning_group.learning_offering_id
            AND offering.kind = 'activity'
           JOIN activity_offering_details detail
             ON detail.learning_offering_id = offering.id
           WHERE learning_group.id = $1
           FOR UPDATE OF offering, learning_group"#,
    )
    .bind(group_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string()))
}

async fn require_registration_window(
    transaction: &mut Transaction<'_, Postgres>,
    context: &RegistrationLockRow,
    query: &StudentActivityRegistrationQuery,
) -> Result<TermContext, AppError> {
    if context.academic_term_id != query.academic_term_id {
        return Err(AppError::ValidationError(
            "กลุ่มกิจกรรมไม่อยู่ในภาคเรียนที่เลือก".to_string(),
        ));
    }
    if context.registration_type != ActivityRegistrationType::SelfRegistration {
        return Err(AppError::Forbidden(
            "กิจกรรมนี้จัดรายชื่อโดยโรงเรียนและไม่เปิดให้นักเรียนลงทะเบียนเอง".to_string(),
        ));
    }
    if context.offering_status != LearningOfferingStatus::Published
        || context.group_status != LearningOfferingStatus::Published
        || context.roster_status != RosterStatus::Draft
    {
        return Err(AppError::Conflict(
            "กิจกรรมนี้ไม่ได้อยู่ในช่วงเปิดลงทะเบียน".to_string(),
        ));
    }
    require_writable_term(transaction, context.academic_term_id, false).await
}

async fn require_eligible_student(
    transaction: &mut Transaction<'_, Postgres>,
    student_id: Uuid,
    context: &RegistrationLockRow,
    term: &TermContext,
) -> Result<EligibleStudentRow, AppError> {
    let learner: EligibleStudentRow = sqlx::query_as(
        r#"SELECT student_year.id AS student_academic_year_id,
                  student_year.grade_level_id,
                  student_year.study_program_id,
                  placement.homeroom_id
           FROM student_academic_years student_year
           LEFT JOIN LATERAL (
               SELECT candidate.homeroom_id
               FROM homeroom_placements candidate
               WHERE candidate.student_academic_year_id = student_year.id
                 AND candidate.status IN ('current', 'planned')
                 AND candidate.start_date <= $3
                 AND (candidate.end_date IS NULL OR candidate.end_date >= $2)
               ORDER BY (candidate.status = 'current') DESC,
                        candidate.start_date DESC,
                        candidate.id
               LIMIT 1
           ) placement ON true
           WHERE student_year.student_id = $1
             AND student_year.academic_year_id = $4
             AND student_year.status IN ('planned', 'active')"#,
    )
    .bind(student_id)
    .bind(term.start_date)
    .bind(term.end_date)
    .bind(context.academic_year_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::Forbidden("นักเรียนไม่มีสิทธิ์ลงทะเบียนกิจกรรมในปีการศึกษานี้".to_string()))?;

    let target_allowed: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1
               FROM learning_offering_targets target
               WHERE target.learning_offering_id = $1
                 AND target.grade_level_id = $2
                 AND target.study_program_id = $3
                 AND (
                     target.target_kind = 'grade_program'
                     OR (target.target_kind = 'homeroom' AND target.homeroom_id = $4)
                 )
           )"#,
    )
    .bind(context.learning_offering_id)
    .bind(learner.grade_level_id)
    .bind(learner.study_program_id)
    .bind(learner.homeroom_id)
    .fetch_one(&mut **transaction)
    .await?;
    let coverage_allowed: bool = sqlx::query_scalar(
        r#"SELECT NOT EXISTS (
                   SELECT 1 FROM learning_group_homerooms
                   WHERE learning_group_id = $1
               ) OR EXISTS (
                   SELECT 1 FROM learning_group_homerooms
                   WHERE learning_group_id = $1 AND homeroom_id = $2
               )"#,
    )
    .bind(context.learning_group_id)
    .bind(learner.homeroom_id)
    .fetch_one(&mut **transaction)
    .await?;
    if !target_allowed || !coverage_allowed {
        return Err(AppError::Forbidden(
            "กิจกรรมนี้ไม่ได้เปิดสำหรับระดับชั้น แผนการเรียน หรือห้องเรียนของนักเรียน".to_string(),
        ));
    }
    Ok(learner)
}

fn registration_date(term: &TermContext) -> chrono::NaiveDate {
    Utc::now()
        .date_naive()
        .max(term.start_date)
        .min(term.end_date)
}

async fn increment_group_revision(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
) -> Result<i64, AppError> {
    let revision = sqlx::query_scalar(
        "UPDATE learning_groups SET row_version = row_version + 1, updated_at = now() \
         WHERE id = $1 RETURNING row_version",
    )
    .bind(group_id)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(revision)
}
