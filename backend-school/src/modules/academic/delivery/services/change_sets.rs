use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::delivery::models::{
    AcademicTermChangeActionKind, AcademicTermChangeItem, AcademicTermChangeSet,
    AcademicTermChangeSetStatus, CancelAcademicTermChangeSetRequest,
    CreateAcademicTermChangeSetRequest, DeleteAcademicTermChangeItemRequest,
    LearningOfferingStatus, UpdateAcademicTermChangeSetRequest,
    UpsertAcademicTermChangeItemRequest,
};
use crate::modules::academic::services::timetable_version_service;

use super::{
    append_audit, offerings, require_active_owner, require_writable_term, stable_hash,
    validate_row_version, TermContext,
};

#[derive(Debug, FromRow)]
struct ChangeSetRow {
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    effective_from: NaiveDate,
    reason: String,
    status: AcademicTermChangeSetStatus,
    base_timetable_version_id: Option<Uuid>,
    target_timetable_version_id: Option<Uuid>,
    row_version: i64,
    created_by: Uuid,
    published_by: Option<Uuid>,
    published_at: Option<chrono::DateTime<Utc>>,
    cancelled_by: Option<Uuid>,
    cancelled_at: Option<chrono::DateTime<Utc>>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ChangeItemRow {
    id: Uuid,
    change_set_id: Uuid,
    action_kind: AcademicTermChangeActionKind,
    learning_offering_id: Uuid,
    weekly_period_target: Option<i32>,
    row_version: i64,
    created_by: Uuid,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NormalizedCreateRequest<'a> {
    academic_term_id: Uuid,
    effective_from: NaiveDate,
    reason: &'a str,
}

const CHANGE_SET_COLUMNS: &str = r#"
    id, academic_term_id, academic_year_id, effective_from, reason, status,
    base_timetable_version_id, target_timetable_version_id, row_version,
    created_by, published_by, published_at, cancelled_by, cancelled_at,
    created_at, updated_at
"#;

pub async fn list_change_sets(
    pool: &PgPool,
    academic_term_id: Uuid,
) -> Result<Vec<AcademicTermChangeSet>, AppError> {
    let query = format!(
        "SELECT {CHANGE_SET_COLUMNS} FROM academic_term_change_sets \
         WHERE academic_term_id = $1 \
         ORDER BY effective_from DESC, created_at DESC, id"
    );
    let rows = sqlx::query_as::<_, ChangeSetRow>(&query)
        .bind(academic_term_id)
        .fetch_all(pool)
        .await?;
    hydrate_many(pool, rows).await
}

pub async fn get_change_set(pool: &PgPool, id: Uuid) -> Result<AcademicTermChangeSet, AppError> {
    let query = format!("SELECT {CHANGE_SET_COLUMNS} FROM academic_term_change_sets WHERE id = $1");
    let row = sqlx::query_as::<_, ChangeSetRow>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()))?;
    let mut values = hydrate_many(pool, vec![row]).await?;
    values
        .pop()
        .ok_or_else(|| AppError::InternalServerError("ไม่สามารถโหลดชุดการเปลี่ยนแปลงได้".to_string()))
}

