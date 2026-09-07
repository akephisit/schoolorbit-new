use super::*;
use crate::{
    middleware::permission::ActorContext, policies::academic_result_access_policy as access_policy,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct CourseGroupRow {
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    group_name: String,
    offering_name: String,
    assigned: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CourseLockStudentSnapshot {
    student_academic_year_id: Uuid,
    calculated_score: String,
    calculated_grade: String,
    selection: CourseOutcomeSelection,
    numeric_grade: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CourseLockGroupSnapshot {
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    roster_checksum: String,
    source_checksum: String,
    confirmation_id: Uuid,
    confirmation_row_version: i64,
    students: Vec<CourseLockStudentSnapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CourseLockSourceSnapshot {
    groups: Vec<CourseLockGroupSnapshot>,
}

struct InitialCourseResult {
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    student_academic_year_id: Uuid,
    calculated_score: BigDecimal,
    calculated_grade: BigDecimal,
    outcome: &'static str,
    numeric_grade: Option<BigDecimal>,
    selection_source: &'static str,
}

fn missing_group_confirmation() -> ResultBlocker {
    ResultBlocker {
        code: ResultBlockerCode::MissingGroupConfirmation,
        assessment_phase_id: None,
        student_academic_year_id: None,
    }
}

fn stale_group_confirmation() -> ResultBlocker {
    ResultBlocker {
        code: ResultBlockerCode::StaleGroupConfirmation,
        assessment_phase_id: None,
        student_academic_year_id: None,
    }
}

pub(crate) async fn require_course_offering_unlocked(
    tx: &mut Transaction<'_, Postgres>,
    learning_offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
) -> Result<(), AppError> {
    let locked: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1
               FROM course_offering_details detail
               JOIN academic_course_result_locks result_lock
                 ON result_lock.subject_id=detail.subject_id
                AND result_lock.academic_term_id=detail.academic_term_id
                AND result_lock.academic_year_id=detail.academic_year_id
               WHERE detail.learning_offering_id=$1
                 AND detail.academic_term_id=$2
                 AND detail.academic_year_id=$3
           )"#,
    )
    .bind(learning_offering_id)
    .bind(academic_term_id)
    .bind(academic_year_id)
    .fetch_one(&mut **tx)
    .await?;
    if locked {
        return Err(AppError::Conflict(
            "Course results are locked and source data is read-only".into(),
        ));
    }
    Ok(())
}

pub(crate) async fn require_activity_group_unlocked(
    tx: &mut Transaction<'_, Postgres>,
    learning_group_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
) -> Result<(), AppError> {
    let locked: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1 FROM academic_activity_result_locks
               WHERE learning_group_id=$1 AND academic_term_id=$2 AND academic_year_id=$3
           )"#,
    )
    .bind(learning_group_id)
    .bind(academic_term_id)
    .bind(academic_year_id)
    .fetch_one(&mut **tx)
    .await?;
    if locked {
        return Err(AppError::Conflict(
            "Activity result is locked and source data is read-only".into(),
        ));
    }
    Ok(())
}

