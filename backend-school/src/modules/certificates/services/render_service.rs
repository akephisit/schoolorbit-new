use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, NaiveDate, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::{
        certificates::models::{
            CertificateBuiltInFont, CertificateElement, CertificateFontSource,
            CertificatePreviewKind, CertificatePreviewManifestRequest,
            CertificateRenderCampaignValues, CertificateRenderFileGrant,
            CertificateRenderFontGrant, CertificateRenderImageGrant, CertificateRenderManifest,
            CertificateTemplateAssetKind, PageGeometry,
        },
        files::{
            consumer_service::map_platform_error, platform_service::FilePlatform,
            platform_types::DownloadGrant, repository::SqlFileRepository,
        },
    },
    scheduling::SCHOOL_TIMEZONE,
};

use super::{
    candidate_service,
    import_validation::{normalize_display_text, normalize_name_for_match},
    layout::validate_layout,
    template_service,
};

#[derive(Debug, FromRow)]
struct CampaignRenderRow {
    academic_year_name: String,
    campaign_name: String,
    event_date: NaiveDate,
    owner_organization_unit_name: Option<String>,
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
