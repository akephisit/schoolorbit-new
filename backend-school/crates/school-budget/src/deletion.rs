//! Never-approved deletion keeps Budget authorization and Workflow ownership.
use crate::permission_codes_generated as codes;
use crate::{models::*, policy, repository, settings};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_workflow::{process_models::ProcessSource, process_purge};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(crate) async fn project_eligible(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<bool, AppError> {
    Ok(DbSession::new(&mut **tx)
        .scalar(
            "budget.deletion.eligible",
            "SELECT budget_draft_project_deletable($1)",
        )
        .bind(id)
        .fetch_one()
        .await?)
}

async fn purge_locked_project(
    tx: &mut Transaction<'_, Postgres>,
    project: &repository::ProjectRecord,
    expected_revision: Option<i64>,
) -> Result<(), AppError> {
    settings::require_open(tx, project.document.project.fiscal_year_id).await?;
    repository::revision(
        project.revision,
        expected_revision.unwrap_or(project.revision),
    )?;
    if !project_eligible(tx, project.id).await? {
        return Err(AppError::Conflict(
            "ลบได้เฉพาะโครงการที่ไม่เคยอนุมัติและไม่มีรายการทางการเงิน".into(),
        ));
    }
    let attempts: Vec<Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "budget.deletion.prepare",
            "SELECT budget_prepare_draft_removal($1,$2)",
        )
        .bind(project.id)
        .bind(project.revision)
        .fetch_all()
        .await?;
    for attempt in attempts {
        process_purge::purge_source_in_transaction(
            tx,
            &ProcessSource {
                module_code: "budget".into(),
                resource_type: "budget_project_plan".into(),
                resource_id: attempt,
            },
        )
        .await?;
    }
    DbSession::new(&mut **tx)
        .query(
            "budget.deletion.finalize",
            "SELECT budget_finalize_draft_removal($1)",
        )
        .bind(project.id)
        .execute()
        .await?;
    Ok(())
}

/// Rechecks read and delete scope after acquiring the annual/project guards.
pub(crate) async fn delete_project_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: Option<i64>,
) -> Result<(), AppError> {
    let read = policy::read_scope_on_connection(&mut **tx, actor, false).await?;
    let delete = policy::delete_scope_on_connection(&mut **tx, actor).await?;
    let project = repository::lock_project(tx, id).await?;
    policy::require_read_in_transaction(tx, actor, &project, &read).await?;
    if !policy::direct(
        &delete,
        actor.user_id,
        project.owner_id,
        project.document.project.organization_unit_id,
    ) {
        return Err(repository::denied());
    }
    purge_locked_project(tx, &project, expected_revision).await
}

/// Annual settings management authorizes only children of that exact year.
pub(crate) async fn delete_project_for_year_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    fiscal_year_id: Uuid,
) -> Result<(), AppError> {
    actor.require_permission(codes::BUDGET_MANAGE_SCHOOL)?;
    settings::lock_years(tx, &[fiscal_year_id], false).await?;
    let project = repository::lock_project(tx, id).await?;
    if project.document.project.fiscal_year_id != fiscal_year_id {
        return Err(repository::stale());
    }
    purge_locked_project(tx, &project, None).await
}

pub async fn delete_project(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: BudgetMutationRequest,
) -> Result<BudgetDeletedResource, AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "budget.deletion.project").await?;
    delete_project_in_transaction(&mut tx, actor, id, Some(input.expected_revision)).await?;
    tx.commit().await?;
    Ok(BudgetDeletedResource { deleted_id: id })
}

async fn impact(
    tx: &mut Transaction<'_, Postgres>,
    projects: &[Uuid],
) -> Result<BudgetDeletionImpact, AppError> {
    let (count, activities, lines, attempts, instances, history): (i64, i64, i64, i64, i64, i64) = DbSession::new(&mut **tx)
        .query_as(
            "budget.deletion.impact",
            "WITH owned AS (SELECT id FROM budget_projects WHERE id=ANY($1)), attempts AS (SELECT a.id FROM budget_project_attempts a JOIN owned p ON p.id=a.project_id), instances AS (SELECT i.id FROM process_instances i JOIN attempts a ON a.id=i.source_id WHERE i.source_module='budget' AND i.source_type='budget_project_plan') SELECT (SELECT count(*) FROM owned),(SELECT count(*) FROM budget_activities a JOIN owned p ON p.id=a.project_id),(SELECT count(*) FROM budget_plan_lines l JOIN owned p ON p.id=l.project_id),(SELECT count(*) FROM attempts),(SELECT count(*) FROM instances),(SELECT count(*) FROM budget_history h JOIN owned p ON h.resource_id=p.id WHERE h.resource_kind='project')+(SELECT count(*) FROM budget_project_revisions r JOIN owned p ON r.project_id=p.id)+(SELECT count(*) FROM process_action_events e JOIN instances i ON i.id=e.instance_id)+(SELECT count(*) FROM process_action_history e JOIN instances i ON i.id=e.instance_id)+(SELECT count(*) FROM process_instance_revisions r JOIN instances i ON i.id=r.instance_id)",
        )
        .bind(projects)
        .fetch_one()
        .await?;
    Ok(BudgetDeletionImpact {
        projects: count,
        activities,
        plan_lines: lines,
        attempts,
        workflow_instances: instances,
        history_entries: history,
    })
}

pub(crate) async fn project_impact(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<BudgetDeletionImpact, AppError> {
    impact(tx, &[id]).await
}

pub(crate) async fn year_impact(
    tx: &mut Transaction<'_, Postgres>,
    fiscal_year_id: Uuid,
) -> Result<BudgetDeletionImpact, AppError> {
    let projects: Vec<Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "budget.deletion.year_projects",
            "SELECT id FROM budget_projects WHERE fiscal_year_id=$1 ORDER BY id",
        )
        .bind(fiscal_year_id)
        .fetch_all()
        .await?;
    let mut result = impact(tx, &projects).await?;
    let configuration_history:i64=DbSession::new(&mut **tx).scalar("budget.deletion.year_configuration_impact", "SELECT (SELECT count(*) FROM budget_history WHERE resource_kind='settings' AND resource_id=$1)+(SELECT count(*) FROM process_template_versions v JOIN budget_year_template_ownership o ON o.template_id=v.template_id WHERE o.fiscal_year_id=$1)").bind(fiscal_year_id).fetch_one().await?;
    result.history_entries += configuration_history;
    Ok(result)
}
