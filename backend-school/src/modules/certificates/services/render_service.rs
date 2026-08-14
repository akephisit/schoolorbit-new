use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use chrono::{Datelike, NaiveDate, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::{
        certificates::models::{
            CertificateBuiltInFont, CertificateElement, CertificateFontSource, CertificatePageBox,
            CertificatePageGeometry, CertificatePreviewKind, CertificatePreviewManifestRequest,
            CertificateRenderCampaignValues, CertificateRenderFileGrant,
            CertificateRenderFontGrant, CertificateRenderImageGrant, CertificateRenderManifest,
            CertificateRenderManifestBatchRequest, CertificateTemplateAssetKind, PageGeometry,
        },
        certificates::verification_limiter::CertificateVerificationLimiter,
        files::{
            consumer_service::map_platform_error,
            platform_service::{FilePlatform, FilePlatformError},
            platform_types::DownloadGrant,
            repository::SqlFileRepository,
        },
    },
    policies::certificate_access_policy::{require_owner_action, CertificateAction},
    scheduling::SCHOOL_TIMEZONE,
    utils::field_encryption,
};

use super::{
    candidate_service,
    import_validation::{normalize_display_text, normalize_name_for_match},
    layout::validate_layout,
    template_service, verification_service,
};

#[derive(Debug, FromRow)]
struct CampaignRenderRow {
    academic_year_name: String,
    campaign_name: String,
    event_date: NaiveDate,
    owner_organization_unit_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct IssuedRenderAccessRow {
    owner_organization_unit_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct IssuedRenderRow {
    template_id: Uuid,
    template_name_snapshot: String,
    academic_year_value: i32,
    campaign_name: String,
    event_date: NaiveDate,
    title_snapshot: Option<String>,
    first_name_snapshot: String,
    last_name_snapshot: String,
    activity_item_snapshot: Option<String>,
    award_or_role_snapshot: Option<String>,
    custom_values_snapshot: sqlx::types::Json<BTreeMap<String, String>>,
    school_name_snapshot: String,
    owner_organization_unit_name_snapshot: Option<String>,
    issue_date: NaiveDate,
    certificate_number: String,
    qr_proof_encrypted: String,
    background_file_id: Uuid,
    background_lifecycle_status: String,
    crop_box_x: f64,
    crop_box_y: f64,
    crop_box_width: f64,
    crop_box_height: f64,
    media_box_x: f64,
    media_box_y: f64,
    media_box_width: f64,
    media_box_height: f64,
    page_rotation: i16,
    paper_label: String,
    layout: sqlx::types::Json<crate::modules::certificates::models::CertificateLayoutV1>,
}

#[derive(Debug, FromRow)]
struct IssuedRenderAssetRow {
    id: Uuid,
    file_id: Uuid,
    kind: String,
    font_family: Option<String>,
    font_weight: Option<i16>,
    lifecycle_status: String,
}

pub async fn preview_manifest(
    pool: &PgPool,
    actor: &ActorContext,
    platform: &FilePlatform,
    school_name: String,
    template_id: Uuid,
    request: CertificatePreviewManifestRequest,
) -> Result<CertificateRenderManifest, AppError> {
    let CertificatePreviewManifestRequest {
        preview_kind,
        candidate_id,
        sample_values,
        layout,
    } = request;
    let candidate_preview_id = match (preview_kind, candidate_id) {
        (CertificatePreviewKind::Candidate, Some(candidate_id)) => Some(candidate_id),
        (CertificatePreviewKind::Candidate, None) => {
            return Err(AppError::ValidationError(
                "การพรีวิวผู้รับจริงต้องระบุรายชื่อผู้รับ".to_string(),
            ));
        }
        (_, Some(_)) => {
            return Err(AppError::ValidationError(
                "รหัสผู้รับใช้ได้เฉพาะการพรีวิวผู้รับจริง".to_string(),
            ));
        }
        (_, None) => None,
    };
    if candidate_preview_id.is_some() && !sample_values.is_empty() {
        return Err(AppError::ValidationError(
            "การพรีวิวผู้รับจริงไม่รับค่าตัวอย่างทับข้อมูล".to_string(),
        ));
    }
    let template = template_service::get_template(pool, actor, template_id).await?;
    if !template.is_ready {
        return Err(AppError::Conflict(
            "แม่แบบยังไม่มีพื้นหลังหรือทรัพยากรที่พร้อมใช้งาน".to_string(),
        ));
    }
    let page_geometry = template.page_geometry.clone().ok_or_else(|| {
        AppError::InternalServerError("certificate_template_geometry_missing".to_string())
    })?;
    let background_file_id = template.background_file_id.ok_or_else(|| {
        AppError::InternalServerError("certificate_template_background_missing".to_string())
    })?;
    let source_page = PageGeometry::new(
        page_geometry.crop_box.width_points,
        page_geometry.crop_box.height_points,
        page_geometry.rotation,
    )
    .map_err(|_| {
        AppError::InternalServerError("certificate_template_geometry_invalid".to_string())
    })?;
    let campaign = sqlx::query_as::<_, CampaignRenderRow>(
        "SELECT academic_year.name AS academic_year_name,
                campaign.name AS campaign_name,
                campaign.event_date,
                owner.name AS owner_organization_unit_name
         FROM certificate_campaigns campaign
         JOIN academic_years academic_year ON academic_year.id = campaign.academic_year_id
         LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
         WHERE campaign.id = $1",
    )
    .bind(template.campaign_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรมเกียรติบัตร".to_string()))?;

    let catalog = template_service::variable_catalog(pool, actor, template_id).await?;
    let preview_layout = layout.unwrap_or_else(|| template.layout.clone());
    validate_layout(&preview_layout, source_page, &catalog.variables)
        .map_err(|_| AppError::ValidationError("layout สำหรับพรีวิวไม่ถูกต้อง".to_string()))?;
    let catalog = catalog
        .variables
        .into_iter()
        .map(|value| normalize_name_for_match(&value))
        .collect::<BTreeSet<_>>();
    let mut recipient_values = if let Some(candidate_id) = candidate_preview_id {
        candidate_service::preview_values(pool, actor, candidate_id, template_id).await?
    } else {
        sample_recipient_values(preview_kind)
    };
    for (key, value) in sample_values {
        let display_key = normalize_display_text(&key);
        let normalized_key = normalize_name_for_match(&display_key);
        if !catalog.contains(&normalized_key) || value.chars().count() > 500 {
            return Err(AppError::ValidationError(
                "ค่าตัวอย่างมีตัวแปรที่ไม่รองรับหรือยาวเกินกำหนด".to_string(),
            ));
        }
        recipient_values.insert(display_key, normalize_display_text(&value));
    }

    let issue_date = Utc::now().with_timezone(&SCHOOL_TIMEZONE).date_naive();
    let owner_name = campaign
        .owner_organization_unit_name
        .unwrap_or_else(|| school_name.clone());
    for (key, value) in [
        ("ปีการศึกษา", campaign.academic_year_name.clone()),
        ("ชื่อกิจกรรมหลัก", campaign.campaign_name.clone()),
        ("เลขเกียรติบัตร", "ตัวอย่าง".to_string()),
        ("วันที่จัดกิจกรรม", thai_date(campaign.event_date)),
        ("วันที่ออก", thai_date(issue_date)),
        ("ชื่อโรงเรียนผู้ออก", school_name.clone()),
        ("ชื่อหน่วยงานเจ้าของกิจกรรม", owner_name.clone()),
        ("QR_CODE", "ตัวอย่าง — ไม่มีหลักฐานการออกจริง".to_string()),
    ] {
        recipient_values.insert(key.to_string(), value);
    }

    let referenced_assets = referenced_asset_ids(&preview_layout);
    for element in &preview_layout.elements {
        let expected = match element {
            CertificateElement::Image(image) => {
                Some((image.asset_id, CertificateTemplateAssetKind::Image, None))
            }
            CertificateElement::Text(text) => match text.font_source {
                CertificateFontSource::Asset { asset_id } => Some((
                    asset_id,
                    CertificateTemplateAssetKind::Font,
                    Some((&text.font_family, text.font_weight)),
                )),
                CertificateFontSource::BuiltIn => None,
            },
            CertificateElement::Qr(_) => None,
        };
        let Some((asset_id, expected_kind, expected_font)) = expected else {
            continue;
        };
        let Some(asset) = template.assets.iter().find(|asset| asset.id == asset_id) else {
            return Err(AppError::ValidationError(
                "layout สำหรับพรีวิวอ้างถึงทรัพยากรที่ไม่อยู่ในแม่แบบ".to_string(),
            ));
        };
        let font_matches = expected_font.is_none_or(|(family, weight)| {
            asset.font_family.as_ref() == Some(family) && asset.font_weight == Some(weight)
        });
        if asset.kind != expected_kind || !font_matches {
            return Err(AppError::ValidationError(
                "layout สำหรับพรีวิวอ้างถึงชนิดหรือข้อมูลฟอนต์ที่ไม่ตรงกับแม่แบบ".to_string(),
            ));
        }
    }
    let repository = SqlFileRepository::new(pool.clone());
    let background_grant = file_grant(
        background_file_id,
        platform
            .private_download(&repository, background_file_id)
            .await
            .map_err(map_platform_error)?,
    )?;
    let mut font_grants = Vec::new();
    let mut image_grants = Vec::new();
    for asset in template
        .assets
        .iter()
        .filter(|asset| referenced_assets.contains(&asset.id))
    {
        let grant = file_grant(
            asset.file_id,
            platform
                .private_download(&repository, asset.file_id)
                .await
                .map_err(map_platform_error)?,
        )?;
        match asset.kind {
            CertificateTemplateAssetKind::Font => {
                font_grants.push(CertificateRenderFontGrant {
                    asset_id: asset.id,
                    file_id: asset.file_id,
                    family: asset.font_family.clone().ok_or_else(|| {
                        AppError::InternalServerError(
                            "certificate_template_font_family_missing".to_string(),
                        )
                    })?,
                    weight: asset.font_weight.ok_or_else(|| {
                        AppError::InternalServerError(
                            "certificate_template_font_weight_missing".to_string(),
                        )
                    })?,
                    url: grant.url,
                    expires_at: grant.expires_at,
                });
            }
            CertificateTemplateAssetKind::Image => {
                image_grants.push(CertificateRenderImageGrant {
                    asset_id: asset.id,
                    file_id: asset.file_id,
                    url: grant.url,
                    expires_at: grant.expires_at,
                });
            }
        }
    }

    Ok(CertificateRenderManifest {
        template_id,
        page_geometry,
        layout: preview_layout,
        campaign_values: CertificateRenderCampaignValues {
            academic_year: campaign.academic_year_name,
            campaign_name: campaign.campaign_name,
            event_date: campaign.event_date,
            issue_date,
            school_name,
            owner_organization_unit_name: owner_name,
        },
        recipient_values,
        certificate_number: "ตัวอย่าง".to_string(),
        qr_payload: "ตัวอย่าง — ไม่มีหลักฐานการออกจริง".to_string(),
        built_in_fonts: vec![
            CertificateBuiltInFont {
                family: "Sarabun".to_string(),
                weight: 400,
                asset_path: "/fonts/Sarabun-Regular.ttf".to_string(),
            },
            CertificateBuiltInFont {
                family: "Sarabun".to_string(),
                weight: 700,
                asset_path: "/fonts/Sarabun-Bold.ttf".to_string(),
            },
        ],
        font_grants,
        image_grants,
        background_grant,
        suggested_filename: format!("ตัวอย่าง-{}.pdf", filename_part(&template.name)),
    })
}

pub async fn issued_manifest(
    pool: &PgPool,
    actor: &ActorContext,
    platform: &FilePlatform,
    tenant_subdomain: &str,
    base_domain: &str,
    certificate_id: Uuid,
) -> Result<CertificateRenderManifest, AppError> {
    issued_manifest_inner(
        pool,
        Some(actor),
        platform,
        tenant_subdomain,
        base_domain,
        certificate_id,
    )
    .await
}

pub async fn public_manifest(
    pool: &PgPool,
    platform: &FilePlatform,
    tenant_subdomain: &str,
    base_domain: &str,
    tenant_id: Uuid,
    receipt: &str,
) -> Result<CertificateRenderManifest, AppError> {
    let certificate_id =
        verification_service::validate_public_render_receipt(receipt, tenant_id, Utc::now())?;
    public_manifest_for_certificate(
        pool,
        platform,
        tenant_subdomain,
        base_domain,
        certificate_id,
    )
    .await
}

async fn public_manifest_for_certificate(
    pool: &PgPool,
    platform: &FilePlatform,
    tenant_subdomain: &str,
    base_domain: &str,
    certificate_id: Uuid,
) -> Result<CertificateRenderManifest, AppError> {
    issued_manifest_inner(
        pool,
        None,
        platform,
        tenant_subdomain,
        base_domain,
        certificate_id,
    )
    .await
    .map_err(public_render_error)
}

pub async fn public_manifest_rate_limited(
    pool: &PgPool,
    platform: &FilePlatform,
    tenant_subdomain: &str,
    base_domain: &str,
    tenant_id: Uuid,
    client_ip: IpAddr,
    limiter: &CertificateVerificationLimiter,
    receipt: &str,
) -> Result<CertificateRenderManifest, AppError> {
    limiter.begin_ip_attempt(tenant_id, client_ip)?;
    let invalid_receipt_target = CertificateVerificationLimiter::target_digest(receipt);
    limiter.check_target(tenant_id, client_ip, invalid_receipt_target)?;
    let certificate_id = match verification_service::validate_public_render_receipt(
        receipt,
        tenant_id,
        Utc::now(),
    ) {
        Ok(certificate_id) => certificate_id,
        Err(error) => {
            limiter.record_failure(tenant_id, client_ip, invalid_receipt_target)?;
            return Err(error);
        }
    };
    let target = CertificateVerificationLimiter::target_digest(&certificate_id.to_string());
    limiter.check_target(tenant_id, client_ip, target)?;
    match public_manifest_for_certificate(
        pool,
        platform,
        tenant_subdomain,
        base_domain,
        certificate_id,
    )
    .await
    {
        Ok(manifest) => {
            limiter.record_success(tenant_id, client_ip, target);
            Ok(manifest)
        }
        Err(error) => {
            if matches!(&error, AppError::NotFound(message) if message == "ไม่พบข้อมูลที่ตรงกัน")
            {
                limiter.record_failure(tenant_id, client_ip, target)?;
            }
            Err(error)
        }
    }
}

async fn issued_manifest_inner(
    pool: &PgPool,
    actor: Option<&ActorContext>,
    platform: &FilePlatform,
    tenant_subdomain: &str,
    base_domain: &str,
    certificate_id: Uuid,
) -> Result<CertificateRenderManifest, AppError> {
    let mut tx = pool.begin().await?;
    if let Some(actor) = actor {
        let access = sqlx::query_as::<_, IssuedRenderAccessRow>(
            "SELECT campaign.owner_organization_unit_id
             FROM certificates certificate
             JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
             WHERE certificate.id = $1
             FOR SHARE OF campaign",
        )
        .bind(certificate_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบเกียรติบัตร".to_string()))?;
        require_owner_action(
            pool,
            actor,
            access.owner_organization_unit_id,
            CertificateAction::Download,
        )
        .await?;
    }
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status
         FROM certificates
         WHERE id = $1
         FOR SHARE",
    )
    .bind(certificate_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบเกียรติบัตร".to_string()))?;
    if status != "issued" {
        return Err(AppError::Conflict(
            "เกียรติบัตรที่ถูกเพิกถอนไม่สามารถสร้างไฟล์ได้".to_string(),
        ));
    }
    let canonical_origin = canonical_tenant_origin(tenant_subdomain, base_domain)?;
    let row = sqlx::query_as::<_, IssuedRenderRow>(
        "SELECT certificate.template_id, certificate.template_name_snapshot,
                certificate.academic_year_value, campaign.name AS campaign_name,
                campaign.event_date, certificate.title_snapshot,
                certificate.first_name_snapshot, certificate.last_name_snapshot,
                certificate.activity_item_snapshot, certificate.award_or_role_snapshot,
                certificate.custom_values_snapshot, certificate.school_name_snapshot,
                certificate.owner_organization_unit_name_snapshot,
                certificate.issue_date, certificate.certificate_number,
                certificate.qr_proof_encrypted, template.background_file_id,
                background.lifecycle_status AS background_lifecycle_status,
                template.crop_box_x, template.crop_box_y, template.crop_box_width,
                template.crop_box_height, template.media_box_x, template.media_box_y,
                template.media_box_width, template.media_box_height,
                template.page_rotation, template.paper_label, template.layout
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         JOIN certificate_templates template
           ON template.id = certificate.template_id
          AND template.campaign_id = certificate.campaign_id
         JOIN files background ON background.id = template.background_file_id
         WHERE certificate.id = $1
         FOR SHARE OF template, background",
    )
    .bind(certificate_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::Conflict("แม่แบบปัจจุบันไม่พร้อมสร้างเกียรติบัตร".to_string()))?;
    if row.background_lifecycle_status != "ready" {
        return Err(AppError::Conflict("พื้นหลังแม่แบบยังไม่พร้อมใช้งาน".to_string()));
    }
    let source_page = PageGeometry::new(row.crop_box_width, row.crop_box_height, row.page_rotation)
        .map_err(|_| {
            AppError::InternalServerError("certificate_template_geometry_invalid".to_string())
        })?;
    let (displayed_width_points, displayed_height_points) = source_page.displayed_size();
    let page_geometry = CertificatePageGeometry {
        crop_box: CertificatePageBox {
            x_points: row.crop_box_x,
            y_points: row.crop_box_y,
            width_points: row.crop_box_width,
            height_points: row.crop_box_height,
        },
        media_box: CertificatePageBox {
            x_points: row.media_box_x,
            y_points: row.media_box_y,
            width_points: row.media_box_width,
            height_points: row.media_box_height,
        },
        rotation: row.page_rotation,
        displayed_width_points,
        displayed_height_points,
        paper_label: row.paper_label,
    };
    let custom_headers = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT custom_key
         FROM (
             SELECT jsonb_object_keys(custom_values) AS custom_key
             FROM certificate_candidates candidate
             JOIN certificates certificate ON certificate.campaign_id = candidate.campaign_id
             WHERE certificate.id = $1 AND candidate.deleted_at IS NULL
             UNION
             SELECT jsonb_object_keys(other.custom_values_snapshot) AS custom_key
             FROM certificates other
             JOIN certificates certificate ON certificate.campaign_id = other.campaign_id
             WHERE certificate.id = $1
         ) custom_keys
         ORDER BY custom_key",
    )
    .bind(certificate_id)
    .fetch_all(&mut *tx)
    .await?;
    let catalog = super::import_validation::variable_catalog(&custom_headers).map_err(|_| {
        AppError::InternalServerError("certificate_variable_catalog_invalid".to_string())
    })?;
    validate_layout(&row.layout.0, source_page, &catalog)
        .map_err(|_| AppError::Conflict("layout ปัจจุบันไม่พร้อมสร้างเกียรติบัตร".to_string()))?;

    let assets = sqlx::query_as::<_, IssuedRenderAssetRow>(
        "SELECT asset.id, asset.file_id, asset.kind, asset.font_family,
                asset.font_weight, file.lifecycle_status
         FROM certificate_template_assets asset
         JOIN files file ON file.id = asset.file_id
         WHERE asset.template_id = $1
         ORDER BY asset.id
         FOR SHARE OF asset, file",
    )
    .bind(row.template_id)
    .fetch_all(&mut *tx)
    .await?;
    let referenced_assets = referenced_asset_ids(&row.layout.0);
    validate_issued_assets(&row.layout.0, &assets, &referenced_assets)?;

    let proof = Zeroizing::new(
        field_encryption::decrypt(&row.qr_proof_encrypted)
            .map_err(|_| AppError::InternalServerError("certificate_proof_invalid".to_string()))?,
    );
    let qr_payload = format!(
        "{canonical_origin}/verify/certificate/{}#proof={}",
        row.certificate_number,
        proof.as_str()
    );
    let owner_name = row
        .owner_organization_unit_name_snapshot
        .clone()
        .unwrap_or_else(|| row.school_name_snapshot.clone());
    let mut recipient_values = row.custom_values_snapshot.0;
    for header in &custom_headers {
        recipient_values.entry(header.clone()).or_default();
    }
    for (key, value) in [
        ("คำนำหน้า", row.title_snapshot.unwrap_or_default()),
        ("ชื่อ", row.first_name_snapshot.clone()),
        ("นามสกุล", row.last_name_snapshot.clone()),
        (
            "รายการกิจกรรม",
            row.activity_item_snapshot.unwrap_or_default(),
        ),
        (
            "รางวัลหรือบทบาท",
            row.award_or_role_snapshot.unwrap_or_default(),
        ),
        ("ปีการศึกษา", row.academic_year_value.to_string()),
        ("ชื่อกิจกรรมหลัก", row.campaign_name.clone()),
        ("เลขเกียรติบัตร", row.certificate_number.clone()),
        ("วันที่จัดกิจกรรม", thai_date(row.event_date)),
        ("วันที่ออก", thai_date(row.issue_date)),
        ("ชื่อโรงเรียนผู้ออก", row.school_name_snapshot.clone()),
        ("ชื่อหน่วยงานเจ้าของกิจกรรม", owner_name.clone()),
        ("QR_CODE", qr_payload.clone()),
    ] {
        recipient_values.insert(key.to_string(), value);
    }

    let background_grant =
        transaction_file_grant(&mut tx, platform, row.background_file_id).await?;
    let mut font_grants = Vec::new();
    let mut image_grants = Vec::new();
    for asset in assets
        .iter()
        .filter(|asset| referenced_assets.contains(&asset.id))
    {
        let grant = transaction_file_grant(&mut tx, platform, asset.file_id).await?;
        match asset.kind.as_str() {
            "font" => font_grants.push(CertificateRenderFontGrant {
                asset_id: asset.id,
                file_id: asset.file_id,
                family: asset.font_family.clone().ok_or_else(|| {
                    AppError::InternalServerError(
                        "certificate_template_font_family_missing".to_string(),
                    )
                })?,
                weight: u16::try_from(asset.font_weight.ok_or_else(|| {
                    AppError::InternalServerError(
                        "certificate_template_font_weight_missing".to_string(),
                    )
                })?)
                .map_err(|_| {
                    AppError::InternalServerError(
                        "certificate_template_font_weight_invalid".to_string(),
                    )
                })?,
                url: grant.url,
                expires_at: grant.expires_at,
            }),
            "image" => image_grants.push(CertificateRenderImageGrant {
                asset_id: asset.id,
                file_id: asset.file_id,
                url: grant.url,
                expires_at: grant.expires_at,
            }),
            _ => {
                return Err(AppError::InternalServerError(
                    "certificate_template_asset_kind_invalid".to_string(),
                ));
            }
        }
    }

    let manifest = CertificateRenderManifest {
        template_id: row.template_id,
        page_geometry,
        layout: row.layout.0,
        campaign_values: CertificateRenderCampaignValues {
            academic_year: row.academic_year_value.to_string(),
            campaign_name: row.campaign_name,
            event_date: row.event_date,
            issue_date: row.issue_date,
            school_name: row.school_name_snapshot,
            owner_organization_unit_name: owner_name,
        },
        recipient_values,
        certificate_number: row.certificate_number.clone(),
        qr_payload,
        built_in_fonts: built_in_fonts(),
        font_grants,
        image_grants,
        background_grant,
        suggested_filename: format!(
            "{}-{}-{}-{}.pdf",
            filename_part(&row.certificate_number),
            filename_part(&row.first_name_snapshot),
            filename_part(&row.last_name_snapshot),
            filename_part(&row.template_name_snapshot),
        ),
    };
    tx.commit().await?;
    Ok(manifest)
}

fn public_render_error(error: AppError) -> AppError {
    match error {
        AppError::NotFound(_) | AppError::Conflict(_) | AppError::ValidationError(_) => {
            AppError::NotFound("ไม่พบข้อมูลที่ตรงกัน".to_string())
        }
        other => other,
    }
}

pub async fn issued_manifests(
    pool: &PgPool,
    actor: &ActorContext,
    platform: &FilePlatform,
    tenant_subdomain: &str,
    base_domain: &str,
    campaign_id: Uuid,
    request: CertificateRenderManifestBatchRequest,
) -> Result<Vec<CertificateRenderManifest>, AppError> {
    if request.certificate_ids.is_empty() || request.certificate_ids.len() > 200 {
        return Err(AppError::ValidationError(
            "เลือกเกียรติบัตรสำหรับดาวน์โหลดได้ครั้งละ 1 ถึง 200 ใบ".to_string(),
        ));
    }
    let unique_ids = request
        .certificate_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != request.certificate_ids.len() {
        return Err(AppError::ValidationError(
            "รายการเกียรติบัตรสำหรับดาวน์โหลดต้องไม่ซ้ำกัน".to_string(),
        ));
    }
    let owner_organization_unit_id = sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT owner_organization_unit_id
         FROM certificate_campaigns
         WHERE id = $1",
    )
    .bind(campaign_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรมเกียรติบัตร".to_string()))?;
    require_owner_action(
        pool,
        actor,
        owner_organization_unit_id,
        CertificateAction::Download,
    )
    .await?;
    let matching_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM certificates
         WHERE campaign_id = $1 AND id = ANY($2::uuid[])",
    )
    .bind(campaign_id)
    .bind(&request.certificate_ids)
    .fetch_one(pool)
    .await?;
    if matching_count != request.certificate_ids.len() as i64 {
        return Err(AppError::NotFound("ไม่พบเกียรติบัตรที่เลือกในกิจกรรมนี้".to_string()));
    }

    let mut manifests = Vec::with_capacity(request.certificate_ids.len());
    for certificate_id in request.certificate_ids {
        manifests.push(
            issued_manifest(
                pool,
                actor,
                platform,
                tenant_subdomain,
                base_domain,
                certificate_id,
            )
            .await?,
        );
    }
    Ok(manifests)
}

async fn transaction_file_grant(
    tx: &mut Transaction<'_, Postgres>,
    platform: &FilePlatform,
    file_id: Uuid,
) -> Result<CertificateRenderFileGrant, AppError> {
    let delivery = SqlFileRepository::load_delivery_in_transaction(tx, file_id)
        .await
        .map_err(|error| map_platform_error(FilePlatformError::from(error)))?
        .ok_or_else(|| map_platform_error(FilePlatformError::NotFound))?;
    file_grant(
        file_id,
        platform
            .private_download_delivery(delivery)
            .await
            .map_err(map_platform_error)?,
    )
}

fn canonical_tenant_origin(tenant_subdomain: &str, base_domain: &str) -> Result<String, AppError> {
    fn valid_label(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 63
            && !value.starts_with('-')
            && !value.ends_with('-')
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }
    if tenant_subdomain.trim() != tenant_subdomain
        || !tenant_subdomain.is_ascii()
        || !valid_label(tenant_subdomain)
        || base_domain.trim() != base_domain
        || !base_domain.is_ascii()
        || base_domain.len() > 253
        || base_domain.split('.').count() < 2
        || base_domain.split('.').any(|label| !valid_label(label))
    {
        return Err(AppError::ConfigError(
            "certificate_canonical_origin_invalid".to_string(),
        ));
    }
    Ok(format!(
        "https://{}.{}",
        tenant_subdomain.to_ascii_lowercase(),
        base_domain.to_ascii_lowercase()
    ))
}

fn validate_issued_assets(
    layout: &crate::modules::certificates::models::CertificateLayoutV1,
    assets: &[IssuedRenderAssetRow],
    referenced: &BTreeSet<Uuid>,
) -> Result<(), AppError> {
    let by_id = assets
        .iter()
        .map(|asset| (asset.id, asset))
        .collect::<BTreeMap<_, _>>();
    for element in &layout.elements {
        let expected = match element {
            CertificateElement::Image(image) => Some((image.asset_id, "image", None)),
            CertificateElement::Text(text) => match text.font_source {
                CertificateFontSource::Asset { asset_id } => Some((
                    asset_id,
                    "font",
                    Some((&text.font_family, i16::try_from(text.font_weight).ok())),
                )),
                CertificateFontSource::BuiltIn => None,
            },
            CertificateElement::Qr(_) => None,
        };
        let Some((asset_id, kind, expected_font)) = expected else {
            continue;
        };
        let Some(asset) = by_id.get(&asset_id) else {
            return Err(AppError::Conflict(
                "แม่แบบอ้างถึงทรัพยากรที่ไม่พร้อมใช้งาน".to_string(),
            ));
        };
        let font_matches = expected_font.is_none_or(|(family, weight)| {
            weight.is_some()
                && asset.font_family.as_ref() == Some(family)
                && asset.font_weight == weight
        });
        if asset.kind != kind || asset.lifecycle_status != "ready" || !font_matches {
            return Err(AppError::Conflict(
                "ทรัพยากรของแม่แบบไม่ตรงกับ layout ปัจจุบัน".to_string(),
            ));
        }
    }
    if referenced
        .iter()
        .any(|asset_id| !by_id.contains_key(asset_id))
    {
        return Err(AppError::Conflict(
            "แม่แบบอ้างถึงทรัพยากรที่ไม่พร้อมใช้งาน".to_string(),
        ));
    }
    Ok(())
}

fn built_in_fonts() -> Vec<CertificateBuiltInFont> {
    vec![
        CertificateBuiltInFont {
            family: "Sarabun".to_string(),
            weight: 400,
            asset_path: "/fonts/Sarabun-Regular.ttf".to_string(),
        },
        CertificateBuiltInFont {
            family: "Sarabun".to_string(),
            weight: 700,
            asset_path: "/fonts/Sarabun-Bold.ttf".to_string(),
        },
    ]
}

fn sample_recipient_values(kind: CertificatePreviewKind) -> BTreeMap<String, String> {
    let (title, first_name, last_name) = match kind {
        CertificatePreviewKind::Short => ("ด.ช.", "ปัน", "ดี"),
        CertificatePreviewKind::Normal => ("เด็กหญิง", "กมลชนก", "สุขสวัสดิ์"),
        CertificatePreviewKind::Long => ("เด็กหญิง", "ณัฏฐณิชาภัทรวรรณ", "รัตนสุวรรณกุลชัยวัฒนา"),
        CertificatePreviewKind::Candidate => unreachable!("candidate preview is rejected above"),
    };
    BTreeMap::from([
        ("คำนำหน้า".to_string(), title.to_string()),
        ("ชื่อ".to_string(), first_name.to_string()),
        ("นามสกุล".to_string(), last_name.to_string()),
        ("รายการกิจกรรม".to_string(), "การแข่งขันคำคม".to_string()),
        (
            "รางวัลหรือบทบาท".to_string(),
            "รางวัลรองชนะเลิศอันดับที่ 1".to_string(),
        ),
    ])
}

fn referenced_asset_ids(
    layout: &crate::modules::certificates::models::CertificateLayoutV1,
) -> BTreeSet<Uuid> {
    layout
        .elements
        .iter()
        .filter_map(|element| match element {
            CertificateElement::Image(image) => Some(image.asset_id),
            CertificateElement::Text(text) => match text.font_source {
                CertificateFontSource::Asset { asset_id } => Some(asset_id),
                CertificateFontSource::BuiltIn => None,
            },
            CertificateElement::Qr(_) => None,
        })
        .collect()
}

fn file_grant(file_id: Uuid, grant: DownloadGrant) -> Result<CertificateRenderFileGrant, AppError> {
    match grant {
        DownloadGrant::Redirect {
            location,
            expires_at,
        } => Ok(CertificateRenderFileGrant {
            file_id,
            url: location,
            expires_at,
        }),
        DownloadGrant::Stream { .. } => Err(AppError::ServiceUnavailable(
            "certificate_render_grant_not_supported".to_string(),
        )),
    }
}

fn thai_date(date: NaiveDate) -> String {
    const MONTHS: [&str; 12] = [
        "มกราคม",
        "กุมภาพันธ์",
        "มีนาคม",
        "เมษายน",
        "พฤษภาคม",
        "มิถุนายน",
        "กรกฎาคม",
        "สิงหาคม",
        "กันยายน",
        "ตุลาคม",
        "พฤศจิกายน",
        "ธันวาคม",
    ];
    format!(
        "{} {} {}",
        date.day(),
        MONTHS[date.month0() as usize],
        date.year() + 543
    )
}

fn filename_part(value: &str) -> String {
    let value = value
        .chars()
        .take(100)
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value.trim().replace(' ', "-");
    if value.is_empty() {
        "เกียรติบัตร".to_string()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_samples_dates_and_filenames_are_deterministic() {
        let short = sample_recipient_values(CertificatePreviewKind::Short);
        let normal = sample_recipient_values(CertificatePreviewKind::Normal);
        let long = sample_recipient_values(CertificatePreviewKind::Long);

        assert_eq!(short["ชื่อ"], "ปัน");
        assert_eq!(normal["ชื่อ"], "กมลชนก");
        assert!(long["นามสกุล"].chars().count() > normal["นามสกุล"].chars().count());
        assert_eq!(
            thai_date(NaiveDate::from_ymd_opt(2026, 8, 14).unwrap()),
            "14 สิงหาคม 2569"
        );
        assert_eq!(filename_part("แบบ / นักเรียน"), "แบบ---นักเรียน");
    }
}
