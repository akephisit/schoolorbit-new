use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::{
        files::{
            platform_types::{FileLifecycleStatus, FilePurpose, FileVisibility},
            repository::PlatformFile,
        },
        school_fonts::{
            models::{
                AttachSchoolFontBatchRequest, InspectSchoolFontUploadsRequest, SchoolFontStyle,
                SchoolFontUploadStatus,
            },
            services::{self, SchoolFontDeleteOutcome, SchoolFontStagingRelation},
        },
    },
    permissions::registry::codes,
    policies::file_access_policy::{
        authorize_existing, authorize_school_font_delete_guard, FilePolicyAction,
    },
    test_helpers::{create_named_test_pool, create_test_user, run_test_migrations},
};

fn actor(user_id: Uuid, permissions: &[&str]) -> ActorContext {
    ActorContext {
        user_id,
        permissions: permissions
            .iter()
            .map(|permission| permission.to_string())
            .collect(),
    }
}

async fn font_test_context(test_name: &str) -> (PgPool, ActorContext, ActorContext, ActorContext) {
    let pool = create_named_test_pool(test_name).await;
    run_test_migrations(&pool).await;
    let manager_id = create_test_user(
        &pool,
        &format!("{test_name}-manager@example.test"),
        "test-password",
    )
    .await
    .expect("manager fixture should insert");
    let designer_id = create_test_user(
        &pool,
        &format!("{test_name}-designer@example.test"),
        "test-password",
    )
    .await
    .expect("designer fixture should insert");
    let ordinary_id = create_test_user(
        &pool,
        &format!("{test_name}-ordinary@example.test"),
        "test-password",
    )
    .await
    .expect("ordinary-user fixture should insert");
    (
        pool,
        actor(manager_id, &[codes::FONT_MANAGE_SCHOOL]),
        actor(designer_id, &[codes::CERTIFICATE_UPDATE_SCHOOL]),
        actor(ordinary_id, &[]),
    )
}

fn font_inspection(
    family: Option<&str>,
    weight: u16,
    style: &str,
    is_variable: bool,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "font",
        "family_name": family,
        "units_per_em": 1000,
        "weight": weight,
        "style": style,
        "is_variable": is_variable
    })
}

async fn insert_staged_font(
    pool: &PgPool,
    uploaded_by: Uuid,
    filename: &str,
    family: Option<&str>,
    weight: u16,
    style: &str,
    is_variable: bool,
) -> Uuid {
    let file_id: Uuid = sqlx::query_scalar(
        "INSERT INTO files (
            owner_user_id, display_filename, created_by, purpose_code, visibility,
            lifecycle_status, retention_class, expires_at, inspection_metadata
         ) VALUES ($1, $2, $1, 'school_font', 'private', 'processing',
                   'temporary', now() + interval '1 hour', $3)
         RETURNING id",
    )
    .bind(uploaded_by)
    .bind(filename)
    .bind(font_inspection(family, weight, style, is_variable))
    .fetch_one(pool)
    .await
    .expect("school-font file fixture should insert");
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO file_versions (
            file_id, version_number, provider_code, storage_class, storage_status,
            object_key, detected_mime_type, canonical_extension, byte_size, checksum,
            scan_status, scanned_at, created_by
         ) VALUES ($1, 1, 'test', 'private', 'stored', $2, 'font/ttf', 'ttf',
                   1024, repeat('a', 64), 'clean', now(), $3)
         RETURNING id",
    )
    .bind(file_id)
    .bind(format!(
        "tenants/{}/school/font/{file_id}/v1/original.ttf",
        Uuid::nil()
    ))
    .bind(uploaded_by)
    .fetch_one(pool)
    .await
    .expect("school-font version fixture should insert");
    sqlx::query(
        "UPDATE files SET current_version_id = $2, lifecycle_status = 'ready' WHERE id = $1",
    )
    .bind(file_id)
    .bind(version_id)
    .execute(pool)
    .await
    .expect("school-font fixture should become ready");
    sqlx::query("INSERT INTO school_font_file_uploads (file_id, uploaded_by) VALUES ($1, $2)")
        .bind(file_id)
        .bind(uploaded_by)
        .execute(pool)
        .await
        .expect("central school-font staging fixture should insert");
    file_id
}

