use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::files::platform_types::{FileInspectionMetadata, FilePurpose, FontInspectionStyle},
    permissions::registry::codes,
};

use super::models::{
    AttachSchoolFontBatchRequest, InspectSchoolFontUploadsRequest, SchoolFontDeleteConflict,
    SchoolFontListResponse, SchoolFontStyle, SchoolFontSummary, SchoolFontUploadInspection,
    SchoolFontUploadInspectionFile, SchoolFontUploadStatus,
};

#[cfg(test)]
#[path = "services_tests.rs"]
mod tests;

const MAX_FONT_BATCH_FILES: usize = 40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SchoolFontStagingRelation {
    Central,
    CertificateTemplate(Uuid),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchoolFontDeleteOutcome {
    Deleted { file_id: Uuid },
    Conflict(SchoolFontDeleteConflict),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SchoolFontRecord {
    pub id: Uuid,
    pub file_id: Uuid,
    pub font_family: String,
    pub font_weight: u16,
    pub font_style: SchoolFontStyle,
}

#[derive(Clone, Debug, FromRow)]
struct SchoolFontRow {
    id: Uuid,
    display_name: String,
    font_family: String,
    font_weight: i16,
    font_style: String,
    reference_count: i64,
    created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, FromRow)]
struct UploadedFileRow {
    file_id: Uuid,
    display_filename: String,
    purpose_code: String,
    lifecycle_status: String,
    retention_class: String,
    inspection_metadata: sqlx::types::Json<FileInspectionMetadata>,
    storage_status: String,
    scan_status: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FontVariantKey {
    normalized_family: String,
    weight: u16,
    style: SchoolFontStyle,
}

pub(crate) fn normalize_family(value: &str) -> String {
    value.nfkc().collect::<String>().trim().to_lowercase()
}

fn display_family(value: &str) -> String {
    value.nfkc().collect::<String>().trim().to_string()
}

fn require_font_manager(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_permission(codes::FONT_MANAGE_SCHOOL)
}

pub async fn list_for_manager(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<SchoolFontListResponse, AppError> {
    require_font_manager(actor)?;
    list_authorized(pool).await
}

pub(crate) async fn list_authorized(pool: &PgPool) -> Result<SchoolFontListResponse, AppError> {
    let rows = sqlx::query_as::<_, SchoolFontRow>(
        "SELECT font.id, font.display_name, font.font_family,
                font.font_weight, font.font_style, font.created_at,
                COUNT(reference.font_id) AS reference_count
         FROM school_fonts AS font
         LEFT JOIN certificate_template_font_references AS reference
           ON reference.font_id = font.id
         GROUP BY font.id
         ORDER BY font.normalized_family, font.font_weight, font.font_style, font.id",
    )
    .fetch_all(pool)
    .await?;
    Ok(SchoolFontListResponse {
        items: rows
            .into_iter()
            .map(summary_from_row)
            .collect::<Result<_, _>>()?,
    })
}

pub async fn inspect_for_manager(
    pool: &PgPool,
    actor: &ActorContext,
    payload: InspectSchoolFontUploadsRequest,
) -> Result<SchoolFontUploadInspection, AppError> {
    require_font_manager(actor)?;
    inspect_authorized(pool, SchoolFontStagingRelation::Central, payload).await
}

/// Shared consumer primitive. Its caller must first authorize the exact
/// consumer operation; this function proves only the typed staging relation.
pub(crate) async fn inspect_authorized(
    pool: &PgPool,
    relation: SchoolFontStagingRelation,
    payload: InspectSchoolFontUploadsRequest,
) -> Result<SchoolFontUploadInspection, AppError> {
    let file_ids = validate_file_ids(payload.file_ids)?;
    let uploads = load_uploads(pool, relation, &file_ids).await?;
    let existing = load_existing_variants(pool).await?;
    Ok(classify_uploads(&uploads, &existing))
}

pub async fn attach_for_manager(
    pool: &PgPool,
    actor: &ActorContext,
    payload: AttachSchoolFontBatchRequest,
) -> Result<SchoolFontListResponse, AppError> {
    require_font_manager(actor)?;
    attach_authorized(
        pool,
        actor.user_id,
        SchoolFontStagingRelation::Central,
        payload,
    )
    .await
}

/// Shared consumer primitive. Its caller must first authorize the exact
/// consumer operation; the typed relation is locked and consumed atomically.
pub(crate) async fn attach_authorized(
    pool: &PgPool,
    actor_user_id: Uuid,
    relation: SchoolFontStagingRelation,
    payload: AttachSchoolFontBatchRequest,
) -> Result<SchoolFontListResponse, AppError> {
    let file_ids = validate_file_ids(payload.file_ids)?;
    if !payload.rights_confirmed {
        return Err(AppError::ValidationError(
            "school_font_rights_confirmation_required".to_string(),
        ));
    }

    let mut tx = pool.begin().await?;
    if let SchoolFontStagingRelation::CertificateTemplate(template_id) = relation {
        lock_active_certificate_campaign(&mut tx, template_id).await?;
    }
    let uploads = lock_uploads(&mut tx, relation, &file_ids).await?;
    let existing = load_existing_variants_tx(&mut tx).await?;
    let inspection = classify_uploads(&uploads, &existing);
    reject_non_ready_inspection(&inspection)?;

    let mut items = Vec::with_capacity(inspection.files.len());
    let mut font_ids = Vec::with_capacity(inspection.files.len());
    for file in &inspection.files {
        let family = file.font_family.as_deref().ok_or_else(invalid_font_state)?;
        let weight = file.font_weight.ok_or_else(invalid_font_state)?;
        let style = file.font_style.ok_or_else(invalid_font_state)?;
        let display_name = validated_display_name(&file.display_filename)?;
        let (id, created_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
            "INSERT INTO school_fonts (
                file_id, display_name, font_family, normalized_family,
                font_weight, font_style, rights_confirmed_by,
                rights_confirmed_at, created_by
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, now(), $7)
             RETURNING id, created_at",
        )
        .bind(file.file_id)
        .bind(&display_name)
        .bind(family)
        .bind(normalize_family(family))
        .bind(i16::try_from(weight).map_err(|_| invalid_font_state())?)
        .bind(style.as_str())
        .bind(actor_user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_font_insert_error)?;
        font_ids.push(id);
        items.push(SchoolFontSummary {
            id,
            display_name,
            font_family: family.to_string(),
            font_weight: weight,
            font_style: style,
            reference_count: 0,
            created_at,
        });
    }

    let promoted = sqlx::query(
        "UPDATE files
         SET retention_class = 'standard', expires_at = NULL, updated_at = clock_timestamp()
         WHERE id = ANY($1::uuid[]) AND retention_class = 'temporary'",
    )
    .bind(&file_ids)
    .execute(&mut *tx)
    .await?;
    if promoted.rows_affected() != file_ids.len() as u64 {
        return Err(AppError::ValidationError(
            "school_font_unavailable".to_string(),
        ));
    }
    delete_staging_rows(&mut tx, relation, &file_ids).await?;
    sqlx::query(
        "INSERT INTO audit_logs (user_id, action, entity_type, metadata)
         VALUES ($1, 'attach_batch', 'school_font_library', $2)",
    )
    .bind(actor_user_id)
    .bind(serde_json::json!({
        "fileCount": file_ids.len(),
        "fontIds": font_ids,
    }))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(SchoolFontListResponse { items })
}

async fn lock_active_certificate_campaign(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
) -> Result<(), AppError> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT campaign.status
         FROM certificate_templates AS template
         JOIN certificate_campaigns AS campaign ON campaign.id = template.campaign_id
         WHERE template.id = $1
         FOR UPDATE OF campaign",
    )
    .bind(template_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบเกียรติบัตร".to_string()))?;
    if status == "purging" {
        return Err(AppError::Conflict(
            "certificate_campaign_purging".to_string(),
        ));
    }
    Ok(())
}

pub async fn delete(
    pool: &PgPool,
    actor: &ActorContext,
    font_id: Uuid,
) -> Result<SchoolFontDeleteOutcome, AppError> {
    require_font_manager(actor)?;
    let mut tx = pool.begin().await?;
    let file_id =
        sqlx::query_scalar::<_, Uuid>("SELECT file_id FROM school_fonts WHERE id = $1 FOR UPDATE")
            .bind(font_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("school_font_not_found".to_string()))?;
    let reference_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM certificate_template_font_references
         WHERE font_id = $1",
    )
    .bind(font_id)
    .fetch_one(&mut *tx)
    .await?;
    if reference_count != 0 {
        return Ok(SchoolFontDeleteOutcome::Conflict(
            SchoolFontDeleteConflict { reference_count },
        ));
    }
    sqlx::query("DELETE FROM school_fonts WHERE id = $1")
        .bind(font_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO audit_logs (user_id, action, entity_type, entity_id, metadata)
         VALUES ($1, 'delete', 'school_font', $2, '{\"referenceCount\":0}'::jsonb)",
    )
    .bind(actor.user_id)
    .bind(font_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(SchoolFontDeleteOutcome::Deleted { file_id })
}

pub(crate) async fn lock_authorized(
    tx: &mut Transaction<'_, Postgres>,
    font_ids: &[Uuid],
) -> Result<Vec<SchoolFontRecord>, AppError> {
    if font_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, i16, String)>(
        "SELECT id, file_id, font_family, font_weight, font_style
         FROM school_fonts
         WHERE id = ANY($1::uuid[])
         ORDER BY id
         FOR KEY SHARE",
    )
    .bind(font_ids)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() != font_ids.iter().copied().collect::<BTreeSet<_>>().len() {
        return Err(AppError::NotFound("school_font_not_found".to_string()));
    }
    let mut by_id = rows
        .into_iter()
        .map(|row| school_font_record(row).map(|record| (record.id, record)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    font_ids
        .iter()
        .map(|font_id| {
            by_id
                .remove(font_id)
                .ok_or_else(|| AppError::NotFound("school_font_not_found".to_string()))
        })
        .collect()
}

fn validate_file_ids(file_ids: Vec<Uuid>) -> Result<Vec<Uuid>, AppError> {
    if file_ids.is_empty() || file_ids.len() > MAX_FONT_BATCH_FILES {
        return Err(AppError::ValidationError(
            "เลือกไฟล์ฟอนต์ได้ครั้งละ 1 ถึง 40 ไฟล์".to_string(),
        ));
    }
    if file_ids.iter().copied().collect::<BTreeSet<_>>().len() != file_ids.len() {
        return Err(AppError::ValidationError(
            "รายการไฟล์ฟอนต์ต้องไม่ซ้ำกัน".to_string(),
        ));
    }
    Ok(file_ids)
}

async fn load_uploads(
    pool: &PgPool,
    relation: SchoolFontStagingRelation,
    file_ids: &[Uuid],
) -> Result<Vec<UploadedFileRow>, AppError> {
    let rows = match relation {
        SchoolFontStagingRelation::Central => {
            sqlx::query_as::<_, UploadedFileRow>(
                "SELECT file.id AS file_id, file.display_filename, file.purpose_code,
                    file.lifecycle_status, file.retention_class, file.inspection_metadata,
                    version.storage_status, version.scan_status
             FROM school_font_file_uploads AS upload
             JOIN files AS file ON file.id = upload.file_id
             JOIN file_versions AS version
               ON version.id = file.current_version_id AND version.file_id = file.id
             WHERE file.id = ANY($1::uuid[])
               AND upload.purpose_code = 'school_font'
               AND file.purpose_code = 'school_font'
             ORDER BY file.id",
            )
            .bind(file_ids)
            .fetch_all(pool)
            .await?
        }
        SchoolFontStagingRelation::CertificateTemplate(template_id) => {
            sqlx::query_as::<_, UploadedFileRow>(
                "SELECT file.id AS file_id, file.display_filename, file.purpose_code,
                        file.lifecycle_status, file.retention_class, file.inspection_metadata,
                        version.storage_status, version.scan_status
                 FROM certificate_school_font_file_uploads AS upload
                 JOIN files AS file ON file.id = upload.file_id
                 JOIN file_versions AS version
                   ON version.id = file.current_version_id AND version.file_id = file.id
                 WHERE upload.template_id = $1
                   AND file.id = ANY($2::uuid[])
                   AND upload.purpose_code = 'school_font'
                   AND file.purpose_code = 'school_font'
                 ORDER BY file.id",
            )
            .bind(template_id)
            .bind(file_ids)
            .fetch_all(pool)
            .await?
        }
    };
    order_uploads(file_ids, rows)
}

async fn lock_uploads(
    tx: &mut Transaction<'_, Postgres>,
    relation: SchoolFontStagingRelation,
    file_ids: &[Uuid],
) -> Result<Vec<UploadedFileRow>, AppError> {
    let rows = match relation {
        SchoolFontStagingRelation::Central => {
            sqlx::query_as::<_, UploadedFileRow>(
                "SELECT file.id AS file_id, file.display_filename, file.purpose_code,
                    file.lifecycle_status, file.retention_class, file.inspection_metadata,
                    version.storage_status, version.scan_status
             FROM school_font_file_uploads AS upload
             JOIN files AS file ON file.id = upload.file_id
             JOIN file_versions AS version
               ON version.id = file.current_version_id AND version.file_id = file.id
             WHERE file.id = ANY($1::uuid[])
               AND upload.purpose_code = 'school_font'
               AND file.purpose_code = 'school_font'
             ORDER BY file.id
             FOR UPDATE OF upload, file, version",
            )
            .bind(file_ids)
            .fetch_all(&mut **tx)
            .await?
        }
        SchoolFontStagingRelation::CertificateTemplate(template_id) => {
            sqlx::query_as::<_, UploadedFileRow>(
                "SELECT file.id AS file_id, file.display_filename, file.purpose_code,
                        file.lifecycle_status, file.retention_class, file.inspection_metadata,
                        version.storage_status, version.scan_status
                 FROM certificate_school_font_file_uploads AS upload
                 JOIN files AS file ON file.id = upload.file_id
                 JOIN file_versions AS version
                   ON version.id = file.current_version_id AND version.file_id = file.id
                 WHERE upload.template_id = $1
                   AND file.id = ANY($2::uuid[])
                   AND upload.purpose_code = 'school_font'
                   AND file.purpose_code = 'school_font'
                 ORDER BY file.id
                 FOR UPDATE OF upload, file, version",
            )
            .bind(template_id)
            .bind(file_ids)
            .fetch_all(&mut **tx)
            .await?
        }
    };
    order_uploads(file_ids, rows)
}

fn order_uploads(
    file_ids: &[Uuid],
    rows: Vec<UploadedFileRow>,
) -> Result<Vec<UploadedFileRow>, AppError> {
    if rows.len() != file_ids.len() {
        return Err(AppError::ValidationError(
            "school_font_unavailable".to_string(),
        ));
    }
    let mut by_id = rows
        .into_iter()
        .map(|row| (row.file_id, row))
        .collect::<BTreeMap<_, _>>();
    file_ids
        .iter()
        .map(|file_id| {
            by_id
                .remove(file_id)
                .ok_or_else(|| AppError::ValidationError("school_font_unavailable".to_string()))
        })
        .collect()
}

async fn load_existing_variants(pool: &PgPool) -> Result<BTreeSet<FontVariantKey>, AppError> {
    let rows = sqlx::query_as::<_, (String, i16, String)>(
        "SELECT normalized_family, font_weight, font_style FROM school_fonts",
    )
    .fetch_all(pool)
    .await?;
    persisted_variant_set(rows)
}

async fn load_existing_variants_tx(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<BTreeSet<FontVariantKey>, AppError> {
    let rows = sqlx::query_as::<_, (String, i16, String)>(
        "SELECT normalized_family, font_weight, font_style FROM school_fonts",
    )
    .fetch_all(&mut **tx)
    .await?;
    persisted_variant_set(rows)
}

fn persisted_variant_set(
    rows: Vec<(String, i16, String)>,
) -> Result<BTreeSet<FontVariantKey>, AppError> {
    rows.into_iter()
        .map(|(normalized_family, weight, style)| {
            Ok(FontVariantKey {
                normalized_family,
                weight: u16::try_from(weight).map_err(|_| invalid_font_state())?,
                style: SchoolFontStyle::parse(&style).ok_or_else(invalid_font_state)?,
            })
        })
        .collect()
}

fn classify_uploads(
    uploads: &[UploadedFileRow],
    existing: &BTreeSet<FontVariantKey>,
) -> SchoolFontUploadInspection {
    let mut candidates = uploads
        .iter()
        .map(|upload| {
            let available = upload.purpose_code == FilePurpose::SchoolFont.code()
                && upload.retention_class == "temporary"
                && upload.lifecycle_status == "ready"
                && upload.storage_status == "stored"
                && upload.scan_status == "clean";
            let (family, weight, style, is_variable, metadata_is_font) =
                match &upload.inspection_metadata.0 {
                    FileInspectionMetadata::Font {
                        family_name,
                        weight,
                        style,
                        is_variable,
                        ..
                    } => (
                        family_name
                            .as_deref()
                            .map(display_family)
                            .filter(|value| !value.is_empty() && value.chars().count() <= 200),
                        Some(*weight),
                        Some(match style {
                            FontInspectionStyle::Normal => SchoolFontStyle::Normal,
                            FontInspectionStyle::Italic => SchoolFontStyle::Italic,
                        }),
                        *is_variable,
                        true,
                    ),
                    _ => (None, None, None, false, false),
                };
            let status = if !available || !metadata_is_font {
                Some(SchoolFontUploadStatus::Unavailable)
            } else if validated_display_name(&upload.display_filename).is_err() {
                Some(SchoolFontUploadStatus::InvalidDisplayName)
            } else if family.is_none() {
                Some(SchoolFontUploadStatus::MissingFamily)
            } else if is_variable {
                Some(SchoolFontUploadStatus::UnsupportedVariable)
            } else if weight.is_none_or(|value| !(100..=900).contains(&value) || value % 100 != 0) {
                Some(SchoolFontUploadStatus::UnsupportedWeight)
            } else {
                None
            };
            let key = status.is_none().then(|| FontVariantKey {
                normalized_family: normalize_family(family.as_deref().unwrap_or_default()),
                weight: weight.unwrap_or_default(),
                style: style.unwrap_or(SchoolFontStyle::Normal),
            });
            (
                SchoolFontUploadInspectionFile {
                    file_id: upload.file_id,
                    display_filename: upload.display_filename.clone(),
                    font_family: family,
                    font_weight: weight,
                    font_style: style,
                    status: status.unwrap_or(SchoolFontUploadStatus::Ready),
                },
                key,
            )
        })
        .collect::<Vec<_>>();
    let mut selected_counts = BTreeMap::<FontVariantKey, usize>::new();
    for (_, key) in &candidates {
        if let Some(key) = key {
            *selected_counts.entry(key.clone()).or_default() += 1;
        }
    }
    for (file, key) in &mut candidates {
        let Some(key) = key else {
            continue;
        };
        if selected_counts.get(key).copied().unwrap_or_default() > 1 {
            file.status = SchoolFontUploadStatus::DuplicateSelection;
        } else if existing.contains(key) {
            file.status = SchoolFontUploadStatus::DuplicateExisting;
        }
    }
    SchoolFontUploadInspection {
        files: candidates.into_iter().map(|(file, _)| file).collect(),
    }
}

fn reject_non_ready_inspection(inspection: &SchoolFontUploadInspection) -> Result<(), AppError> {
    if inspection
        .files
        .iter()
        .any(|file| file.status == SchoolFontUploadStatus::DuplicateExisting)
    {
        return Err(AppError::Conflict(
            "school_font_variant_conflict".to_string(),
        ));
    }
    if inspection
        .files
        .iter()
        .any(|file| file.status == SchoolFontUploadStatus::Unavailable)
    {
        return Err(AppError::ValidationError(
            "school_font_unavailable".to_string(),
        ));
    }
    if inspection
        .files
        .iter()
        .any(|file| file.status != SchoolFontUploadStatus::Ready)
    {
        return Err(AppError::ValidationError("school_font_invalid".to_string()));
    }
    Ok(())
}

async fn delete_staging_rows(
    tx: &mut Transaction<'_, Postgres>,
    relation: SchoolFontStagingRelation,
    file_ids: &[Uuid],
) -> Result<(), AppError> {
    let deleted = match relation {
        SchoolFontStagingRelation::Central => {
            sqlx::query("DELETE FROM school_font_file_uploads WHERE file_id = ANY($1::uuid[])")
                .bind(file_ids)
                .execute(&mut **tx)
                .await?
        }
        SchoolFontStagingRelation::CertificateTemplate(template_id) => {
            sqlx::query(
                "DELETE FROM certificate_school_font_file_uploads
             WHERE template_id = $1 AND file_id = ANY($2::uuid[])",
            )
            .bind(template_id)
            .bind(file_ids)
            .execute(&mut **tx)
            .await?
        }
    };
    if deleted.rows_affected() != file_ids.len() as u64 {
        return Err(AppError::ValidationError(
            "school_font_unavailable".to_string(),
        ));
    }
    Ok(())
}

fn validated_display_name(filename: &str) -> Result<String, AppError> {
    let value = filename.nfkc().collect::<String>().trim().to_string();
    if value.is_empty() || value.chars().count() > 200 {
        return Err(AppError::ValidationError("school_font_invalid".to_string()));
    }
    Ok(value)
}

fn summary_from_row(row: SchoolFontRow) -> Result<SchoolFontSummary, AppError> {
    Ok(SchoolFontSummary {
        id: row.id,
        display_name: row.display_name,
        font_family: row.font_family,
        font_weight: u16::try_from(row.font_weight).map_err(|_| invalid_font_state())?,
        font_style: SchoolFontStyle::parse(&row.font_style).ok_or_else(invalid_font_state)?,
        reference_count: row.reference_count,
        created_at: row.created_at,
    })
}

fn school_font_record(
    row: (Uuid, Uuid, String, i16, String),
) -> Result<SchoolFontRecord, AppError> {
    Ok(SchoolFontRecord {
        id: row.0,
        file_id: row.1,
        font_family: row.2,
        font_weight: u16::try_from(row.3).map_err(|_| invalid_font_state())?,
        font_style: SchoolFontStyle::parse(&row.4).ok_or_else(invalid_font_state)?,
    })
}

fn map_font_insert_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.code().as_deref() == Some("23505") {
            return AppError::Conflict("school_font_variant_conflict".to_string());
        }
    }
    AppError::DbError(error)
}

fn invalid_font_state() -> AppError {
    AppError::InternalServerError("school_font_state_invalid".to_string())
}
