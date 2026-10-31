use super::models::*;
use crate::policy::learner_evaluation as policy;
use school_academic_core::services::lifecycle_guard::{self, AcademicWriteState};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
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
pub use summary::summarize_domains;
pub use summary::summarize_student_term_in_transaction;
pub use summary::summarize_student_terms_in_transaction;
pub use summary::{summarize_student_term, summary_for_actor};

pub async fn list_subjects(
    pool: &PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
) -> Result<Vec<EvaluationSubject>, AppError> {
    let access = policy::list_access(pool, actor).await?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_assessment.learner_evaluation.services.list_subjects",
    )
    .await?;
    validate_context(&mut tx, ctx).await?;
    let assignments =
        school_academic_delivery::services::versions::assigned_group_ids_on_connection(
            &mut tx,
            ctx.academic_year_id,
            ctx.academic_term_id,
            actor.user_id,
        )
        .await?;
    let rows:Vec<EvaluationSubject>=DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.list_subjects", r#"WITH groups AS (
        SELECT d.subject_id,g.id AS learning_group_id,o.id AS learning_offering_id,o.code_snapshot AS code,o.name_snapshot AS name,g.name AS group_name,o.owning_organization_unit_id,
        g.id=ANY($7::uuid[]) AS assigned,
        EXISTS(SELECT 1 FROM course_assessment_plans p JOIN course_offering_details coordinated ON coordinated.learning_offering_id=p.learning_offering_id WHERE coordinated.subject_id=d.subject_id AND p.academic_term_id=g.academic_term_id AND p.academic_year_id=g.academic_year_id AND p.assessment_coordinator_id=$3) AS coordinator
        FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id JOIN course_offering_details d ON d.learning_offering_id=o.id JOIN academic_terms t ON t.id=g.academic_term_id
        WHERE g.academic_term_id=$1 AND g.academic_year_id=$2 AND g.status<>'closed')
        SELECT subject_id,learning_group_id,learning_offering_id,code,name,group_name,assigned FROM groups
        WHERE $4 OR owning_organization_unit_id=ANY($5) OR ($6 AND (assigned OR coordinator))
        ORDER BY assigned DESC,code,subject_id,learning_group_id LIMIT 1001"#).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(actor.user_id).bind(access.includes_school_owned).bind(&access.organization_unit_ids).bind(access.assigned_actor_id.is_some()).bind(&assignments.learning_group_ids).fetch_all().await?;
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
    let valid: bool = DbSession::new(&mut **tx)
        .scalar(
            "academic_assessment.learner_evaluation.services.validate_context",
            "SELECT EXISTS(SELECT 1 FROM academic_terms WHERE id=$1 AND academic_year_id=$2)",
        )
        .bind(ctx.academic_term_id)
        .bind(ctx.academic_year_id)
        .fetch_one()
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
    pub primary_teacher_ids: Vec<Uuid>,
}
pub(super) struct SubjectScope {
    pub academic_state: AcademicWriteState,
    pub subject_id: Uuid,
    pub domain: LearnerEvaluationDomain,
    pub delivery_version_id: Option<Uuid>,
    pub groups: Vec<GroupScope>,
    pub coordinator: bool,
    pub entry_enabled: bool,
    pub locked: bool,
}

/// Every overlapping writer acquires lifecycle -> offering -> group -> configuration/control/source.
/// Locking all subject offerings serializes first access and all-room snapshots.
pub(super) async fn begin_subject<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    subject: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
    write: bool,
) -> Result<(school_tenancy::ObservedTransaction, SubjectScope), AppError> {
    begin_subject_with_selection(pool, actor, subject, domain, ctx, write, None).await
}

