use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

use super::super::models::PromotionPolicyOptions;
use super::super::models::{PromotionPolicyInput, PromotionPolicyVersion, PromotionRuleInput};

pub async fn get_promotion_policy_options(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<PromotionPolicyOptions, AppError> {
    actor.require_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_lifecycle.services.promotion_policy.get_promotion_policy_options",
    )
    .await?;
    DbSession::new(&mut *tx)
        .query(
            "academic_lifecycle.services.promotion_policy.get_promotion_policy_options",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let options = super::promotion_context::policy_options(&mut tx).await?;
    tx.commit().await?;
    Ok(options)
}
use super::promotion_context::validate_promotion_rules;
use school_permissions::registry::codes;

#[derive(sqlx::FromRow)]
struct StoredPolicy {
    id: Uuid,
    name: String,
    rules: Json<Vec<PromotionRuleInput>>,
    progression_row_version: i64,
    reviewed_by: Uuid,
    reviewed_at: chrono::DateTime<chrono::Utc>,
}

impl From<StoredPolicy> for PromotionPolicyVersion {
    fn from(row: StoredPolicy) -> Self {
        Self {
            id: row.id,
            name: row.name,
            rules: row.rules.0,
            progression_row_version: row.progression_row_version,
            reviewed_by: row.reviewed_by,
            reviewed_at: row.reviewed_at,
        }
    }
}

pub async fn list_promotion_policies(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<Vec<PromotionPolicyVersion>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_lifecycle.services.promotion_policy.list_promotion_policies",
    )
    .await?;
    list_promotion_policies_on_connection(&mut connection, actor).await
}

pub async fn list_promotion_policies_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
) -> Result<Vec<PromotionPolicyVersion>, AppError> {
    actor.require_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)?;
    let rows: Vec<StoredPolicy> = DbSession::new(connection).query_as("academic_lifecycle.services.promotion_policy.list_promotion_policies_on_connection", "SELECT id,name,rules,progression_row_version,reviewed_by,reviewed_at FROM academic_promotion_policy_versions ORDER BY reviewed_at DESC,id LIMIT 101")
        .fetch_all().await?;
    if rows.len() > 100 {
        return Err(AppError::ValidationError(
            "ประวัติเกณฑ์เกิน 100 รุ่น กรุณาใช้การแบ่งหน้าก่อนเรียกประวัติทั้งหมด".into(),
        ));
    }
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn create_promotion_policy(
    pool: &PgPool,
    actor: &ActorContext,
    input: PromotionPolicyInput,
) -> Result<PromotionPolicyVersion, AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_PROMOTION_READ_SCHOOL,
        codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL,
        codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL,
    ])?;
    super::promotion_recommendation::validate_policy(&input)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_lifecycle.services.promotion_policy.create_promotion_policy",
    )
    .await?;
    let revision = validate_promotion_rules(&mut tx, &input.rules).await?;
    let stored: StoredPolicy = DbSession::new(&mut *tx).query_as("academic_lifecycle.services.promotion_policy.create_promotion_policy", "INSERT INTO academic_promotion_policy_versions (id,name,rules,progression_row_version,reviewed_by) VALUES ($1,$2,$3,$4,$5) RETURNING id,name,rules,progression_row_version,reviewed_by,reviewed_at")
        .bind(Uuid::now_v7()).bind(input.name.trim()).bind(Json(&input.rules))
        .bind(revision).bind(actor.user_id).fetch_one().await?;
    DbSession::new(&mut *tx).query("academic_lifecycle.services.promotion_policy.create_promotion_policy", "INSERT INTO academic_audit_events (event_code,entity_type,entity_id,actor_user_id,payload) VALUES ('promotion_policy.approved','promotion_policy',$1,$2,$3)")
        .bind(stored.id).bind(actor.user_id)
        .bind(Json(serde_json::json!({"progressionRowVersion":revision,"ruleCount":input.rules.len()})))
        .execute().await?;
    tx.commit().await?;
    Ok(stored.into())
}
