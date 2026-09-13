use super::super::models::*;
use super::{
    promotion_calculation::{evidence_checksum, ITEM_COLUMNS},
    promotion_runs::RUN_COLUMNS,
};
use crate::{error::AppError, middleware::permission::ActorContext};
use crate::{
    modules::academic::{
        core::services::{
            lifecycle_guard, promotion_execution as core_execution, promotion_students,
        },
        results,
    },
    permissions::registry::codes,
};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;

const RECEIPT_COLUMNS: &str = "item_id,run_id,request_id,approval_id,target_student_year_id,target_placement_id,source_row_version,executed_by,executed_at";

#[derive(Debug, sqlx::FromRow)]
struct Batch {
    request_id: Uuid,
    run_id: Uuid,
    approval_id: Uuid,
    actor_user_id: Uuid,
    request_checksum: String,
    item_ids: Vec<Uuid>,
}

pub async fn execute_run(
    pool: &PgPool,
    actor: &ActorContext,
    run_id: Uuid,
    input: ExecutePromotionRunInput,
) -> Result<PromotionExecutionResult, AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_PROMOTION_READ_SCHOOL,
        codes::ACADEMIC_PROMOTION_EXECUTE_SCHOOL,
    ])?;
    if run_id.is_nil()
        || input.request_id.is_nil()
        || input.row_version < 1
        || !(1..=100).contains(&input.limit)
    {
        return Err(AppError::ValidationError(
            "ระบุรอบ รุ่นข้อมูล รหัสคำขอ และจำนวน 1–100 รายการ".into(),
        ));
    }
    let checksum = super::checksum(&(actor.user_id, run_id, "execute", &input))?;
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(result) =
        super::promotion_runs::replay_command(&mut tx, input.request_id, "execute", &checksum)
            .await?
    {
        tx.commit().await?;
        return Ok(result);
    }
    let batch = start_batch(&mut tx, actor, run_id, &input, &checksum).await?;
    tx.commit().await?;
    let mut failures = Vec::new();
    for item_id in &batch.item_ids {
        if let Err(error) = execute_item(pool, &batch, *item_id).await {
            // No raw database details or student identifiers enter the failure text.
            let message = error.public_message().to_owned();
            record_failure(pool, &batch, *item_id, &message).await?;
            failures.push(PromotionExecutionFailure {
                item_id: *item_id,
                message,
            });
        }
    }
    finish_batch(pool, &batch, failures).await
}

