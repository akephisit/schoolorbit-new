use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use bytes::Bytes;
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::certificates::{
        models::{
            AttachCertificateAssetRequest, AttachCertificateBackgroundRequest,
            CandidateMatchStatus, CandidateNameSource, CandidateValidationCode,
            CandidateValidationStatus, CertificateAccountSearchQuery, CertificateCampaignListQuery,
            CertificateCampaignStatus, CertificateCandidateBulkRequest,
            CertificateCandidateListQuery, CertificateElement, CertificateFontSource,
            CertificateImportRequest, CertificateImportRowInput, CertificateImportSource,
            CertificateLayoutV1, CertificatePreviewKind, CertificatePreviewManifestRequest,
            CertificateTemplateAssetKind, CertificateTemplateDeleteDisposition,
            ChangeCertificateCampaignStatusRequest, CreateAccountCertificateCandidateRequest,
            CreateCertificateCampaignRequest, CreateCertificateTemplateRequest,
            CreateManualExternalCandidateRequest, ElementFrame, GeometryAction, NullableUuidUpdate,
            RecipientType, TextAlignment, TextElement, UpdateCertificateCampaignRequest,
            UpdateCertificateCandidateRequest, UpdateCertificateTemplateRequest,
        },
        services::{campaign_service, candidate_service, render_service, template_service},
    },
    permissions::registry::codes,
    policies::{
        certificate_access_policy::{require_owner_action, CertificateAction},
        file_access_policy::{self, FilePolicyAction},
        resource_access_policy::accessible_exact_units_for_permission,
    },
    test_helpers::{create_named_test_pool, create_test_user, run_test_migrations},
};

use chrono::NaiveDate;

struct PreviewStorage;

#[async_trait]
impl crate::modules::files::storage_provider::StorageProvider for PreviewStorage {
    async fn check_readiness(
        &self,
    ) -> Result<(), crate::modules::files::storage_provider::StorageError> {
        Ok(())
    }

    async fn put(
        &self,
        _object: &crate::modules::files::storage_provider::StoredObject,
        _body: Bytes,
    ) -> Result<(), crate::modules::files::storage_provider::StorageError> {
        Ok(())
    }

    async fn get(
        &self,
        _object: &crate::modules::files::storage_provider::StoredObject,
        _max_bytes: u64,
    ) -> Result<Bytes, crate::modules::files::storage_provider::StorageError> {
        Ok(Bytes::new())
    }

    async fn head(
        &self,
        _object: &crate::modules::files::storage_provider::StoredObject,
    ) -> Result<
        Option<crate::modules::files::storage_provider::ObjectMetadata>,
        crate::modules::files::storage_provider::StorageError,
    > {
        Ok(None)
    }

    async fn delete(
        &self,
        _object: &crate::modules::files::storage_provider::StoredObject,
    ) -> Result<(), crate::modules::files::storage_provider::StorageError> {
        Ok(())
    }

    async fn private_download_grant(
        &self,
        _object: &crate::modules::files::storage_provider::StoredObject,
        filename: &str,
        ttl: Duration,
    ) -> Result<
        crate::modules::files::platform_types::DownloadGrant,
        crate::modules::files::storage_provider::StorageError,
    > {
        Ok(
            crate::modules::files::platform_types::DownloadGrant::Redirect {
                location: format!("https://private.example.test/{filename}"),
                expires_at: chrono::Utc::now()
                    + chrono::Duration::from_std(ttl).expect("test TTL should fit chrono"),
            },
        )
    }

    fn public_location(
        &self,
        _object: &crate::modules::files::storage_provider::StoredObject,
    ) -> Result<Url, crate::modules::files::storage_provider::StorageError> {
        Url::parse("https://public.example.test/file")
            .map_err(|_| crate::modules::files::storage_provider::StorageError::OperationFailed)
    }
}

struct PreviewScanner;

#[async_trait]
impl crate::modules::files::malware_scanner::MalwareScanner for PreviewScanner {
    async fn scan(&self, _content: &[u8]) -> crate::modules::files::malware_scanner::ScanOutcome {
        crate::modules::files::malware_scanner::ScanOutcome::Clean
    }
}

struct CertificatePolicyFixture {
    pool: PgPool,
    actor: ActorContext,
    unit_a: Uuid,
    unit_b: Uuid,
}

