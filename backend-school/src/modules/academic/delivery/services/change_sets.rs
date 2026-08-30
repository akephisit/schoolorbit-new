use std::collections::{BTreeSet, HashMap};

use chrono::{NaiveDate, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::delivery::models::{
    AcademicChangeFinding, AcademicChangeFindingCode, AcademicChangeFindingSeverity,
    AcademicChangeImpactCounts, AcademicOfferingScheduleCount, AcademicTermChangeActionKind,
    AcademicTermChangeItem, AcademicTermChangeSet, AcademicTermChangeSetPreview,
    AcademicTermChangeSetStatus, CancelAcademicTermChangeSetRequest,
    CreateAcademicTermChangeSetRequest, DeleteAcademicTermChangeItemRequest,
    LearningOfferingStatus, PublishAcademicTermChangeSetRequest,
    UpdateAcademicTermChangeSetRequest, UpsertAcademicTermChangeItemRequest,
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

#[derive(Debug, FromRow)]
struct TargetVersionPreviewRow {
    id: Uuid,
    status: String,
    row_version: i64,
    change_set_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct ScheduleCountRow {
    learning_offering_id: Uuid,
    learning_group_id: Uuid,
    offering_label: String,
    learning_group_label: String,
    actual_periods: i64,
    target_periods: i32,
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

pub async fn preview_change_set(
    pool: &PgPool,
    id: Uuid,
) -> Result<AcademicTermChangeSetPreview, AppError> {
    let mut transaction = pool.begin().await?;
    let preview = build_preview_in_transaction(&mut transaction, id, false).await?;
    transaction.rollback().await?;
    Ok(preview)
}

pub async fn publish_change_set(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: PublishAcademicTermChangeSetRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    validate_row_version(request.row_version)?;
    validate_row_version(request.target_timetable_version_row_version)?;
    if request.preview_hash.len() != 64
        || !request
            .preview_hash
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err(AppError::ValidationError(
            "previewHash ต้องเป็น SHA-256 ตัวพิมพ์เล็ก 64 ตัวอักษร".to_string(),
        ));
    }
    let mut acknowledged_warning_codes = request.acknowledged_warning_codes.clone();
    acknowledged_warning_codes.sort_unstable();
    acknowledged_warning_codes.dedup();
    if acknowledged_warning_codes.len() != request.acknowledged_warning_codes.len() {
        return Err(AppError::ValidationError(
            "รหัสคำเตือนที่ยืนยันต้องไม่ซ้ำกัน".to_string(),
        ));
    }
    let publication_request_hash = stable_hash(&(
        id,
        request.row_version,
        request.target_timetable_version_row_version,
        &request.preview_hash,
        &acknowledged_warning_codes,
        request.idempotency_key,
    ))?;

    if let Some((existing_id, existing_hash)) = sqlx::query_as::<_, (Uuid, String)>(
        r#"SELECT id, publication_request_hash
           FROM academic_term_change_sets
           WHERE publication_idempotency_key = $1"#,
    )
    .bind(request.idempotency_key)
    .fetch_optional(pool)
    .await?
    {
        if existing_id == id && existing_hash == publication_request_hash {
            return get_change_set(pool, id).await;
        }
        return Err(AppError::Conflict(
            "idempotencyKey การเผยแพร่นี้ถูกใช้กับคำขออื่นแล้ว".to_string(),
        ));
    }

    let (academic_term_id, current_status): (Uuid, AcademicTermChangeSetStatus) = sqlx::query_as(
        "SELECT academic_term_id, status FROM academic_term_change_sets WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()))?;
    if current_status != AcademicTermChangeSetStatus::Draft {
        return Err(AppError::Conflict(
            "ชุดการเปลี่ยนแปลงนี้เผยแพร่หรือยกเลิกแล้ว".to_string(),
        ));
    }

    let mut transaction = pool.begin().await?;
    require_writable_term(&mut transaction, academic_term_id, true).await?;
    let preview = build_preview_in_transaction(&mut transaction, id, true).await?;
    if preview.change_set_row_version != request.row_version {
        return Err(AppError::Conflict(
            "ชุดการเปลี่ยนแปลงถูกแก้ไขหลังการตรวจ กรุณาตรวจความพร้อมใหม่".to_string(),
        ));
    }
    if preview.target_timetable_version_row_version != request.target_timetable_version_row_version
    {
        return Err(AppError::Conflict(
            "รุ่นตารางแบบร่างถูกแก้ไขหลังการตรวจ กรุณาตรวจความพร้อมใหม่".to_string(),
        ));
    }
    if preview.preview_hash != request.preview_hash {
        return Err(AppError::Conflict(
            "ข้อมูลที่ใช้ตรวจความพร้อมเปลี่ยนไป กรุณาตรวจความพร้อมใหม่".to_string(),
        ));
    }
    let blocking_count = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking)
        .count();
    if blocking_count > 0 {
        return Err(AppError::ValidationError(format!(
            "ยังมีเงื่อนไขที่ต้องแก้ไข {blocking_count} รายการก่อนเผยแพร่"
        )));
    }
    let current_warning_codes = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Warning)
        .map(|finding| finding.code)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if acknowledged_warning_codes != current_warning_codes {
        return Err(AppError::Conflict(
            "คำเตือนที่ยืนยันไม่ตรงกับผลตรวจล่าสุด กรุณาตรวจและยืนยันใหม่".to_string(),
        ));
    }

    let change_set = sqlx::query_as::<_, ChangeSetRow>(&format!(
        "SELECT {CHANGE_SET_COLUMNS} FROM academic_term_change_sets WHERE id = $1"
    ))
    .bind(id)
    .fetch_one(&mut *transaction)
    .await?;
    let target_version_id = required_version_id(
        change_set.target_timetable_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนเป้าหมาย",
    )?;
    let item_rows: Vec<ChangeItemRow> = sqlx::query_as(
        r#"SELECT id, change_set_id, action_kind, learning_offering_id,
                  weekly_period_target, row_version, created_by, created_at, updated_at
           FROM academic_term_change_items
           WHERE change_set_id = $1 ORDER BY id"#,
    )
    .bind(id)
    .fetch_all(&mut *transaction)
    .await?;
    let add_offering_ids = item_rows
        .iter()
        .filter(|item| item.action_kind == AcademicTermChangeActionKind::AddOffering)
        .map(|item| item.learning_offering_id)
        .collect::<Vec<_>>();
    let stop_offering_ids = item_rows
        .iter()
        .filter(|item| item.action_kind == AcademicTermChangeActionKind::StopOffering)
        .map(|item| item.learning_offering_id)
        .collect::<Vec<_>>();

    if !add_offering_ids.is_empty() {
        let added_group_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM learning_groups WHERE learning_offering_id = ANY($1) ORDER BY id",
        )
        .bind(&add_offering_ids)
        .fetch_all(&mut *transaction)
        .await?;
        if !added_group_ids.is_empty() {
            sqlx::query(
                r#"UPDATE learning_group_students
                   SET published_at = COALESCE(published_at, now()), updated_at = now()
                   WHERE learning_group_id = ANY($1) AND membership_status = 'active'"#,
            )
            .bind(&added_group_ids)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                r#"UPDATE learning_groups
                   SET status = 'published', roster_status = 'published',
                       roster_published_at = now(),
                       roster_publish_idempotency_key = uuid_generate_v5(
                           $2, 'change-set-roster:' || id::text
                       ),
                       row_version = row_version + 1, updated_at = now()
                   WHERE id = ANY($1) AND status = 'draft'"#,
            )
            .bind(&added_group_ids)
            .bind(request.idempotency_key)
            .execute(&mut *transaction)
            .await?;
        }
        let published_offerings = sqlx::query(
            r#"UPDATE learning_offerings
               SET status = 'published', published_at = now(),
                   publish_idempotency_key = uuid_generate_v5(
                       $2, 'change-set-offering:' || id::text
                   ),
                   row_version = row_version + 1, updated_at = now()
               WHERE id = ANY($1) AND status = 'draft'"#,
        )
        .bind(&add_offering_ids)
        .bind(request.idempotency_key)
        .execute(&mut *transaction)
        .await?;
        if published_offerings.rows_affected() != add_offering_ids.len() as u64 {
            return Err(AppError::Conflict(
                "รายการเปิดสอนที่เพิ่มเปลี่ยนสถานะก่อนเผยแพร่ กรุณาตรวจความพร้อมใหม่".to_string(),
            ));
        }
    }

    if !stop_offering_ids.is_empty() {
        let ends_on = change_set
            .effective_from
            .checked_sub_signed(chrono::Duration::days(1))
            .ok_or_else(|| AppError::ValidationError("วันที่เริ่มใช้ไม่ถูกต้อง".to_string()))?;
        let stopped = sqlx::query(
            r#"UPDATE learning_offerings
               SET ends_on = $1, stop_reason = $2, stopped_at = now(),
                   stopped_by = $3, stop_change_set_id = $4,
                   row_version = row_version + 1, updated_at = now()
               WHERE id = ANY($5) AND status = 'published' AND ends_on IS NULL"#,
        )
        .bind(ends_on)
        .bind(&change_set.reason)
        .bind(actor_user_id)
        .bind(change_set.id)
        .bind(&stop_offering_ids)
        .execute(&mut *transaction)
        .await?;
        if stopped.rows_affected() != stop_offering_ids.len() as u64 {
            return Err(AppError::Conflict(
                "รายการเปิดสอนที่จะหยุดเปลี่ยนไป กรุณาตรวจความพร้อมใหม่".to_string(),
            ));
        }
    }

    let published_version = sqlx::query(
        r#"UPDATE academic_timetable_versions
           SET status = 'published', published_by = $1, published_at = now(),
               row_version = row_version + 1, updated_at = now()
           WHERE id = $2 AND status = 'draft' AND row_version = $3"#,
    )
    .bind(actor_user_id)
    .bind(target_version_id)
    .bind(request.target_timetable_version_row_version)
    .execute(&mut *transaction)
    .await?;
    if published_version.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "รุ่นตารางแบบร่างเปลี่ยนไป กรุณาตรวจความพร้อมใหม่".to_string(),
        ));
    }
    let warning_code_values = acknowledged_warning_codes
        .iter()
        .map(|code| finding_code_text(*code).to_string())
        .collect::<Vec<_>>();
    let published_change_set = sqlx::query(
        r#"UPDATE academic_term_change_sets
           SET status = 'published', published_by = $1, published_at = now(),
               publication_idempotency_key = $2, publication_request_hash = $3,
               acknowledged_warning_codes = $4,
               row_version = row_version + 1, updated_at = now()
           WHERE id = $5 AND status = 'draft' AND row_version = $6"#,
    )
    .bind(actor_user_id)
    .bind(request.idempotency_key)
    .bind(&publication_request_hash)
    .bind(&warning_code_values)
    .bind(change_set.id)
    .bind(request.row_version)
    .execute(&mut *transaction)
    .await?;
    if published_change_set.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "ชุดการเปลี่ยนแปลงเปลี่ยนไป กรุณาตรวจความพร้อมใหม่".to_string(),
        ));
    }
    let item_count = i32::try_from(item_rows.len())
        .map_err(|_| AppError::ValidationError("จำนวนรายการเปลี่ยนแปลงมากเกินไป".to_string()))?;

    sqlx::query(
        r#"INSERT INTO academic_audit_events (
               event_code, entity_type, entity_id, academic_year_id,
               academic_term_id, actor_user_id, payload
           ) VALUES (
               'academic_term_change_set.published', 'academic_term_change_set',
               $1, $2, $3, $4,
               jsonb_build_object(
                   'targetTimetableVersionId', $5::text,
                   'effectiveFrom', $6::text,
                   'itemCount', $7::integer,
                   'requestHash', $8::text
               )
           )"#,
    )
    .bind(change_set.id)
    .bind(change_set.academic_year_id)
    .bind(change_set.academic_term_id)
    .bind(actor_user_id)
    .bind(target_version_id)
    .bind(change_set.effective_from)
    .bind(item_count)
    .bind(&publication_request_hash)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    get_change_set(pool, id).await
}

