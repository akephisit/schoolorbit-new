use super::*;
use crate::policy::aggregate::{require_aggregate_policy_manage, require_aggregate_read};
use school_authorization::ActorContext;
use school_database::DbSession;
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

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_results.services.aggregate_policy.create_aggregate_policy",
    )
    .await?;
    create_aggregate_policy_on_connection(&mut connection, actor, input).await
}

pub async fn create_aggregate_policy_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    input: AggregatePolicyInput,
) -> Result<AggregatePolicyVersion, AppError> {
    require_aggregate_policy_manage(actor)?;
    validate_aggregate_policy(&input)?;
    Ok(DbSession::new(connection).query_as("academic_results.services.aggregate_policy.create_aggregate_policy_on_connection", "INSERT INTO academic_aggregate_policy_versions (name,passing_grade,minimum_learner_level,allow_reviewed_holds,approved_by) VALUES ($1,$2,$3,$4,$5) RETURNING id,name,passing_grade::text,minimum_learner_level,allow_reviewed_holds,approved_by,approved_at")
        .bind(input.name.trim()).bind(decimal(&input.passing_grade)?).bind(input.minimum_learner_level).bind(input.allow_reviewed_holds).bind(actor.user_id).fetch_one().await?)
}

pub async fn list_aggregate_policies(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<Vec<AggregatePolicyVersion>, AppError> {
    require_aggregate_read(actor)?;

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_results.services.aggregate_policy.list_aggregate_policies",
    )
    .await?;
    list_aggregate_policies_on_connection(&mut connection, actor).await
}

pub async fn list_aggregate_policies_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
) -> Result<Vec<AggregatePolicyVersion>, AppError> {
    require_aggregate_read(actor)?;
    let policies: Vec<AggregatePolicyVersion> = DbSession::new(connection).query_as("academic_results.services.aggregate_policy.list_aggregate_policies_on_connection", "SELECT id,name,passing_grade::text,minimum_learner_level,allow_reviewed_holds,approved_by,approved_at FROM academic_aggregate_policy_versions ORDER BY approved_at DESC,id LIMIT 101").fetch_all().await?;
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
    DbSession::new(&mut **tx).query_as("academic_results.services.aggregate_policy.load_aggregate_policy", "SELECT id,name,passing_grade::text,minimum_learner_level,allow_reviewed_holds,approved_by,approved_at FROM academic_aggregate_policy_versions WHERE id=$1").bind(id).fetch_optional().await?
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

#[cfg(test)]
mod pool_facade_preflight_tests {
    use super::*;
    async fn closed_pool() -> sqlx::PgPool {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        pool
    }

    #[tokio::test]
    async fn pool_facade_preflight_aggregate_denial_precedes_checkout() {
        let pool = closed_pool().await;
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![],
        };
        assert!(matches!(
            list_aggregate_policies(&pool, &actor).await,
            Err(AppError::Forbidden(_))
        ));
    }
    #[tokio::test]
    async fn pool_facade_preflight_aggregate_validation_precedes_checkout() {
        let pool = closed_pool().await;
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![school_permissions::registry::codes::WILDCARD.into()],
        };
        let input = AggregatePolicyInput {
            name: "Synthetic".into(),
            passing_grade: "not-a-decimal".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
        };
        assert!(matches!(
            create_aggregate_policy(&pool, &actor, input).await,
            Err(AppError::ValidationError(_))
        ));
    }
}
