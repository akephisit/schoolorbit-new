use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    env,
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
            CertificateFontStyle, CertificateImportRequest, CertificateImportRowInput,
            CertificateImportSource, CertificateIssueCode, CertificateIssueRequestListQuery,
            CertificateIssueRequestStatus, CertificateLayoutV1, CertificatePreviewKind,
            CertificatePreviewManifestRequest, CertificateRenderManifestBatchRequest,
            CertificateStatus, CertificateTemplateAssetKind, CertificateTemplateDeleteDisposition,
            ChangeCertificateCampaignStatusRequest, CreateAccountCertificateCandidateRequest,
            CreateCertificateCampaignRequest, CreateCertificateTemplateRequest,
            CreateManualExternalCandidateRequest, ElementFrame, GeometryAction, ImageElement,
            IssueCertificateOutcome, IssueCertificateRequest, IssuedCertificateSummary,
            ManualCertificateVerificationRequest, NullableUuidUpdate,
            QrCertificateVerificationRequest, RecipientType, RevokeCertificateRequest,
            TextAlignment, TextElement, UpdateCertificateCampaignRequest,
            UpdateCertificateCandidateRequest, UpdateCertificateTemplateRequest,
        },
        services::{
            campaign_service, candidate_service, issuance_service, render_service, request_service,
            template_service, verification_service,
        },
    },
    permissions::registry::codes,
    policies::{
        certificate_access_policy::{require_owner_action, CertificateAction},
        file_access_policy::{self, FilePolicyAction},
        resource_access_policy::accessible_exact_units_for_permission,
    },
    test_helpers::{
        create_named_test_pool, create_named_test_pool_with_max_connections, create_test_user,
        run_test_migrations,
    },
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

struct BlockingGrantStorage {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl crate::modules::files::storage_provider::StorageProvider for BlockingGrantStorage {
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
        self.entered.notify_one();
        self.release.notified().await;
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
            font_style: CertificateFontStyle::Normal,
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
async fn certificate_layout_contract_persists_explicit_font_and_image_fields() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_layout_contract", 3160).await;
    let template = create_template_fixture(&pool, &actor, academic_year_id).await;
    let background_file_id = insert_ready_template_file(
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
            file_id: background_file_id,
            geometry_action: GeometryAction::Preserve,
            preview_confirmed: true,
        },
    )
    .await
    .unwrap();
    let image_file_id = insert_ready_template_file(
        &pool,
        &actor,
        template.id,
        "certificate_template_image",
        serde_json::json!({"kind": "image", "width_px": 800, "height_px": 400}),
    )
    .await;
    let with_image = template_service::attach_asset(
        &pool,
        &actor,
        template.id,
        AttachCertificateAssetRequest {
            file_id: image_file_id,
            kind: CertificateTemplateAssetKind::Image,
            display_name: "ตราสัญลักษณ์".to_string(),
            font_weight: None,
            rights_confirmed: false,
        },
    )
    .await
    .unwrap();
    let image_asset_id = with_image.assets[0].id;

