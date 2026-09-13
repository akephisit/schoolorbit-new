mod opening_policy;
#[cfg(test)]
mod opening_policy_tests;
mod opening_readiness;
#[cfg(test)]
mod opening_readiness_tests;
mod promotion_approval;
pub use opening_policy::{get_opening_policy, update_opening_policy};
pub(crate) use opening_readiness::activation_evidence_in_transaction;
pub use opening_readiness::get_activation_workspace;
pub use promotion_approval::approve_run;
mod promotion_calculation;
pub use promotion_calculation::calculate_run;
mod promotion_decision;
mod promotion_execution;
pub use promotion_execution::execute_run;
mod promotion_opening;
mod promotion_policy;
mod promotion_recommendation;
mod promotion_run_review;
pub use promotion_run_review::review_item;
mod promotion_runs;
mod promotion_workspace;
pub use promotion_policy::{
    create_promotion_policy, get_promotion_policy_options, list_promotion_policies,
};
pub use promotion_runs::create_run;
pub use promotion_workspace::{get_run_workspace, list_runs};
#[cfg(test)]
mod provider_tests;
mod readiness;
mod year_readiness;
mod year_reopening;
pub use year_readiness::get_year_workspace;
pub(crate) use year_readiness::year_workspace_in_transaction;
pub use year_reopening::get_year_reopening_workspace;
pub(crate) use year_reopening::reopening_workspace_in_transaction;
#[cfg(test)]
mod promotion_impact_resolution_tests;
mod promotion_impact_resolutions;
#[cfg(test)]
mod promotion_impact_tests;
mod promotion_impacts;
mod term_preparation;
#[cfg(test)]
mod term_preparation_tests;
#[cfg(test)]
mod transition_tests;
pub use term_preparation::{apply as apply_term_preparation, preview as preview_term_preparation};
#[cfg(test)]
mod year_reopening_tests;
pub use promotion_impact_resolutions::resolve_promotion_impact;
pub use promotion_impacts::get_promotion_impacts;
#[cfg(test)]
mod year_transition_tests;

pub async fn get_workspace(
    pool: &sqlx::PgPool,
    actor: &crate::middleware::permission::ActorContext,
    year: uuid::Uuid,
    term: uuid::Uuid,
) -> Result<super::models::TermLifecycleWorkspace, crate::error::AppError> {
    actor
        .require_permission(crate::permissions::registry::codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let workspace = readiness::workspace_in_transaction(&mut tx, actor, year, term).await?;
    tx.commit().await?;
    Ok(workspace)
}

pub(crate) use readiness::workspace_in_transaction;

pub(crate) fn checksum(value: &impl serde::Serialize) -> Result<String, crate::error::AppError> {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(value).map_err(|_| {
        crate::error::AppError::InternalServerError("ไม่สามารถสร้างรุ่นความพร้อมได้".into())
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