#[tokio::test]
async fn certificate_template_file_policy_requires_exact_persisted_relationships() {
    use crate::modules::files::{
        platform_types::{FileLifecycleStatus, FilePurpose, FileVisibility},
        repository::PlatformFile,
    };

    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_file_policy", 3117).await;
    let template = create_template_fixture(&pool, &actor, academic_year_id).await;
    assert_eq!(
        file_access_policy::authorize_create(
            &pool,
            &actor,
            FilePurpose::CertificateTemplateBackground,
            Some(template.id),
        )
        .await
        .unwrap(),
        actor.user_id
    );
    assert!(matches!(
        file_access_policy::authorize_create(
            &pool,
            &actor,
            FilePurpose::CertificateTemplateBackground,
            None,
        )
        .await,
        Err(AppError::BadRequest(_))
    ));

    let file_id = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(841.89, 595.28, 0),
    )
    .await;
    let platform_file = |visibility| PlatformFile {
        id: file_id,
        owner_user_id: Some(actor.user_id),
        purpose: FilePurpose::CertificateTemplateBackground,
        visibility,
        lifecycle_status: FileLifecycleStatus::Ready,
        current_version: Some(1),
        display_filename: "background.pdf".to_string(),
        detected_mime_type: "application/pdf".to_string(),
        byte_size: 10,
    };
    let read_only = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::CERTIFICATE_READ_SCHOOL.to_string()],
    };
    assert!(file_access_policy::authorize_existing(
        &pool,
        &read_only,
        &platform_file(FileVisibility::Private),
        FilePolicyAction::Read,
        Some(template.id),
    )
    .await
    .is_err());
    assert!(file_access_policy::authorize_existing(
        &pool,
        &actor,
        &platform_file(FileVisibility::Private),
        FilePolicyAction::Delete,
        Some(template.id),
    )
    .await
    .is_ok());
    assert!(matches!(
        file_access_policy::authorize_existing(
            &pool,
            &actor,
            &platform_file(FileVisibility::Private),
            FilePolicyAction::Read,
            Some(Uuid::new_v4()),
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        file_access_policy::authorize_existing(
            &pool,
            &actor,
            &platform_file(FileVisibility::Public),
            FilePolicyAction::Read,
            Some(template.id),
        )
        .await,
        Err(AppError::Forbidden(_))
    ));

    let unmapped = PlatformFile {
        id: Uuid::new_v4(),
        ..platform_file(FileVisibility::Private)
    };
    assert!(matches!(
        file_access_policy::authorize_existing(
            &pool,
            &actor,
            &unmapped,
            FilePolicyAction::Read,
            Some(template.id),
        )
        .await,
        Err(AppError::NotFound(_))
    ));

    template_service::attach_background(
        &pool,
        &actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap();
    assert!(file_access_policy::authorize_existing(
        &pool,
        &read_only,
        &platform_file(FileVisibility::Private),
        FilePolicyAction::Read,
        Some(template.id),
    )
    .await
    .is_ok());
    assert!(matches!(
        file_access_policy::authorize_existing(
            &pool,
            &actor,
            &platform_file(FileVisibility::Private),
            FilePolicyAction::Delete,
            Some(template.id),
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn certificate_template_file_delete_guard_serializes_with_attachment() {
    use crate::modules::files::{
        platform_types::{FileLifecycleStatus, FilePurpose, FileVisibility},
        repository::{PlatformFile, SqlFileRepository},
    };

    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_file_delete_race", 3120).await;
    let template = create_template_fixture_with_types(
        &pool,
        &actor,
        academic_year_id,
        "กิจกรรมทดสอบลบไฟล์พร้อมแนบ",
        "แบบทดสอบลบไฟล์พร้อมแนบ",
        vec![RecipientType::External],
    )
    .await;
    let file_id = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(841.89, 595.28, 0),
    )
    .await;
    let file = PlatformFile {
        id: file_id,
        owner_user_id: Some(actor.user_id),
        purpose: FilePurpose::CertificateTemplateBackground,
        visibility: FileVisibility::Private,
        lifecycle_status: FileLifecycleStatus::Ready,
        current_version: Some(1),
        display_filename: "background.pdf".to_string(),
        detected_mime_type: "application/pdf".to_string(),
        byte_size: 10,
    };
    let mut guard = file_access_policy::authorize_certificate_template_delete_guard(
        &pool,
        &actor,
        &file,
        Some(template.id),
    )
    .await
    .unwrap();

    let attach_pool = pool.clone();
    let attach_actor = actor.clone();
    let attach = tokio::spawn(async move {
        template_service::attach_background(
            &attach_pool,
            &attach_actor,
            template.id,
            AttachCertificateBackgroundRequest {
                file_id,
                geometry_action: GeometryAction::Preserve,
                preview_confirmed: true,
            },
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !attach.is_finished(),
        "attachment should wait while file deletion holds the template lock"
    );

    let repository = SqlFileRepository::new(pool.clone());
    let delete_work = repository
        .request_delete_in_transaction(&mut guard, file_id)
        .await
        .unwrap();
    assert_eq!(delete_work.len(), 1);
    guard.commit().await.unwrap();

    assert!(matches!(attach.await.unwrap(), Err(AppError::Conflict(_))));
    let background: Option<Uuid> =
        sqlx::query_scalar("SELECT background_file_id FROM certificate_templates WHERE id = $1")
            .bind(template.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(background, None);
}

async fn create_template_fixture(
    pool: &PgPool,
    actor: &ActorContext,
    academic_year_id: Uuid,
) -> crate::modules::certificates::models::CertificateTemplateDetail {
    let campaign = campaign_service::create_campaign(
        pool,
        actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมทดสอบแม่แบบ"),
    )
    .await
    .unwrap();
    template_service::create_template(
        pool,
        actor,
        campaign.id,
        CreateCertificateTemplateRequest {
            name: "แบบสำหรับนักเรียน".to_string(),
            allowed_recipient_types: vec![RecipientType::Student],
        },
    )
    .await
    .unwrap()
}

async fn create_template_fixture_with_types(
    pool: &PgPool,
    actor: &ActorContext,
    academic_year_id: Uuid,
    campaign_name: &str,
    template_name: &str,
    allowed_recipient_types: Vec<RecipientType>,
) -> crate::modules::certificates::models::CertificateTemplateDetail {
    let campaign = campaign_service::create_campaign(
        pool,
        actor,
        campaign_create_payload(academic_year_id, None, campaign_name),
    )
    .await
    .unwrap();
    template_service::create_template(
        pool,
        actor,
        campaign.id,
        CreateCertificateTemplateRequest {
            name: template_name.to_string(),
            allowed_recipient_types,
        },
    )
    .await
    .unwrap()
}

fn pdf_inspection(width: f64, height: f64, rotation: i16) -> serde_json::Value {
    serde_json::json!({
        "kind": "pdf",
        "page_count": 1,
        "crop_box": {"x": 0.0, "y": 0.0, "width": width, "height": height},
        "media_box": {"x": 0.0, "y": 0.0, "width": width, "height": height},
        "rotation": rotation
    })
}

fn text_layout(font_source: CertificateFontSource) -> CertificateLayoutV1 {
    CertificateLayoutV1 {
        schema_version: 1,
        elements: vec![CertificateElement::Text(TextElement {
            id: Uuid::new_v4(),
            content: "มอบให้ {ชื่อ} {นามสกุล}".to_string(),
            frame: ElementFrame {
                x: 20.0,
                y: 30.0,
                width: 200.0,
                height: 50.0,
            },
            rotation: 0.0,
            font_source,
            font_family: "Sarabun".to_string(),
            font_weight: 400,
            font_size: 24.0,
            min_font_size: 12.0,
            color: "#112233".to_string(),
            alignment: TextAlignment::Center,
            line_height: 1.2,
            auto_shrink: true,
            shadow: None,
        })],
    }
}

async fn insert_ready_template_file(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    purpose: &str,
    inspection_metadata: serde_json::Value,
) -> Uuid {
    let (purpose_segment, detected_mime_type, extension) = match purpose {
        "certificate_template_background" => ("template-background", "application/pdf", "pdf"),
        "certificate_template_image" => ("template-image", "image/jpeg", "jpg"),
        "certificate_template_font" => ("template-font", "font/ttf", "ttf"),
        _ => panic!("unsupported certificate test purpose"),
    };
    let file_id: Uuid = sqlx::query_scalar(
        "INSERT INTO files (
            owner_user_id, display_filename, created_by, purpose_code, visibility,
            lifecycle_status, retention_class, expires_at, inspection_metadata
         ) VALUES ($1, 'certificate-test.bin', $1, $2, 'private', 'processing',
                   'temporary', now() + interval '1 hour', $3)
         RETURNING id",
    )
    .bind(actor.user_id)
    .bind(purpose)
    .bind(inspection_metadata)
    .fetch_one(pool)
    .await
    .unwrap();
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO file_versions (
            file_id, version_number, provider_code, storage_class, storage_status,
            object_key, detected_mime_type, canonical_extension, byte_size, checksum,
            scan_status, scanned_at, created_by
         ) VALUES ($1, 1, 'test', 'private', 'stored', $2, $3,
                   $4, 10, repeat('a', 64), 'clean', now(), $5)
         RETURNING id",
    )
    .bind(file_id)
    .bind(format!(
        "tenants/{}/certificate/{purpose_segment}/{file_id}/v1/original.{extension}",
        Uuid::nil()
    ))
    .bind(detected_mime_type)
    .bind(extension)
    .bind(actor.user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE files SET current_version_id = $2, lifecycle_status = 'ready' WHERE id = $1",
    )
    .bind(file_id)
    .bind(version_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO certificate_template_file_uploads
            (file_id, template_id, purpose_code, uploaded_by)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(purpose)
    .bind(actor.user_id)
    .execute(pool)
    .await
    .unwrap();
    file_id
}

#[tokio::test]
async fn background_geometry_comes_from_the_ready_file_not_the_request() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_geometry", 3110).await;
    let template = create_template_fixture(&pool, &actor, academic_year_id).await;
    let file_id = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        serde_json::json!({
            "kind": "pdf",
            "page_count": 1,
            "crop_box": {"x": 0.0, "y": 0.0, "width": 841.89, "height": 595.28},
            "media_box": {"x": 0.0, "y": 0.0, "width": 841.89, "height": 595.28},
            "rotation": 0
        }),
    )
    .await;

    let updated = template_service::attach_background(
        &pool,
        &actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        updated
            .template
            .page_geometry
            .unwrap()
            .crop_box
            .width_points,
        841.89
    );
}

#[tokio::test]
async fn preview_manifest_uses_private_short_lived_grants_and_never_audits_them() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_preview_manifest", 3118).await;
    let template = create_template_fixture(&pool, &actor, academic_year_id).await;
    let file_id = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(841.89, 595.28, 0),
    )
    .await;
    template_service::attach_background(
        &pool,
        &actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap();
    let font_file_id = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_font",
        serde_json::json!({
            "kind": "font",
            "family_name": "Preview Thai Font",
            "units_per_em": 1000
        }),
    )
    .await;
    let with_font = template_service::attach_asset(
        &pool,
        &actor,
        template.id,
        AttachCertificateAssetRequest {
            file_id: font_file_id,
            kind: CertificateTemplateAssetKind::Font,
            display_name: "ฟอนต์พรีวิว".to_string(),
            font_weight: Some(400),
            rights_confirmed: true,
        },
    )
    .await
    .unwrap();
    let font_asset = with_font.assets[0].clone();
    let mut local_layout = text_layout(CertificateFontSource::Asset {
        asset_id: font_asset.id,
    });
    let CertificateElement::Text(local_text) = &mut local_layout.elements[0] else {
        panic!("expected text element")
    };
    local_text.font_family = font_asset.font_family.clone().unwrap();
    local_text.font_weight = font_asset.font_weight.unwrap();
    let audit_count_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE entity_type = 'certificate_template'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let platform = crate::modules::files::platform_service::FilePlatform::new(
        Arc::new(PreviewStorage),
        Arc::new(PreviewScanner),
    );
    let requested_at = chrono::Utc::now();
    let manifest = render_service::preview_manifest(
        &pool,
        &actor,
        &platform,
        "โรงเรียนตัวอย่าง".to_string(),
        template.id,
        CertificatePreviewManifestRequest {
            preview_kind: CertificatePreviewKind::Long,
            candidate_id: None,
            sample_values: Default::default(),
            layout: Some(local_layout.clone()),
        },
    )
    .await
    .unwrap();

    assert_eq!(manifest.certificate_number, "ตัวอย่าง");
    assert!(manifest.qr_payload.contains("ตัวอย่าง"));
    assert_eq!(manifest.recipient_values["ชื่อโรงเรียนผู้ออก"], "โรงเรียนตัวอย่าง");
    assert!(manifest
        .background_grant
        .url
        .starts_with("https://private.example.test/"));
    assert!(manifest.background_grant.expires_at > requested_at);
    assert!(manifest.background_grant.expires_at <= requested_at + chrono::Duration::minutes(5));
    assert_eq!(manifest.built_in_fonts.len(), 2);
    assert_eq!(manifest.layout, local_layout);
    assert_eq!(manifest.font_grants.len(), 1);
    assert_eq!(manifest.font_grants[0].asset_id, font_asset.id);
    let audit_count_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE entity_type = 'certificate_template'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count_after, audit_count_before);
}

#[tokio::test]
async fn background_rejects_wrong_relation_unready_pages_and_unsafe_geometry() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_invalid_background", 3111).await;
    let template = create_template_fixture(&pool, &actor, academic_year_id).await;

    let wrong_purpose = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_image",
        serde_json::json!({"kind": "image", "width_px": 100, "height_px": 100}),
    )
    .await;
    assert!(matches!(
        template_service::attach_background(
            &pool,
            &actor,
            template.id,
            AttachCertificateBackgroundRequest {
                file_id: wrong_purpose,
                geometry_action: GeometryAction::Preserve,
                preview_confirmed: true,
            },
        )
        .await,
        Err(AppError::Forbidden(_))
    ));

    let not_ready = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(841.89, 595.28, 0),
    )
    .await;
    sqlx::query("UPDATE files SET lifecycle_status = 'processing' WHERE id = $1")
        .bind(not_ready)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        template_service::attach_background(
            &pool,
            &actor,
            template.id,
            AttachCertificateBackgroundRequest {
                file_id: not_ready,
                geometry_action: GeometryAction::Preserve,
                preview_confirmed: true,
            },
        )
        .await,
        Err(AppError::Conflict(_))
    ));

    let multiple_pages = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        serde_json::json!({
            "kind": "pdf",
            "page_count": 2,
            "crop_box": {"x": 0.0, "y": 0.0, "width": 841.89, "height": 595.28},
            "media_box": {"x": 0.0, "y": 0.0, "width": 841.89, "height": 595.28},
            "rotation": 0
        }),
    )
    .await;
    assert!(matches!(
        template_service::attach_background(
            &pool,
            &actor,
            template.id,
            AttachCertificateBackgroundRequest {
                file_id: multiple_pages,
                geometry_action: GeometryAction::Preserve,
                preview_confirmed: true,
            },
        )
        .await,
        Err(AppError::ValidationError(_))
    ));

    let points_per_mm = 72.0 / 25.4;
    for (index, geometry) in [
        pdf_inspection(24.9 * points_per_mm, 100.0 * points_per_mm, 0),
        pdf_inspection(600.1 * points_per_mm, 100.0 * points_per_mm, 0),
        pdf_inspection(500.0 * points_per_mm, 501.0 * points_per_mm, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let file_id = insert_ready_template_file(
            &pool,
            &actor,
            template.id,
            "certificate_template_background",
            geometry,
        )
        .await;
        let result = template_service::attach_background(
            &pool,
            &actor,
            template.id,
            AttachCertificateBackgroundRequest {
                file_id,
                geometry_action: GeometryAction::Preserve,
                preview_confirmed: true,
            },
        )
        .await;
        assert!(
            matches!(result, Err(AppError::ValidationError(_))),
            "unsafe geometry case {index} must be rejected"
        );
    }
}

