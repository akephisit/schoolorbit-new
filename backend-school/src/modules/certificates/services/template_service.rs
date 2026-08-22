use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::{
        certificates::models::{
            AttachCertificateAssetRequest, AttachCertificateBackgroundRequest, CertificateElement,
            CertificateFontSource, CertificateLayoutV1, CertificatePageBox,
            CertificatePageGeometry, CertificateTemplateAsset, CertificateTemplateAssetKind,
            CertificateTemplateCapabilities, CertificateTemplateDeleteDisposition,
            CertificateTemplateDeleteResult, CertificateTemplateDetail,
            CertificateTemplateVariableCatalog, CreateCertificateTemplateRequest, GeometryAction,
            PageGeometry, RecipientType, UpdateCertificateTemplateRequest,
        },
        files::platform_types::{FileInspectionMetadata, PdfPageBox},
        school_fonts::{
            models::{
                AttachSchoolFontBatchRequest, InspectSchoolFontUploadsRequest,
                SchoolFontListResponse, SchoolFontUploadInspection,
            },
            services::{self as school_font_services, SchoolFontStagingRelation},
        },
    },
    policies::certificate_access_policy::{
        require_owner_action, require_template_action, CertificateAction,
    },
};

use super::{
    audit_service::{record_template_audit, CertificateTemplateAuditMetadata},
    import_validation::{
        normalize_display_text, normalize_name_for_match, normalize_template_name,
        referenced_variables, RESERVED_RENDER_VARIABLES,
    },
    layout::{
        adapt_layout_for_background, paper_label, validate_layout, validate_safe_margin,
        BackgroundLayoutAction,
    },
};

const POINTS_PER_MM: f64 = 72.0 / 25.4;
const MIN_PAGE_SIDE_MM: f64 = 25.0;
const MAX_PAGE_SIDE_MM: f64 = 600.0;
const MAX_PAGE_AREA_MM2: f64 = 250_000.0;

#[derive(Debug)]
pub struct CertificateTemplateMutationOutcome {
    pub template: CertificateTemplateDetail,
    pub detached_file_ids: Vec<Uuid>,
}

#[derive(Debug)]
pub struct CertificateTemplateDeleteOutcome {
    pub result: CertificateTemplateDeleteResult,
    pub detached_file_ids: Vec<Uuid>,
}