pub async fn lock_course_subject(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    subject_id: Uuid,
) -> Result<CourseResultLockOutcome, AppError> {
    if !access_policy::can_lock(actor) {
        return Err(AppError::Forbidden(
            "School academic-result lock permission is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;

    let offering_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT offering.id
           FROM learning_offerings offering
           JOIN course_offering_details detail ON detail.learning_offering_id=offering.id
           WHERE detail.subject_id=$1
             AND offering.academic_term_id=$2
             AND offering.academic_year_id=$3
           ORDER BY offering.id
           FOR UPDATE OF offering"#,
    )
    .bind(subject_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .fetch_all(&mut *tx)
    .await?;
    if offering_ids.is_empty() {
        return Err(AppError::NotFound(
            "Course subject not found in this academic context".into(),
        ));
    }

    let groups: Vec<CourseGroupRow> = sqlx::query_as(
        r#"SELECT learning_group.id AS learning_group_id,
                  learning_group.learning_offering_id,
                  learning_group.name AS group_name,
                  offering.name_snapshot AS offering_name,
                  EXISTS(
                      SELECT 1
                      FROM learning_group_teachers teacher
                      JOIN users user_account ON user_account.id=teacher.teacher_id
                                             AND user_account.status='active'
                      JOIN academic_terms term ON term.id=learning_group.academic_term_id
                      WHERE teacher.learning_group_id=learning_group.id
                        AND teacher.teacher_id=$2
                        AND teacher.starts_on<=LEAST(GREATEST(current_date,term.start_date),term.planned_end_date)
                        AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,term.start_date),term.planned_end_date))
                  ) AS assigned
           FROM learning_groups learning_group
           JOIN learning_offerings offering ON offering.id=learning_group.learning_offering_id
           WHERE learning_group.learning_offering_id=ANY($1)
             AND learning_group.status<>'closed'
           ORDER BY learning_group.id
           FOR UPDATE OF learning_group"#,
    )
    .bind(&offering_ids)
    .bind(actor.user_id)
    .fetch_all(&mut *tx)
    .await?;
    if groups.is_empty() {
        return Err(AppError::ValidationError(
            "Course subject has no active learning groups to lock".into(),
        ));
    }
    if groups.len() > 500 {
        return Err(AppError::ValidationError(
            "Course subject exceeds 500 active learning groups".into(),
        ));
    }

    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM academic_course_result_locks WHERE subject_id=$1 AND academic_term_id=$2 AND academic_year_id=$3)",
    )
    .bind(subject_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .fetch_one(&mut *tx)
    .await?
    {
        return Err(AppError::Conflict(
            "Course results are already locked".into(),
        ));
    }

    // Offering/group locks were acquired first, matching every result-source writer. Holding the
    // current policy row prevents a concurrent activation from changing the effective policy
    // between source revalidation and the immutable snapshot insert.
    sqlx::query(
        "SELECT id FROM academic_grading_policy_versions WHERE lifecycle='active' FOR SHARE",
    )
    .execute(&mut *tx)
    .await?;

    let mut readiness = Vec::with_capacity(groups.len());
    let mut snapshots = Vec::with_capacity(groups.len());
    let mut initial_results = Vec::new();
    let mut seen_students = BTreeSet::new();
    let mut policy = None;
    for group in groups {
        let workspace = course_preparation::workspace_for_lock(
            &mut tx,
            actor,
            context,
            group.learning_group_id,
        )
        .await?;
        let mut blockers = workspace.blockers.clone();
        if workspace.confirmation.is_none() {
            blockers.push(missing_group_confirmation());
        } else if !workspace.confirmation_is_current {
            blockers.push(stale_group_confirmation());
        }
        let ready = blockers.is_empty();
        readiness.push(GroupResultReadiness {
            learning_group_id: group.learning_group_id,
            learning_offering_id: group.learning_offering_id,
            subject_id: Some(subject_id),
            group_name: group.group_name,
            offering_name: group.offering_name,
            assigned: group.assigned,
            ready,
            locked: false,
            blockers,
        });
        if !ready {
            continue;
        }

        if policy
            .as_ref()
            .is_some_and(|saved: &GradingPolicyVersion| saved.id != workspace.policy.id)
        {
            return Err(AppError::Conflict(
                "Course groups no longer use one grading policy version".into(),
            ));
        }
        policy.get_or_insert_with(|| workspace.policy.clone());
        let confirmation = workspace.confirmation.as_ref().ok_or_else(|| {
            AppError::InternalServerError("Ready course group has no confirmation".into())
        })?;
        let mut student_snapshots = Vec::with_capacity(workspace.students.len());
        for student in workspace.students {
            if !seen_students.insert(student.student_academic_year_id) {
                return Err(AppError::ValidationError(
                    "A student belongs to more than one active group for this course subject"
                        .into(),
                ));
            }
            let calculated_grade = student.calculated_grade.clone().ok_or_else(|| {
                AppError::ValidationError(
                    "A ready course result must have a calculated grade".into(),
                )
            })?;
            let (outcome, numeric_grade) = match student.selection {
                CourseOutcomeSelection::Derived | CourseOutcomeSelection::ExplicitZero => (
                    "numeric",
                    Some(decimal(student.numeric_grade.as_deref().ok_or_else(
                        || {
                            AppError::ValidationError(
                                "A numeric course result must include its grade".into(),
                            )
                        },
                    )?)?),
                ),
                CourseOutcomeSelection::Incomplete => ("incomplete", None),
                CourseOutcomeSelection::InsufficientAttendance => ("insufficient_attendance", None),
            };
            initial_results.push(InitialCourseResult {
                learning_group_id: group.learning_group_id,
                learning_offering_id: group.learning_offering_id,
                student_academic_year_id: student.student_academic_year_id,
                calculated_score: decimal(&student.calculated_score)?,
                calculated_grade: decimal(&calculated_grade)?,
                outcome,
                numeric_grade,
                selection_source: student.selection.as_str(),
            });
            student_snapshots.push(CourseLockStudentSnapshot {
                student_academic_year_id: student.student_academic_year_id,
                calculated_score: student.calculated_score,
                calculated_grade,
                selection: student.selection,
                numeric_grade: student.numeric_grade,
            });
        }
        snapshots.push(CourseLockGroupSnapshot {
            learning_group_id: group.learning_group_id,
            learning_offering_id: group.learning_offering_id,
            roster_checksum: workspace.roster_checksum,
            source_checksum: workspace.source_checksum,
            confirmation_id: confirmation.id,
            confirmation_row_version: confirmation.row_version,
            students: student_snapshots,
        });
    }

    if readiness.iter().any(|group| !group.ready) {
        tx.commit().await?;
        return Ok(CourseResultLockOutcome {
            lock: None,
            groups: readiness,
        });
    }

    let policy = policy.ok_or_else(|| {
        AppError::ValidationError("Course subject has no active grading policy".into())
    })?;
    let roster_checksum = hash(
        &snapshots
            .iter()
            .map(|group| (group.learning_group_id, &group.roster_checksum))
            .collect::<Vec<_>>(),
    )?;
    let source_checksum = hash(
        &snapshots
            .iter()
            .map(|group| {
                (
                    group.learning_group_id,
                    &group.source_checksum,
                    group.confirmation_id,
                    group.confirmation_row_version,
                )
            })
            .collect::<Vec<_>>(),
    )?;
    let source_snapshot = CourseLockSourceSnapshot { groups: snapshots };
    let (id, row_version, locked_at): (Uuid, i64, DateTime<Utc>) = sqlx::query_as(
        r#"INSERT INTO academic_course_result_locks (
               subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,
               roster_checksum,source_checksum,source_snapshot,locked_by
           ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
           RETURNING id,row_version,locked_at"#,
    )
    .bind(subject_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(policy.id)
    .bind(sqlx::types::Json(&policy))
    .bind(&roster_checksum)
    .bind(&source_checksum)
    .bind(sqlx::types::Json(&source_snapshot))
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await?;

    if !initial_results.is_empty() {
        let learning_group_ids = initial_results
            .iter()
            .map(|result| result.learning_group_id)
            .collect::<Vec<_>>();
        let learning_offering_ids = initial_results
            .iter()
            .map(|result| result.learning_offering_id)
            .collect::<Vec<_>>();
        let student_ids = initial_results
            .iter()
            .map(|result| result.student_academic_year_id)
            .collect::<Vec<_>>();
        let calculated_scores = initial_results
            .iter()
            .map(|result| result.calculated_score.clone())
            .collect::<Vec<_>>();
        let calculated_grades = initial_results
            .iter()
            .map(|result| result.calculated_grade.clone())
            .collect::<Vec<_>>();
        let outcomes = initial_results
            .iter()
            .map(|result| result.outcome)
            .collect::<Vec<_>>();
        let numeric_grades = initial_results
            .iter()
            .map(|result| result.numeric_grade.clone())
            .collect::<Vec<_>>();
        let selection_sources = initial_results
            .iter()
            .map(|result| result.selection_source)
            .collect::<Vec<_>>();
        sqlx::query(
            r#"INSERT INTO academic_course_results (
                   learning_group_id,learning_offering_id,academic_term_id,academic_year_id,
                   subject_id,course_result_lock_id,student_academic_year_id,calculated_score,
                   calculated_grade,outcome,numeric_grade,selection_source
               )
               SELECT value.learning_group_id,value.learning_offering_id,$1,$2,$3,$4,
                      value.student_academic_year_id,value.calculated_score,
                      value.calculated_grade,value.outcome,value.numeric_grade,
                      value.selection_source
               FROM unnest(
                   $5::uuid[],$6::uuid[],$7::uuid[],$8::numeric[],$9::numeric[],
                   $10::text[],$11::numeric[],$12::text[]
               ) AS value(
                   learning_group_id,learning_offering_id,student_academic_year_id,
                   calculated_score,calculated_grade,outcome,numeric_grade,selection_source
               )"#,
        )
        .bind(context.academic_term_id)
        .bind(context.academic_year_id)
        .bind(subject_id)
        .bind(id)
        .bind(learning_group_ids)
        .bind(learning_offering_ids)
        .bind(student_ids)
        .bind(calculated_scores)
        .bind(calculated_grades)
        .bind(outcomes)
        .bind(numeric_grades)
        .bind(selection_sources)
        .execute(&mut *tx)
        .await?;
    }

    let result_count = u32::try_from(initial_results.len()).map_err(|_| {
        AppError::InternalServerError("Course result count exceeds the API limit".into())
    })?;
    tx.commit().await?;
    Ok(CourseResultLockOutcome {
        lock: Some(CourseResultLock {
            id,
            subject_id,
            academic_year_id: context.academic_year_id,
            academic_term_id: context.academic_term_id,
            policy_version_id: policy.id,
            roster_checksum,
            source_checksum,
            row_version,
            locked_by: actor.user_id,
            locked_at,
            result_count,
        }),
        groups: readiness,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityLockValueSnapshot {
    student_academic_year_id: Uuid,
    outcome: ActivityOutcome,
    row_version: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityLockSourceSnapshot {
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    confirmation_id: Uuid,
    confirmation_row_version: i64,
    roster_checksum: String,
    source_checksum: String,
    outcomes: Vec<ActivityLockValueSnapshot>,
}

pub async fn lock_activity_group(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    group_id: Uuid,
) -> Result<ActivityResultLockOutcome, AppError> {
    if !access_policy::can_lock(actor) {
        return Err(AppError::Forbidden(
            "School academic-result lock permission is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    let offering_id: Uuid = sqlx::query_scalar(
        r#"SELECT offering.id
           FROM learning_groups learning_group
           JOIN learning_offerings offering
             ON offering.id=learning_group.learning_offering_id
            AND offering.kind='activity'
           WHERE learning_group.id=$1
             AND learning_group.academic_term_id=$2
             AND learning_group.academic_year_id=$3
             AND learning_group.status<>'closed'
           FOR UPDATE OF offering"#,
    )
    .bind(group_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Activity group not found in context".into()))?;
    sqlx::query("SELECT id FROM learning_groups WHERE id=$1 FOR UPDATE")
        .bind(group_id)
        .execute(&mut *tx)
        .await?;

    let workspace = activities::workspace_for_lock(&mut tx, actor, context, group_id).await?;
    let mut blockers = workspace.blockers.clone();
    if workspace.confirmation.is_none() {
        blockers.push(missing_group_confirmation());
    } else if !workspace.confirmation_is_current {
        blockers.push(stale_group_confirmation());
    }
    if !blockers.is_empty() {
        tx.commit().await?;
        return Ok(ActivityResultLockOutcome {
            lock: None,
            blockers,
        });
    }

    let confirmation = workspace.confirmation.as_ref().ok_or_else(|| {
        AppError::InternalServerError("Ready activity group has no confirmation".into())
    })?;
    let outcomes = workspace
        .students
        .iter()
        .map(|student| {
            Ok(ActivityLockValueSnapshot {
                student_academic_year_id: student.student_academic_year_id,
                outcome: student.outcome.ok_or_else(|| {
                    AppError::ValidationError(
                        "Activity outcomes must be complete before locking".into(),
                    )
                })?,
                row_version: student.row_version.ok_or_else(|| {
                    AppError::ValidationError(
                        "Activity outcomes must be versioned before locking".into(),
                    )
                })?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let source_snapshot = ActivityLockSourceSnapshot {
        learning_group_id: group_id,
        learning_offering_id: offering_id,
        confirmation_id: confirmation.id,
        confirmation_row_version: confirmation.row_version,
        roster_checksum: workspace.roster_checksum.clone(),
        source_checksum: workspace.source_checksum.clone(),
        outcomes,
    };
    let (id, row_version, locked_at): (Uuid, i64, DateTime<Utc>) = sqlx::query_as(
        r#"INSERT INTO academic_activity_result_locks (
               learning_group_id,learning_offering_id,academic_term_id,academic_year_id,
               roster_checksum,source_checksum,source_snapshot,locked_by
           ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
           RETURNING id,row_version,locked_at"#,
    )
    .bind(group_id)
    .bind(offering_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(&workspace.roster_checksum)
    .bind(&workspace.source_checksum)
    .bind(sqlx::types::Json(&source_snapshot))
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await?;
    if !source_snapshot.outcomes.is_empty() {
        let student_ids = source_snapshot
            .outcomes
            .iter()
            .map(|value| value.student_academic_year_id)
            .collect::<Vec<_>>();
        let outcomes = source_snapshot
            .outcomes
            .iter()
            .map(|value| value.outcome.as_str())
            .collect::<Vec<_>>();
        sqlx::query(
            r#"INSERT INTO academic_activity_results (
                   learning_group_id,learning_offering_id,academic_term_id,academic_year_id,
                   activity_result_lock_id,student_academic_year_id,outcome
               )
               SELECT $1,$2,$3,$4,$5,value.student_academic_year_id,value.outcome
               FROM unnest($6::uuid[],$7::text[])
                    AS value(student_academic_year_id,outcome)"#,
        )
        .bind(group_id)
        .bind(offering_id)
        .bind(context.academic_term_id)
        .bind(context.academic_year_id)
        .bind(id)
        .bind(student_ids)
        .bind(outcomes)
        .execute(&mut *tx)
        .await?;
    }
    let result_count = u32::try_from(source_snapshot.outcomes.len()).map_err(|_| {
        AppError::InternalServerError("Activity result count exceeds the API limit".into())
    })?;
    tx.commit().await?;
    Ok(ActivityResultLockOutcome {
        lock: Some(ActivityResultLock {
            id,
            learning_group_id: group_id,
            learning_offering_id: offering_id,
            academic_year_id: context.academic_year_id,
            academic_term_id: context.academic_term_id,
            roster_checksum: workspace.roster_checksum,
            source_checksum: workspace.source_checksum,
            row_version,
            locked_by: actor.user_id,
            locked_at,
            result_count,
        }),
        blockers: Vec::new(),
    })
}

pub async fn lock_all_ready_activities(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
) -> Result<BulkActivityResultLockOutcome, AppError> {
    if !access_policy::can_lock(actor) {
        return Err(AppError::Forbidden(
            "School academic-result lock permission is required".into(),
        ));
    }
    let queue = readiness::readiness(pool, actor, context).await?;
    let mut locked = Vec::new();
    let mut skipped = Vec::new();
    for mut group in queue.activities {
        if !group.ready {
            skipped.push(group);
            continue;
        }
        let outcome = lock_activity_group(pool, actor, context, group.learning_group_id).await?;
        if let Some(lock) = outcome.lock {
            locked.push(lock);
        } else {
            group.ready = false;
            group.blockers = outcome.blockers;
            skipped.push(group);
        }
    }
    Ok(BulkActivityResultLockOutcome { locked, skipped })
}