#[tokio::test]
async fn template_names_recipient_compatibility_and_cross_template_files_are_enforced() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_identity", 3112).await;
    let first = create_template_fixture_with_types(
        &pool,
        &actor,
        academic_year_id,
        "กิจกรรมชื่อแบบ",
        " แบบ  รางวัล ",
        vec![RecipientType::External],
    )
    .await;
    let duplicate = template_service::create_template(
        &pool,
        &actor,
        first.campaign_id,
        CreateCertificateTemplateRequest {
            name: "แบบ รางวัล".to_string(),
            allowed_recipient_types: vec![RecipientType::Student],
        },
    )
    .await;
    assert!(matches!(duplicate, Err(AppError::Conflict(_))));

    sqlx::query(
        "INSERT INTO certificate_candidates (
            campaign_id, template_id, recipient_type, imported_first_name,
            imported_last_name, selected_name_source, match_status, validation_status
         ) VALUES ($1, $2, 'external', 'สมชาย', 'ทดสอบ', 'file',
                   'external_confirmed', 'ready')",
    )
    .bind(first.campaign_id)
    .bind(first.id)
    .execute(&pool)
    .await
    .unwrap();
    let incompatible = template_service::update_template(
        &pool,
        &actor,
        first.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: first.updated_at,
            name: None,
            allowed_recipient_types: Some(vec![RecipientType::Student]),
            safe_margin_points: None,
            show_safe_area: None,
            layout: None,
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await;
    assert!(matches!(incompatible, Err(AppError::Conflict(_))));

    let sibling = template_service::create_template(
        &pool,
        &actor,
        first.campaign_id,
        CreateCertificateTemplateRequest {
            name: "แบบเข้าร่วมกิจกรรม".to_string(),
            allowed_recipient_types: vec![RecipientType::External],
        },
    )
    .await
    .unwrap();
    let listed = template_service::list_templates(&pool, &actor, first.campaign_id)
        .await
        .unwrap();
    assert_eq!(
        listed
            .iter()
            .map(|template| template.id)
            .collect::<Vec<_>>(),
        vec![first.id, sibling.id]
    );

    let second = create_template_fixture_with_types(
        &pool,
        &actor,
        academic_year_id,
        "อีกกิจกรรมหนึ่ง",
        "แบบที่สอง",
        vec![RecipientType::Student],
    )
    .await;
    let first_file = insert_ready_template_file(
        &pool,
        &actor,
        first.id,
        "certificate_template_background",
        pdf_inspection(841.89, 595.28, 0),
    )
    .await;
    let cross_template = template_service::attach_background(
        &pool,
        &actor,
        second.id,
        AttachCertificateBackgroundRequest {
            file_id: first_file,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await;
    assert!(matches!(cross_template, Err(AppError::Forbidden(_))));
}

#[tokio::test]
async fn background_preserve_scale_and_reset_are_deterministic() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_geometry_actions", 3113).await;
    let template = create_template_fixture(&pool, &actor, academic_year_id).await;
    let initial_file = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(400.0, 300.0, 0),
    )
    .await;
    let attached = template_service::attach_background(
        &pool,
        &actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id: initial_file,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap()
    .template;
    let designed = template_service::update_template(
        &pool,
        &actor,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: attached.updated_at,
            name: None,
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: Some(text_layout(CertificateFontSource::BuiltIn)),
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await
    .unwrap()
    .template;

    let same_file = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(400.0, 300.0, 0),
    )
    .await;
    let preserved = template_service::attach_background(
        &pool,
        &actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id: same_file,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: false,
        },
    )
    .await
    .unwrap()
    .template;
    assert_eq!(preserved.layout, designed.layout);

    let changed_file = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(800.0, 600.0, 0),
    )
    .await;
    assert!(matches!(
        template_service::attach_background(
            &pool,
            &actor,
            template.id,
            AttachCertificateBackgroundRequest {
                file_id: changed_file,
                geometry_action: GeometryAction::Preserve,
                preview_confirmed: true,
            },
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    let scaled = template_service::attach_background(
        &pool,
        &actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id: changed_file,
            geometry_action: GeometryAction::Scale,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap()
    .template;
    let CertificateElement::Text(text) = &scaled.layout.elements[0] else {
        panic!("expected text element")
    };
    assert_eq!(text.frame.x, 40.0);
    assert_eq!(text.frame.y, 60.0);
    assert_eq!(text.font_size, 48.0);

    let reset_file = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(600.0, 400.0, 90),
    )
    .await;
    let reset = template_service::attach_background(
        &pool,
        &actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id: reset_file,
            geometry_action: GeometryAction::Reset,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap()
    .template;
    assert_eq!(reset.layout, CertificateLayoutV1::default());
}

#[tokio::test]
async fn active_request_locks_referenced_template_mutations() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_request_lock", 3114).await;
    let selected = create_template_fixture_with_types(
        &pool,
        &actor,
        academic_year_id,
        "กิจกรรมที่ส่งตรวจ",
        "แบบที่เลือก",
        vec![RecipientType::External],
    )
    .await;
    let unselected = template_service::create_template(
        &pool,
        &actor,
        selected.campaign_id,
        CreateCertificateTemplateRequest {
            name: "แบบที่ไม่ได้เลือก".to_string(),
            allowed_recipient_types: vec![RecipientType::External],
        },
    )
    .await
    .unwrap();
    let candidate_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_candidates (
            campaign_id, template_id, recipient_type, imported_first_name,
            imported_last_name, selected_name_source, match_status, validation_status
         ) VALUES ($1, $2, 'external', 'ผู้รับ', 'ที่เลือก', 'file',
                   'external_confirmed', 'ready')
         RETURNING id",
    )
    .bind(selected.campaign_id)
    .bind(selected.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let request_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_issue_requests (campaign_id, submitted_by)
         VALUES ($1, $2) RETURNING id",
    )
    .bind(selected.campaign_id)
    .bind(actor.user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO certificate_issue_request_items (request_id, candidate_id, campaign_id)
         VALUES ($1, $2, $3)",
    )
    .bind(request_id)
    .bind(candidate_id)
    .bind(selected.campaign_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO certificate_candidate_issue_locks (candidate_id, request_id)
         VALUES ($1, $2)",
    )
    .bind(candidate_id)
    .bind(request_id)
    .execute(&pool)
    .await
    .unwrap();

    let locked = template_service::update_template(
        &pool,
        &actor,
        selected.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: selected.updated_at,
            name: Some("ห้ามแก้ระหว่างตรวจ".to_string()),
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: None,
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await;
    assert!(matches!(locked, Err(AppError::Conflict(_))));
    assert!(matches!(
        template_service::delete_template(&pool, &actor, selected.id).await,
        Err(AppError::Conflict(_))
    ));

    let updated = template_service::update_template(
        &pool,
        &actor,
        unselected.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: unselected.updated_at,
            name: Some("แก้แบบที่ไม่ได้อยู่ในคำขอได้".to_string()),
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: None,
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.template.name, "แก้แบบที่ไม่ได้อยู่ในคำขอได้");
}

#[tokio::test]
async fn font_rights_and_referenced_asset_deletion_are_enforced() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_font_rights", 3115).await;
    let template = create_template_fixture(&pool, &actor, academic_year_id).await;
    let background_id = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(841.89, 595.28, 0),
    )
    .await;
    let with_background = template_service::attach_background(
        &pool,
        &actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id: background_id,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap()
    .template;
    let font_file_id = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_font",
        serde_json::json!({
            "kind": "font",
            "family_name": "Test Thai Font",
            "units_per_em": 1000
        }),
    )
    .await;
    let unconfirmed = template_service::attach_asset(
        &pool,
        &actor,
        template.id,
        AttachCertificateAssetRequest {
            file_id: font_file_id,
            kind: CertificateTemplateAssetKind::Font,
            display_name: "ฟอนต์ทดสอบ".to_string(),
            font_weight: Some(400),
            rights_confirmed: false,
        },
    )
    .await;
    assert!(matches!(unconfirmed, Err(AppError::ValidationError(_))));

    let with_font = template_service::attach_asset(
        &pool,
        &actor,
        template.id,
        AttachCertificateAssetRequest {
            file_id: font_file_id,
            kind: CertificateTemplateAssetKind::Font,
            display_name: "ฟอนต์ทดสอบ".to_string(),
            font_weight: Some(400),
            rights_confirmed: true,
        },
    )
    .await
    .unwrap();
    let asset = with_font.assets[0].clone();
    let mut layout = text_layout(CertificateFontSource::Asset { asset_id: asset.id });
    let CertificateElement::Text(text) = &mut layout.elements[0] else {
        panic!("expected text element")
    };
    text.font_family = asset.font_family.clone().unwrap();
    text.font_weight = asset.font_weight.unwrap();
    let designed = template_service::update_template(
        &pool,
        &actor,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: with_font.updated_at,
            name: None,
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: Some(layout),
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await
    .unwrap()
    .template;
    assert!(matches!(
        template_service::delete_asset(&pool, &actor, template.id, asset.id).await,
        Err(AppError::Conflict(_))
    ));

    let without_reference = template_service::update_template(
        &pool,
        &actor,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: designed.updated_at,
            name: None,
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: Some(text_layout(CertificateFontSource::BuiltIn)),
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await
    .unwrap()
    .template;
    let deleted = template_service::delete_asset(&pool, &actor, template.id, asset.id)
        .await
        .unwrap();
    assert_eq!(deleted.detached_file_ids, vec![font_file_id]);
    assert!(deleted.template.assets.is_empty());
    assert!(deleted.template.updated_at > without_reference.updated_at);
    assert!(with_background.background_file_id.is_some());
}

async fn insert_issued_certificate_for_template(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    campaign_id: Uuid,
    academic_year_id: Uuid,
    academic_year_value: i32,
) {
    sqlx::query(
        "UPDATE certificate_campaigns
         SET status = 'active', activity_sequence = 1
         WHERE id = $1",
    )
    .bind(campaign_id)
    .execute(pool)
    .await
    .unwrap();
    let candidate_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_candidates (
            campaign_id, template_id, recipient_type, imported_first_name,
            imported_last_name, selected_name_source, match_status, validation_status
         ) VALUES ($1, $2, 'external', 'ผู้รับ', 'ใบเดิม', 'file',
                   'external_confirmed', 'ready')
         RETURNING id",
    )
    .bind(campaign_id)
    .bind(template_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let request_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_issue_requests (
            campaign_id, status, submitted_by, reviewed_by, reviewed_at, issued_at
         ) VALUES ($1, 'issued', $2, $2, now(), now())
         RETURNING id",
    )
    .bind(campaign_id)
    .bind(actor.user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO certificate_issue_request_items (request_id, candidate_id, campaign_id)
         VALUES ($1, $2, $3)",
    )
    .bind(request_id)
    .bind(candidate_id)
    .bind(campaign_id)
    .execute(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_issue_runs (
            request_id, idempotency_key, issued_by, outcome, issued_count,
            first_certificate_sequence, last_certificate_sequence
         ) VALUES ($1, $2, $3, 'issued', 1, 1, 1)
         RETURNING id",
    )
    .bind(request_id)
    .bind(Uuid::new_v4())
    .bind(actor.user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let certificate_number = format!("{academic_year_value:04}-0001-000001-0");
    let certificate_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificates (
            campaign_id, template_id, candidate_id, issue_run_id, academic_year_id,
            academic_year_value, activity_sequence, certificate_sequence, check_digit,
            certificate_number, recipient_type, first_name_snapshot, last_name_snapshot,
            template_name_snapshot, custom_values_snapshot, school_name_snapshot,
            issue_date, qr_proof_encrypted, qr_proof_hash
         ) VALUES ($1, $2, $3, $4, $5, $6, 1, 1, 0, $7, 'external',
                   'ผู้รับ', 'ใบเดิม', 'แบบที่ใช้แล้ว', '{}'::jsonb, 'โรงเรียนทดสอบ',
                   CURRENT_DATE, 'encrypted-test-proof', repeat('c', 64))
         RETURNING id",
    )
    .bind(campaign_id)
    .bind(template_id)
    .bind(candidate_id)
    .bind(run_id)
    .bind(academic_year_id)
    .bind(academic_year_value)
    .bind(certificate_number)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE certificate_candidates
         SET issued_certificate_id = $2
         WHERE id = $1",
    )
    .bind(candidate_id)
    .bind(certificate_id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn used_template_updates_report_missing_values_and_delete_deactivates() {
    let year = 3116;
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_used", year).await;
    let template = create_template_fixture_with_types(
        &pool,
        &actor,
        academic_year_id,
        "กิจกรรมที่ออกใบแล้ว",
        "แบบที่ใช้แล้ว",
        vec![RecipientType::External],
    )
    .await;
    let background_id = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(841.89, 595.28, 0),
    )
    .await;
    let current = template_service::attach_background(
        &pool,
        &actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id: background_id,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap()
    .template;
    insert_issued_certificate_for_template(
        &pool,
        &actor,
        template.id,
        template.campaign_id,
        academic_year_id,
        year,
    )
    .await;

    let mut missing_standard_layout = text_layout(CertificateFontSource::BuiltIn);
    let CertificateElement::Text(text) = &mut missing_standard_layout.elements[0] else {
        panic!("expected text element")
    };
    text.content = "ได้รับ {รางวัลหรือบทบาท}".to_string();
    let missing_standard = template_service::update_template(
        &pool,
        &actor,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: current.updated_at,
            name: None,
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: Some(missing_standard_layout),
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await;
    assert!(matches!(missing_standard, Err(AppError::Conflict(_))));

    sqlx::query(
        "INSERT INTO certificate_candidates (
            campaign_id, template_id, recipient_type, imported_first_name,
            imported_last_name, selected_name_source, custom_values,
            match_status, validation_status
         ) VALUES ($1, $2, 'external', 'ผู้รับ', 'รายใหม่', 'file',
                   '{\"ครูผู้ควบคุม\":\"ครูตัวอย่าง\"}'::jsonb,
                   'external_confirmed', 'ready')",
    )
    .bind(template.campaign_id)
    .bind(template.id)
    .execute(&pool)
    .await
    .unwrap();
    let mut custom_layout = text_layout(CertificateFontSource::BuiltIn);
    let CertificateElement::Text(text) = &mut custom_layout.elements[0] else {
        panic!("expected text element")
    };
    text.content = "มอบให้ {ชื่อ} โดย {ครูผู้ควบคุม}".to_string();
    let unconfirmed = template_service::update_template(
        &pool,
        &actor,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: current.updated_at,
            name: None,
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: Some(custom_layout.clone()),
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await;
    assert!(matches!(unconfirmed, Err(AppError::Conflict(_))));
    let confirmed = template_service::update_template(
        &pool,
        &actor,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: current.updated_at,
            name: None,
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: Some(custom_layout.clone()),
            is_active: None,
            confirm_missing_issued_values: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(confirmed.template.missing_variable_certificate_count, 1);

    let CertificateElement::Text(text) = &mut custom_layout.elements[0] else {
        panic!("expected text element")
    };
    text.frame.x += 1.0;
    let same_variables = template_service::update_template(
        &pool,
        &actor,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: confirmed.template.updated_at,
            name: None,
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: Some(custom_layout),
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await
    .expect("moving an existing variable must not require another missing-value confirmation");
    assert_eq!(
        same_variables.template.missing_variable_certificate_count,
        1
    );

    let deleted = template_service::delete_template(&pool, &actor, template.id)
        .await
        .unwrap();
    assert_eq!(
        deleted.result.disposition,
        CertificateTemplateDeleteDisposition::Deactivated
    );
    assert!(deleted.detached_file_ids.is_empty());
    let persisted = template_service::get_template(&pool, &actor, template.id)
        .await
        .unwrap();
    assert!(!persisted.is_active);
}

impl CertificatePolicyFixture {
    async fn new(test_name: &str) -> Self {
        let pool = create_named_test_pool(test_name).await;
        run_test_migrations(&pool).await;
        let actor_user_id = create_test_user(
            &pool,
            &format!("{test_name}-actor@example.invalid"),
            "test-password",
        )
        .await
        .expect("policy actor should insert");
        let unit_a = insert_unit(&pool, &format!("{test_name}_a"), None).await;
        let unit_b = insert_unit(&pool, &format!("{test_name}_b"), None).await;
        for unit_id in [unit_a, unit_b] {
            sqlx::query(
                "INSERT INTO organization_members
                    (user_id, organization_unit_id, position_code, started_at)
                 VALUES ($1, $2, 'head', CURRENT_DATE)",
            )
            .bind(actor_user_id)
            .bind(unit_id)
            .execute(&pool)
            .await
            .expect("active organization membership should insert");
        }
        Self {
            pool,
            actor: ActorContext {
                user_id: actor_user_id,
                permissions: vec![codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT.to_string()],
            },
            unit_a,
            unit_b,
        }
    }

    async fn with_position_grant(test_name: &str, position: &str) -> Self {
        let fixture = Self::new(test_name).await;
        fixture.add_position_grant(fixture.unit_a, position).await;
        fixture
    }

    async fn permission_id(&self) -> Uuid {
        sqlx::query_scalar("SELECT id FROM permissions WHERE code = $1")
            .bind(codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT)
            .fetch_one(&self.pool)
            .await
            .expect("generated certificate permission should be migrated")
    }

    async fn add_position_grant(&self, unit_id: Uuid, position: &str) {
        sqlx::query(
            "INSERT INTO organization_permission_grants
                (organization_unit_id, permission_id, position_code)
             VALUES ($1, $2, $3)",
        )
        .bind(unit_id)
        .bind(self.permission_id().await)
        .bind(position)
        .execute(&self.pool)
        .await
        .expect("exact-unit permission grant should insert");
    }

    async fn add_scoped_role(
        &self,
        scope: Option<Uuid>,
        started_offset_days: i32,
        ended_offset_days: Option<i32>,
    ) {
        let role_id: Uuid = sqlx::query_scalar(
            "INSERT INTO roles (code, name, user_type, is_active)
             VALUES ($1, $1, 'staff', true)
             RETURNING id",
        )
        .bind(format!("certificate_role_{}", Uuid::new_v4().simple()))
        .fetch_one(&self.pool)
        .await
        .expect("role fixture should insert");
        sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)")
            .bind(role_id)
            .bind(self.permission_id().await)
            .execute(&self.pool)
            .await
            .expect("role permission fixture should insert");
        sqlx::query(
            "INSERT INTO user_roles
                (user_id, role_id, organization_unit_id, started_at, ended_at)
             VALUES ($1, $2, $3, $4::date, $5::date)",
        )
        .bind(self.actor.user_id)
        .bind(role_id)
        .bind(scope)
        .bind(
            chrono::Utc::now().date_naive()
                + chrono::Duration::days(i64::from(started_offset_days)),
        )
        .bind(ended_offset_days.map(|offset| {
            chrono::Utc::now().date_naive() + chrono::Duration::days(i64::from(offset))
        }))
        .execute(&self.pool)
        .await
        .expect("scoped user role fixture should insert");
    }
}

async fn insert_unit(pool: &PgPool, code: &str, parent_id: Option<Uuid>) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO organization_units
            (code, name, category, unit_type, parent_unit_id, is_active)
         VALUES ($1, $1, 'academic', 'unit', $2, true)
         RETURNING id",
    )
    .bind(code)
    .bind(parent_id)
    .fetch_one(pool)
    .await
    .expect("organization unit fixture should insert")
}

#[tokio::test]
async fn grant_in_unit_a_does_not_authorize_campaign_in_unit_b() {
    let fixture =
        CertificatePolicyFixture::with_position_grant("certificate_exact_unit", "head").await;

    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_ok());
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_b),
        CertificateAction::Update,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn concurrent_campaign_owner_transfer_blocks_stale_template_mutation() {
    let mut fixture = CertificatePolicyFixture::new("certificate_template_owner_race").await;
    fixture
        .actor
        .permissions
        .push(codes::CERTIFICATE_CREATE_ORGANIZATION_UNIT.to_string());
    for permission in [
        codes::CERTIFICATE_CREATE_ORGANIZATION_UNIT,
        codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
    ] {
        add_exact_grant(&fixture.pool, fixture.unit_a, permission).await;
    }
    let academic_year_id = insert_academic_year(&fixture.pool, 3119).await;
    let campaign = campaign_service::create_campaign(
        &fixture.pool,
        &fixture.actor,
        CreateCertificateCampaignRequest {
            academic_year_id,
            owner_organization_unit_id: Some(fixture.unit_a),
            name: "กิจกรรมก่อนย้ายหน่วยงาน".to_string(),
            event_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
        },
    )
    .await
    .unwrap();
    let template = template_service::create_template(
        &fixture.pool,
        &fixture.actor,
        campaign.id,
        CreateCertificateTemplateRequest {
            name: "แบบก่อนย้ายหน่วยงาน".to_string(),
            allowed_recipient_types: vec![RecipientType::External],
        },
    )
    .await
    .unwrap();

    let mut owner_transfer = fixture.pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE certificate_campaigns
         SET owner_organization_unit_id = $2, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(campaign.id)
    .bind(fixture.unit_b)
    .execute(&mut *owner_transfer)
    .await
    .unwrap();

    let mutation_pool = fixture.pool.clone();
    let mutation_actor = fixture.actor.clone();
    let mutation = tokio::spawn(async move {
        template_service::update_template(
            &mutation_pool,
            &mutation_actor,
            template.id,
            UpdateCertificateTemplateRequest {
                expected_updated_at: template.updated_at,
                name: Some("ชื่อที่ไม่ควรถูกบันทึก".to_string()),
                allowed_recipient_types: None,
                safe_margin_points: None,
                show_safe_area: None,
                layout: None,
                is_active: None,
                confirm_missing_issued_values: false,
            },
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !mutation.is_finished(),
        "template mutation should wait for the campaign-owner lock"
    );
    owner_transfer.commit().await.unwrap();

    let mutation_result = mutation.await.unwrap();
    assert!(
        matches!(
            mutation_result,
            Err(AppError::Conflict(_)) | Err(AppError::Forbidden(_))
        ),
        "stale owner mutation should be rejected, got {mutation_result:?}"
    );
    let persisted_name: String =
        sqlx::query_scalar("SELECT name FROM certificate_templates WHERE id = $1")
            .bind(template.id)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(persisted_name, "แบบก่อนย้ายหน่วยงาน");
}

#[tokio::test]
async fn concurrent_campaign_delete_and_template_update_do_not_deadlock() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_campaign_template_lock_order", 3121).await;
    let template = create_template_fixture_with_types(
        &pool,
        &actor,
        academic_year_id,
        "กิจกรรมทดสอบลำดับล็อก",
        "แบบทดสอบลำดับล็อก",
        vec![RecipientType::External],
    )
    .await;
    let barrier = Arc::new(tokio::sync::Barrier::new(3));

    let delete_pool = pool.clone();
    let delete_actor = actor.clone();
    let delete_barrier = barrier.clone();
    let campaign_id = template.campaign_id;
    let delete = tokio::spawn(async move {
        delete_barrier.wait().await;
        campaign_service::delete_campaign(&delete_pool, &delete_actor, campaign_id).await
    });

    let update_pool = pool.clone();
    let update_actor = actor.clone();
    let update_barrier = barrier.clone();
    let template_id = template.id;
    let expected_updated_at = template.updated_at;
    let update = tokio::spawn(async move {
        update_barrier.wait().await;
        template_service::update_template(
            &update_pool,
            &update_actor,
            template_id,
            UpdateCertificateTemplateRequest {
                expected_updated_at,
                name: Some("แบบที่แก้พร้อมการลบกิจกรรม".to_string()),
                allowed_recipient_types: None,
                safe_margin_points: None,
                show_safe_area: None,
                layout: None,
                is_active: None,
                confirm_missing_issued_values: false,
            },
        )
        .await
    });

    barrier.wait().await;
    let (delete_result, update_result) = tokio::time::timeout(Duration::from_secs(5), async {
        (delete.await.unwrap(), update.await.unwrap())
    })
    .await
    .expect("campaign deletion and template update must serialize without deadlock");
    assert!(
        !matches!(delete_result, Err(AppError::DbError(_))),
        "campaign deletion returned a database concurrency error: {delete_result:?}"
    );
    assert!(
        !matches!(update_result, Err(AppError::DbError(_))),
        "template update returned a database concurrency error: {update_result:?}"
    );
}

#[tokio::test]
async fn exact_unit_grants_require_matching_position_active_membership_and_active_unit() {
    let fixture =
        CertificatePolicyFixture::with_position_grant("certificate_grant_bounds", "coordinator")
            .await;
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE organization_members
         SET position_code = 'coordinator'
         WHERE user_id = $1 AND organization_unit_id = $2",
    )
    .bind(fixture.actor.user_id)
    .bind(fixture.unit_a)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_ok());

    sqlx::query(
        "UPDATE organization_members SET ended_at = CURRENT_DATE
         WHERE user_id = $1 AND organization_unit_id = $2",
    )
    .bind(fixture.actor.user_id)
    .bind(fixture.unit_a)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE organization_members
         SET started_at = CURRENT_DATE + 1, ended_at = NULL
         WHERE user_id = $1 AND organization_unit_id = $2",
    )
    .bind(fixture.actor.user_id)
    .bind(fixture.unit_a)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query("UPDATE organization_units SET is_active = false WHERE id = $1")
        .bind(fixture.unit_a)
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn scoped_roles_and_parent_membership_do_not_leak_to_other_or_child_units() {
    let fixture = CertificatePolicyFixture::new("certificate_role_scope").await;
    fixture.add_scoped_role(Some(fixture.unit_a), 0, None).await;
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_ok());
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_b),
        CertificateAction::Update,
    )
    .await
    .is_err());

    let child = insert_unit(
        &fixture.pool,
        "certificate_role_scope_child",
        Some(fixture.unit_a),
    )
    .await;
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(child),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE user_roles
         SET organization_unit_id = NULL, ended_at = NULL
         WHERE user_id = $1",
    )
    .bind(fixture.actor.user_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    for member_unit in [fixture.unit_a, fixture.unit_b] {
        assert!(require_owner_action(
            &fixture.pool,
            &fixture.actor,
            Some(member_unit),
            CertificateAction::Update,
        )
        .await
        .is_ok());
    }
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(child),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query("UPDATE user_roles SET started_at = CURRENT_DATE + 1 WHERE user_id = $1")
        .bind(fixture.actor.user_id)
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE user_roles SET started_at = CURRENT_DATE - 2, ended_at = CURRENT_DATE
         WHERE user_id = $1",
    )
    .bind(fixture.actor.user_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn exact_delegation_authorizes_non_member_only_during_its_active_window() {
    let fixture = CertificatePolicyFixture::new("certificate_delegation").await;
    let delegated_unit = insert_unit(&fixture.pool, "certificate_delegation_target", None).await;
    let grantor = create_test_user(
        &fixture.pool,
        "certificate-delegation-grantor@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let delegation_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organization_permission_delegations
            (from_user_id, to_user_id, permission_id, organization_unit_id, started_at, expires_at)
         VALUES ($1, $2, $3, $4, NOW(), NOW() + INTERVAL '1 day')
         RETURNING id",
    )
    .bind(grantor)
    .bind(fixture.actor.user_id)
    .bind(fixture.permission_id().await)
    .bind(delegated_unit)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();

    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(delegated_unit),
        CertificateAction::Update,
    )
    .await
    .is_ok());
    sqlx::query(
        "UPDATE organization_permission_delegations
         SET started_at = NOW() + INTERVAL '1 day', expires_at = NOW() + INTERVAL '2 days'
         WHERE id = $1",
    )
    .bind(delegation_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(delegated_unit),
        CertificateAction::Update,
    )
    .await
    .is_err());
    sqlx::query(
        "UPDATE organization_permission_delegations
         SET started_at = NOW() - INTERVAL '2 days', expires_at = NOW() - INTERVAL '1 day'
         WHERE id = $1",
    )
    .bind(delegation_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(delegated_unit),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE organization_permission_delegations
         SET started_at = NOW() - INTERVAL '1 hour', expires_at = NULL, revoked_at = NOW()
         WHERE id = $1",
    )
    .bind(delegation_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(delegated_unit),
        CertificateAction::Update,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn school_override_null_owner_and_list_scope_union_are_explicit() {
    let fixture =
        CertificatePolicyFixture::with_position_grant("certificate_scope_union", "head").await;
    fixture.add_scoped_role(Some(fixture.unit_b), 0, None).await;
    let units = accessible_exact_units_for_permission(
        &fixture.pool,
        fixture.actor.user_id,
        codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
    )
    .await
    .unwrap();
    assert_eq!(
        units.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([fixture.unit_a, fixture.unit_b])
    );
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        None,
        CertificateAction::Update,
    )
    .await
    .is_err());

    let school_actor = ActorContext {
        user_id: fixture.actor.user_id,
        permissions: vec![codes::CERTIFICATE_UPDATE_SCHOOL.to_string()],
    };
    assert!(require_owner_action(
        &fixture.pool,
        &school_actor,
        None,
        CertificateAction::Update,
    )
    .await
    .is_ok());
    assert!(require_owner_action(
        &fixture.pool,
        &school_actor,
        Some(fixture.unit_b),
        CertificateAction::Update,
    )
    .await
    .is_ok());
}

