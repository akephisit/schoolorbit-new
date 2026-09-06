use super::*;
pub async fn list_controls(
    pool: &PgPool,
    actor: &ActorContext,
    context: &GradebookContext,
) -> Result<Vec<GradebookControl>, AppError> {
    policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    let rows=sqlx::query_as("SELECT id,academic_year_id,academic_term_id,phase_code,score_entry_enabled,row_version FROM academic_gradebook_phase_controls WHERE academic_term_id=$1 AND academic_year_id=$2 ORDER BY CASE phase_code WHEN 'before_midterm' THEN 1 WHEN 'midterm' THEN 2 WHEN 'after_midterm' THEN 3 ELSE 4 END").bind(context.academic_term_id).bind(context.academic_year_id).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(rows)
}
pub async fn update_control(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    context: &GradebookContext,
    input: UpdateControlInput,
) -> Result<GradebookControl, AppError> {
    if !policy::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "School Gradebook management is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    let control=sqlx::query_as("UPDATE academic_gradebook_phase_controls SET score_entry_enabled=$4,row_version=row_version+1,updated_by=$6,updated_at=now() WHERE id=$1 AND academic_term_id=$2 AND academic_year_id=$3 AND row_version=$5 RETURNING id,academic_year_id,academic_term_id,phase_code,score_entry_enabled,row_version").bind(id).bind(context.academic_term_id).bind(context.academic_year_id).bind(input.score_entry_enabled).bind(input.row_version).bind(actor.user_id).fetch_optional(&mut *tx).await?.ok_or_else(conflict)?;
    tx.commit().await?;
    Ok(control)
}
