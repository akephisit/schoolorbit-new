use school_database::DbSession;
pub mod attendance_roster;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use school_academic_core::services::lifecycle_guard;
use school_errors::AppError;

pub mod activities;
pub mod bulk_group_setup;
pub mod current;
pub mod deletion;
pub mod groups;
pub mod offerings;
pub mod opening;
pub mod roster_memberships;
pub mod roster_tracking;
pub mod workspaces;

pub async fn pending_term_work(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<Vec<school_academic_core::ports::PendingTermWork>, AppError> {
    let _ = (tx, year, term);
    Ok(Vec::new())
}

/// Validate that a delivery write may mutate the selected term.
///
/// This boundary-safe variant intentionally does not expose the delivery
/// crate's internal term projection.
pub async fn ensure_writable_term(
    transaction: &mut Transaction<'_, Postgres>,
    academic_term_id: Uuid,
    lock_for_update: bool,
) -> Result<(), AppError> {
    require_writable_term(transaction, academic_term_id, lock_for_update)
        .await
        .map(|_| ())
}

pub(super) fn validate_row_version(row_version: i64) -> Result<(), AppError> {
    if row_version <= 0 {
        return Err(AppError::ValidationError(
            "rowVersion ต้องมากกว่าศูนย์".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn stable_hash<T: Serialize>(value: &T) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถสร้าง source hash ได้".to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub(super) async fn require_writable_term(
    transaction: &mut Transaction<'_, Postgres>,
    academic_term_id: Uuid,
    lock_for_update: bool,
) -> Result<TermContext, AppError> {
    let year_id: Uuid = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.require_writable_term",
            "SELECT academic_year_id FROM academic_terms WHERE id=$1",
        )
        .bind(academic_term_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::ValidationError("ไม่พบภาคเรียนที่เลือก".to_string()))?;
    if lock_for_update {
        lifecycle_guard::require_term_write_exclusive(transaction, year_id, academic_term_id)
            .await?;
    } else {
        lifecycle_guard::require_term_write(transaction, year_id, academic_term_id).await?;
    }
    load_term_context(transaction, academic_term_id).await
}

/// Load immutable term metadata without taking row locks. This is reserved for
/// repeatable-read previews whose caller already owns the lifecycle advisory
/// lock and must remain a genuinely read-only transaction.
pub(super) async fn load_term_context(
    transaction: &mut Transaction<'_, Postgres>,
    academic_term_id: Uuid,
) -> Result<TermContext, AppError> {
    let query = "SELECT term.id, term.academic_year_id, term.code, (now() AT TIME ZONE 'Asia/Bangkok')::date AS operation_date, \
         term.term_type, (SELECT count(*)::integer FROM academic_terms occurrence \
             WHERE occurrence.academic_year_id = term.academic_year_id \
               AND occurrence.term_type = term.term_type \
               AND occurrence.sequence_no <= term.sequence_no) AS type_occurrence, \
         term.status, term.row_version \
         FROM academic_terms term \
         JOIN academic_years year ON year.id = term.academic_year_id \
         WHERE term.id = $1";
    let term = DbSession::new(&mut **transaction)
        .query_as("academic_delivery.services.load_term_context", query)
        .bind(academic_term_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::ValidationError("ไม่พบภาคเรียนที่เลือก".to_string()))?;
    Ok(term)
}

pub(super) async fn require_active_owner(
    transaction: &mut Transaction<'_, Postgres>,
    owner_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.require_active_owner",
            "SELECT EXISTS (SELECT 1 FROM organization_units WHERE id = $1 AND is_active)",
        )
        .bind(owner_id)
        .fetch_one()
        .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "หน่วยงานเจ้าของข้อมูลไม่ถูกต้องหรือไม่ได้ใช้งาน".to_string(),
        ))
    }
}

/// Resolve immutable context without locking the offering; lifecycle locks
/// must precede offering/group locks in every writer.
pub(super) async fn require_writable_offering_term(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
) -> Result<TermContext, AppError> {
    let term_id = DbSession::new(&mut **transaction)
        .scalar(
            "academic_delivery.services.require_writable_offering_term",
            "SELECT academic_term_id FROM learning_offerings WHERE id=$1",
        )
        .bind(offering_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการเปิดสอน".to_string()))?;
    require_writable_term(transaction, term_id, false).await
}

/// Mark every academic confirmation derived from a learning group's roster or
/// teacher assignments as stale. Callers must already hold the offering and
/// group write locks, in that order, so cross-domain mutations retain the
/// shared academic lock order before these source rows are touched.
pub(super) async fn invalidate_group_academic_confirmations(
    transaction: &mut Transaction<'_, Postgres>,
    group_ids: &[Uuid],
) -> Result<(), AppError> {
    if group_ids.is_empty() {
        return Ok(());
    }

    let mut group_ids = group_ids.to_vec();
    group_ids.sort_unstable();
    group_ids.dedup();
    for table in [
        "learning_group_phase_confirmations",
        "learning_group_result_confirmations",
        "learning_group_evaluation_confirmations",
        "academic_activity_result_confirmations",
    ] {
        let query = format!(
            "UPDATE {table} \
             SET row_version = row_version + 1, \
                 source_snapshot = jsonb_set(source_snapshot, '{{invalidated}}', 'true'::jsonb) \
             WHERE learning_group_id = ANY($1) \
               AND NOT COALESCE((source_snapshot->>'invalidated')::boolean, false)"
        );
        DbSession::new(&mut **transaction)
            .query(
                "academic_delivery.services.invalidate_group_academic_confirmations",
                sqlx::AssertSqlSafe(query),
            )
            .bind(&group_ids)
            .execute()
            .await?;
    }
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct TermContext {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub code: String,
    pub operation_date: chrono::NaiveDate,
    pub term_type: String,
    pub type_occurrence: i32,
    pub status: String,
    pub row_version: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_hash_is_deterministic_and_revision_validation_fails_closed() {
        assert_eq!(
            stable_hash(&vec![1, 2, 3]).unwrap(),
            stable_hash(&vec![1, 2, 3]).unwrap()
        );
        assert_ne!(
            stable_hash(&vec![1, 2, 3]).unwrap(),
            stable_hash(&vec![3, 2, 1]).unwrap()
        );
        assert!(validate_row_version(1).is_ok());
        assert!(validate_row_version(0).is_err());
    }
}