    let mut layout = text_layout(CertificateFontSource::BuiltIn);
    layout
        .elements
        .push(CertificateElement::Image(ImageElement {
            id: Uuid::new_v4(),
            frame: ElementFrame {
                x: 300.0,
                y: 30.0,
                width: 160.0,
                height: 80.0,
            },
            rotation: 0.0,
            asset_id: image_asset_id,
            lock_aspect_ratio: true,
            aspect_ratio: 2.0,
        }));
    let updated = template_service::update_template(
        &pool,
        &actor,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: with_image.updated_at,
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
    let CertificateElement::Text(text) = &updated.layout.elements[0] else {
        panic!("expected text")
    };
    assert_eq!(text.font_style, CertificateFontStyle::Normal);
    let CertificateElement::Image(image) = &updated.layout.elements[1] else {
        panic!("expected image")
    };
    assert!(image.lock_aspect_ratio);
    assert_eq!(image.aspect_ratio, 2.0);
    assert_eq!((image.frame.width, image.frame.height), (160.0, 80.0));

    let persisted: serde_json::Value =
        sqlx::query_scalar("SELECT layout FROM certificate_templates WHERE id = $1")
            .bind(template.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(persisted["elements"][0]["fontStyle"], "normal");
    assert_eq!(persisted["elements"][1]["lockAspectRatio"], true);
    assert_eq!(persisted["elements"][1]["aspectRatio"], 2.0);
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
    assert!(matches!(
        locked,
        Err(AppError::CertificateResourceLocked {
            request_id: Some(locked_request_id)
        }) if locked_request_id == request_id
    ));
    assert!(matches!(
        template_service::delete_template(&pool, &actor, selected.id).await,
        Err(AppError::CertificateResourceLocked {
            request_id: Some(locked_request_id)
        }) if locked_request_id == request_id
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
    let mut mismatched_layout = layout.clone();
    let CertificateElement::Text(mismatched_text) = &mut mismatched_layout.elements[0] else {
        panic!("expected text element")
    };
    mismatched_text.font_style = CertificateFontStyle::Italic;
    assert!(matches!(
        template_service::update_template(
            &pool,
            &actor,
            template.id,
            UpdateCertificateTemplateRequest {
                expected_updated_at: with_font.updated_at,
                name: None,
                allowed_recipient_types: None,
                safe_margin_points: None,
                show_safe_area: None,
                layout: Some(mismatched_layout),
                is_active: None,
                confirm_missing_issued_values: false,
            },
        )
        .await,
        Err(AppError::Conflict(_))
    ));
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
async fn candidate_preparation_capability_uses_the_exact_campaign_owner_scope() {
    let mut fixture =
        CertificatePolicyFixture::with_position_grant("certificate_candidate_capability", "head")
            .await;
    fixture
        .actor
        .permissions
        .push(codes::CERTIFICATE_READ_SCHOOL.to_string());
    let academic_year_id = insert_academic_year(&fixture.pool, 3136).await;
    let mut campaign_ids = Vec::new();
    for (owner_id, name) in [
        (fixture.unit_a, "กิจกรรมในขอบเขต"),
        (fixture.unit_b, "กิจกรรมนอกขอบเขต"),
    ] {
        let campaign_id: Uuid = sqlx::query_scalar(
            "INSERT INTO certificate_campaigns
                (academic_year_id, owner_organization_unit_id, name, event_date,
                 created_by, updated_by)
             VALUES ($1, $2, $3, CURRENT_DATE, $4, $4)
             RETURNING id",
        )
        .bind(academic_year_id)
        .bind(owner_id)
        .bind(name)
        .bind(fixture.actor.user_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        campaign_ids.push(campaign_id);
    }

    let allowed = campaign_service::get_campaign(&fixture.pool, &fixture.actor, campaign_ids[0])
        .await
        .unwrap();
    let denied = campaign_service::get_campaign(&fixture.pool, &fixture.actor, campaign_ids[1])
        .await
        .unwrap();
    assert!(allowed.capabilities.can_prepare_candidates);
    assert!(!denied.capabilities.can_prepare_candidates);
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
    school_campaign_fixture_in_pool(pool, test_name, year).await
}

async fn concurrent_school_campaign_fixture(
    test_name: &str,
    year: i32,
) -> (PgPool, ActorContext, Uuid) {
    let pool = create_named_test_pool_with_max_connections(test_name, 3).await;
    school_campaign_fixture_in_pool(pool, test_name, year).await
}

async fn school_campaign_fixture_in_pool(
    pool: PgPool,
    test_name: &str,
    year: i32,
) -> (PgPool, ActorContext, Uuid) {
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

    let request_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_issue_requests (campaign_id, submitted_by)
         VALUES ($1, $2)
         RETURNING id",
    )
    .bind(campaign.id)
    .bind(actor.user_id)
    .fetch_one(&pool)
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
    assert!(matches!(
        locked,
        Err(AppError::CertificateResourceLocked {
            request_id: Some(locked_request_id)
        }) if locked_request_id == request_id
    ));
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
async fn concurrent_account_creation_is_ordered_before_external_confirmation() {
    let (pool, actor, academic_year_id) =
        concurrent_school_campaign_fixture("certificate_candidate_external_account_race", 3137)
            .await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมตรวจบัญชีที่สร้างพร้อมกัน"),
    )
    .await
    .unwrap();
    create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบตรวจบัญชีที่สร้างพร้อมกัน",
        vec![RecipientType::Student, RecipientType::External],
    )
    .await;
    let imported = candidate_service::import_candidates(
        &pool,
        &actor,
        campaign.id,
        candidate_import_request(vec![candidate_import_row(
            "student",
            Some("S-CONCURRENT-ACCOUNT"),
            None,
            "บัญชี",
            "กำลังสร้าง",
            "แบบตรวจบัญชีที่สร้างพร้อมกัน",
        )]),
    )
    .await
    .unwrap();
    let candidate_id = imported.candidates[0].id;
    let account_user_id = create_test_user(
        &pool,
        "certificate-concurrent-account@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let mut account_creation = pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE users
         SET username = 'student-concurrent-account', user_type = 'student',
             title = 'เด็กหญิง', first_name = 'บัญชี', last_name = 'กำลังสร้าง',
             status = 'active'
         WHERE id = $1",
    )
    .bind(account_user_id)
    .execute(&mut *account_creation)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO student_info (user_id, student_id)
         VALUES ($1, 'S-CONCURRENT-ACCOUNT')",
    )
    .bind(account_user_id)
    .execute(&mut *account_creation)
    .await
    .unwrap();

    let confirm_pool = pool.clone();
    let confirm_actor = actor.clone();
    let confirmation = tokio::spawn(async move {
        candidate_service::confirm_external(&confirm_pool, &confirm_actor, candidate_id).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !confirmation.is_finished(),
        "external confirmation should wait for an in-flight matching account creation"
    );
    account_creation.commit().await.unwrap();

    let result = tokio::time::timeout(Duration::from_secs(5), confirmation)
        .await
        .expect("external confirmation should finish after account creation commits")
        .unwrap();
    assert!(matches!(result, Err(AppError::Conflict(_))));
    let persisted = candidate_service::get_candidate(&pool, &actor, candidate_id)
        .await
        .unwrap();
    assert_eq!(persisted.recipient_type, RecipientType::Student);
    assert_eq!(persisted.match_status, CandidateMatchStatus::NotFound);
}

#[tokio::test]
async fn account_search_requires_update_permission_for_school_and_exact_unit_scopes() {
    let fixture = CertificatePolicyFixture::new("certificate_account_search_permission").await;
    let academic_year_id = insert_academic_year(&fixture.pool, 3139).await;
    let campaign_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_campaigns
            (academic_year_id, owner_organization_unit_id, name, event_date,
             created_by, updated_by)
         VALUES ($1, $2, 'กิจกรรมค้นหาบัญชี', CURRENT_DATE, $3, $3)
         RETURNING id",
    )
    .bind(academic_year_id)
    .bind(fixture.unit_a)
    .bind(fixture.actor.user_id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    insert_certificate_student(
        &fixture.pool,
        "student-account-search",
        "S-ACCOUNT-SEARCH",
        "กมลชนก",
        "สุขสวัสดิ์",
        "active",
    )
    .await;

    let school_reader = ActorContext {
        user_id: fixture.actor.user_id,
        permissions: vec![codes::CERTIFICATE_READ_SCHOOL.to_string()],
    };
    let school_read_error = candidate_service::search_accounts(
        &fixture.pool,
        &school_reader,
        campaign_id,
        CertificateAccountSearchQuery {
            recipient_type: RecipientType::Student,
            search: "กมลชนก".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(school_read_error, AppError::Forbidden(_)));

    add_exact_grant(
        &fixture.pool,
        fixture.unit_a,
        codes::CERTIFICATE_READ_ORGANIZATION_UNIT,
    )
    .await;
    let exact_unit_reader = ActorContext {
        user_id: fixture.actor.user_id,
        permissions: vec![codes::CERTIFICATE_READ_ORGANIZATION_UNIT.to_string()],
    };
    let exact_read_error = candidate_service::search_accounts(
        &fixture.pool,
        &exact_unit_reader,
        campaign_id,
        CertificateAccountSearchQuery {
            recipient_type: RecipientType::Student,
            search: "กมลชนก".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(exact_read_error, AppError::Forbidden(_)));

    add_exact_grant(
        &fixture.pool,
        fixture.unit_a,
        codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
    )
    .await;
    let exact_unit_updater = ActorContext {
        user_id: fixture.actor.user_id,
        permissions: vec![codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT.to_string()],
    };
    let exact_matches = candidate_service::search_accounts(
        &fixture.pool,
        &exact_unit_updater,
        campaign_id,
        CertificateAccountSearchQuery {
            recipient_type: RecipientType::Student,
            search: "กมลชนก".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(exact_matches.len(), 1);

    let school_updater = ActorContext {
        user_id: fixture.actor.user_id,
        permissions: vec![codes::CERTIFICATE_UPDATE_SCHOOL.to_string()],
    };
    let school_matches = candidate_service::search_accounts(
        &fixture.pool,
        &school_updater,
        campaign_id,
        CertificateAccountSearchQuery {
            recipient_type: RecipientType::Student,
            search: "กมลชนก".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(school_matches.len(), 1);
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

async fn create_ready_external_request_candidate(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    template_name: &str,
    first_name: &str,
) -> (
    crate::modules::certificates::models::CertificateTemplateDetail,
    crate::modules::certificates::models::CertificateCandidateDetail,
) {
    let template = create_ready_candidate_template(
        pool,
        actor,
        campaign_id,
        template_name,
        vec![RecipientType::External],
    )
    .await;
    let created = candidate_service::create_manual_external(
        pool,
        actor,
        campaign_id,
        CreateManualExternalCandidateRequest {
            template_id: Some(template.id),
            title: Some("คุณ".to_string()),
            first_name: first_name.to_string(),
            last_name: "ผู้รับ".to_string(),
            activity_item: Some("กิจกรรมทดสอบคำขอ".to_string()),
            award_or_role: Some("ผู้เข้าร่วม".to_string()),
            custom_values: BTreeMap::new(),
        },
    )
    .await
    .unwrap();
    (template, created.candidates[0].clone())
}

fn update_external_candidate_payload(
    candidate: &crate::modules::certificates::models::CertificateCandidateDetail,
    first_name: &str,
) -> UpdateCertificateCandidateRequest {
    UpdateCertificateCandidateRequest {
        expected_updated_at: candidate.updated_at,
        template_id: candidate.template_id,
        recipient_type: RecipientType::External,
        student_id: None,
        staff_username: None,
        imported_title: candidate.imported_title.clone(),
        imported_first_name: first_name.to_string(),
        imported_last_name: candidate.imported_last_name.clone(),
        selected_name_source: Some(CandidateNameSource::File),
        activity_item: candidate.activity_item.clone(),
        award_or_role: candidate.award_or_role.clone(),
        custom_values: candidate.custom_values.clone(),
    }
}

#[tokio::test]
async fn active_request_locks_only_selected_candidates_and_referenced_templates() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_request_resource_locks", 3129).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมทดสอบการล็อกคำขอ"),
    )
    .await
    .unwrap();
    let (selected_template, selected) = create_ready_external_request_candidate(
        &pool,
        &actor,
        campaign.id,
        "แบบที่ส่งตรวจ",
        "ผู้รับที่เลือก",
    )
    .await;
    let (unselected_template, unselected) = create_ready_external_request_candidate(
        &pool,
        &actor,
        campaign.id,
        "แบบที่ยังไม่ส่ง",
        "ผู้รับที่ยังไม่เลือก",
    )
    .await;

    let request =
        request_service::submit_issue_request(&pool, &actor, campaign.id, vec![selected.id])
            .await
            .unwrap();
    assert_eq!(request.status, CertificateIssueRequestStatus::Pending);

    assert!(matches!(
        request_service::submit_issue_request(
            &pool,
            &actor,
            campaign.id,
            vec![selected.id],
        )
        .await,
        Err(AppError::CertificateResourceLocked {
            request_id: Some(request_id)
        }) if request_id == request.id
    ));
    let submit_without_read = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::CERTIFICATE_SUBMIT_SCHOOL.to_string()],
    };
    assert!(matches!(
        request_service::submit_issue_request(
            &pool,
            &submit_without_read,
            campaign.id,
            vec![selected.id],
        )
        .await,
        Err(AppError::CertificateResourceLocked { request_id: None })
    ));

    assert!(matches!(
        candidate_service::update_candidate(
            &pool,
            &actor,
            selected.id,
            update_external_candidate_payload(&selected, "ห้ามแก้"),
        )
        .await,
        Err(AppError::CertificateResourceLocked {
            request_id: Some(request_id)
        }) if request_id == request.id
    ));
    assert!(matches!(
        template_service::update_template(
            &pool,
            &actor,
            selected_template.id,
            UpdateCertificateTemplateRequest {
                expected_updated_at: selected_template.updated_at,
                name: Some("ห้ามแก้แบบ".to_string()),
                allowed_recipient_types: None,
                safe_margin_points: None,
                show_safe_area: None,
                layout: None,
                is_active: None,
                confirm_missing_issued_values: false,
            },
        )
        .await,
        Err(AppError::CertificateResourceLocked {
            request_id: Some(request_id)
        }) if request_id == request.id
    ));
    assert!(matches!(
        campaign_service::update_campaign(
            &pool,
            &actor,
            campaign.id,
            UpdateCertificateCampaignRequest {
                expected_updated_at: campaign.updated_at,
                academic_year_id: None,
                owner_organization_unit_id: None,
                name: Some("ห้ามแก้ข้อมูลร่วม".to_string()),
                event_date: None,
                confirm_affects_issued_certificates: false,
            },
        )
        .await,
        Err(AppError::CertificateResourceLocked {
            request_id: Some(request_id)
        }) if request_id == request.id
    ));

    let updated_candidate = candidate_service::update_candidate(
        &pool,
        &actor,
        unselected.id,
        update_external_candidate_payload(&unselected, "แก้รายการที่ไม่เลือกได้"),
    )
    .await
    .unwrap();
    assert_eq!(updated_candidate.imported_first_name, "แก้รายการที่ไม่เลือกได้");
    let updated_template = template_service::update_template(
        &pool,
        &actor,
        unselected_template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: unselected_template.updated_at,
            name: Some("แก้แบบที่ไม่ถูกอ้างอิงได้".to_string()),
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
    assert_eq!(updated_template.template.name, "แก้แบบที่ไม่ถูกอ้างอิงได้");

    let locked_candidate = candidate_service::get_candidate(&pool, &actor, selected.id)
        .await
        .unwrap();
    let locked_template = template_service::get_template(&pool, &actor, selected_template.id)
        .await
        .unwrap();
    let locked_campaign = campaign_service::get_campaign(&pool, &actor, campaign.id)
        .await
        .unwrap();
    assert!(!locked_candidate.capabilities.can_update);
    assert!(!locked_template.capabilities.can_update);
    assert!(locked_campaign.capabilities.can_submit);
    assert!(locked_campaign.capabilities.can_prepare_candidates);
}

fn school_certificate_issuer(user_id: Uuid) -> ActorContext {
    ActorContext {
        user_id,
        permissions: vec![codes::CERTIFICATE_ISSUE_SCHOOL.to_string()],
    }
}

async fn issue_public_verification_fixture(
    test_name: &str,
    year: i32,
) -> (
    PgPool,
    crate::modules::certificates::models::IssuedCertificateSummary,
    Uuid,
) {
    let (pool, preparer, academic_year_id) = school_campaign_fixture(test_name, year).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมตรวจสอบสาธารณะ"),
    )
    .await
    .unwrap();
    let template = create_ready_candidate_template(
        &pool,
        &preparer,
        campaign.id,
        "แบบตรวจสอบสาธารณะ",
        vec![RecipientType::External],
    )
    .await;
    let mut candidate_row =
        candidate_import_row("external", None, None, "กมล", "ผู้รับ", "แบบตรวจสอบสาธารณะ");
    candidate_row.title = Some("คุณ".to_string());
    candidate_row.custom_values = BTreeMap::from([
        ("แสดงผล".to_string(), "เผยแพร่ได้".to_string()),
        ("ไม่แสดง".to_string(), "ต้องไม่เผยแพร่".to_string()),
    ]);
    let mut import_request = candidate_import_request(vec![candidate_row]);
    import_request
        .headers
        .extend(["แสดงผล".to_string(), "ไม่แสดง".to_string()]);
    let imported =
        candidate_service::import_candidates(&pool, &preparer, campaign.id, import_request)
            .await
            .unwrap();
    let candidate = &imported.candidates[0];
    let mut public_layout = text_layout(CertificateFontSource::BuiltIn);
    let CertificateElement::Text(text) = &mut public_layout.elements[0] else {
        panic!("public render fixture must use a text element");
    };
    text.content = "มอบให้ {ชื่อ} {แสดงผล}".to_string();
    template_service::update_template(
        &pool,
        &preparer,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: template.updated_at,
            name: None,
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: Some(public_layout),
            is_active: None,
            confirm_missing_issued_values: false,
        },
    )
    .await
    .unwrap();
    let request =
        request_service::submit_issue_request(&pool, &preparer, campaign.id, vec![candidate.id])
            .await
            .unwrap();
    let issuer_user_id = create_test_user(
        &pool,
        &format!("{test_name}-issuer@example.invalid"),
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    request_service::start_review(&pool, &issuer, request.id)
        .await
        .unwrap();
    let issued = match issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนตัวอย่าง".to_string(),
        request.id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap()
    {
        IssueCertificateOutcome::Issued { certificates, .. } => certificates[0].clone(),
        IssueCertificateOutcome::Returned { .. } => panic!("verification fixture should issue"),
    };
    (pool, issued, Uuid::new_v4())
}

#[tokio::test]
async fn public_verification_failures_share_one_status_and_shape() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-public-verification-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-public-verification-blind-index-test-key",
    );
    let (pool, issued, tenant_id) =
        issue_public_verification_fixture("certificate_public_verification_generic", 3149).await;
    let cases = [
        verification_service::CertificateVerificationAttempt::Manual(
            ManualCertificateVerificationRequest {
                certificate_number: "0000-0000-000000-0".to_string(),
                first_name: "กมล".to_string(),
                last_name: "ผู้รับ".to_string(),
            },
        ),
        verification_service::CertificateVerificationAttempt::Manual(
            ManualCertificateVerificationRequest {
                certificate_number: issued.certificate_number.clone(),
                first_name: "ชื่อผิด".to_string(),
                last_name: "ผู้รับ".to_string(),
            },
        ),
        verification_service::CertificateVerificationAttempt::Manual(
            ManualCertificateVerificationRequest {
                certificate_number: issued.certificate_number.clone(),
                first_name: "กมล".to_string(),
                last_name: "นามสกุลผิด".to_string(),
            },
        ),
        verification_service::CertificateVerificationAttempt::Qr(
            QrCertificateVerificationRequest {
                certificate_number: issued.certificate_number.clone(),
                proof: "invalid-proof".to_string(),
            },
        ),
    ];

    for attempt in cases {
        let error = verification_service::verify(&pool, tenant_id, attempt)
            .await
            .unwrap_err();
        assert_eq!(error.status_code(), axum::http::StatusCode::NOT_FOUND);
        assert_eq!(error.public_message(), "ไม่พบข้อมูลที่ตรงกัน");
    }
}

#[tokio::test]
async fn public_verification_rate_limits_after_six_failed_attempts_for_one_target() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-public-rate-limit-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-public-rate-limit-blind-index-test-key",
    );
    let (pool, issued, tenant_id) =
        issue_public_verification_fixture("certificate_public_rate_limit", 3153).await;
    let limiter =
        crate::modules::certificates::verification_limiter::CertificateVerificationLimiter::new();
    let client_ip = "198.51.100.42".parse().unwrap();

    for _ in 0..6 {
        let error = verification_service::verify_rate_limited(
            &pool,
            tenant_id,
            client_ip,
            &limiter,
            verification_service::CertificateVerificationAttempt::Manual(
                ManualCertificateVerificationRequest {
                    certificate_number: issued.certificate_number.clone(),
                    first_name: "ชื่อผิด".to_string(),
                    last_name: "ผู้รับ".to_string(),
                },
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(error.status_code(), axum::http::StatusCode::NOT_FOUND);
        assert_eq!(error.public_message(), "ไม่พบข้อมูลที่ตรงกัน");
    }

    let limited = verification_service::verify_rate_limited(
        &pool,
        tenant_id,
        client_ip,
        &limiter,
        verification_service::CertificateVerificationAttempt::Manual(
            ManualCertificateVerificationRequest {
                certificate_number: issued.certificate_number,
                first_name: "กมล".to_string(),
                last_name: "ผู้รับ".to_string(),
            },
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(
        limited.status_code(),
        axum::http::StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn public_verification_success_is_allowlisted_and_issues_a_short_lived_receipt() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-public-success-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-public-success-blind-index-test-key",
    );
    let (pool, issued, tenant_id) =
        issue_public_verification_fixture("certificate_public_verification_success", 3150).await;
    let verified = verification_service::verify(
        &pool,
        tenant_id,
        verification_service::CertificateVerificationAttempt::Manual(
            ManualCertificateVerificationRequest {
                certificate_number: issued.certificate_number.clone(),
                first_name: "  กมล  ".to_string(),
                last_name: " ผู้รับ ".to_string(),
            },
        ),
    )
    .await
    .unwrap();
    assert_eq!(verified.status, CertificateStatus::Issued);
    assert_eq!(verified.certificate_number, issued.certificate_number);
    assert_eq!(verified.first_name, "กมล");
    assert_eq!(verified.last_name, "ผู้รับ");
    assert_eq!(verified.campaign_name, "กิจกรรมตรวจสอบสาธารณะ");
    assert_eq!(verified.academic_year, 3150);
    assert_eq!(verified.template_name, "แบบตรวจสอบสาธารณะ");
    assert_eq!(verified.issuer_school_name, "โรงเรียนตัวอย่าง");
    let receipt = verified.receipt.as_deref().expect("issued receipt");
    assert!(!receipt.is_empty());
    let receipt_expires_at = verified.receipt_expires_at.expect("receipt expiry");
    let remaining = receipt_expires_at - chrono::Utc::now();
    assert!(remaining > chrono::Duration::minutes(4));
    assert!(remaining <= chrono::Duration::minutes(5));

    let serialized = serde_json::to_value(&verified).unwrap();
    let keys = serialized
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        BTreeSet::from([
            "academicYear",
            "activityItem",
            "awardOrRole",
            "campaignName",
            "certificateNumber",
            "firstName",
            "issueDate",
            "issuerSchoolName",
            "lastName",
            "receipt",
            "receiptExpiresAt",
            "replacementCertificateNumber",
            "status",
            "templateName",
            "title",
        ])
    );

    let encrypted_proof: String =
        sqlx::query_scalar("SELECT qr_proof_encrypted FROM certificates WHERE id = $1")
            .bind(issued.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let proof = crate::utils::field_encryption::decrypt(&encrypted_proof).unwrap();
    let qr_verified = verification_service::verify(
        &pool,
        tenant_id,
        verification_service::CertificateVerificationAttempt::Qr(
            QrCertificateVerificationRequest {
                certificate_number: issued.certificate_number.clone(),
                proof,
            },
        ),
    )
    .await
    .unwrap();
    assert_eq!(qr_verified.certificate_number, issued.certificate_number);
    assert_eq!(qr_verified.first_name, "กมล");
    assert!(qr_verified.receipt.is_some());
}

#[tokio::test]
async fn public_verification_reports_revoked_without_issuing_a_render_receipt() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-public-revoked-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-public-revoked-blind-index-test-key",
    );
    let (pool, issued, tenant_id) =
        issue_public_verification_fixture("certificate_public_verification_revoked", 3151).await;
    let revoker_user_id = create_test_user(
        &pool,
        "certificate-public-verification-revoker@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let revoker = ActorContext {
        user_id: revoker_user_id,
        permissions: vec![codes::CERTIFICATE_REVOKE_SCHOOL.to_string()],
    };
    issuance_service::revoke_certificate(
        &pool,
        &revoker,
        issued.id,
        RevokeCertificateRequest {
            reason: "ยืนยันว่าใบเดิมถูกเพิกถอน".to_string(),
            create_replacement_candidate: false,
        },
    )
    .await
    .unwrap();

    let verified = verification_service::verify(
        &pool,
        tenant_id,
        verification_service::CertificateVerificationAttempt::Manual(
            ManualCertificateVerificationRequest {
                certificate_number: issued.certificate_number,
                first_name: "กมล".to_string(),
                last_name: "ผู้รับ".to_string(),
            },
        ),
    )
    .await
    .unwrap();

    assert_eq!(verified.status, CertificateStatus::Revoked);
    assert!(verified.receipt.is_none());
    assert!(verified.receipt_expires_at.is_none());
}

#[tokio::test]
async fn public_render_manifest_requires_the_tenant_receipt_and_rechecks_revocation() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-public-render-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-public-render-blind-index-test-key",
    );
    let (pool, issued, tenant_id) =
        issue_public_verification_fixture("certificate_public_render", 3152).await;
    let verified = verification_service::verify(
        &pool,
        tenant_id,
        verification_service::CertificateVerificationAttempt::Manual(
            ManualCertificateVerificationRequest {
                certificate_number: issued.certificate_number.clone(),
                first_name: "กมล".to_string(),
                last_name: "ผู้รับ".to_string(),
            },
        ),
    )
    .await
    .unwrap();
    let receipt = verified.receipt.expect("issued receipt");
    let platform = crate::modules::files::platform_service::FilePlatform::new(
        Arc::new(PreviewStorage),
        Arc::new(PreviewScanner),
    );
    let limiter =
        crate::modules::certificates::verification_limiter::CertificateVerificationLimiter::new();
    let client_ip = "198.51.100.43".parse().unwrap();

    let manifest = render_service::public_manifest_rate_limited(
        &pool,
        &platform,
        "sandbox",
        "schoolorbit.test",
        tenant_id,
        client_ip,
        &limiter,
        &receipt,
    )
    .await
    .unwrap();
    assert_eq!(manifest.certificate_number, issued.certificate_number);
    assert_eq!(manifest.recipient_values["ชื่อ"], "กมล");
    assert_eq!(manifest.recipient_values["แสดงผล"], "เผยแพร่ได้");
    assert!(!manifest.recipient_values.contains_key("ไม่แสดง"));

    let wrong_tenant_error = render_service::public_manifest_rate_limited(
        &pool,
        &platform,
        "sandbox",
        "schoolorbit.test",
        Uuid::new_v4(),
        client_ip,
        &limiter,
        &receipt,
    )
    .await
    .unwrap_err();
    assert_eq!(
        wrong_tenant_error.status_code(),
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(wrong_tenant_error.public_message(), "ไม่พบข้อมูลที่ตรงกัน");

    let revoker_user_id = create_test_user(
        &pool,
        "certificate-public-render-revoker@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    issuance_service::revoke_certificate(
        &pool,
        &ActorContext {
            user_id: revoker_user_id,
            permissions: vec![codes::CERTIFICATE_REVOKE_SCHOOL.to_string()],
        },
        issued.id,
        RevokeCertificateRequest {
            reason: "เพิกถอนก่อนดาวน์โหลดสาธารณะ".to_string(),
            create_replacement_candidate: false,
        },
    )
    .await
    .unwrap();

    let revoked_error = render_service::public_manifest_rate_limited(
        &pool,
        &platform,
        "sandbox",
        "schoolorbit.test",
        tenant_id,
        client_ip,
        &limiter,
        &receipt,
    )
    .await
    .unwrap_err();
    assert_eq!(
        revoked_error.status_code(),
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(revoked_error.public_message(), "ไม่พบข้อมูลที่ตรงกัน");
}

struct OwnCertificateFixture {
    pool: PgPool,
    owner_user_id: Uuid,
    other_user_id: Uuid,
    owner_certificate: IssuedCertificateSummary,
    other_certificate: IssuedCertificateSummary,
    revoker: ActorContext,
}

async fn issue_own_certificate_fixture(test_name: &str, year: i32) -> OwnCertificateFixture {
    let (pool, preparer, academic_year_id) = school_campaign_fixture(test_name, year).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมคลังเกียรติบัตรส่วนตัว"),
    )
    .await
    .unwrap();
    let template = create_ready_candidate_template(
        &pool,
        &preparer,
        campaign.id,
        "แบบคลังเกียรติบัตรส่วนตัว",
        vec![RecipientType::Student],
    )
    .await;
    let owner_user_id = insert_certificate_student(
        &pool,
        &format!("{test_name}-owner"),
        &format!("{test_name}-owner-id"),
        "อรทัย",
        "เจ้าของใบ",
        "active",
    )
    .await;
    let other_user_id = insert_certificate_student(
        &pool,
        &format!("{test_name}-other"),
        &format!("{test_name}-other-id"),
        "บุษบา",
        "ผู้รับอีกคน",
        "active",
    )
    .await;
    let mut candidate_ids = Vec::new();
    for user_id in [owner_user_id, other_user_id] {
        let created = candidate_service::create_account_candidate(
            &pool,
            &preparer,
            campaign.id,
            CreateAccountCertificateCandidateRequest {
                user_id,
                template_id: Some(template.id),
                activity_item: Some("การแข่งขันทักษะ".to_string()),
                award_or_role: Some("ผู้เข้าร่วม".to_string()),
                custom_values: BTreeMap::new(),
            },
        )
        .await
        .unwrap();
        candidate_ids.push(created.candidates[0].id);
    }
    let request =
        request_service::submit_issue_request(&pool, &preparer, campaign.id, candidate_ids)
            .await
            .unwrap();
    let issuer_user_id = create_test_user(
        &pool,
        &format!("{test_name}-issuer@example.invalid"),
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    request_service::start_review(&pool, &issuer, request.id)
        .await
        .unwrap();
    let certificates = match issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนตัวอย่าง".to_string(),
        request.id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap()
    {
        IssueCertificateOutcome::Issued { certificates, .. } => certificates,
        IssueCertificateOutcome::Returned { .. } => panic!("own certificate fixture should issue"),
    };
    let owner_certificate = certificates
        .iter()
        .find(|certificate| certificate.first_name == "อรทัย")
        .cloned()
        .expect("owner certificate");
    let other_certificate = certificates
        .iter()
        .find(|certificate| certificate.first_name == "บุษบา")
        .cloned()
        .expect("other certificate");

    OwnCertificateFixture {
        pool,
        owner_user_id,
        other_user_id,
        owner_certificate,
        other_certificate,
        revoker: ActorContext {
            user_id: issuer_user_id,
            permissions: vec![codes::CERTIFICATE_REVOKE_SCHOOL.to_string()],
        },
    }
}

#[tokio::test]
async fn own_certificate_routes_cannot_read_another_linked_user() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-own-scope-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-own-scope-blind-index-test-key",
    );
    let fixture = issue_own_certificate_fixture("certificate_own_scope", 3154).await;

    let listed = issuance_service::list_own_certificates(&fixture.pool, fixture.owner_user_id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, fixture.owner_certificate.id);
    assert!(listed[0].capabilities.can_read);
    assert!(listed[0].capabilities.can_download);
    assert!(!listed[0].capabilities.can_revoke);

    let own_detail = issuance_service::get_own_certificate(
        &fixture.pool,
        fixture.owner_user_id,
        fixture.owner_certificate.id,
    )
    .await
    .unwrap();
    assert_eq!(own_detail.summary.id, fixture.owner_certificate.id);
    assert!(matches!(
        issuance_service::get_own_certificate(
            &fixture.pool,
            fixture.owner_user_id,
            fixture.other_certificate.id,
        )
        .await,
        Err(AppError::NotFound(_))
    ));

    let platform = crate::modules::files::platform_service::FilePlatform::new(
        Arc::new(PreviewStorage),
        Arc::new(PreviewScanner),
    );
    assert!(matches!(
        render_service::own_manifest(
            &fixture.pool,
            fixture.owner_user_id,
            &platform,
            "sandbox",
            "schoolorbit.test",
            fixture.other_certificate.id,
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert_ne!(fixture.owner_user_id, fixture.other_user_id);
}

#[tokio::test]
async fn own_revoked_certificate_stays_visible_but_cannot_render() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-own-revoked-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-own-revoked-blind-index-test-key",
    );
    let fixture = issue_own_certificate_fixture("certificate_own_revoked", 3155).await;
    issuance_service::revoke_certificate(
        &fixture.pool,
        &fixture.revoker,
        fixture.owner_certificate.id,
        RevokeCertificateRequest {
            reason: "เพิกถอนเพื่อทดสอบคลังส่วนตัว".to_string(),
            create_replacement_candidate: false,
        },
    )
    .await
    .unwrap();

    let listed = issuance_service::list_own_certificates(&fixture.pool, fixture.owner_user_id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].status, CertificateStatus::Revoked);
    assert!(!listed[0].capabilities.can_download);

    let platform = crate::modules::files::platform_service::FilePlatform::new(
        Arc::new(PreviewStorage),
        Arc::new(PreviewScanner),
    );
    assert!(matches!(
        render_service::own_manifest(
            &fixture.pool,
            fixture.owner_user_id,
            &platform,
            "sandbox",
            "schoolorbit.test",
            fixture.owner_certificate.id,
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn issue_request_transitions_release_locks_and_require_a_new_request_after_return() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_request_transitions", 3130).await;
    let reviewer_user_id = create_test_user(
        &pool,
        "certificate-request-reviewer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let other_submitter_user_id = create_test_user(
        &pool,
        "certificate-request-other-submitter@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let reviewer = school_certificate_issuer(reviewer_user_id);
    let other_submitter = ActorContext {
        user_id: other_submitter_user_id,
        permissions: vec![codes::CERTIFICATE_SUBMIT_SCHOOL.to_string()],
    };
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมทดสอบสถานะคำขอ"),
    )
    .await
    .unwrap();
    let (_, candidate) = create_ready_external_request_candidate(
        &pool,
        &actor,
        campaign.id,
        "แบบทดสอบสถานะคำขอ",
        "ผู้รับสถานะ",
    )
    .await;
    let request =
        request_service::submit_issue_request(&pool, &actor, campaign.id, vec![candidate.id])
            .await
            .unwrap();

    assert!(matches!(
        request_service::withdraw(&pool, &other_submitter, request.id).await,
        Err(AppError::Forbidden(_))
    ));
    assert!(matches!(
        request_service::start_review(&pool, &actor, request.id).await,
        Err(AppError::Forbidden(_))
    ));
    let reviewing = request_service::start_review(&pool, &reviewer, request.id)
        .await
        .unwrap();
    assert_eq!(reviewing.status, CertificateIssueRequestStatus::Reviewing);
    assert_eq!(reviewing.reviewed_by, Some(reviewer.user_id));
    assert!(request_service::withdraw(&pool, &actor, request.id)
        .await
        .is_err());

    for unsafe_note in [
        "พบเลขบัตรประชาชนในเอกสาร",
        "กรุณาตรวจ 1-2345-67890-12-3 อีกครั้ง",
        "NATIONAL-ID appears in the source file",
        "Please remove the Citizen ID column",
    ] {
        assert!(matches!(
            request_service::return_request(
                &pool,
                &reviewer,
                request.id,
                vec![CertificateIssueCode::ReviewerRequestedChanges],
                unsafe_note.to_string(),
            )
            .await,
            Err(AppError::ValidationError(_))
        ));
    }
    let returned = request_service::return_request(
        &pool,
        &reviewer,
        request.id,
        vec![CertificateIssueCode::ReviewerRequestedChanges],
        "  กรุณา   ตรวจข้อมูลผู้รับอีกครั้ง  ".to_string(),
    )
    .await
    .unwrap();
    assert_eq!(returned.status, CertificateIssueRequestStatus::Returned);
    assert_eq!(
        returned.return_note.as_deref(),
        Some("กรุณา ตรวจข้อมูลผู้รับอีกครั้ง")
    );
    assert_eq!(
        returned.issue_codes,
        vec![CertificateIssueCode::ReviewerRequestedChanges]
    );
    let active_lock_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM certificate_candidate_issue_locks WHERE request_id = $1",
    )
    .bind(request.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active_lock_count, 0);
    let audit_metadata: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT metadata FROM audit_logs
         WHERE entity_type = 'certificate_issue_request' AND entity_id = $1",
    )
    .bind(request.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(audit_metadata
        .iter()
        .all(|metadata| !metadata.to_string().contains("กรุณา ตรวจข้อมูลผู้รับอีกครั้ง")));

    let updated = candidate_service::update_candidate(
        &pool,
        &actor,
        candidate.id,
        update_external_candidate_payload(&candidate, "ผู้รับหลังส่งกลับ"),
    )
    .await
    .unwrap();
    let next_request =
        request_service::submit_issue_request(&pool, &actor, campaign.id, vec![updated.id])
            .await
            .unwrap();
    assert_ne!(next_request.id, request.id);
    let withdrawn = request_service::withdraw(&pool, &actor, next_request.id)
        .await
        .unwrap();
    assert_eq!(withdrawn.status, CertificateIssueRequestStatus::Withdrawn);
    assert!(request_service::start_review(&pool, &reviewer, request.id)
        .await
        .is_err());
}

#[tokio::test]
async fn pending_request_with_inactive_owner_can_still_be_withdrawn_by_its_scoped_submitter() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_request_inactive_owner_withdraw", 3133).await;
    let owner_unit = insert_unit(&pool, "certificate_request_withdraw_owner", None).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(
            academic_year_id,
            Some(owner_unit),
            "กิจกรรมที่หน่วยงานถูกปิดหลังส่ง",
        ),
    )
    .await
    .unwrap();
    let (_, candidate) = create_ready_external_request_candidate(
        &pool,
        &actor,
        campaign.id,
        "แบบสำหรับถอนคำขอ",
        "ผู้รับก่อนปิดหน่วยงาน",
    )
    .await;
    let request =
        request_service::submit_issue_request(&pool, &actor, campaign.id, vec![candidate.id])
            .await
            .unwrap();

    sqlx::query("UPDATE organization_units SET is_active = false WHERE id = $1")
        .bind(owner_unit)
        .execute(&pool)
        .await
        .unwrap();

    let withdrawn = request_service::withdraw(&pool, &actor, request.id)
        .await
        .unwrap();
    assert_eq!(withdrawn.status, CertificateIssueRequestStatus::Withdrawn);
}

#[tokio::test]
async fn issue_request_submission_revalidates_candidates_templates_and_active_owner() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_request_revalidation", 3131).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมตรวจซ้ำก่อนส่ง"),
    )
    .await
    .unwrap();
    let (template, candidate) = create_ready_external_request_candidate(
        &pool,
        &actor,
        campaign.id,
        "แบบตรวจซ้ำ",
        "ผู้รับตรวจซ้ำ",
    )
    .await;
    sqlx::query("UPDATE certificate_templates SET is_active = false WHERE id = $1")
        .bind(template.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        request_service::submit_issue_request(&pool, &actor, campaign.id, vec![candidate.id],)
            .await,
        Err(AppError::ValidationError(_))
    ));
    let request_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM certificate_issue_requests WHERE campaign_id = $1",
    )
    .bind(campaign.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(request_count, 0);

    let owner_unit = insert_unit(&pool, "certificate_request_inactive_owner", None).await;
    let owned_campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, Some(owner_unit), "กิจกรรมหน่วยงานที่ถูกปิด"),
    )
    .await
    .unwrap();
    let (_, owned_candidate) = create_ready_external_request_candidate(
        &pool,
        &actor,
        owned_campaign.id,
        "แบบหน่วยงานที่ถูกปิด",
        "ผู้รับหน่วยงาน",
    )
    .await;
    sqlx::query("UPDATE organization_units SET is_active = false WHERE id = $1")
        .bind(owner_unit)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        request_service::submit_issue_request(
            &pool,
            &actor,
            owned_campaign.id,
            vec![owned_candidate.id],
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    assert!(matches!(
        request_service::submit_issue_request(&pool, &actor, campaign.id, Vec::new()).await,
        Err(AppError::ValidationError(_))
    ));
}

