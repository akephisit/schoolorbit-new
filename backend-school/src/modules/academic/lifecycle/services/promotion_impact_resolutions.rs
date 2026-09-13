use super::super::models::{
    PromotionImpactResolution, PromotionImpactResolutionKind, PromotionImpactResolutionOutcome,
    ResolvePromotionImpactInput,
};
use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::core::services::{lifecycle_guard, promotion_reconciliation, years_terms},
    permissions::registry::codes,
};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;

const COLUMNS: &str = "id,request_id,run_id,item_id,correction_id,impact_id,resolution_kind,replacement_decision,reason,source_checksum,outcome,resolved_by,resolved_at";

pub async fn resolve_promotion_impact(
    pool: &PgPool,
    actor: &ActorContext,
    run_id: Uuid,
    impact_id: Uuid,
    input: ResolvePromotionImpactInput,
) -> Result<PromotionImpactResolution, AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_PROMOTION_READ_SCHOOL,
        codes::ACADEMIC_PROMOTION_CORRECT_SCHOOL,
    ])?;
    validate_input(run_id, impact_id, &input)?;
    let reason = input.reason.trim().to_owned();
    super::promotion_decision::validate_reason(&reason)?;
    let request_checksum = super::checksum(&(
        actor.user_id,
        run_id,
        impact_id,
        input.resolution_kind,
        &input.replacement_decision,
        &reason,
        &input.source_checksum,
    ))?;
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(replay) = replay(
        &mut tx,
        input.request_id,
        actor.user_id,
        run_id,
        impact_id,
        &request_checksum,
    )
    .await?
    {
        tx.commit().await?;
        return Ok(replay);
    }
    let (source_year_id, target_year_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT source_year_id,target_year_id FROM academic_promotion_runs WHERE id=$1 FOR UPDATE",
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบเลื่อนชั้น".into()))?;
    let impacts =
        super::promotion_impacts::impacts_in_transaction(&mut tx, run_id, source_year_id).await?;
    let current_checksum = super::checksum(&(run_id, source_year_id, target_year_id, &impacts))?;
    if current_checksum != input.source_checksum {
        return Err(AppError::Conflict(
            "ผลแก้ไขหรือการจัดการผลกระทบเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".into(),
        ));
    }
    let impact = impacts
        .iter()
        .find(|row| row.id == impact_id)
        .ok_or_else(|| AppError::NotFound("ไม่พบผลกระทบในรอบเลื่อนชั้นนี้".into()))?;
    if impact.resolution.is_some() {
        return Err(AppError::Conflict(
            "ผลกระทบนี้ได้รับการจัดการแล้ว กรุณาโหลดข้อมูลล่าสุด".into(),
        ));
    }
    let outcome = match input.resolution_kind {
        PromotionImpactResolutionKind::KeepExisting => {
            current_outcome(&mut tx, impact.item_id).await?
        }
        PromotionImpactResolutionKind::ReplaceDecision => {
            adjusted_outcome(
                &mut tx,
                actor.user_id,
                run_id,
                source_year_id,
                target_year_id,
                impact.item_id,
                input
                    .replacement_decision
                    .as_ref()
                    .ok_or_else(|| AppError::ValidationError("ระบุผลเลื่อนชั้นใหม่".into()))?,
            )
            .await?
        }
    };
    let id = Uuid::new_v4();
    let resolution: PromotionImpactResolution = sqlx::query_as(&format!(
        "INSERT INTO academic_promotion_impact_resolutions(
             id,request_id,run_id,item_id,correction_id,impact_id,resolution_kind,
             replacement_decision,reason,source_checksum,request_checksum,outcome,resolved_by
         ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
         RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(input.request_id)
    .bind(run_id)
    .bind(impact.item_id)
    .bind(impact.evidence.correction.id)
    .bind(impact.id)
    .bind(input.resolution_kind)
    .bind(input.replacement_decision.as_ref().map(Json))
    .bind(&reason)
    .bind(&input.source_checksum)
    .bind(&request_checksum)
    .bind(Json(&outcome))
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await?;
    years_terms::append_audit(
        &mut tx,
        "promotion_impact.resolved",
        "promotion_impact",
        impact.id,
        Some(source_year_id),
        None,
        actor.user_id,
        &resolution,
    )
    .await?;
    tx.commit().await?;
    Ok(resolution)
}

async fn adjusted_outcome(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    run_id: Uuid,
    source_year_id: Uuid,
    target_year_id: Uuid,
    item_id: Uuid,
    decision: &super::super::models::PromotionDecisionInput,
) -> Result<PromotionImpactResolutionOutcome, AppError> {
    let (
        source_student_year_id,
        source_grade_level_id,
        Json(recommendation),
        receipt_target_student_year_id,
        receipt_target_placement_id,
    ): (
        Uuid,
        Uuid,
        Json<super::super::models::PromotionRecommendation>,
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT item.student_academic_year_id,item.source_grade_level_id,item.recommendation,
                receipt.target_student_year_id,receipt.target_placement_id
         FROM academic_promotion_run_items item
         JOIN academic_promotion_execution_receipts receipt ON receipt.item_id=item.id AND receipt.run_id=item.run_id
         WHERE item.id=$1 AND item.run_id=$2",
    )
    .bind(item_id)
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Conflict("ไม่พบหลักฐานดำเนินการของผลกระทบนี้".into()))?;
    super::promotion_decision::validate_decision(decision, source_grade_level_id, &recommendation)?;
    // A no-target replacement retains the previous rows as immutable history.
    // Search the full receipt/resolution lineage for the latest owned IDs so a
    // later correction can restore those rows instead of creating duplicates.
    let (resolved_target_student_year_id, resolved_target_placement_id): (
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT
             (SELECT NULLIF(outcome->>'targetStudentYearId','')::uuid
              FROM academic_promotion_impact_resolutions
              WHERE item_id=$1 AND NULLIF(outcome->>'targetStudentYearId','') IS NOT NULL
              ORDER BY resolved_at DESC,id DESC LIMIT 1),
             (SELECT NULLIF(outcome->>'targetPlacementId','')::uuid
              FROM academic_promotion_impact_resolutions
              WHERE item_id=$1 AND NULLIF(outcome->>'targetPlacementId','') IS NOT NULL
              ORDER BY resolved_at DESC,id DESC LIMIT 1)",
    )
    .bind(item_id)
    .fetch_one(&mut **tx)
    .await?;
    let owned_target_student_year_id =
        resolved_target_student_year_id.or(receipt_target_student_year_id);
    let owned_target_placement_id = resolved_target_placement_id.or(receipt_target_placement_id);
    let adjusted = promotion_reconciliation::reconcile_decision(
        tx,
        actor_user_id,
        source_year_id,
        target_year_id,
        source_student_year_id,
        owned_target_student_year_id,
        owned_target_placement_id,
        decision,
    )
    .await?;
    Ok(PromotionImpactResolutionOutcome {
        adjusted: true,
        target_student_year_id: adjusted.target_student_year_id,
        target_placement_id: adjusted.target_placement_id,
        source_row_version: adjusted.source_row_version,
    })
}

fn validate_input(
    run_id: Uuid,
    impact_id: Uuid,
    input: &ResolvePromotionImpactInput,
) -> Result<(), AppError> {
    let reason_length = input.reason.trim().chars().count();
    if run_id.is_nil()
        || impact_id.is_nil()
        || input.request_id.is_nil()
        || input.source_checksum.len() != 64
        || !input
            .source_checksum
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !(1..=1000).contains(&reason_length)
    {
        return Err(AppError::ValidationError(
            "ระบุรอบ ผลกระทบ รุ่นข้อมูล และเหตุผลให้ถูกต้อง".into(),
        ));
    }
    let decision_matches = match input.resolution_kind {
        PromotionImpactResolutionKind::KeepExisting => input.replacement_decision.is_none(),
        PromotionImpactResolutionKind::ReplaceDecision => input.replacement_decision.is_some(),
    };
    if !decision_matches {
        return Err(AppError::ValidationError(
            "ระบุผลการจัดการและผลเลื่อนชั้นใหม่ให้สอดคล้องกัน".into(),
        ));
    }
    Ok(())
}

async fn current_outcome(
    tx: &mut Transaction<'_, Postgres>,
    item_id: Uuid,
) -> Result<PromotionImpactResolutionOutcome, AppError> {
    let (target_student_year_id, target_placement_id, source_row_version): (
        Option<Uuid>,
        Option<Uuid>,
        i64,
    ) = sqlx::query_as(
        "SELECT receipt.target_student_year_id,receipt.target_placement_id,source.row_version
         FROM academic_promotion_execution_receipts receipt
         JOIN academic_promotion_run_items item ON item.id=receipt.item_id
         JOIN student_academic_years source ON source.id=item.student_academic_year_id
         WHERE receipt.item_id=$1",
    )
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Conflict("ไม่พบหลักฐานดำเนินการของผลกระทบนี้".into()))?;
    let latest_replacement: Option<Json<PromotionImpactResolutionOutcome>> = sqlx::query_scalar(
        "SELECT outcome FROM academic_promotion_impact_resolutions
         WHERE item_id=$1 AND resolution_kind='replace_decision'
         ORDER BY resolved_at DESC,id DESC LIMIT 1",
    )
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (target_student_year_id, target_placement_id) = latest_replacement
        .map(|Json(outcome)| (outcome.target_student_year_id, outcome.target_placement_id))
        .unwrap_or((target_student_year_id, target_placement_id));
    Ok(PromotionImpactResolutionOutcome {
        adjusted: false,
        target_student_year_id,
        target_placement_id,
        source_row_version,
    })
}

async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
    actor_id: Uuid,
    run_id: Uuid,
    impact_id: Uuid,
    checksum: &str,
) -> Result<Option<PromotionImpactResolution>, AppError> {
    let stored: Option<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT resolved_by,run_id,impact_id,request_checksum
         FROM academic_promotion_impact_resolutions WHERE request_id=$1",
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((stored_actor, stored_run, stored_impact, stored_checksum)) = stored else {
        return Ok(None);
    };
    if stored_actor != actor_id
        || stored_run != run_id
        || stored_impact != impact_id
        || stored_checksum != checksum
    {
        return Err(AppError::Conflict(
            "รหัสคำขอนี้ถูกใช้กับการจัดการผลกระทบอื่นแล้ว".into(),
        ));
    }
    let resolution = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM academic_promotion_impact_resolutions WHERE request_id=$1"
    ))
    .bind(request_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(Some(resolution))
}