async fn insert_academic_year(pool: &PgPool, year: i32) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO academic_years
            (year, name, start_date, end_date, is_active)
         VALUES ($1, $2, $3, $4, false)
         RETURNING id",
    )
    .bind(year)
    .bind(format!("ปีการศึกษา {year}"))
    .bind(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap())
    .bind(NaiveDate::from_ymd_opt(2027, 4, 30).unwrap())
    .fetch_one(pool)
    .await
    .expect("academic year fixture should insert")
}

async fn add_exact_grant(pool: &PgPool, unit_id: Uuid, permission_code: &str) {
    sqlx::query(
        "INSERT INTO organization_permission_grants
            (organization_unit_id, permission_id, position_code)
         SELECT $1, id, 'head'
         FROM permissions
         WHERE code = $2
         ON CONFLICT DO NOTHING",
    )
    .bind(unit_id)
    .bind(permission_code)
    .execute(pool)
    .await
    .expect("certificate exact-unit grant should insert");
}

#[tokio::test]
async fn campaign_create_and_list_are_limited_to_the_exact_owner_unit() {
    let fixture = CertificatePolicyFixture::new("certificate_campaign_scope").await;
    let academic_year_id = insert_academic_year(&fixture.pool, 3101).await;
    for permission in [
        codes::CERTIFICATE_CREATE_ORGANIZATION_UNIT,
        codes::CERTIFICATE_READ_ORGANIZATION_UNIT,
    ] {
        add_exact_grant(&fixture.pool, fixture.unit_a, permission).await;
    }

    let created = campaign_service::create_campaign(
        &fixture.pool,
        &fixture.actor,
        CreateCertificateCampaignRequest {
            academic_year_id,
            owner_organization_unit_id: Some(fixture.unit_a),
            name: "  กิจกรรมวันภาษาไทย  ".to_string(),
            event_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
        },
    )
    .await
    .expect("exact-unit campaign should be created");
    assert_eq!(created.name, "กิจกรรมวันภาษาไทย");
    assert_eq!(created.owner_organization_unit_id, Some(fixture.unit_a));

    let denied = campaign_service::create_campaign(
        &fixture.pool,
        &fixture.actor,
        CreateCertificateCampaignRequest {
            academic_year_id,
            owner_organization_unit_id: Some(fixture.unit_b),
            name: "กิจกรรมนอกขอบเขต".to_string(),
            event_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
        },
    )
    .await;
    assert!(matches!(denied, Err(AppError::Forbidden(_))));

    sqlx::query(
        "INSERT INTO certificate_campaigns
            (academic_year_id, owner_organization_unit_id, name, event_date, created_by, updated_by)
         VALUES ($1, $2, 'กิจกรรมหน่วยงานอื่น', $3, $4, $4)",
    )
    .bind(academic_year_id)
    .bind(fixture.unit_b)
    .bind(NaiveDate::from_ymd_opt(2026, 8, 15).unwrap())
    .bind(fixture.actor.user_id)
    .execute(&fixture.pool)
    .await
    .unwrap();

    let listed = campaign_service::list_campaigns(
        &fixture.pool,
        &fixture.actor,
        CertificateCampaignListQuery::default(),
    )
    .await
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let owner_options = campaign_service::list_owner_options(&fixture.pool, &fixture.actor)
        .await
        .unwrap();
    assert_eq!(owner_options.len(), 1);
    assert_eq!(owner_options[0].id, fixture.unit_a);

    sqlx::query("UPDATE organization_units SET is_active = false WHERE id = $1")
        .bind(fixture.unit_a)
        .execute(&fixture.pool)
        .await
        .unwrap();
    let school_update_actor = ActorContext {
        user_id: fixture.actor.user_id,
        permissions: vec![codes::CERTIFICATE_UPDATE_SCHOOL.to_string()],
    };
    let inactive_owner_update = campaign_service::update_campaign(
        &fixture.pool,
        &school_update_actor,
        created.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: created.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: None,
            name: Some("แก้หน่วยงานที่ปิดใช้งาน".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: false,
        },
    )
    .await;
    assert!(matches!(
        inactive_owner_update,
        Err(AppError::ValidationError(_))
    ));
}