async fn build_preview_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    lock_for_publication: bool,
) -> Result<AcademicTermChangeSetPreview, AppError> {
    let lock = if lock_for_publication {
        "FOR UPDATE"
    } else {
        "FOR SHARE"
    };
    let change_query =
        format!("SELECT {CHANGE_SET_COLUMNS} FROM academic_term_change_sets WHERE id = $1 {lock}");
    let change_set = sqlx::query_as::<_, ChangeSetRow>(&change_query)
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()))?;
    if change_set.status != AcademicTermChangeSetStatus::Draft {
        return Err(AppError::Conflict(
            "ตรวจความพร้อมได้เฉพาะชุดการเปลี่ยนแปลงฉบับร่าง".to_string(),
        ));
    }
    let base_version_id = required_version_id(
        change_set.base_timetable_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนต้นทาง",
    )?;
    let target_version_id = required_version_id(
        change_set.target_timetable_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นตารางเรียนเป้าหมาย",
    )?;
    if lock_for_publication {
        let mut version_ids = vec![base_version_id, target_version_id];
        version_ids.sort_unstable();
        version_ids.dedup();
        sqlx::query(
            "SELECT id FROM academic_timetable_versions \
             WHERE id = ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(&version_ids)
        .fetch_all(&mut **transaction)
        .await?;
    }
    let version_lock = if lock_for_publication {
        "FOR UPDATE"
    } else {
        "FOR SHARE"
    };
    let target_query = format!(
        "SELECT id, status, row_version, change_set_id \
         FROM academic_timetable_versions WHERE id = $1 {version_lock}"
    );
    let target = sqlx::query_as::<_, TargetVersionPreviewRow>(&target_query)
        .bind(target_version_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::Conflict("ไม่พบรุ่นตารางเรียนเป้าหมาย".to_string()))?;
    if target.status != "draft" || target.change_set_id != Some(change_set.id) {
        return Err(AppError::Conflict(
            "รุ่นตารางเรียนเป้าหมายไม่ใช่แบบร่างของชุดการเปลี่ยนแปลงนี้".to_string(),
        ));
    }
    let item_query = format!(
        r#"SELECT id, change_set_id, action_kind, learning_offering_id,
                  weekly_period_target, row_version, created_by, created_at, updated_at
           FROM academic_term_change_items
           WHERE change_set_id = $1
           ORDER BY id {lock}"#
    );
    let items = sqlx::query_as::<_, ChangeItemRow>(&item_query)
        .bind(change_set.id)
        .fetch_all(&mut **transaction)
        .await?;
    let target_pristine = target_is_pristine(
        transaction,
        change_set.id,
        base_version_id,
        target_version_id,
    )
    .await?;
    if lock_for_publication {
        lock_publication_resources(transaction, target_version_id, &items).await?;
    }
    let stop_offering_ids = items
        .iter()
        .filter(|item| item.action_kind == AcademicTermChangeActionKind::StopOffering)
        .map(|item| item.learning_offering_id)
        .collect::<Vec<_>>();

    let impact_counts =
        load_stop_impact_counts(transaction, base_version_id, &stop_offering_ids).await?;
    let schedule_rows: Vec<ScheduleCountRow> = sqlx::query_as(
        r#"SELECT target.learning_offering_id, learning_group.id AS learning_group_id,
                  concat_ws(' · ', nullif(offering.code_snapshot, ''), offering.name_snapshot)
                    AS offering_label,
                  concat_ws(' · ', nullif(learning_group.code, ''), learning_group.name)
                    AS learning_group_label,
                  count(entry.id)::bigint AS actual_periods,
                  target.weekly_period_target AS target_periods
           FROM academic_timetable_version_targets target
           JOIN learning_offerings offering ON offering.id = target.learning_offering_id
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = target.learning_offering_id
            AND learning_group.status <> 'closed'
           LEFT JOIN academic_timetable_entries entry
             ON entry.timetable_version_id = target.timetable_version_id
            AND entry.learning_group_id = learning_group.id
            AND entry.is_active
           WHERE target.timetable_version_id = $1
           GROUP BY target.learning_offering_id, learning_group.id,
                    offering.code_snapshot, offering.name_snapshot,
                    learning_group.code, learning_group.name,
                    target.weekly_period_target
           ORDER BY target.learning_offering_id, learning_group.id"#,
    )
    .bind(target.id)
    .fetch_all(&mut **transaction)
    .await?;
    let schedule_counts = schedule_rows
        .into_iter()
        .map(|row| AcademicOfferingScheduleCount {
            learning_offering_id: row.learning_offering_id,
            learning_group_id: row.learning_group_id,
            offering_label: row.offering_label,
            learning_group_label: row.learning_group_label,
            actual_periods: row.actual_periods,
            target_periods: row.target_periods,
        })
        .collect::<Vec<_>>();

    let mut findings = Vec::new();
    if items.is_empty() && target_pristine {
        findings.push(change_finding(
            AcademicChangeFindingCode::ChangeSetNoItems,
            AcademicChangeFindingSeverity::Blocking,
            "ยังไม่มีการเปลี่ยนแปลง",
            "แก้ตารางในรุ่นแบบร่าง หรือเพิ่มรายการเปลี่ยนแปลงอย่างน้อยหนึ่งรายการก่อนเผยแพร่",
            1,
            None,
            None,
            Some(change_set.id),
        ));
    }
    append_term_and_version_findings(
        transaction,
        &change_set,
        base_version_id,
        target_version_id,
        &mut findings,
    )
    .await?;
    append_resource_readiness_findings(
        transaction,
        &change_set,
        target_version_id,
        &items,
        &schedule_counts,
        &mut findings,
    )
    .await?;
    findings.sort_by_key(|finding| {
        (
            finding.severity,
            finding.code,
            finding.learning_offering_id,
            finding.learning_group_id,
            finding.resource_id,
        )
    });

    let item_fingerprint = items
        .iter()
        .map(|item| {
            (
                item.id,
                item.action_kind,
                item.learning_offering_id,
                item.weekly_period_target,
                item.row_version,
            )
        })
        .collect::<Vec<_>>();
    let affected_offering_ids = items
        .iter()
        .map(|item| item.learning_offering_id)
        .collect::<Vec<_>>();
    let resource_fingerprint =
        load_preview_resource_fingerprint(transaction, target_version_id, &affected_offering_ids)
            .await?;
    let preview_hash = stable_hash(&(
        change_set.id,
        change_set.row_version,
        base_version_id,
        target.id,
        target.row_version,
        change_set.effective_from,
        item_fingerprint,
        resource_fingerprint,
        &impact_counts,
        &schedule_counts,
        &findings,
    ))?;

    Ok(AcademicTermChangeSetPreview {
        change_set_id: change_set.id,
        change_set_row_version: change_set.row_version,
        target_timetable_version_id: target.id,
        target_timetable_version_row_version: target.row_version,
        effective_from: change_set.effective_from,
        impact_counts,
        schedule_counts,
        findings,
        preview_hash,
    })
}

