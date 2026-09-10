use super::*;
use bigdecimal::BigDecimal;
use sqlx::PgPool;
use uuid::Uuid;

pub fn validate_aggregation_policy(bands: &[AggregationPolicyBand]) -> Result<(), AppError> {
    if bands.len() != 4 {
        return Err(AppError::ValidationError(
            "An aggregation policy requires exactly four bands".into(),
        ));
    }
    let mut previous = None;
    for (index, band) in bands.iter().enumerate() {
        if band.quality_level != index as i16 {
            return Err(AppError::ValidationError(
                "Aggregation quality levels must be exactly 0, 1, 2, and 3".into(),
            ));
        }
        let lower = crate::modules::academic::core::services::validate_canonical_decimal(
            &band.lower_bound,
            2,
        )?;
        if (index == 0 && lower != BigDecimal::from(0))
            || lower > BigDecimal::from(3)
            || previous.as_ref().is_some_and(|value| value >= &lower)
        {
            return Err(AppError::ValidationError(
                "Aggregation bounds must start at zero and increase through three".into(),
            ));
        }
        previous = Some(lower);
    }
    Ok(())
}

async fn load_policy(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<AggregationPolicyVersion, AppError> {
    let header: Option<(Uuid, i32, String, String, i64)> = sqlx::query_as(
        "SELECT id,version_no,name,lifecycle,row_version FROM academic_learner_evaluation_policy_versions WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    let (id, version_no, name, lifecycle, row_version) =
        header.ok_or_else(|| AppError::NotFound("Learner aggregation policy not found".into()))?;
    let bands = sqlx::query_as::<_, (i16, String)>(
        "SELECT quality_level,lower_bound::text FROM academic_learner_evaluation_policy_bands WHERE policy_version_id=$1 ORDER BY quality_level",
    )
    .bind(id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|(quality_level, lower_bound)| AggregationPolicyBand {
        quality_level,
        lower_bound,
    })
    .collect();
    Ok(AggregationPolicyVersion {
        id,
        version_no,
        name,
        lifecycle,
        row_version,
        bands,
    })
}

pub async fn list_policies(
    pool: &PgPool,
    actor: &ActorContext,
    context: &EvaluationContext,
) -> Result<Vec<AggregationPolicyVersion>, AppError> {
    policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM academic_learner_evaluation_policy_versions ORDER BY version_no DESC LIMIT 101",
    )
    .fetch_all(&mut *tx)
    .await?;
    if ids.len() > 100 {
        return Err(AppError::ValidationError(
            "Learner aggregation policy history exceeds 100 versions".into(),
        ));
    }
    let mut result = Vec::with_capacity(ids.len());
    for id in ids {
        result.push(load_policy(&mut tx, id).await?);
    }
    tx.commit().await?;
    Ok(result)
}

pub async fn create_policy(
    pool: &PgPool,
    actor: &ActorContext,
    context: &EvaluationContext,
    input: AggregationPolicyInput,
) -> Result<AggregationPolicyVersion, AppError> {
    if !policy::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "School learner-evaluation management permission is required".into(),
        ));
    }
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::ValidationError(
            "Aggregation policy name is required".into(),
        ));
    }
    validate_aggregation_policy(&input.bands)?;
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    sqlx::query(
        "SELECT id FROM academic_learner_evaluation_policy_versions ORDER BY version_no FOR UPDATE",
    )
    .execute(&mut *tx)
    .await?;
    let version_no: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(version_no),0)+1 FROM academic_learner_evaluation_policy_versions",
    )
    .fetch_one(&mut *tx)
    .await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO academic_learner_evaluation_policy_versions (version_no,name,created_by) VALUES ($1,$2,$3) RETURNING id",
    )
    .bind(version_no)
    .bind(name)
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await?;
    let levels = input
        .bands
        .iter()
        .map(|band| band.quality_level)
        .collect::<Vec<_>>();
    let bounds = input
        .bands
        .iter()
        .map(|band| {
            crate::modules::academic::core::services::validate_canonical_decimal(
                &band.lower_bound,
                2,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    sqlx::query(
        "INSERT INTO academic_learner_evaluation_policy_bands (policy_version_id,quality_level,lower_bound) SELECT $1,b.quality_level,b.lower_bound FROM unnest($2::smallint[],$3::numeric[]) b(quality_level,lower_bound)",
    )
    .bind(id)
    .bind(levels)
    .bind(bounds)
    .execute(&mut *tx)
    .await?;
    let result = load_policy(&mut tx, id).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn activate_policy(
    pool: &PgPool,
    actor: &ActorContext,
    context: &EvaluationContext,
    id: Uuid,
    row_version: i64,
) -> Result<AggregationPolicyVersion, AppError> {
    if !policy::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "School learner-evaluation management permission is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition_shared(&mut tx).await?;
    validate_context(&mut tx, context).await?;
    sqlx::query(
        "SELECT id FROM academic_learner_evaluation_policy_versions ORDER BY version_no FOR UPDATE",
    )
    .execute(&mut *tx)
    .await?;
    let target: Option<(String, i64)> = sqlx::query_as(
        "SELECT lifecycle,row_version FROM academic_learner_evaluation_policy_versions WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (lifecycle, actual_version) =
        target.ok_or_else(|| AppError::NotFound("Learner aggregation policy not found".into()))?;
    if actual_version != row_version {
        return Err(conflict());
    }
    if lifecycle != "draft" {
        return Err(AppError::Conflict(
            "An activated learner aggregation policy is immutable".into(),
        ));
    }
    sqlx::query(
        "UPDATE academic_learner_evaluation_policy_versions SET lifecycle='retired',row_version=row_version+1,updated_at=now() WHERE lifecycle='active'",
    )
    .execute(&mut *tx)
    .await?;
    let changed = sqlx::query(
        "UPDATE academic_learner_evaluation_policy_versions SET lifecycle='active',activated_at=now(),row_version=row_version+1,updated_at=now() WHERE id=$1 AND lifecycle='draft' AND row_version=$2",
    )
    .bind(id)
    .bind(row_version)
    .execute(&mut *tx)
    .await?;
    if changed.rows_affected() != 1 {
        return Err(conflict());
    }
    let result = load_policy(&mut tx, id).await?;
    tx.commit().await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregation_policy_requires_exact_inclusive_levels() {
        let bands = vec![
            AggregationPolicyBand {
                quality_level: 0,
                lower_bound: "0".into(),
            },
            AggregationPolicyBand {
                quality_level: 1,
                lower_bound: "1".into(),
            },
            AggregationPolicyBand {
                quality_level: 2,
                lower_bound: "1.5".into(),
            },
            AggregationPolicyBand {
                quality_level: 3,
                lower_bound: "2.5".into(),
            },
        ];
        assert!(validate_aggregation_policy(&bands).is_ok());
        assert!(validate_aggregation_policy(&bands[..3]).is_err());
    }
}