fn school_certificate_actor(user_id: Uuid) -> ActorContext {
    ActorContext {
        user_id,
        permissions: vec![
            codes::CERTIFICATE_READ_SCHOOL.to_string(),
            codes::CERTIFICATE_CREATE_SCHOOL.to_string(),
            codes::CERTIFICATE_UPDATE_SCHOOL.to_string(),
            codes::CERTIFICATE_DELETE_SCHOOL.to_string(),
            codes::CERTIFICATE_SUBMIT_SCHOOL.to_string(),
            codes::CERTIFICATE_DOWNLOAD_SCHOOL.to_string(),
        ],
    }
}

async fn school_campaign_fixture(test_name: &str, year: i32) -> (PgPool, ActorContext, Uuid) {
    let pool = create_named_test_pool(test_name).await;
    run_test_migrations(&pool).await;
    let user_id = create_test_user(
        &pool,
        &format!("{test_name}-school@example.invalid"),
        "test-password",
    )
    .await
    .unwrap();
    let academic_year_id = insert_academic_year(&pool, year).await;
    (pool, school_certificate_actor(user_id), academic_year_id)
}

fn campaign_create_payload(
    academic_year_id: Uuid,
    owner_organization_unit_id: Option<Uuid>,
    name: &str,
) -> CreateCertificateCampaignRequest {
    CreateCertificateCampaignRequest {
        academic_year_id,
        owner_organization_unit_id,
        name: name.to_string(),
        event_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
    }
}

