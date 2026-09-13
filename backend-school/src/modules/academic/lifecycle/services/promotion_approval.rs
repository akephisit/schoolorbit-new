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
use sqlx::types::Json;
use sqlx::PgPool;
use uuid::Uuid;

pub(super) fn intent_checksum(
    run: &PromotionRun,
    items: &[PromotionRunItem],
) -> Result<String, AppError> {
    super::checksum(&(run, items))
}

pub async fn approve_run(
    pool: &PgPool,
    actor: &ActorContext,
    run_id: Uuid,
    input: ApprovePromotionRunInput,
) -> Result<PromotionRun, AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_PROMOTION_READ_SCHOOL,
        codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL,
    ])?;
    if run_id.is_nil()
        || input.request_id.is_nil()
        || input.row_version < 1
        || input.source_checksum.len() != 64
        || !input
            .source_checksum
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(AppError::ValidationError(
            "รอบ รุ่นรายการ รหัสคำขอ หรือข้อมูลยืนยันไม่ถูกต้อง".into(),
        ));
    }
    let request_checksum = super::checksum(&(actor.user_id, run_id, "approve", &input))?;
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(outcome) = super::promotion_runs::replay_command(
        &mut tx,
        input.request_id,
        "approve",
        &request_checksum,
    )
    .await?
    {
        tx.commit().await?;
        return Ok(outcome);
    }
    let run: PromotionRun = sqlx::query_as(&format!(
        "SELECT {RUN_COLUMNS} FROM academic_promotion_runs WHERE id=$1"
    ))
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบเลื่อนชั้น".into()))?;
    if run.row_version != input.row_version || run.status != PromotionRunStatus::Reviewed {
        return Err(AppError::Conflict(
            "รอบนี้ยังตรวจไม่ครบหรือมีการเปลี่ยนแปลง กรุณาตรวจข้อมูลล่าสุดก่อนอนุมัติ".into(),
        ));
    }
    promotion_students::validate_run_years(&mut tx, run.source_year_id, run.target_year_id).await?;
    sqlx::query("SELECT id FROM academic_promotion_runs WHERE id=$1 FOR UPDATE")
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
    let items:Vec<PromotionRunItem>=sqlx::query_as(&format!("SELECT {ITEM_COLUMNS} FROM academic_promotion_run_items WHERE run_id=$1 ORDER BY student_academic_year_id LIMIT 10001 FOR UPDATE"))
        .bind(run_id).fetch_all(&mut *tx).await?;
    if items.is_empty()
        || items.len() > 10000
        || items.iter().any(|item| {
            !matches!(
                item.status,
                PromotionItemStatus::Reviewed | PromotionItemStatus::Executed
            ) || item.decision.is_none()
                || item.reviewed_by.is_none()
        })
    {
        return Err(AppError::Conflict(
            "ต้องตรวจผลของนักเรียนทุกรายให้ครบก่อนอนุมัติรอบ".into(),
        ));
    }
    if intent_checksum(&run, &items)? != input.source_checksum {
        return Err(AppError::Conflict(
            "ผลที่กำลังอนุมัติไม่ตรงกับข้อมูลล่าสุด กรุณาโหลดและตรวจอีกครั้ง".into(),
        ));
    }
    let pending: Vec<_> = items
        .iter()
        .filter(|item| item.status != PromotionItemStatus::Executed)
        .collect();
    if pending.is_empty() {
        return Err(AppError::Conflict("รอบนี้ไม่มีรายการรอดำเนินการ".into()));
    }
    for batch in pending.chunks(500) {
        let ids: Vec<_> = batch
            .iter()
            .map(|item| item.student_academic_year_id)
            .collect();
        let sources = promotion_students::read_promotion_students(
            &mut tx,
            run.source_year_id,
            run.target_year_id,
            &ids,
        )
        .await?;
        let sources: std::collections::BTreeMap<_, _> = sources
            .into_iter()
            .map(|source| (source.student_academic_year_id, source))
            .collect();
        let annual =
            results::services::promotion_annual_sources(&mut tx, run.source_year_id, &ids).await?;
        let mut destinations = Vec::with_capacity(batch.len());
        for item in batch {
            let source = sources
                .get(&item.student_academic_year_id)
                .ok_or_else(|| AppError::NotFound("ไม่พบนักเรียนต้นทาง".into()))?;
            let evidence = annual
                .get(&item.student_academic_year_id)
                .and_then(Option::as_ref)
                .filter(|row| row.is_current)
                .ok_or_else(|| {
                    AppError::Conflict(
                        "ผลรายปีเปลี่ยนแปลงหรือยังไม่ล็อก ต้องสรุปผล คำนวณ และตรวจรายการใหม่".into(),
                    )
                })?;
            if evidence_checksum(run.policy_id, source, Some(evidence))? != item.source_checksum {
                return Err(AppError::Conflict(
                    "ข้อมูลนักเรียนหรือผลรายปีเปลี่ยนหลังตรวจ ต้องคำนวณและตรวจใหม่".into(),
                ));
            }
            let decision = item
                .decision
                .as_ref()
                .ok_or_else(|| AppError::Conflict("ยังไม่ได้เลือกผลของนักเรียน".into()))?;
            super::promotion_decision::validate_decision(
                decision,
                source.grade_level_id,
                &item.recommendation,
            )?;
            destinations.push((source, decision));
        }
        promotion_targets::validate_destinations(&mut tx, run.target_year_id, &destinations)
            .await?;
    }
    let next_version = run
        .row_version
        .checked_add(1)
        .ok_or_else(|| AppError::Conflict("รุ่นข้อมูลเกินขอบเขต".into()))?;
    let intent = PromotionRunCalculation { run, items };
    sqlx::query("INSERT INTO academic_promotion_run_approvals(id,run_id,approved_run_version,source_checksum,intent,approved_by) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(input.request_id).bind(run_id).bind(next_version).bind(&input.source_checksum).bind(Json(&intent)).bind(actor.user_id).execute(&mut *tx).await?;
    let run:PromotionRun=sqlx::query_as(&format!("UPDATE academic_promotion_runs SET status='approved',row_version=$1,approved_by=$2,approved_at=now(),approval_id=$3,updated_at=now() WHERE id=$4 RETURNING {RUN_COLUMNS}"))
        .bind(next_version).bind(actor.user_id).bind(input.request_id).bind(run_id).fetch_one(&mut *tx).await?;
    let audit = ApprovalAudit {
        approval_id: input.request_id,
        row_version: next_version,
        item_count: intent.items.len(),
    };
    sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,actor_user_id,payload) VALUES('promotion_run.approved','promotion_run',$1,$2,$3)")
        .bind(run_id).bind(actor.user_id).bind(Json(audit)).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO academic_promotion_run_commands(request_id,run_id,actor_user_id,action,request_checksum,outcome) VALUES($1,$2,$3,'approve',$4,$5)")
        .bind(input.request_id).bind(run_id).bind(actor.user_id).bind(request_checksum).bind(Json(&run)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(run)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalAudit {
    approval_id: Uuid,
    row_version: i64,
    item_count: usize,
}

#[cfg(test)]
#[path = "promotion_approval_tests.rs"]
mod tests;