async fn insert_template(pool: &PgPool, actor_id: Uuid, year: i32) -> Uuid {
    let academic_year_id: Uuid = sqlx::query_scalar(
        "INSERT INTO academic_years (year, name, start_date, end_date)
         VALUES ($1, 'School font service test', make_date($1, 1, 1), make_date($1, 12, 31))
         RETURNING id",
    )
    .bind(year)
    .fetch_one(pool)
    .await
    .expect("academic-year fixture should insert");
    let campaign_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_campaigns (
            academic_year_id, name, event_date, status, created_by
         ) VALUES ($1, 'School font service test', make_date($2, 6, 1), 'active', $3)
         RETURNING id",
    )
    .bind(academic_year_id)
    .bind(year)
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .expect("campaign fixture should insert");
    sqlx::query_scalar(
        "INSERT INTO certificate_templates (campaign_id, name, normalized_name)
         VALUES ($1, 'School font service test', $2)
         RETURNING id",
    )
    .bind(campaign_id)
    .bind(format!("school-font-service-{year}"))
    .fetch_one(pool)
    .await
    .expect("template fixture should insert")
}

async fn move_to_template_staging(pool: &PgPool, file_id: Uuid, template_id: Uuid, actor_id: Uuid) {
    sqlx::query("DELETE FROM school_font_file_uploads WHERE file_id = $1")
        .bind(file_id)
        .execute(pool)
        .await
        .expect("central staging fixture should delete");
    sqlx::query(
        "INSERT INTO certificate_school_font_file_uploads (file_id, template_id, uploaded_by)
         VALUES ($1, $2, $3)",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(actor_id)
    .execute(pool)
    .await
    .expect("template staging fixture should insert");
}

async fn insert_wrong_purpose_file(pool: &PgPool, actor_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO files (
            owner_user_id, display_filename, created_by, purpose_code, visibility,
            lifecycle_status, retention_class, inspection_metadata
         ) VALUES ($1, 'not-a-font.png', $1, 'certificate_template_image', 'private',
                   'ready', 'temporary', '{\"kind\":\"image\",\"width_px\":1,\"height_px\":1}'::jsonb)
         RETURNING id",
    )
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .expect("wrong-purpose fixture should insert")
}

#[test]
fn normalized_family_uses_nfkc_trim_and_lowercase_without_losing_thai() {
    assert_eq!(services::normalize_family("  ＳaRaBuN  "), "sarabun");
    assert_eq!(
        services::normalize_family("\u{3000}ไทย สารบรรณ\u{00a0}"),
        "ไทย สารบรรณ"
    );
    assert_eq!(services::normalize_family("  ไทยＡBC  "), "ไทยabc");
}

#[tokio::test]
async fn manager_entry_points_require_only_the_font_manager_permission() {
    let (pool, manager, template_designer, ordinary_user) =
        font_test_context("school_font_manager_authority").await;

    assert!(services::list_for_manager(&pool, &manager).await.is_ok());
    assert!(services::list_for_manager(&pool, &template_designer)
        .await
        .is_err());
    assert!(services::attach_for_manager(
        &pool,
        &ordinary_user,
        AttachSchoolFontBatchRequest {
            file_ids: vec![Uuid::new_v4()],
            rights_confirmed: true,
        },
    )
    .await
    .is_err());
}

