use super::models::*;
use crate::policies::{
    gradebook_access_policy as policy, resource_access_policy::AcademicResourceListFilter,
};
use crate::{error::AppError, middleware::permission::ActorContext};
use bigdecimal::BigDecimal;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

mod confirmations;
mod controls;
mod items;
mod scores;
mod workspace;
pub use confirmations::confirm_phase;
pub use controls::*;
pub use items::*;
pub use scores::save_scores_batch;
pub use workspace::{get_group_phase_workspace, source_checksums};

#[derive(sqlx::FromRow)]
pub(super) struct Scope {
    pub group_id: Uuid,
    pub offering_id: Uuid,
    pub plan_id: Uuid,
    pub phase_id: Uuid,
    pub phase_code: String,
    pub phase_max_score: String,
    pub phase_row_version: i64,
    pub owning_organization_unit_id: Option<Uuid>,
    pub assigned: bool,
    pub primary_teacher: bool,
    pub score_entry_enabled: bool,
    pub locked: bool,
}

pub(super) fn decimal(value: &str) -> Result<BigDecimal, AppError> {
    let value = crate::modules::academic::core::services::validate_canonical_decimal(value, 2)?;
    if value > BigDecimal::from(99_999_999) + BigDecimal::from(99) / BigDecimal::from(100) {
        return Err(AppError::ValidationError(
            "Score exceeds decimal storage range".into(),
        ));
    }
    Ok(value)
}
pub(super) fn conflict() -> AppError {
    AppError::Conflict("Gradebook source changed; refresh before retrying".into())
}
pub(super) fn check_version(expected: Option<i64>, actual: Option<i64>) -> Result<(), AppError> {
    if expected != actual {
        Err(conflict())
    } else {
        Ok(())
    }
}
pub(super) async fn validate_context(
    tx: &mut Transaction<'_, Postgres>,
    context: &GradebookContext,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM academic_terms WHERE id=$1 AND academic_year_id=$2)",
    )
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(AppError::ValidationError(
            "Academic term does not belong to the selected year".into(),
        ));
    }
    Ok(())
}

