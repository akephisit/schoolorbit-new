use super::*;
use crate::{
    middleware::permission::ActorContext,
    policies::academic_aggregate_access_policy::{
        require_aggregate_policy_manage, require_aggregate_read,
    },
};
use sqlx::PgPool;
use uuid::Uuid;

pub fn validate_aggregate_policy(input: &AggregatePolicyInput) -> Result<(), AppError> {
    aggregate_course_credits(&[], &input.passing_grade)?;
    if input.name.trim().is_empty()
        || input.name.trim().chars().count() > 160
        || !(0..=3).contains(&input.minimum_learner_level)
    {
        return Err(AppError::ValidationError(
            "ระบุชื่อนโยบายไม่เกิน 160 ตัวอักษร และระดับผลประเมินขั้นต่ำ 0–3".into(),
        ));
    }
    Ok(())
}

pub async fn create_aggregate_policy(
    pool: &PgPool,
    actor: &ActorContext,
    input: AggregatePolicyInput,
) -> Result<AggregatePolicyVersion, AppError> {
    require_aggregate_policy_manage(actor)?;
    validate_aggregate_policy(&input)?;
    Ok(sqlx::query_as("INSERT INTO academic_aggregate_policy_versions (name,passing_grade,minimum_learner_level,allow_reviewed_holds,approved_by) VALUES ($1,$2,$3,$4,$5) RETURNING id,name,passing_grade::text,minimum_learner_level,allow_reviewed_holds,approved_by,approved_at")
        .bind(input.name.trim()).bind(decimal(&input.passing_grade)?).bind(input.minimum_learner_level).bind(input.allow_reviewed_holds).bind(actor.user_id).fetch_one(pool).await?)
}

pub async fn list_aggregate_policies(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<Vec<AggregatePolicyVersion>, AppError> {
    require_aggregate_read(actor)?;
    let policies: Vec<AggregatePolicyVersion> = sqlx::query_as("SELECT id,name,passing_grade::text,minimum_learner_level,allow_reviewed_holds,approved_by,approved_at FROM academic_aggregate_policy_versions ORDER BY approved_at DESC,id LIMIT 101").fetch_all(pool).await?;
    if policies.len() > 100 {
        return Err(AppError::ValidationError(
            "Aggregate policy history exceeds 100 versions".into(),
        ));
    }
    Ok(policies)
}

pub(super) async fn load_aggregate_policy(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<AggregatePolicyVersion, AppError> {
    sqlx::query_as("SELECT id,name,passing_grade::text,minimum_learner_level,allow_reviewed_holds,approved_by,approved_at FROM academic_aggregate_policy_versions WHERE id=$1").bind(id).fetch_optional(&mut **tx).await?
        .ok_or_else(|| AppError::NotFound("ไม่พบนโยบายผลรวมที่เลือก".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_name_is_bounded_and_threshold_cannot_be_zero() {
        let mut input = AggregatePolicyInput {
            name: "Policy".into(),
            passing_grade: "0".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
        };
        assert!(validate_aggregate_policy(&input).is_err());
        input.passing_grade = "1".into();
        assert!(validate_aggregate_policy(&input).is_ok());
        input.name = "ก".repeat(161);
        assert!(validate_aggregate_policy(&input).is_err());
    }
}
