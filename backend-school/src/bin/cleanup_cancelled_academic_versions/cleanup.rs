use school_academic_delivery::models::versions::DeleteDeliveryVersionRequest;
use school_academic_delivery::services::change_sets;
use school_academic_timetable::models::timetable_version::DeleteTimetableDraftRequest;
use school_academic_timetable::services::timetable_lifecycle;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub timetable_versions: usize,
    pub delivery_versions: usize,
    pub preview_hash: String,
    pub published_fingerprint: String,
    pub applied: bool,
}

fn authorize(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_permission(codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL)?;
    actor.require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)
}

/// Operational adapter: one scope, all preflights, then the provider-owned deletions in one transaction.
pub async fn cleanup(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
    expected_hash: Option<&str>,
) -> Result<CleanupReport, AppError> {
    authorize(actor)?;
    let mut tx = pool.begin().await?;
    let actual_year: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM academic_terms WHERE id=$1 FOR UPDATE")
            .bind(term)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนเป้าหมาย".into()))?;
    if actual_year != year {
        return Err(AppError::Conflict("ปีการศึกษาไม่ตรงกับภาคเรียนเป้าหมาย".into()));
    }
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND user_type='staff' AND status='active')",
    )
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await?;
    if !active {
        return Err(AppError::Forbidden("ผู้ดำเนินการต้องเป็นบุคลากรที่ยังใช้งาน".into()));
    }
    let tables:Vec<(Uuid,i64,i64)>=sqlx::query_as("SELECT version.id,version.row_version,(SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=version.id) FROM academic_timetable_versions version WHERE version.academic_term_id=$1 AND version.status='cancelled' ORDER BY version.id FOR UPDATE OF version").bind(term).fetch_all(&mut *tx).await?;
    let openings:Vec<(Uuid,i64,i64,i64,i64,i64)>=sqlx::query_as("SELECT version.id,version.row_version,COALESCE(journal.row_version,0),
        (SELECT count(*) FROM academic_term_change_items WHERE change_set_id=journal.id),jsonb_array_length(version.snapshot->'offerings')::bigint,
        COALESCE((SELECT sum(jsonb_array_length(offering->'groups')) FROM jsonb_array_elements(version.snapshot->'offerings') offering),0)::bigint
        FROM academic_delivery_versions version LEFT JOIN academic_term_change_sets journal ON journal.target_delivery_version_id=version.id
        WHERE version.academic_term_id=$1 AND version.status='cancelled' ORDER BY version.id FOR UPDATE OF version").bind(term).fetch_all(&mut *tx).await?;
    let preview_hash = hex::encode(Sha256::digest(
        serde_json::to_vec(&(year, term, &tables, &openings))
            .map_err(|_| AppError::InternalServerError("ไม่สามารถตรวจรายการล้างข้อมูล".into()))?,
    ));
    if expected_hash.is_some_and(|hash| hash != preview_hash) {
        return Err(AppError::Conflict(
            "รายการล้างข้อมูลเปลี่ยนไป กรุณาตรวจทั้งชุดใหม่".into(),
        ));
    }
    let before = published_fingerprint(&mut tx, term).await?;
    // Never delete the first eligible target before checking the last one.
    for (id, row_version, expected_block_count) in &tables {
        timetable_lifecycle::delete_in_transaction(
            &mut tx,
            actor.user_id,
            *id,
            &DeleteTimetableDraftRequest {
                row_version: *row_version,
                expected_block_count: *expected_block_count,
            },
            false,
        )
        .await?;
    }
    for (
        id,
        row_version,
        change_set_row_version,
        expected_item_count,
        expected_offering_count,
        expected_group_count,
    ) in &openings
    {
        change_sets::delete_version_in_transaction(
            &mut tx,
            actor.user_id,
            *id,
            &DeleteDeliveryVersionRequest {
                row_version: *row_version,
                change_set_row_version: *change_set_row_version,
                expected_item_count: *expected_item_count,
                expected_offering_count: *expected_offering_count,
                expected_group_count: *expected_group_count,
            },
            false,
        )
        .await?;
    }
    if expected_hash.is_some() {
        for (id, row_version, expected_block_count) in &tables {
            timetable_lifecycle::delete_in_transaction(
                &mut tx,
                actor.user_id,
                *id,
                &DeleteTimetableDraftRequest {
                    row_version: *row_version,
                    expected_block_count: *expected_block_count,
                },
                true,
            )
            .await?;
        }
        for (
            id,
            row_version,
            change_set_row_version,
            expected_item_count,
            expected_offering_count,
            expected_group_count,
        ) in &openings
        {
            change_sets::delete_version_in_transaction(
                &mut tx,
                actor.user_id,
                *id,
                &DeleteDeliveryVersionRequest {
                    row_version: *row_version,
                    change_set_row_version: *change_set_row_version,
                    expected_item_count: *expected_item_count,
                    expected_offering_count: *expected_offering_count,
                    expected_group_count: *expected_group_count,
                },
                true,
            )
            .await?;
        }
        let after = published_fingerprint(&mut tx, term).await?;
        if before != after {
            return Err(AppError::Conflict(
                "ข้อมูลเผยแพร่เปลี่ยนระหว่างล้างข้อมูล ยกเลิกทั้ง transaction".into(),
            ));
        }
        tx.commit().await?;
    } else {
        tx.rollback().await?;
    }
    Ok(CleanupReport {
        academic_year_id: year,
        academic_term_id: term,
        timetable_versions: tables.len(),
        delivery_versions: openings.len(),
        preview_hash,
        published_fingerprint: before,
        applied: expected_hash.is_some(),
    })
}

async fn published_fingerprint(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    term: Uuid,
) -> Result<String, AppError> {
    // Bounded aggregate evidence; the captured graphs and rows never leave the database.
    Ok(sqlx::query_scalar("SELECT md5(jsonb_build_object(
        'delivery',(SELECT jsonb_agg(to_jsonb(version) ORDER BY id) FROM academic_delivery_versions version WHERE academic_term_id=$1 AND status='published'),
        'timetable',(SELECT jsonb_agg(to_jsonb(version) ORDER BY id) FROM academic_timetable_versions version WHERE academic_term_id=$1 AND status='published'),
        'blocks',(SELECT jsonb_agg(to_jsonb(block) ORDER BY block.id) FROM academic_timetable_blocks block JOIN academic_timetable_versions version ON version.id=block.timetable_version_id WHERE version.academic_term_id=$1 AND version.status='published')
        )::text)").bind(term).fetch_one(&mut **tx).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_requires_both_school_management_scopes() {
        for permissions in [
            vec![],
            vec![codes::LEARNING_OFFERING_MANAGE_SCHOOL],
            vec![codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL],
            vec![
                codes::LEARNING_OFFERING_MANAGE_ASSIGNED,
                codes::ACADEMIC_TIMETABLE_MANAGE_ASSIGNED,
            ],
        ] {
            assert!(authorize(&ActorContext {
                user_id: Uuid::nil(),
                permissions: permissions.into_iter().map(String::from).collect()
            })
            .is_err());
        }
        assert!(authorize(&ActorContext {
            user_id: Uuid::nil(),
            permissions: vec![
                codes::LEARNING_OFFERING_MANAGE_SCHOOL.into(),
                codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL.into()
            ]
        })
        .is_ok());
    }
}