#[derive(Debug, FromRow)]
struct TemplateRow {
    id: Uuid,
    campaign_id: Uuid,
    owner_organization_unit_id: Option<Uuid>,
    name: String,
    background_file_id: Option<Uuid>,
    background_file_lifecycle_status: Option<String>,
    crop_box_x: Option<f64>,
    crop_box_y: Option<f64>,
    crop_box_width: Option<f64>,
    crop_box_height: Option<f64>,
    media_box_x: Option<f64>,
    media_box_y: Option<f64>,
    media_box_width: Option<f64>,
    media_box_height: Option<f64>,
    page_rotation: Option<i16>,
    paper_label: Option<String>,
    safe_margin_points: f64,
    show_safe_area: bool,
    allowed_recipient_types: Vec<String>,
    layout: sqlx::types::Json<CertificateLayoutV1>,
    is_active: bool,
    issued_certificate_count: i64,
    locked_request_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct AssetRow {
    template_id: Uuid,
    id: Uuid,
    file_id: Uuid,
    kind: String,
    display_name: String,
    created_at: DateTime<Utc>,
    lifecycle_status: String,
    inspection_metadata: sqlx::types::Json<FileInspectionMetadata>,
}

#[derive(Clone, Debug, FromRow)]
struct UploadedFileRow {
    file_id: Uuid,
    display_filename: String,
    purpose_code: String,
    lifecycle_status: String,
    retention_class: String,
    inspection_metadata: sqlx::types::Json<FileInspectionMetadata>,
    storage_status: Option<String>,
    scan_status: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct MissingVariableRequirement {
    title: bool,
    first_name: bool,
    last_name: bool,
    activity_item: bool,
    award_or_role: bool,
    custom_key_groups: Vec<Vec<String>>,
}

#[derive(Clone, Copy, Debug)]
struct TemplateCapabilityFlags {
    can_read: bool,
    can_update: bool,
    can_delete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExpectedAsset {
    Image,
}

const TEMPLATE_SELECT: &str = r#"
    SELECT
        t.id,
        t.campaign_id,
        c.owner_organization_unit_id,
        t.name,
        t.background_file_id,
        (SELECT file.lifecycle_status FROM files file WHERE file.id = t.background_file_id)
            AS background_file_lifecycle_status,
        t.crop_box_x,
        t.crop_box_y,
        t.crop_box_width,
        t.crop_box_height,
        t.media_box_x,
        t.media_box_y,
        t.media_box_width,
        t.media_box_height,
        t.page_rotation,
        t.paper_label,
        t.safe_margin_points,
        t.show_safe_area,
        t.allowed_recipient_types,
        t.layout,
        t.is_active,
        (SELECT COUNT(*) FROM certificates certificate WHERE certificate.template_id = t.id)
            AS issued_certificate_count,
        (SELECT candidate_lock.request_id
         FROM certificate_candidates candidate
         JOIN certificate_candidate_issue_locks candidate_lock
           ON candidate_lock.candidate_id = candidate.id
         JOIN certificate_issue_requests request ON request.id = candidate_lock.request_id
         WHERE candidate.template_id = t.id
           AND request.status IN ('pending', 'reviewing')
         ORDER BY request.submitted_at, request.id
         LIMIT 1) AS locked_request_id,
        t.created_at,
        t.updated_at
    FROM certificate_templates t
    JOIN certificate_campaigns c
      ON c.id = t.campaign_id
     AND c.status <> 'purging'
"#;

pub async fn list_templates(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
) -> Result<Vec<CertificateTemplateDetail>, AppError> {
    let owner_id = campaign_owner(pool, campaign_id).await?;
    require_owner_action(pool, actor, owner_id, CertificateAction::Read).await?;
    let rows = sqlx::query_as::<_, TemplateRow>(&format!(
        "{TEMPLATE_SELECT} WHERE t.campaign_id = $1 ORDER BY t.created_at, t.id"
    ))
    .bind(campaign_id)
    .fetch_all(pool)
    .await
    .map_err(template_db_error)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let template_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let mut assets_by_template = load_assets_for_templates(pool, &template_ids).await?;
    let custom_headers = load_custom_headers(pool, campaign_id).await?;
    let missing_counts = load_missing_counts(pool, &rows, &custom_headers).await?;
    let capabilities = load_template_capabilities(pool, actor, owner_id).await;
    rows.into_iter()
        .map(|row| {
            let assets = assets_by_template.remove(&row.id).unwrap_or_default();
            let missing_count = missing_counts.get(&row.id).copied().unwrap_or(0);
            build_detail_from_parts(row, assets, missing_count, capabilities)
        })
        .collect()
}

pub async fn get_template(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
) -> Result<CertificateTemplateDetail, AppError> {
    let row = fetch_template_row(pool, template_id).await?;
    require_owner_action(
        pool,
        actor,
        row.owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await?;
    build_detail(pool, actor, row).await
}

pub async fn create_template(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    payload: CreateCertificateTemplateRequest,
) -> Result<CertificateTemplateDetail, AppError> {
    let name = validate_template_name(&payload.name)?;
    let normalized_name = normalize_template_name(&name);
    let allowed = validate_recipient_types(payload.allowed_recipient_types)?;
    let owner_id = campaign_owner(pool, campaign_id).await?;
    require_owner_action(pool, actor, owner_id, CertificateAction::Create).await?;

    let mut tx = pool.begin().await.map_err(template_db_error)?;
    let locked_owner_id = lock_campaign_owner(&mut tx, campaign_id).await?;
    require_authorized_owner_unchanged(owner_id, locked_owner_id)?;
    let template_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_templates
            (campaign_id, name, normalized_name, allowed_recipient_types, created_by, updated_by)
         VALUES ($1, $2, $3, $4, $5, $5)
         RETURNING id",
    )
    .bind(campaign_id)
    .bind(name)
    .bind(normalized_name)
    .bind(allowed.iter().map(|kind| kind.as_str()).collect::<Vec<_>>())
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(template_db_error)?;
    record_template_audit(
        &mut tx,
        actor.user_id,
        "create",
        template_audit(
            campaign_id,
            template_id,
            None,
            None,
            ["name", "allowedRecipientTypes"],
            None,
        ),
    )
    .await?;
    tx.commit().await.map_err(template_db_error)?;
    fetch_detail_with_capabilities(pool, actor, template_id).await
}

pub async fn update_template(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    payload: UpdateCertificateTemplateRequest,
) -> Result<CertificateTemplateMutationOutcome, AppError> {
    let authorization = fetch_template_row(pool, template_id).await?;
    require_owner_action(
        pool,
        actor,
        authorization.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let can_read_locked_request = super::request_service::can_read_request(
        pool,
        actor,
        authorization.owner_organization_unit_id,
    )
    .await?;

    let requested_name = payload
        .name
        .as_deref()
        .map(validate_template_name)
        .transpose()?;
    let requested_allowed = payload
        .allowed_recipient_types
        .map(validate_recipient_types)
        .transpose()?;

    let mut tx = pool.begin().await.map_err(template_db_error)?;
    require_locked_campaign_owner_unchanged(
        &mut tx,
        authorization.owner_organization_unit_id,
        authorization.campaign_id,
    )
    .await?;
    let locked = lock_template(&mut tx, template_id).await?;
    require_template_campaign_unchanged(authorization.campaign_id, locked.campaign_id)?;
    if locked.updated_at != payload.expected_updated_at {
        return Err(AppError::Conflict(
            "แม่แบบถูกแก้ไขแล้ว กรุณาโหลดข้อมูลล่าสุด".to_string(),
        ));
    }
    require_template_not_locked(&mut tx, template_id, can_read_locked_request).await?;

    let next_allowed = requested_allowed.clone().unwrap_or_else(|| {
        parse_recipient_types(&locked.allowed_recipient_types).unwrap_or_default()
    });
    if requested_allowed.is_some() {
        require_recipient_compatibility(&mut tx, template_id, &next_allowed).await?;
    }

    let mut next_layout = locked.layout.0.clone();
    if let Some(layout) = payload.layout.clone() {
        next_layout = layout;
    }
    let next_margin = payload
        .safe_margin_points
        .unwrap_or(locked.safe_margin_points);
    let page = source_page_geometry(&locked)?;
    let custom_variables = if let Some(page) = page {
        let (width, height) = page.displayed_size();
        validate_safe_margin(next_margin, width, height).map_err(layout_error)?;
        let custom_variables = load_custom_headers_tx(&mut tx, locked.campaign_id).await?;
        validate_layout(&next_layout, page, &custom_variables).map_err(layout_error)?;
        validate_layout_asset_references(&mut tx, template_id, &next_layout).await?;
        custom_variables
    } else if !next_layout.elements.is_empty() {
        return Err(AppError::Conflict(
            "ต้องแนบพื้นหลังที่ถูกต้องก่อนบันทึกองค์ประกอบ".to_string(),
        ));
    } else {
        Vec::new()
    };
    if payload.layout.is_some() {
        sync_school_font_references(&mut tx, template_id, &next_layout).await?;
    }

    let missing_count = if payload.layout.is_some() && locked.issued_certificate_count > 0 {
        let previous_variables = variables_in_layout(&locked.layout.0)?;
        let next_variables = variables_in_layout(&next_layout)?;
        let introduced_variables = next_variables
            .difference(&previous_variables)
            .cloned()
            .collect::<BTreeSet<_>>();
        count_missing_issued_variables_tx(
            &mut tx,
            template_id,
            &introduced_variables,
            &custom_variables,
        )
        .await?
    } else {
        0
    };
    if missing_count > 0 && !payload.confirm_missing_issued_values {
        return Err(AppError::Conflict(format!(
            "แม่แบบนี้ทำให้เกียรติบัตรเดิม {missing_count} ใบมีตัวแปรว่าง กรุณายืนยันก่อนบันทึก"
        )));
    }

    let mut changed_fields = Vec::new();
    if requested_name.is_some() {
        changed_fields.push("name");
    }
    if requested_allowed.is_some() {
        changed_fields.push("allowedRecipientTypes");
    }
    if payload.safe_margin_points.is_some() {
        changed_fields.push("safeMarginPoints");
    }
    if payload.show_safe_area.is_some() {
        changed_fields.push("showSafeArea");
    }
    if payload.layout.is_some() {
        changed_fields.push("layout");
    }
    if payload.is_active.is_some() {
        changed_fields.push("isActive");
    }
    if changed_fields.is_empty() {
        drop(tx);
        return Ok(CertificateTemplateMutationOutcome {
            template: fetch_detail_with_capabilities(pool, actor, template_id).await?,
            detached_file_ids: Vec::new(),
        });
    }

    let next_name = requested_name.unwrap_or_else(|| locked.name.clone());
    let next_normalized_name = normalize_template_name(&next_name);
    sqlx::query(
        "UPDATE certificate_templates
         SET name = $2,
             normalized_name = $3,
             allowed_recipient_types = $4,
             safe_margin_points = $5,
             show_safe_area = $6,
             layout = $7,
             is_active = $8,
             updated_by = $9,
             updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(template_id)
    .bind(next_name)
    .bind(next_normalized_name)
    .bind(
        next_allowed
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>(),
    )
    .bind(next_margin)
    .bind(payload.show_safe_area.unwrap_or(locked.show_safe_area))
    .bind(sqlx::types::Json(&next_layout))
    .bind(payload.is_active.unwrap_or(locked.is_active))
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await
    .map_err(template_db_error)?;
    record_template_audit(
        &mut tx,
        actor.user_id,
        "update",
        template_audit(
            locked.campaign_id,
            template_id,
            None,
            None,
            changed_fields,
            (missing_count > 0).then_some(missing_count),
        ),
    )
    .await?;
    tx.commit().await.map_err(template_db_error)?;

    Ok(CertificateTemplateMutationOutcome {
        template: fetch_detail_with_capabilities(pool, actor, template_id).await?,
        detached_file_ids: Vec::new(),
    })
}

pub async fn attach_background(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    payload: AttachCertificateBackgroundRequest,
) -> Result<CertificateTemplateMutationOutcome, AppError> {
    let authorization = fetch_template_row(pool, template_id).await?;
    require_owner_action(
        pool,
        actor,
        authorization.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let can_read_locked_request = super::request_service::can_read_request(
        pool,
        actor,
        authorization.owner_organization_unit_id,
    )
    .await?;

    let mut tx = pool.begin().await.map_err(template_db_error)?;
    require_locked_campaign_owner_unchanged(
        &mut tx,
        authorization.owner_organization_unit_id,
        authorization.campaign_id,
    )
    .await?;
    let locked = lock_template(&mut tx, template_id).await?;
    require_template_campaign_unchanged(authorization.campaign_id, locked.campaign_id)?;
    require_template_not_locked(&mut tx, template_id, can_read_locked_request).await?;
    let file = load_uploaded_file(
        &mut tx,
        template_id,
        payload.file_id,
        "certificate_template_background",
    )
    .await?;
    require_ready_file(&file)?;
    let metadata = file.inspection_metadata.0;
    let FileInspectionMetadata::Pdf {
        page_count,
        crop_box,
        media_box,
        rotation,
    } = metadata
    else {
        return Err(AppError::ValidationError(
            "พื้นหลังต้องเป็น PDF ที่ตรวจสอบแล้ว".to_string(),
        ));
    };
    if page_count != 1 {
        return Err(AppError::ValidationError(
            "พื้นหลังเกียรติบัตรต้องมีหนึ่งหน้า".to_string(),
        ));
    }
    let new_page = validated_pdf_page(&crop_box, rotation)?;
    validate_pdf_box(&media_box)?;
    let mut next_layout = locked.layout.0.clone();
    if let Some(old_page) = source_page_geometry(&locked)? {
        let geometry_changed =
            old_page != new_page && !super::layout::geometries_are_equivalent(old_page, new_page);
        if geometry_changed && !payload.preview_confirmed {
            return Err(AppError::Conflict(
                "ต้องพรีวิวและยืนยันการจัดวางเมื่อขนาดพื้นหลังเปลี่ยน".to_string(),
            ));
        }
        next_layout = adapt_layout_for_background(
            &next_layout,
            old_page,
            new_page,
            background_action(payload.geometry_action),
        )
        .map_err(layout_error)?;
    } else {
        next_layout = match payload.geometry_action {
            GeometryAction::Preserve => next_layout,
            GeometryAction::Reset => CertificateLayoutV1::default(),
            GeometryAction::Scale => {
                return Err(AppError::ValidationError(
                    "ยังไม่มี geometry เดิมสำหรับปรับตามสัดส่วน".to_string(),
                ))
            }
        };
    }
    let custom_variables = load_custom_headers_tx(&mut tx, locked.campaign_id).await?;
    validate_layout(&next_layout, new_page, &custom_variables).map_err(layout_error)?;
    validate_layout_asset_references(&mut tx, template_id, &next_layout).await?;
    sync_school_font_references(&mut tx, template_id, &next_layout).await?;
    let (displayed_width, displayed_height) = new_page.displayed_size();
    validate_safe_margin(locked.safe_margin_points, displayed_width, displayed_height)
        .map_err(layout_error)?;

    let label = paper_label(displayed_width, displayed_height);
    sqlx::query(
        "UPDATE certificate_templates
         SET background_file_id = $2,
             crop_box_x = $3,
             crop_box_y = $4,
             crop_box_width = $5,
             crop_box_height = $6,
             media_box_x = $7,
             media_box_y = $8,
             media_box_width = $9,
             media_box_height = $10,
             page_rotation = $11,
             paper_label = $12,
             layout = $13,
             updated_by = $14,
             updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(template_id)
    .bind(payload.file_id)
    .bind(crop_box.x)
    .bind(crop_box.y)
    .bind(crop_box.width)
    .bind(crop_box.height)
    .bind(media_box.x)
    .bind(media_box.y)
    .bind(media_box.width)
    .bind(media_box.height)
    .bind(rotation)
    .bind(label)
    .bind(sqlx::types::Json(&next_layout))
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await
    .map_err(template_db_error)?;
    promote_file(&mut tx, payload.file_id).await?;
    record_template_audit(
        &mut tx,
        actor.user_id,
        "attach_background",
        template_audit(
            locked.campaign_id,
            template_id,
            None,
            Some(payload.file_id),
            ["backgroundFileId", "pageGeometry", "layout"],
            Some(locked.issued_certificate_count),
        ),
    )
    .await?;
    tx.commit().await.map_err(template_db_error)?;

    let detached_file_ids = locked
        .background_file_id
        .filter(|old| *old != payload.file_id)
        .into_iter()
        .collect();
    Ok(CertificateTemplateMutationOutcome {
        template: fetch_detail_with_capabilities(pool, actor, template_id).await?,
        detached_file_ids,
    })
}

pub async fn attach_asset(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    payload: AttachCertificateAssetRequest,
) -> Result<CertificateTemplateDetail, AppError> {
    let authorization = fetch_template_row(pool, template_id).await?;
    require_owner_action(
        pool,
        actor,
        authorization.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let can_read_locked_request = super::request_service::can_read_request(
        pool,
        actor,
        authorization.owner_organization_unit_id,
    )
    .await?;
    let display_name = validate_asset_name(&payload.display_name)?;
    let expected_purpose = "certificate_template_image";

    let mut tx = pool.begin().await.map_err(template_db_error)?;
    require_locked_campaign_owner_unchanged(
        &mut tx,
        authorization.owner_organization_unit_id,
        authorization.campaign_id,
    )
    .await?;
    let locked = lock_template(&mut tx, template_id).await?;
    require_template_campaign_unchanged(authorization.campaign_id, locked.campaign_id)?;
    require_template_not_locked(&mut tx, template_id, can_read_locked_request).await?;
    let file = load_uploaded_file(&mut tx, template_id, payload.file_id, expected_purpose).await?;
    require_ready_file(&file)?;
    if !matches!(
        file.inspection_metadata.0,
        FileInspectionMetadata::Image { .. }
    ) {
        return Err(AppError::ValidationError(
            "ชนิดไฟล์ไม่ตรงกับชนิดทรัพยากรแม่แบบ".to_string(),
        ));
    }

    let asset_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_template_assets (
            template_id, file_id, kind, display_name, created_by
         ) VALUES ($1, $2, $3, $4, $5)
         RETURNING id",
    )
    .bind(template_id)
    .bind(payload.file_id)
    .bind(payload.kind.as_str())
    .bind(display_name)
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(template_db_error)?;
    promote_file(&mut tx, payload.file_id).await?;
    sqlx::query(
        "UPDATE certificate_templates
         SET updated_by = $2, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(template_id)
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await
    .map_err(template_db_error)?;
    record_template_audit(
        &mut tx,
        actor.user_id,
        "attach_asset",
        template_audit(
            locked.campaign_id,
            template_id,
            Some(asset_id),
            Some(payload.file_id),
            ["assets"],
            Some(locked.issued_certificate_count),
        ),
    )
    .await?;
    tx.commit().await.map_err(template_db_error)?;
    fetch_detail_with_capabilities(pool, actor, template_id).await
}

pub async fn list_fonts(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
) -> Result<SchoolFontListResponse, AppError> {
    require_template_action(pool, actor, template_id, CertificateAction::Read).await?;
    school_font_services::list_authorized(pool).await
}

pub async fn inspect_font_uploads(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    payload: InspectSchoolFontUploadsRequest,
) -> Result<SchoolFontUploadInspection, AppError> {
    require_template_action(pool, actor, template_id, CertificateAction::Update).await?;
    school_font_services::inspect_authorized(
        pool,
        SchoolFontStagingRelation::CertificateTemplate(template_id),
        payload,
    )
    .await
}

pub async fn attach_font_batch(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    payload: AttachSchoolFontBatchRequest,
) -> Result<SchoolFontListResponse, AppError> {
    require_template_action(pool, actor, template_id, CertificateAction::Update).await?;
    school_font_services::attach_authorized(
        pool,
        actor.user_id,
        SchoolFontStagingRelation::CertificateTemplate(template_id),
        payload,
    )
    .await
}

pub async fn delete_asset(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    asset_id: Uuid,
) -> Result<CertificateTemplateMutationOutcome, AppError> {
    let authorization = fetch_template_row(pool, template_id).await?;
    require_owner_action(
        pool,
        actor,
        authorization.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let can_read_locked_request = super::request_service::can_read_request(
        pool,
        actor,
        authorization.owner_organization_unit_id,
    )
    .await?;
    let mut tx = pool.begin().await.map_err(template_db_error)?;
    require_locked_campaign_owner_unchanged(
        &mut tx,
        authorization.owner_organization_unit_id,
        authorization.campaign_id,
    )
    .await?;
    let locked = lock_template(&mut tx, template_id).await?;
    require_template_campaign_unchanged(authorization.campaign_id, locked.campaign_id)?;
    require_template_not_locked(&mut tx, template_id, can_read_locked_request).await?;
    let layout = locked.layout.0.clone();
    if referenced_asset_ids(&layout).contains(&asset_id) {
        return Err(AppError::Conflict(
            "ทรัพยากรนี้ยังถูกใช้อยู่ในแม่แบบ กรุณานำออกจากงานออกแบบก่อน".to_string(),
        ));
    }
    let file_id = sqlx::query_scalar::<_, Uuid>(
        "DELETE FROM certificate_template_assets
         WHERE id = $1 AND template_id = $2
         RETURNING file_id",
    )
    .bind(asset_id)
    .bind(template_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(template_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบทรัพยากรแม่แบบ".to_string()))?;
    sqlx::query(
        "UPDATE certificate_templates
         SET updated_by = $2, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(template_id)
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await
    .map_err(template_db_error)?;
    record_template_audit(
        &mut tx,
        actor.user_id,
        "delete_asset",
        template_audit(
            locked.campaign_id,
            template_id,
            Some(asset_id),
            Some(file_id),
            ["assets"],
            Some(locked.issued_certificate_count),
        ),
    )
    .await?;
    tx.commit().await.map_err(template_db_error)?;
    Ok(CertificateTemplateMutationOutcome {
        template: fetch_detail_with_capabilities(pool, actor, template_id).await?,
        detached_file_ids: vec![file_id],
    })
}

pub async fn delete_template(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
) -> Result<CertificateTemplateDeleteOutcome, AppError> {
    let authorization = fetch_template_row(pool, template_id).await?;
    require_owner_action(
        pool,
        actor,
        authorization.owner_organization_unit_id,
        CertificateAction::Delete,
    )
    .await?;
    let can_read_locked_request = super::request_service::can_read_request(
        pool,
        actor,
        authorization.owner_organization_unit_id,
    )
    .await?;
    let mut tx = pool.begin().await.map_err(template_db_error)?;
    require_locked_campaign_owner_unchanged(
        &mut tx,
        authorization.owner_organization_unit_id,
        authorization.campaign_id,
    )
    .await?;
    let locked = lock_template(&mut tx, template_id).await?;
    require_template_campaign_unchanged(authorization.campaign_id, locked.campaign_id)?;
    require_template_not_locked(&mut tx, template_id, can_read_locked_request).await?;
    if locked.issued_certificate_count > 0 {
        sqlx::query(
            "UPDATE certificate_templates
             SET is_active = false, updated_by = $2, updated_at = clock_timestamp()
             WHERE id = $1",
        )
        .bind(template_id)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await
        .map_err(template_db_error)?;
        record_template_audit(
            &mut tx,
            actor.user_id,
            "deactivate",
            template_audit(
                locked.campaign_id,
                template_id,
                None,
                None,
                ["isActive"],
                Some(locked.issued_certificate_count),
            ),
        )
        .await?;
        tx.commit().await.map_err(template_db_error)?;
        return Ok(CertificateTemplateDeleteOutcome {
            result: CertificateTemplateDeleteResult {
                disposition: CertificateTemplateDeleteDisposition::Deactivated,
                detached_file_count: 0,
            },
            detached_file_ids: Vec::new(),
        });
    }

    let has_candidate = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1 FROM certificate_candidates WHERE template_id = $1
         )",
    )
    .bind(template_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(template_db_error)?;
    if has_candidate {
        return Err(AppError::Conflict(
            "ยังมีรายชื่อผู้รับอ้างถึงแม่แบบนี้ กรุณาย้ายหรือลบรายชื่อก่อน".to_string(),
        ));
    }

    let mut detached_file_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT file_id FROM certificate_template_assets WHERE template_id = $1",
    )
    .bind(template_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(template_db_error)?;
    if let Some(background_file_id) = locked.background_file_id {
        detached_file_ids.push(background_file_id);
    }
    record_template_audit(
        &mut tx,
        actor.user_id,
        "delete",
        template_audit(
            locked.campaign_id,
            template_id,
            None,
            None,
            ["template"],
            None,
        ),
    )
    .await?;
    sqlx::query("DELETE FROM certificate_templates WHERE id = $1")
        .bind(template_id)
        .execute(&mut *tx)
        .await
        .map_err(template_db_error)?;
    tx.commit().await.map_err(template_db_error)?;
    detached_file_ids.sort_unstable();
    detached_file_ids.dedup();
    Ok(CertificateTemplateDeleteOutcome {
        result: CertificateTemplateDeleteResult {
            disposition: CertificateTemplateDeleteDisposition::Deleted,
            detached_file_count: detached_file_ids.len().try_into().unwrap_or(u32::MAX),
        },
        detached_file_ids,
    })
}

pub async fn variable_catalog(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
) -> Result<CertificateTemplateVariableCatalog, AppError> {
    let row = fetch_template_row(pool, template_id).await?;
    require_owner_action(
        pool,
        actor,
        row.owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await?;
    let custom = load_custom_headers(pool, row.campaign_id).await?;
    let variables = super::import_validation::variable_catalog(&custom).map_err(|_| {
        AppError::ValidationError("คอลัมน์เสริมของกิจกรรมไม่สามารถใช้เป็นตัวแปรได้".to_string())
    })?;
    Ok(CertificateTemplateVariableCatalog { variables })
}

async fn build_detail(
    pool: &PgPool,
    actor: &ActorContext,
    row: TemplateRow,
) -> Result<CertificateTemplateDetail, AppError> {
    let assets = load_assets(pool, row.id).await?;
    let custom_headers = load_custom_headers(pool, row.campaign_id).await?;
    let missing_count = load_missing_counts(pool, std::slice::from_ref(&row), &custom_headers)
        .await?
        .get(&row.id)
        .copied()
        .unwrap_or(0);
    let capabilities =
        load_template_capabilities(pool, actor, row.owner_organization_unit_id).await;
    build_detail_from_parts(row, assets, missing_count, capabilities)
}

fn build_detail_from_parts(
    row: TemplateRow,
    assets: Vec<AssetRow>,
    missing_variable_certificate_count: i64,
    capabilities: TemplateCapabilityFlags,
) -> Result<CertificateTemplateDetail, AppError> {
    let layout = row.layout.0.clone();
    let referenced = referenced_asset_ids(&layout);
    let available = assets.iter().map(|asset| asset.id).collect::<BTreeSet<_>>();
    let all_references_available = referenced.is_subset(&available);
    let page_geometry = response_page_geometry(&row)?;
    let is_ready = row.background_file_id.is_some()
        && row.background_file_lifecycle_status.as_deref() == Some("ready")
        && all_references_available
        && assets
            .iter()
            .filter(|asset| referenced.contains(&asset.id))
            .all(|asset| asset.lifecycle_status == "ready");
    let unlocked = row.locked_request_id.is_none();
    Ok(CertificateTemplateDetail {
        id: row.id,
        campaign_id: row.campaign_id,
        name: row.name,
        background_file_id: row.background_file_id,
        page_geometry,
        safe_margin_points: row.safe_margin_points,
        show_safe_area: row.show_safe_area,
        allowed_recipient_types: parse_recipient_types(&row.allowed_recipient_types)?,
        layout,
        is_ready,
        assets: assets
            .into_iter()
            .map(asset_response)
            .collect::<Result<_, _>>()?,
        is_active: row.is_active,
        issued_certificate_count: row.issued_certificate_count,
        missing_variable_certificate_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
        capabilities: CertificateTemplateCapabilities {
            can_read: capabilities.can_read,
            can_update: capabilities.can_update && unlocked,
            can_delete: capabilities.can_delete && unlocked,
            can_preview: capabilities.can_read && is_ready,
        },
    })
}

async fn load_template_capabilities(
    pool: &PgPool,
    actor: &ActorContext,
    owner_id: Option<Uuid>,
) -> TemplateCapabilityFlags {
    TemplateCapabilityFlags {
        can_read: require_owner_action(pool, actor, owner_id, CertificateAction::Read)
            .await
            .is_ok(),
        can_update: require_owner_action(pool, actor, owner_id, CertificateAction::Update)
            .await
            .is_ok(),
        can_delete: require_owner_action(pool, actor, owner_id, CertificateAction::Delete)
            .await
            .is_ok(),
    }
}

async fn fetch_detail_with_capabilities(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
) -> Result<CertificateTemplateDetail, AppError> {
    let row = fetch_template_row(pool, template_id).await?;
    build_detail(pool, actor, row).await
}

async fn fetch_template_row(pool: &PgPool, template_id: Uuid) -> Result<TemplateRow, AppError> {
    sqlx::query_as::<_, TemplateRow>(&format!("{TEMPLATE_SELECT} WHERE t.id = $1"))
        .bind(template_id)
        .fetch_optional(pool)
        .await
        .map_err(template_db_error)?
        .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบเกียรติบัตร".to_string()))
}

async fn lock_template(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
) -> Result<TemplateRow, AppError> {
    sqlx::query_as::<_, TemplateRow>(&format!(
        "{TEMPLATE_SELECT} WHERE t.id = $1 FOR UPDATE OF t"
    ))
    .bind(template_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(template_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบเกียรติบัตร".to_string()))
}

async fn lock_campaign_owner(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    let (owner_organization_unit_id, status) = sqlx::query_as::<_, (Option<Uuid>, String)>(
        "SELECT owner_organization_unit_id, status
         FROM certificate_campaigns
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(campaign_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(template_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรมเกียรติบัตร".to_string()))?;
    if status == "purging" {
        return Err(AppError::Conflict(
            "certificate_campaign_purging".to_string(),
        ));
    }
    Ok(owner_organization_unit_id)
}

async fn require_locked_campaign_owner_unchanged(
    tx: &mut Transaction<'_, Postgres>,
    authorized_owner_id: Option<Uuid>,
    campaign_id: Uuid,
) -> Result<(), AppError> {
    let locked_owner_id = lock_campaign_owner(tx, campaign_id).await?;
    require_authorized_owner_unchanged(authorized_owner_id, locked_owner_id)
}

fn require_authorized_owner_unchanged(
    authorized_owner_id: Option<Uuid>,
    locked_owner_id: Option<Uuid>,
) -> Result<(), AppError> {
    if authorized_owner_id == locked_owner_id {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "หน่วยงานเจ้าของกิจกรรมเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".to_string(),
        ))
    }
}

fn require_template_campaign_unchanged(
    authorized_campaign_id: Uuid,
    locked_campaign_id: Uuid,
) -> Result<(), AppError> {
    if authorized_campaign_id == locked_campaign_id {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "กิจกรรมของแม่แบบเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".to_string(),
        ))
    }
}

async fn campaign_owner(pool: &PgPool, campaign_id: Uuid) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT owner_organization_unit_id
         FROM certificate_campaigns
         WHERE id = $1 AND status <> 'purging'",
    )
    .bind(campaign_id)
    .fetch_optional(pool)
    .await
    .map_err(template_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรมเกียรติบัตร".to_string()))
}

async fn load_assets(pool: &PgPool, template_id: Uuid) -> Result<Vec<AssetRow>, AppError> {
    Ok(load_assets_for_templates(pool, &[template_id])
        .await?
        .remove(&template_id)
        .unwrap_or_default())
}

async fn load_assets_for_templates(
    pool: &PgPool,
    template_ids: &[Uuid],
) -> Result<BTreeMap<Uuid, Vec<AssetRow>>, AppError> {
    if template_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows = sqlx::query_as::<_, AssetRow>(
        "SELECT a.template_id, a.id, a.file_id, a.kind, a.display_name, a.created_at,
                f.lifecycle_status, f.inspection_metadata
         FROM certificate_template_assets a
         JOIN files f ON f.id = a.file_id
         WHERE a.template_id = ANY($1::uuid[])
         ORDER BY a.template_id, a.created_at, a.id",
    )
    .bind(template_ids)
    .fetch_all(pool)
    .await
    .map_err(template_db_error)?;
    let mut grouped = BTreeMap::<Uuid, Vec<AssetRow>>::new();
    for row in rows {
        grouped.entry(row.template_id).or_default().push(row);
    }
    Ok(grouped)
}

fn asset_response(row: AssetRow) -> Result<CertificateTemplateAsset, AppError> {
    let (image_width_pixels, image_height_pixels) = match row.kind.as_str() {
        "image" => {
            let FileInspectionMetadata::Image {
                width_px,
                height_px,
            } = row.inspection_metadata.0
            else {
                return Err(invalid_persisted_template());
            };
            (Some(width_px), Some(height_px))
        }
        _ => return Err(invalid_persisted_template()),
    };
    Ok(CertificateTemplateAsset {
        id: row.id,
        file_id: row.file_id,
        kind: CertificateTemplateAssetKind::Image,
        display_name: row.display_name,
        image_width_pixels,
        image_height_pixels,
        created_at: row.created_at,
    })
}

async fn load_uploaded_file(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
    file_id: Uuid,
    expected_purpose: &str,
) -> Result<UploadedFileRow, AppError> {
    let row = sqlx::query_as::<_, UploadedFileRow>(
        "SELECT f.id AS file_id, f.display_filename, f.purpose_code,
                f.lifecycle_status, f.retention_class,
                f.inspection_metadata,
                v.storage_status, v.scan_status
         FROM certificate_template_file_uploads upload
         JOIN files f ON f.id = upload.file_id
         LEFT JOIN file_versions v
           ON v.id = f.current_version_id AND v.file_id = f.id
         WHERE upload.file_id = $1
           AND upload.template_id = $2
           AND upload.purpose_code = $3
           AND f.purpose_code = $3
         FOR UPDATE OF f",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(expected_purpose)
    .fetch_optional(&mut **tx)
    .await
    .map_err(template_db_error)?
    .ok_or_else(|| AppError::Forbidden("ไฟล์นี้ไม่ได้อัปโหลดสำหรับแม่แบบและชนิดที่ระบุ".to_string()))?;
    if row.purpose_code != expected_purpose || row.retention_class != "temporary" {
        return Err(AppError::Conflict(
            "ไฟล์นี้ถูกใช้งานหรือไม่ใช่ไฟล์ชั่วคราวที่แนบได้".to_string(),
        ));
    }
    Ok(row)
}

fn require_ready_file(file: &UploadedFileRow) -> Result<(), AppError> {
    if file.lifecycle_status != "ready"
        || file.storage_status.as_deref() != Some("stored")
        || file.scan_status.as_deref() != Some("clean")
    {
        return Err(AppError::Conflict("ไฟล์ยังไม่พร้อมใช้งาน".to_string()));
    }
    Ok(())
}

async fn promote_file(tx: &mut Transaction<'_, Postgres>, file_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE files
         SET retention_class = 'standard', expires_at = NULL, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(file_id)
    .execute(&mut **tx)
    .await
    .map_err(template_db_error)?;
    Ok(())
}

async fn require_template_not_locked(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
    can_read_locked_request: bool,
) -> Result<(), AppError> {
    let request_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT candidate_lock.request_id
         FROM certificate_candidates candidate
         JOIN certificate_candidate_issue_locks candidate_lock
           ON candidate_lock.candidate_id = candidate.id
         JOIN certificate_issue_requests request ON request.id = candidate_lock.request_id
         WHERE candidate.template_id = $1
           AND request.status IN ('pending', 'reviewing')
         ORDER BY request.submitted_at, request.id
         LIMIT 1",
    )
    .bind(template_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(template_db_error)?;
    if let Some(request_id) = request_id {
        Err(super::request_service::resource_locked_error(
            request_id,
            can_read_locked_request,
        ))
    } else {
        Ok(())
    }
}

async fn require_recipient_compatibility(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
    allowed: &[RecipientType],
) -> Result<(), AppError> {
    let values = allowed.iter().map(|kind| kind.as_str()).collect::<Vec<_>>();
    let incompatible: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM certificate_candidates
             WHERE template_id = $1 AND deleted_at IS NULL
               AND NOT (recipient_type = ANY($2::text[]))
             UNION ALL
             SELECT 1 FROM certificates
             WHERE template_id = $1
               AND NOT (recipient_type = ANY($2::text[]))
         )",
    )
    .bind(template_id)
    .bind(values)
    .fetch_one(&mut **tx)
    .await
    .map_err(template_db_error)?;
    if incompatible {
        Err(AppError::Conflict(
            "ไม่สามารถนำประเภทผู้รับที่มีรายการใช้งานอยู่แล้วออกได้".to_string(),
        ))
    } else {
        Ok(())
    }
}

async fn validate_layout_asset_references(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
    layout: &CertificateLayoutV1,
) -> Result<(), AppError> {
    let expected = layout
        .elements
        .iter()
        .filter_map(|element| match element {
            CertificateElement::Image(image) => Some((image.asset_id, ExpectedAsset::Image)),
            CertificateElement::Text(_) | CertificateElement::Qr(_) => None,
        })
        .collect::<BTreeMap<_, _>>();
    if expected.is_empty() {
        return Ok(());
    }
    let ids = expected.keys().copied().collect::<Vec<_>>();
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT asset.id, asset.kind, file.lifecycle_status
         FROM certificate_template_assets asset
         JOIN files file ON file.id = asset.file_id
         WHERE asset.template_id = $1 AND asset.id = ANY($2::uuid[])",
    )
    .bind(template_id)
    .bind(ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(template_db_error)?;
    if rows.len() != expected.len()
        || rows.iter().any(|(id, kind, status)| {
            expected.get(id).is_none() || kind != "image" || status != "ready"
        })
    {
        return Err(AppError::Conflict(
            "งานออกแบบอ้างถึงรูปที่ไม่พร้อมใช้งาน".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn referenced_school_font_ids(layout: &CertificateLayoutV1) -> BTreeSet<Uuid> {
    layout
        .elements
        .iter()
        .filter_map(|element| match element {
            CertificateElement::Text(text) => match text.font_source {
                CertificateFontSource::SchoolFont { font_id } => Some(font_id),
                CertificateFontSource::BuiltIn => None,
            },
            CertificateElement::Image(_) | CertificateElement::Qr(_) => None,
        })
        .collect()
}

async fn sync_school_font_references(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
    layout: &CertificateLayoutV1,
) -> Result<(), AppError> {
    let mut expected = BTreeMap::new();
    for element in &layout.elements {
        let CertificateElement::Text(text) = element else {
            continue;
        };
        let CertificateFontSource::SchoolFont { font_id } = text.font_source else {
            continue;
        };
        let next = (text.font_family.clone(), text.font_weight, text.font_style);
        if expected
            .insert(font_id, next.clone())
            .is_some_and(|previous| previous != next)
        {
            return Err(AppError::ValidationError(
                "ฟอนต์กลางหนึ่งรายการต้องใช้ family น้ำหนัก และรูปแบบเดียวกัน".to_string(),
            ));
        }
    }

    let font_ids = expected.keys().copied().collect::<Vec<_>>();
    let fonts = match school_font_services::lock_authorized(tx, &font_ids).await {
        Err(AppError::NotFound(_)) => {
            return Err(AppError::Conflict(
                "งานออกแบบอ้างถึงฟอนต์กลางที่ไม่พร้อมใช้งาน".to_string(),
            ));
        }
        result => result?,
    };
    if fonts.iter().any(|font| {
        expected
            .get(&font.id)
            .is_none_or(|(family, weight, style)| {
                font.font_family != *family
                    || font.font_weight != *weight
                    || font.font_style != *style
            })
    }) {
        return Err(AppError::Conflict(
            "family น้ำหนัก หรือรูปแบบของฟอนต์กลางไม่ตรงกับงานออกแบบ".to_string(),
        ));
    }

    sqlx::query(
        "INSERT INTO certificate_template_font_references (template_id, font_id)
         SELECT $1, unnest($2::uuid[])
         ON CONFLICT (template_id, font_id) DO NOTHING",
    )
    .bind(template_id)
    .bind(&font_ids)
    .execute(&mut **tx)
    .await
    .map_err(template_db_error)?;
    sqlx::query(
        "DELETE FROM certificate_template_font_references
         WHERE template_id = $1 AND NOT (font_id = ANY($2::uuid[]))",
    )
    .bind(template_id)
    .bind(&font_ids)
    .execute(&mut **tx)
    .await
    .map_err(template_db_error)?;
    Ok(())
}

pub(super) fn referenced_asset_ids(layout: &CertificateLayoutV1) -> BTreeSet<Uuid> {
    layout
        .elements
        .iter()
        .filter_map(|element| match element {
            CertificateElement::Image(image) => Some(image.asset_id),
            CertificateElement::Text(_) => None,
            CertificateElement::Qr(_) => None,
        })
        .collect()
}

async fn load_custom_headers(pool: &PgPool, campaign_id: Uuid) -> Result<Vec<String>, AppError> {
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT custom_key
         FROM (
             SELECT jsonb_object_keys(custom_values) AS custom_key
             FROM certificate_candidates
             WHERE campaign_id = $1 AND deleted_at IS NULL
             UNION
             SELECT jsonb_object_keys(custom_values_snapshot) AS custom_key
             FROM certificates
             WHERE campaign_id = $1
         ) custom_keys
         ORDER BY custom_key",
    )
    .bind(campaign_id)
    .fetch_all(pool)
    .await
    .map_err(template_db_error)
}

async fn load_custom_headers_tx(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<Vec<String>, AppError> {
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT custom_key
         FROM (
             SELECT jsonb_object_keys(custom_values) AS custom_key
             FROM certificate_candidates
             WHERE campaign_id = $1 AND deleted_at IS NULL
             UNION
             SELECT jsonb_object_keys(custom_values_snapshot) AS custom_key
             FROM certificates
             WHERE campaign_id = $1
         ) custom_keys
         ORDER BY custom_key",
    )
    .bind(campaign_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(template_db_error)
}

async fn count_missing_issued_variables_tx(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
    required: &BTreeSet<String>,
    custom_headers: &[String],
) -> Result<i64, AppError> {
    if required.is_empty() {
        return Ok(0);
    }
    let requirements = BTreeMap::from([(
        template_id.to_string(),
        missing_variable_requirement(required, custom_headers)?,
    )]);
    let rows = sqlx::query_as::<_, (Uuid, i64)>(MISSING_VARIABLE_COUNTS_SQL)
        .bind(sqlx::types::Json(requirements))
        .fetch_all(&mut **tx)
        .await
        .map_err(template_db_error)?;
    Ok(rows
        .into_iter()
        .find_map(|(id, count)| (id == template_id).then_some(count))
        .unwrap_or(0))
}

async fn load_missing_counts(
    pool: &PgPool,
    templates: &[TemplateRow],
    custom_headers: &[String],
) -> Result<BTreeMap<Uuid, i64>, AppError> {
    let requirements = templates
        .iter()
        .map(|template| {
            let variables = variables_in_layout(&template.layout.0)?;
            Ok((
                template.id.to_string(),
                missing_variable_requirement(&variables, custom_headers)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, AppError>>()?;
    if requirements.is_empty() {
        return Ok(BTreeMap::new());
    }
    sqlx::query_as::<_, (Uuid, i64)>(MISSING_VARIABLE_COUNTS_SQL)
        .bind(sqlx::types::Json(requirements))
        .fetch_all(pool)
        .await
        .map(|rows| rows.into_iter().collect())
        .map_err(template_db_error)
}

const MISSING_VARIABLE_COUNTS_SQL: &str = r#"
    WITH requirements AS (
        SELECT key::uuid AS template_id, value AS required
        FROM jsonb_each($1::jsonb)
    )
    SELECT certificate.template_id, COUNT(*)::bigint AS missing_count
    FROM certificates certificate
    JOIN requirements requirement ON requirement.template_id = certificate.template_id
    WHERE
        (COALESCE((requirement.required ->> 'title')::boolean, false)
            AND NULLIF(BTRIM(COALESCE(certificate.title_snapshot, '')), '') IS NULL)
        OR (COALESCE((requirement.required ->> 'firstName')::boolean, false)
            AND NULLIF(BTRIM(certificate.first_name_snapshot), '') IS NULL)
        OR (COALESCE((requirement.required ->> 'lastName')::boolean, false)
            AND NULLIF(BTRIM(certificate.last_name_snapshot), '') IS NULL)
        OR (COALESCE((requirement.required ->> 'activityItem')::boolean, false)
            AND NULLIF(BTRIM(COALESCE(certificate.activity_item_snapshot, '')), '') IS NULL)
        OR (COALESCE((requirement.required ->> 'awardOrRole')::boolean, false)
            AND NULLIF(BTRIM(COALESCE(certificate.award_or_role_snapshot, '')), '') IS NULL)
        OR EXISTS (
            SELECT 1
            FROM jsonb_array_elements(
                COALESCE(requirement.required -> 'customKeyGroups', '[]'::jsonb)
            ) custom_group(keys)
            WHERE NOT EXISTS (
                SELECT 1
                FROM jsonb_array_elements_text(custom_group.keys) custom_key(key)
                WHERE NULLIF(
                    BTRIM(COALESCE(certificate.custom_values_snapshot ->> custom_key.key, '')),
                    ''
                ) IS NOT NULL
            )
        )
    GROUP BY certificate.template_id
"#;

fn missing_variable_requirement(
    required: &BTreeSet<String>,
    custom_headers: &[String],
) -> Result<MissingVariableRequirement, AppError> {
    let title_key = normalize_name_for_match("คำนำหน้า");
    let first_name_key = normalize_name_for_match("ชื่อ");
    let last_name_key = normalize_name_for_match("นามสกุล");
    let activity_key = normalize_name_for_match("รายการกิจกรรม");
    let award_key = normalize_name_for_match("รางวัลหรือบทบาท");
    let always_available = RESERVED_RENDER_VARIABLES
        .into_iter()
        .map(normalize_name_for_match)
        .collect::<BTreeSet<_>>();
    let mut custom_aliases = BTreeMap::<String, Vec<String>>::new();
    for header in custom_headers {
        custom_aliases
            .entry(normalize_name_for_match(header))
            .or_default()
            .push(header.clone());
    }
    let mut requirement = MissingVariableRequirement::default();
    for key in required {
        if key == &title_key {
            requirement.title = true;
        } else if key == &first_name_key {
            requirement.first_name = true;
        } else if key == &last_name_key {
            requirement.last_name = true;
        } else if key == &activity_key {
            requirement.activity_item = true;
        } else if key == &award_key {
            requirement.award_or_role = true;
        } else if always_available.contains(key) {
            continue;
        } else if let Some(aliases) = custom_aliases.get(key) {
            requirement.custom_key_groups.push(aliases.clone());
        } else {
            requirement.custom_key_groups.push(vec![key.clone()]);
        }
    }
    Ok(requirement)
}

fn variables_in_layout(layout: &CertificateLayoutV1) -> Result<BTreeSet<String>, AppError> {
    let mut variables = BTreeSet::new();
    for element in &layout.elements {
        if let CertificateElement::Text(text) = element {
            for variable in referenced_variables(&text.content)
                .map_err(|_| AppError::ValidationError("รูปแบบตัวแปรในข้อความไม่ถูกต้อง".to_string()))?
            {
                let normalized = normalize_name_for_match(&variable);
                variables.insert(normalized);
            }
        }
    }
    Ok(variables)
}

fn response_page_geometry(row: &TemplateRow) -> Result<Option<CertificatePageGeometry>, AppError> {
    match (
        row.crop_box_x,
        row.crop_box_y,
        row.crop_box_width,
        row.crop_box_height,
        row.media_box_x,
        row.media_box_y,
        row.media_box_width,
        row.media_box_height,
        row.page_rotation,
        row.paper_label.as_ref(),
    ) {
        (None, None, None, None, None, None, None, None, None, None) => Ok(None),
        (
            Some(crop_x),
            Some(crop_y),
            Some(crop_width),
            Some(crop_height),
            Some(media_x),
            Some(media_y),
            Some(media_width),
            Some(media_height),
            Some(rotation),
            Some(label),
        ) => {
            let page = PageGeometry::new(crop_width, crop_height, rotation)
                .map_err(|_| invalid_persisted_template())?;
            let (displayed_width, displayed_height) = page.displayed_size();
            Ok(Some(CertificatePageGeometry {
                crop_box: response_box(crop_x, crop_y, crop_width, crop_height),
                media_box: response_box(media_x, media_y, media_width, media_height),
                rotation,
                displayed_width_points: displayed_width,
                displayed_height_points: displayed_height,
                paper_label: label.clone(),
            }))
        }
        _ => Err(invalid_persisted_template()),
    }
}

fn source_page_geometry(row: &TemplateRow) -> Result<Option<PageGeometry>, AppError> {
    match (row.crop_box_width, row.crop_box_height, row.page_rotation) {
        (None, None, None) => Ok(None),
        (Some(width), Some(height), Some(rotation)) => PageGeometry::new(width, height, rotation)
            .map(Some)
            .map_err(|_| invalid_persisted_template()),
        _ => Err(invalid_persisted_template()),
    }
}

fn response_box(x: f64, y: f64, width: f64, height: f64) -> CertificatePageBox {
    CertificatePageBox {
        x_points: x,
        y_points: y,
        width_points: width,
        height_points: height,
    }
}

fn validated_pdf_page(box_value: &PdfPageBox, rotation: i16) -> Result<PageGeometry, AppError> {
    validate_pdf_box(box_value)?;
    let page = PageGeometry::new(box_value.width, box_value.height, rotation)
        .map_err(|_| AppError::ValidationError("geometry ของหน้า PDF ไม่ถูกต้อง".to_string()))?;
    let (width, height) = page.displayed_size();
    let width_mm = width / POINTS_PER_MM;
    let height_mm = height / POINTS_PER_MM;
    if width_mm < MIN_PAGE_SIDE_MM
        || height_mm < MIN_PAGE_SIDE_MM
        || width_mm > MAX_PAGE_SIDE_MM
        || height_mm > MAX_PAGE_SIDE_MM
        || width_mm * height_mm > MAX_PAGE_AREA_MM2
    {
        return Err(AppError::ValidationError(
            "ขนาดหน้า PDF ต้องอยู่ระหว่าง 25–600 มม. และพื้นที่ไม่เกิน 250,000 ตร.มม.".to_string(),
        ));
    }
    Ok(page)
}

fn validate_pdf_box(box_value: &PdfPageBox) -> Result<(), AppError> {
    if [box_value.x, box_value.y, box_value.width, box_value.height]
        .into_iter()
        .all(f64::is_finite)
        && box_value.width > 0.0
        && box_value.height > 0.0
    {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "geometry ของหน้า PDF ไม่ถูกต้อง".to_string(),
        ))
    }
}

fn validate_template_name(value: &str) -> Result<String, AppError> {
    let value = normalize_display_text(value);
    if value.is_empty() || value.chars().count() > 200 {
        Err(AppError::ValidationError(
            "ชื่อแม่แบบต้องมี 1 ถึง 200 ตัวอักษร".to_string(),
        ))
    } else {
        Ok(value)
    }
}

fn validate_asset_name(value: &str) -> Result<String, AppError> {
    let value = normalize_display_text(value);
    if value.is_empty() || value.chars().count() > 200 {
        Err(AppError::ValidationError(
            "ชื่อทรัพยากรต้องมี 1 ถึง 200 ตัวอักษร".to_string(),
        ))
    } else {
        Ok(value)
    }
}

fn validate_recipient_types(values: Vec<RecipientType>) -> Result<Vec<RecipientType>, AppError> {
    let present = values.into_iter().collect::<BTreeSet<_>>();
    let ordered = [
        RecipientType::Student,
        RecipientType::Staff,
        RecipientType::External,
    ]
    .into_iter()
    .filter(|kind| present.contains(kind))
    .collect::<Vec<_>>();
    if ordered.is_empty() {
        Err(AppError::ValidationError(
            "แม่แบบต้องรองรับผู้รับอย่างน้อยหนึ่งประเภท".to_string(),
        ))
    } else {
        Ok(ordered)
    }
}

fn parse_recipient_types(values: &[String]) -> Result<Vec<RecipientType>, AppError> {
    values
        .iter()
        .map(|value| RecipientType::parse(value).ok_or_else(invalid_persisted_template))
        .collect()
}

fn background_action(action: GeometryAction) -> BackgroundLayoutAction {
    match action {
        GeometryAction::Preserve => BackgroundLayoutAction::Preserve,
        GeometryAction::Scale => BackgroundLayoutAction::Scale,
        GeometryAction::Reset => BackgroundLayoutAction::Reset,
    }
}

fn template_audit(
    campaign_id: Uuid,
    template_id: Uuid,
    asset_id: Option<Uuid>,
    file_id: Option<Uuid>,
    changed_fields: impl IntoIterator<Item = &'static str>,
    affected_certificate_count: Option<i64>,
) -> CertificateTemplateAuditMetadata {
    CertificateTemplateAuditMetadata {
        campaign_id,
        template_id,
        asset_id,
        file_id,
        asset_ids: Vec::new(),
        file_ids: Vec::new(),
        changed_fields: changed_fields.into_iter().map(str::to_string).collect(),
        affected_certificate_count,
    }
}

fn layout_error(_error: super::layout::LayoutValidationError) -> AppError {
    AppError::ValidationError("ข้อมูลการจัดวางแม่แบบไม่ถูกต้อง".to_string())
}

fn invalid_persisted_template() -> AppError {
    AppError::InternalServerError("certificate_template_state_invalid".to_string())
}

fn template_db_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.code().as_deref() == Some("23505") {
            return AppError::Conflict("ชื่อแม่แบบหรือไฟล์นี้ถูกใช้งานแล้ว".to_string());
        }
    }
    AppError::DbError(error)
}
