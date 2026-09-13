use super::super::models::{
    PromotionCorrectionImpact, PromotionImpactQuery, PromotionImpactResolution,
    PromotionImpactWorkspace,
};
use crate::{
    error::AppError, middleware::permission::ActorContext,
    modules::academic::results::services::corrections_after_annuals, permissions::registry::codes,
};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct ExecutedSource {
    item_id: Uuid,
    student_academic_year_id: Uuid,
    annual_revision_id: Uuid,
}

pub async fn get_promotion_impacts(
    pool: &PgPool,
    actor: &ActorContext,
    run: Uuid,
    query: PromotionImpactQuery,
) -> Result<PromotionImpactWorkspace, AppError> {
    actor.require_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)?;
    if run.is_nil() || query.after_id.is_some_and(|id| id.is_nil()) {
        return Err(AppError::ValidationError(
            "ระบุรอบเลื่อนชั้นและตำแหน่งรายการให้ถูกต้อง".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let (source, target): (Uuid, Uuid) = sqlx::query_as(
        "SELECT source_year_id,target_year_id FROM academic_promotion_runs WHERE id=$1",
    )
    .bind(run)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบเลื่อนชั้น".into()))?;
    let impacts = impacts_in_transaction(&mut tx, run, source).await?;
    let source_checksum = super::checksum(&(run, source, target, &impacts))?;
    let total_count = impacts.len();
    let pending_count = impacts
        .iter()
        .filter(|row| row.resolution.is_none())
        .count();
    let start = page_start(
        &impacts.iter().map(|row| row.id).collect::<Vec<_>>(),
        query.after_id,
    )?;
    let has_more = total_count.saturating_sub(start) > 50;
    let impacts: Vec<_> = impacts.into_iter().skip(start).take(50).collect();
    let next_cursor = if has_more {
        impacts.last().map(|row| row.id)
    } else {
        None
    };
    tx.commit().await?;
    Ok(PromotionImpactWorkspace {
        run_id: run,
        source_year_id: source,
        target_year_id: target,
        impacts,
        total_count,
        pending_count,
        next_cursor,
        source_checksum,
    })
}

/// Executed item evidence is retained even when the mutable run later reports a
/// failed batch. The receipt, rather than a current run status, proves execution.
pub(crate) async fn impacts_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    run: Uuid,
    source_year: Uuid,
) -> Result<Vec<PromotionCorrectionImpact>, AppError> {
    let sources: Vec<ExecutedSource> = sqlx::query_as(
        "SELECT item.id AS item_id,item.student_academic_year_id,item.annual_revision_id
         FROM academic_promotion_execution_receipts receipt
         JOIN academic_promotion_run_items item ON item.id=receipt.item_id AND item.run_id=receipt.run_id
         WHERE receipt.run_id=$1 AND receipt.source_year_id=$2
         ORDER BY item.id LIMIT 10001",
    ).bind(run).bind(source_year).fetch_all(&mut **tx).await?;
    if sources.len() > 10_000 {
        return Err(AppError::ValidationError(
            "รอบมีนักเรียนเกิน 10,000 รายการ".into(),
        ));
    }
    let annuals: Vec<_> = sources
        .iter()
        .map(|row| row.annual_revision_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut owners: BTreeMap<Uuid, Vec<&ExecutedSource>> = BTreeMap::new();
    for source in &sources {
        owners
            .entry(source.annual_revision_id)
            .or_default()
            .push(source);
    }
    let mut impacts = Vec::new();
    for batch in annuals.chunks(500) {
        for evidence in corrections_after_annuals(tx, source_year, batch).await? {
            let rows = owners
                .get(&evidence.annual_revision_id)
                .ok_or_else(|| AppError::InternalServerError("ผลแก้ไขไม่ตรงกับหลักฐานเลื่อนชั้น".into()))?;
            for source in rows {
                if impacts.len() >= 10_000 {
                    return Err(AppError::ValidationError(
                        "ผลกระทบในรอบนี้เกิน 10,000 รายการ กรุณาติดต่อผู้ดูแลระบบ".into(),
                    ));
                }
                impacts.push(PromotionCorrectionImpact {
                    id: impact_id(source.item_id, evidence.correction.id),
                    item_id: source.item_id,
                    student_academic_year_id: source.student_academic_year_id,
                    evidence: evidence.clone(),
                    resolution: None,
                });
            }
        }
    }
    let correction_ids: Vec<_> = impacts
        .iter()
        .map(|row| row.evidence.correction.id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !correction_ids.is_empty() {
        let resolutions: Vec<PromotionImpactResolution> = sqlx::query_as(
            "SELECT id,request_id,run_id,item_id,correction_id,impact_id,resolution_kind,
                    replacement_decision,reason,source_checksum,outcome,resolved_by,resolved_at
             FROM academic_promotion_impact_resolutions
             WHERE run_id=$1 AND correction_id=ANY($2)
             ORDER BY item_id,correction_id",
        )
        .bind(run)
        .bind(&correction_ids)
        .fetch_all(&mut **tx)
        .await?;
        let mut by_identity: BTreeMap<_, _> = resolutions
            .into_iter()
            .map(|row| ((row.item_id, row.correction_id), row))
            .collect();
        for impact in &mut impacts {
            impact.resolution =
                by_identity.remove(&(impact.item_id, impact.evidence.correction.id));
        }
        if !by_identity.is_empty() {
            return Err(AppError::InternalServerError(
                "หลักฐานการจัดการผลกระทบไม่ตรงกับผลแก้ไข".into(),
            ));
        }
    }
    impacts.sort_by_key(|row| row.id);
    Ok(impacts)
}

fn impact_id(item: Uuid, correction: Uuid) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("schoolorbit/promotion-impact/{item}/{correction}").as_bytes(),
    )
}

fn page_start(sorted_ids: &[Uuid], after: Option<Uuid>) -> Result<usize, AppError> {
    match after {
        None => Ok(0),
        Some(id) => sorted_ids
            .binary_search(&id)
            .map(|index| index + 1)
            .map_err(|_| AppError::NotFound("ไม่พบตำแหน่งผลกระทบในรอบเลื่อนชั้นนี้".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_impact_identity_and_cursor_keep_execution_and_correction_distinct() {
        let item = Uuid::new_v4();
        let correction = Uuid::new_v4();
        assert_eq!(impact_id(item, correction), impact_id(item, correction));
        assert_ne!(
            impact_id(item, correction),
            impact_id(Uuid::new_v4(), correction)
        );
        assert_ne!(impact_id(item, correction), impact_id(item, Uuid::new_v4()));
        let ids: Vec<_> = (1..=55).map(Uuid::from_u128).collect();
        assert_eq!(page_start(&ids, None).unwrap(), 0);
        assert_eq!(page_start(&ids, Some(ids[49])).unwrap(), 50);
        assert_eq!(page_start(&ids, Some(ids[54])).unwrap(), 55);
        assert!(matches!(
            page_start(&ids, Some(Uuid::nil())),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            page_start(&[], Some(ids[0])),
            Err(AppError::NotFound(_))
        ));
    }
}
