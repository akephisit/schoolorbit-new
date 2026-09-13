use super::super::models::*;
use super::{
    promotion_calculation::{evidence_checksum, ITEM_COLUMNS},
    promotion_runs::RUN_COLUMNS,
};
use crate::{error::AppError, middleware::permission::ActorContext};
use crate::{
    modules::academic::{
        core::services::{promotion_students, year_transitions},
        results,
    },
    permissions::registry::codes,
};
use sqlx::PgPool;
use std::collections::BTreeMap;
use uuid::Uuid;

pub async fn list_runs(
    pool: &PgPool,
    actor: &ActorContext,
    query: PromotionRunListQuery,
) -> Result<PromotionRunList, AppError> {
    actor.require_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)?;
    if query.source_year_id.is_nil()
        || query.target_year_id.is_some_and(|id| id.is_nil())
        || query.before_id.is_some_and(|id| id.is_nil())
    {
        return Err(AppError::ValidationError(
            "ระบุปีและตำแหน่งรายการให้ถูกต้อง".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    year_transitions::read_context(&mut tx, query.source_year_id).await?;
    if let Some(year) = query.target_year_id {
        year_transitions::read_context(&mut tx, year).await?;
    }
    let cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)> = if let Some(id) = query.before_id {
        Some(sqlx::query_as("SELECT created_at,id FROM academic_promotion_runs WHERE id=$1 AND source_year_id=$2 AND ($3::uuid IS NULL OR target_year_id=$3)")
            .bind(id).bind(query.source_year_id).bind(query.target_year_id).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::NotFound("ไม่พบตำแหน่งรายการในปีที่เลือก".into()))?)
    } else {
        None
    };
    let mut runs:Vec<PromotionRun>=sqlx::query_as(&format!("SELECT {RUN_COLUMNS} FROM academic_promotion_runs WHERE source_year_id=$1 AND ($2::uuid IS NULL OR target_year_id=$2) AND ($3::timestamptz IS NULL OR (created_at,id)<($3,$4)) ORDER BY created_at DESC,id DESC LIMIT 51"))
        .bind(query.source_year_id).bind(query.target_year_id).bind(cursor.map(|row|row.0)).bind(cursor.map(|row|row.1)).fetch_all(&mut *tx).await?;
    let has_more = runs.len() > 50;
    runs.truncate(50);
    let next_cursor = has_more.then(|| runs.last().map(|run| run.id)).flatten();
    tx.commit().await?;
    Ok(PromotionRunList { runs, next_cursor })
}

pub async fn get_run_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    run_id: Uuid,
) -> Result<PromotionRunWorkspace, AppError> {
    actor.require_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)?;
    if run_id.is_nil() {
        return Err(AppError::ValidationError("ระบุรอบเลื่อนชั้น".into()));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let run: PromotionRun = sqlx::query_as(&format!(
        "SELECT {RUN_COLUMNS} FROM academic_promotion_runs WHERE id=$1"
    ))
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบเลื่อนชั้น".into()))?;
    let source_year = year_transitions::read_context(&mut tx, run.source_year_id).await?;
    let target_year = year_transitions::read_context(&mut tx, run.target_year_id).await?;
    let items:Vec<PromotionRunItem>=sqlx::query_as(&format!("SELECT {ITEM_COLUMNS} FROM academic_promotion_run_items WHERE run_id=$1 ORDER BY student_academic_year_id LIMIT 10001"))
        .bind(run_id).fetch_all(&mut *tx).await?;
    if items.len() > 10000 {
        return Err(AppError::ValidationError(
            "รอบมีนักเรียนเกิน 10000 รายการ".into(),
        ));
    }
    let approval_checksum = super::promotion_approval::intent_checksum(&run, &items)?;
    let receipts:Vec<PromotionExecutionReceipt>=sqlx::query_as("SELECT item_id,run_id,request_id,approval_id,target_student_year_id,target_placement_id,source_row_version,executed_by,executed_at FROM academic_promotion_execution_receipts WHERE run_id=$1 ORDER BY item_id")
        .bind(run_id).fetch_all(&mut *tx).await?;
    let mut receipts: BTreeMap<_, _> = receipts.into_iter().map(|row| (row.item_id, row)).collect();
    let mut students = Vec::with_capacity(items.len());
    for batch in items.chunks(500) {
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
        let mut sources: BTreeMap<_, _> = sources
            .into_iter()
            .map(|row| (row.student_academic_year_id, row))
            .collect();
        let annual =
            results::services::promotion_annual_sources(&mut tx, run.source_year_id, &ids).await?;
        for item in batch {
            let source = sources
                .remove(&item.student_academic_year_id)
                .ok_or_else(|| AppError::NotFound("ไม่พบนักเรียนต้นทาง".into()))?;
            let annual = annual
                .get(&item.student_academic_year_id)
                .and_then(Option::as_ref);
            let annual_result_current =
                annual.is_some_and(|row| row.is_current && Some(row.id) == item.annual_revision_id);
            let needs_recalculation = item.status != PromotionItemStatus::Executed
                && (!annual_result_current
                    || evidence_checksum(run.policy_id, &source, annual)? != item.source_checksum);
            students.push(PromotionRunStudent {
                item: item.clone(),
                student_code: source.student_code,
                student_name: source.student_name,
                annual_result_current,
                needs_recalculation,
                receipt: receipts.remove(&item.id),
            });
        }
    }
    tx.commit().await?;
    Ok(PromotionRunWorkspace {
        run,
        source_year,
        target_year,
        students,
        approval_checksum,
    })
}

#[cfg(test)]
#[path = "promotion_workspace_tests.rs"]
mod tests;