async fn load_stop_impact_counts(
    transaction: &mut Transaction<'_, Postgres>,
    base_version_id: Uuid,
    stop_offering_ids: &[Uuid],
) -> Result<AcademicChangeImpactCounts, AppError> {
    if stop_offering_ids.is_empty() {
        return Ok(AcademicChangeImpactCounts::default());
    }
    let row = sqlx::query(
        r#"SELECT
             (SELECT count(*) FROM learning_groups
                WHERE learning_offering_id = ANY($1)) AS groups,
             (SELECT count(*) FROM learning_group_homerooms coverage
                JOIN learning_groups learning_group ON learning_group.id = coverage.learning_group_id
                WHERE learning_group.learning_offering_id = ANY($1)) AS homerooms,
             (SELECT count(*) FROM learning_group_students membership
                JOIN learning_groups learning_group ON learning_group.id = membership.learning_group_id
                WHERE learning_group.learning_offering_id = ANY($1)) AS membership_intervals,
             (SELECT count(*) FROM learning_group_teachers teacher
                JOIN learning_groups learning_group ON learning_group.id = teacher.learning_group_id
                WHERE learning_group.learning_offering_id = ANY($1)) AS teacher_assignments,
             (SELECT count(*) FROM academic_timetable_entries entry
                WHERE entry.timetable_version_id = $2
                  AND entry.learning_offering_id = ANY($1) AND entry.is_active)
                AS target_timetable_entries,
             (SELECT count(*) FROM course_assessment_plans plan
                WHERE plan.learning_offering_id = ANY($1)) AS course_assessment_plans,
             (SELECT count(*) FROM course_assessment_categories category
                JOIN course_assessment_plans plan ON plan.id = category.plan_id
                WHERE plan.learning_offering_id = ANY($1)) AS course_assessment_categories,
             (SELECT count(*) FROM course_assessment_items item
                JOIN course_assessment_categories category ON category.id = item.category_id
                JOIN course_assessment_plans plan ON plan.id = category.plan_id
                WHERE plan.learning_offering_id = ANY($1)) AS course_assessment_items,
             (SELECT count(*) FROM learning_results result
                WHERE result.learning_offering_id = ANY($1)) AS learning_results,
             (SELECT count(*) FROM academic_exam_schedule_items item
                WHERE item.learning_offering_id = ANY($1)) AS exam_schedule_items,
             (SELECT count(*) FROM supervision_observations observation
                JOIN learning_groups learning_group ON learning_group.id = observation.learning_group_id
                WHERE learning_group.learning_offering_id = ANY($1)) AS supervision_observations"#,
    )
    .bind(stop_offering_ids)
    .bind(base_version_id)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(AcademicChangeImpactCounts {
        groups: row.get("groups"),
        homerooms: row.get("homerooms"),
        membership_intervals: row.get("membership_intervals"),
        teacher_assignments: row.get("teacher_assignments"),
        target_timetable_entries: row.get("target_timetable_entries"),
        course_assessment_plans: row.get("course_assessment_plans"),
        course_assessment_categories: row.get("course_assessment_categories"),
        course_assessment_items: row.get("course_assessment_items"),
        learning_results: row.get("learning_results"),
        exam_schedule_items: row.get("exam_schedule_items"),
        supervision_observations: row.get("supervision_observations"),
    })
}