#[tokio::test]
async fn concurrent_account_deactivation_is_ordered_before_issue_request_revalidation() {
    let (pool, actor, academic_year_id) =
        concurrent_school_campaign_fixture("certificate_request_account_deactivation_race", 3134)
            .await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมตรวจสถานะบัญชีพร้อมกัน"),
    )
    .await
    .unwrap();
    let template = create_ready_candidate_template(
        &pool,
        &actor,
        campaign.id,
        "แบบตรวจบัญชีพร้อมกัน",
        vec![RecipientType::Student],
    )
    .await;
    let student_user_id = insert_certificate_student(
        &pool,
        "student-request-race",
        "S-REQUEST-RACE",
        "กมลชนก",
        "พร้อมเรียน",
        "active",
    )
    .await;
    let imported = candidate_service::import_candidates(
        &pool,
        &actor,
        campaign.id,
        candidate_import_request(vec![candidate_import_row(
            "student",
            Some("S-REQUEST-RACE"),
            None,
            "กมลชนก",
            "พร้อมเรียน",
            &template.name,
        )]),
    )
    .await
    .unwrap();
    let candidate = imported.candidates[0].clone();
    assert_eq!(
        candidate.validation_status,
        CandidateValidationStatus::Ready
    );

    let mut deactivation = pool.begin().await.unwrap();
    sqlx::query("UPDATE users SET status = 'inactive' WHERE id = $1")
        .bind(student_user_id)
        .execute(&mut *deactivation)
        .await
        .unwrap();

    let campaign_id = campaign.id;
    let candidate_id = candidate.id;
    let submit_pool = pool.clone();
    let submit_actor = actor.clone();
    let submit = tokio::spawn(async move {
        request_service::submit_issue_request(
            &submit_pool,
            &submit_actor,
            campaign_id,
            vec![candidate_id],
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !submit.is_finished(),
        "submission should wait for an in-flight account status change"
    );
    deactivation.commit().await.unwrap();

    let result = tokio::time::timeout(Duration::from_secs(5), submit)
        .await
        .expect("submission should finish after account deactivation commits")
        .unwrap();
    assert!(matches!(result, Err(AppError::ValidationError(_))));
    let request_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM certificate_issue_requests WHERE campaign_id = $1",
    )
    .bind(campaign_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(request_count, 0);
}

#[tokio::test]
async fn concurrent_owner_deactivation_is_ordered_before_issue_request_submission() {
    let (pool, actor, academic_year_id) =
        concurrent_school_campaign_fixture("certificate_request_owner_deactivation_race", 3135)
            .await;
    let owner_unit = insert_unit(&pool, "certificate_request_owner_race", None).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(
            academic_year_id,
            Some(owner_unit),
            "กิจกรรมตรวจหน่วยงานพร้อมกัน",
        ),
    )
    .await
    .unwrap();
    let (_, candidate) = create_ready_external_request_candidate(
        &pool,
        &actor,
        campaign.id,
        "แบบตรวจหน่วยงานพร้อมกัน",
        "ผู้รับตรวจหน่วยงาน",
    )
    .await;

    let mut deactivation = pool.begin().await.unwrap();
    sqlx::query("UPDATE organization_units SET is_active = false WHERE id = $1")
        .bind(owner_unit)
        .execute(&mut *deactivation)
        .await
        .unwrap();

    let campaign_id = campaign.id;
    let candidate_id = candidate.id;
    let submit_pool = pool.clone();
    let submit_actor = actor.clone();
    let submit = tokio::spawn(async move {
        request_service::submit_issue_request(
            &submit_pool,
            &submit_actor,
            campaign_id,
            vec![candidate_id],
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !submit.is_finished(),
        "submission should wait for an in-flight owner status change"
    );
    deactivation.commit().await.unwrap();

    let result = tokio::time::timeout(Duration::from_secs(5), submit)
        .await
        .expect("submission should finish after owner deactivation commits")
        .unwrap();
    assert!(matches!(result, Err(AppError::ValidationError(_))));
    let request_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM certificate_issue_requests WHERE campaign_id = $1",
    )
    .bind(campaign_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(request_count, 0);
}

#[tokio::test]
async fn issue_request_lists_respect_campaign_read_and_school_issue_scopes() {
    let (pool, school_actor, academic_year_id) =
        school_campaign_fixture("certificate_request_list_scope", 3132).await;
    let unit_a = insert_unit(&pool, "certificate_request_list_a", None).await;
    let unit_b = insert_unit(&pool, "certificate_request_list_b", None).await;
    let mut requests = Vec::new();
    for (unit, suffix) in [(unit_a, "ก"), (unit_b, "ข")] {
        let campaign = campaign_service::create_campaign(
            &pool,
            &school_actor,
            campaign_create_payload(
                academic_year_id,
                Some(unit),
                &format!("กิจกรรมหน่วยงาน {suffix}"),
            ),
        )
        .await
        .unwrap();
        let (_, candidate) = create_ready_external_request_candidate(
            &pool,
            &school_actor,
            campaign.id,
            &format!("แบบหน่วยงาน {suffix}"),
            &format!("ผู้รับ {suffix}"),
        )
        .await;
        requests.push(
            request_service::submit_issue_request(
                &pool,
                &school_actor,
                campaign.id,
                vec![candidate.id],
            )
            .await
            .unwrap(),
        );
    }

    let reader_user_id = create_test_user(
        &pool,
        "certificate-request-reader@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO organization_members
            (user_id, organization_unit_id, position_code, started_at)
         VALUES ($1, $2, 'head', CURRENT_DATE)",
    )
    .bind(reader_user_id)
    .bind(unit_a)
    .execute(&pool)
    .await
    .unwrap();
    add_exact_grant(&pool, unit_a, codes::CERTIFICATE_READ_ORGANIZATION_UNIT).await;
    let reader = ActorContext {
        user_id: reader_user_id,
        permissions: vec![codes::CERTIFICATE_READ_ORGANIZATION_UNIT.to_string()],
    };
    let own_campaign_requests =
        request_service::list_campaign_requests(&pool, &reader, requests[0].campaign_id)
            .await
            .unwrap();
    assert_eq!(own_campaign_requests.len(), 1);
    assert!(matches!(
        request_service::list_campaign_requests(&pool, &reader, requests[1].campaign_id).await,
        Err(AppError::Forbidden(_))
    ));
    assert!(matches!(
        request_service::list_issue_queue(
            &pool,
            &reader,
            CertificateIssueRequestListQuery::default(),
        )
        .await,
        Err(AppError::Forbidden(_))
    ));

    let issuer_user_id = create_test_user(
        &pool,
        "certificate-request-list-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    let queue = request_service::list_issue_queue(
        &pool,
        &issuer,
        CertificateIssueRequestListQuery::default(),
    )
    .await
    .unwrap();
    assert_eq!(queue.len(), 2);
    let detail = request_service::get_issue_request(&pool, &issuer, requests[1].id)
        .await
        .unwrap();
    assert_eq!(detail.id, requests[1].id);
    assert_eq!(detail.items.len(), 1);
}

#[tokio::test]
async fn request_return_uses_request_before_campaign_lock_order() {
    let (pool, preparer, academic_year_id) =
        concurrent_school_campaign_fixture("certificate_request_return_lock_order", 3149).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมตรวจลำดับล็อก"),
    )
    .await
    .unwrap();
    let (_, candidate) = create_ready_external_request_candidate(
        &pool,
        &preparer,
        campaign.id,
        "แบบตรวจลำดับล็อก",
        "ผู้รับตรวจลำดับล็อก",
    )
    .await;
    let request =
        request_service::submit_issue_request(&pool, &preparer, campaign.id, vec![candidate.id])
            .await
            .unwrap();
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-request-lock-order-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    request_service::start_review(&pool, &issuer, request.id)
        .await
        .unwrap();

    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM certificate_issue_requests WHERE id = $1 FOR UPDATE")
        .bind(request.id)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();

    let return_pool = pool.clone();
    let return_actor = issuer.clone();
    let request_id = request.id;
    let returning = tokio::spawn(async move {
        request_service::return_request(
            &return_pool,
            &return_actor,
            request_id,
            vec![CertificateIssueCode::ReviewerRequestedChanges],
            "ปรับข้อมูลก่อนออกเกียรติบัตร".to_string(),
        )
        .await
    });

    let wait_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1
                 FROM pg_stat_activity
                 WHERE datname = current_database()
                   AND pid <> pg_backend_pid()
                   AND wait_event_type = 'Lock'
                   AND query LIKE '%FOR UPDATE OF request%'
             )",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if blocked {
            break;
        }
        assert!(
            tokio::time::Instant::now() < wait_deadline,
            "return transition did not reach its blocked request-row lock"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    sqlx::query("SET LOCAL lock_timeout = '250ms'")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let campaign_lock =
        sqlx::query("SELECT id FROM certificate_campaigns WHERE id = $1 FOR UPDATE")
            .bind(campaign.id)
            .fetch_one(&mut *blocker)
            .await;
    blocker.rollback().await.unwrap();
    let returned = tokio::time::timeout(Duration::from_secs(5), returning)
        .await
        .expect("return transition should finish after the request lock is released")
        .unwrap()
        .unwrap();
    assert_eq!(returned.status, CertificateIssueRequestStatus::Returned);
    assert!(
        campaign_lock.is_ok(),
        "return transition held the campaign while waiting for the request row"
    );
}

