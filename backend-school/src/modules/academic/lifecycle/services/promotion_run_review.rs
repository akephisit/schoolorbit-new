use super::super::models::*;
use super::{
    promotion_calculation::{evidence_checksum, ITEM_COLUMNS},
    promotion_runs::RUN_COLUMNS,
};
use crate::{error::AppError, middleware::permission::ActorContext};
use crate::{
    modules::academic::{
        core::services::{lifecycle_guard, promotion_students, promotion_targets},
        results,
    },
    permissions::registry::codes,
};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn review_item(
    pool: &PgPool,
    actor: &ActorContext,
    run_id: Uuid,
    item_id: Uuid,
    input: ReviewPromotionItemInput,
) -> Result<PromotionItemReview, AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_PROMOTION_READ_SCHOOL,
        codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL,
    ])?;
    if run_id.is_nil() || item_id.is_nil() || input.row_version < 1 {
        return Err(AppError::ValidationError(
            "รอบ รายการ หรือรุ่นข้อมูลไม่ถูกต้อง".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    let run: PromotionRun = sqlx::query_as(&format!(
        "SELECT {RUN_COLUMNS} FROM academic_promotion_runs WHERE id=$1"
    ))
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบเลื่อนชั้น".into()))?;
    if matches!(
        run.status,
        PromotionRunStatus::Draft | PromotionRunStatus::Executing | PromotionRunStatus::Completed
    ) {
        return Err(AppError::Conflict(
            "รอบนี้ยังไม่คำนวณ กำลังดำเนินการ หรือเสร็จแล้ว".into(),
        ));
    }
    promotion_students::validate_run_years(&mut tx, run.source_year_id, run.target_year_id).await?;
    sqlx::query("SELECT id FROM academic_promotion_runs WHERE id=$1 FOR UPDATE")
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
    let item:PromotionRunItem=sqlx::query_as(&format!("SELECT {ITEM_COLUMNS} FROM academic_promotion_run_items WHERE id=$1 AND run_id=$2 FOR UPDATE"))
        .bind(item_id).bind(run_id).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::NotFound("ไม่พบรายการในรอบเลื่อนชั้นนี้".into()))?;
    if item.row_version != input.row_version || item.status == PromotionItemStatus::Executed {
        return Err(AppError::Conflict(
            "รายการเปลี่ยนแปลงหรือดำเนินการแล้ว กรุณาโหลดข้อมูลล่าสุด".into(),
        ));
    }
    let source = promotion_students::read_promotion_students(
        &mut tx,
        run.source_year_id,
        run.target_year_id,
        &[item.student_academic_year_id],
    )
    .await?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::NotFound("ไม่พบนักเรียนต้นทาง".into()))?;
    let annual = results::services::promotion_annual_sources(
        &mut tx,
        run.source_year_id,
        &[item.student_academic_year_id],
    )
    .await?
    .remove(&item.student_academic_year_id)
    .flatten()
    .filter(|row| row.is_current)
    .ok_or_else(|| {
        AppError::Conflict("ผลรายปียังไม่ล็อกหรือเปลี่ยนแปลงแล้ว กรุณาสรุปผลและคำนวณรอบใหม่".into())
    })?;
    if evidence_checksum(run.policy_id, &source, Some(&annual))? != item.source_checksum {
        return Err(AppError::Conflict(
            "ข้อมูลต้นทางเปลี่ยนหลังคำนวณ ต้องคำนวณรอบใหม่ก่อนตรวจรายการ".into(),
        ));
    }
    super::promotion_decision::validate_decision(
        &input.decision,
        source.grade_level_id,
        &item.recommendation,
    )?;
    promotion_targets::validate_destination(&mut tx, &source, run.target_year_id, &input.decision)
        .await?;
    let item:PromotionRunItem=sqlx::query_as(&format!("UPDATE academic_promotion_run_items SET decision=$1,reviewed_by=$2,reviewed_at=now(),status='reviewed',row_version=row_version+1,updated_at=now() WHERE id=$3 RETURNING {ITEM_COLUMNS}"))
        .bind(sqlx::types::Json(&input.decision)).bind(actor.user_id).bind(item_id).fetch_one(&mut *tx).await?;
    let all_reviewed:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM academic_promotion_run_items WHERE run_id=$1 AND status NOT IN ('reviewed','executed'))")
        .bind(run_id).fetch_one(&mut *tx).await?;
    let run:PromotionRun=sqlx::query_as(&format!("UPDATE academic_promotion_runs SET status=$1,row_version=row_version+1,reviewed_by=CASE WHEN $2 THEN $3 ELSE NULL END,reviewed_at=CASE WHEN $2 THEN now() ELSE NULL END,approved_by=NULL,approved_at=NULL,approval_id=NULL,updated_at=now() WHERE id=$4 RETURNING {RUN_COLUMNS}"))
        .bind(if all_reviewed {PromotionRunStatus::Reviewed}else{PromotionRunStatus::Calculated}).bind(all_reviewed).bind(actor.user_id).bind(run_id).fetch_one(&mut *tx).await?;
    let audit = ReviewAudit {
        run_id,
        row_version: item.row_version,
        decision: &input.decision,
    };
    sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,actor_user_id,payload) VALUES('promotion_item.reviewed','promotion_item',$1,$2,$3)")
        .bind(item_id).bind(actor.user_id).bind(sqlx::types::Json(audit)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(PromotionItemReview { run, item })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewAudit<'a> {
    run_id: Uuid,
    row_version: i64,
    decision: &'a PromotionDecisionInput,
}

#[cfg(test)]
#[path = "promotion_review_tests.rs"]
pub(super) mod tests;
