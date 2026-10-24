use crate::models::{
    CurriculumCreditCriteriaResult, CurriculumCreditCriterion,
    ReplaceCurriculumCreditCriteriaRequest,
};
use school_database::DbSession;
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
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.curriculum_credit_criteria.replace",
    )
    .await?;
    super::curriculum::require_draft_level(&mut tx, level_id, request.draft_id).await?;
    let (Json(previous), mut row_version, Json(mut grades)): (Json<Vec<CurriculumCreditCriterion>>, i64, Json<Vec<Uuid>>) = DbSession::new(&mut *tx).query_as("academic_core.services.curriculum_credit_criteria.replace",
        "SELECT credit_criteria,credit_criteria_row_version,grade_level_ids FROM curriculum_levels WHERE id=$1",
    ).bind(level_id).fetch_one().await?;
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
        row_version = DbSession::new(&mut *tx).scalar("academic_core.services.curriculum_credit_criteria.replace", "UPDATE curriculum_levels SET credit_criteria=$2,credit_criteria_row_version=credit_criteria_row_version+1,updated_at=now() WHERE id=$1 RETURNING credit_criteria_row_version")
            .bind(level_id).bind(Json(&criteria)).fetch_one().await?;
    }
    tx.commit().await?;
    Ok(CurriculumCreditCriteriaResult {
        criteria,
        row_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        CurriculumCreditCriteriaComparison as Comparison, CurriculumCreditCriteriaScope as Scope,
    };

    fn rule(target: &str) -> CurriculumCreditCriterion {
        CurriculumCreditCriterion {
            scope: Scope::Annual,
            comparison: Comparison::Equal,
            target_credits: target.into(),
            enabled: true,
            grade_level_ids: vec![Uuid::from_u128(1)],
        }
    }

    #[test]
    fn credit_criteria_decimal_normalization_and_bounds_are_semantic() {
        let expected = normalize(vec![rule("1")]).unwrap();
        for value in ["1.0", "1.00"] {
            assert_eq!(normalize(vec![rule(value)]).unwrap(), expected);
        }
        for value in ["0", "9999.99"] {
            assert!(normalize(vec![rule(value)]).is_ok());
        }
        for value in [
            "10000",
            "9999.999",
            "-1",
            "1e2",
            "01",
            "",
            "NaN",
            "99999999999999999999999999",
        ] {
            assert!(normalize(vec![rule(value)]).is_err(), "{value}");
        }
        assert!(normalize(vec![rule("1"), rule("2")]).is_err());
        let mut duplicate = rule("1");
        duplicate.grade_level_ids.push(duplicate.grade_level_ids[0]);
        assert!(normalize(vec![duplicate]).is_err());
        let mut missing = rule("1");
        missing.grade_level_ids.clear();
        assert!(normalize(vec![missing]).is_err());
    }

    #[test]
    fn credit_criteria_inputs_reject_unknown_and_unreviewed_fields() {
        let mut value =
            serde_json::json!({"draftId":Uuid::new_v4(),"rowVersion":1,"criteria":[rule("22")]});
        assert!(
            serde_json::from_value::<ReplaceCurriculumCreditCriteriaRequest>(value.clone()).is_ok()
        );
        value["criteria"][0]["national"] = true.into();
        assert!(
            serde_json::from_value::<ReplaceCurriculumCreditCriteriaRequest>(value.clone())
                .is_err()
        );
        value["criteria"][0]
            .as_object_mut()
            .unwrap()
            .remove("national");
        value["criteria"][0]
            .as_object_mut()
            .unwrap()
            .remove("gradeLevelIds");
        assert!(serde_json::from_value::<ReplaceCurriculumCreditCriteriaRequest>(value).is_err());
    }
}
