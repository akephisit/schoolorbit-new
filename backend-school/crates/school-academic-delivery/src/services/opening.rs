use school_database::DbSession;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct CurrentOfferingReference {
    id: Uuid,
    kind: String,
    row_version: i64,
}
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentOfferingEvidence {
    references: Vec<CurrentOfferingReference>,
    pub source_checksum: String,
}
impl CurrentOfferingEvidence {
    pub fn count(&self) -> usize {
        self.references.len()
    }
}
pub async fn current_for_term(
    tx: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
) -> Result<CurrentOfferingEvidence, AppError> {
    let row_version =
        super::current::term_revision_on_connection(tx, academic_year_id, academic_term_id).await?;
    let references:Vec<CurrentOfferingReference>=DbSession::new(&mut **tx).query_as("academic_delivery.opening.current_for_term","SELECT id,kind,row_version FROM learning_offerings WHERE academic_year_id=$1 AND academic_term_id=$2 ORDER BY id LIMIT 5001").bind(academic_year_id).bind(academic_term_id).fetch_all().await?;
    if references.len() > 5000 {
        return Err(AppError::ValidationError(
            "รายการเปิดสอนเกินขนาดที่รองรับ".into(),
        ));
    }
    let source_checksum = super::stable_hash(&(row_version, &references))?;
    Ok(CurrentOfferingEvidence {
        references,
        source_checksum,
    })
}
