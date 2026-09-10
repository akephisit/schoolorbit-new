use super::models::*;
use crate::{
    error::AppError, middleware::permission::ActorContext,
    policies::learner_evaluation_access_policy as policy,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

mod catalog;
mod configuration;
mod confirmation;
mod entry;
mod locking;
mod policies;
mod summary;
pub use catalog::*;
pub use configuration::{get_configuration, remove_criterion, save_criterion};
pub use confirmation::confirm_group;
pub use entry::{get_workspace, save_responses};
pub use locking::{lock_subject, read_lock_readiness};
pub use policies::{activate_policy, create_policy, list_policies};
#[cfg(test)]
pub use summary::summarize_domains;
pub(crate) use summary::summarize_student_term_in_transaction;
pub use summary::{summarize_student_term, summary_for_actor};

pub async fn list_subjects(
    pool: &PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
) -> Result<Vec<EvaluationSubject>, AppError> {
    let access = policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, ctx).await?;
    let rows:Vec<EvaluationSubject>=sqlx::query_as(r#"WITH groups AS (
        SELECT d.subject_id,g.id AS learning_group_id,o.id AS learning_offering_id,o.code_snapshot AS code,o.name_snapshot AS name,g.name AS group_name,o.owning_organization_unit_id,
        EXISTS(SELECT 1 FROM learning_group_teachers teacher WHERE teacher.learning_group_id=g.id AND teacher.teacher_id=$3 AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date) AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS assigned,
        EXISTS(SELECT 1 FROM course_assessment_plans p JOIN course_offering_details coordinated ON coordinated.learning_offering_id=p.learning_offering_id WHERE coordinated.subject_id=d.subject_id AND p.academic_term_id=g.academic_term_id AND p.academic_year_id=g.academic_year_id AND p.assessment_coordinator_id=$3) AS coordinator
        FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id JOIN course_offering_details d ON d.learning_offering_id=o.id JOIN academic_terms t ON t.id=g.academic_term_id
        WHERE g.academic_term_id=$1 AND g.academic_year_id=$2 AND g.status<>'closed')
        SELECT subject_id,learning_group_id,learning_offering_id,code,name,group_name,assigned FROM groups
        WHERE $4 OR owning_organization_unit_id=ANY($5) OR ($6 AND (assigned OR coordinator))
        ORDER BY assigned DESC,code,subject_id,learning_group_id LIMIT 1001"#).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(actor.user_id).bind(access.includes_school_owned).bind(&access.organization_unit_ids).bind(access.assigned_actor_id.is_some()).fetch_all(&mut *tx).await?;
    if rows.len() > 1000 {
        return Err(AppError::ValidationError(
            "Subject list exceeds 1000 groups".into(),
        ));
    }
    tx.commit().await?;
    Ok(rows)
}