pub(super) async fn begin_scope<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    context: &GradebookContext,
    write: bool,
    confirm: bool,
) -> Result<(Transaction<'a, Postgres>, Scope), AppError> {
    let access = policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    if !write {
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;
    }
    validate_context(&mut tx, context).await?;
    if write {
        // Shared with Assessment and official result locking: offering, then group, then phase.
        let offering:Option<Uuid>=sqlx::query_scalar("SELECT o.id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id WHERE g.id=$1 AND g.academic_term_id=$2 AND g.academic_year_id=$3 FOR UPDATE OF o").bind(group).bind(context.academic_term_id).bind(context.academic_year_id).fetch_optional(&mut *tx).await?;
        if offering.is_none() {
            return Err(AppError::NotFound(
                "Learning group not found in this context".into(),
            ));
        }
        sqlx::query("SELECT id FROM learning_groups WHERE id=$1 FOR UPDATE")
            .bind(group)
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT id FROM course_assessment_phases WHERE id=$1 FOR SHARE")
            .bind(phase)
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT id FROM academic_gradebook_phase_controls WHERE academic_term_id=$1 ORDER BY id FOR SHARE").bind(context.academic_term_id).execute(&mut *tx).await?;
    }
    let scope:Option<Scope>=sqlx::query_as(r#"SELECT g.id AS group_id,g.learning_offering_id AS offering_id,p.id AS plan_id,phase.id AS phase_id,
        phase.phase_code,phase.max_score::text AS phase_max_score,phase.row_version AS phase_row_version,
        o.owning_organization_unit_id,c.score_entry_enabled,
        EXISTS(SELECT 1 FROM academic_course_result_locks l WHERE l.subject_id=d.subject_id AND l.academic_term_id=g.academic_term_id) AS locked,
        EXISTS(SELECT 1 FROM learning_group_teachers teacher WHERE teacher.learning_group_id=g.id AND teacher.academic_term_id=g.academic_term_id AND teacher.academic_year_id=g.academic_year_id AND teacher.teacher_id=$4
          AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
          AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS assigned,
        EXISTS(SELECT 1 FROM learning_group_teachers teacher WHERE teacher.learning_group_id=g.id AND teacher.academic_term_id=g.academic_term_id AND teacher.academic_year_id=g.academic_year_id AND teacher.teacher_id=$4 AND teacher.role='primary'
          AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
          AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS primary_teacher
        FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id
        JOIN academic_terms t ON t.id=g.academic_term_id
        JOIN course_offering_details d ON d.learning_offering_id=o.id
        JOIN course_assessment_plans p ON p.learning_offering_id=o.id AND p.academic_term_id=g.academic_term_id AND p.academic_year_id=g.academic_year_id
        JOIN course_assessment_phases phase ON phase.plan_id=p.id AND phase.id=$5
        JOIN academic_gradebook_phase_controls c ON c.academic_term_id=g.academic_term_id AND c.academic_year_id=g.academic_year_id AND c.phase_code=phase.phase_code
        WHERE g.id=$1 AND g.academic_term_id=$2 AND g.academic_year_id=$3"#)
        .bind(group).bind(context.academic_term_id).bind(context.academic_year_id).bind(actor.user_id).bind(phase).fetch_optional(&mut *tx).await?;
    let scope = scope.ok_or_else(|| {
        AppError::NotFound("Group phase not found in this academic context".into())
    })?;
    if !policy::can_read_group(&access, scope.owning_organization_unit_id, scope.assigned) {
        return Err(AppError::Forbidden("Gradebook group access denied".into()));
    }
    if write {
        if !policy::can_manage_group(actor, scope.assigned)
            || (confirm && !policy::can_confirm_group_phase(actor, scope.primary_teacher))
        {
            return Err(AppError::Forbidden(
                "Current group assignment and primary role are required for this action".into(),
            ));
        }
        if scope.locked {
            return Err(AppError::Conflict(
                "Course result is locked and Gradebook is read-only".into(),
            ));
        }
        if !scope.score_entry_enabled && !policy::can_manage_school(actor) {
            return Err(AppError::Forbidden(
                "Gradebook entry window is closed".into(),
            ));
        }
    }
    Ok((tx, scope))
}

pub(super) async fn invalidate(
    tx: &mut Transaction<'_, Postgres>,
    scope: &Scope,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM learning_group_phase_confirmations WHERE learning_group_id=$1 AND assessment_phase_id=$2").bind(scope.group_id).bind(scope.phase_id).execute(&mut **tx).await?;
    Ok(())
}

pub async fn list_subjects(
    pool: &PgPool,
    actor: &ActorContext,
    context: &GradebookContext,
) -> Result<Vec<GradebookSubject>, AppError> {
    let access = policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    let rows = list_subjects_in_tx(&mut tx, context, &access, actor.user_id).await?;
    if rows.len() > 1000 {
        return Err(AppError::ValidationError(
            "Gradebook subject list exceeds 1000 groups".into(),
        ));
    }
    tx.commit().await?;
    Ok(rows)
}
async fn list_subjects_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    context: &GradebookContext,
    access: &AcademicResourceListFilter,
    user_id: Uuid,
) -> Result<Vec<GradebookSubject>, AppError> {
    Ok(sqlx::query_as(r#"WITH groups AS (
      SELECT o.id AS learning_offering_id,d.subject_id,o.code_snapshot AS code,o.name_snapshot AS name,g.id AS learning_group_id,g.name AS group_name,o.owning_organization_unit_id,
      EXISTS(SELECT 1 FROM learning_group_teachers teacher WHERE teacher.learning_group_id=g.id AND teacher.academic_term_id=g.academic_term_id AND teacher.academic_year_id=g.academic_year_id AND teacher.teacher_id=$3
        AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
        AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS assigned
      FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id JOIN course_offering_details d ON d.learning_offering_id=o.id JOIN academic_terms t ON t.id=g.academic_term_id
      WHERE g.academic_term_id=$1 AND g.academic_year_id=$2)
      SELECT learning_offering_id,subject_id,code,name,learning_group_id,group_name,assigned,
        COALESCE((SELECT jsonb_agg(jsonb_build_object('id',phase.id,'phaseCode',phase.phase_code,'maxScore',phase.max_score::text,'rowVersion',phase.row_version)
          ORDER BY CASE phase.phase_code WHEN 'before_midterm' THEN 1 WHEN 'midterm' THEN 2 WHEN 'after_midterm' THEN 3 ELSE 4 END)
          FROM course_assessment_plans plan JOIN course_assessment_phases phase ON phase.plan_id=plan.id WHERE plan.learning_offering_id=groups.learning_offering_id),'[]'::jsonb) AS phases
      FROM groups
      WHERE $4 OR owning_organization_unit_id=ANY($5) OR ($6 AND assigned)
      ORDER BY assigned DESC,code,learning_offering_id,learning_group_id LIMIT 1001"#)
      .bind(context.academic_term_id).bind(context.academic_year_id).bind(user_id).bind(access.includes_school_owned).bind(&access.organization_unit_ids).bind(access.assigned_actor_id.is_some()).fetch_all(&mut **tx).await?)
}
