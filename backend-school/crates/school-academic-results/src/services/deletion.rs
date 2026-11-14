//! Scoped removal of official contributions for the current Delivery delete command.
//! The SQL owner seals exact immutable-row replacements; no global bypass is exposed.
use school_academic_delivery::models::current::DeliveryDeletionScope;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, Postgres, Transaction};

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentResultsDeletionImpact {
    pub official_results: i64,
    pub result_corrections: i64,
    pub term_aggregates: i64,
    pub annual_aggregates: i64,
    pub source_digest: String,
}

pub async fn preview_current_contribution_removal(
    connection: &mut PgConnection,
    scope: &DeliveryDeletionScope,
) -> Result<CurrentResultsDeletionImpact, AppError> {
    let impact: sqlx::types::Json<CurrentResultsDeletionImpact> = DbSession::new(connection)
        .scalar(
            "academic_results.deletion.preview_current_contribution_removal",
            "SELECT academic_current_results_deletion_impact($1,$2,$3)",
        )
        .bind(scope.learning_offering_id)
        .bind(&scope.learning_group_ids)
        .bind(scope.delete_offering)
        .fetch_one()
        .await?;
    Ok(impact.0)
}

/// Called only from the app's authorized current Delivery deletion transaction,
/// before Assessment or Delivery removes the live group/offering identity.
pub async fn remove_current_contributions(
    tx: &mut Transaction<'_, Postgres>,
    scope: &DeliveryDeletionScope,
) -> Result<(), AppError> {
    let valid:bool=DbSession::new(&mut **tx).scalar("academic_results.deletion.validate_scope","SELECT EXISTS(SELECT 1 FROM learning_offerings WHERE id=$1 AND academic_year_id=$2 AND academic_term_id=$3) AND (SELECT count(*) FROM learning_groups WHERE learning_offering_id=$1 AND academic_year_id=$2 AND academic_term_id=$3 AND id=ANY($4))=cardinality($4) AND (NOT $5 OR (SELECT count(*) FROM learning_groups WHERE learning_offering_id=$1)=cardinality($4))").bind(scope.learning_offering_id).bind(scope.academic_year_id).bind(scope.academic_term_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).fetch_one().await?;
    if !valid {
        return Err(AppError::Conflict("Results deletion scope changed".into()));
    }
    if scope.delete_offering {
        DbSession::new(&mut **tx)
            .query(
                "academic_results.deletion.remove_offering",
                "SELECT academic_remove_offering_results($1)",
            )
            .bind(scope.learning_offering_id)
            .execute()
            .await?;
    } else {
        if scope.learning_group_ids.len() != 1 {
            return Err(AppError::ValidationError(
                "Group deletion requires one exact group".into(),
            ));
        }
        DbSession::new(&mut **tx)
            .query(
                "academic_results.deletion.remove_group",
                "SELECT academic_remove_group_results($1)",
            )
            .bind(scope.learning_group_ids[0])
            .execute()
            .await?;
    }
    Ok(())
}