async fn read_run(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<PromotionRun, AppError> {
    sqlx::query_as(&format!(
        "SELECT {RUN_COLUMNS} FROM academic_promotion_runs WHERE id=$1"
    ))
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบเลื่อนชั้น".into()))
}

async fn start_batch(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    run_id: Uuid,
    input: &ExecutePromotionRunInput,
    checksum: &str,
) -> Result<Batch, AppError> {
    let previous:Option<Batch>=sqlx::query_as("SELECT request_id,run_id,approval_id,actor_user_id,request_checksum,item_ids FROM academic_promotion_execution_batches WHERE request_id=$1")
        .bind(input.request_id).fetch_optional(&mut **tx).await?;
    let run = read_run(tx, run_id).await?;
    if let Some(batch) = previous {
        if batch.run_id != run_id
            || batch.actor_user_id != actor.user_id
            || batch.request_checksum != checksum
            || run.approval_id != Some(batch.approval_id)
        {
            return Err(AppError::Conflict(
                "คำขอหรือการอนุมัติเปลี่ยนไปแล้ว ต้องตรวจรอบล่าสุด".into(),
            ));
        }
        return Ok(batch);
    }
    if run.row_version != input.row_version
        || !matches!(
            run.status,
            PromotionRunStatus::Approved
                | PromotionRunStatus::Executing
                | PromotionRunStatus::Failed
        )
    {
        return Err(AppError::Conflict("รอบยังไม่อนุมัติหรือรุ่นข้อมูลเปลี่ยนแล้ว".into()));
    }
    let approval_id = run
        .approval_id
        .ok_or_else(|| AppError::Conflict("ต้องอนุมัติรอบก่อนดำเนินการ".into()))?;
    promotion_students::validate_run_years(tx, run.source_year_id, run.target_year_id).await?;
    sqlx::query("SELECT id FROM academic_promotion_runs WHERE id=$1 FOR UPDATE")
        .bind(run_id)
        .execute(&mut **tx)
        .await?;
    let mut item_ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM academic_promotion_run_items WHERE run_id=$1 AND status IN ('reviewed','failed') ORDER BY student_academic_year_id LIMIT $2 FOR UPDATE")
        .bind(run_id).bind(i64::from(input.limit)).fetch_all(&mut **tx).await?;
    if item_ids.is_empty() && run.status == PromotionRunStatus::Executing {
        // A reload can lose the old request ID after the last item committed.
        // Reuse owned receipts only to finish metadata, never repeat Core writes.
        item_ids=sqlx::query_scalar("SELECT item.id FROM academic_promotion_run_items item JOIN academic_promotion_execution_receipts receipt ON receipt.item_id=item.id AND receipt.run_id=item.run_id WHERE item.run_id=$1 AND NOT EXISTS(SELECT 1 FROM academic_promotion_run_items pending LEFT JOIN academic_promotion_execution_receipts completed ON completed.item_id=pending.id AND completed.run_id=pending.run_id WHERE pending.run_id=$1 AND (pending.status<>'executed' OR completed.item_id IS NULL)) ORDER BY item.student_academic_year_id LIMIT $2")
            .bind(run_id).bind(i64::from(input.limit)).fetch_all(&mut **tx).await?;
    }
    if item_ids.is_empty() {
        return Err(AppError::Conflict("ไม่มีรายการที่อนุมัติรอดำเนินการ".into()));
    }
    sqlx::query("INSERT INTO academic_promotion_execution_batches(request_id,run_id,approval_id,actor_user_id,request_checksum,item_ids) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(input.request_id).bind(run_id).bind(approval_id).bind(actor.user_id).bind(checksum).bind(&item_ids).execute(&mut **tx).await?;
    sqlx::query("UPDATE academic_promotion_runs SET status='executing',row_version=row_version+1,updated_at=now() WHERE id=$1").bind(run_id).execute(&mut **tx).await?;
    Ok(Batch {
        request_id: input.request_id,
        run_id,
        approval_id,
        actor_user_id: actor.user_id,
        request_checksum: checksum.into(),
        item_ids,
    })
}

async fn execute_item(pool: &PgPool, batch: &Batch, item_id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    let run = read_run(&mut tx, batch.run_id).await?;
    if run.approval_id != Some(batch.approval_id) {
        return Err(AppError::Conflict("การอนุมัติเดิมไม่ใช่รุ่นล่าสุดแล้ว".into()));
    }
    let completed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_promotion_execution_receipts WHERE item_id=$1 AND run_id=$2)").bind(item_id).bind(batch.run_id).fetch_one(&mut *tx).await?;
    if completed {
        tx.commit().await?;
        return Ok(());
    }
    promotion_students::validate_run_years(&mut tx, run.source_year_id, run.target_year_id).await?;
    sqlx::query("SELECT id FROM academic_promotion_runs WHERE id=$1 FOR UPDATE")
        .bind(run.id)
        .execute(&mut *tx)
        .await?;
    let item:PromotionRunItem=sqlx::query_as(&format!("SELECT {ITEM_COLUMNS} FROM academic_promotion_run_items WHERE id=$1 AND run_id=$2 FOR UPDATE"))
        .bind(item_id).bind(run.id).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::NotFound("ไม่พบรายการในรอบ".into()))?;
    if !matches!(
        item.status,
        PromotionItemStatus::Reviewed | PromotionItemStatus::Failed
    ) {
        return Err(AppError::Conflict(
            "รายการนี้ไม่ได้อยู่ในสถานะพร้อมดำเนินการ".into(),
        ));
    }
    let Json(approved): Json<PromotionRunItem> = sqlx::query_scalar(
        "SELECT item FROM academic_promotion_run_approvals approval CROSS JOIN LATERAL jsonb_array_elements(approval.intent->'items') item WHERE approval.id=$1 AND approval.run_id=$2 AND item->>'id'=$3",
    )
    .bind(batch.approval_id)
    .bind(run.id)
    .bind(item_id.to_string())
    .fetch_optional(&mut *tx)
    .await?
        .ok_or_else(|| AppError::Conflict("รายการนี้ไม่อยู่ในการอนุมัติเดิม".into()))?;
    if approved.source_checksum != item.source_checksum
        || super::checksum(&approved.decision)? != super::checksum(&item.decision)?
    {
        return Err(AppError::Conflict("ผลการพิจารณาเปลี่ยนหลังอนุมัติ".into()));
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
    .ok_or_else(|| AppError::Conflict("ผลรายปีเปลี่ยนหรือยังไม่ล็อก ต้องคำนวณและตรวจใหม่".into()))?;
    if evidence_checksum(run.policy_id, &source, Some(&annual))? != approved.source_checksum {
        return Err(AppError::Conflict("ข้อมูลนักเรียนหรือผลรายปีเปลี่ยนหลังอนุมัติ".into()));
    }
    let decision = approved
        .decision
        .as_ref()
        .ok_or_else(|| AppError::Conflict("ยังไม่มีผลพิจารณาที่อนุมัติ".into()))?;
    super::promotion_decision::validate_decision(
        decision,
        source.grade_level_id,
        &approved.recommendation,
    )?;
    let outcome = core_execution::execute_decision(
        &mut tx,
        batch.actor_user_id,
        run.source_year_id,
        run.target_year_id,
        &source,
        decision,
    )
    .await?;
    sqlx::query("INSERT INTO academic_promotion_execution_receipts(item_id,run_id,request_id,approval_id,source_year_id,target_year_id,student_id,target_student_year_id,target_placement_id,source_row_version,executed_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(item_id).bind(run.id).bind(batch.request_id).bind(batch.approval_id).bind(run.source_year_id).bind(run.target_year_id).bind(item.student_id)
        .bind(outcome.target_student_year_id).bind(outcome.target_placement_id).bind(outcome.source_row_version).bind(batch.actor_user_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE academic_promotion_run_items SET status='executed',row_version=row_version+1,updated_at=now() WHERE id=$1").bind(item_id).execute(&mut *tx).await?;
    audit(&mut tx, "promotion_item.executed", item_id, batch, &outcome).await?;
    tx.commit().await?;
    Ok(())
}

async fn record_failure(
    pool: &PgPool,
    batch: &Batch,
    item_id: Uuid,
    message: &str,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    let run = read_run(&mut tx, batch.run_id).await?;
    if run.approval_id != Some(batch.approval_id) {
        return Err(AppError::Conflict(
            "รอบถูกตรวจหรืออนุมัติใหม่แล้ว กรุณาโหลดข้อมูลล่าสุด".into(),
        ));
    }
    let changed=sqlx::query("UPDATE academic_promotion_run_items SET status='failed',updated_at=now() WHERE id=$1 AND run_id=$2 AND status IN ('reviewed','failed')")
        .bind(item_id).bind(batch.run_id).execute(&mut *tx).await?;
    if changed.rows_affected() > 0 {
        audit(
            &mut tx,
            "promotion_item.failed",
            item_id,
            batch,
            &PromotionExecutionFailure {
                item_id,
                message: message.into(),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn finish_batch(
    pool: &PgPool,
    batch: &Batch,
    failures: Vec<PromotionExecutionFailure>,
) -> Result<PromotionExecutionResult, AppError> {
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(result) = super::promotion_runs::replay_command(
        &mut tx,
        batch.request_id,
        "execute",
        &batch.request_checksum,
    )
    .await?
    {
        tx.commit().await?;
        return Ok(result);
    }
    let current = read_run(&mut tx, batch.run_id).await?;
    if current.approval_id != Some(batch.approval_id) {
        return Err(AppError::Conflict(
            "การอนุมัติรอบเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".into(),
        ));
    }
    let (remaining_count,hold_count,failed_count):(i64,i64,i64)=sqlx::query_as("SELECT count(*) FILTER(WHERE status<>'executed'),count(*) FILTER(WHERE status='executed' AND decision->>'outcome'='hold'),count(*) FILTER(WHERE status='failed') FROM academic_promotion_run_items WHERE run_id=$1")
        .bind(batch.run_id).fetch_one(&mut *tx).await?;
    let status = if remaining_count == 0 {
        PromotionRunStatus::Completed
    } else if failed_count > 0 {
        PromotionRunStatus::Failed
    } else {
        PromotionRunStatus::Executing
    };
    let run: PromotionRun = if current.status == PromotionRunStatus::Completed {
        if remaining_count != 0 {
            return Err(AppError::Conflict(
                "สถานะรอบไม่ตรงกับผลดำเนินการ กรุณาตรวจสอบข้อมูล".into(),
            ));
        }
        current
    } else {
        sqlx::query_as(&format!("UPDATE academic_promotion_runs SET status=$1,row_version=row_version+1,updated_at=now(),executed_by=CASE WHEN $2 THEN $3 ELSE executed_by END,executed_at=CASE WHEN $2 THEN now() ELSE executed_at END WHERE id=$4 RETURNING {RUN_COLUMNS}"))
        .bind(status).bind(remaining_count==0).bind(batch.actor_user_id).bind(batch.run_id).fetch_one(&mut *tx).await?
    };
    let receipts=sqlx::query_as(&format!("SELECT {RECEIPT_COLUMNS} FROM academic_promotion_execution_receipts WHERE run_id=$1 AND item_id=ANY($2) ORDER BY item_id"))
        .bind(batch.run_id).bind(&batch.item_ids).fetch_all(&mut *tx).await?;
    let result = PromotionExecutionResult {
        run,
        receipts,
        failures,
        remaining_count,
        hold_count,
    };
    audit(
        &mut tx,
        "promotion_run.execution_batch_completed",
        batch.run_id,
        batch,
        &result,
    )
    .await?;
    sqlx::query("INSERT INTO academic_promotion_run_commands(request_id,run_id,actor_user_id,action,request_checksum,outcome) VALUES($1,$2,$3,'execute',$4,$5)")
        .bind(batch.request_id).bind(batch.run_id).bind(batch.actor_user_id).bind(&batch.request_checksum).bind(Json(&result)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(result)
}

async fn audit<T: serde::Serialize>(
    tx: &mut Transaction<'_, Postgres>,
    event: &str,
    entity: Uuid,
    batch: &Batch,
    payload: &T,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,actor_user_id,payload) VALUES($1,'promotion_execution',$2,$3,$4)")
        .bind(event).bind(entity).bind(batch.actor_user_id).bind(Json(payload)).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
#[path = "promotion_execution_tests.rs"]
pub(super) mod tests;
