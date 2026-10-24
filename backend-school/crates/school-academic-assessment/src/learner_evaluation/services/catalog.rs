use super::*;
use school_database::DbSession;

pub async fn list_catalog(
    pool: &PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
) -> Result<Vec<CatalogCriterion>, AppError> {
    policy::list_access(pool, actor).await?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_assessment.learner_evaluation.services.catalog.list_catalog",
    )
    .await?;
    validate_context(&mut tx, ctx).await?;
    let rows=DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.catalog.list_catalog", "SELECT id,domain,name,applicability,lifecycle,display_order,row_version FROM academic_learner_evaluation_criteria ORDER BY domain,display_order,id").fetch_all().await?;
    tx.commit().await?;
    Ok(rows)
}
pub async fn save_catalog(
    pool: &PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
    id: Option<Uuid>,
    input: CatalogInput,
) -> Result<CatalogCriterion, AppError> {
    if !policy::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "School learner evaluation management is required".into(),
        ));
    }
    validate_catalog(&input)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_assessment.learner_evaluation.services.catalog.save_catalog",
    )
    .await?;
    validate_context(&mut tx, ctx).await?;
    let lifecycle = if input.criterion.active {
        "active"
    } else {
        "inactive"
    };
    let row = if let Some(id) = id {
        DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.catalog.save_catalog", "UPDATE academic_learner_evaluation_criteria SET name=$2,applicability=$3,lifecycle=$4,display_order=$5,row_version=row_version+1,updated_at=now() WHERE id=$1 AND row_version=$6 AND domain=$7 RETURNING id,domain,name,applicability,lifecycle,display_order,row_version").bind(id).bind(input.criterion.name.trim()).bind(&input.applicability).bind(lifecycle).bind(input.criterion.display_order).bind(input.criterion.row_version).bind(input.domain.as_str()).fetch_optional().await?.ok_or_else(conflict)?
    } else {
        check_version(input.criterion.row_version, None)?;
        DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.catalog.save_catalog", "INSERT INTO academic_learner_evaluation_criteria (domain,name,applicability,lifecycle,display_order) VALUES ($1,$2,$3,$4,$5) RETURNING id,domain,name,applicability,lifecycle,display_order,row_version").bind(input.domain.as_str()).bind(input.criterion.name.trim()).bind(&input.applicability).bind(lifecycle).bind(input.criterion.display_order).fetch_one().await?
    };
    tx.commit().await?;
    Ok(row)
}
pub async fn remove_catalog(
    pool: &PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
    id: Uuid,
    version: i64,
) -> Result<(), AppError> {
    if !policy::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "School learner evaluation management is required".into(),
        ));
    }
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_assessment.learner_evaluation.services.catalog.remove_catalog",
    )
    .await?;
    validate_context(&mut tx, ctx).await?;
    let current: Option<i64> = DbSession::new(&mut *tx)
        .scalar(
            "academic_assessment.learner_evaluation.services.catalog.remove_catalog",
            "SELECT row_version FROM academic_learner_evaluation_criteria WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional()
        .await?;
    check_version(Some(version), current)?;
    let used:bool=DbSession::new(&mut *tx).scalar("academic_assessment.learner_evaluation.services.catalog.remove_catalog", "SELECT EXISTS(SELECT 1 FROM subject_term_evaluation_criteria WHERE school_criterion_id=$1)").bind(id).fetch_one().await?;
    if used {
        DbSession::new(&mut *tx).query("academic_assessment.learner_evaluation.services.catalog.remove_catalog", "UPDATE academic_learner_evaluation_criteria SET lifecycle='inactive',row_version=row_version+1,updated_at=now() WHERE id=$1").bind(id).execute().await?;
    } else {
        DbSession::new(&mut *tx)
            .query(
                "academic_assessment.learner_evaluation.services.catalog.remove_catalog",
                "DELETE FROM academic_learner_evaluation_criteria WHERE id=$1",
            )
            .bind(id)
            .execute()
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
fn validate_catalog(input: &CatalogInput) -> Result<(), AppError> {
    configuration::validate_criterion(&input.criterion)?;
    if !matches!(
        input.applicability.as_str(),
        "all" | "primary" | "secondary"
    ) {
        return Err(AppError::ValidationError(
            "Unknown catalog applicability".into(),
        ));
    }
    Ok(())
}

pub async fn list_controls(
    pool: &PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
) -> Result<Vec<EvaluationControl>, AppError> {
    policy::list_access(pool, actor).await?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_assessment.learner_evaluation.services.catalog.list_controls",
    )
    .await?;
    validate_context(&mut tx, ctx).await?;
    if lifecycle_guard::lock_context(&mut tx, ctx.academic_year_id, ctx.academic_term_id)
        .await?
        .is_writable()
    {
        ensure_controls(&mut tx, ctx).await?;
    }
    let rows=DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.catalog.list_controls", "SELECT id,domain,entry_enabled,row_version FROM academic_learner_evaluation_controls WHERE academic_term_id=$1 AND academic_year_id=$2 ORDER BY domain").bind(ctx.academic_term_id).bind(ctx.academic_year_id).fetch_all().await?;
    tx.commit().await?;
    Ok(rows)
}
pub async fn update_control(
    pool: &PgPool,
    actor: &ActorContext,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
    input: ControlInput,
) -> Result<EvaluationControl, AppError> {
    if !policy::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "School learner evaluation management is required".into(),
        ));
    }
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_assessment.learner_evaluation.services.catalog.update_control",
    )
    .await?;
    validate_context(&mut tx, ctx).await?;
    lifecycle_guard::require_term_write(&mut tx, ctx.academic_year_id, ctx.academic_term_id)
        .await?;
    let row=DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.catalog.update_control", "UPDATE academic_learner_evaluation_controls SET entry_enabled=$4,row_version=row_version+1,updated_by=$6,updated_at=now() WHERE academic_term_id=$1 AND academic_year_id=$2 AND domain=$3 AND row_version=$5 RETURNING id,domain,entry_enabled,row_version").bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(domain.as_str()).bind(input.entry_enabled).bind(input.row_version).bind(actor.user_id).fetch_optional().await?.ok_or_else(conflict)?;
    tx.commit().await?;
    Ok(row)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_applicability_is_rejected_before_writing() {
        let input = CatalogInput {
            domain: LearnerEvaluationDomain::ReadingThinkingWriting,
            applicability: "custom".into(),
            criterion: CriterionInput {
                name: "A".into(),
                display_order: 0,
                active: true,
                row_version: None,
            },
        };
        assert!(validate_catalog(&input).is_err());
    }
}
