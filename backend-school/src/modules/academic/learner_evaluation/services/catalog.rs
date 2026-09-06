use super::*;

pub async fn list_catalog(
    pool: &PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
) -> Result<Vec<CatalogCriterion>, AppError> {
    policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, ctx).await?;
    let rows=sqlx::query_as("SELECT id,domain,name,applicability,lifecycle,display_order,row_version FROM academic_learner_evaluation_criteria ORDER BY domain,display_order,id").fetch_all(&mut *tx).await?;
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
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, ctx).await?;
    let lifecycle = if input.criterion.active {
        "active"
    } else {
        "inactive"
    };
    let row = if let Some(id) = id {
        sqlx::query_as("UPDATE academic_learner_evaluation_criteria SET name=$2,applicability=$3,lifecycle=$4,display_order=$5,row_version=row_version+1,updated_at=now() WHERE id=$1 AND row_version=$6 AND domain=$7 RETURNING id,domain,name,applicability,lifecycle,display_order,row_version").bind(id).bind(input.criterion.name.trim()).bind(&input.applicability).bind(lifecycle).bind(input.criterion.display_order).bind(input.criterion.row_version).bind(input.domain.as_str()).fetch_optional(&mut *tx).await?.ok_or_else(conflict)?
    } else {
        check_version(input.criterion.row_version, None)?;
        sqlx::query_as("INSERT INTO academic_learner_evaluation_criteria (domain,name,applicability,lifecycle,display_order) VALUES ($1,$2,$3,$4,$5) RETURNING id,domain,name,applicability,lifecycle,display_order,row_version").bind(input.domain.as_str()).bind(input.criterion.name.trim()).bind(&input.applicability).bind(lifecycle).bind(input.criterion.display_order).fetch_one(&mut *tx).await?
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
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, ctx).await?;
    let current: Option<i64> = sqlx::query_scalar(
        "SELECT row_version FROM academic_learner_evaluation_criteria WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    check_version(Some(version), current)?;
    let used:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM subject_term_evaluation_criteria WHERE school_criterion_id=$1)").bind(id).fetch_one(&mut *tx).await?;
    if used {
        sqlx::query("UPDATE academic_learner_evaluation_criteria SET lifecycle='inactive',row_version=row_version+1,updated_at=now() WHERE id=$1").bind(id).execute(&mut *tx).await?;
    } else {
        sqlx::query("DELETE FROM academic_learner_evaluation_criteria WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
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
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, ctx).await?;
    ensure_controls(&mut tx, ctx).await?;
    let rows=sqlx::query_as("SELECT id,domain,entry_enabled,row_version FROM academic_learner_evaluation_controls WHERE academic_term_id=$1 AND academic_year_id=$2 ORDER BY domain").bind(ctx.academic_term_id).bind(ctx.academic_year_id).fetch_all(&mut *tx).await?;
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
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, ctx).await?;
    let row=sqlx::query_as("UPDATE academic_learner_evaluation_controls SET entry_enabled=$4,row_version=row_version+1,updated_by=$6,updated_at=now() WHERE academic_term_id=$1 AND academic_year_id=$2 AND domain=$3 AND row_version=$5 RETURNING id,domain,entry_enabled,row_version").bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(domain.as_str()).bind(input.entry_enabled).bind(input.row_version).bind(actor.user_id).fetch_optional(&mut *tx).await?.ok_or_else(conflict)?;
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