#[tokio::test]
async fn issued_campaign_identity_is_immutable_and_live_text_requires_confirmation() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_campaign_issued_update", 3102).await;
    let second_academic_year_id = insert_academic_year(&pool, 3103).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "การแข่งขันคำคม"),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE certificate_campaigns
         SET activity_sequence = 42, status = 'active', updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(campaign.id)
    .execute(&pool)
    .await
    .unwrap();
    let issued = campaign_service::get_campaign(&pool, &actor, campaign.id)
        .await
        .unwrap();

    let immutable_year = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: issued.updated_at,
            academic_year_id: Some(second_academic_year_id),
            owner_organization_unit_id: None,
            name: None,
            event_date: None,
            confirm_affects_issued_certificates: false,
        },
    )
    .await;
    assert!(matches!(immutable_year, Err(AppError::Conflict(_))));

    let unconfirmed_name = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: issued.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: None,
            name: Some("การแข่งขันคำคมฉบับแก้ไข".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: false,
        },
    )
    .await;
    assert!(matches!(unconfirmed_name, Err(AppError::Conflict(_))));

    let confirmed = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: issued.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: None,
            name: Some("การแข่งขันคำคมฉบับแก้ไข".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(confirmed.name, "การแข่งขันคำคมฉบับแก้ไข");

    let stale = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: issued.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: Some(NullableUuidUpdate { value: None }),
            name: Some("ข้อมูลเก่า".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: true,
        },
    )
    .await;
    assert!(matches!(stale, Err(AppError::Conflict(_))));

    sqlx::query(
        "INSERT INTO certificate_issue_requests (campaign_id, submitted_by)
         VALUES ($1, $2)",
    )
    .bind(campaign.id)
    .bind(actor.user_id)
    .execute(&pool)
    .await
    .unwrap();
    let locked = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: confirmed.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: None,
            name: Some("แก้ระหว่างตรวจไม่ได้".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: true,
        },
    )
    .await;
    assert!(matches!(locked, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn campaign_manual_transitions_delete_rules_and_audit_are_explicit() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_campaign_lifecycle", 3104).await;
    let draft = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "ร่างสำหรับลบ"),
    )
    .await
    .unwrap();
    let activate_draft = campaign_service::change_campaign_status(
        &pool,
        &actor,
        draft.id,
        ChangeCertificateCampaignStatusRequest {
            expected_updated_at: draft.updated_at,
            status: CertificateCampaignStatus::Active,
        },
    )
    .await;
    assert!(matches!(activate_draft, Err(AppError::Conflict(_))));

    let draft_template = template_service::create_template(
        &pool,
        &actor,
        draft.id,
        CreateCertificateTemplateRequest {
            name: "แบบพร้อมพื้นหลัง".to_string(),
            allowed_recipient_types: vec![RecipientType::External],
        },
    )
    .await
    .unwrap();
    let background_id = insert_ready_template_file(
        &pool,
        &actor,
        draft_template.id,
        "certificate_template_background",
        pdf_inspection(841.89, 595.28, 0),
    )
    .await;
    template_service::attach_background(
        &pool,
        &actor,
        draft_template.id,
        AttachCertificateBackgroundRequest {
            file_id: background_id,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap();

    let detached_files = campaign_service::delete_campaign(&pool, &actor, draft.id)
        .await
        .unwrap();
    assert_eq!(detached_files, vec![background_id]);
    assert!(matches!(
        campaign_service::get_campaign(&pool, &actor, draft.id).await,
        Err(AppError::NotFound(_))
    ));

    let audit_metadata: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT metadata FROM audit_logs
         WHERE entity_type = 'certificate_campaign' AND entity_id = $1
         ORDER BY created_at, id",
    )
    .bind(draft.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audit_metadata.len(), 2);
    let serialized_audit = serde_json::to_string(&audit_metadata).unwrap();
    assert!(!serialized_audit.contains("ร่างสำหรับลบ"));
    assert!(!serialized_audit.contains("firstName"));
    assert!(!serialized_audit.contains("lastName"));

    let issued = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมที่ออกแล้ว"),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE certificate_campaigns
         SET activity_sequence = 43, status = 'active', updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(issued.id)
    .execute(&pool)
    .await
    .unwrap();
    let mut current = campaign_service::get_campaign(&pool, &actor, issued.id)
        .await
        .unwrap();
    for next in [
        CertificateCampaignStatus::Closed,
        CertificateCampaignStatus::Active,
        CertificateCampaignStatus::Archived,
        CertificateCampaignStatus::Active,
    ] {
        current = campaign_service::change_campaign_status(
            &pool,
            &actor,
            issued.id,
            ChangeCertificateCampaignStatusRequest {
                expected_updated_at: current.updated_at,
                status: next,
            },
        )
        .await
        .unwrap();
        assert_eq!(current.status, next);
    }
    assert!(matches!(
        campaign_service::delete_campaign(&pool, &actor, issued.id).await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn unused_template_with_draft_candidates_returns_lifecycle_conflict() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_template_candidate_delete", 3118).await;
    let template = create_template_fixture_with_types(
        &pool,
        &actor,
        academic_year_id,
        "กิจกรรมที่ยังมีรายชื่อร่าง",
        "แบบที่ยังมีรายชื่อร่าง",
        vec![RecipientType::External],
    )
    .await;
    sqlx::query(
        "INSERT INTO certificate_candidates (
            campaign_id, template_id, recipient_type, imported_first_name,
            imported_last_name, selected_name_source, match_status, validation_status
         ) VALUES ($1, $2, 'external', 'ผู้รับ', 'ฉบับร่าง', 'file',
                   'external_confirmed', 'ready')",
    )
    .bind(template.campaign_id)
    .bind(template.id)
    .execute(&pool)
    .await
    .unwrap();

    assert!(matches!(
        template_service::delete_template(&pool, &actor, template.id).await,
        Err(AppError::Conflict(_))
    ));
    assert!(template_service::get_template(&pool, &actor, template.id)
        .await
        .is_ok());
}

#[tokio::test]
async fn matched_accounts_cannot_become_external_in_single_or_bulk_flows() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_candidate_external_guard", 3122).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "การแข่งขันคำคม"),
    )
    .await
    .unwrap();
    template_service::create_template(
        &pool,
        &actor,
        campaign.id,
        CreateCertificateTemplateRequest {
            name: "แบบรางวัลนักเรียน".to_string(),
            allowed_recipient_types: vec![RecipientType::Student, RecipientType::External],
        },
    )
    .await
    .unwrap();

    let student_user_id = create_test_user(
        &pool,
        "certificate-candidate-student@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE users
         SET username = 'student-candidate-0069', user_type = 'student',
             title = 'เด็กหญิง', first_name = 'กมล', last_name = 'ใจดี', status = 'active'
         WHERE id = $1",
    )
    .bind(student_user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO student_info (user_id, student_id) VALUES ($1, 'S-0069')")
        .bind(student_user_id)
        .execute(&pool)
        .await
        .unwrap();

    let imported = candidate_service::import_candidates(
        &pool,
        &actor,
        campaign.id,
        CertificateImportRequest {
            source: CertificateImportSource::Csv,
            headers: vec![
                "ประเภทผู้รับ".to_string(),
                "รหัสนักเรียน".to_string(),
                "ชื่อ".to_string(),
                "นามสกุล".to_string(),
            ],
            rows: vec![CertificateImportRowInput {
                recipient_type: "student".to_string(),
                student_id: Some("S-0069".to_string()),
                staff_username: None,
                title: Some("เด็กหญิง".to_string()),
                first_name: "กมล".to_string(),
                last_name: "ใจดี".to_string(),
                activity_item: Some("การแข่งขันคำคม".to_string()),
                award_or_role: Some("รองชนะเลิศอันดับที่ 1".to_string()),
                template_name: Some("แบบรางวัลนักเรียน".to_string()),
                custom_values: BTreeMap::new(),
            }],
        },
    )
    .await
    .unwrap();
    let candidate = &imported.candidates[0];
    assert_eq!(candidate.matched_user_id, Some(student_user_id));

    assert!(
        candidate_service::confirm_external(&pool, &actor, candidate.id)
            .await
            .is_err()
    );
    assert!(candidate_service::bulk_update(
        &pool,
        &actor,
        CertificateCandidateBulkRequest::ConfirmExternal {
            candidate_ids: vec![candidate.id],
        },
    )
    .await
    .is_err());
}