async fn lock_publication_resources(
    transaction: &mut Transaction<'_, Postgres>,
    target_version_id: Uuid,
    items: &[ChangeItemRow],
) -> Result<(), AppError> {
    let mut affected_offering_ids = items
        .iter()
        .map(|item| item.learning_offering_id)
        .collect::<Vec<_>>();
    let target_offering_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT learning_offering_id
           FROM academic_timetable_version_targets
           WHERE timetable_version_id = $1
           ORDER BY learning_offering_id
           FOR UPDATE"#,
    )
    .bind(target_version_id)
    .fetch_all(&mut **transaction)
    .await?;
    affected_offering_ids.extend(target_offering_ids);
    affected_offering_ids.sort_unstable();
    affected_offering_ids.dedup();
    if affected_offering_ids.is_empty() {
        return Ok(());
    }

    sqlx::query("SELECT id FROM learning_offerings WHERE id = ANY($1) ORDER BY id FOR UPDATE")
        .bind(&affected_offering_ids)
        .fetch_all(&mut **transaction)
        .await?;
    let group_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM learning_groups
           WHERE learning_offering_id = ANY($1)
           ORDER BY id FOR UPDATE"#,
    )
    .bind(&affected_offering_ids)
    .fetch_all(&mut **transaction)
    .await?;
    if !group_ids.is_empty() {
        sqlx::query(
            "SELECT id FROM learning_group_students \
             WHERE learning_group_id = ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(&group_ids)
        .fetch_all(&mut **transaction)
        .await?;
        sqlx::query(
            "SELECT id FROM learning_group_teachers \
             WHERE learning_group_id = ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(&group_ids)
        .fetch_all(&mut **transaction)
        .await?;
    }
    let entry_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM academic_timetable_entries
           WHERE timetable_version_id = $1
           ORDER BY id FOR UPDATE"#,
    )
    .bind(target_version_id)
    .fetch_all(&mut **transaction)
    .await?;
    if !entry_ids.is_empty() {
        sqlx::query(
            "SELECT id FROM timetable_entry_instructors \
             WHERE entry_id = ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(&entry_ids)
        .fetch_all(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn load_preview_resource_fingerprint(
    transaction: &mut Transaction<'_, Postgres>,
    target_version_id: Uuid,
    affected_offering_ids: &[Uuid],
) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar(
        r#"WITH target_offerings AS (
               SELECT learning_offering_id
               FROM academic_timetable_version_targets
               WHERE timetable_version_id = $1
               UNION
               SELECT unnest($2::uuid[])
           ), target_groups AS (
               SELECT learning_group.id
               FROM learning_groups learning_group
               JOIN target_offerings target
                 ON target.learning_offering_id = learning_group.learning_offering_id
           ), target_entries AS (
               SELECT id FROM academic_timetable_entries WHERE timetable_version_id = $1
           )
           SELECT state FROM (
               SELECT concat_ws('|', 'offering', offering.id::text,
                                offering.row_version::text, offering.status,
                                offering.starts_on::text, coalesce(offering.ends_on::text, '')) AS state
               FROM learning_offerings offering
               JOIN target_offerings target ON target.learning_offering_id = offering.id
               UNION ALL
               SELECT concat_ws('|', 'group', learning_group.id::text,
                                learning_group.row_version::text, learning_group.status,
                                learning_group.roster_status,
                                coalesce(learning_group.roster_source_hash::text, ''))
               FROM learning_groups learning_group
               JOIN target_groups target ON target.id = learning_group.id
               UNION ALL
               SELECT concat_ws('|', 'teacher', assignment.id::text,
                                assignment.learning_group_id::text,
                                assignment.teacher_id::text, assignment.role)
               FROM learning_group_teachers assignment
               JOIN target_groups target ON target.id = assignment.learning_group_id
               UNION ALL
               SELECT concat_ws('|', 'membership', membership.id::text,
                                membership.learning_group_id::text,
                                membership.row_version::text, membership.membership_status,
                                membership.joined_at::text, coalesce(membership.left_at::text, ''),
                                coalesce(membership.published_at::text, ''))
               FROM learning_group_students membership
               JOIN target_groups target ON target.id = membership.learning_group_id
               UNION ALL
               SELECT concat_ws('|', 'target', target.learning_offering_id::text,
                                target.weekly_period_target::text)
               FROM academic_timetable_version_targets target
               WHERE target.timetable_version_id = $1
               UNION ALL
               SELECT concat_ws('|', 'entry', entry.id::text, entry.row_version::text,
                                entry.day_of_week, entry.bell_schedule_period_id::text,
                                coalesce(entry.learning_group_id::text, ''),
                                coalesce(entry.homeroom_id::text, ''),
                                coalesce(entry.room_id::text, ''), entry.is_active::text)
               FROM academic_timetable_entries entry
               JOIN target_entries target ON target.id = entry.id
               UNION ALL
               SELECT concat_ws('|', 'instructor', instructor.id::text,
                                instructor.entry_id::text, instructor.instructor_id::text,
                                instructor.role)
               FROM timetable_entry_instructors instructor
               JOIN target_entries target ON target.id = instructor.entry_id
           ) fingerprint
           ORDER BY state"#,
    )
    .bind(target_version_id)
    .bind(affected_offering_ids)
    .fetch_all(&mut **transaction)
    .await?)
}

async fn append_term_and_version_findings(
    transaction: &mut Transaction<'_, Postgres>,
    change_set: &ChangeSetRow,
    base_version_id: Uuid,
    target_version_id: Uuid,
    findings: &mut Vec<AcademicChangeFinding>,
) -> Result<(), AppError> {
    let (term_status, term_start, academic_year_end): (String, NaiveDate, NaiveDate) =
        sqlx::query_as(
            r#"SELECT term.status, term.start_date, year.end_date
               FROM academic_terms term
               JOIN academic_years year ON year.id = term.academic_year_id
               WHERE term.id = $1"#,
        )
        .bind(change_set.academic_term_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::Conflict("ไม่พบภาคเรียนของชุดการเปลี่ยนแปลง".to_string()))?;
    if matches!(term_status.as_str(), "closing" | "closed" | "cancelled") {
        findings.push(change_finding(
            AcademicChangeFindingCode::TermNotWritable,
            AcademicChangeFindingSeverity::Blocking,
            "ภาคเรียนปิดรับการแก้ไข",
            "เปิดภาคเรียนสำหรับงานจัดการเรียนก่อนเผยแพร่ชุดนี้",
            1,
            None,
            None,
            Some(change_set.academic_term_id),
        ));
    }
    if change_set.effective_from < term_start
        || change_set.effective_from > academic_year_end
        || (term_status == "active" && change_set.effective_from < Utc::now().date_naive())
    {
        findings.push(change_finding(
            AcademicChangeFindingCode::EffectiveDateInvalid,
            AcademicChangeFindingSeverity::Blocking,
            "วันที่เริ่มใช้ไม่อยู่ในช่วงปีการศึกษา",
            "แก้วันที่เริ่มใช้ให้อยู่ตั้งแต่วันเปิดภาคเรียนถึงวันสิ้นสุดปีการศึกษา",
            1,
            None,
            None,
            Some(change_set.id),
        ));
    }
    let base_is_published: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM academic_timetable_versions
               WHERE id = $1 AND academic_term_id = $2 AND status = 'published'
           )"#,
    )
    .bind(base_version_id)
    .bind(change_set.academic_term_id)
    .fetch_one(&mut **transaction)
    .await?;
    if !base_is_published {
        findings.push(change_finding(
            AcademicChangeFindingCode::BaseTimetableVersionStale,
            AcademicChangeFindingSeverity::Blocking,
            "รุ่นตารางต้นทางไม่พร้อมใช้งาน",
            "สร้างชุดการเปลี่ยนแปลงใหม่จากรุ่นตารางที่เผยแพร่ล่าสุด",
            1,
            None,
            None,
            Some(base_version_id),
        ));
    }
    let target_matches: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM academic_timetable_versions
               WHERE id = $1 AND academic_term_id = $2 AND status = 'draft'
                 AND change_set_id = $3 AND effective_from = $4
           )"#,
    )
    .bind(target_version_id)
    .bind(change_set.academic_term_id)
    .bind(change_set.id)
    .bind(change_set.effective_from)
    .fetch_one(&mut **transaction)
    .await?;
    if !target_matches {
        findings.push(change_finding(
            AcademicChangeFindingCode::TargetTimetableVersionStale,
            AcademicChangeFindingSeverity::Blocking,
            "รุ่นตารางแบบร่างไม่ตรงกับชุดการเปลี่ยนแปลง",
            "โหลดชุดการเปลี่ยนแปลงใหม่ก่อนแก้ตารางหรือเผยแพร่",
            1,
            None,
            None,
            Some(target_version_id),
        ));
    }
    Ok(())
}