pub async fn create_change_set(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateAcademicTermChangeSetRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    let reason = normalized_reason(&request.reason)?;
    let request_hash = stable_hash(&NormalizedCreateRequest {
        academic_term_id: request.academic_term_id,
        effective_from: request.effective_from,
        reason: &reason,
    })?;

    let mut transaction = pool.begin().await?;
    let term = require_writable_term(&mut transaction, request.academic_term_id, true).await?;
    validate_effective_date(&term, request.effective_from)?;

    if let Some((existing_id, existing_hash)) = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, creation_request_hash FROM academic_term_change_sets \
         WHERE academic_term_id = $1 AND idempotency_key = $2",
    )
    .bind(request.academic_term_id)
    .bind(request.idempotency_key.to_string())
    .fetch_optional(&mut *transaction)
    .await?
    {
        if existing_hash != request_hash {
            return Err(AppError::Conflict(
                "idempotencyKey นี้ถูกใช้กับรายละเอียดชุดเปลี่ยนแปลงอื่นแล้ว".to_string(),
            ));
        }
        transaction.commit().await?;
        return get_change_set(pool, existing_id).await;
    }

    let (base_version_id, base_row_version): (Uuid, i64) = sqlx::query_as(
        r#"SELECT id, row_version
           FROM academic_timetable_versions
           WHERE academic_term_id = $1
             AND status = 'published'
             AND effective_from <= $2
           ORDER BY effective_from DESC, id
           LIMIT 1
           FOR SHARE"#,
    )
    .bind(request.academic_term_id)
    .bind(request.effective_from)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| {
        AppError::ValidationError("ยังไม่มีรุ่นตารางเรียนที่เผยแพร่และใช้เป็นต้นทางในวันที่เลือก".to_string())
    })?;

    let change_set_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_term_change_sets (
               id, academic_term_id, academic_year_id, effective_from, reason,
               idempotency_key, creation_request_hash, created_by
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(change_set_id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(request.effective_from)
    .bind(&reason)
    .bind(request.idempotency_key.to_string())
    .bind(&request_hash)
    .bind(actor_user_id)
    .execute(&mut *transaction)
    .await?;

    let target_version_id = timetable_version_service::clone_draft_in_transaction(
        &mut transaction,
        actor_user_id,
        base_version_id,
        base_row_version,
        request.effective_from,
        Some(change_set_id),
    )
    .await?;

    sqlx::query(
        r#"UPDATE academic_term_change_sets
           SET base_timetable_version_id = $1,
               target_timetable_version_id = $2,
               updated_at = now()
           WHERE id = $3"#,
    )
    .bind(base_version_id)
    .bind(target_version_id)
    .bind(change_set_id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    append_audit(
        pool,
        "academic_term_change_set.created",
        "academic_term_change_set",
        change_set_id,
        term.academic_year_id,
        term.id,
        actor_user_id,
        serde_json::json!({
            "effectiveFrom": request.effective_from,
            "baseTimetableVersionId": base_version_id,
            "targetTimetableVersionId": target_version_id,
            "requestHash": request_hash,
        }),
    )
    .await?;
    get_change_set(pool, change_set_id).await
}

pub async fn update_change_set(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: UpdateAcademicTermChangeSetRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    validate_row_version(request.row_version)?;
    let reason = normalized_reason(&request.reason)?;
    let mut transaction = pool.begin().await?;
    let academic_term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_term_change_sets WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()))?;
    let term = require_writable_term(&mut transaction, academic_term_id, true).await?;
    validate_effective_date(&term, request.effective_from)?;
    let row = require_draft_change_set_for_update(&mut transaction, id).await?;
    if row.row_version != request.row_version {
        return Err(AppError::Conflict(
            "ชุดการเปลี่ยนแปลงถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    let target_version_id = required_version_id(
        row.target_timetable_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนเป้าหมาย",
    )?;
    let base_version_id = required_version_id(
        row.base_timetable_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนต้นทาง",
    )?;

    if request.effective_from != row.effective_from {
        if !target_is_pristine(&mut transaction, id, base_version_id, target_version_id).await? {
            return Err(AppError::Conflict(
                "มีรายการเปลี่ยนแปลงหรือแก้ตารางในแบบร่างแล้ว กรุณาสร้างชุดใหม่หากต้องเปลี่ยนวันที่เริ่มใช้"
                    .to_string(),
            ));
        }
        sqlx::query(
            r#"UPDATE academic_timetable_versions
               SET effective_from = $1, row_version = row_version + 1, updated_at = now()
               WHERE id = $2 AND status = 'draft'"#,
        )
        .bind(request.effective_from)
        .bind(target_version_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_live_effective_conflict)?;
    }

    sqlx::query(
        r#"UPDATE academic_term_change_sets
           SET effective_from = $1, reason = $2,
               row_version = row_version + 1, updated_at = now()
           WHERE id = $3"#,
    )
    .bind(request.effective_from)
    .bind(&reason)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    append_audit(
        pool,
        "academic_term_change_set.updated",
        "academic_term_change_set",
        id,
        row.academic_year_id,
        row.academic_term_id,
        actor_user_id,
        serde_json::json!({
            "effectiveFrom": request.effective_from,
            "rowVersion": request.row_version,
        }),
    )
    .await?;
    get_change_set(pool, id).await
}

pub async fn cancel_change_set(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: CancelAcademicTermChangeSetRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    validate_row_version(request.row_version)?;
    let mut transaction = pool.begin().await?;
    let academic_term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_term_change_sets WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()))?;
    require_writable_term(&mut transaction, academic_term_id, true).await?;
    let row = require_draft_change_set_for_update(&mut transaction, id).await?;
    if row.row_version != request.row_version {
        return Err(AppError::Conflict(
            "ชุดการเปลี่ยนแปลงถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    let target_version_id = required_version_id(
        row.target_timetable_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนเป้าหมาย",
    )?;

    sqlx::query(
        r#"UPDATE learning_offerings offering
           SET status = 'cancelled', row_version = row_version + 1, updated_at = now()
           FROM academic_term_change_items item
           WHERE item.change_set_id = $1
             AND item.action_kind = 'add_offering'
             AND offering.id = item.learning_offering_id
             AND offering.status = 'draft'"#,
    )
    .bind(id)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        r#"UPDATE academic_timetable_versions
           SET status = 'cancelled', row_version = row_version + 1, updated_at = now()
           WHERE id = $1 AND status = 'draft'"#,
    )
    .bind(target_version_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"UPDATE academic_term_change_sets
           SET status = 'cancelled', cancelled_by = $1, cancelled_at = now(),
               row_version = row_version + 1, updated_at = now()
           WHERE id = $2"#,
    )
    .bind(actor_user_id)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    append_audit(
        pool,
        "academic_term_change_set.cancelled",
        "academic_term_change_set",
        id,
        row.academic_year_id,
        row.academic_term_id,
        actor_user_id,
        serde_json::json!({ "rowVersion": request.row_version }),
    )
    .await?;
    get_change_set(pool, id).await
}

pub async fn upsert_change_item(
    pool: &PgPool,
    actor_user_id: Uuid,
    change_set_id: Uuid,
    request: UpsertAcademicTermChangeItemRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    let expected_change_set_row_version = request.change_set_row_version();
    validate_row_version(expected_change_set_row_version)?;
    let mut transaction = pool.begin().await?;
    let academic_term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_term_change_sets WHERE id = $1")
            .bind(change_set_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()))?;
    let term = require_writable_term(&mut transaction, academic_term_id, true).await?;
    let row = require_draft_change_set_for_update(&mut transaction, change_set_id).await?;
    if row.row_version != expected_change_set_row_version {
        return Err(AppError::Conflict(
            "ชุดการเปลี่ยนแปลงถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    let target_version_id = required_version_id(
        row.target_timetable_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนเป้าหมาย",
    )?;

    let (item_id, action_code, no_op) = match request {
        UpsertAcademicTermChangeItemRequest::AddCourse { offering, .. } => {
            if offering.academic_term_id != term.id {
                return Err(AppError::ValidationError(
                    "รายการเปิดสอนต้องอยู่ในภาคเรียนเดียวกับชุดการเปลี่ยนแปลง".to_string(),
                ));
            }
            require_active_owner(&mut transaction, offering.owning_organization_unit_id).await?;
            offerings::validate_targets(&mut transaction, &term, &offering.targets).await?;
            let subject_version_id = offering.subject_version_id;
            let offering_id = Uuid::new_v4();
            offerings::insert_course(&mut transaction, offering_id, &term, offering).await?;
            set_added_offering_start(&mut transaction, offering_id, row.effective_from).await?;
            let weekly_period_target: i32 =
                sqlx::query_scalar("SELECT periods_per_week FROM subject_versions WHERE id = $1")
                    .bind(subject_version_id)
                    .fetch_one(&mut *transaction)
                    .await?;
            insert_version_target(
                &mut transaction,
                target_version_id,
                offering_id,
                &term,
                weekly_period_target,
                change_set_id,
            )
            .await?;
            create_default_draft_groups(&mut transaction, offering_id, &term).await?;
            let item_id = insert_change_item(
                &mut transaction,
                change_set_id,
                &term,
                AcademicTermChangeActionKind::AddOffering,
                offering_id,
                Some(weekly_period_target),
                actor_user_id,
            )
            .await?;
            (item_id, "add_offering", false)
        }
        UpsertAcademicTermChangeItemRequest::AddActivity {
            weekly_period_target,
            offering,
            ..
        } => {
            validate_weekly_period_target(weekly_period_target)?;
            if offering.academic_term_id != term.id {
                return Err(AppError::ValidationError(
                    "รายการเปิดสอนต้องอยู่ในภาคเรียนเดียวกับชุดการเปลี่ยนแปลง".to_string(),
                ));
            }
            require_active_owner(&mut transaction, offering.owning_organization_unit_id).await?;
            offerings::validate_targets(&mut transaction, &term, &offering.targets).await?;
            let offering_id = Uuid::new_v4();
            offerings::insert_activity(&mut transaction, offering_id, &term, offering).await?;
            set_added_offering_start(&mut transaction, offering_id, row.effective_from).await?;
            insert_version_target(
                &mut transaction,
                target_version_id,
                offering_id,
                &term,
                weekly_period_target,
                change_set_id,
            )
            .await?;
            create_default_draft_groups(&mut transaction, offering_id, &term).await?;
            let item_id = insert_change_item(
                &mut transaction,
                change_set_id,
                &term,
                AcademicTermChangeActionKind::AddOffering,
                offering_id,
                Some(weekly_period_target),
                actor_user_id,
            )
            .await?;
            (item_id, "add_offering", false)
        }
        UpsertAcademicTermChangeItemRequest::StopOffering {
            item_row_version,
            learning_offering_id,
            ..
        } => {
            require_stoppable_offering(
                &mut transaction,
                change_set_id,
                &term,
                learning_offering_id,
                row.effective_from,
            )
            .await?;
            if let Some(existing) = find_change_item(
                &mut transaction,
                change_set_id,
                AcademicTermChangeActionKind::StopOffering,
                learning_offering_id,
            )
            .await?
            {
                if item_row_version != Some(existing.row_version) {
                    return Err(AppError::Conflict(
                        "รายการหยุดเปิดสอนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
                    ));
                }
                (existing.id, "stop_offering", true)
            } else {
                if item_row_version.is_some() {
                    return Err(AppError::Conflict(
                        "ไม่พบรายการหยุดเปิดสอนรุ่นที่ต้องการแก้ไข".to_string(),
                    ));
                }
                sqlx::query(
                    "DELETE FROM academic_timetable_entries \
                     WHERE timetable_version_id = $1 AND learning_offering_id = $2",
                )
                .bind(target_version_id)
                .bind(learning_offering_id)
                .execute(&mut *transaction)
                .await?;
                let deleted_target = sqlx::query(
                    "DELETE FROM academic_timetable_version_targets \
                     WHERE timetable_version_id = $1 AND learning_offering_id = $2",
                )
                .bind(target_version_id)
                .bind(learning_offering_id)
                .execute(&mut *transaction)
                .await?;
                if deleted_target.rows_affected() != 1 {
                    return Err(AppError::Conflict(
                        "รายการเปิดสอนนี้ไม่ได้อยู่ในรุ่นตารางเป้าหมาย".to_string(),
                    ));
                }
                let item_id = insert_change_item(
                    &mut transaction,
                    change_set_id,
                    &term,
                    AcademicTermChangeActionKind::StopOffering,
                    learning_offering_id,
                    None,
                    actor_user_id,
                )
                .await?;
                (item_id, "stop_offering", false)
            }
        }
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            item_row_version,
            learning_offering_id,
            weekly_period_target,
            ..
        } => {
            validate_weekly_period_target(weekly_period_target)?;
            ensure_no_action(
                &mut transaction,
                change_set_id,
                AcademicTermChangeActionKind::StopOffering,
                learning_offering_id,
                "หยุดและปรับจำนวนคาบของรายการเดียวกันในชุดเดียวไม่ได้",
            )
            .await?;
            let updated_target = sqlx::query(
                r#"UPDATE academic_timetable_version_targets
                   SET weekly_period_target = $1, updated_at = now()
                   WHERE timetable_version_id = $2 AND learning_offering_id = $3"#,
            )
            .bind(weekly_period_target)
            .bind(target_version_id)
            .bind(learning_offering_id)
            .execute(&mut *transaction)
            .await?;
            if updated_target.rows_affected() != 1 {
                return Err(AppError::Conflict(
                    "รายการเปิดสอนนี้ไม่มีเป้าหมายในรุ่นตารางแบบร่าง".to_string(),
                ));
            }
            if let Some(existing) = find_change_item(
                &mut transaction,
                change_set_id,
                AcademicTermChangeActionKind::AdjustWeeklyPeriodTarget,
                learning_offering_id,
            )
            .await?
            {
                if item_row_version != Some(existing.row_version) {
                    return Err(AppError::Conflict(
                        "รายการปรับจำนวนคาบถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
                    ));
                }
                sqlx::query(
                    r#"UPDATE academic_term_change_items
                       SET weekly_period_target = $1, row_version = row_version + 1,
                           updated_at = now()
                       WHERE id = $2"#,
                )
                .bind(weekly_period_target)
                .bind(existing.id)
                .execute(&mut *transaction)
                .await?;
                (existing.id, "adjust_weekly_period_target", false)
            } else {
                if item_row_version.is_some() {
                    return Err(AppError::Conflict(
                        "ไม่พบรายการปรับจำนวนคาบรุ่นที่ต้องการแก้ไข".to_string(),
                    ));
                }
                let item_id = insert_change_item(
                    &mut transaction,
                    change_set_id,
                    &term,
                    AcademicTermChangeActionKind::AdjustWeeklyPeriodTarget,
                    learning_offering_id,
                    Some(weekly_period_target),
                    actor_user_id,
                )
                .await?;
                (item_id, "adjust_weekly_period_target", false)
            }
        }
    };

    if no_op {
        transaction.commit().await?;
        return get_change_set(pool, change_set_id).await;
    }
    increment_change_set_revision(&mut transaction, change_set_id).await?;
    transaction.commit().await?;
    append_audit(
        pool,
        "academic_term_change_item.upserted",
        "academic_term_change_item",
        item_id,
        row.academic_year_id,
        row.academic_term_id,
        actor_user_id,
        serde_json::json!({
            "changeSetId": change_set_id,
            "action": action_code,
            "changeSetRowVersion": expected_change_set_row_version,
        }),
    )
    .await?;
    get_change_set(pool, change_set_id).await
}

pub async fn delete_change_item(
    pool: &PgPool,
    actor_user_id: Uuid,
    change_set_id: Uuid,
    item_id: Uuid,
    request: DeleteAcademicTermChangeItemRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    validate_row_version(request.change_set_row_version)?;
    validate_row_version(request.item_row_version)?;
    let mut transaction = pool.begin().await?;
    let academic_term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_term_change_sets WHERE id = $1")
            .bind(change_set_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()))?;
    require_writable_term(&mut transaction, academic_term_id, true).await?;
    let row = require_draft_change_set_for_update(&mut transaction, change_set_id).await?;
    if row.row_version != request.change_set_row_version {
        return Err(AppError::Conflict(
            "ชุดการเปลี่ยนแปลงถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    let item = lock_change_item(&mut transaction, change_set_id, item_id).await?;
    if item.row_version != request.item_row_version {
        return Err(AppError::Conflict(
            "รายการเปลี่ยนแปลงถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    let base_version_id = required_version_id(
        row.base_timetable_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนต้นทาง",
    )?;
    let target_version_id = required_version_id(
        row.target_timetable_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนเป้าหมาย",
    )?;

    match item.action_kind {
        AcademicTermChangeActionKind::AddOffering => {
            require_draft_only_delete(&mut transaction, item.learning_offering_id).await?;
            sqlx::query(
                "DELETE FROM academic_timetable_entries \
                 WHERE timetable_version_id = $1 AND learning_offering_id = $2",
            )
            .bind(target_version_id)
            .bind(item.learning_offering_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "DELETE FROM academic_timetable_version_targets \
                 WHERE timetable_version_id = $1 AND learning_offering_id = $2",
            )
            .bind(target_version_id)
            .bind(item.learning_offering_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query("DELETE FROM academic_term_change_items WHERE id = $1")
                .bind(item.id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM learning_groups WHERE learning_offering_id = $1")
                .bind(item.learning_offering_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM learning_offerings WHERE id = $1 AND status = 'draft'")
                .bind(item.learning_offering_id)
                .execute(&mut *transaction)
                .await?;
        }
        AcademicTermChangeActionKind::StopOffering => {
            sqlx::query("DELETE FROM academic_term_change_items WHERE id = $1")
                .bind(item.id)
                .execute(&mut *transaction)
                .await?;
            restore_version_target(
                &mut transaction,
                base_version_id,
                target_version_id,
                item.learning_offering_id,
            )
            .await?;
            restore_version_entries(
                &mut transaction,
                actor_user_id,
                base_version_id,
                target_version_id,
                item.learning_offering_id,
            )
            .await?;
        }
        AcademicTermChangeActionKind::AdjustWeeklyPeriodTarget => {
            sqlx::query("DELETE FROM academic_term_change_items WHERE id = $1")
                .bind(item.id)
                .execute(&mut *transaction)
                .await?;
            restore_version_target(
                &mut transaction,
                base_version_id,
                target_version_id,
                item.learning_offering_id,
            )
            .await?;
        }
    }
    increment_change_set_revision(&mut transaction, change_set_id).await?;
    transaction.commit().await?;
    append_audit(
        pool,
        "academic_term_change_item.deleted",
        "academic_term_change_item",
        item_id,
        row.academic_year_id,
        row.academic_term_id,
        actor_user_id,
        serde_json::json!({
            "changeSetId": change_set_id,
            "action": item.action_kind,
            "learningOfferingId": item.learning_offering_id,
        }),
    )
    .await?;
    get_change_set(pool, change_set_id).await
}

async fn set_added_offering_start(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    effective_from: NaiveDate,
) -> Result<(), AppError> {
    sqlx::query("UPDATE learning_offerings SET starts_on = $1, updated_at = now() WHERE id = $2")
        .bind(effective_from)
        .bind(offering_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn insert_version_target(
    transaction: &mut Transaction<'_, Postgres>,
    target_version_id: Uuid,
    offering_id: Uuid,
    term: &TermContext,
    weekly_period_target: i32,
    change_set_id: Uuid,
) -> Result<(), AppError> {
    validate_weekly_period_target(weekly_period_target)?;
    sqlx::query(
        r#"INSERT INTO academic_timetable_version_targets (
               timetable_version_id, learning_offering_id, academic_term_id,
               academic_year_id, weekly_period_target, migration_provenance
           ) VALUES ($1, $2, $3, $4, $5, jsonb_build_object('changeSetId', $6::text))"#,
    )
    .bind(target_version_id)
    .bind(offering_id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(weekly_period_target)
    .bind(change_set_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn create_default_draft_groups(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    term: &TermContext,
) -> Result<Vec<Uuid>, AppError> {
    let offering_name: String =
        sqlx::query_scalar("SELECT name_snapshot FROM learning_offerings WHERE id = $1")
            .bind(offering_id)
            .fetch_one(&mut **transaction)
            .await?;
    let homerooms: Vec<(Uuid, String)> = sqlx::query_as(
        r#"SELECT DISTINCT homeroom.id, homeroom.name
           FROM learning_offering_targets target
           JOIN homerooms homeroom
             ON homeroom.academic_year_id = target.academic_year_id
            AND (
                (target.target_kind = 'homeroom' AND homeroom.id = target.homeroom_id)
                OR
                (target.target_kind = 'grade_program'
                 AND homeroom.grade_level_id = target.grade_level_id
                 AND homeroom.study_program_id = target.study_program_id)
            )
           WHERE target.learning_offering_id = $1
           ORDER BY homeroom.name, homeroom.id"#,
    )
    .bind(offering_id)
    .fetch_all(&mut **transaction)
    .await?;
    let mut group_ids = Vec::with_capacity(homerooms.len());
    for (index, (homeroom_id, homeroom_name)) in homerooms.into_iter().enumerate() {
        let group_id = Uuid::new_v4();
        let suffix = i32::try_from(index + 1)
            .map_err(|_| AppError::ValidationError("จำนวนกลุ่มเรียนมากเกินไป".to_string()))?;
        sqlx::query(
            r#"INSERT INTO learning_groups (
                   id, learning_offering_id, academic_term_id, academic_year_id,
                   code, name, status, roster_status
               ) VALUES ($1, $2, $3, $4, $5, $6, 'draft', 'draft')"#,
        )
        .bind(group_id)
        .bind(offering_id)
        .bind(term.id)
        .bind(term.academic_year_id)
        .bind(format!("MID-{suffix:03}"))
        .bind(format!("{} · {}", offering_name, homeroom_name))
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            r#"INSERT INTO learning_group_homerooms (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   homeroom_id, coverage_source
               ) VALUES ($1, $2, $3, $4, $5, 'operational_change')"#,
        )
        .bind(Uuid::new_v4())
        .bind(group_id)
        .bind(term.id)
        .bind(term.academic_year_id)
        .bind(homeroom_id)
        .execute(&mut **transaction)
        .await?;
        group_ids.push(group_id);
    }
    Ok(group_ids)
}

async fn insert_change_item(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    term: &TermContext,
    action_kind: AcademicTermChangeActionKind,
    learning_offering_id: Uuid,
    weekly_period_target: Option<i32>,
    actor_user_id: Uuid,
) -> Result<Uuid, AppError> {
    let item_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_term_change_items (
               id, change_set_id, academic_term_id, academic_year_id, action_kind,
               learning_offering_id, weekly_period_target, created_by
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(item_id)
    .bind(change_set_id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(action_kind)
    .bind(learning_offering_id)
    .bind(weekly_period_target)
    .bind(actor_user_id)
    .execute(&mut **transaction)
    .await?;
    Ok(item_id)
}

async fn require_stoppable_offering(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    term: &TermContext,
    offering_id: Uuid,
    effective_from: NaiveDate,
) -> Result<(), AppError> {
    ensure_no_action(
        transaction,
        change_set_id,
        AcademicTermChangeActionKind::AddOffering,
        offering_id,
        "รายการที่เพิ่งเพิ่มในชุดเดียวกันควรลบรายการเพิ่ม ไม่ต้องสร้างรายการหยุด",
    )
    .await?;
    ensure_no_action(
        transaction,
        change_set_id,
        AcademicTermChangeActionKind::AdjustWeeklyPeriodTarget,
        offering_id,
        "หยุดและปรับจำนวนคาบของรายการเดียวกันในชุดเดียวไม่ได้",
    )
    .await?;
    let (offering_term_id, offering_year_id, status, starts_on, ends_on): (
        Uuid,
        Uuid,
        LearningOfferingStatus,
        NaiveDate,
        Option<NaiveDate>,
    ) = sqlx::query_as(
        r#"SELECT academic_term_id, academic_year_id, status, starts_on, ends_on
           FROM learning_offerings
           WHERE id = $1
           FOR UPDATE"#,
    )
    .bind(offering_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายการเปิดสอนที่ต้องการหยุด".to_string()))?;
    if offering_term_id != term.id || offering_year_id != term.academic_year_id {
        return Err(AppError::ValidationError(
            "รายการเปิดสอนไม่อยู่ในภาคเรียนของชุดการเปลี่ยนแปลง".to_string(),
        ));
    }
    if status != LearningOfferingStatus::Published {
        return Err(AppError::Conflict(
            "หยุดได้เฉพาะรายการเปิดสอนที่เผยแพร่แล้ว".to_string(),
        ));
    }
    if effective_from < starts_on || ends_on.is_some_and(|end| effective_from > end) {
        return Err(AppError::ValidationError(
            "รายการเปิดสอนไม่ได้เปิดใช้งานในวันที่เลือก".to_string(),
        ));
    }
    Ok(())
}

async fn find_change_item(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    action_kind: AcademicTermChangeActionKind,
    offering_id: Uuid,
) -> Result<Option<ChangeItemRow>, AppError> {
    Ok(sqlx::query_as(
        r#"SELECT id, change_set_id, action_kind, learning_offering_id,
                  weekly_period_target, row_version, created_by, created_at, updated_at
           FROM academic_term_change_items
           WHERE change_set_id = $1 AND action_kind = $2 AND learning_offering_id = $3
           FOR UPDATE"#,
    )
    .bind(change_set_id)
    .bind(action_kind)
    .bind(offering_id)
    .fetch_optional(&mut **transaction)
    .await?)
}

async fn lock_change_item(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    item_id: Uuid,
) -> Result<ChangeItemRow, AppError> {
    sqlx::query_as(
        r#"SELECT id, change_set_id, action_kind, learning_offering_id,
                  weekly_period_target, row_version, created_by, created_at, updated_at
           FROM academic_term_change_items
           WHERE change_set_id = $1 AND id = $2
           FOR UPDATE"#,
    )
    .bind(change_set_id)
    .bind(item_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายการเปลี่ยนแปลง".to_string()))
}

async fn ensure_no_action(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    action_kind: AcademicTermChangeActionKind,
    offering_id: Uuid,
    message: &str,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM academic_term_change_items
               WHERE change_set_id = $1 AND action_kind = $2 AND learning_offering_id = $3
           )"#,
    )
    .bind(change_set_id)
    .bind(action_kind)
    .bind(offering_id)
    .fetch_one(&mut **transaction)
    .await?;
    if exists {
        Err(AppError::Conflict(message.to_string()))
    } else {
        Ok(())
    }
}

async fn increment_change_set_revision(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE academic_term_change_sets \
         SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(change_set_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn require_draft_only_delete(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
) -> Result<(), AppError> {
    let (status, downstream_count): (LearningOfferingStatus, i64) = sqlx::query_as(
        r#"SELECT offering.status,
                  (SELECT count(*) FROM academic_timetable_entries entry
                    WHERE entry.learning_offering_id = offering.id)
                + (SELECT count(*) FROM course_assessment_plans plan
                    WHERE plan.learning_offering_id = offering.id)
                + (SELECT count(*) FROM learning_results result
                    WHERE result.learning_offering_id = offering.id)
                + (SELECT count(*) FROM academic_exam_schedule_items item
                    WHERE item.learning_offering_id = offering.id)
                + (SELECT count(*) FROM supervision_observations observation
                    JOIN learning_groups learning_group
                      ON learning_group.id = observation.learning_group_id
                    WHERE learning_group.learning_offering_id = offering.id)
           FROM learning_offerings offering
           WHERE offering.id = $1
           FOR UPDATE"#,
    )
    .bind(offering_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายการเปิดสอนฉบับร่าง".to_string()))?;
    if status != LearningOfferingStatus::Draft || downstream_count != 0 {
        return Err(AppError::Conflict(
            "ลบถาวรได้เฉพาะรายการฉบับร่างที่ยังไม่มีตาราง แผนคะแนน ผลการเรียน หรือข้อมูลปลายทาง".to_string(),
        ));
    }
    Ok(())
}

async fn restore_version_target(
    transaction: &mut Transaction<'_, Postgres>,
    base_version_id: Uuid,
    target_version_id: Uuid,
    offering_id: Uuid,
) -> Result<(), AppError> {
    let restored = sqlx::query(
        r#"INSERT INTO academic_timetable_version_targets (
               timetable_version_id, learning_offering_id, academic_term_id,
               academic_year_id, weekly_period_target, migration_provenance
           )
           SELECT $2, base.learning_offering_id, base.academic_term_id,
                  base.academic_year_id, base.weekly_period_target,
                  base.migration_provenance || jsonb_build_object(
                      'restoredFromVersionId', $1::text
                  )
           FROM academic_timetable_version_targets base
           WHERE base.timetable_version_id = $1
             AND base.learning_offering_id = $3
           ON CONFLICT (timetable_version_id, learning_offering_id)
           DO UPDATE SET weekly_period_target = EXCLUDED.weekly_period_target,
                         migration_provenance = EXCLUDED.migration_provenance,
                         updated_at = now()"#,
    )
    .bind(base_version_id)
    .bind(target_version_id)
    .bind(offering_id)
    .execute(&mut **transaction)
    .await?;
    if restored.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "รุ่นตารางต้นทางไม่มีเป้าหมายของรายการนี้ให้คืนค่า".to_string(),
        ));
    }
    Ok(())
}

async fn restore_version_entries(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    base_version_id: Uuid,
    target_version_id: Uuid,
    offering_id: Uuid,
) -> Result<(), AppError> {
    let existing_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_entries \
         WHERE timetable_version_id = $1 AND learning_offering_id = $2",
    )
    .bind(target_version_id)
    .bind(offering_id)
    .fetch_one(&mut **transaction)
    .await?;
    if existing_count != 0 {
        return Err(AppError::Conflict(
            "รุ่นตารางเป้าหมายมีคาบของรายการนี้อยู่แล้ว กรุณาโหลดข้อมูลล่าสุด".to_string(),
        ));
    }
    sqlx::query(
        r#"WITH source_entries AS MATERIALIZED (
               SELECT entry.*,
                      gen_random_uuid() AS new_entry_id,
                      CASE
                          WHEN entry.batch_id IS NULL THEN NULL
                          ELSE uuid_generate_v5(
                              $2,
                              'restore-batch:' || entry.batch_id::text
                          )
                      END AS new_batch_id
               FROM academic_timetable_entries entry
               WHERE entry.timetable_version_id = $1
                 AND entry.learning_offering_id = $3
                 AND entry.is_active
           ), inserted_entries AS (
               INSERT INTO academic_timetable_entries (
                   id, day_of_week, bell_schedule_period_id, room_id, note,
                   is_active, created_by, updated_by, entry_type, title,
                   homeroom_id, academic_term_id, batch_id, academic_year_id,
                   learning_offering_id, learning_group_id, bell_schedule_id,
                   migration_provenance, row_version, timetable_version_id,
                   created_at, updated_at
               )
               SELECT source.new_entry_id, source.day_of_week,
                      source.bell_schedule_period_id, source.room_id, source.note,
                      true, $4, $4, source.entry_type, source.title,
                      source.homeroom_id, source.academic_term_id, source.new_batch_id,
                      source.academic_year_id, source.learning_offering_id,
                      source.learning_group_id, source.bell_schedule_id,
                      source.migration_provenance || jsonb_build_object(
                          'restoredFromEntryId', source.id::text,
                          'sourceVersionId', $1::text
                      ),
                      1, $2, now(), now()
               FROM source_entries source
               RETURNING id
           )
           INSERT INTO timetable_entry_instructors (id, entry_id, instructor_id, role)
           SELECT gen_random_uuid(), source.new_entry_id,
                  instructor.instructor_id, instructor.role
           FROM source_entries source
           JOIN inserted_entries inserted ON inserted.id = source.new_entry_id
           JOIN timetable_entry_instructors instructor ON instructor.entry_id = source.id"#,
    )
    .bind(base_version_id)
    .bind(target_version_id)
    .bind(offering_id)
    .bind(actor_user_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn validate_weekly_period_target(value: i32) -> Result<(), AppError> {
    if value <= 0 {
        Err(AppError::ValidationError(
            "จำนวนคาบต่อสัปดาห์ต้องมากกว่าศูนย์".to_string(),
        ))
    } else {
        Ok(())
    }
}

async fn require_draft_change_set_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<ChangeSetRow, AppError> {
    let query = format!(
        "SELECT {CHANGE_SET_COLUMNS} FROM academic_term_change_sets \
         WHERE id = $1 FOR UPDATE"
    );
    let row = sqlx::query_as::<_, ChangeSetRow>(&query)
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()))?;
    if row.status != AcademicTermChangeSetStatus::Draft {
        return Err(AppError::Conflict(
            "แก้ไขหรือยกเลิกได้เฉพาะชุดการเปลี่ยนแปลงฉบับร่าง".to_string(),
        ));
    }
    Ok(row)
}

async fn target_is_pristine(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    base_version_id: Uuid,
    target_version_id: Uuid,
) -> Result<bool, AppError> {
    let pristine: bool = sqlx::query_scalar(
        r#"SELECT
               NOT EXISTS (
                   SELECT 1 FROM academic_term_change_items WHERE change_set_id = $1
               )
               AND NOT EXISTS (
                   SELECT 1
                   FROM academic_timetable_entries target
                   WHERE target.timetable_version_id = $3
                     AND (
                         target.row_version <> 1
                         OR target.migration_provenance ->> 'sourceVersionId' <> $2::text
                     )
               )
               AND (
                   SELECT count(*) FROM academic_timetable_entries
                   WHERE timetable_version_id = $3 AND is_active
               ) = (
                   SELECT count(*) FROM academic_timetable_entries
                   WHERE timetable_version_id = $2 AND is_active
               )
               AND NOT EXISTS (
                   SELECT 1
                   FROM academic_timetable_version_targets target
                   FULL JOIN academic_timetable_version_targets base
                     ON base.learning_offering_id = target.learning_offering_id
                    AND base.timetable_version_id = $2
                   WHERE target.timetable_version_id = $3
                     AND (
                         base.learning_offering_id IS NULL
                         OR target.learning_offering_id IS NULL
                         OR target.weekly_period_target <> base.weekly_period_target
                     )
               )
               AND (
                   SELECT count(*) FROM academic_timetable_version_targets
                   WHERE timetable_version_id = $3
               ) = (
                   SELECT count(*) FROM academic_timetable_version_targets
                   WHERE timetable_version_id = $2
               )"#,
    )
    .bind(change_set_id)
    .bind(base_version_id)
    .bind(target_version_id)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(pristine)
}

fn normalized_reason(reason: &str) -> Result<String, AppError> {
    let value = reason.trim();
    if value.is_empty() {
        return Err(AppError::ValidationError(
            "กรุณาระบุเหตุผลของการเปลี่ยนแปลง".to_string(),
        ));
    }
    if value.chars().count() > 1000 {
        return Err(AppError::ValidationError(
            "เหตุผลต้องไม่เกิน 1,000 ตัวอักษร".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn validate_effective_date(term: &TermContext, effective_from: NaiveDate) -> Result<(), AppError> {
    if effective_from < term.start_date || effective_from > term.academic_year_end_date {
        return Err(AppError::ValidationError(
            "วันที่เริ่มใช้ต้องอยู่ตั้งแต่วันเปิดภาคเรียนถึงวันสิ้นสุดปีการศึกษา".to_string(),
        ));
    }
    if term.status == "active" && effective_from < Utc::now().date_naive() {
        return Err(AppError::ValidationError(
            "ภาคเรียนที่เปิดใช้งานแล้วไม่สามารถกำหนดวันที่เริ่มใช้ย้อนหลังได้".to_string(),
        ));
    }
    Ok(())
}

fn required_version_id(value: Option<Uuid>, message: &str) -> Result<Uuid, AppError> {
    value.ok_or_else(|| AppError::InternalServerError(message.to_string()))
}

async fn hydrate_many(
    pool: &PgPool,
    rows: Vec<ChangeSetRow>,
) -> Result<Vec<AcademicTermChangeSet>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let item_rows: Vec<ChangeItemRow> = sqlx::query_as(
        r#"SELECT id, change_set_id, action_kind, learning_offering_id,
                  weekly_period_target, row_version, created_by, created_at, updated_at
           FROM academic_term_change_items
           WHERE change_set_id = ANY($1)
           ORDER BY change_set_id, created_at, id"#,
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let mut items_by_change_set = HashMap::<Uuid, Vec<AcademicTermChangeItem>>::new();
    for row in item_rows {
        let item = match row.action_kind {
            AcademicTermChangeActionKind::AddOffering => AcademicTermChangeItem::AddOffering {
                id: row.id,
                learning_offering_id: row.learning_offering_id,
                weekly_period_target: row.weekly_period_target.ok_or_else(|| {
                    AppError::InternalServerError(
                        "รายการเพิ่มการเปิดสอนไม่มีจำนวนคาบเป้าหมาย".to_string(),
                    )
                })?,
                row_version: row.row_version,
                created_by: row.created_by,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            AcademicTermChangeActionKind::StopOffering => AcademicTermChangeItem::StopOffering {
                id: row.id,
                learning_offering_id: row.learning_offering_id,
                row_version: row.row_version,
                created_by: row.created_by,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            AcademicTermChangeActionKind::AdjustWeeklyPeriodTarget => {
                AcademicTermChangeItem::AdjustWeeklyPeriodTarget {
                    id: row.id,
                    learning_offering_id: row.learning_offering_id,
                    weekly_period_target: row.weekly_period_target.ok_or_else(|| {
                        AppError::InternalServerError("รายการปรับจำนวนคาบไม่มีค่าเป้าหมาย".to_string())
                    })?,
                    row_version: row.row_version,
                    created_by: row.created_by,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                }
            }
        };
        items_by_change_set
            .entry(row.change_set_id)
            .or_default()
            .push(item);
    }

    rows.into_iter()
        .map(|row| {
            Ok(AcademicTermChangeSet {
                id: row.id,
                academic_term_id: row.academic_term_id,
                academic_year_id: row.academic_year_id,
                effective_from: row.effective_from,
                reason: row.reason,
                status: row.status,
                base_timetable_version_id: required_version_id(
                    row.base_timetable_version_id,
                    "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนต้นทาง",
                )?,
                target_timetable_version_id: required_version_id(
                    row.target_timetable_version_id,
                    "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนเป้าหมาย",
                )?,
                row_version: row.row_version,
                created_by: row.created_by,
                published_by: row.published_by,
                published_at: row.published_at,
                cancelled_by: row.cancelled_by,
                cancelled_at: row.cancelled_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
                items: items_by_change_set.remove(&row.id).unwrap_or_default(),
            })
        })
        .collect()
}

fn map_live_effective_conflict(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("academic_timetable_versions_live_effective_key") {
            return AppError::Conflict("มีรุ่นตารางเรียนแบบร่างหรือเผยแพร่ในวันที่นี้แล้ว".to_string());
        }
    }
    AppError::DbError(error)
}
