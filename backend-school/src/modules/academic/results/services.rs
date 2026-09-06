use super::models::*;
use crate::error::AppError;
use bigdecimal::BigDecimal;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};

mod activities;
mod corrections;
mod course_preparation;
mod locking;
mod policies;
mod readiness;

pub use activities::{confirm_activity, get_activity_workspace, save_activity_outcomes};
pub use course_preparation::{confirm_group_results, get_course_workspace, save_selection};
pub use policies::{activate_policy, create_policy, derive_grade, list_policies, validate_policy};
pub use readiness::readiness;

pub(super) fn decimal(value: &str) -> Result<BigDecimal, AppError> {
    let value = crate::modules::academic::core::services::validate_canonical_decimal(value, 2)?;
    if value > BigDecimal::from(99_999_999) + BigDecimal::from(99) / BigDecimal::from(100) {
        return Err(AppError::ValidationError(
            "Decimal value exceeds storage range".into(),
        ));
    }
    Ok(value)
}

pub(super) fn decimal_wire(value: &BigDecimal) -> String {
    format!("{value:.2}")
}

pub(super) fn conflict() -> AppError {
    AppError::Conflict("Academic result source changed; refresh before retrying".into())
}

pub(super) fn check_version(expected: Option<i64>, actual: Option<i64>) -> Result<(), AppError> {
    if expected == actual {
        Ok(())
    } else {
        Err(conflict())
    }
}

pub(super) fn hash(value: &impl serde::Serialize) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| AppError::InternalServerError("Could not encode result revision".into()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(super) async fn validate_context(
    tx: &mut Transaction<'_, Postgres>,
    context: &ResultContext,
) -> Result<(), AppError> {
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM academic_terms WHERE id=$1 AND academic_year_id=$2)",
    )
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .fetch_one(&mut **tx)
    .await?;
    if !valid {
        return Err(AppError::ValidationError(
            "Academic term does not belong to selected year".into(),
        ));
    }
    Ok(())
}