#[tokio::test]
async fn concurrent_first_issue_allocates_distinct_campaign_and_certificate_ranges() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-concurrent-issue-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-concurrent-issue-blind-index-test-key",
    );
    let (pool, preparer, academic_year_id) =
        concurrent_school_campaign_fixture("certificate_concurrent_issue", 3138).await;
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-concurrent-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);

    let mut reviewing_requests = Vec::new();
    for (campaign_name, recipient_count) in [
        ("กิจกรรมออกพร้อมกัน ก", 2_usize),
        ("กิจกรรมออกพร้อมกัน ข", 3_usize),
    ] {
        let campaign = campaign_service::create_campaign(
            &pool,
            &preparer,
            campaign_create_payload(academic_year_id, None, campaign_name),
        )
        .await
        .unwrap();
        let mut candidate_ids = Vec::new();
        for index in 0..recipient_count {
            let (_, candidate) = create_ready_external_request_candidate(
                &pool,
                &preparer,
                campaign.id,
                &format!("แบบ {campaign_name} {index}"),
                &format!("ผู้รับ {campaign_name} {index}"),
            )
            .await;
            candidate_ids.push(candidate.id);
        }
        let request =
            request_service::submit_issue_request(&pool, &preparer, campaign.id, candidate_ids)
                .await
                .unwrap();
        reviewing_requests.push(
            request_service::start_review(&pool, &issuer, request.id)
                .await
                .unwrap(),
        );
    }

    let issue_a = issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        reviewing_requests[0].id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    );
    let issue_b = issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        reviewing_requests[1].id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    );
    let (outcome_a, outcome_b) = tokio::join!(issue_a, issue_b);

    let issued_a = match outcome_a.unwrap() {
        IssueCertificateOutcome::Issued { certificates, .. } => certificates,
        IssueCertificateOutcome::Returned { .. } => panic!("first request should issue"),
    };
    let issued_b = match outcome_b.unwrap() {
        IssueCertificateOutcome::Issued { certificates, .. } => certificates,
        IssueCertificateOutcome::Returned { .. } => panic!("second request should issue"),
    };
    assert_eq!(issued_a.len(), 2);
    assert_eq!(issued_b.len(), 3);
    let numbers = issued_a
        .iter()
        .chain(&issued_b)
        .map(|certificate| certificate.certificate_number.clone())
        .collect::<HashSet<_>>();
    assert_eq!(numbers.len(), 5);
    let activities = issued_a
        .iter()
        .chain(&issued_b)
        .map(|certificate| certificate.activity_sequence)
        .collect::<HashSet<_>>();
    assert_eq!(activities.len(), 2);
}