async fn append_resource_readiness_findings(
    transaction: &mut Transaction<'_, Postgres>,
    change_set: &ChangeSetRow,
    target_version_id: Uuid,
    items: &[ChangeItemRow],
    schedule_counts: &[AcademicOfferingScheduleCount],
    findings: &mut Vec<AcademicChangeFinding>,
) -> Result<(), AppError> {
    let missing_targets: Vec<(Uuid, Uuid)> = sqlx::query_as(
        r#"SELECT item.id, item.learning_offering_id
           FROM academic_term_change_items item
           LEFT JOIN academic_timetable_version_targets target
             ON target.timetable_version_id = $2
            AND target.learning_offering_id = item.learning_offering_id
           WHERE item.change_set_id = $1
             AND item.action_kind IN ('add_offering', 'adjust_weekly_period_target')
             AND target.learning_offering_id IS NULL
           ORDER BY item.id"#,
    )
    .bind(change_set.id)
    .bind(target_version_id)
    .fetch_all(&mut **transaction)
    .await?;
    for (item_id, offering_id) in missing_targets {
        findings.push(change_finding(
            AcademicChangeFindingCode::MissingWeeklyPeriodTarget,
            AcademicChangeFindingSeverity::Blocking,
            "ยังไม่ได้กำหนดจำนวนคาบเป้าหมาย",
            "กำหนดจำนวนคาบต่อสัปดาห์ของรายการนี้ในรุ่นตารางแบบร่าง",
            1,
            Some(offering_id),
            None,
            Some(item_id),
        ));
    }

    let stopped_schedule_rows: Vec<(Uuid, i64)> = sqlx::query_as(
        r#"SELECT item.learning_offering_id,
                  (CASE WHEN target.learning_offering_id IS NULL THEN 0 ELSE 1 END
                   + count(entry.id))::bigint AS remaining_count
           FROM academic_term_change_items item
           LEFT JOIN academic_timetable_version_targets target
             ON target.timetable_version_id = $2
            AND target.learning_offering_id = item.learning_offering_id
           LEFT JOIN academic_timetable_entries entry
             ON entry.timetable_version_id = $2
            AND entry.learning_offering_id = item.learning_offering_id
            AND entry.is_active
           WHERE item.change_set_id = $1 AND item.action_kind = 'stop_offering'
           GROUP BY item.learning_offering_id, target.learning_offering_id
           HAVING CASE WHEN target.learning_offering_id IS NULL THEN 0 ELSE 1 END
                  + count(entry.id) > 0
           ORDER BY item.learning_offering_id"#,
    )
    .bind(change_set.id)
    .bind(target_version_id)
    .fetch_all(&mut **transaction)
    .await?;
    for (offering_id, affected_count) in stopped_schedule_rows {
        findings.push(change_finding(
            AcademicChangeFindingCode::StoppedOfferingStillScheduled,
            AcademicChangeFindingSeverity::Blocking,
            "รายการที่จะหยุดยังอยู่ในตารางรุ่นใหม่",
            "นำเป้าหมายและคาบของรายการนี้ออกจากรุ่นตารางแบบร่าง",
            affected_count,
            Some(offering_id),
            None,
            Some(target_version_id),
        ));
    }
    let invalid_stop_offerings: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT offering.id
           FROM academic_term_change_items item
           JOIN learning_offerings offering ON offering.id = item.learning_offering_id
           WHERE item.change_set_id = $1 AND item.action_kind = 'stop_offering'
             AND (offering.status <> 'published'
                  OR offering.starts_on >= $2
                  OR (offering.ends_on IS NOT NULL AND offering.ends_on < $2))
           ORDER BY offering.id"#,
    )
    .bind(change_set.id)
    .bind(change_set.effective_from)
    .fetch_all(&mut **transaction)
    .await?;
    for offering_id in invalid_stop_offerings {
        findings.push(change_finding(
            AcademicChangeFindingCode::OfferingUnavailable,
            AcademicChangeFindingSeverity::Blocking,
            "รายการเปิดสอนไม่สามารถสิ้นสุดก่อนวันที่เริ่มใช้ได้",
            "เลือกวันที่เริ่มใช้หลังวันที่รายการเริ่มสอน หรือยกเลิกรายการฉบับร่างแทน",
            1,
            Some(offering_id),
            None,
            Some(offering_id),
        ));
    }

    let group_rows = sqlx::query(
        r#"SELECT learning_group.id, learning_group.learning_offering_id,
                  learning_group.status, learning_group.roster_status,
                  learning_group.roster_source_hash IS NOT NULL AS roster_prepared,
                  learning_group.status <> 'draft' AS teachers_locked,
                  EXISTS (
                      SELECT 1
                      FROM learning_group_teachers teacher_assignment
                      JOIN users teacher ON teacher.id = teacher_assignment.teacher_id
                      WHERE teacher_assignment.learning_group_id = learning_group.id
                        AND teacher_assignment.role = 'primary'
                        AND teacher.status = 'active'
                  ) AS has_active_primary,
                  EXISTS (
                      SELECT 1 FROM academic_term_change_items item
                      WHERE item.change_set_id = $2
                        AND item.action_kind = 'add_offering'
                        AND item.learning_offering_id = learning_group.learning_offering_id
                  ) AS is_added
           FROM academic_timetable_version_targets target
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = target.learning_offering_id
            AND learning_group.status <> 'closed'
           WHERE target.timetable_version_id = $1
           ORDER BY learning_group.id"#,
    )
    .bind(target_version_id)
    .bind(change_set.id)
    .fetch_all(&mut **transaction)
    .await?;
    for row in group_rows {
        let group_id: Uuid = row.get("id");
        let offering_id: Uuid = row.get("learning_offering_id");
        let status: String = row.get("status");
        let roster_status: String = row.get("roster_status");
        let roster_prepared: bool = row.get("roster_prepared");
        let teachers_locked: bool = row.get("teachers_locked");
        let has_active_primary: bool = row.get("has_active_primary");
        let is_added: bool = row.get("is_added");
        if status != "published" && !(is_added && status == "draft") {
            findings.push(change_finding(
                AcademicChangeFindingCode::DraftGroup,
                AcademicChangeFindingSeverity::Blocking,
                "กลุ่มเรียนยังไม่พร้อมเผยแพร่",
                "ตรวจข้อมูลกลุ่มเรียนให้ครบก่อนเผยแพร่รุ่นตาราง",
                1,
                Some(offering_id),
                Some(group_id),
                Some(group_id),
            ));
        }
        if !has_active_primary || (!is_added && !teachers_locked) {
            findings.push(change_finding(
                AcademicChangeFindingCode::MissingPrimaryTeacher,
                AcademicChangeFindingSeverity::Blocking,
                "กลุ่มเรียนยังไม่มีครูหลักที่พร้อมใช้งาน",
                "กำหนดครูหลักให้กลุ่มเรียนก่อนเผยแพร่ ครูจะถูกล็อกเมื่อเผยแพร่",
                1,
                Some(offering_id),
                Some(group_id),
                Some(group_id),
            ));
        }
        let roster_ready = if is_added {
            roster_status == "published" || (roster_status == "draft" && roster_prepared)
        } else {
            roster_status == "published" || roster_status == "closed"
        };
        if !roster_ready {
            findings.push(change_finding(
                AcademicChangeFindingCode::UnpublishedRoster,
                AcademicChangeFindingSeverity::Blocking,
                "รายชื่อนักเรียนยังไม่พร้อมเผยแพร่",
                "จัดรายชื่อนักเรียนฉบับร่างให้ครบก่อนเผยแพร่ชุดการเปลี่ยนแปลง",
                1,
                Some(offering_id),
                Some(group_id),
                Some(group_id),
            ));
        }
    }

    let offering_rows = sqlx::query(
        r#"SELECT offering.id, offering.status, offering.starts_on, offering.ends_on,
                  EXISTS (
                      SELECT 1 FROM academic_term_change_items item
                      WHERE item.change_set_id = $2
                        AND item.action_kind = 'add_offering'
                        AND item.learning_offering_id = offering.id
                  ) AS is_added
           FROM academic_timetable_version_targets target
           JOIN learning_offerings offering ON offering.id = target.learning_offering_id
           WHERE target.timetable_version_id = $1
           ORDER BY offering.id"#,
    )
    .bind(target_version_id)
    .bind(change_set.id)
    .fetch_all(&mut **transaction)
    .await?;
    for row in offering_rows {
        let offering_id: Uuid = row.get("id");
        let status: String = row.get("status");
        let starts_on: NaiveDate = row.get("starts_on");
        let ends_on: Option<NaiveDate> = row.get("ends_on");
        let is_added: bool = row.get("is_added");
        let status_ready = status == "published" || (is_added && status == "draft");
        if !status_ready
            || change_set.effective_from < starts_on
            || ends_on.is_some_and(|end| change_set.effective_from > end)
        {
            findings.push(change_finding(
                AcademicChangeFindingCode::OfferingUnavailable,
                AcademicChangeFindingSeverity::Blocking,
                "รายการเปิดสอนไม่พร้อมใช้ในวันที่เริ่มรุ่นตาราง",
                "ตรวจสถานะและช่วงวันที่เปิดสอนของรายการนี้",
                1,
                Some(offering_id),
                None,
                Some(offering_id),
            ));
        }
    }

    for count in schedule_counts {
        if count.actual_periods < i64::from(count.target_periods) {
            findings.push(change_finding(
                AcademicChangeFindingCode::WeeklyPeriodDeficit,
                AcademicChangeFindingSeverity::Blocking,
                "จำนวนคาบยังไม่ครบตามเป้าหมาย",
                "เพิ่มคาบให้กลุ่มเรียนนี้จนครบเป้าหมายของรุ่นตาราง",
                i64::from(count.target_periods) - count.actual_periods,
                Some(count.learning_offering_id),
                Some(count.learning_group_id),
                Some(count.learning_group_id),
            ));
        } else if count.actual_periods > i64::from(count.target_periods) {
            findings.push(change_finding(
                AcademicChangeFindingCode::WeeklyPeriodExcess,
                AcademicChangeFindingSeverity::Warning,
                "จำนวนคาบมากกว่าเป้าหมาย",
                "ตรวจยืนยันว่าต้องการใช้จำนวนคาบเกินเป้าหมายในรุ่นนี้",
                count.actual_periods - i64::from(count.target_periods),
                Some(count.learning_offering_id),
                Some(count.learning_group_id),
                Some(count.learning_group_id),
            ));
        }
    }

    append_conflict_findings(transaction, target_version_id, findings).await?;
    let _ = items;
    Ok(())
}