#[tokio::test]
async fn inspection_preserves_selection_order_and_classifies_batch_failures() {
    let (pool, manager, _, _) = font_test_context("school_font_inspection").await;
    let regular = insert_staged_font(
        &pool,
        manager.user_id,
        "regular.ttf",
        Some("Reviewed Thai"),
        400,
        "normal",
        false,
    )
    .await;
    let bold = insert_staged_font(
        &pool,
        manager.user_id,
        "bold.ttf",
        Some("Reviewed Thai"),
        700,
        "normal",
        false,
    )
    .await;
    let inspected = services::inspect_for_manager(
        &pool,
        &manager,
        InspectSchoolFontUploadsRequest {
            file_ids: vec![bold, regular],
        },
    )
    .await
    .expect("ready font batch should inspect");
    assert_eq!(
        inspected
            .files
            .iter()
            .map(|file| file.file_id)
            .collect::<Vec<_>>(),
        vec![bold, regular]
    );
    assert!(inspected
        .files
        .iter()
        .all(|file| file.status == SchoolFontUploadStatus::Ready));

    let duplicate = insert_staged_font(
        &pool,
        manager.user_id,
        "duplicate.ttf",
        Some("  reviewed thai  "),
        400,
        "normal",
        false,
    )
    .await;
    let duplicate_selection = services::inspect_for_manager(
        &pool,
        &manager,
        InspectSchoolFontUploadsRequest {
            file_ids: vec![regular, duplicate],
        },
    )
    .await
    .expect("duplicate variants should be returned as inspection statuses");
    assert!(duplicate_selection
        .files
        .iter()
        .all(|file| file.status == SchoolFontUploadStatus::DuplicateSelection));

    let variable = insert_staged_font(
        &pool,
        manager.user_id,
        "variable.ttf",
        Some("Variable Thai"),
        400,
        "italic",
        true,
    )
    .await;
    let variable_inspection = services::inspect_for_manager(
        &pool,
        &manager,
        InspectSchoolFontUploadsRequest {
            file_ids: vec![variable],
        },
    )
    .await
    .expect("variable font should produce an inspection status");
    assert_eq!(
        variable_inspection.files[0].status,
        SchoolFontUploadStatus::UnsupportedVariable
    );

    let unavailable = insert_staged_font(
        &pool,
        manager.user_id,
        "unavailable.ttf",
        Some("Unavailable Thai"),
        400,
        "normal",
        false,
    )
    .await;
    sqlx::query("UPDATE files SET lifecycle_status = 'processing' WHERE id = $1")
        .bind(unavailable)
        .execute(&pool)
        .await
        .expect("unavailable fixture should update");
    let unavailable_inspection = services::inspect_for_manager(
        &pool,
        &manager,
        InspectSchoolFontUploadsRequest {
            file_ids: vec![unavailable],
        },
    )
    .await
    .expect("non-ready font should produce an inspection status");
    assert_eq!(
        unavailable_inspection.files[0].status,
        SchoolFontUploadStatus::Unavailable
    );

    for file_ids in [Vec::new(), vec![regular, regular], vec![regular; 41]] {
        assert!(matches!(
            services::inspect_for_manager(
                &pool,
                &manager,
                InspectSchoolFontUploadsRequest { file_ids },
            )
            .await,
            Err(AppError::ValidationError(_))
        ));
    }

    let wrong_purpose = insert_wrong_purpose_file(&pool, manager.user_id).await;
    assert!(matches!(
        services::inspect_for_manager(
            &pool,
            &manager,
            InspectSchoolFontUploadsRequest {
                file_ids: vec![wrong_purpose],
            },
        )
        .await,
        Err(AppError::ValidationError(_))
    ));

    let template_id = insert_template(&pool, manager.user_id, 3281).await;
    move_to_template_staging(&pool, bold, template_id, manager.user_id).await;
    assert!(matches!(
        services::inspect_for_manager(
            &pool,
            &manager,
            InspectSchoolFontUploadsRequest {
                file_ids: vec![bold],
            },
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
}

#[tokio::test]
async fn ready_batch_attaches_atomically_and_rejects_existing_variants_or_missing_rights() {
    let (pool, manager, _, _) = font_test_context("school_font_atomic_attach").await;
    let regular = insert_staged_font(
        &pool,
        manager.user_id,
        "reviewed-regular.ttf",
        Some("Reviewed Thai"),
        400,
        "normal",
        false,
    )
    .await;
    let bold = insert_staged_font(
        &pool,
        manager.user_id,
        "reviewed-bold.ttf",
        Some("Reviewed Thai"),
        700,
        "normal",
        false,
    )
    .await;
    assert!(matches!(
        services::attach_for_manager(
            &pool,
            &manager,
            AttachSchoolFontBatchRequest {
                file_ids: vec![regular, bold],
                rights_confirmed: false,
            },
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    let before_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM school_fonts")
        .fetch_one(&pool)
        .await
        .expect("font count should load");
    assert_eq!(before_count, 0);

    let attached = services::attach_for_manager(
        &pool,
        &manager,
        AttachSchoolFontBatchRequest {
            file_ids: vec![bold, regular],
            rights_confirmed: true,
        },
    )
    .await
    .expect("ready batch should attach");
    assert_eq!(attached.items.len(), 2);
    assert_eq!(attached.items[0].font_weight, 700);
    assert_eq!(attached.items[1].font_weight, 400);
    assert!(attached
        .items
        .iter()
        .all(|font| font.font_style == SchoolFontStyle::Normal && font.reference_count == 0));
    let promoted_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM files
         WHERE id = ANY($1::uuid[]) AND retention_class = 'standard' AND expires_at IS NULL",
    )
    .bind(vec![regular, bold])
    .fetch_one(&pool)
    .await
    .expect("promoted font count should load");
    assert_eq!(promoted_count, 2);
    let staging_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM school_font_file_uploads WHERE file_id = ANY($1::uuid[])",
    )
    .bind(vec![regular, bold])
    .fetch_one(&pool)
    .await
    .expect("staging count should load");
    assert_eq!(staging_count, 0);
    let audit_file_count: i64 = sqlx::query_scalar(
        "SELECT (metadata ->> 'fileCount')::bigint
         FROM audit_logs
         WHERE entity_type = 'school_font_library' AND action = 'attach_batch'
         ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("safe batch audit should load");
    assert_eq!(audit_file_count, 2);

    let existing_variant = insert_staged_font(
        &pool,
        manager.user_id,
        "existing.ttf",
        Some("ＲＥＶＩＥＷＥＤ ＴＨＡＩ"),
        400,
        "normal",
        false,
    )
    .await;
    let inspected = services::inspect_for_manager(
        &pool,
        &manager,
        InspectSchoolFontUploadsRequest {
            file_ids: vec![existing_variant],
        },
    )
    .await
    .expect("existing variant should inspect");
    assert_eq!(
        inspected.files[0].status,
        SchoolFontUploadStatus::DuplicateExisting
    );
    assert!(matches!(
        services::attach_for_manager(
            &pool,
            &manager,
            AttachSchoolFontBatchRequest {
                file_ids: vec![existing_variant],
                rights_confirmed: true,
            },
        )
        .await,
        Err(AppError::Conflict(message)) if message == "school_font_variant_conflict"
    ));
}

#[tokio::test]
async fn certificate_batch_attach_rejects_a_purging_campaign_before_promotion() {
    let (pool, _, designer, _) = font_test_context("school_font_purging_campaign_attach").await;
    let template_id = insert_template(&pool, designer.user_id, 2991).await;
    let file_id = insert_staged_font(
        &pool,
        designer.user_id,
        "purging-campaign.ttf",
        Some("Purging Campaign"),
        400,
        "normal",
        false,
    )
    .await;
    move_to_template_staging(&pool, file_id, template_id, designer.user_id).await;
    sqlx::query(
        "UPDATE certificate_campaigns
         SET status = 'purging'
         WHERE id = (SELECT campaign_id FROM certificate_templates WHERE id = $1)",
    )
    .bind(template_id)
    .execute(&pool)
    .await
    .expect("campaign fixture should enter purging state");

    assert!(matches!(
        services::attach_authorized(
            &pool,
            designer.user_id,
            SchoolFontStagingRelation::CertificateTemplate(template_id),
            AttachSchoolFontBatchRequest {
                file_ids: vec![file_id],
                rights_confirmed: true,
            },
        )
        .await,
        Err(AppError::Conflict(message)) if message == "certificate_campaign_purging"
    ));

    let state: (String, String, i64) = sqlx::query_as(
        "SELECT file.retention_class, file.lifecycle_status,
                (SELECT COUNT(*) FROM school_fonts WHERE file_id = file.id)
         FROM files AS file
         WHERE file.id = $1",
    )
    .bind(file_id)
    .fetch_one(&pool)
    .await
    .expect("font fixture should remain queryable");
    assert_eq!(state, ("temporary".to_string(), "ready".to_string(), 0));
}

#[tokio::test]
async fn database_failure_rolls_back_every_font_promotion_and_library_insert() {
    let (pool, manager, _, _) = font_test_context("school_font_database_rollback").await;
    let first = insert_staged_font(
        &pool,
        manager.user_id,
        "first.ttf",
        Some("First Family"),
        400,
        "normal",
        false,
    )
    .await;
    let second = insert_staged_font(
        &pool,
        manager.user_id,
        "second.ttf",
        Some("Rejected Family"),
        400,
        "normal",
        false,
    )
    .await;
    sqlx::query(
        "CREATE FUNCTION reject_school_font_fixture() RETURNS trigger
         LANGUAGE plpgsql AS $$
         BEGIN
             IF NEW.font_family = 'Rejected Family' THEN
                 RAISE EXCEPTION 'fixture database failure';
             END IF;
             RETURN NEW;
         END
         $$",
    )
    .execute(&pool)
    .await
    .expect("fixture trigger function should create");
    sqlx::query(
        "CREATE TRIGGER reject_school_font_fixture
         BEFORE INSERT ON school_fonts
         FOR EACH ROW EXECUTE FUNCTION reject_school_font_fixture()",
    )
    .execute(&pool)
    .await
    .expect("fixture trigger should create");

    assert!(matches!(
        services::attach_for_manager(
            &pool,
            &manager,
            AttachSchoolFontBatchRequest {
                file_ids: vec![first, second],
                rights_confirmed: true,
            },
        )
        .await,
        Err(AppError::DbError(_))
    ));
    let library_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM school_fonts")
        .fetch_one(&pool)
        .await
        .expect("library count should load");
    assert_eq!(library_count, 0);
    let promoted_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM files
         WHERE id = ANY($1::uuid[]) AND retention_class = 'standard'",
    )
    .bind(vec![first, second])
    .fetch_one(&pool)
    .await
    .expect("promotion count should load");
    assert_eq!(promoted_count, 0);
    let staging_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM school_font_file_uploads WHERE file_id = ANY($1::uuid[])",
    )
    .bind(vec![first, second])
    .fetch_one(&pool)
    .await
    .expect("staging count should load");
    assert_eq!(staging_count, 2);
}

#[tokio::test]
async fn delete_returns_safe_reference_conflict_then_removes_an_unreferenced_font() {
    let (pool, manager, _, _) = font_test_context("school_font_delete_conflict").await;
    let file_id = insert_staged_font(
        &pool,
        manager.user_id,
        "delete-me.ttf",
        Some("Delete Me"),
        400,
        "italic",
        false,
    )
    .await;
    let attached = services::attach_for_manager(
        &pool,
        &manager,
        AttachSchoolFontBatchRequest {
            file_ids: vec![file_id],
            rights_confirmed: true,
        },
    )
    .await
    .expect("delete fixture font should attach");
    let font_id = attached.items[0].id;
    let template_id = insert_template(&pool, manager.user_id, 3282).await;
    sqlx::query(
        "INSERT INTO certificate_template_font_references (template_id, font_id)
         VALUES ($1, $2)",
    )
    .bind(template_id)
    .bind(font_id)
    .execute(&pool)
    .await
    .expect("font-reference fixture should insert");

    let conflict = services::delete(&pool, &manager, font_id)
        .await
        .expect("referenced font should return a typed outcome");
    assert_eq!(
        conflict,
        SchoolFontDeleteOutcome::Conflict(
            crate::modules::school_fonts::models::SchoolFontDeleteConflict { reference_count: 1 }
        )
    );
    sqlx::query("DELETE FROM certificate_template_font_references WHERE font_id = $1")
        .bind(font_id)
        .execute(&pool)
        .await
        .expect("font-reference fixture should delete");
    let deleted = services::delete(&pool, &manager, font_id)
        .await
        .expect("unreferenced font should delete");
    assert_eq!(deleted, SchoolFontDeleteOutcome::Deleted { file_id });
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM school_fonts WHERE id = $1")
        .bind(font_id)
        .fetch_one(&pool)
        .await
        .expect("font row count should load");
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn generic_file_policy_allows_only_central_staging_cleanup_and_never_durable_fonts() {
    let (pool, manager, _, ordinary) = font_test_context("school_font_file_policy").await;
    let file_id = insert_staged_font(
        &pool,
        manager.user_id,
        "temporary.ttf",
        Some("Temporary Font"),
        400,
        "normal",
        false,
    )
    .await;
    let file = PlatformFile {
        id: file_id,
        owner_user_id: Some(manager.user_id),
        purpose: FilePurpose::SchoolFont,
        visibility: FileVisibility::Private,
        lifecycle_status: FileLifecycleStatus::Ready,
        current_version: Some(1),
        display_filename: "temporary.ttf".to_string(),
        detected_mime_type: "font/ttf".to_string(),
        byte_size: 1024,
    };
    authorize_existing(&pool, &manager, &file, FilePolicyAction::Read, None)
        .await
        .expect("font manager should inspect central staging metadata");
    assert!(
        authorize_existing(&pool, &ordinary, &file, FilePolicyAction::Read, None)
            .await
            .is_err()
    );
    let delete_guard = authorize_school_font_delete_guard(&pool, &manager, &file, None)
        .await
        .expect("font manager should delete unattached central staging");
    delete_guard
        .rollback()
        .await
        .expect("test delete guard should roll back");
    assert!(
        authorize_school_font_delete_guard(&pool, &ordinary, &file, None)
            .await
            .is_err()
    );

    services::attach_for_manager(
        &pool,
        &manager,
        AttachSchoolFontBatchRequest {
            file_ids: vec![file_id],
            rights_confirmed: true,
        },
    )
    .await
    .expect("policy fixture font should attach");
    assert!(
        authorize_school_font_delete_guard(&pool, &manager, &file, None)
            .await
            .is_err()
    );
}