#[tokio::test]
async fn draft_numbering_is_lazy_shared_across_runs_and_idempotent() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-shared-sequence-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-shared-sequence-blind-index-test-key",
    );
    let (pool, preparer, academic_year_id) =
        school_campaign_fixture("certificate_shared_issue_sequence", 3139).await;
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-shared-sequence-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมออกหลายรอบ"),
    )
    .await
    .unwrap();
    let initial_numbering: (Option<i32>, i32, i64) = sqlx::query_as(
        "SELECT campaign.activity_sequence, campaign.next_certificate_sequence,
                (SELECT COUNT(*) FROM certificate_academic_year_counters
                 WHERE academic_year_id = campaign.academic_year_id)
         FROM certificate_campaigns campaign WHERE campaign.id = $1",
    )
    .bind(campaign.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(initial_numbering, (None, 1, 0));

    let mut candidates = Vec::new();
    for (template_name, first_name) in [("แบบนักเรียน", "ผู้รับรอบแรก"), ("แบบวิทยากร", "ผู้รับรอบสอง")]
    {
        let (_, candidate) = create_ready_external_request_candidate(
            &pool,
            &preparer,
            campaign.id,
            template_name,
            first_name,
        )
        .await;
        candidates.push(candidate);
    }

    let first_request = request_service::submit_issue_request(
        &pool,
        &preparer,
        campaign.id,
        vec![candidates[0].id],
    )
    .await
    .unwrap();
    request_service::start_review(&pool, &issuer, first_request.id)
        .await
        .unwrap();
    let first_key = Uuid::new_v4();
    assert!(
        issuance_service::replay_issue_request(&pool, &issuer, first_request.id, first_key,)
            .await
            .unwrap()
            .is_none()
    );
    let first_outcome = issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        first_request.id,
        IssueCertificateRequest {
            idempotency_key: first_key,
        },
    )
    .await
    .unwrap();
    let replay =
        issuance_service::replay_issue_request(&pool, &issuer, first_request.id, first_key)
            .await
            .unwrap()
            .expect("completed issue request should replay before a school-name lookup");
    assert_eq!(replay, first_outcome);
    assert!(matches!(
        issuance_service::replay_issue_request(&pool, &issuer, first_request.id, Uuid::new_v4(),)
            .await,
        Err(AppError::Conflict(_))
    ));

    let second_request = request_service::submit_issue_request(
        &pool,
        &preparer,
        campaign.id,
        vec![candidates[1].id],
    )
    .await
    .unwrap();
    request_service::start_review(&pool, &issuer, second_request.id)
        .await
        .unwrap();
    let second_outcome = issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        second_request.id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();

    let issued = [first_outcome, second_outcome]
        .into_iter()
        .map(|outcome| match outcome {
            IssueCertificateOutcome::Issued { certificates, .. } => certificates[0].clone(),
            IssueCertificateOutcome::Returned { .. } => panic!("request should issue"),
        })
        .collect::<Vec<_>>();
    assert_eq!(issued[0].activity_sequence, issued[1].activity_sequence);
    assert_eq!(issued[0].certificate_sequence, 1);
    assert_eq!(issued[1].certificate_sequence, 2);
    assert_ne!(issued[0].template_id, issued[1].template_id);
    let final_numbering: (i32, i32, i32) = sqlx::query_as(
        "SELECT campaign.activity_sequence, campaign.next_certificate_sequence,
                counter.next_activity_sequence
         FROM certificate_campaigns campaign
         JOIN certificate_academic_year_counters counter
           ON counter.academic_year_id = campaign.academic_year_id
         WHERE campaign.id = $1",
    )
    .bind(campaign.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(final_numbering, (1, 3, 2));
}

