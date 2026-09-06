use super::*;
use crate::{
    middleware::permission::ActorContext, policies::academic_result_access_policy as access_policy,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct ActivityScope {
    group_id: Uuid,
    offering_id: Uuid,
    owning_organization_unit_id: Option<Uuid>,
    assigned: bool,
    primary_teacher_id: Option<Uuid>,
    locked: bool,
}

#[derive(sqlx::FromRow)]
struct ActivityStudentRow {
    membership_id: Uuid,
    student_academic_year_id: Uuid,
    display_name: String,
    row_version: i64,
}

#[derive(sqlx::FromRow)]
struct ActivityValueRow {
    student_academic_year_id: Uuid,
    outcome: String,
    row_version: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityRosterRevision {
    membership_id: Uuid,
    student_academic_year_id: Uuid,
    row_version: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityValueRevision {
    student_academic_year_id: Uuid,
    outcome: ActivityOutcome,
    row_version: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityConfirmationSnapshot<'a> {
    invalidated: bool,
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    source_checksum: &'a str,
    roster_checksum: &'a str,
    roster: Vec<ActivityRosterRevision>,
    outcomes: Vec<ActivityValueRevision>,
}

async fn begin_activity<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    group: Uuid,
    context: &ResultContext,
    write: bool,
) -> Result<(Transaction<'a, Postgres>, ActivityScope), AppError> {
    let access = access_policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    if write {
        let offering: Uuid = sqlx::query_scalar(
            "SELECT learning_offering_id FROM learning_groups WHERE id=$1 AND academic_term_id=$2 AND academic_year_id=$3",
        )
        .bind(group)
        .bind(context.academic_term_id)
        .bind(context.academic_year_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("Activity group not found in context".into()))?;
        sqlx::query("SELECT id FROM learning_offerings WHERE id=$1 FOR UPDATE")
            .bind(offering)
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT id FROM learning_groups WHERE id=$1 FOR UPDATE")
            .bind(group)
            .execute(&mut *tx)
            .await?;
    }
    let scope: Option<ActivityScope> = sqlx::query_as(
        r#"SELECT g.id AS group_id,g.learning_offering_id AS offering_id,o.owning_organization_unit_id,
                  EXISTS(SELECT 1 FROM learning_group_teachers teacher JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                         WHERE teacher.learning_group_id=g.id AND teacher.academic_term_id=g.academic_term_id
                           AND teacher.academic_year_id=g.academic_year_id AND teacher.teacher_id=$4
                           AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                           AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS assigned,
                  (SELECT teacher.teacher_id FROM learning_group_teachers teacher
                     JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                    WHERE teacher.learning_group_id=g.id AND teacher.academic_term_id=g.academic_term_id
                      AND teacher.academic_year_id=g.academic_year_id AND teacher.role='primary'
                      AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                      AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))
                    ORDER BY teacher.id LIMIT 1) AS primary_teacher_id,
                  lock.id IS NOT NULL AS locked
           FROM learning_groups g
           JOIN learning_offerings o ON o.id=g.learning_offering_id AND o.kind='activity'
           JOIN activity_offering_details detail ON detail.learning_offering_id=o.id
           JOIN academic_terms t ON t.id=g.academic_term_id
           LEFT JOIN academic_activity_result_locks lock ON lock.learning_group_id=g.id
           WHERE g.id=$1 AND g.academic_term_id=$2 AND g.academic_year_id=$3 AND g.status<>'closed'"#,
    )
    .bind(group)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(actor.user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let scope =
        scope.ok_or_else(|| AppError::NotFound("Activity group not found in context".into()))?;
    if !access_policy::can_read_group(&access, scope.owning_organization_unit_id, scope.assigned) {
        return Err(AppError::Forbidden("Activity result access denied".into()));
    }
    if write {
        if !access_policy::can_manage_group(actor, scope.assigned) {
            return Err(AppError::Forbidden(
                "Current activity assignment is required".into(),
            ));
        }
        if scope.locked {
            return Err(AppError::Conflict(
                "Activity result is locked and preparation is read-only".into(),
            ));
        }
        sqlx::query("SELECT id FROM academic_activity_evaluations WHERE learning_group_id=$1 ORDER BY id FOR UPDATE")
            .bind(group)
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT id FROM academic_activity_result_confirmations WHERE learning_group_id=$1 FOR UPDATE")
            .bind(group)
            .execute(&mut *tx)
            .await?;
    }
    Ok((tx, scope))
}

fn blocker(code: ResultBlockerCode, student: Option<Uuid>) -> ResultBlocker {
    ResultBlocker {
        code,
        assessment_phase_id: None,
        student_academic_year_id: student,
    }
}

async fn invalidate_confirmation(
    tx: &mut Transaction<'_, Postgres>,
    group: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE academic_activity_result_confirmations SET row_version=row_version+1,source_snapshot=jsonb_set(source_snapshot,'{invalidated}','true'::jsonb) WHERE learning_group_id=$1",
    )
    .bind(group)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn load_workspace(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    scope: &ActivityScope,
) -> Result<(ActivityPreparationWorkspace, Vec<ActivityRosterRevision>), AppError> {
    let students: Vec<ActivityStudentRow> = sqlx::query_as(
        "SELECT m.id AS membership_id,m.student_academic_year_id,concat_ws(' ',u.first_name,u.last_name) AS display_name,m.row_version FROM learning_group_students m JOIN users u ON u.id=m.student_id WHERE m.learning_group_id=$1 AND m.membership_status='active' ORDER BY m.student_academic_year_id LIMIT 2001",
    )
    .bind(scope.group_id)
    .fetch_all(&mut **tx)
    .await?;
    let values: Vec<ActivityValueRow> = sqlx::query_as(
        "SELECT student_academic_year_id,outcome,row_version FROM academic_activity_evaluations WHERE learning_group_id=$1 ORDER BY student_academic_year_id LIMIT 2001",
    )
    .bind(scope.group_id)
    .fetch_all(&mut **tx)
    .await?;
    if students.len() > 2000 || values.len() > 2000 {
        return Err(AppError::ValidationError(
            "Activity preparation workspace exceeds supported size".into(),
        ));
    }
    let roster = students
        .iter()
        .map(|student| ActivityRosterRevision {
            membership_id: student.membership_id,
            student_academic_year_id: student.student_academic_year_id,
            row_version: student.row_version,
        })
        .collect::<Vec<_>>();
    let roster_checksum = hash(&roster)?;
    let active = students
        .iter()
        .map(|student| student.student_academic_year_id)
        .collect::<std::collections::BTreeSet<_>>();
    let outcome_revisions = values
        .iter()
        .filter(|value| active.contains(&value.student_academic_year_id))
        .map(|value| {
            Ok(ActivityValueRevision {
                student_academic_year_id: value.student_academic_year_id,
                outcome: ActivityOutcome::try_from(value.outcome.as_str()).map_err(|_| {
                    AppError::ValidationError("Stored activity outcome is invalid".into())
                })?,
                row_version: value.row_version,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let source_checksum = hash(&(scope.group_id, &roster_checksum, &outcome_revisions))?;
    let value_map = values
        .iter()
        .map(|value| (value.student_academic_year_id, value))
        .collect::<BTreeMap<_, _>>();
    let prepared = students
        .iter()
        .map(|student| {
            let value = value_map.get(&student.student_academic_year_id);
            Ok(PreparedActivityStudent {
                student_academic_year_id: student.student_academic_year_id,
                display_name: student.display_name.clone(),
                outcome: value
                    .map(|value| ActivityOutcome::try_from(value.outcome.as_str()))
                    .transpose()
                    .map_err(|_| {
                        AppError::ValidationError("Stored activity outcome is invalid".into())
                    })?,
                row_version: value.map(|value| value.row_version),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let mut blockers = prepared
        .iter()
        .filter(|student| student.outcome.is_none())
        .map(|student| {
            blocker(
                ResultBlockerCode::MissingActivityOutcome,
                Some(student.student_academic_year_id),
            )
        })
        .collect::<Vec<_>>();
    if scope.primary_teacher_id.is_none() {
        blockers.push(blocker(ResultBlockerCode::MissingPrimaryTeacher, None));
    }
    if scope.locked {
        blockers.push(blocker(ResultBlockerCode::AlreadyLocked, None));
    }
    let mut confirmation: Option<ResultConfirmation> = sqlx::query_as(
        "SELECT id,row_version,source_checksum,roster_checksum,confirmed_by,COALESCE((source_snapshot->>'invalidated')::boolean,false) AS invalidated FROM academic_activity_result_confirmations WHERE learning_group_id=$1",
    )
    .bind(scope.group_id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(saved) = confirmation.as_mut() {
        if !scope.locked
            && !saved.invalidated
            && (saved.source_checksum != source_checksum
                || saved.roster_checksum != roster_checksum
                || Some(saved.confirmed_by) != scope.primary_teacher_id)
        {
            invalidate_confirmation(tx, scope.group_id).await?;
            saved.invalidated = true;
            saved.row_version += 1;
        }
    }
    let confirmation_is_current = confirmation.as_ref().is_some_and(|saved| {
        !saved.invalidated
            && saved.source_checksum == source_checksum
            && saved.roster_checksum == roster_checksum
            && Some(saved.confirmed_by) == scope.primary_teacher_id
            && !blockers.iter().any(|item| {
                matches!(
                    item.code,
                    ResultBlockerCode::MissingActivityOutcome
                        | ResultBlockerCode::MissingPrimaryTeacher
                )
            })
    });
    let editable = !scope.locked && access_policy::can_enter_activity(actor, scope.assigned);
    Ok((
        ActivityPreparationWorkspace {
            learning_group_id: scope.group_id,
            students: prepared,
            source_checksum,
            roster_checksum,
            confirmation,
            confirmation_is_current,
            blockers,
            locked: scope.locked,
            can_manage: editable,
            can_confirm: editable
                && access_policy::can_confirm_activity(
                    actor,
                    scope.primary_teacher_id == Some(actor.user_id),
                ),
        },
        roster,
    ))
}

pub async fn get_activity_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    group: Uuid,
) -> Result<ActivityPreparationWorkspace, AppError> {
    let (mut tx, scope) = begin_activity(pool, actor, group, context, false).await?;
    let (workspace, _) = load_workspace(&mut tx, actor, &scope).await?;
    tx.commit().await?;
    Ok(workspace)
}

fn normalize(
    cells: Vec<ActivityCellInput>,
) -> Result<BTreeMap<Uuid, (Option<ActivityOutcome>, Option<i64>)>, AppError> {
    if cells.is_empty() || cells.len() > 500 {
        return Err(AppError::ValidationError(
            "Activity batch must contain 1–500 cells".into(),
        ));
    }
    let mut normalized = BTreeMap::new();
    for cell in cells {
        let value = (cell.outcome, cell.row_version);
        if let Some(previous) = normalized.insert(cell.student_academic_year_id, value) {
            if previous != value {
                return Err(AppError::ValidationError(
                    "Activity batch contains conflicting duplicate cells".into(),
                ));
            }
        }
    }
    Ok(normalized)
}

pub async fn save_activity_outcomes(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    group: Uuid,
    input: ActivityBatchInput,
) -> Result<ActivityPreparationWorkspace, AppError> {
    let cells = normalize(input.cells)?;
    let (mut tx, scope) = begin_activity(pool, actor, group, context, true).await?;
    if !access_policy::can_enter_activity(actor, scope.assigned) {
        return Err(AppError::Forbidden(
            "Current activity assignment is required for outcome entry".into(),
        ));
    }
    let (workspace, _) = load_workspace(&mut tx, actor, &scope).await?;
    for (student_id, (_, version)) in &cells {
        let student = workspace
            .students
            .iter()
            .find(|student| student.student_academic_year_id == *student_id)
            .ok_or_else(|| {
                AppError::ValidationError("Student is not a current activity participant".into())
            })?;
        check_version(*version, student.row_version)?;
    }
    let revision: i64 = sqlx::query_scalar(
        "UPDATE learning_groups SET row_version=row_version+1,updated_at=now() WHERE id=$1 RETURNING row_version",
    )
    .bind(group)
    .fetch_one(&mut *tx)
    .await?;
    let (mut set_students, mut outcomes, mut clear_students) = (Vec::new(), Vec::new(), Vec::new());
    for (student, (outcome, _)) in cells {
        if let Some(outcome) = outcome {
            set_students.push(student);
            outcomes.push(outcome.as_str().to_string());
        } else {
            clear_students.push(student);
        }
    }
    if !set_students.is_empty() {
        sqlx::query(
            "INSERT INTO academic_activity_evaluations (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,student_academic_year_id,outcome,row_version,updated_by) SELECT $1,$2,$3,$4,c.student,c.outcome,$7,$8 FROM unnest($5::uuid[],$6::text[]) c(student,outcome) ON CONFLICT (learning_group_id,student_academic_year_id) DO UPDATE SET outcome=EXCLUDED.outcome,row_version=EXCLUDED.row_version,updated_by=EXCLUDED.updated_by,updated_at=now()",
        )
        .bind(group)
        .bind(scope.offering_id)
        .bind(context.academic_term_id)
        .bind(context.academic_year_id)
        .bind(set_students)
        .bind(outcomes)
        .bind(revision)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await?;
    }
    if !clear_students.is_empty() {
        sqlx::query(
            "DELETE FROM academic_activity_evaluations WHERE learning_group_id=$1 AND student_academic_year_id=ANY($2)",
        )
        .bind(group)
        .bind(clear_students)
        .execute(&mut *tx)
        .await?;
    }
    invalidate_confirmation(&mut tx, group).await?;
    let (workspace, _) = load_workspace(&mut tx, actor, &scope).await?;
    tx.commit().await?;
    Ok(workspace)
}

pub async fn confirm_activity(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    group: Uuid,
    input: ResultConfirmationInput,
) -> Result<ActivityPreparationWorkspace, AppError> {
    let (mut tx, scope) = begin_activity(pool, actor, group, context, true).await?;
    if !access_policy::can_confirm_activity(actor, scope.primary_teacher_id == Some(actor.user_id))
    {
        return Err(AppError::Forbidden(
            "Current activity primary teacher is required for confirmation".into(),
        ));
    }
    let (workspace, roster) = load_workspace(&mut tx, actor, &scope).await?;
    if input.source_checksum != workspace.source_checksum
        || input.roster_checksum != workspace.roster_checksum
    {
        return Err(conflict());
    }
    check_version(
        input.row_version,
        workspace
            .confirmation
            .as_ref()
            .map(|saved| saved.row_version),
    )?;
    if workspace.blockers.iter().any(|item| {
        matches!(
            item.code,
            ResultBlockerCode::MissingActivityOutcome | ResultBlockerCode::MissingPrimaryTeacher
        )
    }) {
        tx.commit().await?;
        return Ok(workspace);
    }
    let outcomes = workspace
        .students
        .iter()
        .map(|student| {
            Ok(ActivityValueRevision {
                student_academic_year_id: student.student_academic_year_id,
                outcome: student.outcome.ok_or_else(|| {
                    AppError::ValidationError(
                        "Activity outcomes must be complete before confirmation".into(),
                    )
                })?,
                row_version: student.row_version.ok_or_else(|| {
                    AppError::ValidationError(
                        "Activity outcomes must be complete before confirmation".into(),
                    )
                })?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let snapshot = ActivityConfirmationSnapshot {
        invalidated: false,
        learning_group_id: group,
        learning_offering_id: scope.offering_id,
        source_checksum: &workspace.source_checksum,
        roster_checksum: &workspace.roster_checksum,
        roster,
        outcomes,
    };
    sqlx::query(
        "INSERT INTO academic_activity_result_confirmations (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,roster_checksum,source_checksum,source_snapshot,confirmed_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (learning_group_id) DO UPDATE SET roster_checksum=EXCLUDED.roster_checksum,source_checksum=EXCLUDED.source_checksum,source_snapshot=EXCLUDED.source_snapshot,confirmed_by=EXCLUDED.confirmed_by,confirmed_at=now(),row_version=academic_activity_result_confirmations.row_version+1",
    )
    .bind(group)
    .bind(scope.offering_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(&workspace.roster_checksum)
    .bind(&workspace.source_checksum)
    .bind(sqlx::types::Json(snapshot))
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await?;
    let (workspace, _) = load_workspace(&mut tx, actor, &scope).await?;
    tx.commit().await?;
    Ok(workspace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_batch_rejects_conflicting_duplicates() {
        let student = Uuid::new_v4();
        let pass = ActivityCellInput {
            student_academic_year_id: student,
            outcome: Some(ActivityOutcome::Pass),
            row_version: None,
        };
        assert!(normalize(vec![pass.clone(), pass]).is_ok());
        assert!(normalize(vec![
            ActivityCellInput {
                student_academic_year_id: student,
                outcome: Some(ActivityOutcome::Pass),
                row_version: None,
            },
            ActivityCellInput {
                student_academic_year_id: student,
                outcome: Some(ActivityOutcome::Fail),
                row_version: None,
            },
        ])
        .is_err());
    }
}