async fn create_ready_candidate_template(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    name: &str,
    allowed_recipient_types: Vec<RecipientType>,
) -> crate::modules::certificates::models::CertificateTemplateDetail {
    let template = template_service::create_template(
        pool,
        actor,
        campaign_id,
        CreateCertificateTemplateRequest {
            name: name.to_string(),
            allowed_recipient_types,
        },
    )
    .await
    .unwrap();
    let file_id = insert_ready_template_file(
        pool,
        actor,
        template.id,
        "certificate_template_background",
        pdf_inspection(841.89, 595.28, 0),
    )
    .await;
    template_service::attach_background(
        pool,
        actor,
        template.id,
        AttachCertificateBackgroundRequest {
            file_id,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap()
    .template
}

async fn insert_certificate_student(
    pool: &PgPool,
    username: &str,
    student_id: &str,
    first_name: &str,
    last_name: &str,
    status: &str,
) -> Uuid {
    let user_id = create_test_user(
        pool,
        &format!("{username}@candidate-test.invalid"),
        "test-password",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE users
         SET username = $2, user_type = 'student', title = 'เด็กหญิง',
             first_name = $3, last_name = $4, status = $5
         WHERE id = $1",
    )
    .bind(user_id)
    .bind(username)
    .bind(first_name)
    .bind(last_name)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO student_info (user_id, student_id) VALUES ($1, $2)")
        .bind(user_id)
        .bind(student_id)
        .execute(pool)
        .await
        .unwrap();
    user_id
}

async fn insert_certificate_staff(
    pool: &PgPool,
    username: &str,
    first_name: &str,
    last_name: &str,
    status: &str,
) -> Uuid {
    let user_id = create_test_user(
        pool,
        &format!("{username}@candidate-test.invalid"),
        "test-password",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE users
         SET username = $2, user_type = 'staff', title = 'นาย',
             first_name = $3, last_name = $4, status = $5
         WHERE id = $1",
    )
    .bind(user_id)
    .bind(username)
    .bind(first_name)
    .bind(last_name)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    user_id
}

fn candidate_import_request(rows: Vec<CertificateImportRowInput>) -> CertificateImportRequest {
    CertificateImportRequest {
        source: CertificateImportSource::Csv,
        headers: vec![
            "ประเภทผู้รับ".to_string(),
            "รหัสนักเรียน".to_string(),
            "ชื่อผู้ใช้บุคลากร".to_string(),
            "คำนำหน้า".to_string(),
            "ชื่อ".to_string(),
            "นามสกุล".to_string(),
            "รายการกิจกรรม".to_string(),
            "รางวัลหรือบทบาท".to_string(),
            "แบบเกียรติบัตร".to_string(),
        ],
        rows,
    }
}

fn candidate_import_row(
    recipient_type: &str,
    student_id: Option<&str>,
    staff_username: Option<&str>,
    first_name: &str,
    last_name: &str,
    template_name: &str,
) -> CertificateImportRowInput {
    CertificateImportRowInput {
        recipient_type: recipient_type.to_string(),
        student_id: student_id.map(str::to_string),
        staff_username: staff_username.map(str::to_string),
        title: None,
        first_name: first_name.to_string(),
        last_name: last_name.to_string(),
        activity_item: Some("การแข่งขันคำคม".to_string()),
        award_or_role: Some("รองชนะเลิศอันดับที่ 1".to_string()),
        template_name: Some(template_name.to_string()),
        custom_values: BTreeMap::new(),
    }
}

#[tokio::test]
async fn candidate_matching_decision_table_recomputes_names_accounts_and_statuses() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_candidate_matching_table", 3123).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "การแข่งขันทักษะภาษาไทย"),
    )
    .await
    .unwrap();
    let shared_template = create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบนักเรียนและภายนอก",
        vec![RecipientType::Student, RecipientType::External],
    )
    .await;
    create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบบุคลากร",
        vec![RecipientType::Staff],
    )
    .await;
    let student_id = insert_certificate_student(
        &pool,
        "student-match-1",
        "S-MATCH-1",
        "กมล",
        "ใจดี",
        "active",
    )
    .await;
    let staff_id =
        insert_certificate_staff(&pool, "teacher.exact", "สมชาย", "รักเรียน", "active").await;
    let inactive_id = insert_certificate_student(
        &pool,
        "student-inactive-1",
        "S-INACTIVE-1",
        "พิมพ์ใจ",
        "งามดี",
        "inactive",
    )
    .await;

    let imported = candidate_service::import_candidates(
        &pool,
        &actor,
        campaign.id,
        candidate_import_request(vec![
            candidate_import_row(
                "student",
                Some("S-MATCH-1"),
                None,
                "  กมล ",
                "ใจดี",
                "แบบนักเรียนและภายนอก",
            ),
            candidate_import_row(
                "staff",
                None,
                Some("teacher.exact"),
                "สมชาย",
                "ชื่อจากไฟล์",
                "แบบบุคลากร",
            ),
            candidate_import_row(
                "student",
                Some("S-INACTIVE-1"),
                None,
                "พิมพ์ใจ",
                "งามดี",
                "แบบนักเรียนและภายนอก",
            ),
            candidate_import_row(
                "student",
                Some("S-NOT-FOUND"),
                None,
                "เด็กนอกระบบ",
                "ทดลอง",
                "แบบนักเรียนและภายนอก",
            ),
            candidate_import_row(
                "staff",
                None,
                Some("Teacher.Exact"),
                "สมชาย",
                "รักเรียน",
                "แบบบุคลากร",
            ),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(imported.batch.ready_count, 1);
    assert_eq!(imported.batch.review_count, 4);
    assert_eq!(imported.batch.invalid_count, 0);

    let matched = &imported.candidates[0];
    assert_eq!(matched.matched_user_id, Some(student_id));
    assert_eq!(matched.match_status, CandidateMatchStatus::Matched);
    assert_eq!(
        matched.selected_name_source,
        Some(CandidateNameSource::Account)
    );
    assert_eq!(matched.validation_status, CandidateValidationStatus::Ready);

    let mismatch = &imported.candidates[1];
    assert_eq!(mismatch.matched_user_id, Some(staff_id));
    assert_eq!(mismatch.match_status, CandidateMatchStatus::NameMismatch);
    assert!(mismatch
        .validation_codes
        .contains(&CandidateValidationCode::NameSourceRequired));
    let resolved = candidate_service::bulk_update(
        &pool,
        &actor,
        CertificateCandidateBulkRequest::ChooseName {
            candidate_ids: vec![mismatch.id],
            name_source: CandidateNameSource::File,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        resolved.candidates[0].validation_status,
        CandidateValidationStatus::Ready
    );
    assert_eq!(
        resolved.candidates[0].selected_name_source,
        Some(CandidateNameSource::File)
    );

    let inactive = &imported.candidates[2];
    assert_eq!(inactive.matched_user_id, Some(inactive_id));
    assert_eq!(inactive.match_status, CandidateMatchStatus::Inactive);
    assert!(
        candidate_service::confirm_external(&pool, &actor, inactive.id)
            .await
            .is_err()
    );

    let unmatched = &imported.candidates[3];
    let converted = candidate_service::confirm_external(&pool, &actor, unmatched.id)
        .await
        .unwrap();
    assert_eq!(converted.recipient_type, RecipientType::External);
    assert_eq!(
        converted.match_status,
        CandidateMatchStatus::ExternalConfirmed
    );
    assert_eq!(converted.student_id.as_deref(), Some("S-NOT-FOUND"));
    assert_eq!(
        converted.validation_status,
        CandidateValidationStatus::Ready
    );
    let reassigned = candidate_service::bulk_update(
        &pool,
        &actor,
        CertificateCandidateBulkRequest::AssignTemplate {
            candidate_ids: vec![converted.id],
            template_id: shared_template.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        reassigned.candidates[0].student_id.as_deref(),
        Some("S-NOT-FOUND"),
        "converted external candidates retain their internal lookup for issuance revalidation"
    );
    assert_eq!(
        reassigned.candidates[0].match_status,
        CandidateMatchStatus::ExternalConfirmed
    );
    assert_eq!(
        reassigned.candidates[0].validation_status,
        CandidateValidationStatus::Ready
    );

    assert_eq!(
        imported.candidates[4].match_status,
        CandidateMatchStatus::NotFound
    );
}

#[tokio::test]
async fn candidate_import_keeps_row_errors_atomic_headers_duplicates_and_safe_audit() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_candidate_validation_table", 3124).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมวันภาษาไทย"),
    )
    .await
    .unwrap();
    create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบการแข่งขันภายนอก",
        vec![RecipientType::External],
    )
    .await;
    create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบนักเรียนภายใน",
        vec![RecipientType::Student],
    )
    .await;

    let mut external = candidate_import_row(
        "external",
        None,
        None,
        "บุคคลลับทดสอบ",
        "นามสกุลลับทดสอบ",
        "แบบการแข่งขันภายนอก",
    );
    external
        .custom_values
        .insert("ครูผู้ควบคุม".to_string(), "ค่าลับเฉพาะแถว".to_string());
    let duplicate = external.clone();
    let incompatible = candidate_import_row(
        "external",
        None,
        None,
        "ผู้แข่งขันต่างโรงเรียน",
        "ใจกล้า",
        "แบบนักเรียนภายใน",
    );
    let invalid =
        candidate_import_row("external", None, None, "", "ข้อมูลไม่ครบ", "แบบการแข่งขันภายนอก");
    let mut request = candidate_import_request(vec![external, duplicate, incompatible, invalid]);
    request.headers.push("ครูผู้ควบคุม".to_string());
    let imported = candidate_service::import_candidates(&pool, &actor, campaign.id, request)
        .await
        .unwrap();
    assert_eq!(imported.batch.ready_count, 0);
    assert_eq!(imported.batch.review_count, 3);
    assert_eq!(imported.batch.invalid_count, 1);
    assert!(imported.candidates[0]
        .validation_codes
        .contains(&CandidateValidationCode::DuplicateCandidate));
    assert!(imported.candidates[1]
        .validation_codes
        .contains(&CandidateValidationCode::DuplicateCandidate));
    assert!(imported.candidates[2]
        .validation_codes
        .contains(&CandidateValidationCode::TemplateIncompatible));
    assert_eq!(
        imported.candidates[3].validation_status,
        CandidateValidationStatus::Invalid
    );

    let confirmed = candidate_service::bulk_update(
        &pool,
        &actor,
        CertificateCandidateBulkRequest::ConfirmDuplicate {
            candidate_ids: vec![imported.candidates[0].id, imported.candidates[1].id],
        },
    )
    .await
    .unwrap();
    assert!(confirmed
        .candidates
        .iter()
        .all(|candidate| candidate.validation_status == CandidateValidationStatus::Ready));

    let batch_count_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM certificate_import_batches WHERE campaign_id = $1",
    )
    .bind(campaign.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let forbidden = CertificateImportRequest {
        source: CertificateImportSource::Csv,
        headers: vec![
            "ประเภทผู้รับ".to_string(),
            "ชื่อ".to_string(),
            "นามสกุล".to_string(),
            "เลขบัตรประชาชน".to_string(),
        ],
        rows: vec![candidate_import_row(
            "external",
            None,
            None,
            "ไม่ควรถูกบันทึก",
            "ทั้งแถว",
            "แบบการแข่งขันภายนอก",
        )],
    };
    assert!(
        candidate_service::import_candidates(&pool, &actor, campaign.id, forbidden)
            .await
            .is_err()
    );
    let mut forbidden_cell = candidate_import_row(
        "external",
        None,
        None,
        "ไม่ควรถูกบันทึก",
        "จากค่าต้องห้าม",
        "แบบการแข่งขันภายนอก",
    );
    forbidden_cell.award_or_role = Some("ข้อมูล 0-0000-00000-00-0".to_string());
    assert!(candidate_service::import_candidates(
        &pool,
        &actor,
        campaign.id,
        candidate_import_request(vec![forbidden_cell]),
    )
    .await
    .is_err());
    let batch_count_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM certificate_import_batches WHERE campaign_id = $1",
    )
    .bind(campaign.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(batch_count_after, batch_count_before);

    let audit_text: String = sqlx::query_scalar(
        "SELECT COALESCE(string_agg(metadata::text, ' '), '')
         FROM audit_logs WHERE entity_type = 'certificate_candidate'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for forbidden_value in ["บุคคลลับทดสอบ", "นามสกุลลับทดสอบ", "ค่าลับเฉพาะแถว"]
    {
        assert!(!audit_text.contains(forbidden_value));
    }
}

#[tokio::test]
async fn soft_delete_recomputes_duplicate_status_for_remaining_candidates() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_candidate_duplicate_delete", 3128).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมรายการซ้ำ"),
    )
    .await
    .unwrap();
    let template = create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบบุคคลภายนอก",
        vec![RecipientType::External],
    )
    .await;
    let first_row = candidate_import_row(
        "external",
        None,
        None,
        "ผู้สมัคร",
        "รายการหนึ่ง",
        "แบบบุคคลภายนอก",
    );
    let second_row = candidate_import_row(
        "external",
        None,
        None,
        "ผู้สมัคร",
        "รายการสอง",
        "แบบบุคคลภายนอก",
    );
    let imported = candidate_service::import_candidates(
        &pool,
        &actor,
        campaign.id,
        candidate_import_request(vec![first_row, second_row]),
    )
    .await
    .unwrap();
    assert!(imported
        .candidates
        .iter()
        .all(|candidate| candidate.validation_status == CandidateValidationStatus::Ready));

    let changed = candidate_service::update_candidate(
        &pool,
        &actor,
        imported.candidates[1].id,
        UpdateCertificateCandidateRequest {
            expected_updated_at: imported.candidates[1].updated_at,
            template_id: Some(template.id),
            recipient_type: RecipientType::External,
            student_id: None,
            staff_username: None,
            imported_title: None,
            imported_first_name: "ผู้สมัคร".to_string(),
            imported_last_name: "รายการหนึ่ง".to_string(),
            selected_name_source: Some(CandidateNameSource::File),
            activity_item: Some("การแข่งขันคำคม".to_string()),
            award_or_role: Some("รองชนะเลิศอันดับที่ 1".to_string()),
            custom_values: BTreeMap::new(),
        },
    )
    .await
    .unwrap();

    let peer = candidate_service::get_candidate(&pool, &actor, imported.candidates[0].id)
        .await
        .unwrap();
    assert!(peer
        .validation_codes
        .contains(&CandidateValidationCode::DuplicateCandidate));

    candidate_service::delete_candidate(&pool, &actor, changed.id)
        .await
        .unwrap();

    let survivor = candidate_service::get_candidate(&pool, &actor, imported.candidates[0].id)
        .await
        .unwrap();
    assert!(!survivor
        .validation_codes
        .contains(&CandidateValidationCode::DuplicateCandidate));
    assert_eq!(survivor.validation_status, CandidateValidationStatus::Ready);
}