#[tokio::test]
async fn issue_revalidation_returns_atomically_when_converted_external_gets_an_account() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-returned-issue-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-returned-issue-blind-index-test-key",
    );
    let (pool, preparer, academic_year_id) =
        school_campaign_fixture("certificate_issue_external_recheck", 3140).await;
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-external-recheck-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมตรวจบัญชีก่อนออก"),
    )
    .await
    .unwrap();
    let template = create_ready_candidate_template(
        &pool,
        &preparer,
        campaign.id,
        "แบบรองรับผู้รับภายนอก",
        vec![RecipientType::Student, RecipientType::External],
    )
    .await;
    let imported = candidate_service::import_candidates(
        &pool,
        &preparer,
        campaign.id,
        candidate_import_request(vec![candidate_import_row(
            "student",
            Some("S-LATE-ACCOUNT"),
            None,
            "กมลชนก",
            "เกิดบัญชีภายหลัง",
            &template.name,
        )]),
    )
    .await
    .unwrap();
    let converted = candidate_service::bulk_update_for_campaign(
        &pool,
        &preparer,
        campaign.id,
        CertificateCandidateBulkRequest::ConfirmExternal {
            candidate_ids: vec![imported.candidates[0].id],
        },
    )
    .await
    .unwrap()
    .candidates
    .remove(0);
    assert_eq!(converted.recipient_type, RecipientType::External);
    assert_eq!(
        converted.validation_status,
        CandidateValidationStatus::Ready
    );
    let deactivated_user_id = insert_certificate_student(
        &pool,
        "student-deactivated-during-review",
        "S-DEACTIVATED-DURING-REVIEW",
        "ผู้รับบัญชี",
        "ถูกปิดภายหลัง",
        "active",
    )
    .await;
    let deactivated_candidate = candidate_service::import_candidates(
        &pool,
        &preparer,
        campaign.id,
        candidate_import_request(vec![candidate_import_row(
            "student",
            Some("S-DEACTIVATED-DURING-REVIEW"),
            None,
            "ผู้รับบัญชี",
            "ถูกปิดภายหลัง",
            &template.name,
        )]),
    )
    .await
    .unwrap()
    .candidates
    .remove(0);
    assert_eq!(
        deactivated_candidate.validation_status,
        CandidateValidationStatus::Ready
    );
    let manual = candidate_service::create_manual_external(
        &pool,
        &preparer,
        campaign.id,
        CreateManualExternalCandidateRequest {
            template_id: Some(template.id),
            title: Some("คุณ".to_string()),
            first_name: "ผู้รับที่ข้อมูลไม่เปลี่ยน".to_string(),
            last_name: "ปกติ".to_string(),
            activity_item: None,
            award_or_role: Some("ผู้เข้าร่วม".to_string()),
            custom_values: BTreeMap::new(),
        },
    )
    .await
    .unwrap()
    .candidates
    .remove(0);
    let request = request_service::submit_issue_request(
        &pool,
        &preparer,
        campaign.id,
        vec![converted.id, deactivated_candidate.id, manual.id],
    )
    .await
    .unwrap();
    request_service::start_review(&pool, &issuer, request.id)
        .await
        .unwrap();

    insert_certificate_student(
        &pool,
        "student-late-account",
        "S-LATE-ACCOUNT",
        "กมลชนก",
        "เกิดบัญชีภายหลัง",
        "active",
    )
    .await;
    sqlx::query("UPDATE users SET status = 'inactive' WHERE id = $1")
        .bind(deactivated_user_id)
        .execute(&pool)
        .await
        .unwrap();
    let idempotency_key = Uuid::new_v4();
    let outcome = issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        request.id,
        IssueCertificateRequest { idempotency_key },
    )
    .await
    .unwrap();
    let problems = match &outcome {
        IssueCertificateOutcome::Returned {
            issue_codes,
            candidate_problems,
            ..
        } => {
            assert!(issue_codes.contains(&CertificateIssueCode::AccountStateChanged));
            candidate_problems
        }
        IssueCertificateOutcome::Issued { .. } => panic!("changed account must return request"),
    };
    assert!(problems.iter().any(|problem| {
        problem.candidate_id == converted.id
            && problem
                .issue_codes
                .contains(&CertificateIssueCode::AccountStateChanged)
    }));
    let replay = issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        request.id,
        IssueCertificateRequest { idempotency_key },
    )
    .await
    .unwrap();
    assert_eq!(replay, outcome);

    let returned_candidate = candidate_service::get_candidate(&pool, &preparer, converted.id)
        .await
        .unwrap();
    assert!(returned_candidate
        .validation_codes
        .contains(&CandidateValidationCode::UnexpectedInternalLookup));
    assert!(!returned_candidate
        .validation_codes
        .contains(&CandidateValidationCode::AccountInactive));
    let returned_deactivated =
        candidate_service::get_candidate(&pool, &preparer, deactivated_candidate.id)
            .await
            .unwrap();
    assert!(returned_deactivated
        .validation_codes
        .contains(&CandidateValidationCode::AccountInactive));
    assert!(!returned_deactivated
        .validation_codes
        .contains(&CandidateValidationCode::UnexpectedInternalLookup));
    let reconciled = candidate_service::bulk_update_for_campaign(
        &pool,
        &preparer,
        campaign.id,
        CertificateCandidateBulkRequest::AssignTemplate {
            candidate_ids: vec![converted.id],
            template_id: template.id,
        },
    )
    .await
    .unwrap()
    .candidates
    .remove(0);
    assert_eq!(reconciled.recipient_type, RecipientType::Student);
    assert!(reconciled.matched_user_id.is_some());
    assert_eq!(reconciled.match_status, CandidateMatchStatus::Matched);
    assert_eq!(
        reconciled.validation_status,
        CandidateValidationStatus::Ready
    );

    let persisted: (Option<i32>, i32, i64, String, i64) = sqlx::query_as(
        "SELECT campaign.activity_sequence, campaign.next_certificate_sequence,
                (SELECT COUNT(*) FROM certificate_academic_year_counters
                 WHERE academic_year_id = campaign.academic_year_id),
                request.status,
                (SELECT COUNT(*) FROM certificates certificate
                 WHERE certificate.campaign_id = campaign.id)
         FROM certificate_campaigns campaign
         JOIN certificate_issue_requests request ON request.campaign_id = campaign.id
         WHERE campaign.id = $1 AND request.id = $2",
    )
    .bind(campaign.id)
    .bind(request.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(persisted, (None, 1, 0, "returned".to_string(), 0));
    let lock_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM certificate_candidate_issue_locks WHERE request_id = $1",
    )
    .bind(request.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(lock_count, 0);
}