async fn append_conflict_findings(
    transaction: &mut Transaction<'_, Postgres>,
    target_version_id: Uuid,
    findings: &mut Vec<AcademicChangeFinding>,
) -> Result<(), AppError> {
    let group_conflicts: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM (
               SELECT learning_group_id, day_of_week, bell_schedule_period_id
               FROM academic_timetable_entries
               WHERE timetable_version_id = $1 AND is_active
                 AND learning_group_id IS NOT NULL
               GROUP BY learning_group_id, day_of_week, bell_schedule_period_id
               HAVING count(*) > 1
           ) conflicts"#,
    )
    .bind(target_version_id)
    .fetch_one(&mut **transaction)
    .await?;
    push_conflict_finding(
        findings,
        AcademicChangeFindingCode::LearningGroupConflict,
        "กลุ่มเรียนมีคาบซ้อนกัน",
        "ย้ายคาบที่ซ้อนกันของกลุ่มเรียนออกจากช่วงเวลาเดียวกัน",
        group_conflicts,
        target_version_id,
    );

    let homeroom_conflicts: i64 = sqlx::query_scalar(
        r#"WITH entry_homerooms AS (
               SELECT entry.id, entry.day_of_week, entry.bell_schedule_period_id,
                      coverage.homeroom_id
               FROM academic_timetable_entries entry
               JOIN learning_group_homerooms coverage
                 ON coverage.learning_group_id = entry.learning_group_id
               WHERE entry.timetable_version_id = $1 AND entry.is_active
               UNION
               SELECT entry.id, entry.day_of_week, entry.bell_schedule_period_id,
                      entry.homeroom_id
               FROM academic_timetable_entries entry
               WHERE entry.timetable_version_id = $1 AND entry.is_active
                 AND entry.homeroom_id IS NOT NULL
           )
           SELECT count(*) FROM (
               SELECT homeroom_id, day_of_week, bell_schedule_period_id
               FROM entry_homerooms
               GROUP BY homeroom_id, day_of_week, bell_schedule_period_id
               HAVING count(DISTINCT id) > 1
           ) conflicts"#,
    )
    .bind(target_version_id)
    .fetch_one(&mut **transaction)
    .await?;
    push_conflict_finding(
        findings,
        AcademicChangeFindingCode::HomeroomConflict,
        "ห้องประจำชั้นมีคาบซ้อนกัน",
        "ย้ายคาบของห้องประจำชั้นที่อยู่ในช่วงเวลาเดียวกัน",
        homeroom_conflicts,
        target_version_id,
    );

    let teacher_conflicts: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM (
               SELECT instructor.instructor_id, entry.day_of_week,
                      entry.bell_schedule_period_id
               FROM academic_timetable_entries entry
               JOIN timetable_entry_instructors instructor ON instructor.entry_id = entry.id
               WHERE entry.timetable_version_id = $1 AND entry.is_active
               GROUP BY instructor.instructor_id, entry.day_of_week,
                        entry.bell_schedule_period_id
               HAVING count(DISTINCT entry.id) > 1
           ) conflicts"#,
    )
    .bind(target_version_id)
    .fetch_one(&mut **transaction)
    .await?;
    push_conflict_finding(
        findings,
        AcademicChangeFindingCode::TeacherConflict,
        "ครูผู้สอนมีคาบซ้อนกัน",
        "ย้ายคาบของครูที่อยู่ในช่วงเวลาเดียวกัน",
        teacher_conflicts,
        target_version_id,
    );

    let room_conflicts: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM (
               SELECT room_id, day_of_week, bell_schedule_period_id
               FROM academic_timetable_entries
               WHERE timetable_version_id = $1 AND is_active AND room_id IS NOT NULL
               GROUP BY room_id, day_of_week, bell_schedule_period_id
               HAVING count(*) > 1
           ) conflicts"#,
    )
    .bind(target_version_id)
    .fetch_one(&mut **transaction)
    .await?;
    push_conflict_finding(
        findings,
        AcademicChangeFindingCode::RoomConflict,
        "ห้องเรียนมีคาบซ้อนกัน",
        "ย้ายคาบที่ใช้ห้องเดียวกันในช่วงเวลาเดียวกัน",
        room_conflicts,
        target_version_id,
    );
    Ok(())
}

