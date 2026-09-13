use sqlx::{types::Json, PgPool};
use uuid::Uuid;

use super::super::models::PromotionPolicyOptions;
use super::super::models::{PromotionPolicyInput, PromotionPolicyVersion, PromotionRuleInput};

pub async fn get_promotion_policy_options(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<PromotionPolicyOptions, AppError> {
    actor.require_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let options =
        crate::modules::academic::core::services::promotion_context::policy_options(&mut tx)
            .await?;
    tx.commit().await?;
    Ok(options)
}
use crate::{
    error::AppError, middleware::permission::ActorContext,
    modules::academic::core::services::promotion_context::validate_promotion_rules,
    permissions::registry::codes,
};

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
    actor.require_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)?;
    let rows: Vec<StoredPolicy> = sqlx::query_as("SELECT id,name,rules,progression_row_version,reviewed_by,reviewed_at FROM academic_promotion_policy_versions ORDER BY reviewed_at DESC,id LIMIT 101")
        .fetch_all(pool).await?;
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
    let mut tx = pool.begin().await?;
    let revision = validate_promotion_rules(&mut tx, &input.rules).await?;
    let stored: StoredPolicy = sqlx::query_as("INSERT INTO academic_promotion_policy_versions (id,name,rules,progression_row_version,reviewed_by) VALUES ($1,$2,$3,$4,$5) RETURNING id,name,rules,progression_row_version,reviewed_by,reviewed_at")
        .bind(Uuid::new_v4()).bind(input.name.trim()).bind(Json(&input.rules))
        .bind(revision).bind(actor.user_id).fetch_one(&mut *tx).await?;
    sqlx::query("INSERT INTO academic_audit_events (event_code,entity_type,entity_id,actor_user_id,payload) VALUES ('promotion_policy.approved','promotion_policy',$1,$2,$3)")
        .bind(stored.id).bind(actor.user_id)
        .bind(Json(serde_json::json!({"progressionRowVersion":revision,"ruleCount":input.rules.len()})))
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(stored.into())
}

#[cfg(test)]
#[path = "promotion_policy_tests.rs"]
mod tests;
