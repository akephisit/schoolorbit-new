use school_database::DbSession;
use std::collections::HashMap;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use school_errors::AppError;

use super::super::models::{
    ActivityRegistrationType, LearningOfferingStatus, RosterStatus, StudentActivityGroupOption,
    StudentActivityOfferingOption, StudentActivityRegistrationQuery,
    StudentActivityRegistrationResult,
};
use super::{
    append_audit, bangkok_today, invalidate_group_academic_confirmations, require_writable_term,
    TermContext,
};

pub async fn list_registration_options(
    pool: &PgPool,
    student_id: Uuid,
    query: StudentActivityRegistrationQuery,
) -> Result<Vec<StudentActivityOfferingOption>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.activities.list_registration_options",
    )
    .await?;
    list_registration_options_on_connection(&mut connection, student_id, query).await
}

pub async fn list_registration_options_on_connection(
    connection: &mut sqlx::PgConnection,
    student_id: Uuid,
    query: StudentActivityRegistrationQuery,
) -> Result<Vec<StudentActivityOfferingOption>, AppError> {
    let rows: Vec<StudentActivityRegistrationRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.activities.list_registration_options_on_connection",
            r#"WITH student_context AS (
               SELECT student_year.id AS student_academic_year_id,
                      student_year.grade_level_id,
                      student_year.study_program_id,
                      placement.homeroom_id
               FROM student_academic_years student_year
               JOIN academic_terms selected_term
                 ON selected_term.id = $1
                AND selected_term.academic_year_id = student_year.academic_year_id
               JOIN academic_years selected_year
                 ON selected_year.id = selected_term.academic_year_id
               LEFT JOIN LATERAL (
                   SELECT candidate.homeroom_id
                   FROM homeroom_placements candidate
                   WHERE candidate.student_academic_year_id = student_year.id
                     AND candidate.status IN ('current', 'planned')
                     AND candidate.start_date <= COALESCE(
                         selected_term.closed_on,
                         selected_term.planned_end_date,
                         selected_year.end_date
                     )
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
                  learner.homeroom_id,
                  GREATEST(selected_term.start_date, LEAST(
                      (now() AT TIME ZONE 'Asia/Bangkok')::date,
                      COALESCE(selected_term.closed_on, selected_term.planned_end_date,
                          (SELECT end_date FROM academic_years WHERE id=selected_term.academic_year_id))
                  )) AS registration_date,
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
                      SELECT teacher.display_name
                      FROM learning_group_teachers assignment
                      JOIN user_display_names teacher ON teacher.id = assignment.teacher_id
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
        .fetch_all()
        .await?;

    let offering_ids = rows
        .iter()
        .map(|row| row.learning_offering_id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let constraints = match rows.first() {
        Some(row) => {
            super::audiences::dated_constraints_on_connection(
                connection,
                &offering_ids,
                row.registration_date,
            )
            .await?
        }
        None => HashMap::new(),
    };
    let mut offerings = Vec::<StudentActivityOfferingOption>::new();
    let mut offering_indexes = HashMap::<Uuid, usize>::new();
    for row in rows {
        let audience_allowed = constraints
            .get(&row.learning_offering_id)
            .is_none_or(|scope| scope.allows_homeroom(row.learning_group_id, row.homeroom_id));
        if !audience_allowed && !row.enrolled {
            continue;
        }
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
            registration_open: row.registration_open && audience_allowed,
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
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.activities.enroll",
    )
    .await?;
    let (context, term) = lock_registration_context(&mut transaction, group_id).await?;
    require_registration_window(&context, &query)?;
    let learner = require_eligible_student(&mut transaction, student_id, &context, &term).await?;

    let existing: Option<(Uuid, Uuid)> = DbSession::new(&mut *transaction)
        .query_as(
            "academic_delivery.services.activities.enroll",
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
        .fetch_optional()
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
        let member_count: i64 = DbSession::new(&mut *transaction)
            .scalar(
                "academic_delivery.services.activities.enroll",
                "SELECT count(*)::bigint FROM learning_group_students \
             WHERE learning_group_id = $1 AND membership_status = 'active'",
            )
            .bind(group_id)
            .fetch_one()
            .await?;
        if member_count >= i64::from(capacity) {
            return Err(AppError::Conflict("กลุ่มกิจกรรมนี้เต็มแล้ว".to_string()));
        }
    }

    let membership_id = Uuid::now_v7();
    let joined_at = registration_date(&term);
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.activities.enroll",
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
        .execute()
        .await?;
    let revision = increment_group_revision(&mut transaction, group_id).await?;
    invalidate_group_academic_confirmations(&mut transaction, &[group_id]).await?;
    transaction.commit().await?;
    let mut completion = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.activities.enroll.completion",
    )
    .await?;
    append_audit(
        &mut completion,
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
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.activities.unenroll",
    )
    .await?;
    let (context, term) = lock_registration_context(&mut transaction, group_id).await?;
    require_registration_window(&context, &query)?;
    let membership: (Uuid, Uuid) = DbSession::new(&mut *transaction)
        .query_as(
            "academic_delivery.services.activities.unenroll",
            r#"SELECT id, student_academic_year_id
           FROM learning_group_students
           WHERE learning_group_id = $1
             AND student_id = $2
             AND membership_status = 'active'
           FOR UPDATE"#,
        )
        .bind(group_id)
        .bind(student_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบการลงทะเบียนกิจกรรมที่ต้องการยกเลิก".to_string()))?;
    let left_at = registration_date(&term);
    DbSession::new(&mut *transaction)
        .query(
            "academic_delivery.services.activities.unenroll",
            r#"UPDATE learning_group_students
           SET membership_status = 'removed',
               left_at = GREATEST(joined_at, $1),
               row_version = row_version + 1,
               updated_at = now()
           WHERE id = $2"#,
        )
        .bind(left_at)
        .bind(membership.0)
        .execute()
        .await?;
    let revision = increment_group_revision(&mut transaction, group_id).await?;
    invalidate_group_academic_confirmations(&mut transaction, &[group_id]).await?;
    transaction.commit().await?;
    let mut completion = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.activities.unenroll.completion",
    )
    .await?;
    append_audit(
        &mut completion,
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

#[derive(Debug, sqlx::FromRow)]
struct StudentActivityRegistrationRow {
    homeroom_id: Option<Uuid>,
    registration_date: chrono::NaiveDate,
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
) -> Result<(RegistrationLockRow, TermContext), AppError> {
    let (offering_id, academic_term_id): (Uuid, Uuid) = DbSession::new(&mut **transaction)
        .query_as(
            "academic_delivery.services.activities.lock_registration_context",
            "SELECT learning_offering_id, academic_term_id FROM learning_groups WHERE id = $1",
        )
        .bind(group_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string()))?;
    let term = require_writable_term(transaction, academic_term_id, false).await?;
    DbSession::new(&mut **transaction)
        .query(
            "academic_delivery.services.activities.lock_registration_context",
            "SELECT id FROM learning_offerings WHERE id = $1 FOR UPDATE",
        )
        .bind(offering_id)
        .execute()
        .await?;
    let context = DbSession::new(&mut **transaction)
        .query_as(
            "academic_delivery.services.activities.lock_registration_context",
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
             AND learning_group.academic_term_id = $2
             AND offering.id = $3
           FOR UPDATE OF learning_group"#,
        )
        .bind(group_id)
        .bind(academic_term_id)
        .bind(offering_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string()))?;
    Ok((context, term))
}

fn require_registration_window(
    context: &RegistrationLockRow,
    query: &StudentActivityRegistrationQuery,
) -> Result<(), AppError> {
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
    Ok(())
}

async fn require_eligible_student(
    transaction: &mut Transaction<'_, Postgres>,
    student_id: Uuid,
    context: &RegistrationLockRow,
    term: &TermContext,
) -> Result<EligibleStudentRow, AppError> {
    let learner: EligibleStudentRow = DbSession::new(&mut **transaction)
        .query_as(
            "academic_delivery.services.activities.require_eligible_student",
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
        .bind(term.date_upper_bound())
        .bind(context.academic_year_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::Forbidden("นักเรียนไม่มีสิทธิ์ลงทะเบียนกิจกรรมในปีการศึกษานี้".to_string()))?;

    let target_allowed: bool = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.activities.require_eligible_student",
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
        .fetch_one()
        .await?;
    let coverage_allowed: bool = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.activities.require_eligible_student",
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
        .fetch_one()
        .await?;
    let constraint = super::audiences::dated_constraint_on_connection(
        &mut **transaction,
        context.learning_offering_id,
        registration_date(term),
    )
    .await?;
    let audience_allowed = constraint
        .as_ref()
        .is_none_or(|scope| scope.allows_homeroom(context.learning_group_id, learner.homeroom_id));
    if !target_allowed || !coverage_allowed || !audience_allowed {
        return Err(AppError::Forbidden(
            "กิจกรรมนี้ไม่ได้เปิดสำหรับระดับชั้น แผนการเรียน หรือห้องเรียนของนักเรียน".to_string(),
        ));
    }
    Ok(learner)
}

fn registration_date(term: &TermContext) -> chrono::NaiveDate {
    bounded_registration_date(bangkok_today(), term.start_date, term.date_upper_bound())
}

fn bounded_registration_date(
    today: chrono::NaiveDate,
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> chrono::NaiveDate {
    today.max(start).min(end)
}

async fn increment_group_revision(
    transaction: &mut Transaction<'_, Postgres>,
    group_id: Uuid,
) -> Result<i64, AppError> {
    let revision = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.activities.increment_group_revision",
            "UPDATE learning_groups SET row_version = row_version + 1, updated_at = now() \
         WHERE id = $1 RETURNING row_version",
        )
        .bind(group_id)
        .fetch_one()
        .await?;
    Ok(revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};

    #[test]
    fn registration_dates_use_bangkok_midnight_and_preserve_term_bounds() {
        let july_1 = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let july_2 = NaiveDate::from_ymd_opt(2026, 7, 2).unwrap();
        let july_3 = NaiveDate::from_ymd_opt(2026, 7, 3).unwrap();
        let before_midnight =
            super::super::bangkok_date_at(Utc.with_ymd_and_hms(2026, 7, 1, 16, 59, 59).unwrap());
        let midnight =
            super::super::bangkok_date_at(Utc.with_ymd_and_hms(2026, 7, 1, 17, 0, 0).unwrap());
        assert_eq!(
            bounded_registration_date(before_midnight, july_1, july_3),
            july_1
        );
        assert_eq!(bounded_registration_date(midnight, july_1, july_3), july_2);
        assert_eq!(
            bounded_registration_date(before_midnight, july_2, july_3),
            july_2
        );
        let after_term =
            super::super::bangkok_date_at(Utc.with_ymd_and_hms(2026, 7, 3, 17, 0, 0).unwrap());
        assert_eq!(
            bounded_registration_date(after_term, july_1, july_3),
            july_3
        );
    }
}