#[tokio::test]
async fn issue_revalidation_returns_without_numbers_when_background_is_no_longer_ready() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-background-recheck-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-background-recheck-blind-index-test-key",
    );
    let (pool, preparer, academic_year_id) =
        school_campaign_fixture("certificate_issue_background_recheck", 3146).await;
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-background-recheck-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมตรวจพื้นหลังซ้ำก่อนออก"),
    )
    .await
    .unwrap();
    let (template, candidate) = create_ready_external_request_candidate(
        &pool,
        &preparer,
        campaign.id,
        "แบบที่พื้นหลังไม่พร้อมภายหลัง",
        "ผู้รับตรวจพื้นหลัง",
    )
    .await;
    let request =
        request_service::submit_issue_request(&pool, &preparer, campaign.id, vec![candidate.id])
            .await
            .unwrap();
    request_service::start_review(&pool, &issuer, request.id)
        .await
        .unwrap();

    sqlx::query("UPDATE files SET lifecycle_status = 'failed' WHERE id = $1")
        .bind(template.background_file_id.unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let outcome = issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        request.id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let IssueCertificateOutcome::Returned {
        issue_codes,
        candidate_problems,
        ..
    } = outcome
    else {
        panic!("an unavailable background must return the whole request")
    };
    assert!(issue_codes.contains(&CertificateIssueCode::AssetUnavailable));
    assert_eq!(candidate_problems.len(), 1);
    assert!(candidate_problems[0]
        .issue_codes
        .contains(&CertificateIssueCode::AssetUnavailable));

    let numbering: (Option<i32>, i32, i64, i64) = sqlx::query_as(
        "SELECT campaign.activity_sequence, campaign.next_certificate_sequence,
                (SELECT COUNT(*) FROM certificate_academic_year_counters
                 WHERE academic_year_id = campaign.academic_year_id),
                (SELECT COUNT(*) FROM certificates certificate
                 WHERE certificate.campaign_id = campaign.id)
         FROM certificate_campaigns campaign
         WHERE campaign.id = $1",
    )
    .bind(campaign.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(numbering, (None, 1, 0, 0));
}

#[tokio::test]
async fn issue_revalidation_returns_when_referenced_asset_metadata_changes() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-asset-recheck-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-asset-recheck-blind-index-test-key",
    );
    let (pool, preparer, academic_year_id) =
        school_campaign_fixture("certificate_issue_asset_recheck", 3147).await;
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-asset-recheck-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมตรวจฟอนต์ซ้ำก่อนออก"),
    )
    .await
    .unwrap();
    let template = create_ready_candidate_template(
        &pool,
        &preparer,
        campaign.id,
        "แบบที่ฟอนต์เปลี่ยนภายหลัง",
        vec![RecipientType::External],
    )
    .await;
    let font_file_id = insert_ready_template_file(
        &pool,
        &preparer,
        template.id,
        "certificate_template_font",
        serde_json::json!({
            "kind": "font",
            "family_name": "Issuance Thai Font",
            "units_per_em": 1000
        }),
    )
    .await;
    let with_font = template_service::attach_asset(
        &pool,
        &preparer,
        template.id,
        AttachCertificateAssetRequest {
            file_id: font_file_id,
            kind: CertificateTemplateAssetKind::Font,
            display_name: "ฟอนต์สำหรับออกเกียรติบัตร".to_string(),
            font_weight: Some(400),
            rights_confirmed: true,
        },
    )
    .await
    .unwrap();
    let font_asset = with_font.assets[0].clone();
    let mut layout = text_layout(CertificateFontSource::Asset {
        asset_id: font_asset.id,
    });
    let CertificateElement::Text(text) = &mut layout.elements[0] else {
        panic!("expected text element")
    };
    text.font_family = font_asset.font_family.clone().unwrap();
    text.font_weight = font_asset.font_weight.unwrap();
    template_service::update_template(
        &pool,
        &preparer,
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
    .unwrap();
    let candidate = candidate_service::create_manual_external(
        &pool,
        &preparer,
        campaign.id,
        CreateManualExternalCandidateRequest {
            template_id: Some(template.id),
            title: Some("คุณ".to_string()),
            first_name: "ผู้รับตรวจฟอนต์".to_string(),
            last_name: "ก่อนออกใบ".to_string(),
            activity_item: None,
            award_or_role: Some("ผู้เข้าร่วม".to_string()),
            custom_values: BTreeMap::new(),
        },
    )
    .await
    .unwrap()
    .candidates
    .remove(0);
    let request =
        request_service::submit_issue_request(&pool, &preparer, campaign.id, vec![candidate.id])
            .await
            .unwrap();
    request_service::start_review(&pool, &issuer, request.id)
        .await
        .unwrap();

    sqlx::query("UPDATE certificate_template_assets SET font_weight = 700 WHERE id = $1")
        .bind(font_asset.id)
        .execute(&pool)
        .await
        .unwrap();

    let outcome = issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        request.id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let IssueCertificateOutcome::Returned {
        issue_codes,
        candidate_problems,
        ..
    } = outcome
    else {
        panic!("changed referenced asset metadata must return the whole request")
    };
    assert!(issue_codes.contains(&CertificateIssueCode::AssetUnavailable));
    assert_eq!(candidate_problems.len(), 1);
    assert!(candidate_problems[0]
        .issue_codes
        .contains(&CertificateIssueCode::AssetUnavailable));

    let numbering: (Option<i32>, i32, i64) = sqlx::query_as(
        "SELECT campaign.activity_sequence, campaign.next_certificate_sequence,
                (SELECT COUNT(*) FROM certificates certificate
                 WHERE certificate.campaign_id = campaign.id)
         FROM certificate_campaigns campaign
         WHERE campaign.id = $1",
    )
    .bind(campaign.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(numbering, (None, 1, 0));
}

#[tokio::test]
async fn revoked_numbers_are_never_reused_and_replacement_links_only_after_issuance() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-replacement-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-replacement-blind-index-test-key",
    );
    let (pool, preparer, academic_year_id) =
        school_campaign_fixture("certificate_revoke_replace", 3141).await;
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-replacement-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    let revoker = ActorContext {
        user_id: issuer_user_id,
        permissions: vec![codes::CERTIFICATE_REVOKE_SCHOOL.to_string()],
    };
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมออกใบแทน"),
    )
    .await
    .unwrap();
    let (_, candidate) = create_ready_external_request_candidate(
        &pool,
        &preparer,
        campaign.id,
        "แบบออกใบแทน",
        "ผู้รับที่ต้องแก้ไข",
    )
    .await;
    let request =
        request_service::submit_issue_request(&pool, &preparer, campaign.id, vec![candidate.id])
            .await
            .unwrap();
    request_service::start_review(&pool, &issuer, request.id)
        .await
        .unwrap();
    let original = match issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        request.id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap()
    {
        IssueCertificateOutcome::Issued { certificates, .. } => certificates[0].clone(),
        IssueCertificateOutcome::Returned { .. } => panic!("original should issue"),
    };

    let revoked = issuance_service::revoke_certificate(
        &pool,
        &revoker,
        original.id,
        RevokeCertificateRequest {
            reason: "ชื่อผู้รับในใบเดิมไม่ถูกต้อง".to_string(),
            create_replacement_candidate: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        revoked.certificate.summary.status,
        CertificateStatus::Revoked
    );
    assert_eq!(
        revoked.certificate.summary.certificate_number,
        original.certificate_number
    );
    assert!(revoked
        .certificate
        .summary
        .replaced_by_certificate_id
        .is_none());
    let replacement_candidate = revoked
        .replacement_candidate
        .expect("replacement draft should be created");
    assert_eq!(replacement_candidate.campaign_id, campaign.id);
    assert_eq!(
        replacement_candidate.validation_status,
        CandidateValidationStatus::NeedsReview
    );
    assert!(matches!(
        issuance_service::revoke_certificate(
            &pool,
            &revoker,
            original.id,
            RevokeCertificateRequest {
                reason: "ห้ามเพิกถอนซ้ำ".to_string(),
                create_replacement_candidate: false,
            },
        )
        .await,
        Err(AppError::Conflict(_))
    ));

    let replacement_draft =
        candidate_service::get_candidate(&pool, &preparer, replacement_candidate.id)
            .await
            .unwrap();
    let replacement_ready = candidate_service::update_candidate(
        &pool,
        &preparer,
        replacement_candidate.id,
        update_external_candidate_payload(&replacement_draft, "ผู้รับที่แก้ไขแล้ว"),
    )
    .await
    .unwrap();
    assert_eq!(
        replacement_ready.validation_status,
        CandidateValidationStatus::Ready
    );
    let replacement_request = request_service::submit_issue_request(
        &pool,
        &preparer,
        campaign.id,
        vec![replacement_ready.id],
    )
    .await
    .unwrap();
    request_service::start_review(&pool, &issuer, replacement_request.id)
        .await
        .unwrap();
    let replacement = match issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        replacement_request.id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap()
    {
        IssueCertificateOutcome::Issued { certificates, .. } => certificates[0].clone(),
        IssueCertificateOutcome::Returned { .. } => panic!("replacement should issue"),
    };
    assert_eq!(
        replacement.replacement_for_certificate_id,
        Some(original.id)
    );
    assert_eq!(
        replacement.certificate_sequence,
        original.certificate_sequence + 1
    );
    assert_ne!(replacement.certificate_number, original.certificate_number);
    let original_links: (String, Option<Uuid>) = sqlx::query_as(
        "SELECT certificate_number, replaced_by_certificate_id
         FROM certificates WHERE id = $1",
    )
    .bind(original.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(original_links.0, original.certificate_number);
    assert_eq!(original_links.1, Some(replacement.id));
}

#[tokio::test]
async fn issued_lists_and_details_enforce_exact_read_and_separate_download_scope() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-issued-list-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-issued-list-blind-index-test-key",
    );
    let (pool, school_preparer, academic_year_id) =
        school_campaign_fixture("certificate_issued_list_scope", 3142).await;
    let owner_unit = insert_unit(&pool, "certificate_issued_list_owner", None).await;
    let other_unit = insert_unit(&pool, "certificate_issued_list_other", None).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &school_preparer,
        campaign_create_payload(academic_year_id, Some(owner_unit), "กิจกรรมรายการใบที่ออกแล้ว"),
    )
    .await
    .unwrap();
    let (_, candidate) = create_ready_external_request_candidate(
        &pool,
        &school_preparer,
        campaign.id,
        "แบบรายการใบที่ออกแล้ว",
        "ผู้รับสำหรับค้นหา",
    )
    .await;
    let request = request_service::submit_issue_request(
        &pool,
        &school_preparer,
        campaign.id,
        vec![candidate.id],
    )
    .await
    .unwrap();
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-issued-list-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    request_service::start_review(&pool, &issuer, request.id)
        .await
        .unwrap();
    let issued = match issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนทดสอบ".to_string(),
        request.id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap()
    {
        IssueCertificateOutcome::Issued { certificates, .. } => certificates[0].clone(),
        IssueCertificateOutcome::Returned { .. } => panic!("request should issue"),
    };

    let reader_user_id = create_test_user(
        &pool,
        "certificate-issued-list-reader@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO organization_members
            (user_id, organization_unit_id, position_code, started_at)
         VALUES ($1, $2, 'head', CURRENT_DATE)",
    )
    .bind(reader_user_id)
    .bind(owner_unit)
    .execute(&pool)
    .await
    .unwrap();
    add_exact_grant(&pool, owner_unit, codes::CERTIFICATE_READ_ORGANIZATION_UNIT).await;
    let reader = ActorContext {
        user_id: reader_user_id,
        permissions: vec![codes::CERTIFICATE_READ_ORGANIZATION_UNIT.to_string()],
    };
    let list = issuance_service::list_campaign_certificates(
        &pool,
        &reader,
        campaign.id,
        crate::modules::certificates::models::IssuedCertificateListQuery {
            search: Some("ผู้รับสำหรับค้นหา".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].capabilities.can_read);
    assert!(!list[0].capabilities.can_download);
    assert!(!list[0].capabilities.can_revoke);
    let detail = issuance_service::get_certificate(&pool, &reader, issued.id)
        .await
        .unwrap();
    assert_eq!(detail.summary.id, issued.id);
    assert_eq!(detail.school_name, "โรงเรียนทดสอบ");

    add_exact_grant(
        &pool,
        owner_unit,
        codes::CERTIFICATE_DOWNLOAD_ORGANIZATION_UNIT,
    )
    .await;
    let with_download = issuance_service::get_certificate(&pool, &reader, issued.id)
        .await
        .unwrap();
    assert!(with_download.summary.capabilities.can_download);

    let outsider_user_id = create_test_user(
        &pool,
        "certificate-issued-list-outsider@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO organization_members
            (user_id, organization_unit_id, position_code, started_at)
         VALUES ($1, $2, 'head', CURRENT_DATE)",
    )
    .bind(outsider_user_id)
    .bind(other_unit)
    .execute(&pool)
    .await
    .unwrap();
    let outsider = ActorContext {
        user_id: outsider_user_id,
        permissions: vec![codes::CERTIFICATE_READ_ORGANIZATION_UNIT.to_string()],
    };
    assert!(matches!(
        issuance_service::list_campaign_certificates(
            &pool,
            &outsider,
            campaign.id,
            Default::default(),
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
    assert!(matches!(
        issuance_service::get_certificate(&pool, &outsider, issued.id).await,
        Err(AppError::Forbidden(_))
    ));
}

#[tokio::test]
async fn batch_render_authorizes_campaign_before_disclosing_certificate_membership() {
    let (pool, preparer, academic_year_id) =
        school_campaign_fixture("certificate_batch_render_scope", 3148).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมดาวน์โหลดหลายใบ"),
    )
    .await
    .unwrap();
    let outsider_user_id = create_test_user(
        &pool,
        "certificate-batch-render-outsider@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let outsider = ActorContext {
        user_id: outsider_user_id,
        permissions: Vec::new(),
    };
    let platform = crate::modules::files::platform_service::FilePlatform::new(
        Arc::new(PreviewStorage),
        Arc::new(PreviewScanner),
    );

    assert!(matches!(
        render_service::issued_manifests(
            &pool,
            &outsider,
            &platform,
            "sandbox",
            "schoolorbit.test",
            campaign.id,
            CertificateRenderManifestBatchRequest {
                certificate_ids: vec![Uuid::new_v4()],
            },
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
}

#[tokio::test]
async fn issued_manifest_uses_canonical_proof_url_and_refuses_revoked_certificates() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-issued-render-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-issued-render-blind-index-test-key",
    );
    let (pool, preparer, academic_year_id) =
        concurrent_school_campaign_fixture("certificate_issued_manifest", 3143).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมดาวน์โหลดใบจริง"),
    )
    .await
    .unwrap();
    let (template, candidate) = create_ready_external_request_candidate(
        &pool,
        &preparer,
        campaign.id,
        "แบบดาวน์โหลดใบจริง",
        "กมลชนก",
    )
    .await;
    let request =
        request_service::submit_issue_request(&pool, &preparer, campaign.id, vec![candidate.id])
            .await
            .unwrap();
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-issued-render-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);
    request_service::start_review(&pool, &issuer, request.id)
        .await
        .unwrap();
    let issued = match issuance_service::issue_request(
        &pool,
        &issuer,
        "โรงเรียนตัวอย่าง".to_string(),
        request.id,
        IssueCertificateRequest {
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap()
    {
        IssueCertificateOutcome::Issued { certificates, .. } => certificates[0].clone(),
        IssueCertificateOutcome::Returned { .. } => panic!("request should issue"),
    };
    let platform = crate::modules::files::platform_service::FilePlatform::new(
        Arc::new(PreviewStorage),
        Arc::new(PreviewScanner),
    );
    let manifest = render_service::issued_manifest(
        &pool,
        &preparer,
        &platform,
        "sandbox",
        "schoolorbit.test",
        issued.id,
    )
    .await
    .unwrap();
    assert_eq!(manifest.certificate_number, issued.certificate_number);
    assert_eq!(manifest.recipient_values["ชื่อ"], "กมลชนก");
    assert_eq!(manifest.recipient_values["ชื่อโรงเรียนผู้ออก"], "โรงเรียนตัวอย่าง");
    assert!(manifest.qr_payload.starts_with(&format!(
        "https://sandbox.schoolorbit.test/verify/certificate/{}#proof=",
        issued.certificate_number
    )));
    assert_eq!(manifest.font_grants.len(), 0);
    assert_eq!(manifest.image_grants.len(), 0);

    candidate_service::create_manual_external(
        &pool,
        &preparer,
        campaign.id,
        CreateManualExternalCandidateRequest {
            template_id: Some(template.id),
            title: None,
            first_name: "ผู้รับรายใหม่".to_string(),
            last_name: "มีค่าตัวแปร".to_string(),
            activity_item: None,
            award_or_role: None,
            custom_values: BTreeMap::from([("ครูผู้ควบคุม".to_string(), "ครูตัวอย่าง".to_string())]),
        },
    )
    .await
    .unwrap();
    let current_template = template_service::get_template(&pool, &preparer, template.id)
        .await
        .unwrap();
    let mut layout_with_new_variable = text_layout(CertificateFontSource::BuiltIn);
    let CertificateElement::Text(text) = &mut layout_with_new_variable.elements[0] else {
        panic!("expected text element")
    };
    text.content = "มอบให้ {ชื่อ} โดย {ครูผู้ควบคุม}".to_string();
    let updated_template = template_service::update_template(
        &pool,
        &preparer,
        template.id,
        UpdateCertificateTemplateRequest {
            expected_updated_at: current_template.updated_at,
            name: None,
            allowed_recipient_types: None,
            safe_margin_points: None,
            show_safe_area: None,
            layout: Some(layout_with_new_variable),
            is_active: None,
            confirm_missing_issued_values: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        updated_template.template.missing_variable_certificate_count,
        1
    );
    let manifest_with_new_variable = render_service::issued_manifest(
        &pool,
        &preparer,
        &platform,
        "sandbox",
        "schoolorbit.test",
        issued.id,
    )
    .await
    .unwrap();
    assert_eq!(
        manifest_with_new_variable
            .recipient_values
            .get("ครูผู้ควบคุม")
            .map(String::as_str),
        Some("")
    );

    let revoker = ActorContext {
        user_id: issuer_user_id,
        permissions: vec![codes::CERTIFICATE_REVOKE_SCHOOL.to_string()],
    };
    let grant_entered = Arc::new(tokio::sync::Notify::new());
    let grant_release = Arc::new(tokio::sync::Notify::new());
    let blocking_platform = Arc::new(crate::modules::files::platform_service::FilePlatform::new(
        Arc::new(BlockingGrantStorage {
            entered: Arc::clone(&grant_entered),
            release: Arc::clone(&grant_release),
        }),
        Arc::new(PreviewScanner),
    ));
    let render_pool = pool.clone();
    let render_actor = preparer.clone();
    let render_platform = Arc::clone(&blocking_platform);
    let certificate_id = issued.id;
    let rendering = tokio::spawn(async move {
        render_service::issued_manifest(
            &render_pool,
            &render_actor,
            render_platform.as_ref(),
            "sandbox",
            "schoolorbit.test",
            certificate_id,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), grant_entered.notified())
        .await
        .expect("render should reach private grant generation");

    let revoke_pool = pool.clone();
    let revoke_actor = revoker.clone();
    let revoking = tokio::spawn(async move {
        issuance_service::revoke_certificate(
            &revoke_pool,
            &revoke_actor,
            certificate_id,
            RevokeCertificateRequest {
                reason: "ยกเลิกใบเดิมเพื่อทดสอบการดาวน์โหลด".to_string(),
                create_replacement_candidate: false,
            },
        )
        .await
    });
    let wait_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let revocation_blocked = loop {
        if revoking.is_finished() {
            break false;
        }
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1
                 FROM pg_stat_activity
                 WHERE datname = current_database()
                   AND pid <> pg_backend_pid()
                   AND wait_event_type = 'Lock'
                   AND query LIKE '%FROM certificates%'
                   AND query LIKE '%FOR UPDATE%'
             )",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if blocked {
            break true;
        }
        assert!(
            tokio::time::Instant::now() < wait_deadline,
            "revocation neither completed nor waited on the certificate row"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    grant_release.notify_one();
    rendering.await.unwrap().unwrap();
    revoking.await.unwrap().unwrap();
    assert!(
        revocation_blocked,
        "revocation committed while an issued render manifest was still being created"
    );
    assert!(matches!(
        render_service::issued_manifest(
            &pool,
            &preparer,
            &platform,
            "sandbox",
            "schoolorbit.test",
            issued.id,
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn issuance_rejects_activity_and_certificate_sequence_upper_bounds_without_partial_writes() {
    let _crypto_guard = crate::utils::field_encryption::test_env_lock();
    env::set_var(
        "ENCRYPTION_KEY",
        "certificate-upper-bound-encryption-test-key",
    );
    env::set_var(
        "BLIND_INDEX_KEY",
        "certificate-upper-bound-blind-index-test-key",
    );
    let (pool, preparer, academic_year_id) =
        school_campaign_fixture("certificate_issue_upper_bounds", 3144).await;
    let issuer_user_id = create_test_user(
        &pool,
        "certificate-upper-bound-issuer@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let issuer = school_certificate_issuer(issuer_user_id);

    let activity_campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(academic_year_id, None, "กิจกรรมเกินลำดับกิจกรรม"),
    )
    .await
    .unwrap();
    let (_, activity_candidate) = create_ready_external_request_candidate(
        &pool,
        &preparer,
        activity_campaign.id,
        "แบบเกินลำดับกิจกรรม",
        "ผู้รับเกินลำดับกิจกรรม",
    )
    .await;
    let activity_request = request_service::submit_issue_request(
        &pool,
        &preparer,
        activity_campaign.id,
        vec![activity_candidate.id],
    )
    .await
    .unwrap();
    request_service::start_review(&pool, &issuer, activity_request.id)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO certificate_academic_year_counters
            (academic_year_id, next_activity_sequence)
         VALUES ($1, 10000)",
    )
    .bind(academic_year_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        issuance_service::issue_request(
            &pool,
            &issuer,
            "โรงเรียนทดสอบ".to_string(),
            activity_request.id,
            IssueCertificateRequest {
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let activity_state: (Option<i32>, i32, String, i64) = sqlx::query_as(
        "SELECT campaign.activity_sequence, counter.next_activity_sequence,
                request.status,
                (SELECT COUNT(*) FROM certificate_issue_runs run
                 WHERE run.request_id = request.id)
         FROM certificate_campaigns campaign
         JOIN certificate_academic_year_counters counter
           ON counter.academic_year_id = campaign.academic_year_id
         JOIN certificate_issue_requests request ON request.campaign_id = campaign.id
         WHERE campaign.id = $1 AND request.id = $2",
    )
    .bind(activity_campaign.id)
    .bind(activity_request.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(activity_state, (None, 10000, "reviewing".to_string(), 0));

    let second_year_id = insert_academic_year(&pool, 3145).await;
    let certificate_campaign = campaign_service::create_campaign(
        &pool,
        &preparer,
        campaign_create_payload(second_year_id, None, "กิจกรรมเกินลำดับใบ"),
    )
    .await
    .unwrap();
    let (_, certificate_candidate) = create_ready_external_request_candidate(
        &pool,
        &preparer,
        certificate_campaign.id,
        "แบบเกินลำดับใบ",
        "ผู้รับเกินลำดับใบ",
    )
    .await;
    let certificate_request = request_service::submit_issue_request(
        &pool,
        &preparer,
        certificate_campaign.id,
        vec![certificate_candidate.id],
    )
    .await
    .unwrap();
    request_service::start_review(&pool, &issuer, certificate_request.id)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE certificate_campaigns
         SET activity_sequence = 1, next_certificate_sequence = 1000000
         WHERE id = $1",
    )
    .bind(certificate_campaign.id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        issuance_service::issue_request(
            &pool,
            &issuer,
            "โรงเรียนทดสอบ".to_string(),
            certificate_request.id,
            IssueCertificateRequest {
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let certificate_state: (i32, String, i64, i64) = sqlx::query_as(
        "SELECT campaign.next_certificate_sequence, request.status,
                (SELECT COUNT(*) FROM certificate_issue_runs run
                 WHERE run.request_id = request.id),
                (SELECT COUNT(*) FROM certificates certificate
                 WHERE certificate.campaign_id = campaign.id)
         FROM certificate_campaigns campaign
         JOIN certificate_issue_requests request ON request.campaign_id = campaign.id
         WHERE campaign.id = $1 AND request.id = $2",
    )
    .bind(certificate_campaign.id)
    .bind(certificate_request.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(certificate_state, (1000000, "reviewing".to_string(), 0, 0));
}
