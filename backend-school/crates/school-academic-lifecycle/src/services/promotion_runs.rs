use super::super::models::{CreatePromotionRunInput, PromotionRun};
use school_academic_core::services::{lifecycle_guard, promotion_students};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;

pub(super) const RUN_COLUMNS: &str = "id,source_year_id,target_year_id,target_placement_starts_on,policy_id,status,row_version,created_by,created_at,updated_at,reviewed_by,reviewed_at,approved_by,approved_at,approval_id,executed_by,executed_at";

/// Validate the immutable receipt's action and actor-bound intent before decoding
/// its action-specific outcome. A nonce collision is a conflict, not a JSON error.
#[doc(hidden)]
pub async fn replay_command<T: serde::de::DeserializeOwned + Send + Unpin + 'static>(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    request_id: uuid::Uuid,
    action: &str,
    checksum: &str,
) -> Result<Option<T>, AppError> {
    let previous: Option<(String, String)> = DbSession::new(&mut **tx).query_as("academic_lifecycle.services.promotion_runs.replay_command",
        "SELECT action,request_checksum FROM academic_promotion_run_commands WHERE request_id=$1",
    )
    .bind(request_id)
    .fetch_optional()
    .await?;
    let Some((original_action, original_checksum)) = previous else {
        let reserved: Option<String> = DbSession::new(&mut **tx).scalar("academic_lifecycle.services.promotion_runs.replay_command",
            "SELECT request_checksum FROM academic_promotion_execution_batches WHERE request_id=$1",
        )
        .bind(request_id)
        .fetch_optional()
        .await?;
        if reserved.is_some_and(|original| action != "execute" || original != checksum) {
            return Err(AppError::Conflict(
                "รหัสคำขอนี้ถูกใช้กับการดำเนินการเลื่อนชั้นแล้ว".into(),
            ));
        }
        return Ok(None);
    };
    if original_action != action || original_checksum != checksum {
        return Err(AppError::Conflict(
            "รหัสคำขอนี้เคยใช้กับข้อมูลหรือผู้ดำเนินการอื่นแล้ว".into(),
        ));
    }
    let outcome: sqlx::types::Json<T> = DbSession::new(&mut **tx)
        .scalar(
            "academic_lifecycle.services.promotion_runs.replay_command",
            "SELECT outcome FROM academic_promotion_run_commands WHERE request_id=$1",
        )
        .bind(request_id)
        .fetch_one()
        .await?;
    Ok(Some(outcome.0))
}

pub async fn create_run(
    pool: &PgPool,
    actor: &ActorContext,
    input: CreatePromotionRunInput,
) -> Result<PromotionRun, AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_PROMOTION_READ_SCHOOL,
        codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL,
    ])?;
    if [
        input.request_id,
        input.source_year_id,
        input.target_year_id,
        input.policy_id,
    ]
    .into_iter()
    .any(|id| id.is_nil())
        || input.source_year_id == input.target_year_id
    {
        return Err(AppError::ValidationError(
            "ระบุปีต้นทาง ปีปลายทาง เกณฑ์ และรหัสคำขอให้ถูกต้อง".into(),
        ));
    }
    let checksum = super::checksum(&(actor.user_id, &input))?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_lifecycle.services.promotion_runs.create_run",
    )
    .await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    let previous: Option<(uuid::Uuid, String)> = DbSession::new(&mut *tx)
        .query_as(
            "academic_lifecycle.services.promotion_runs.create_run",
            "SELECT id,request_checksum FROM academic_promotion_runs WHERE request_id=$1",
        )
        .bind(input.request_id)
        .fetch_optional()
        .await?;
    if let Some((id, original)) = previous {
        if original != checksum {
            return Err(AppError::Conflict(
                "รหัสคำขอนี้เคยใช้กับข้อมูลหรือผู้ดำเนินการอื่นแล้ว".into(),
            ));
        }
        let run = DbSession::new(&mut *tx)
            .query_as(
                "academic_lifecycle.services.promotion_runs.create_run",
                sqlx::AssertSqlSafe(format!(
                    "SELECT {RUN_COLUMNS} FROM academic_promotion_runs WHERE id=$1"
                )),
            )
            .bind(id)
            .fetch_one()
            .await?;
        tx.commit().await?;
        return Ok(run);
    }
    promotion_students::validate_run_years(&mut tx, input.source_year_id, input.target_year_id)
        .await?;
    let exists: bool = DbSession::new(&mut *tx)
        .scalar(
            "academic_lifecycle.services.promotion_runs.create_run",
            "SELECT EXISTS(SELECT 1 FROM academic_promotion_policy_versions WHERE id=$1)",
        )
        .bind(input.policy_id)
        .fetch_one()
        .await?;
    if !exists {
        return Err(AppError::NotFound("ไม่พบเกณฑ์เลื่อนชั้นที่ยืนยันแล้ว".into()));
    }
    let run:PromotionRun=DbSession::new(&mut *tx).query_as("academic_lifecycle.services.promotion_runs.create_run", sqlx::AssertSqlSafe(format!("INSERT INTO academic_promotion_runs(source_year_id,target_year_id,policy_id,request_id,request_checksum,created_by,target_placement_starts_on) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING {RUN_COLUMNS}")))
        .bind(input.source_year_id).bind(input.target_year_id).bind(input.policy_id)
        .bind(input.request_id).bind(checksum).bind(actor.user_id).bind(input.target_placement_starts_on).fetch_one().await?;
    DbSession::new(&mut *tx).query("academic_lifecycle.services.promotion_runs.create_run", "INSERT INTO academic_audit_events(event_code,entity_type,entity_id,actor_user_id,payload) VALUES ('promotion_run.created','promotion_run',$1,$2,$3)")
        .bind(run.id).bind(actor.user_id).bind(sqlx::types::Json(serde_json::json!({"sourceYearId":input.source_year_id,"targetYearId":input.target_year_id,"policyId":input.policy_id})))
        .execute().await?;
    tx.commit().await?;
    Ok(run)
}