#[tokio::test]
async fn external_confirmation_rechecks_new_accounts_and_bulk_is_all_or_nothing() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_candidate_external_recheck", 3125).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมแข่งขันภายนอก"),
    )
    .await
    .unwrap();
    create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบนักเรียนหรือภายนอก",
        vec![RecipientType::Student, RecipientType::External],
    )
    .await;
    let imported = candidate_service::import_candidates(
        &pool,
        &actor,
        campaign.id,
        candidate_import_request(vec![
            candidate_import_row(
                "student",
                Some("S-APPEARED"),
                None,
                "บัญชี",
                "เพิ่งสร้าง",
                "แบบนักเรียนหรือภายนอก",
            ),
            candidate_import_row(
                "student",
                Some("S-STILL-MISSING"),
                None,
                "ยังไม่มี",
                "บัญชี",
                "แบบนักเรียนหรือภายนอก",
            ),
        ]),
    )
    .await
    .unwrap();
    insert_certificate_student(
        &pool,
        "student-appeared",
        "S-APPEARED",
        "บัญชี",
        "เพิ่งสร้าง",
        "inactive",
    )
    .await;

    assert!(candidate_service::bulk_update(
        &pool,
        &actor,
        CertificateCandidateBulkRequest::ConfirmExternal {
            candidate_ids: imported
                .candidates
                .iter()
                .map(|candidate| candidate.id)
                .collect(),
        },
    )
    .await
    .is_err());
    let listed = candidate_service::list_candidates(
        &pool,
        &actor,
        campaign.id,
        CertificateCandidateListQuery::default(),
    )
    .await
    .unwrap();
    assert!(listed
        .items
        .iter()
        .all(|candidate| candidate.recipient_type == RecipientType::Student));
    assert!(listed
        .items
        .iter()
        .all(|candidate| candidate.match_status == CandidateMatchStatus::NotFound));
}

#[tokio::test]
async fn candidate_preview_uses_selected_draft_values_without_issued_identity() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_candidate_preview_values", 3126).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมวิทยากร"),
    )
    .await
    .unwrap();
    let template = create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบวิทยากรภายนอก",
        vec![RecipientType::External],
    )
    .await;
    let created = candidate_service::create_manual_external(
        &pool,
        &actor,
        campaign.id,
        CreateManualExternalCandidateRequest {
            template_id: Some(template.id),
            title: Some("ดร.".to_string()),
            first_name: "ชลธิชา".to_string(),
            last_name: "แบ่งปัน".to_string(),
            activity_item: Some("บรรยายการเขียนคำคม".to_string()),
            award_or_role: Some("วิทยากร".to_string()),
            custom_values: BTreeMap::from([(
                "หัวข้อพิเศษ".to_string(),
                "ภาษาไทยสร้างสรรค์".to_string(),
            )]),
        },
    )
    .await
    .unwrap();
    let platform = crate::modules::files::platform_service::FilePlatform::new(
        Arc::new(PreviewStorage),
        Arc::new(PreviewScanner),
    );
    let manifest = render_service::preview_manifest(
        &pool,
        &actor,
        &platform,
        "โรงเรียนตัวอย่าง".to_string(),
        template.id,
        CertificatePreviewManifestRequest {
            preview_kind: CertificatePreviewKind::Candidate,
            candidate_id: Some(created.candidates[0].id),
            sample_values: BTreeMap::new(),
            layout: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(manifest.recipient_values["คำนำหน้า"], "ดร.");
    assert_eq!(manifest.recipient_values["ชื่อ"], "ชลธิชา");
    assert_eq!(manifest.recipient_values["นามสกุล"], "แบ่งปัน");
    assert_eq!(manifest.recipient_values["รางวัลหรือบทบาท"], "วิทยากร");
    assert_eq!(manifest.recipient_values["หัวข้อพิเศษ"], "ภาษาไทยสร้างสรรค์");
    assert_eq!(manifest.certificate_number, "ตัวอย่าง");
    assert!(manifest.qr_payload.contains("ตัวอย่าง"));
}

#[tokio::test]
async fn candidate_edit_rechecks_authoritative_account_and_search_returns_minimal_fields() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_candidate_account_recheck", 3127).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมเชื่อมบัญชี"),
    )
    .await
    .unwrap();
    let template = create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบเชื่อมบัญชีนักเรียน",
        vec![RecipientType::Student],
    )
    .await;
    let student_user_id = insert_certificate_student(
        &pool,
        "student-recheck-1",
        "S-RECHECK-1",
        "กมลชนก",
        "สุขสวัสดิ์",
        "inactive",
    )
    .await;
    sqlx::query("UPDATE users SET email = 'private-account@example.invalid', phone = '0800000000' WHERE id = $1")
        .bind(student_user_id)
        .execute(&pool)
        .await
        .unwrap();
    let imported = candidate_service::import_candidates(
        &pool,
        &actor,
        campaign.id,
        candidate_import_request(vec![candidate_import_row(
            "student",
            Some("S-RECHECK-1"),
            None,
            "กมลชนก",
            "สุขสวัสดิ์",
            "แบบเชื่อมบัญชีนักเรียน",
        )]),
    )
    .await
    .unwrap();
    let inactive = &imported.candidates[0];
    assert_eq!(inactive.match_status, CandidateMatchStatus::Inactive);
    sqlx::query("UPDATE users SET status = 'active' WHERE id = $1")
        .bind(student_user_id)
        .execute(&pool)
        .await
        .unwrap();
    let updated = candidate_service::update_candidate(
        &pool,
        &actor,
        inactive.id,
        UpdateCertificateCandidateRequest {
            expected_updated_at: inactive.updated_at,
            template_id: Some(template.id),
            recipient_type: RecipientType::Student,
            student_id: Some("S-RECHECK-1".to_string()),
            staff_username: None,
            imported_title: None,
            imported_first_name: "กมลชนก".to_string(),
            imported_last_name: "สุขสวัสดิ์".to_string(),
            selected_name_source: Some(CandidateNameSource::Account),
            activity_item: Some("การแข่งขันเขียนคำคม".to_string()),
            award_or_role: Some("รางวัลชนะเลิศ".to_string()),
            custom_values: BTreeMap::new(),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.match_status, CandidateMatchStatus::Matched);
    assert_eq!(updated.validation_status, CandidateValidationStatus::Ready);
    assert!(candidate_service::update_candidate(
        &pool,
        &actor,
        updated.id,
        UpdateCertificateCandidateRequest {
            expected_updated_at: updated.updated_at,
            template_id: Some(template.id),
            recipient_type: RecipientType::Student,
            student_id: Some("S-RECHECK-1".to_string()),
            staff_username: None,
            imported_title: None,
            imported_first_name: "กมลชนก".to_string(),
            imported_last_name: "สุขสวัสดิ์".to_string(),
            selected_name_source: Some(CandidateNameSource::Account),
            activity_item: updated.activity_item.clone(),
            award_or_role: updated.award_or_role.clone(),
            custom_values: BTreeMap::from([("ตัวแปรที่ไม่เคยประกาศ".to_string(), "ค่า".to_string(),)]),
        },
    )
    .await
    .is_err());

    let accounts = candidate_service::search_accounts(
        &pool,
        &actor,
        campaign.id,
        CertificateAccountSearchQuery {
            recipient_type: RecipientType::Student,
            search: "กมลชนก".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(accounts.len(), 1);
    let serialized = serde_json::to_value(&accounts[0]).unwrap();
    let keys = serialized
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        BTreeSet::from([
            "userId",
            "recipientType",
            "studentId",
            "staffUsername",
            "title",
            "firstName",
            "lastName",
        ])
    );
    assert!(!serialized.to_string().contains("private-account"));
    assert!(!serialized.to_string().contains("0800000000"));
    assert!(candidate_service::search_accounts(
        &pool,
        &actor,
        campaign.id,
        CertificateAccountSearchQuery {
            recipient_type: RecipientType::Student,
            search: "0-0000-00000-00-0".to_string(),
        },
    )
    .await
    .is_err());

    let from_account = candidate_service::create_account_candidate(
        &pool,
        &actor,
        campaign.id,
        CreateAccountCertificateCandidateRequest {
            user_id: student_user_id,
            template_id: Some(template.id),
            activity_item: Some("อบรมภาษาไทย".to_string()),
            award_or_role: Some("ผู้เข้าร่วม".to_string()),
            custom_values: BTreeMap::new(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        from_account.candidates[0].selected_name_source,
        Some(CandidateNameSource::Account)
    );
    assert_eq!(
        from_account.candidates[0].validation_status,
        CandidateValidationStatus::Ready
    );
    let deleted = candidate_service::delete_candidate(&pool, &actor, from_account.candidates[0].id)
        .await
        .unwrap();
    assert!(deleted.deleted_at.is_some());
    let still_persisted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM certificate_candidates WHERE id = $1 AND deleted_at IS NOT NULL)",
    )
    .bind(deleted.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        still_persisted,
        "candidate deletion must preserve request history"
    );
}
