use crate::models::{
    CurriculumCreditCriteriaResult, CurriculumCreditCriterion,
    ReplaceCurriculumCreditCriteriaRequest,
};
use school_errors::AppError;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

fn normalize(
    mut criteria: Vec<CurriculumCreditCriterion>,
) -> Result<Vec<CurriculumCreditCriterion>, AppError> {
    if criteria.len() > 2 {
        return Err(AppError::ValidationError(
            "กำหนดเกณฑ์ได้ไม่เกินสองขอบเขต".into(),
        ));
    }
    criteria.sort_by_key(|criterion| criterion.scope);
    if criteria
        .windows(2)
        .any(|pair| pair[0].scope == pair[1].scope)
    {
        return Err(AppError::ValidationError("ขอบเขตเกณฑ์ต้องไม่ซ้ำกัน".into()));
    }
    for criterion in &mut criteria {
        if criterion.target_credits.len() > 7 {
            return Err(AppError::ValidationError(
                "หน่วยกิตเป้าหมายต้องอยู่ระหว่าง 0–9999.99".into(),
            ));
        }
        let target = super::validate_canonical_decimal(&criterion.target_credits, 2)?;
        if target > bigdecimal::BigDecimal::from(9999) + bigdecimal::BigDecimal::new(99.into(), 2) {
            return Err(AppError::ValidationError(
                "หน่วยกิตเป้าหมายต้องอยู่ระหว่าง 0–9999.99".into(),
            ));
        }
        criterion.target_credits = target.normalized().to_string();
        if criterion.grade_level_ids.is_empty() || criterion.grade_level_ids.len() > 500 {
            return Err(AppError::ValidationError(
                "เลือกระดับชั้นของเกณฑ์ระหว่าง 1–500 ชั้น".into(),
            ));
        }
        criterion.grade_level_ids.sort_unstable();
        if criterion
            .grade_level_ids
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(AppError::ValidationError("ระดับชั้นของเกณฑ์ต้องไม่ซ้ำกัน".into()));
        }
    }
    Ok(criteria)
}

/// Criteria own their optimistic token; saving them never advances a staged
/// requirement, term-slot, program-order or level-metadata baseline.
pub async fn replace(
    pool: &PgPool,
    level_id: Uuid,
    request: ReplaceCurriculumCreditCriteriaRequest,
) -> Result<CurriculumCreditCriteriaResult, AppError> {
    super::parse_row_version(request.row_version)?;
    let criteria = normalize(request.criteria)?;
    let mut tx = pool.begin().await?;
    super::curriculum::require_draft_level(&mut tx, level_id, request.draft_id).await?;
    let (Json(previous), mut row_version, Json(mut grades)): (Json<Vec<CurriculumCreditCriterion>>, i64, Json<Vec<Uuid>>) = sqlx::query_as(
        "SELECT credit_criteria,credit_criteria_row_version,grade_level_ids FROM curriculum_levels WHERE id=$1",
    ).bind(level_id).fetch_one(&mut *tx).await?;
    if row_version != request.row_version {
        return Err(AppError::Conflict(
            "เกณฑ์หน่วยกิตถูกแก้ไขแล้ว กรุณาตรวจสอบเกณฑ์ล่าสุด".into(),
        ));
    }
    grades.sort_unstable();
    if criteria
        .iter()
        .any(|criterion| criterion.grade_level_ids != grades)
    {
        return Err(AppError::Conflict(
            "ชั้นที่ครอบคลุมเปลี่ยนไป โปรดตรวจเกณฑ์อีกครั้ง".into(),
        ));
    }
    if previous != criteria {
        row_version = sqlx::query_scalar("UPDATE curriculum_levels SET credit_criteria=$2,credit_criteria_row_version=credit_criteria_row_version+1,updated_at=now() WHERE id=$1 RETURNING credit_criteria_row_version")
            .bind(level_id).bind(Json(&criteria)).fetch_one(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(CurriculumCreditCriteriaResult {
        criteria,
        row_version,
    })
}
