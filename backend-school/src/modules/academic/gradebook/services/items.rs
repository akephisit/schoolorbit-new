use super::*;
fn validate_item(input: &ItemInput) -> Result<BigDecimal, AppError> {
    if input.name.trim().is_empty() || input.name.chars().count() > 200 {
        return Err(AppError::ValidationError(
            "Item name must contain 1–200 characters".into(),
        ));
    }
    decimal(&input.max_score)
}
pub async fn create_item(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    context: &GradebookContext,
    input: ItemInput,
) -> Result<ScoreItem, AppError> {
    let maximum = validate_item(&input)?;
    if input.row_version.is_some() {
        return Err(conflict());
    }
    let (mut tx, scope) = begin_scope(pool, actor, group, phase, context, true, false).await?;
    validate_allocation(&mut tx, &scope, &maximum, None).await?;
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM learning_group_score_items WHERE learning_group_id=$1 AND assessment_phase_id=$2").bind(group).bind(phase).fetch_one(&mut *tx).await?;
    if count >= 1000 {
        return Err(AppError::ValidationError(
            "A phase supports at most 1000 retained items".into(),
        ));
    }
    let item=sqlx::query_as("INSERT INTO learning_group_score_items (learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score,display_order,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10) RETURNING id,name,max_score::text AS max_score,display_order,lifecycle,row_version").bind(group).bind(scope.offering_id).bind(scope.plan_id).bind(phase).bind(context.academic_term_id).bind(context.academic_year_id).bind(input.name.trim()).bind(maximum).bind(input.display_order).bind(actor.user_id).fetch_one(&mut *tx).await?;
    invalidate(&mut tx, &scope).await?;
    tx.commit().await?;
    Ok(item)
}
pub async fn update_item(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    item_id: Uuid,
    context: &GradebookContext,
    input: ItemInput,
) -> Result<ScoreItem, AppError> {
    let maximum = validate_item(&input)?;
    let (mut tx, scope) = begin_scope(pool, actor, group, phase, context, true, false).await?;
    let prior = load_item(&mut tx, &scope, item_id).await?;
    check_version(input.row_version, Some(prior.row_version))?;
    validate_allocation(&mut tx, &scope, &maximum, Some(&prior)).await?;
    let exceeds:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM learning_group_student_scores WHERE score_item_id=$1 AND score>$2)").bind(item_id).bind(&maximum).fetch_one(&mut *tx).await?;
    if exceeds {
        return Err(AppError::ValidationError(
            "Item maximum is below a retained student score; reduce or clear that score first"
                .into(),
        ));
    }
    let changed = decimal(&prior.max_score)? != maximum;
    let item=sqlx::query_as("UPDATE learning_group_score_items SET name=$2,max_score=$3,display_order=$4,row_version=row_version+1,updated_by=$5,updated_at=now() WHERE id=$1 RETURNING id,name,max_score::text AS max_score,display_order,lifecycle,row_version").bind(item_id).bind(input.name.trim()).bind(maximum).bind(input.display_order).bind(actor.user_id).fetch_one(&mut *tx).await?;
    if changed {
        invalidate(&mut tx, &scope).await?;
    }
    tx.commit().await?;
    Ok(item)
}
pub async fn remove_item(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    item_id: Uuid,
    context: &GradebookContext,
    row_version: i64,
) -> Result<ScoreItemRemovalOutcome, AppError> {
    let (mut tx, scope) = begin_scope(pool, actor, group, phase, context, true, false).await?;
    let prior = load_item(&mut tx, &scope, item_id).await?;
    check_version(Some(row_version), Some(prior.row_version))?;
    let has_scores: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM learning_group_student_scores WHERE score_item_id=$1)",
    )
    .bind(item_id)
    .fetch_one(&mut *tx)
    .await?;
    let outcome = if has_scores {
        let revision=sqlx::query_scalar("UPDATE learning_group_score_items SET lifecycle='cancelled',cancelled_at=now(),cancelled_by=$2,updated_by=$2,updated_at=now(),row_version=row_version+1 WHERE id=$1 RETURNING row_version").bind(item_id).bind(actor.user_id).fetch_one(&mut *tx).await?;
        ScoreItemRemovalOutcome {
            disposition: ScoreItemRemovalDisposition::Cancelled,
            item_id,
            row_version: revision,
        }
    } else {
        sqlx::query("DELETE FROM learning_group_score_items WHERE id=$1")
            .bind(item_id)
            .execute(&mut *tx)
            .await?;
        ScoreItemRemovalOutcome {
            disposition: ScoreItemRemovalDisposition::Deleted,
            item_id,
            row_version,
        }
    };
    invalidate(&mut tx, &scope).await?;
    tx.commit().await?;
    Ok(outcome)
}
async fn load_item(
    tx: &mut Transaction<'_, Postgres>,
    scope: &Scope,
    item_id: Uuid,
) -> Result<ScoreItem, AppError> {
    sqlx::query_as("SELECT id,name,max_score::text AS max_score,display_order,lifecycle,row_version FROM learning_group_score_items WHERE id=$1 AND learning_group_id=$2 AND assessment_phase_id=$3 AND lifecycle='active' FOR UPDATE").bind(item_id).bind(scope.group_id).bind(scope.phase_id).fetch_optional(&mut **tx).await?.ok_or_else(||AppError::NotFound("Active score item not found in this group phase".into()))
}

async fn validate_allocation(
    tx: &mut Transaction<'_, Postgres>,
    scope: &Scope,
    maximum: &BigDecimal,
    prior: Option<&ScoreItem>,
) -> Result<(), AppError> {
    // Reductions and cosmetic edits must remain possible after a phase maximum is lowered.
    let prior_maximum = prior.map(|item| decimal(&item.max_score)).transpose()?;
    if prior_maximum.as_ref().is_some_and(|old| maximum <= old) {
        return Ok(());
    }
    // begin_scope holds the shared offering/group write locks before reading this sum.
    let allocated: BigDecimal = sqlx::query_scalar(
        "SELECT COALESCE(sum(max_score),0) FROM learning_group_score_items WHERE learning_group_id=$1 AND assessment_phase_id=$2 AND lifecycle='active'",
    )
    .bind(scope.group_id)
    .bind(scope.phase_id)
    .fetch_one(&mut **tx)
    .await?;
    let phase_maximum = decimal(&scope.phase_max_score)?;
    if prior.is_none() && allocated >= phase_maximum {
        return Err(AppError::ValidationError(
            "จัดสรรคะแนนครบแล้ว กรุณาลดคะแนนเต็มของรายการเดิมก่อนเพิ่มรายการใหม่".into(),
        ));
    }
    let available = phase_maximum - allocated + prior_maximum.unwrap_or_default();
    if maximum > &available {
        return Err(AppError::ValidationError(format!(
            "คะแนนเต็มของรายการนี้ต้องไม่เกิน {} คะแนน",
            available.normalized()
        )));
    }
    Ok(())
}
