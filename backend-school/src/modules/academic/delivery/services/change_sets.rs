use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::delivery::models::{
    AcademicTermChangeActionKind, AcademicTermChangeItem, AcademicTermChangeSet,
    AcademicTermChangeSetStatus, CancelAcademicTermChangeSetRequest,
    CreateAcademicTermChangeSetRequest, UpdateAcademicTermChangeSetRequest,
};
use crate::modules::academic::services::timetable_version_service;

use super::{append_audit, require_writable_term, stable_hash, validate_row_version, TermContext};

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
