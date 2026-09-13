use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::core::services::lifecycle_guard;

pub mod activities;
pub mod change_sets;
pub mod groups;
pub mod offerings;
pub(crate) mod opening;
pub mod roster_memberships;
pub mod teacher_handoff;
pub mod workspaces;

pub(crate) async fn pending_term_work(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<Vec<crate::modules::academic::lifecycle::models::PendingTermWork>, AppError> {
    Ok(sqlx::query_as(
        "SELECT change.id,
         md5(to_jsonb(change)::text || COALESCE((SELECT jsonb_agg(to_jsonb(item) ORDER BY item.id)::text
             FROM academic_term_change_items item WHERE item.change_set_id=change.id),'[]')) AS revision,
         EXISTS(SELECT 1 FROM academic_term_change_items item WHERE item.change_set_id=change.id
             AND item.action_kind IN ('add_offering','stop_offering')) AS blocks_closure
         FROM academic_term_change_sets change WHERE change.academic_year_id=$1
         AND change.academic_term_id=$2 AND change.status='draft' ORDER BY change.id",
    ).bind(year).bind(term).fetch_all(&mut **tx).await?)
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
    let year_id: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM academic_terms WHERE id=$1")
            .bind(academic_term_id)
            .fetch_optional(&mut **transaction)
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
    let query = "SELECT term.id, term.academic_year_id, term.code, term.start_date, \
         term.planned_end_date, term.closed_on, year.end_date AS academic_year_end_date, \
         term.term_type, (SELECT count(*)::integer FROM academic_terms occurrence \
             WHERE occurrence.academic_year_id = term.academic_year_id \
               AND occurrence.term_type = term.term_type \
               AND occurrence.sequence_no <= term.sequence_no) AS type_occurrence, \
         term.status, term.row_version \
         FROM academic_terms term \
         JOIN academic_years year ON year.id = term.academic_year_id \
         WHERE term.id = $1";
    let term = sqlx::query_as(query)
        .bind(academic_term_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::ValidationError("ไม่พบภาคเรียนที่เลือก".to_string()))?;
    Ok(term)
}

pub(super) async fn require_active_owner(
    transaction: &mut Transaction<'_, Postgres>,
    owner_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM organization_units WHERE id = $1 AND is_active)",
    )
    .bind(owner_id)
    .fetch_one(&mut **transaction)
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
    let term_id = sqlx::query_scalar("SELECT academic_term_id FROM learning_offerings WHERE id=$1")
        .bind(offering_id)
        .fetch_optional(&mut **transaction)
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
        sqlx::query(&query)
            .bind(&group_ids)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

pub(super) async fn append_audit<T: Serialize>(
    pool: &PgPool,
    event_code: &str,
    entity_type: &str,
    entity_id: Uuid,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    actor_user_id: Uuid,
    payload: T,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO academic_audit_events (
               event_code, entity_type, entity_id, academic_year_id,
               academic_term_id, actor_user_id, payload
           ) VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(event_code)
    .bind(entity_type)
    .bind(entity_id)
    .bind(academic_year_id)
    .bind(academic_term_id)
    .bind(actor_user_id)
    .bind(sqlx::types::Json(payload))
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct TermContext {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub code: String,
    pub start_date: chrono::NaiveDate,
    pub planned_end_date: Option<chrono::NaiveDate>,
    pub closed_on: Option<chrono::NaiveDate>,
    pub academic_year_end_date: chrono::NaiveDate,
    pub term_type: String,
    pub type_occurrence: i32,
    pub status: String,
    pub row_version: i64,
}

impl TermContext {
    pub(super) fn date_upper_bound(&self) -> chrono::NaiveDate {
        self.closed_on
            .or(self.planned_end_date)
            .unwrap_or(self.academic_year_end_date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

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

    #[test]
    fn term_date_upper_bound_falls_back_to_the_academic_year_end() {
        let term = TermContext {
            id: Uuid::nil(),
            academic_year_id: Uuid::nil(),
            code: "1".to_string(),
            start_date: NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
            planned_end_date: None,
            closed_on: None,
            academic_year_end_date: NaiveDate::from_ymd_opt(2028, 4, 30).unwrap(),
            term_type: "regular".to_string(),
            type_occurrence: 1,
            status: "planning".to_string(),
            row_version: 1,
        };

        assert_eq!(
            term.date_upper_bound(),
            NaiveDate::from_ymd_opt(2028, 4, 30).unwrap()
        );
    }
}