pub(super) async fn begin_subject_with_selection(
    pool: &PgPool,
    actor: &ActorContext,
    subject: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
    write: bool,
    result_locks: Option<&dyn crate::ports::ResultLockPort>,
) -> Result<(school_tenancy::ObservedTransaction, SubjectScope), AppError> {
    let access = policy::list_access(pool, actor).await?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_assessment.learner_evaluation.services.begin_subject",
    )
    .await?;
    validate_context(&mut tx, ctx).await?;
    let academic_state =
        lifecycle_guard::lock_context(&mut tx, ctx.academic_year_id, ctx.academic_term_id).await?;
    if write {
        academic_state.require_writable()?;
    }
    let offerings:Vec<Uuid>=DbSession::new(&mut *tx).scalar("academic_assessment.learner_evaluation.services.begin_subject", "SELECT o.id FROM learning_offerings o JOIN course_offering_details d ON d.learning_offering_id=o.id WHERE d.subject_id=$1 AND o.academic_term_id=$2 AND o.academic_year_id=$3 ORDER BY o.id FOR UPDATE OF o").bind(subject).bind(ctx.academic_term_id).bind(ctx.academic_year_id).fetch_all().await?;
    if offerings.is_empty() {
        return Err(AppError::NotFound(
            "Course subject not found in this context".into(),
        ));
    }
    let (group_ids, operation_date) = if result_locks.is_some() {
        let locked_groups: Vec<Uuid> = DbSession::new(&mut *tx).scalar(
            "academic_assessment.learner_evaluation.services.begin_subject",
            "SELECT id FROM learning_groups WHERE learning_offering_id=ANY($1) AND status<>'closed' ORDER BY id LIMIT $2 FOR UPDATE",
        ).bind(&offerings).bind((crate::ports::CURRENT_OBLIGATION_GROUP_SCAN_LIMIT+1) as i64).fetch_all().await?;
        if locked_groups.len() > crate::ports::CURRENT_OBLIGATION_GROUP_SCAN_LIMIT {
            return Err(AppError::ValidationError(
                "Learner evaluation raw group scan exceeds its hard limit".into(),
            ));
        }
        (
            locked_groups,
            Some(current_obligation_date(&mut tx, ctx).await?),
        )
    } else {
        let locked_groups = DbSession::new(&mut *tx).scalar("academic_assessment.learner_evaluation.services.begin_subject",
        "SELECT id FROM learning_groups WHERE learning_offering_id=ANY($1) ORDER BY id FOR UPDATE",
    )
    .bind(&offerings)
    .fetch_all()
    .await?;
        (locked_groups, None)
    };
    let assignments =
        school_academic_delivery::services::versions::group_teacher_assignments_on_connection(
            &mut tx,
            ctx.academic_year_id,
            ctx.academic_term_id,
            &group_ids,
        )
        .await?;
    // Persisted subject coordination is independent of active group assignments.
    let coordinator:bool=DbSession::new(&mut *tx).scalar("academic_assessment.learner_evaluation.services.begin_subject", "SELECT EXISTS(SELECT 1 FROM course_assessment_plans WHERE learning_offering_id=ANY($1) AND academic_term_id=$2 AND academic_year_id=$3 AND assessment_coordinator_id=$4)").bind(&offerings).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(actor.user_id).fetch_one().await?;
    let mut groups:Vec<GroupScope>=DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.begin_subject", r#"SELECT g.id AS group_id,g.learning_offering_id AS offering_id,o.owning_organization_unit_id AS owner,
        COALESCE($2=ANY(teaching.teacher_ids),false) AS assigned,
        COALESCE(teaching.primary_teacher_ids,ARRAY[]::uuid[]) AS primary_teacher_ids
        FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id JOIN academic_terms t ON t.id=g.academic_term_id
        LEFT JOIN jsonb_to_recordset($3::jsonb) AS teaching(learning_group_id uuid,teacher_ids uuid[],primary_teacher_ids uuid[])
          ON teaching.learning_group_id=g.id
        WHERE g.learning_offering_id=ANY($1) AND g.status<>'closed' ORDER BY g.id"#).bind(&offerings).bind(actor.user_id).bind(sqlx::types::Json(&assignments.groups)).fetch_all().await?;
    if !groups
        .iter()
        .any(|g| policy::can_read_group(&access, g.owner, g.assigned, coordinator))
    {
        return Err(AppError::Forbidden("Subject access denied".into()));
    }
    if let (Some(result_locks), Some(date)) = (result_locks, operation_date) {
        let candidates = groups
            .iter()
            .map(|group| group.group_id)
            .collect::<Vec<_>>();
        let retained = result_locks
            .current_obligation_group_ids(
                &mut tx,
                ctx.academic_term_id,
                ctx.academic_year_id,
                date,
                &candidates,
            )
            .await?
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        groups.retain(|group| retained.contains(&group.group_id));
        if groups.is_empty() {
            return Err(AppError::Conflict(
                "No current learner evaluation obligations remain for this subject".into(),
            ));
        }
    }
    if academic_state.is_writable() {
        configuration::initialize(&mut tx, subject, domain, ctx, &offerings).await?;
        ensure_controls(&mut tx, ctx).await?;
    }
    DbSession::new(&mut *tx).query("academic_assessment.learner_evaluation.services.begin_subject", "SELECT subject_id FROM subject_term_evaluation_configurations WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3 FOR UPDATE").bind(subject).bind(ctx.academic_term_id).bind(domain.as_str()).execute().await?;
    let entry_enabled:bool=DbSession::new(&mut *tx).scalar("academic_assessment.learner_evaluation.services.begin_subject", "SELECT entry_enabled FROM academic_learner_evaluation_controls WHERE academic_term_id=$1 AND domain=$2 FOR SHARE").bind(ctx.academic_term_id).bind(domain.as_str()).fetch_optional().await?.unwrap_or(false);
    let locked:bool=DbSession::new(&mut *tx).scalar("academic_assessment.learner_evaluation.services.begin_subject", "SELECT EXISTS(SELECT 1 FROM subject_term_evaluation_locks WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3)").bind(subject).bind(ctx.academic_term_id).bind(domain.as_str()).fetch_one().await?;
    Ok((
        tx,
        SubjectScope {
            academic_state,
            subject_id: subject,
            domain,
            delivery_version_id: assignments.delivery_version_id,
            groups,
            coordinator,
            entry_enabled,
            locked,
        },
    ))
}
pub(super) async fn subject_for_group(
    tx: &mut Transaction<'_, Postgres>,
    group: Uuid,
    ctx: &EvaluationContext,
) -> Result<Uuid, AppError> {
    validate_context(tx, ctx).await?;
    let subject=DbSession::new(&mut **tx).scalar("academic_assessment.learner_evaluation.services.subject_for_group", "SELECT d.subject_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$1 AND g.academic_term_id=$2 AND g.academic_year_id=$3").bind(group).bind(ctx.academic_term_id).bind(ctx.academic_year_id).fetch_optional().await?.ok_or_else(||AppError::NotFound("Course group not found in context".into()))?;
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
    DbSession::new(&mut **tx).query("academic_assessment.learner_evaluation.services.invalidate", "UPDATE learning_group_evaluation_confirmations SET row_version=row_version+1,source_snapshot=jsonb_set(source_snapshot,'{invalidated}','true'::jsonb) WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3 AND ($4::uuid IS NULL OR learning_group_id=$4)").bind(scope.subject_id).bind(ctx.academic_term_id).bind(scope.domain.as_str()).bind(group).execute().await?;
    Ok(())
}
pub(super) async fn next_revision(
    tx: &mut Transaction<'_, Postgres>,
    scope: &SubjectScope,
    ctx: &EvaluationContext,
) -> Result<i64, AppError> {
    Ok(DbSession::new(&mut **tx).scalar("academic_assessment.learner_evaluation.services.next_revision", "UPDATE subject_term_evaluation_configurations SET row_version=row_version+1,updated_at=now() WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3 RETURNING row_version").bind(scope.subject_id).bind(ctx.academic_term_id).bind(scope.domain.as_str()).fetch_one().await?)
}
pub(super) async fn ensure_controls(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &EvaluationContext,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx).query("academic_assessment.learner_evaluation.services.ensure_controls", "INSERT INTO academic_learner_evaluation_controls (academic_term_id,academic_year_id,domain) SELECT $1,$2,domain FROM (VALUES ('desirable_characteristic'),('reading_thinking_writing')) d(domain) ON CONFLICT (academic_term_id,domain) DO NOTHING").bind(ctx.academic_term_id).bind(ctx.academic_year_id).execute().await?;
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

pub(super) async fn current_obligation_date(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &EvaluationContext,
) -> Result<chrono::NaiveDate, AppError> {
    DbSession::new(&mut **tx).scalar(
        "academic_assessment.learner_evaluation.services.current_obligation_date",
        "SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1 AND t.academic_year_id=$2",
    ).bind(ctx.academic_term_id).bind(ctx.academic_year_id).fetch_one().await.map_err(Into::into)
}
