mod opening_policy;
mod opening_readiness;
#[cfg(not(feature = "integration-test-support"))]
mod promotion_approval;
#[cfg(feature = "integration-test-support")]
pub mod promotion_approval;
pub use opening_policy::{get_opening_policy, update_opening_policy};
pub(crate) use opening_readiness::activation_evidence_in_transaction;
pub use opening_readiness::get_activation_workspace;
pub use promotion_approval::approve_run;
#[cfg(not(feature = "integration-test-support"))]
mod promotion_calculation;
#[cfg(feature = "integration-test-support")]
pub mod promotion_calculation;
mod promotion_context;
pub use promotion_calculation::calculate_run;
mod promotion_decision;
#[cfg(not(feature = "integration-test-support"))]
mod promotion_execution;
#[cfg(feature = "integration-test-support")]
pub mod promotion_execution;
pub use promotion_execution::execute_run;
#[cfg(not(feature = "integration-test-support"))]
mod promotion_opening;
#[cfg(feature = "integration-test-support")]
pub mod promotion_opening;
mod promotion_policy;
mod promotion_recommendation;
#[cfg(not(feature = "integration-test-support"))]
mod promotion_run_review;
#[cfg(feature = "integration-test-support")]
pub mod promotion_run_review;
pub use promotion_run_review::review_item;
#[cfg(not(feature = "integration-test-support"))]
mod promotion_runs;
#[cfg(feature = "integration-test-support")]
pub mod promotion_runs;
mod promotion_workspace;
pub use promotion_policy::{
    create_promotion_policy, get_promotion_policy_options, list_promotion_policies,
};
pub use promotion_runs::create_run;
pub use promotion_workspace::{get_run_workspace, list_runs};
#[cfg(not(feature = "integration-test-support"))]
mod readiness;
#[cfg(feature = "integration-test-support")]
pub mod readiness;
mod term_activation;
pub mod term_transitions;
mod year_readiness;
mod year_reopening;
pub mod year_reopening_command;
pub mod year_transitions;
pub use year_readiness::get_year_workspace;
pub(crate) use year_readiness::year_workspace_in_transaction;
pub use year_reopening::get_year_reopening_workspace;
pub(crate) use year_reopening::reopening_workspace_in_transaction;
mod promotion_impact_resolutions;
mod promotion_impacts;
#[cfg(not(feature = "integration-test-support"))]
mod term_preparation;
#[cfg(feature = "integration-test-support")]
pub mod term_preparation;
pub use promotion_impact_resolutions::resolve_promotion_impact;
pub use promotion_impacts::get_promotion_impacts;
pub use term_preparation::{apply as apply_term_preparation, preview as preview_term_preparation};

pub async fn get_workspace(
    external: &impl crate::ports::ExternalLifecyclePort,
    pool: &sqlx::PgPool,
    actor: &school_authorization::ActorContext,
    year: uuid::Uuid,
    term: uuid::Uuid,
) -> Result<super::models::TermLifecycleWorkspace, school_errors::AppError> {
    actor
        .require_permission(school_permissions::registry::codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let workspace =
        readiness::workspace_in_transaction(external, &mut tx, actor, year, term).await?;
    tx.commit().await?;
    Ok(workspace)
}

pub(crate) use readiness::workspace_in_transaction;

#[cfg_attr(feature = "integration-test-support", doc(hidden))]
pub fn checksum(value: &impl serde::Serialize) -> Result<String, school_errors::AppError> {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(value).map_err(|_| {
        school_errors::AppError::InternalServerError("ไม่สามารถสร้างรุ่นความพร้อมได้".into())
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
