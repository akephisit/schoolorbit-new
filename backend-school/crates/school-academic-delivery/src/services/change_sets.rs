use std::collections::{BTreeSet, HashMap};

use chrono::{NaiveDate, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::models::{
    AcademicChangeFinding, AcademicChangeFindingCode, AcademicChangeFindingSeverity,
    AcademicChangeImpactCounts, AcademicTermChangeActionKind, AcademicTermChangeItem,
    AcademicTermChangeSet, AcademicTermChangeSetPreview, AcademicTermChangeSetStatus,
    AcademicTermChangeSetSummary, CancelAcademicTermChangeSetRequest,
    CreateAcademicTermChangeSetRequest, DeleteAcademicTermChangeItemRequest,
    LearningOfferingStatus, LearningTeacherRole, PublishAcademicTermChangeSetRequest,
    UpdateAcademicTermChangeSetRequest, UpsertAcademicTermChangeItemRequest,
};
use school_academic_core::{
    models::{AcademicTermStatus, AcademicYearStatus},
    services::lifecycle_guard::AcademicWriteState,
};
use school_errors::AppError;

use super::{
    append_audit, offerings, require_writable_term, stable_hash, validate_row_version, versions,
    TermContext,
};

#[derive(Debug, FromRow)]
struct ChangeSetRow {
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    effective_from: NaiveDate,
    reason: String,
    status: AcademicTermChangeSetStatus,
    base_delivery_version_id: Option<Uuid>,
    target_delivery_version_id: Option<Uuid>,
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
struct ChangeSetSummaryRow {
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    effective_from: NaiveDate,
    reason: String,
    status: AcademicTermChangeSetStatus,
    target_delivery_version_id: Option<Uuid>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ChangeItemRow {
    id: Uuid,
    change_set_id: Uuid,
    action_kind: AcademicTermChangeActionKind,
    learning_offering_id: Option<Uuid>,
    weekly_period_target: Option<i32>,
    learning_group_id: Option<Uuid>,
    learning_group_teacher_id: Option<Uuid>,
    teacher_id: Option<Uuid>,
    teacher_role: Option<LearningTeacherRole>,
    row_version: i64,
    created_by: Uuid,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TeacherEpisodeAuditSnapshot {
    role: LearningTeacherRole,
    starts_on: NaiveDate,
    ends_on: Option<NaiveDate>,
    row_version: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TeacherEpisodeAuditChange {
    action: AcademicTermChangeActionKind,
    item_id: Uuid,
    episode_id: Uuid,
    learning_group_id: Uuid,
    teacher_id: Uuid,
    before: Option<TeacherEpisodeAuditSnapshot>,
    after: TeacherEpisodeAuditSnapshot,
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
    base_delivery_version_id, target_delivery_version_id, row_version,
    created_by, published_by, published_at, cancelled_by, cancelled_at,
    created_at, updated_at
"#;

pub async fn list_change_set_summaries(
    pool: &PgPool,
    academic_term_id: Uuid,
) -> Result<Vec<AcademicTermChangeSetSummary>, AppError> {
    let rows = sqlx::query_as::<_, ChangeSetSummaryRow>(
        r#"SELECT id, academic_term_id, academic_year_id, effective_from, reason, status,
                  target_delivery_version_id, updated_at
           FROM academic_term_change_sets
           WHERE academic_term_id = $1 AND target_delivery_version_id IS NOT NULL
           ORDER BY effective_from DESC, created_at DESC, id"#,
    )
    .bind(academic_term_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(AcademicTermChangeSetSummary {
                id: row.id,
                academic_term_id: row.academic_term_id,
                academic_year_id: row.academic_year_id,
                effective_from: row.effective_from,
                reason: row.reason,
                status: row.status,
                target_delivery_version_id: required_version_id(
                    row.target_delivery_version_id,
                    "ชุดการเปลี่ยนแปลงไม่มีตารางเป้าหมาย",
                )?,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

pub async fn get_change_set(pool: &PgPool, id: Uuid) -> Result<AcademicTermChangeSet, AppError> {
    let query = format!("SELECT {CHANGE_SET_COLUMNS} FROM academic_term_change_sets WHERE id = $1");
    let row = sqlx::query_as::<_, ChangeSetRow>(sqlx::AssertSqlSafe(query))
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
    validate_row_version(request.target_delivery_version_row_version)?;
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
        request.target_delivery_version_row_version,
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
    if preview.target_delivery_version_row_version != request.target_delivery_version_row_version {
        return Err(AppError::Conflict(
            "รุ่นเปิดสอนแบบร่างถูกแก้ไขหลังการตรวจ กรุณาตรวจความพร้อมใหม่".to_string(),
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

    let change_set = sqlx::query_as::<_, ChangeSetRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {CHANGE_SET_COLUMNS} FROM academic_term_change_sets WHERE id = $1"
    )))
    .bind(id)
    .fetch_one(&mut *transaction)
    .await?;
    let target_version_id = required_version_id(
        change_set.target_delivery_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นเปิดสอนเป้าหมาย",
    )?;
    let item_rows: Vec<ChangeItemRow> = sqlx::query_as(
        r#"SELECT id, change_set_id, action_kind, learning_offering_id,
                  weekly_period_target, learning_group_id, learning_group_teacher_id,
                  teacher_id, teacher_role, row_version, created_by, created_at, updated_at
           FROM academic_term_change_items
           WHERE change_set_id = $1 ORDER BY id"#,
    )
    .bind(id)
    .fetch_all(&mut *transaction)
    .await?;
    let target_graph: sqlx::types::Json<crate::models::versions::DeliverySnapshot> =
        sqlx::query_scalar(
            "SELECT snapshot FROM academic_delivery_versions WHERE id=$1 FOR UPDATE",
        )
        .bind(target_version_id)
        .fetch_one(&mut *transaction)
        .await?;
    let add_offering_ids = target_graph
        .offerings
        .iter()
        .map(|offering| offering.id)
        .collect::<Vec<_>>();
    let stop_offering_ids = item_rows
        .iter()
        .filter(|item| item.action_kind == AcademicTermChangeActionKind::StopOffering)
        .filter_map(|item| item.learning_offering_id)
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
                r#"UPDATE learning_groups SET status='published',row_version=row_version+1,updated_at=now()
                   WHERE id=ANY($1) AND status='draft'"#,
            )
            .bind(&added_group_ids)
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query(
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
        let all_published: bool = sqlx::query_scalar(
            "SELECT count(*)=$2 FROM learning_offerings WHERE id=ANY($1) AND status='published'",
        )
        .bind(&add_offering_ids)
        .bind(add_offering_ids.len() as i64)
        .fetch_one(&mut *transaction)
        .await?;
        if !all_published {
            return Err(AppError::Conflict(
                "รายการเปิดสอนเปลี่ยนสถานะ กรุณาตรวจความพร้อมใหม่".into(),
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

    let teacher_episode_changes =
        apply_teacher_episode_changes(&mut transaction, &change_set, &item_rows, actor_user_id)
            .await?;
    let teacher_group_ids = teacher_episode_changes
        .iter()
        .map(|change| change.learning_group_id)
        .collect::<Vec<_>>();
    super::invalidate_group_academic_confirmations(&mut transaction, &teacher_group_ids).await?;

    let published_version = sqlx::query(
        r#"UPDATE academic_delivery_versions
           SET status = 'published', published_by = $1, published_at = now(),
               publication_idempotency_key=$4,publication_request_hash=$5,
               row_version = row_version + 1, updated_at = now()
           WHERE id = $2 AND status = 'draft' AND row_version = $3"#,
    )
    .bind(actor_user_id)
    .bind(target_version_id)
    .bind(request.target_delivery_version_row_version)
    .bind(request.idempotency_key)
    .bind(&publication_request_hash)
    .execute(&mut *transaction)
    .await?;
    if published_version.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "รุ่นเปิดสอนแบบร่างเปลี่ยนไป กรุณาตรวจความพร้อมใหม่".to_string(),
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
                   'targetDeliveryVersionId', $5::text,
                   'effectiveFrom', $6::text,
                   'itemCount', $7::integer,
                   'requestHash', $8::text,
                   'teacherEpisodeChanges', $9::jsonb
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
    .bind(sqlx::types::Json(&teacher_episode_changes))
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    get_change_set(pool, id).await
}

async fn apply_teacher_episode_changes(
    transaction: &mut Transaction<'_, Postgres>,
    change_set: &ChangeSetRow,
    items: &[ChangeItemRow],
    actor_user_id: Uuid,
) -> Result<Vec<TeacherEpisodeAuditChange>, AppError> {
    let mut teacher_items = items
        .iter()
        .filter(|item| {
            matches!(
                item.action_kind,
                AcademicTermChangeActionKind::AddGroupTeacher
                    | AcademicTermChangeActionKind::AdjustGroupTeacherRole
                    | AcademicTermChangeActionKind::StopGroupTeacher
            )
        })
        .collect::<Vec<_>>();
    if teacher_items.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query("SELECT set_config('schoolorbit.academic_change_set_id', $1, true)")
        .bind(change_set.id.to_string())
        .execute(&mut **transaction)
        .await?;
    teacher_items.sort_by_key(|item| {
        (
            match item.action_kind {
                AcademicTermChangeActionKind::StopGroupTeacher => 0,
                AcademicTermChangeActionKind::AdjustGroupTeacherRole => 1,
                AcademicTermChangeActionKind::AddGroupTeacher => 2,
                _ => 3,
            },
            item.learning_group_id,
            item.teacher_id,
            item.id,
        )
    });
    let ends_on = change_set
        .effective_from
        .checked_sub_signed(chrono::Duration::days(1))
        .ok_or_else(|| AppError::ValidationError("วันที่เริ่มใช้ไม่ถูกต้อง".to_string()))?;
    let mut changes = Vec::new();

    for item in teacher_items.iter().filter(|item| {
        matches!(
            item.action_kind,
            AcademicTermChangeActionKind::StopGroupTeacher
                | AcademicTermChangeActionKind::AdjustGroupTeacherRole
        )
    }) {
        let episode_id = required_change_item_field(
            item.learning_group_teacher_id,
            "รายการเปลี่ยนครูไม่มีช่วงการสอนเดิม",
        )?;
        let before: (
            Uuid,
            Uuid,
            LearningTeacherRole,
            NaiveDate,
            Option<NaiveDate>,
            i64,
        ) = sqlx::query_as(
            r#"SELECT learning_group_id, teacher_id, role, starts_on, ends_on, row_version
                   FROM learning_group_teachers
                   WHERE id = $1
                   FOR UPDATE"#,
        )
        .bind(episode_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::Conflict("ช่วงการสอนเดิมไม่พบแล้ว".to_string()))?;
        if Some(before.0) != item.learning_group_id || Some(before.1) != item.teacher_id {
            return Err(AppError::Conflict(
                "บริบทช่วงการสอนเดิมเปลี่ยนแปลงแล้ว".to_string(),
            ));
        }
        let updated = sqlx::query(
            r#"UPDATE learning_group_teachers
               SET ends_on = $1, ended_by_change_set_id = $2,
                   row_version = row_version + 1,
                   updated_by = $3, updated_at = now()
               WHERE id = $4
                 AND starts_on < $5
                 AND (ends_on IS NULL OR ends_on >= $5)
                 AND ended_by_change_set_id IS NULL"#,
        )
        .bind(ends_on)
        .bind(change_set.id)
        .bind(actor_user_id)
        .bind(episode_id)
        .bind(change_set.effective_from)
        .execute(&mut **transaction)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(AppError::Conflict(
                "ช่วงการสอนเดิมเปลี่ยนไป กรุณาตรวจความพร้อมใหม่".to_string(),
            ));
        }
        changes.push(TeacherEpisodeAuditChange {
            action: item.action_kind,
            item_id: item.id,
            episode_id,
            learning_group_id: before.0,
            teacher_id: before.1,
            before: Some(TeacherEpisodeAuditSnapshot {
                role: before.2,
                starts_on: before.3,
                ends_on: before.4,
                row_version: before.5,
            }),
            after: TeacherEpisodeAuditSnapshot {
                role: before.2,
                starts_on: before.3,
                ends_on: Some(ends_on),
                row_version: before.5 + 1,
            },
        });
    }

    for item in teacher_items.iter().filter(|item| {
        matches!(
            item.action_kind,
            AcademicTermChangeActionKind::AddGroupTeacher
                | AcademicTermChangeActionKind::AdjustGroupTeacherRole
        )
    }) {
        let learning_group_id =
            required_change_item_field(item.learning_group_id, "รายการเพิ่มช่วงการสอนไม่มีกลุ่มเรียน")?;
        let teacher_id = required_change_item_field(item.teacher_id, "รายการเพิ่มช่วงการสอนไม่มีครู")?;
        let role = required_change_item_field(item.teacher_role, "รายการเพิ่มช่วงการสอนไม่มีบทบาท")?;
        let episode_id = Uuid::new_v5(&item.id, b"delivery-teacher-episode");
        sqlx::query(
            r#"INSERT INTO learning_group_teachers (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   teacher_id, role, starts_on, started_by_change_set_id,
                   created_by, updated_by
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)"#,
        )
        .bind(episode_id)
        .bind(learning_group_id)
        .bind(change_set.academic_term_id)
        .bind(change_set.academic_year_id)
        .bind(teacher_id)
        .bind(role)
        .bind(change_set.effective_from)
        .bind(change_set.id)
        .bind(actor_user_id)
        .execute(&mut **transaction)
        .await?;
        changes.push(TeacherEpisodeAuditChange {
            action: item.action_kind,
            item_id: item.id,
            episode_id,
            learning_group_id,
            teacher_id,
            before: None,
            after: TeacherEpisodeAuditSnapshot {
                role,
                starts_on: change_set.effective_from,
                ends_on: None,
                row_version: 1,
            },
        });
    }

    Ok(changes)
}

#[derive(Serialize)]
struct OpeningResourceEvidence {
    offerings: Vec<(Uuid, i64, String)>,
    groups: Vec<(Uuid, i64, String)>,
    teachers: Vec<(Uuid, i64, String, NaiveDate, Option<NaiveDate>)>,
    items: Vec<(Uuid, i64)>,
    staff: Vec<(Uuid, String)>,
}

async fn opening_resource_evidence(
    tx: &mut Transaction<'_, Postgres>,
    revision: Uuid,
    snapshot: &crate::models::versions::DeliverySnapshot,
) -> Result<OpeningResourceEvidence, AppError> {
    let mut offering_ids = snapshot
        .offerings
        .iter()
        .map(|offering| offering.id)
        .collect::<BTreeSet<_>>();
    let item_offerings: Vec<Uuid>=sqlx::query_scalar("SELECT learning_offering_id FROM academic_term_change_items WHERE change_set_id=$1 AND learning_offering_id IS NOT NULL")
        .bind(revision).fetch_all(&mut **tx).await?;
    offering_ids.extend(item_offerings);
    let offering_ids = offering_ids.into_iter().collect::<Vec<_>>();
    let offerings=sqlx::query_as("SELECT id,row_version,status FROM learning_offerings WHERE id=ANY($1) ORDER BY id FOR SHARE")
        .bind(&offering_ids).fetch_all(&mut **tx).await?;
    let groups=sqlx::query_as("SELECT id,row_version,status FROM learning_groups WHERE learning_offering_id=ANY($1) ORDER BY id FOR SHARE")
        .bind(&offering_ids).fetch_all(&mut **tx).await?;
    let teachers=sqlx::query_as("SELECT teacher.id,teacher.row_version,teacher.role,teacher.starts_on,teacher.ends_on FROM learning_group_teachers teacher JOIN learning_groups source_group ON source_group.id=teacher.learning_group_id WHERE source_group.learning_offering_id=ANY($1) ORDER BY teacher.id FOR SHARE OF teacher")
        .bind(&offering_ids).fetch_all(&mut **tx).await?;
    let items=sqlx::query_as("SELECT id,row_version FROM academic_term_change_items WHERE change_set_id=$1 ORDER BY id FOR SHARE")
        .bind(revision).fetch_all(&mut **tx).await?;
    let staff_ids = snapshot
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .flat_map(|group| &group.teachers)
        .map(|teacher| teacher.teacher_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let staff=sqlx::query_as("SELECT id,status::text FROM users WHERE id=ANY($1) AND user_type='staff' ORDER BY id FOR SHARE")
        .bind(&staff_ids).fetch_all(&mut **tx).await?;
    Ok(OpeningResourceEvidence {
        offerings,
        groups,
        teachers,
        items,
        staff,
    })
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
    let query =
        format!("SELECT {CHANGE_SET_COLUMNS} FROM academic_term_change_sets WHERE id=$1 {lock}");
    let revision = sqlx::query_as::<_, ChangeSetRow>(sqlx::AssertSqlSafe(query))
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นเปิดสอนแบบร่าง".into()))?;
    if revision.status != AcademicTermChangeSetStatus::Draft {
        return Err(AppError::Conflict(
            "ตรวจความพร้อมได้เฉพาะรุ่นเปิดสอนแบบร่าง".into(),
        ));
    }
    let target_id =
        required_version_id(revision.target_delivery_version_id, "ไม่พบรุ่นเปิดสอนเป้าหมาย")?;
    let query = format!("SELECT snapshot,row_version,effective_from,status,academic_term_id FROM academic_delivery_versions WHERE id=$1 {lock}");
    let (snapshot, target_row_version, effective_from, status, term_id): (
        sqlx::types::Json<crate::models::versions::DeliverySnapshot>,
        i64,
        NaiveDate,
        String,
        Uuid,
    ) = sqlx::query_as(sqlx::AssertSqlSafe(query))
        .bind(target_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::Conflict("รุ่นเปิดสอนแบบร่างไม่พร้อมใช้งาน".into()))?;
    if status != "draft"
        || term_id != revision.academic_term_id
        || effective_from != revision.effective_from
    {
        return Err(AppError::Conflict(
            "ข้อมูลรุ่นเปิดสอนแบบร่างเปลี่ยนไป กรุณาโหลดใหม่".into(),
        ));
    }
    let term = super::load_term_context(transaction, term_id).await?;
    let year_status: AcademicYearStatus =
        sqlx::query_scalar("SELECT status FROM academic_years WHERE id=$1")
            .bind(term.academic_year_id)
            .fetch_one(&mut **transaction)
            .await?;
    let term_status: AcademicTermStatus =
        sqlx::query_scalar("SELECT status FROM academic_terms WHERE id=$1")
            .bind(term_id)
            .fetch_one(&mut **transaction)
            .await?;
    let mut findings = Vec::new();
    if !(AcademicWriteState {
        year_status,
        term_status,
    })
    .is_writable()
    {
        findings.push(change_finding(
            AcademicChangeFindingCode::TermNotWritable,
            AcademicChangeFindingSeverity::Blocking,
            "ภาคเรียนปิดรับการแก้ไข",
            "ดูรุ่นเดิมได้ แต่เผยแพร่รุ่นเปิดสอนใหม่ไม่ได้",
            1,
            None,
            None,
            Some(term_id),
        ));
    }
    if effective_from < term.start_date
        || effective_from > term.academic_year_end_date
        || (term_status == AcademicTermStatus::Active && effective_from < Utc::now().date_naive())
    {
        findings.push(change_finding(
            AcademicChangeFindingCode::EffectiveDateInvalid,
            AcademicChangeFindingSeverity::Blocking,
            "กรุณาตรวจวันที่เริ่มใช้",
            "เลือกวันที่ในช่วงภาคเรียน และไม่ย้อนหลังเมื่อภาคเรียนเริ่มแล้ว",
            1,
            None,
            None,
            Some(id),
        ));
    }
    let latest: Option<(Uuid,NaiveDate)> = sqlx::query_as("SELECT id,effective_from FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1")
        .bind(term_id).fetch_optional(&mut **transaction).await?;
    if latest.is_some_and(|(latest_id, date)| {
        revision.base_delivery_version_id != Some(latest_id) || effective_from <= date
    }) {
        findings.push(change_finding(
            AcademicChangeFindingCode::BaseDeliveryVersionStale,
            AcademicChangeFindingSeverity::Blocking,
            "มีรุ่นเปิดสอนใหม่หรือวันที่เริ่มใช้ซ้ำ",
            "สร้างร่างจากรุ่นเปิดสอนล่าสุด และเลือกวันเริ่มใช้หลังรุ่นนั้น",
            1,
            None,
            None,
            revision.base_delivery_version_id,
        ));
    }
    let fresh: sqlx::types::Json<crate::models::versions::DeliverySnapshot> =
        sqlx::query_scalar("SELECT academic_delivery_revision_snapshot($1)")
            .bind(id)
            .fetch_one(&mut **transaction)
            .await?;
    if stable_hash(&snapshot.0)? != stable_hash(&fresh.0)? {
        findings.push(change_finding(
            AcademicChangeFindingCode::ResourceStale,
            AcademicChangeFindingSeverity::Blocking,
            "ข้อมูลเปิดสอนเปลี่ยนไป",
            "อัปเดตข้อมูลรุ่นเปิดสอนแบบร่างแล้วตรวจความพร้อมอีกครั้ง",
            1,
            None,
            None,
            Some(target_id),
        ));
    }
    let unchanged = if let Some(base_id) = revision.base_delivery_version_id {
        let base: sqlx::types::Json<crate::models::versions::DeliverySnapshot> =
            sqlx::query_scalar("SELECT snapshot FROM academic_delivery_versions WHERE id=$1")
                .bind(base_id)
                .fetch_one(&mut **transaction)
                .await?;
        stable_hash(&base.0)? == stable_hash(&snapshot.0)?
    } else {
        snapshot.0.offerings.is_empty()
    };
    if unchanged {
        findings.push(change_finding(
            AcademicChangeFindingCode::ChangeSetNoItems,
            AcademicChangeFindingSeverity::Blocking,
            "ยังไม่มีข้อมูลเปิดสอนที่เปลี่ยนแปลง",
            "เพิ่มหรือแก้ข้อมูลเปิดสอนก่อนเผยแพร่รุ่นใหม่",
            1,
            None,
            None,
            Some(target_id),
        ));
    }
    for finding in versions::readiness(&snapshot.0) {
        use crate::models::versions::DeliveryReadinessCode;
        let (code, title, guidance) = match finding.code {
            DeliveryReadinessCode::MissingPrimaryTeacher => (
                AcademicChangeFindingCode::MissingPrimaryTeacher,
                "กลุ่มเรียนยังไม่มีครูหลัก",
                "เลือกครูหลักให้กลุ่มเรียนก่อนเผยแพร่",
            ),
            DeliveryReadinessCode::InvalidWeeklyTarget => (
                AcademicChangeFindingCode::MissingWeeklyPeriodTarget,
                "จำนวนคาบไม่ถูกต้อง",
                "กำหนดจำนวนคาบต่อสัปดาห์มากกว่าศูนย์",
            ),
            DeliveryReadinessCode::MissingTargets => (
                AcademicChangeFindingCode::MissingDeliveryTarget,
                "ยังไม่กำหนดกลุ่มเป้าหมาย",
                "เลือกห้องหรือระดับชั้นที่เปิดสอน",
            ),
            DeliveryReadinessCode::MissingGroups => (
                AcademicChangeFindingCode::MissingDeliveryGroup,
                "ยังไม่มีกลุ่มเรียน",
                "สร้างกลุ่มเรียนและเลือกครู",
            ),
            _ => (
                AcademicChangeFindingCode::DeliveryGraphInvalid,
                "ข้อมูลเปิดสอนซ้ำหรือไม่ตรงกัน",
                "ตรวจรายวิชา กลุ่มเรียน และครูที่ระบุ",
            ),
        };
        findings.push(change_finding(
            code,
            AcademicChangeFindingSeverity::Blocking,
            title,
            guidance,
            1,
            Some(finding.learning_offering_id),
            finding.learning_group_id,
            Some(target_id),
        ));
    }
    let teacher_ids = snapshot
        .0
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .flat_map(|group| &group.teachers)
        .map(|teacher| teacher.teacher_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let resources = opening_resource_evidence(transaction, id, &snapshot.0).await?;
    let valid_teachers = resources
        .staff
        .iter()
        .filter(|(_, status)| status == "active")
        .count() as i64;
    if valid_teachers != teacher_ids.len() as i64 {
        findings.push(change_finding(
            AcademicChangeFindingCode::MissingEffectiveTeacher,
            AcademicChangeFindingSeverity::Blocking,
            "ครูบางคนไม่พร้อมใช้งาน",
            "เลือกครูที่ยังปฏิบัติงานในโรงเรียน",
            teacher_ids.len() as i64 - valid_teachers,
            None,
            None,
            Some(target_id),
        ));
    }
    let stop_ids: Vec<Uuid> = sqlx::query_scalar("SELECT learning_offering_id FROM academic_term_change_items WHERE change_set_id=$1 AND action_kind='stop_offering' ORDER BY learning_offering_id")
        .bind(id).fetch_all(&mut **transaction).await?;
    let impacts = load_stop_impact_counts(
        transaction,
        revision.base_delivery_version_id.unwrap_or(Uuid::nil()),
        &stop_ids,
    )
    .await?;
    let preview_hash = stable_hash(&(
        id,
        revision.row_version,
        target_id,
        target_row_version,
        effective_from,
        &snapshot.0,
        &fresh.0,
        (&teacher_ids, &resources),
        valid_teachers,
        &impacts,
        &findings,
    ))?;
    Ok(AcademicTermChangeSetPreview {
        change_set_id: id,
        change_set_row_version: revision.row_version,
        target_delivery_version_id: target_id,
        target_delivery_version_row_version: target_row_version,
        effective_from,
        impact_counts: impacts,
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
             (SELECT count(*) FROM academic_timetable_blocks block
                WHERE block.timetable_version_id IN (SELECT id FROM academic_timetable_versions WHERE delivery_version_id=$2)
                  AND block.learning_offering_id = ANY($1) AND block.is_active)
                AS target_timetable_entries,
             (SELECT count(*) FROM course_assessment_plans plan
                WHERE plan.learning_offering_id = ANY($1)) AS course_assessment_plans,
             (SELECT count(*) FROM course_assessment_phases phase
                JOIN course_assessment_plans plan ON plan.id = phase.plan_id
                WHERE plan.learning_offering_id = ANY($1)) AS course_assessment_phases,
             (SELECT count(*) FROM learning_group_score_items item
                WHERE item.learning_offering_id = ANY($1)) AS learning_group_score_items,
             (SELECT count(*) FROM learning_group_student_scores score
                WHERE score.learning_offering_id = ANY($1)) AS student_scores,
             (SELECT count(*) FROM learning_group_result_overrides selection
                JOIN learning_groups learning_group
                  ON learning_group.id = selection.learning_group_id
                WHERE learning_group.learning_offering_id = ANY($1)) AS result_selections,
             ((SELECT count(*) FROM learning_group_phase_confirmations confirmation
                 JOIN learning_groups learning_group
                   ON learning_group.id = confirmation.learning_group_id
                 WHERE learning_group.learning_offering_id = ANY($1))
              + (SELECT count(*) FROM learning_group_result_confirmations confirmation
                 JOIN learning_groups learning_group
                   ON learning_group.id = confirmation.learning_group_id
                 WHERE learning_group.learning_offering_id = ANY($1))
              + (SELECT count(*) FROM academic_activity_result_confirmations confirmation
                 JOIN learning_groups learning_group
                   ON learning_group.id = confirmation.learning_group_id
                 WHERE learning_group.learning_offering_id = ANY($1))
              + (SELECT count(*) FROM learning_group_evaluation_confirmations confirmation
                 JOIN learning_groups learning_group
                   ON learning_group.id = confirmation.learning_group_id
                 WHERE learning_group.learning_offering_id = ANY($1))) AS result_confirmations,
             (SELECT count(*) FROM academic_activity_evaluations evaluation
                WHERE evaluation.learning_offering_id = ANY($1)) AS activity_evaluations,
             (SELECT count(*) FROM learning_group_student_evaluations evaluation
                WHERE evaluation.learning_offering_id = ANY($1)) AS learner_evaluations,
             (SELECT count(*) FROM (
                  SELECT course_lock.id
                  FROM academic_course_result_locks course_lock
                  WHERE EXISTS (
                      SELECT 1 FROM course_offering_details detail
                      WHERE detail.learning_offering_id = ANY($1)
                        AND detail.subject_id = course_lock.subject_id
                        AND detail.academic_term_id = course_lock.academic_term_id
                        AND detail.academic_year_id = course_lock.academic_year_id
                  )
                  UNION ALL
                  SELECT activity_lock.id
                  FROM academic_activity_result_locks activity_lock
                  WHERE activity_lock.learning_offering_id = ANY($1)
                  UNION ALL
                  SELECT evaluation_lock.id
                  FROM subject_term_evaluation_locks evaluation_lock
                  WHERE EXISTS (
                      SELECT 1 FROM course_offering_details detail
                      WHERE detail.learning_offering_id = ANY($1)
                        AND detail.subject_id = evaluation_lock.subject_id
                        AND detail.academic_term_id = evaluation_lock.academic_term_id
                        AND detail.academic_year_id = evaluation_lock.academic_year_id
                  )
              ) official_lock) AS official_result_locks,
             ((SELECT count(*) FROM academic_course_results result
                 WHERE result.learning_offering_id = ANY($1))
              + (SELECT count(*) FROM academic_activity_results result
                 WHERE result.learning_offering_id = ANY($1))
              + (SELECT count(*) FROM subject_term_student_evaluations result
                 WHERE result.learning_offering_id = ANY($1))) AS official_results,
             (SELECT count(*) FROM academic_result_corrections correction
                WHERE EXISTS (
                    SELECT 1 FROM academic_course_results result
                    WHERE result.id = correction.course_result_id
                      AND result.learning_offering_id = ANY($1)
                ) OR EXISTS (
                    SELECT 1 FROM academic_activity_results result
                    WHERE result.id = correction.activity_result_id
                      AND result.learning_offering_id = ANY($1)
                ) OR EXISTS (
                    SELECT 1 FROM subject_term_student_evaluations result
                    WHERE result.id = correction.subject_student_evaluation_id
                      AND result.learning_offering_id = ANY($1)
                )) AS result_corrections,
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
        course_assessment_phases: row.get("course_assessment_phases"),
        learning_group_score_items: row.get("learning_group_score_items"),
        student_scores: row.get("student_scores"),
        result_selections: row.get("result_selections"),
        result_confirmations: row.get("result_confirmations"),
        activity_evaluations: row.get("activity_evaluations"),
        learner_evaluations: row.get("learner_evaluations"),
        official_result_locks: row.get("official_result_locks"),
        official_results: row.get("official_results"),
        result_corrections: row.get("result_corrections"),
        exam_schedule_items: row.get("exam_schedule_items"),
        supervision_observations: row.get("supervision_observations"),
    })
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
        AcademicChangeFindingCode::TermNotWritable => "term_not_writable",
        AcademicChangeFindingCode::EffectiveDateInvalid => "effective_date_invalid",
        AcademicChangeFindingCode::BaseDeliveryVersionStale => "base_delivery_version_stale",
        AcademicChangeFindingCode::ResourceStale => "resource_stale",
        AcademicChangeFindingCode::MissingDeliveryTarget => "missing_delivery_target",
        AcademicChangeFindingCode::MissingDeliveryGroup => "missing_delivery_group",
        AcademicChangeFindingCode::DeliveryGraphInvalid => "delivery_graph_invalid",
        AcademicChangeFindingCode::MissingPrimaryTeacher => "missing_primary_teacher",
        AcademicChangeFindingCode::MissingWeeklyPeriodTarget => "missing_weekly_period_target",
        AcademicChangeFindingCode::MissingEffectiveTeacher => "missing_effective_teacher",
    }
}

pub async fn create_change_set(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateAcademicTermChangeSetRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    let mut tx = pool.begin().await?;
    let id = create_change_set_in_transaction(&mut tx, actor_user_id, request).await?;
    tx.commit().await?;
    get_change_set(pool, id).await
}

pub async fn create_change_set_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    request: CreateAcademicTermChangeSetRequest,
) -> Result<Uuid, AppError> {
    let reason = normalized_reason(&request.reason)?;
    let request_hash = stable_hash(&NormalizedCreateRequest {
        academic_term_id: request.academic_term_id,
        effective_from: request.effective_from,
        reason: &reason,
    })?;
    let term = require_writable_term(transaction, request.academic_term_id, true).await?;
    validate_effective_date(&term, request.effective_from)?;
    if let Some((existing_id,existing_hash)) = sqlx::query_as::<_, (Uuid,String)>(
        "SELECT id,creation_request_hash FROM academic_term_change_sets WHERE academic_term_id=$1 AND idempotency_key=$2"
    ).bind(term.id).bind(request.idempotency_key.to_string()).fetch_optional(&mut **transaction).await? {
        if existing_hash!=request_hash { return Err(AppError::Conflict("idempotencyKey นี้ถูกใช้กับคำขออื่นแล้ว".into())); }
        return Ok(existing_id);
    }
    let base_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1 FOR SHARE")
        .bind(term.id).fetch_optional(&mut **transaction).await?;
    let revision_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    sqlx::query(r#"INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,source_version_id,effective_from,snapshot,created_by)
        VALUES($1,$2,$3,$4,$5,'{"offerings":[]}'::jsonb,$6)"#)
        .bind(target_id).bind(term.id).bind(term.academic_year_id).bind(base_id).bind(request.effective_from).bind(actor_user_id)
        .execute(&mut **transaction).await?;
    sqlx::query("INSERT INTO academic_term_change_sets(id,academic_term_id,academic_year_id,effective_from,reason,idempotency_key,creation_request_hash,created_by,base_delivery_version_id,target_delivery_version_id)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(revision_id).bind(term.id).bind(term.academic_year_id).bind(request.effective_from).bind(&reason)
        .bind(request.idempotency_key.to_string()).bind(&request_hash).bind(actor_user_id).bind(base_id).bind(target_id)
        .execute(&mut **transaction).await?;
    versions::refresh_revision_snapshot(transaction, revision_id).await?;
    sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload)
        VALUES('academic_delivery_version.created','academic_delivery_version',$1,$2,$3,$4,$5)")
        .bind(target_id).bind(term.academic_year_id).bind(term.id).bind(actor_user_id)
        .bind(sqlx::types::Json(serde_json::json!({"changeSetId":revision_id,"sourceVersionId":base_id,"effectiveFrom":request.effective_from,"requestHash":request_hash})))
        .execute(&mut **transaction).await?;
    Ok(revision_id)
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
        row.target_delivery_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นเปิดสอนเป้าหมาย",
    )?;
    if request.effective_from != row.effective_from {
        let has_items: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM academic_term_change_items WHERE change_set_id=$1)",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if has_items {
            return Err(AppError::Conflict(
                "ร่างนี้มีรายการเปลี่ยนแปลงแล้ว กรุณาสร้างร่างเปิดสอนใหม่หากต้องเปลี่ยนวันที่".into(),
            ));
        }
        sqlx::query("UPDATE academic_delivery_versions SET effective_from=$1,row_version=row_version+1,updated_at=now() WHERE id=$2 AND status='draft'")
            .bind(request.effective_from).bind(target_version_id).execute(&mut *transaction).await?;
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
    versions::refresh_revision_snapshot(&mut transaction, id).await?;
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
        row.target_delivery_version_id,
        "ชุดการเปลี่ยนแปลงไม่มีรุ่นเปิดสอนเป้าหมาย",
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
        r#"UPDATE academic_delivery_versions
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
    let (item_id, action_code, no_op) = match request {
        UpsertAcademicTermChangeItemRequest::AddCourse { offering, .. } => {
            if offering.academic_term_id != term.id {
                return Err(AppError::ValidationError(
                    "รายการเปิดสอนต้องอยู่ในภาคเรียนเดียวกับชุดการเปลี่ยนแปลง".to_string(),
                ));
            }
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
            offerings::validate_targets(&mut transaction, &term, &offering.targets).await?;
            let offering_id = Uuid::new_v4();
            offerings::insert_activity(&mut transaction, offering_id, &term, offering).await?;
            set_added_offering_start(&mut transaction, offering_id, row.effective_from).await?;
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
                require_snapshot_offering(
                    &mut transaction,
                    row.target_delivery_version_id,
                    learning_offering_id,
                )
                .await?;
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
            require_snapshot_offering(
                &mut transaction,
                row.target_delivery_version_id,
                learning_offering_id,
            )
            .await?;
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
        UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
            item_row_version,
            learning_group_id,
            teacher_id,
            teacher_role,
            ..
        } => {
            require_teacher_change_group(&mut transaction, &term, learning_group_id).await?;
            require_active_staff(&mut transaction, teacher_id).await?;
            let overlapping_episode_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT id
                   FROM learning_group_teachers
                   WHERE learning_group_id = $1
                     AND teacher_id = $2
                     AND starts_on <= $3
                     AND (ends_on IS NULL OR ends_on >= $3)
                   ORDER BY starts_on DESC, id
                   LIMIT 1
                   FOR UPDATE"#,
            )
            .bind(learning_group_id)
            .bind(teacher_id)
            .bind(row.effective_from)
            .fetch_optional(&mut *transaction)
            .await?;
            if let Some(episode_id) = overlapping_episode_id {
                let has_matching_stop: bool = sqlx::query_scalar(
                    r#"SELECT EXISTS (
                           SELECT 1 FROM academic_term_change_items
                           WHERE change_set_id = $1
                             AND action_kind = 'stop_group_teacher'
                             AND learning_group_teacher_id = $2
                       )"#,
                )
                .bind(change_set_id)
                .bind(episode_id)
                .fetch_one(&mut *transaction)
                .await?;
                if !has_matching_stop {
                    return Err(AppError::Conflict(
                        "ครูคนนี้มีช่วงการสอนที่ครอบคลุมวันที่เริ่มใช้แล้ว".to_string(),
                    ));
                }
            }
            if let Some(existing) = find_teacher_add_item(
                &mut transaction,
                change_set_id,
                learning_group_id,
                teacher_id,
            )
            .await?
            {
                if item_row_version != Some(existing.row_version) {
                    return Err(AppError::Conflict(
                        "รายการเพิ่มครูถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
                    ));
                }
                if existing.teacher_role == Some(teacher_role) {
                    (existing.id, "add_group_teacher", true)
                } else {
                    sqlx::query(
                        r#"UPDATE academic_term_change_items
                           SET teacher_role = $1, row_version = row_version + 1,
                               updated_at = now()
                           WHERE id = $2"#,
                    )
                    .bind(teacher_role)
                    .bind(existing.id)
                    .execute(&mut *transaction)
                    .await?;
                    (existing.id, "add_group_teacher", false)
                }
            } else {
                if item_row_version.is_some() {
                    return Err(AppError::Conflict(
                        "ไม่พบรายการเพิ่มครูรุ่นที่ต้องการแก้ไข".to_string(),
                    ));
                }
                let item_id = insert_teacher_change_item(
                    &mut transaction,
                    change_set_id,
                    &term,
                    AcademicTermChangeActionKind::AddGroupTeacher,
                    learning_group_id,
                    None,
                    teacher_id,
                    Some(teacher_role),
                    actor_user_id,
                )
                .await?;
                (item_id, "add_group_teacher", false)
            }
        }
        UpsertAcademicTermChangeItemRequest::AdjustGroupTeacherRole {
            item_row_version,
            learning_group_id,
            learning_group_teacher_id,
            teacher_id,
            teacher_role,
            ..
        } => {
            require_teacher_change_group(&mut transaction, &term, learning_group_id).await?;
            require_active_staff(&mut transaction, teacher_id).await?;
            require_effective_teacher_episode(
                &mut transaction,
                &term,
                learning_group_id,
                learning_group_teacher_id,
                teacher_id,
                row.effective_from,
            )
            .await?;
            ensure_no_teacher_episode_action(
                &mut transaction,
                change_set_id,
                AcademicTermChangeActionKind::StopGroupTeacher,
                learning_group_teacher_id,
                "หยุดและปรับบทบาทช่วงการสอนเดียวกันในชุดเดียวไม่ได้",
            )
            .await?;
            if let Some(existing) = find_teacher_episode_item(
                &mut transaction,
                change_set_id,
                AcademicTermChangeActionKind::AdjustGroupTeacherRole,
                learning_group_teacher_id,
            )
            .await?
            {
                if item_row_version != Some(existing.row_version) {
                    return Err(AppError::Conflict(
                        "รายการปรับบทบาทครูถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
                    ));
                }
                if existing.teacher_role == Some(teacher_role) {
                    (existing.id, "adjust_group_teacher_role", true)
                } else {
                    sqlx::query(
                        r#"UPDATE academic_term_change_items
                           SET teacher_role = $1, row_version = row_version + 1,
                               updated_at = now()
                           WHERE id = $2"#,
                    )
                    .bind(teacher_role)
                    .bind(existing.id)
                    .execute(&mut *transaction)
                    .await?;
                    (existing.id, "adjust_group_teacher_role", false)
                }
            } else {
                if item_row_version.is_some() {
                    return Err(AppError::Conflict(
                        "ไม่พบรายการปรับบทบาทครูรุ่นที่ต้องการแก้ไข".to_string(),
                    ));
                }
                let item_id = insert_teacher_change_item(
                    &mut transaction,
                    change_set_id,
                    &term,
                    AcademicTermChangeActionKind::AdjustGroupTeacherRole,
                    learning_group_id,
                    Some(learning_group_teacher_id),
                    teacher_id,
                    Some(teacher_role),
                    actor_user_id,
                )
                .await?;
                (item_id, "adjust_group_teacher_role", false)
            }
        }
        UpsertAcademicTermChangeItemRequest::StopGroupTeacher {
            item_row_version,
            learning_group_id,
            learning_group_teacher_id,
            teacher_id,
            ..
        } => {
            require_teacher_change_group(&mut transaction, &term, learning_group_id).await?;
            require_effective_teacher_episode(
                &mut transaction,
                &term,
                learning_group_id,
                learning_group_teacher_id,
                teacher_id,
                row.effective_from,
            )
            .await?;
            ensure_no_teacher_episode_action(
                &mut transaction,
                change_set_id,
                AcademicTermChangeActionKind::AdjustGroupTeacherRole,
                learning_group_teacher_id,
                "หยุดและปรับบทบาทช่วงการสอนเดียวกันในชุดเดียวไม่ได้",
            )
            .await?;
            if let Some(existing) = find_teacher_episode_item(
                &mut transaction,
                change_set_id,
                AcademicTermChangeActionKind::StopGroupTeacher,
                learning_group_teacher_id,
            )
            .await?
            {
                if item_row_version != Some(existing.row_version) {
                    return Err(AppError::Conflict(
                        "รายการหยุดครูถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
                    ));
                }
                (existing.id, "stop_group_teacher", true)
            } else {
                if item_row_version.is_some() {
                    return Err(AppError::Conflict(
                        "ไม่พบรายการหยุดครูรุ่นที่ต้องการแก้ไข".to_string(),
                    ));
                }
                let item_id = insert_teacher_change_item(
                    &mut transaction,
                    change_set_id,
                    &term,
                    AcademicTermChangeActionKind::StopGroupTeacher,
                    learning_group_id,
                    Some(learning_group_teacher_id),
                    teacher_id,
                    None,
                    actor_user_id,
                )
                .await?;
                (item_id, "stop_group_teacher", false)
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
    match item.action_kind {
        AcademicTermChangeActionKind::AddOffering => {
            let offering_id = required_change_item_field(
                item.learning_offering_id,
                "รายการเพิ่มการเปิดสอนไม่มีรายการเปิดสอน",
            )?;
            let is_draft: bool = sqlx::query_scalar(
                "SELECT status='draft' FROM learning_offerings WHERE id=$1 FOR UPDATE",
            )
            .bind(offering_id)
            .fetch_one(&mut *transaction)
            .await?;
            if is_draft {
                require_draft_only_delete(&mut transaction, offering_id).await?;
            }
            sqlx::query("DELETE FROM academic_term_change_items WHERE id = $1")
                .bind(item.id)
                .execute(&mut *transaction)
                .await?;
            if is_draft {
                sqlx::query("DELETE FROM learning_groups WHERE learning_offering_id = $1")
                    .bind(offering_id)
                    .execute(&mut *transaction)
                    .await?;
                sqlx::query("DELETE FROM learning_offerings WHERE id = $1 AND status = 'draft'")
                    .bind(offering_id)
                    .execute(&mut *transaction)
                    .await?;
            }
        }
        AcademicTermChangeActionKind::StopOffering
        | AcademicTermChangeActionKind::AdjustWeeklyPeriodTarget => {
            sqlx::query("DELETE FROM academic_term_change_items WHERE id=$1")
                .bind(item.id)
                .execute(&mut *transaction)
                .await?;
        }
        AcademicTermChangeActionKind::AddGroupTeacher
        | AcademicTermChangeActionKind::AdjustGroupTeacherRole
        | AcademicTermChangeActionKind::StopGroupTeacher => {
            sqlx::query("DELETE FROM academic_term_change_items WHERE id = $1")
                .bind(item.id)
                .execute(&mut *transaction)
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
            "learningGroupId": item.learning_group_id,
            "learningGroupTeacherId": item.learning_group_teacher_id,
            "teacherId": item.teacher_id,
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

async fn require_snapshot_offering(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Option<Uuid>,
    offering_id: Uuid,
) -> Result<(), AppError> {
    let version_id = required_version_id(version_id, "ไม่พบรุ่นเปิดสอนแบบร่าง")?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM academic_delivery_versions version,
        jsonb_array_elements(version.snapshot->'offerings') offering
        WHERE version.id=$1 AND version.status='draft' AND (offering->>'id')::uuid=$2)",
    )
    .bind(version_id)
    .bind(offering_id)
    .fetch_one(&mut **transaction)
    .await?;
    if !exists {
        return Err(AppError::Conflict(
            "รายการเปิดสอนนี้ไม่ได้อยู่ในรุ่นเปิดสอนแบบร่าง".into(),
        ));
    }
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

#[allow(clippy::too_many_arguments)]
async fn insert_teacher_change_item(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    term: &TermContext,
    action_kind: AcademicTermChangeActionKind,
    learning_group_id: Uuid,
    learning_group_teacher_id: Option<Uuid>,
    teacher_id: Uuid,
    teacher_role: Option<LearningTeacherRole>,
    actor_user_id: Uuid,
) -> Result<Uuid, AppError> {
    let item_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_term_change_items (
               id, change_set_id, academic_term_id, academic_year_id, action_kind,
               learning_group_id, learning_group_teacher_id, teacher_id,
               teacher_role, created_by
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(item_id)
    .bind(change_set_id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(action_kind)
    .bind(learning_group_id)
    .bind(learning_group_teacher_id)
    .bind(teacher_id)
    .bind(teacher_role)
    .bind(actor_user_id)
    .execute(&mut **transaction)
    .await?;
    Ok(item_id)
}

async fn require_teacher_change_group(
    transaction: &mut Transaction<'_, Postgres>,
    term: &TermContext,
    learning_group_id: Uuid,
) -> Result<(), AppError> {
    let context: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        r#"SELECT academic_term_id, academic_year_id, status
           FROM learning_groups
           WHERE id = $1
           FOR UPDATE"#,
    )
    .bind(learning_group_id)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((academic_term_id, academic_year_id, status)) = context else {
        return Err(AppError::NotFound("ไม่พบกลุ่มเรียนที่ต้องการเปลี่ยนครู".to_string()));
    };
    if academic_term_id != term.id || academic_year_id != term.academic_year_id {
        return Err(AppError::ValidationError(
            "กลุ่มเรียนไม่อยู่ในภาคเรียนของชุดการเปลี่ยนแปลง".to_string(),
        ));
    }
    if status != "published" {
        return Err(AppError::Conflict(
            "เปลี่ยนครูกลางภาคได้เฉพาะกลุ่มเรียนที่เผยแพร่แล้ว".to_string(),
        ));
    }
    Ok(())
}

async fn require_active_staff(
    transaction: &mut Transaction<'_, Postgres>,
    teacher_id: Uuid,
) -> Result<(), AppError> {
    let eligible: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM users
               WHERE id = $1 AND user_type = 'staff' AND status = 'active'
           )"#,
    )
    .bind(teacher_id)
    .fetch_one(&mut **transaction)
    .await?;
    if eligible {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "เลือกได้เฉพาะบุคลากรที่ยังใช้งานอยู่".to_string(),
        ))
    }
}

async fn require_effective_teacher_episode(
    transaction: &mut Transaction<'_, Postgres>,
    term: &TermContext,
    learning_group_id: Uuid,
    learning_group_teacher_id: Uuid,
    teacher_id: Uuid,
    effective_from: NaiveDate,
) -> Result<(), AppError> {
    let episode: Option<(NaiveDate, Option<NaiveDate>)> = sqlx::query_as(
        r#"SELECT starts_on, ends_on
           FROM learning_group_teachers
           WHERE id = $1
             AND learning_group_id = $2
             AND teacher_id = $3
             AND academic_term_id = $4
             AND academic_year_id = $5
           FOR UPDATE"#,
    )
    .bind(learning_group_teacher_id)
    .bind(learning_group_id)
    .bind(teacher_id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((starts_on, ends_on)) = episode else {
        return Err(AppError::NotFound(
            "ไม่พบช่วงการสอนของครูในกลุ่มเรียนนี้".to_string(),
        ));
    };
    if effective_from <= starts_on || ends_on.is_some_and(|end| effective_from > end) {
        return Err(AppError::ValidationError(
            "วันที่เริ่มใช้ต้องอยู่หลังวันเริ่มช่วงการสอนเดิมและไม่เกินวันสิ้นสุดเดิม".to_string(),
        ));
    }
    Ok(())
}

async fn find_teacher_add_item(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    learning_group_id: Uuid,
    teacher_id: Uuid,
) -> Result<Option<ChangeItemRow>, AppError> {
    sqlx::query_as(
        r#"SELECT id, change_set_id, action_kind, learning_offering_id,
                  weekly_period_target, learning_group_id, learning_group_teacher_id,
                  teacher_id, teacher_role, row_version, created_by, created_at, updated_at
           FROM academic_term_change_items
           WHERE change_set_id = $1
             AND action_kind = 'add_group_teacher'
             AND learning_group_id = $2
             AND teacher_id = $3
           FOR UPDATE"#,
    )
    .bind(change_set_id)
    .bind(learning_group_id)
    .bind(teacher_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(AppError::from)
}

async fn find_teacher_episode_item(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    action_kind: AcademicTermChangeActionKind,
    learning_group_teacher_id: Uuid,
) -> Result<Option<ChangeItemRow>, AppError> {
    sqlx::query_as(
        r#"SELECT id, change_set_id, action_kind, learning_offering_id,
                  weekly_period_target, learning_group_id, learning_group_teacher_id,
                  teacher_id, teacher_role, row_version, created_by, created_at, updated_at
           FROM academic_term_change_items
           WHERE change_set_id = $1
             AND action_kind = $2
             AND learning_group_teacher_id = $3
           FOR UPDATE"#,
    )
    .bind(change_set_id)
    .bind(action_kind)
    .bind(learning_group_teacher_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(AppError::from)
}

async fn ensure_no_teacher_episode_action(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    action_kind: AcademicTermChangeActionKind,
    learning_group_teacher_id: Uuid,
    message: &str,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM academic_term_change_items
               WHERE change_set_id = $1
                 AND action_kind = $2
                 AND learning_group_teacher_id = $3
           )"#,
    )
    .bind(change_set_id)
    .bind(action_kind)
    .bind(learning_group_teacher_id)
    .fetch_one(&mut **transaction)
    .await?;
    if exists {
        Err(AppError::Conflict(message.to_string()))
    } else {
        Ok(())
    }
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
                  weekly_period_target, learning_group_id, learning_group_teacher_id,
                  teacher_id, teacher_role, row_version, created_by, created_at, updated_at
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
                  weekly_period_target, learning_group_id, learning_group_teacher_id,
                  teacher_id, teacher_role, row_version, created_by, created_at, updated_at
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
    versions::refresh_revision_snapshot(transaction, change_set_id).await?;
    Ok(())
}

async fn require_draft_only_delete(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
) -> Result<(), AppError> {
    let (status, downstream_count): (LearningOfferingStatus, i64) = sqlx::query_as(
        r#"SELECT offering.status,
                  (SELECT count(*) FROM academic_timetable_blocks block
                    WHERE block.learning_offering_id = offering.id)
                + (SELECT count(*) FROM course_assessment_plans plan
                    WHERE plan.learning_offering_id = offering.id)
                + (SELECT count(*) FROM learning_group_score_items item
                    WHERE item.learning_offering_id = offering.id)
                + (SELECT count(*) FROM learning_group_student_scores score
                    WHERE score.learning_offering_id = offering.id)
                + (SELECT count(*) FROM learning_group_result_overrides selection
                    JOIN learning_groups learning_group
                      ON learning_group.id = selection.learning_group_id
                    WHERE learning_group.learning_offering_id = offering.id)
                + (SELECT count(*) FROM learning_group_phase_confirmations confirmation
                    JOIN learning_groups learning_group
                      ON learning_group.id = confirmation.learning_group_id
                    WHERE learning_group.learning_offering_id = offering.id)
                + (SELECT count(*) FROM learning_group_result_confirmations confirmation
                    JOIN learning_groups learning_group
                      ON learning_group.id = confirmation.learning_group_id
                    WHERE learning_group.learning_offering_id = offering.id)
                + (SELECT count(*) FROM academic_activity_evaluations evaluation
                    WHERE evaluation.learning_offering_id = offering.id)
                + (SELECT count(*) FROM academic_activity_result_confirmations confirmation
                    WHERE confirmation.learning_offering_id = offering.id)
                + (SELECT count(*) FROM learning_group_student_evaluations evaluation
                    WHERE evaluation.learning_offering_id = offering.id)
                + (SELECT count(*) FROM learning_group_evaluation_confirmations confirmation
                    JOIN learning_groups learning_group
                      ON learning_group.id = confirmation.learning_group_id
                    WHERE learning_group.learning_offering_id = offering.id)
                + (SELECT count(*) FROM academic_course_result_locks result_lock
                    JOIN course_offering_details detail
                      ON detail.subject_id = result_lock.subject_id
                     AND detail.academic_term_id = result_lock.academic_term_id
                     AND detail.academic_year_id = result_lock.academic_year_id
                    WHERE detail.learning_offering_id = offering.id)
                + (SELECT count(*) FROM academic_activity_result_locks result_lock
                    WHERE result_lock.learning_offering_id = offering.id)
                + (SELECT count(*) FROM subject_term_evaluation_locks result_lock
                    JOIN course_offering_details detail
                      ON detail.subject_id = result_lock.subject_id
                     AND detail.academic_term_id = result_lock.academic_term_id
                     AND detail.academic_year_id = result_lock.academic_year_id
                    WHERE detail.learning_offering_id = offering.id)
                + (SELECT count(*) FROM academic_course_results result
                    WHERE result.learning_offering_id = offering.id)
                + (SELECT count(*) FROM academic_activity_results result
                    WHERE result.learning_offering_id = offering.id)
                + (SELECT count(*) FROM subject_term_student_evaluations result
                    WHERE result.learning_offering_id = offering.id)
                + (SELECT count(*) FROM academic_result_corrections correction
                    WHERE EXISTS (
                        SELECT 1 FROM academic_course_results result
                        WHERE result.id = correction.course_result_id
                          AND result.learning_offering_id = offering.id
                    ) OR EXISTS (
                        SELECT 1 FROM academic_activity_results result
                        WHERE result.id = correction.activity_result_id
                          AND result.learning_offering_id = offering.id
                    ) OR EXISTS (
                        SELECT 1 FROM subject_term_student_evaluations result
                        WHERE result.id = correction.subject_student_evaluation_id
                          AND result.learning_offering_id = offering.id
                    ))
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
    let row = sqlx::query_as::<_, ChangeSetRow>(sqlx::AssertSqlSafe(query))
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
                  weekly_period_target, learning_group_id, learning_group_teacher_id,
                  teacher_id, teacher_role, row_version, created_by, created_at, updated_at
           FROM academic_term_change_items
           WHERE change_set_id = ANY($1)
           ORDER BY change_set_id, created_at, id"#,
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let group_ids = item_rows
        .iter()
        .filter_map(|row| row.learning_group_id)
        .collect::<Vec<_>>();
    let group_labels = if group_ids.is_empty() {
        HashMap::new()
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            r#"SELECT id, concat_ws(' · ', nullif(code, ''), name)
               FROM learning_groups
               WHERE id = ANY($1)"#,
        )
        .bind(&group_ids)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect::<HashMap<_, _>>()
    };
    let teacher_ids = item_rows
        .iter()
        .filter_map(|row| row.teacher_id)
        .collect::<Vec<_>>();
    let teacher_labels = if teacher_ids.is_empty() {
        HashMap::new()
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            r#"SELECT id,
                      concat_ws(' ',
                          nullif(concat(coalesce(title, ''), first_name), ''),
                          nullif(last_name, ''))
               FROM users
               WHERE id = ANY($1)"#,
        )
        .bind(&teacher_ids)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect::<HashMap<_, _>>()
    };
    let mut items_by_change_set = HashMap::<Uuid, Vec<AcademicTermChangeItem>>::new();
    for row in item_rows {
        let learning_offering_id = row.learning_offering_id;
        let learning_group_id = row.learning_group_id;
        let learning_group_teacher_id = row.learning_group_teacher_id;
        let teacher_id = row.teacher_id;
        let teacher_role = row.teacher_role;
        let item = match row.action_kind {
            AcademicTermChangeActionKind::AddOffering => AcademicTermChangeItem::AddOffering {
                id: row.id,
                learning_offering_id: required_change_item_field(
                    learning_offering_id,
                    "รายการเพิ่มการเปิดสอนไม่มีรายการเปิดสอน",
                )?,
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
                learning_offering_id: required_change_item_field(
                    learning_offering_id,
                    "รายการหยุดเปิดสอนไม่มีรายการเปิดสอน",
                )?,
                row_version: row.row_version,
                created_by: row.created_by,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            AcademicTermChangeActionKind::AdjustWeeklyPeriodTarget => {
                AcademicTermChangeItem::AdjustWeeklyPeriodTarget {
                    id: row.id,
                    learning_offering_id: required_change_item_field(
                        learning_offering_id,
                        "รายการปรับจำนวนคาบไม่มีรายการเปิดสอน",
                    )?,
                    weekly_period_target: row.weekly_period_target.ok_or_else(|| {
                        AppError::InternalServerError("รายการปรับจำนวนคาบไม่มีค่าเป้าหมาย".to_string())
                    })?,
                    row_version: row.row_version,
                    created_by: row.created_by,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                }
            }
            AcademicTermChangeActionKind::AddGroupTeacher => {
                let group_id =
                    required_change_item_field(learning_group_id, "รายการเพิ่มครูไม่มีกลุ่มเรียน")?;
                let teacher_id = required_change_item_field(teacher_id, "รายการเพิ่มครูไม่มีครู")?;
                AcademicTermChangeItem::AddGroupTeacher {
                    id: row.id,
                    learning_group_id: group_id,
                    learning_group_label: required_change_item_label(
                        &group_labels,
                        group_id,
                        "ไม่พบชื่อกลุ่มเรียนของรายการเพิ่มครู",
                    )?,
                    teacher_id,
                    teacher_label: required_change_item_label(
                        &teacher_labels,
                        teacher_id,
                        "ไม่พบชื่อครูของรายการเพิ่มครู",
                    )?,
                    teacher_role: required_change_item_field(teacher_role, "รายการเพิ่มครูไม่มีบทบาท")?,
                    row_version: row.row_version,
                    created_by: row.created_by,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                }
            }
            AcademicTermChangeActionKind::AdjustGroupTeacherRole => {
                let group_id =
                    required_change_item_field(learning_group_id, "รายการปรับบทบาทครูไม่มีกลุ่มเรียน")?;
                let teacher_id = required_change_item_field(teacher_id, "รายการปรับบทบาทไม่มีครู")?;
                AcademicTermChangeItem::AdjustGroupTeacherRole {
                    id: row.id,
                    learning_group_id: group_id,
                    learning_group_label: required_change_item_label(
                        &group_labels,
                        group_id,
                        "ไม่พบชื่อกลุ่มเรียนของรายการปรับบทบาทครู",
                    )?,
                    learning_group_teacher_id: required_change_item_field(
                        learning_group_teacher_id,
                        "รายการปรับบทบาทไม่มีช่วงการสอน",
                    )?,
                    teacher_id,
                    teacher_label: required_change_item_label(
                        &teacher_labels,
                        teacher_id,
                        "ไม่พบชื่อครูของรายการปรับบทบาท",
                    )?,
                    teacher_role: required_change_item_field(
                        teacher_role,
                        "รายการปรับบทบาทไม่มีบทบาทใหม่",
                    )?,
                    row_version: row.row_version,
                    created_by: row.created_by,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                }
            }
            AcademicTermChangeActionKind::StopGroupTeacher => {
                let group_id =
                    required_change_item_field(learning_group_id, "รายการหยุดครูไม่มีกลุ่มเรียน")?;
                let teacher_id = required_change_item_field(teacher_id, "รายการหยุดครูไม่มีครู")?;
                let episode_id = required_change_item_field(
                    learning_group_teacher_id,
                    "รายการหยุดครูไม่มีช่วงการสอน",
                )?;
                let episode_role: LearningTeacherRole =
                    sqlx::query_scalar("SELECT role FROM learning_group_teachers WHERE id = $1")
                        .bind(episode_id)
                        .fetch_optional(pool)
                        .await?
                        .ok_or_else(|| {
                            AppError::InternalServerError("ไม่พบบทบาทเดิมของรายการหยุดครู".to_string())
                        })?;
                AcademicTermChangeItem::StopGroupTeacher {
                    id: row.id,
                    learning_group_id: group_id,
                    learning_group_label: required_change_item_label(
                        &group_labels,
                        group_id,
                        "ไม่พบชื่อกลุ่มเรียนของรายการหยุดครู",
                    )?,
                    learning_group_teacher_id: episode_id,
                    teacher_id,
                    teacher_label: required_change_item_label(
                        &teacher_labels,
                        teacher_id,
                        "ไม่พบชื่อครูของรายการหยุดครู",
                    )?,
                    teacher_role: episode_role,
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
                base_delivery_version_id: row.base_delivery_version_id,
                target_delivery_version_id: required_version_id(
                    row.target_delivery_version_id,
                    "ชุดการเปลี่ยนแปลงไม่มีรุ่นเปิดสอนเป้าหมาย",
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

fn required_change_item_field<T>(value: Option<T>, message: &str) -> Result<T, AppError> {
    value.ok_or_else(|| AppError::InternalServerError(message.to_string()))
}

fn required_change_item_label(
    labels: &HashMap<Uuid, String>,
    id: Uuid,
    message: &str,
) -> Result<String, AppError> {
    labels
        .get(&id)
        .cloned()
        .ok_or_else(|| AppError::InternalServerError(message.to_string()))
}