pub(super) fn conflict() -> AppError {
    AppError::Conflict("Learner evaluation source changed; refresh before retrying".into())
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
    ctx: &EvaluationContext,
) -> Result<(), AppError> {
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM academic_terms WHERE id=$1 AND academic_year_id=$2)",
    )
    .bind(ctx.academic_term_id)
    .bind(ctx.academic_year_id)
    .fetch_one(&mut **tx)
    .await?;
    if !valid {
        return Err(AppError::ValidationError(
            "Academic term does not belong to selected year".into(),
        ));
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
pub(super) struct GroupScope {
    pub group_id: Uuid,
    pub offering_id: Uuid,
    pub owner: Option<Uuid>,
    pub assigned: bool,
    pub primary_teacher_id: Option<Uuid>,
}
pub(super) struct SubjectScope {
    pub subject_id: Uuid,
    pub domain: LearnerEvaluationDomain,
    pub groups: Vec<GroupScope>,
    pub coordinator: bool,
    pub entry_enabled: bool,
    pub locked: bool,
}

/// Every overlapping writer acquires offering -> group -> configuration/control/source.
/// Locking all subject offerings serializes first access and all-room snapshots.
pub(super) async fn begin_subject<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    subject: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
) -> Result<(Transaction<'a, Postgres>, SubjectScope), AppError> {
    let access = policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, ctx).await?;
    let offerings:Vec<Uuid>=sqlx::query_scalar("SELECT o.id FROM learning_offerings o JOIN course_offering_details d ON d.learning_offering_id=o.id WHERE d.subject_id=$1 AND o.academic_term_id=$2 AND o.academic_year_id=$3 ORDER BY o.id FOR UPDATE OF o").bind(subject).bind(ctx.academic_term_id).bind(ctx.academic_year_id).fetch_all(&mut *tx).await?;
    if offerings.is_empty() {
        return Err(AppError::NotFound(
            "Course subject not found in this context".into(),
        ));
    }
    sqlx::query(
        "SELECT id FROM learning_groups WHERE learning_offering_id=ANY($1) ORDER BY id FOR UPDATE",
    )
    .bind(&offerings)
    .execute(&mut *tx)
    .await?;
    // Persisted subject coordination is independent of active group assignments.
    let coordinator:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM course_assessment_plans WHERE learning_offering_id=ANY($1) AND academic_term_id=$2 AND academic_year_id=$3 AND assessment_coordinator_id=$4)").bind(&offerings).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(actor.user_id).fetch_one(&mut *tx).await?;
    let groups:Vec<GroupScope>=sqlx::query_as(r#"SELECT g.id AS group_id,g.learning_offering_id AS offering_id,o.owning_organization_unit_id AS owner,
        EXISTS(SELECT 1 FROM learning_group_teachers teacher JOIN users u ON u.id=teacher.teacher_id AND u.status='active' WHERE teacher.learning_group_id=g.id AND teacher.academic_term_id=g.academic_term_id AND teacher.academic_year_id=g.academic_year_id AND teacher.teacher_id=$2 AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date) AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS assigned,
        (SELECT teacher.teacher_id FROM learning_group_teachers teacher JOIN users u ON u.id=teacher.teacher_id AND u.status='active' WHERE teacher.learning_group_id=g.id AND teacher.academic_term_id=g.academic_term_id AND teacher.academic_year_id=g.academic_year_id AND teacher.role='primary' AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date) AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)) ORDER BY teacher.id LIMIT 1) AS primary_teacher_id
        FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id JOIN academic_terms t ON t.id=g.academic_term_id
        WHERE g.learning_offering_id=ANY($1) AND g.status<>'closed' ORDER BY g.id"#).bind(&offerings).bind(actor.user_id).fetch_all(&mut *tx).await?;
    if !groups
        .iter()
        .any(|g| policy::can_read_group(&access, g.owner, g.assigned, coordinator))
    {
        return Err(AppError::Forbidden("Subject access denied".into()));
    }
    configuration::initialize(&mut tx, subject, domain, ctx, &offerings).await?;
    sqlx::query("SELECT subject_id FROM subject_term_evaluation_configurations WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3 FOR UPDATE").bind(subject).bind(ctx.academic_term_id).bind(domain.as_str()).execute(&mut *tx).await?;
    ensure_controls(&mut tx, ctx).await?;
    let entry_enabled:bool=sqlx::query_scalar("SELECT entry_enabled FROM academic_learner_evaluation_controls WHERE academic_term_id=$1 AND domain=$2 FOR SHARE").bind(ctx.academic_term_id).bind(domain.as_str()).fetch_one(&mut *tx).await?;
    let locked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM subject_term_evaluation_locks WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3)").bind(subject).bind(ctx.academic_term_id).bind(domain.as_str()).fetch_one(&mut *tx).await?;
    Ok((
        tx,
        SubjectScope {
            subject_id: subject,
            domain,
            groups,
            coordinator,
            entry_enabled,
            locked,
        },
    ))
}
pub(super) async fn subject_for_group(
    pool: &PgPool,
    group: Uuid,
    ctx: &EvaluationContext,
) -> Result<Uuid, AppError> {
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, ctx).await?;
    let subject=sqlx::query_scalar("SELECT d.subject_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$1 AND g.academic_term_id=$2 AND g.academic_year_id=$3").bind(group).bind(ctx.academic_term_id).bind(ctx.academic_year_id).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::NotFound("Course group not found in context".into()))?;
    tx.commit().await?;
    Ok(subject)
}
pub(super) fn require_edit(
    actor: &ActorContext,
    scope: &SubjectScope,
    allowed: bool,
) -> Result<(), AppError> {
    require_unlocked(scope, allowed)?;
    if !scope.entry_enabled && !policy::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "Learner evaluation entry window is closed".into(),
        ));
    }
    Ok(())
}
pub(super) fn require_unlocked(scope: &SubjectScope, allowed: bool) -> Result<(), AppError> {
    if !allowed {
        return Err(AppError::Forbidden(
            "Current teaching assignment or coordinator is required".into(),
        ));
    }
    if scope.locked {
        return Err(AppError::Conflict(
            "Subject evaluation domain is locked".into(),
        ));
    }
    Ok(())
}
pub(super) async fn invalidate(
    tx: &mut Transaction<'_, Postgres>,
    scope: &SubjectScope,
    ctx: &EvaluationContext,
    group: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query("UPDATE learning_group_evaluation_confirmations SET row_version=row_version+1,source_snapshot=jsonb_set(source_snapshot,'{invalidated}','true'::jsonb) WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3 AND ($4::uuid IS NULL OR learning_group_id=$4)").bind(scope.subject_id).bind(ctx.academic_term_id).bind(scope.domain.as_str()).bind(group).execute(&mut **tx).await?;
    Ok(())
}
pub(super) async fn next_revision(
    tx: &mut Transaction<'_, Postgres>,
    scope: &SubjectScope,
    ctx: &EvaluationContext,
) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar("UPDATE subject_term_evaluation_configurations SET row_version=row_version+1,updated_at=now() WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3 RETURNING row_version").bind(scope.subject_id).bind(ctx.academic_term_id).bind(scope.domain.as_str()).fetch_one(&mut **tx).await?)
}
pub(super) async fn ensure_controls(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &EvaluationContext,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO academic_learner_evaluation_controls (academic_term_id,academic_year_id,domain) SELECT $1,$2,domain FROM (VALUES ('desirable_characteristic'),('reading_thinking_writing')) d(domain) ON CONFLICT (academic_term_id,domain) DO NOTHING").bind(ctx.academic_term_id).bind(ctx.academic_year_id).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_and_missing_versions_conflict() {
        assert!(check_version(Some(2), Some(3)).is_err());
        assert!(check_version(None, Some(1)).is_err());
        assert!(check_version(Some(1), None).is_err());
        assert!(check_version(None, None).is_ok());
    }
}
