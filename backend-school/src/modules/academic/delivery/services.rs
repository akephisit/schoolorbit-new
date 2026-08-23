use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

pub mod activities;
pub mod groups;
pub mod offerings;

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
    let lock = if lock_for_update {
        "FOR UPDATE"
    } else {
        "FOR SHARE"
    };
    let query = format!(
        "SELECT term.id, term.academic_year_id, term.code, term.start_date, \
         term.status, term.row_version \
         FROM academic_terms term WHERE term.id = $1 {lock}"
    );
    let term: TermContext = sqlx::query_as(&query)
        .bind(academic_term_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::ValidationError("ไม่พบภาคเรียนที่เลือก".to_string()))?;
    if matches!(term.status.as_str(), "closing" | "closed" | "cancelled") {
        return Err(AppError::ValidationError(
            "ภาคเรียนนี้ปิดรับการแก้ไขข้อมูลจัดการเรียนแล้ว".to_string(),
        ));
    }
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