fn push_conflict_finding(
    findings: &mut Vec<AcademicChangeFinding>,
    code: AcademicChangeFindingCode,
    title: &str,
    guidance: &str,
    affected_count: i64,
    target_version_id: Uuid,
) {
    if affected_count > 0 {
        findings.push(change_finding(
            code,
            AcademicChangeFindingSeverity::Blocking,
            title,
            guidance,
            affected_count,
            None,
            None,
            Some(target_version_id),
        ));
    }
}

fn change_finding(
    code: AcademicChangeFindingCode,
    severity: AcademicChangeFindingSeverity,
    title: &str,
    guidance: &str,
    affected_count: i64,
    learning_offering_id: Option<Uuid>,
    learning_group_id: Option<Uuid>,
    resource_id: Option<Uuid>,
) -> AcademicChangeFinding {
    AcademicChangeFinding {
        code,
        severity,
        title: title.to_string(),
        guidance: guidance.to_string(),
        affected_count,
        route: None,
        resource_id,
        learning_group_id,
        learning_offering_id,
    }
}

fn finding_code_text(code: AcademicChangeFindingCode) -> &'static str {
    match code {
        AcademicChangeFindingCode::ChangeSetNoItems => "change_set_no_items",
        AcademicChangeFindingCode::ChangeSetStale => "change_set_stale",
        AcademicChangeFindingCode::TermNotWritable => "term_not_writable",
        AcademicChangeFindingCode::EffectiveDateInvalid => "effective_date_invalid",
        AcademicChangeFindingCode::BaseTimetableVersionStale => "base_timetable_version_stale",
        AcademicChangeFindingCode::TargetTimetableVersionStale => "target_timetable_version_stale",
        AcademicChangeFindingCode::ChangeItemStale => "change_item_stale",
        AcademicChangeFindingCode::ResourceStale => "resource_stale",
        AcademicChangeFindingCode::DraftGroup => "draft_group",
        AcademicChangeFindingCode::MissingPrimaryTeacher => "missing_primary_teacher",
        AcademicChangeFindingCode::UnpublishedRoster => "unpublished_roster",
        AcademicChangeFindingCode::OfferingUnavailable => "offering_unavailable",
        AcademicChangeFindingCode::MissingWeeklyPeriodTarget => "missing_weekly_period_target",
        AcademicChangeFindingCode::WeeklyPeriodDeficit => "weekly_period_deficit",
        AcademicChangeFindingCode::WeeklyPeriodExcess => "weekly_period_excess",
        AcademicChangeFindingCode::HomeroomConflict => "homeroom_conflict",
        AcademicChangeFindingCode::LearningGroupConflict => "learning_group_conflict",
        AcademicChangeFindingCode::TeacherConflict => "teacher_conflict",
        AcademicChangeFindingCode::RoomConflict => "room_conflict",
        AcademicChangeFindingCode::StoppedOfferingStillScheduled => {
            "stopped_offering_still_scheduled"
        }
    }
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
           SET status = 'cancelled', row_version = offering.row_version + 1,
               updated_at = now()
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
