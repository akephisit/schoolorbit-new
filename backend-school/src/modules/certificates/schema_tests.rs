use std::{borrow::Cow, fs, path::Path};

use sqlx::{migrate::Migrator, PgPool};
use uuid::Uuid;

use crate::{
    modules::academic::cutover_test_support::{
        apply_migrations_through, seed_academic_cutover_fixture, CutoverFixture,
    },
    test_helpers::{create_named_test_pool, create_test_user},
};

#[test]
fn certificate_migration_is_forward_only_and_complete() {
    let migration = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/035_certificate_issuance.sql"),
    )
    .expect("migration 035 must exist");

    for required in [
        "CREATE TABLE certificate_academic_year_counters",
        "CREATE TABLE certificate_campaigns",
        "CREATE TABLE certificate_templates",
        "CREATE TABLE certificate_template_assets",
        "CREATE TABLE certificate_import_batches",
        "CREATE TABLE certificate_candidates",
        "CREATE TABLE certificate_issue_requests",
        "CREATE TABLE certificate_issue_request_items",
        "CREATE TABLE certificate_candidate_issue_locks",
        "CREATE TABLE certificate_issue_runs",
        "CREATE TABLE certificates",
        "ADD COLUMN inspection_metadata JSONB",
        "UNIQUE (certificate_number)",
        "UNIQUE (qr_proof_hash)",
    ] {
        assert!(migration.contains(required), "missing {required}");
    }

    assert!(
        !migration.to_ascii_lowercase().contains("national_id"),
        "certificate storage must never add a plaintext national ID field"
    );
}

#[test]
fn certificate_template_upload_relation_uses_a_follow_up_migration() {
    let migration = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/036_certificate_template_file_uploads.sql"),
    )
    .expect("migration 036 must exist");

    for required in [
        "CREATE TABLE certificate_template_file_uploads",
        "FOREIGN KEY (file_id, purpose_code)",
        "REFERENCES certificate_templates(id) ON DELETE CASCADE",
        "certificate_template_background",
        "certificate_template_image",
        "certificate_template_font",
        "files_certificate_template_private_check",
    ] {
        assert!(migration.contains(required), "missing {required}");
    }
}

#[test]
fn certificate_font_variant_migration_is_forward_only() {
    let migration = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/038_certificate_font_variants.sql"),
    )
    .expect("migration 038 must exist");

    for required in [
        "ADD COLUMN font_style TEXT",
        "SET font_style = 'normal'",
        "certificate_template_assets_kind_fields_check",
        "kind = 'image'",
        "font_style IS NULL",
        "kind = 'font'",
        "font_style IS NOT NULL",
        "font_style IN ('normal', 'italic')",
        "certificate_template_assets_font_variant_unique_idx",
        "lower(btrim(font_family))",
        "font_weight",
        "font_style",
    ] {
        assert!(migration.contains(required), "missing {required}");
    }

    let lower = migration.to_ascii_lowercase();
    assert!(
        !lower.contains("drop column"),
        "migration must preserve data"
    );
    assert!(
        !lower.contains("drop table"),
        "migration must preserve tables"
    );
    assert!(
        !lower.contains("national_id"),
        "font metadata must never introduce plaintext national IDs"
    );
}

#[test]
fn school_font_library_is_forward_only_private_and_reference_safe() {
    let migration = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/040_school_font_library.sql"),
    )
    .expect("migration 040 must exist");

    assert!(
        migration.trim_start().starts_with("DO $$"),
        "the legacy-empty proof must run before migration 040 changes schema"
    );
    for required in [
        "certificate_template_assets WHERE kind = 'font'",
        "certificate_template_file_uploads",
        "purpose_code = 'certificate_template_font'",
        "jsonb_array_elements(template.layout -> 'elements')",
        "element -> 'fontSource' ->> 'type' = 'asset'",
        "legacy certificate template fonts must be empty before migration 040",
        "CREATE TABLE school_font_file_uploads",
        "CREATE TABLE certificate_school_font_file_uploads",
        "CREATE TABLE school_fonts",
        "CREATE TABLE certificate_template_font_references",
        "font.manage.school",
        "FOREIGN KEY (file_id, purpose_code)",
        "REFERENCES school_fonts(id) ON DELETE RESTRICT",
    ] {
        assert!(migration.contains(required), "missing {required}");
    }

    let lower = migration.to_ascii_lowercase();
    for forbidden in [
        "national_id",
        "delete from certificate_template_assets",
        "delete from certificate_template_file_uploads",
        "insert into school_fonts select",
        "update certificate_template_assets set",
    ] {
        assert!(
            !lower.contains(forbidden),
            "migration 040 must not contain legacy backfill or cleanup fragment {forbidden:?}"
        );
    }
}

#[tokio::test]
async fn school_font_cutover_rejects_each_legacy_font_shape_before_schema_changes() {
    for (test_name, legacy_shape) in [
        ("font_cutover_asset", "asset"),
        ("font_cutover_upload", "upload"),
        ("font_cutover_layout", "layout"),
    ] {
        let pool = pre_school_font_cutover_pool(test_name).await;
        let actor_id =
            create_test_user(&pool, &format!("{test_name}@example.test"), "test-password")
                .await
                .expect("actor fixture should insert");
        let academic_year_id = insert_pre_school_font_academic_year(&pool).await;
        let campaign_id = insert_campaign(&pool, academic_year_id, actor_id, test_name, 1).await;
        let template_id = insert_template(&pool, campaign_id, legacy_shape).await;

        match legacy_shape {
            "asset" => {
                let file_id = insert_legacy_font_file(&pool, actor_id).await;
                sqlx::query(
                    "INSERT INTO certificate_template_assets (
                        template_id, file_id, kind, display_name, font_family,
                        font_weight, font_style, rights_confirmed_by,
                        rights_confirmed_at, created_by
                     ) VALUES (
                        $1, $2, 'font', 'Legacy font', 'Legacy Sans',
                        400, 'normal', $3, NOW(), $3
                     )",
                )
                .bind(template_id)
                .bind(file_id)
                .bind(actor_id)
                .execute(&pool)
                .await
                .expect("legacy font asset fixture should insert");
            }
            "upload" => {
                let file_id = insert_legacy_font_file(&pool, actor_id).await;
                sqlx::query(
                    "INSERT INTO certificate_template_file_uploads
                        (file_id, template_id, purpose_code, uploaded_by)
                     VALUES ($1, $2, 'certificate_template_font', $3)",
                )
                .bind(file_id)
                .bind(template_id)
                .bind(actor_id)
                .execute(&pool)
                .await
                .expect("legacy font upload fixture should insert");
            }
            "layout" => {
                sqlx::query(
                    r#"UPDATE certificate_templates
                       SET layout = '{"schemaVersion":1,"elements":[{"type":"text","fontSource":{"type":"asset"}}]}'::jsonb
                       WHERE id = $1"#,
                )
                .bind(template_id)
                .execute(&pool)
                .await
                .expect("legacy font layout fixture should update");
            }
            _ => unreachable!("test fixture enumerates every legacy shape"),
        }

        let error = apply_school_font_cutover(&pool)
            .await
            .expect_err("every legacy font shape must block migration 040");
        assert!(
            error
                .to_string()
                .contains("legacy certificate template fonts must be empty before migration 040"),
            "unexpected migration error: {error}"
        );
        assert!(
            !certificate_relation_exists(&pool, "school_fonts").await,
            "migration 040 must execute no DDL after a failed preflight"
        );
    }
}

#[test]
fn certificate_campaign_purge_is_forward_only_guarded_and_file_complete() {
    let migration = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/039_certificate_campaign_purge.sql"),
    )
    .expect("migration 039 must exist");

    for required in [
        "CREATE TABLE certificate_campaign_purge_jobs",
        "CREATE TABLE certificate_campaign_purge_files",
        "'purging'",
        "finalize_certificate_campaign_purge",
        "certificate_campaign_purge_guard_allows",
        "certificate_file_purge_guard_allows",
        "certificate_campaign_purge_has_external_file_consumer",
        "admission_application_documents",
        "school_settings",
        "academic_question_bank_questions",
        "academic_question_bank_choices",
        "file_versions_prevent_deletion",
        "file_derivatives_prevent_deletion",
    ] {
        assert!(migration.contains(required), "missing {required}");
    }

    assert!(
        !migration.to_ascii_lowercase().contains("national_id"),
        "campaign purge schema must never introduce plaintext national IDs"
    );
}

#[test]
fn issued_snapshots_and_idempotent_problem_rows_are_immutable_by_migration() {
    let issuance = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/035_certificate_issuance.sql"),
    )
    .expect("migration 035 must exist");
    let snapshot_trigger = issuance
        .split("CREATE FUNCTION enforce_certificate_snapshot_immutability()")
        .nth(1)
        .and_then(|value| {
            value
                .split("CREATE TRIGGER enforce_certificate_snapshot_immutability")
                .next()
        })
        .expect("snapshot immutability function must exist");
    for field in [
        "id",
        "campaign_id",
        "template_id",
        "candidate_id",
        "issue_run_id",
        "academic_year_id",
        "academic_year_value",
        "activity_sequence",
        "certificate_sequence",
        "check_digit",
        "certificate_number",
        "recipient_type",
        "user_id",
        "title_snapshot",
        "first_name_snapshot",
        "last_name_snapshot",
        "template_name_snapshot",
        "activity_item_snapshot",
        "award_or_role_snapshot",
        "custom_values_snapshot",
        "school_name_snapshot",
        "owner_organization_unit_name_snapshot",
        "issue_date",
        "qr_proof_encrypted",
        "qr_proof_hash",
        "replacement_for_certificate_id",
        "created_at",
    ] {
        assert!(
            snapshot_trigger.contains(&format!("NEW.{field}")),
            "snapshot trigger does not protect NEW.{field}"
        );
        assert!(
            snapshot_trigger.contains(&format!("OLD.{field}")),
            "snapshot trigger does not compare OLD.{field}"
        );
    }

    let issue_problems = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/037_certificate_issue_run_problems.sql"),
    )
    .expect("migration 037 must exist");
    for required in [
        "CREATE TABLE certificate_issue_run_problems",
        "PRIMARY KEY (issue_run_id, candidate_id)",
        "prevent_certificate_issue_run_problem_update",
        "prevent_certificate_issue_run_problem_delete",
    ] {
        assert!(issue_problems.contains(required), "missing {required}");
    }
    assert!(
        !issue_problems.to_ascii_lowercase().contains("national_id"),
        "issue result persistence must not add plaintext national IDs"
    );
}

fn assert_constraint_error<T>(result: Result<T, sqlx::Error>, expected_constraint: &str) {
    let error = match result {
        Ok(_) => panic!("invalid certificate state must be rejected"),
        Err(error) => error,
    };
    let sqlx::Error::Database(database_error) = error else {
        panic!("expected a database constraint error, got {error}");
    };
    assert_eq!(
        database_error.constraint(),
        Some(expected_constraint),
        "unexpected database error: {database_error}"
    );
}

async fn pre_school_font_cutover_pool(test_name: &str) -> PgPool {
    let pool = create_named_test_pool(test_name).await;
    let base = sqlx::migrate!("./migrations");
    let migrator = Migrator {
        migrations: Cow::Owned(
            base.iter()
                .filter(|migration| migration.version <= 39)
                .cloned()
                .collect(),
        ),
        locking: false,
        ..base
    };
    migrator
        .run(&pool)
        .await
        .expect("migrations through 039 should apply");
    pool
}

async fn apply_school_font_cutover(pool: &PgPool) -> Result<(), sqlx::Error> {
    let migration = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/040_school_font_library.sql"),
    )
    .expect("migration 040 must exist before cutover tests can pass");
    sqlx::raw_sql(&migration).execute(pool).await.map(|_| ())
}

async fn certificate_relation_exists(pool: &PgPool, relation: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1
            FROM pg_class AS relation
            JOIN pg_namespace AS namespace ON namespace.oid = relation.relnamespace
            WHERE namespace.nspname = current_schema()
              AND relation.relname = $1
         )",
    )
    .bind(relation)
    .fetch_one(pool)
    .await
    .expect("relation existence query should execute")
}

async fn insert_legacy_font_file(pool: &PgPool, actor_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO files (
            display_filename, purpose_code, visibility, lifecycle_status,
            retention_class, inspection_metadata, created_by
         ) VALUES (
            'legacy-font.ttf', 'certificate_template_font', 'private', 'ready',
            'temporary', '{\"kind\":\"font\"}'::jsonb, $1
         )
         RETURNING id",
    )
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .expect("legacy font file fixture should insert")
}

async fn insert_academic_year(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO academic_years (year, name, start_date, end_date, status)
         VALUES (2999, 'Certificate schema test', '2999-01-01', '2999-12-31', 'planning')
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("academic year fixture should insert")
}

async fn insert_pre_school_font_academic_year(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO academic_years (year, name, start_date, end_date, is_active)
         VALUES (2999, 'Certificate schema test', '2999-01-01', '2999-12-31', false)
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("pre-academic-cutover year fixture should insert")
}

async fn run_certificate_test_migrations(pool: &PgPool) {
    apply_migrations_through(pool, 40)
        .await
        .expect("apply pre-cutover certificate migrations");
    seed_academic_cutover_fixture(pool, CutoverFixture::Passing)
        .await
        .expect("seed certificate cutover fixture");
    apply_migrations_through(pool, 44)
        .await
        .expect("apply certificate academic cutover migrations");
    crate::utils::permission_sync::sync_permissions(pool)
        .await
        .expect("sync certificate fixture permissions");
}

async fn insert_campaign(
    pool: &PgPool,
    academic_year_id: Uuid,
    created_by: Uuid,
    name: &str,
    activity_sequence: i32,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO certificate_campaigns (
            academic_year_id, name, event_date, status, activity_sequence, created_by
         ) VALUES ($1, $2, '2999-08-14', 'active', $3, $4)
         RETURNING id",
    )
    .bind(academic_year_id)
    .bind(name)
    .bind(activity_sequence)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .expect("campaign fixture should insert")
}

async fn insert_template(pool: &PgPool, campaign_id: Uuid, suffix: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO certificate_templates (campaign_id, name, normalized_name)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(campaign_id)
    .bind(format!("Certificate template {suffix}"))
    .bind(format!("certificate-template-{suffix}"))
    .fetch_one(pool)
    .await
    .expect("template fixture should insert")
}

async fn insert_candidate(pool: &PgPool, campaign_id: Uuid, template_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO certificate_candidates (
            campaign_id, template_id, recipient_type,
            imported_first_name, imported_last_name, selected_name_source,
            match_status, validation_status
         ) VALUES (
            $1, $2, 'external', 'Certificate', 'Recipient', 'file',
            'external_confirmed', 'ready'
         )
         RETURNING id",
    )
    .bind(campaign_id)
    .bind(template_id)
    .fetch_one(pool)
    .await
    .expect("candidate fixture should insert")
}

async fn insert_issued_run(
    pool: &PgPool,
    campaign_id: Uuid,
    candidate_id: Uuid,
    actor_id: Uuid,
    certificate_sequence: i32,
) -> Uuid {
    let request_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_issue_requests (
            campaign_id, status, submitted_by, reviewed_by, reviewed_at, issued_at
         ) VALUES ($1, 'issued', $2, $2, NOW(), NOW())
         RETURNING id",
    )
    .bind(campaign_id)
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .expect("issued request fixture should insert");

    sqlx::query(
        "INSERT INTO certificate_issue_request_items (
            request_id, candidate_id, campaign_id
         ) VALUES ($1, $2, $3)",
    )
    .bind(request_id)
    .bind(candidate_id)
    .bind(campaign_id)
    .execute(pool)
    .await
    .expect("request item fixture should insert");

    sqlx::query_scalar(
        "INSERT INTO certificate_issue_runs (
            request_id, idempotency_key, issued_by, outcome, issued_count,
            first_certificate_sequence, last_certificate_sequence
         ) VALUES ($1, $2, $3, 'issued', 1, $4, $4)
         RETURNING id",
    )
    .bind(request_id)
    .bind(Uuid::new_v4())
    .bind(actor_id)
    .bind(certificate_sequence)
    .fetch_one(pool)
    .await
    .expect("issue run fixture should insert")
}

struct CertificateInsert<'a> {
    campaign_id: Uuid,
    template_id: Uuid,
    candidate_id: Uuid,
    issue_run_id: Uuid,
    academic_year_id: Uuid,
    activity_sequence: i32,
    certificate_sequence: i32,
    certificate_number: &'a str,
    proof_hash: &'a str,
}

async fn insert_certificate(
    pool: &PgPool,
    input: CertificateInsert<'_>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO certificates (
            campaign_id, template_id, candidate_id, issue_run_id, academic_year_id,
            academic_year_value, activity_sequence, certificate_sequence, check_digit,
            certificate_number, recipient_type, first_name_snapshot, last_name_snapshot,
            template_name_snapshot, school_name_snapshot, issue_date,
            qr_proof_encrypted, qr_proof_hash
         ) VALUES (
            $1, $2, $3, $4, $5,
            2999, $6, $7, 0,
            $8, 'external', 'Certificate', 'Recipient',
            'Certificate template', 'Certificate school', '2999-08-14',
            'encrypted-test-proof', $9
         )
         RETURNING id",
    )
    .bind(input.campaign_id)
    .bind(input.template_id)
    .bind(input.candidate_id)
    .bind(input.issue_run_id)
    .bind(input.academic_year_id)
    .bind(input.activity_sequence)
    .bind(input.certificate_sequence)
    .bind(input.certificate_number)
    .bind(input.proof_hash)
    .fetch_one(pool)
    .await
}

async fn insert_purge_file_fixture(
    pool: &PgPool,
    actor_id: Uuid,
    purpose_code: &str,
    display_filename: &str,
    deleted: bool,
) -> (Uuid, Uuid, Option<Uuid>) {
    let lifecycle_status = if deleted { "deleted" } else { "ready" };
    let file_id: Uuid = sqlx::query_scalar(
        "INSERT INTO files (
            display_filename, purpose_code, visibility, lifecycle_status,
            retention_class, inspection_metadata, created_by, deleted_at
         ) VALUES (
            $1, $2, 'private', $3, 'standard',
            '{\"kind\":\"unknown\"}'::jsonb, $4,
            CASE WHEN $3 = 'deleted' THEN NOW() ELSE NULL END
         ) RETURNING id",
    )
    .bind(display_filename)
    .bind(purpose_code)
    .bind(lifecycle_status)
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .expect("purge file fixture should insert");

    let storage_status = if deleted { "deleted" } else { "stored" };
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO file_versions (
            file_id, version_number, provider_code, storage_class,
            storage_status, object_key, detected_mime_type,
            canonical_extension, byte_size, checksum, scan_status,
            deleted_at, created_by
         ) VALUES (
            $1, 1, 'test', 'private', $2, $3, 'application/octet-stream',
            'bin', 10, repeat('a', 64), 'clean',
            CASE WHEN $2 = 'deleted' THEN NOW() ELSE NULL END, $4
         ) RETURNING id",
    )
    .bind(file_id)
    .bind(storage_status)
    .bind(format!("certificate-purge-test/{file_id}/original"))
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .expect("purge file version fixture should insert");

    sqlx::query("UPDATE files SET current_version_id = $2 WHERE id = $1")
        .bind(file_id)
        .bind(version_id)
        .execute(pool)
        .await
        .expect("purge file current version should update");

    let derivative_id = if deleted {
        let derivative_id: Uuid = sqlx::query_scalar(
            "INSERT INTO file_derivatives (
                file_id, source_version_id, derivative_kind, provider_code,
                storage_class, storage_status, object_key, detected_mime_type,
                canonical_extension, byte_size, checksum, lifecycle_status,
                deleted_at
             ) VALUES (
                $1, $2, 'preview', 'test', 'private', 'deleted', $3,
                'application/octet-stream', 'bin', 5, repeat('b', 64),
                'deleted', NOW()
             ) RETURNING id",
        )
        .bind(file_id)
        .bind(version_id)
        .bind(format!("certificate-purge-test/{file_id}/preview"))
        .fetch_one(pool)
        .await
        .expect("purge file derivative fixture should insert");
        Some(derivative_id)
    } else {
        None
    };

    sqlx::query(
        "INSERT INTO file_operations (
            file_id, file_version_id, operation_type, status,
            attempt_count, started_at, completed_at
         ) VALUES ($1, $2, 'delete_object', 'succeeded', 1, NOW(), NOW())",
    )
    .bind(file_id)
    .bind(version_id)
    .execute(pool)
    .await
    .expect("purge file operation fixture should insert");

    (file_id, version_id, derivative_id)
}

async fn assert_purge_finalizer_rejects_shared_file(pool: &PgPool, campaign_id: Uuid) {
    let error = sqlx::query_scalar::<_, bool>("SELECT finalize_certificate_campaign_purge($1)")
        .bind(campaign_id)
        .fetch_one(pool)
        .await
        .expect_err("shared File Platform consumer must block finalization");
    let sqlx::Error::Database(database_error) = error else {
        panic!("expected a database constraint error, got {error:?}");
    };
    assert_eq!(database_error.message(), "certificate_purge_file_shared");
}

#[tokio::test]
async fn purge_finalizer_removes_complete_campaign_file_and_audit_graph() {
    let pool = create_named_test_pool("certificate_campaign_purge_finalizer").await;
    run_certificate_test_migrations(&pool).await;

    let actor_id = create_test_user(&pool, "certificate-purge@example.test", "test-password")
        .await
        .expect("actor fixture should insert");
    let academic_year_id = insert_academic_year(&pool).await;
    sqlx::query(
        "INSERT INTO certificate_academic_year_counters (
            academic_year_id, next_activity_sequence
         ) VALUES ($1, 12)",
    )
    .bind(academic_year_id)
    .execute(&pool)
    .await
    .expect("counter fixture should insert");

    let campaign_id =
        insert_campaign(&pool, academic_year_id, actor_id, "Permanent purge", 1).await;
    let template_id = insert_template(&pool, campaign_id, "purge").await;
    let (background_file_id, _, _) = insert_purge_file_fixture(
        &pool,
        actor_id,
        "certificate_template_background",
        "background.pdf",
        true,
    )
    .await;
    let (image_file_id, _, _) = insert_purge_file_fixture(
        &pool,
        actor_id,
        "certificate_template_image",
        "image.png",
        true,
    )
    .await;
    let (font_file_id, _, _) =
        insert_purge_file_fixture(&pool, actor_id, "school_font", "font.ttf", false).await;
    let file_ids = vec![background_file_id, image_file_id];

    sqlx::query(
        "UPDATE certificate_templates
         SET background_file_id = $2,
             crop_box_x = 0, crop_box_y = 0,
             crop_box_width = 842, crop_box_height = 595,
             media_box_x = 0, media_box_y = 0,
             media_box_width = 842, media_box_height = 595,
             page_rotation = 0, paper_label = 'A4 landscape'
         WHERE id = $1",
    )
    .bind(template_id)
    .bind(background_file_id)
    .execute(&pool)
    .await
    .expect("background fixture should attach");

    for (file_id, purpose_code) in [
        (background_file_id, "certificate_template_background"),
        (image_file_id, "certificate_template_image"),
    ] {
        sqlx::query(
            "INSERT INTO certificate_template_file_uploads
                (file_id, template_id, purpose_code, uploaded_by)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(file_id)
        .bind(template_id)
        .bind(purpose_code)
        .bind(actor_id)
        .execute(&pool)
        .await
        .expect("certificate upload relation fixture should insert");
    }

    sqlx::query(
        "INSERT INTO certificate_template_assets (
            template_id, file_id, kind, display_name, created_by
         ) VALUES ($1, $2, 'image', 'Seal', $3)",
    )
    .bind(template_id)
    .bind(image_file_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("image asset fixture should insert");
    let school_font_id: Uuid = sqlx::query_scalar(
        "INSERT INTO school_fonts (
            file_id, display_name, font_family, normalized_family,
            font_weight, font_style, rights_confirmed_by,
            rights_confirmed_at, created_by
         ) VALUES (
            $1, 'School font', 'School Sans', 'school sans',
            400, 'normal', $2, NOW(), $2
         )
         RETURNING id",
    )
    .bind(font_file_id)
    .bind(actor_id)
    .fetch_one(&pool)
    .await
    .expect("shared school font fixture should insert");
    sqlx::query(
        "INSERT INTO certificate_template_font_references (template_id, font_id)
         VALUES ($1, $2)",
    )
    .bind(template_id)
    .bind(school_font_id)
    .execute(&pool)
    .await
    .expect("shared school font reference fixture should insert");

    let issued_candidate_id = insert_candidate(&pool, campaign_id, template_id).await;
    let issue_run_id =
        insert_issued_run(&pool, campaign_id, issued_candidate_id, actor_id, 1).await;
    let certificate_id = insert_certificate(
        &pool,
        CertificateInsert {
            campaign_id,
            template_id,
            candidate_id: issued_candidate_id,
            issue_run_id,
            academic_year_id,
            activity_sequence: 1,
            certificate_sequence: 1,
            certificate_number: "2999-0001-000001-0",
            proof_hash: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        },
    )
    .await
    .expect("certificate fixture should insert");
    sqlx::query(
        "UPDATE certificates
         SET status = 'revoked', revoked_by = $2, revoked_at = NOW(),
             revocation_reason = 'Schema purge fixture'
         WHERE id = $1",
    )
    .bind(certificate_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("certificate fixture should revoke");
    sqlx::query("UPDATE certificate_candidates SET issued_certificate_id = $2 WHERE id = $1")
        .bind(issued_candidate_id)
        .bind(certificate_id)
        .execute(&pool)
        .await
        .expect("candidate certificate link fixture should update");
    sqlx::query(
        "INSERT INTO certificate_issue_run_problems
            (issue_run_id, candidate_id, issue_codes)
         VALUES ($1, $2, ARRAY['fixture_problem'])",
    )
    .bind(issue_run_id)
    .bind(issued_candidate_id)
    .execute(&pool)
    .await
    .expect("issue problem fixture should insert");

    let open_candidate_id = insert_candidate(&pool, campaign_id, template_id).await;
    let open_request_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_issue_requests (campaign_id, submitted_by)
         VALUES ($1, $2) RETURNING id",
    )
    .bind(campaign_id)
    .bind(actor_id)
    .fetch_one(&pool)
    .await
    .expect("open request fixture should insert");
    sqlx::query(
        "INSERT INTO certificate_issue_request_items
            (request_id, candidate_id, campaign_id)
         VALUES ($1, $2, $3)",
    )
    .bind(open_request_id)
    .bind(open_candidate_id)
    .bind(campaign_id)
    .execute(&pool)
    .await
    .expect("open request item fixture should insert");
    sqlx::query(
        "INSERT INTO certificate_candidate_issue_locks (candidate_id, request_id)
         VALUES ($1, $2)",
    )
    .bind(open_candidate_id)
    .bind(open_request_id)
    .execute(&pool)
    .await
    .expect("open request lock fixture should insert");

    for (entity_type, entity_id) in [
        ("certificate_campaign", campaign_id),
        ("certificate_template", template_id),
        ("certificate_candidate", issued_candidate_id),
        ("certificate_issue_request", open_request_id),
        ("certificate", certificate_id),
    ] {
        sqlx::query(
            "INSERT INTO audit_logs (
                user_id, action, entity_type, entity_id, metadata
             ) VALUES ($1, 'fixture', $2, $3, jsonb_build_object('campaignId', $4::TEXT))",
        )
        .bind(actor_id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(campaign_id)
        .execute(&pool)
        .await
        .expect("campaign audit fixture should insert");
    }

    assert!(
        sqlx::query("DELETE FROM certificates WHERE id = $1")
            .bind(certificate_id)
            .execute(&pool)
            .await
            .is_err(),
        "certificate deletion must remain guarded"
    );
    assert!(
        sqlx::query("DELETE FROM certificate_campaigns WHERE id = $1")
            .bind(campaign_id)
            .execute(&pool)
            .await
            .is_err(),
        "campaign deletion must remain guarded"
    );

    sqlx::query("UPDATE certificate_campaigns SET status = 'purging' WHERE id = $1")
        .bind(campaign_id)
        .execute(&pool)
        .await
        .expect("campaign fixture should enter purging");
    sqlx::query(
        "INSERT INTO certificate_campaign_purge_jobs (
            campaign_id, status, requested_by, template_count,
            candidate_count, request_count, open_request_count,
            issued_certificate_count, revoked_certificate_count,
            file_count, total_file_bytes
         ) VALUES ($1, 'finalizing', $2, 1, 2, 2, 1, 1, 1, 2, 30)",
    )
    .bind(campaign_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("purge job fixture should insert");
    for file_id in &file_ids {
        sqlx::query(
            "INSERT INTO certificate_campaign_purge_files
                (campaign_id, file_id, object_count, byte_size)
             VALUES ($1, $2, 2, 15)",
        )
        .bind(campaign_id)
        .bind(file_id)
        .execute(&pool)
        .await
        .expect("purge inventory fixture should insert");
    }

    let finalized: bool = sqlx::query_scalar("SELECT finalize_certificate_campaign_purge($1)")
        .bind(campaign_id)
        .fetch_one(&pool)
        .await
        .expect("guarded purge finalizer should succeed");
    assert!(finalized);

    for (table, expected_count) in [
        // The academic cutover fixture owns one unrelated campaign. The campaign under
        // purge must be gone while that migration fixture remains intact.
        ("certificate_campaigns", 1_i64),
        ("certificate_campaign_purge_jobs", 0),
        ("certificate_campaign_purge_files", 0),
        ("certificate_templates", 0),
        ("certificate_candidates", 0),
        ("certificate_issue_requests", 0),
        ("certificate_issue_request_items", 0),
        ("certificate_candidate_issue_locks", 0),
        ("certificate_issue_runs", 0),
        ("certificate_issue_run_problems", 0),
        ("certificates", 0),
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .expect("purged domain table should remain queryable");
        assert_eq!(count, expected_count, "unexpected rows in {table}");
    }

    let file_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE id = ANY($1)")
        .bind(&file_ids)
        .fetch_one(&pool)
        .await
        .expect("purged files should be countable");
    assert_eq!(file_count, 0);
    let retained_school_font: (i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM files WHERE id = $1),
            (SELECT COUNT(*) FROM school_fonts WHERE id = $2),
            (SELECT COUNT(*) FROM certificate_template_font_references WHERE font_id = $2)",
    )
    .bind(font_file_id)
    .bind(school_font_id)
    .fetch_one(&pool)
    .await
    .expect("shared font retention should remain queryable");
    assert_eq!(
        retained_school_font,
        (1, 1, 0),
        "campaign purge must remove only the certificate reference"
    );
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs
         WHERE entity_type LIKE 'certificate%'
           AND metadata ->> 'campaignId' = $1",
    )
    .bind(campaign_id.to_string())
    .fetch_one(&pool)
    .await
    .expect("purged audit rows should be countable");
    assert_eq!(audit_count, 0);
    let next_activity_sequence: i32 = sqlx::query_scalar(
        "SELECT next_activity_sequence
         FROM certificate_academic_year_counters
         WHERE academic_year_id = $1",
    )
    .bind(academic_year_id)
    .fetch_one(&pool)
    .await
    .expect("academic-year counter should remain");
    assert_eq!(next_activity_sequence, 12);
}

#[tokio::test]
async fn purge_finalizer_rolls_back_when_storage_is_not_deleted() {
    let pool = create_named_test_pool("certificate_campaign_purge_incomplete_storage").await;
    run_certificate_test_migrations(&pool).await;

    let actor_id = create_test_user(
        &pool,
        "certificate-purge-incomplete@example.test",
        "test-password",
    )
    .await
    .expect("actor fixture should insert");
    let academic_year_id = insert_academic_year(&pool).await;
    let campaign_id =
        insert_campaign(&pool, academic_year_id, actor_id, "Incomplete purge", 1).await;
    let template_id = insert_template(&pool, campaign_id, "incomplete-purge").await;
    let (file_id, _, _) = insert_purge_file_fixture(
        &pool,
        actor_id,
        "certificate_template_background",
        "not-deleted.pdf",
        false,
    )
    .await;
    sqlx::query(
        "INSERT INTO certificate_template_file_uploads
            (file_id, template_id, purpose_code, uploaded_by)
         VALUES ($1, $2, 'certificate_template_background', $3)",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("upload relation fixture should insert");
    sqlx::query("UPDATE certificate_campaigns SET status = 'purging' WHERE id = $1")
        .bind(campaign_id)
        .execute(&pool)
        .await
        .expect("campaign fixture should enter purging");
    sqlx::query(
        "INSERT INTO certificate_campaign_purge_jobs (
            campaign_id, status, requested_by, template_count,
            candidate_count, request_count, open_request_count,
            issued_certificate_count, revoked_certificate_count,
            file_count, total_file_bytes
         ) VALUES ($1, 'finalizing', $2, 1, 0, 0, 0, 0, 0, 1, 10)",
    )
    .bind(campaign_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("purge job fixture should insert");
    sqlx::query(
        "INSERT INTO certificate_campaign_purge_files
            (campaign_id, file_id, object_count, byte_size)
         VALUES ($1, $2, 1, 10)",
    )
    .bind(campaign_id)
    .bind(file_id)
    .execute(&pool)
    .await
    .expect("purge inventory fixture should insert");

    let result = sqlx::query_scalar::<_, bool>("SELECT finalize_certificate_campaign_purge($1)")
        .bind(campaign_id)
        .fetch_one(&pool)
        .await;
    assert!(
        result.is_err(),
        "incomplete storage must block finalization"
    );

    let campaign_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM certificate_campaigns WHERE id = $1")
            .bind(campaign_id)
            .fetch_one(&pool)
            .await
            .expect("campaign should remain after rollback");
    let template_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM certificate_templates WHERE id = $1")
            .bind(template_id)
            .fetch_one(&pool)
            .await
            .expect("template should remain after rollback");
    let file_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE id = $1")
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .expect("file should remain after rollback");
    assert_eq!((campaign_count, template_count, file_count), (1, 1, 1));
}

#[tokio::test]
async fn purge_finalizer_rolls_back_when_inventory_file_has_external_domain_references() {
    let pool = create_named_test_pool("certificate_campaign_purge_external_references").await;
    run_certificate_test_migrations(&pool).await;

    let actor_id = create_test_user(
        &pool,
        "certificate-purge-external-reference@example.test",
        "test-password",
    )
    .await
    .expect("actor fixture should insert");
    let academic_year_id = insert_academic_year(&pool).await;
    let campaign_id = insert_campaign(
        &pool,
        academic_year_id,
        actor_id,
        "External reference purge",
        1,
    )
    .await;
    let template_id = insert_template(&pool, campaign_id, "external-reference-purge").await;
    let (file_id, _, _) = insert_purge_file_fixture(
        &pool,
        actor_id,
        "certificate_template_background",
        "externally-referenced.pdf",
        true,
    )
    .await;
    sqlx::query(
        "INSERT INTO certificate_template_file_uploads
            (file_id, template_id, purpose_code, uploaded_by)
         VALUES ($1, $2, 'certificate_template_background', $3)",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("upload relation fixture should insert");
    sqlx::query("UPDATE users SET profile_image_file_id = $2 WHERE id = $1")
        .bind(actor_id)
        .bind(file_id)
        .execute(&pool)
        .await
        .expect("profile image reference fixture should update");
    let achievement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO staff_achievements (
            user_id, title, achievement_date, created_by, image_file_id
         ) VALUES ($1, 'External file reference', CURRENT_DATE, $1, $2)
         RETURNING id",
    )
    .bind(actor_id)
    .bind(file_id)
    .fetch_one(&pool)
    .await
    .expect("staff achievement reference fixture should insert");
    sqlx::query("UPDATE certificate_campaigns SET status = 'purging' WHERE id = $1")
        .bind(campaign_id)
        .execute(&pool)
        .await
        .expect("campaign fixture should enter purging");
    sqlx::query(
        "INSERT INTO certificate_campaign_purge_jobs (
            campaign_id, status, requested_by, template_count,
            candidate_count, request_count, open_request_count,
            issued_certificate_count, revoked_certificate_count,
            file_count, total_file_bytes
         ) VALUES ($1, 'finalizing', $2, 1, 0, 0, 0, 0, 0, 1, 15)",
    )
    .bind(campaign_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("purge job fixture should insert");
    sqlx::query(
        "INSERT INTO certificate_campaign_purge_files
            (campaign_id, file_id, object_count, byte_size)
         VALUES ($1, $2, 2, 15)",
    )
    .bind(campaign_id)
    .bind(file_id)
    .execute(&pool)
    .await
    .expect("purge inventory fixture should insert");

    let result = sqlx::query_scalar::<_, bool>("SELECT finalize_certificate_campaign_purge($1)")
        .bind(campaign_id)
        .fetch_one(&pool)
        .await;
    assert!(
        result.is_err(),
        "external profile and achievement references must block finalization"
    );

    let state: (i64, i64, Option<Uuid>, Option<Uuid>, i64) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM certificate_campaigns WHERE id = $1),
            (SELECT COUNT(*) FROM files WHERE id = $2),
            (SELECT profile_image_file_id FROM users WHERE id = $3),
            (SELECT image_file_id FROM staff_achievements WHERE id = $4),
            (SELECT COUNT(*) FROM certificate_campaign_purge_jobs WHERE campaign_id = $1)",
    )
    .bind(campaign_id)
    .bind(file_id)
    .bind(actor_id)
    .bind(achievement_id)
    .fetch_one(&pool)
    .await
    .expect("guarded state should remain queryable");
    assert_eq!(state, (1, 1, Some(file_id), Some(file_id), 1));
}

#[tokio::test]
async fn purge_finalizer_rejects_admission_logo_and_question_bank_file_consumers() {
    let pool = create_named_test_pool("certificate_campaign_purge_all_file_consumers").await;
    run_certificate_test_migrations(&pool).await;

    let actor_id = create_test_user(
        &pool,
        "certificate-purge-all-consumers@example.test",
        "test-password",
    )
    .await
    .expect("actor fixture should insert");
    let academic_year_id = insert_academic_year(&pool).await;
    let campaign_id = insert_campaign(
        &pool,
        academic_year_id,
        actor_id,
        "All file consumers purge",
        1,
    )
    .await;
    let template_id = insert_template(&pool, campaign_id, "all-consumers-purge").await;
    let (file_id, _, _) = insert_purge_file_fixture(
        &pool,
        actor_id,
        "certificate_template_background",
        "all-consumers.pdf",
        true,
    )
    .await;
    sqlx::query(
        "INSERT INTO certificate_template_file_uploads
            (file_id, template_id, purpose_code, uploaded_by)
         VALUES ($1, $2, 'certificate_template_background', $3)",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("upload relation fixture should insert");
    sqlx::query("UPDATE certificate_campaigns SET status = 'purging' WHERE id = $1")
        .bind(campaign_id)
        .execute(&pool)
        .await
        .expect("campaign fixture should enter purging");
    sqlx::query(
        "INSERT INTO certificate_campaign_purge_jobs (
            campaign_id, status, requested_by, template_count,
            candidate_count, request_count, open_request_count,
            issued_certificate_count, revoked_certificate_count,
            file_count, total_file_bytes
         ) VALUES ($1, 'finalizing', $2, 1, 0, 0, 0, 0, 0, 1, 15)",
    )
    .bind(campaign_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .expect("purge job fixture should insert");
    sqlx::query(
        "INSERT INTO certificate_campaign_purge_files
            (campaign_id, file_id, object_count, byte_size)
         VALUES ($1, $2, 2, 15)",
    )
    .bind(campaign_id)
    .bind(file_id)
    .execute(&pool)
    .await
    .expect("purge inventory fixture should insert");

    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("grade-level fixture should exist");
    let study_program_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM study_programs
         WHERE status = 'published'
         ORDER BY is_default DESC, id
         LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("study-program fixture should exist");
    let admission_round_id: Uuid = sqlx::query_scalar(
        "INSERT INTO admission_rounds (
            academic_year_id, grade_level_id, name, apply_start_date, apply_end_date
         ) VALUES ($1, $2, 'รอบทดสอบ finalizer', CURRENT_DATE, CURRENT_DATE)
         RETURNING id",
    )
    .bind(academic_year_id)
    .bind(grade_level_id)
    .fetch_one(&pool)
    .await
    .expect("admission-round fixture should insert");
    let admission_track_id: Uuid = sqlx::query_scalar(
        "INSERT INTO admission_tracks (
             admission_round_id, academic_year_id, study_program_id, name
         )
         VALUES ($1, $2, $3, 'แผนรับสมัคร finalizer')
         RETURNING id",
    )
    .bind(admission_round_id)
    .bind(academic_year_id)
    .bind(study_program_id)
    .fetch_one(&pool)
    .await
    .expect("admission-track fixture should insert");
    let application_id: Uuid = sqlx::query_scalar(
        "INSERT INTO admission_applications (
            admission_round_id, admission_track_id, national_id,
            national_id_hash, first_name, last_name
         ) VALUES ($1, $2, 'encrypted-finalizer-fixture', repeat('e', 64),
                   'Finalizer', 'Applicant')
         RETURNING id",
    )
    .bind(admission_round_id)
    .bind(admission_track_id)
    .fetch_one(&pool)
    .await
    .expect("admission-application fixture should insert");
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO admission_application_documents (application_id, file_id, doc_type)
         VALUES ($1, $2, 'purge_finalizer_fixture')
         RETURNING id",
    )
    .bind(application_id)
    .bind(file_id)
    .fetch_one(&pool)
    .await
    .expect("admission-document fixture should insert");
    assert_purge_finalizer_rejects_shared_file(&pool, campaign_id).await;
    sqlx::query("DELETE FROM admission_application_documents WHERE id = $1")
        .bind(document_id)
        .execute(&pool)
        .await
        .expect("admission-document fixture should clear");

    sqlx::query("UPDATE school_settings SET logo_file_id = $1")
        .bind(file_id)
        .execute(&pool)
        .await
        .expect("school-logo fixture should update");
    assert_purge_finalizer_rejects_shared_file(&pool, campaign_id).await;
    sqlx::query("UPDATE school_settings SET logo_file_id = NULL WHERE logo_file_id = $1")
        .bind(file_id)
        .execute(&pool)
        .await
        .expect("school-logo fixture should clear");

    let subject_id: Uuid = sqlx::query_scalar("SELECT id FROM subjects ORDER BY code, id LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("question-bank stable subject fixture should exist");
    let rich_content = serde_json::json!({
        "schemaVersion": 1,
        "document": {
            "type": "doc",
            "content": [{
                "type": "image",
                "attrs": {
                    "fileId": file_id,
                    "altText": null,
                    "caption": null,
                    "alignment": "center",
                    "widthPercent": 50
                }
            }]
        }
    });
    let question_id: Uuid = sqlx::query_scalar(
        "INSERT INTO academic_question_bank_questions (
            subject_id, owner_user_id, stem_content, created_by, updated_by
         ) VALUES ($1, $2, $3, $2, $2)
         RETURNING id",
    )
    .bind(subject_id)
    .bind(actor_id)
    .bind(sqlx::types::Json(rich_content))
    .fetch_one(&pool)
    .await
    .expect("question-bank fixture should insert");
    assert_purge_finalizer_rejects_shared_file(&pool, campaign_id).await;

    let state: (String, String, i64, i64) = sqlx::query_as(
        "SELECT campaign.status, job.status,
                (SELECT COUNT(*) FROM files WHERE id = $2),
                (SELECT COUNT(*) FROM academic_question_bank_questions WHERE id = $3)
         FROM certificate_campaigns AS campaign
         JOIN certificate_campaign_purge_jobs AS job ON job.campaign_id = campaign.id
         WHERE campaign.id = $1",
    )
    .bind(campaign_id)
    .bind(file_id)
    .bind(question_id)
    .fetch_one(&pool)
    .await
    .expect("guarded state should remain queryable");
    assert_eq!(
        state,
        ("purging".to_string(), "finalizing".to_string(), 1, 1)
    );
}

#[tokio::test]
async fn migrated_schema_enforces_certificate_invariants() {
    let pool = create_named_test_pool("certificate_schema_invariants").await;
    run_certificate_test_migrations(&pool).await;

    let actor_id = create_test_user(&pool, "certificate-schema@example.test", "test-password")
        .await
        .expect("actor fixture should insert");
    let academic_year_id = insert_academic_year(&pool).await;

    assert_constraint_error(
        sqlx::query(
            "INSERT INTO certificate_campaigns (
                academic_year_id, name, event_date, status
             ) VALUES ($1, 'Invalid campaign', '2999-08-14', 'publishing')",
        )
        .bind(academic_year_id)
        .execute(&pool)
        .await,
        "certificate_campaigns_status_check",
    );

    assert_constraint_error(
        sqlx::query(
            "INSERT INTO files (
                display_filename, purpose_code, visibility, lifecycle_status,
                retention_class, inspection_metadata
             ) VALUES (
                'invalid-inspection.bin', 'generic_private_document', 'private',
                'processing', 'standard', '[]'::jsonb
             )",
        )
        .execute(&pool)
        .await,
        "files_inspection_metadata_check",
    );

    let campaign_one = insert_campaign(&pool, academic_year_id, actor_id, "Campaign one", 1).await;

    assert_constraint_error(
        sqlx::query(
            r#"INSERT INTO certificate_templates (
                campaign_id, name, normalized_name, layout
             ) VALUES (
                $1, 'Invalid layout', 'invalid-layout',
                '{"schemaVersion":1,"elements":{}}'::jsonb
             )"#,
        )
        .bind(campaign_one)
        .execute(&pool)
        .await,
        "certificate_templates_layout_check",
    );

    assert_constraint_error(
        sqlx::query(
            "INSERT INTO certificate_templates (
                campaign_id, name, normalized_name, crop_box_x
             ) VALUES ($1, 'Incomplete geometry', 'incomplete-geometry', 0)",
        )
        .bind(campaign_one)
        .execute(&pool)
        .await,
        "certificate_templates_background_geometry_check",
    );

    let template_one = insert_template(&pool, campaign_one, "one").await;
    assert_constraint_error(
        sqlx::query(
            "INSERT INTO files (
                display_filename, purpose_code, visibility, lifecycle_status,
                retention_class, inspection_metadata
             ) VALUES (
                'public-certificate-background.pdf',
                'certificate_template_background', 'public', 'processing',
                'temporary', '{\"kind\":\"unknown\"}'::jsonb
             )",
        )
        .execute(&pool)
        .await,
        "files_certificate_template_private_check",
    );
    let related_file_id: Uuid = sqlx::query_scalar(
        "INSERT INTO files (
            display_filename, purpose_code, visibility, lifecycle_status,
            retention_class, inspection_metadata, created_by
         ) VALUES (
            'certificate-background.pdf', 'certificate_template_background',
            'private', 'processing', 'temporary', '{\"kind\":\"unknown\"}'::jsonb, $1
         ) RETURNING id",
    )
    .bind(actor_id)
    .fetch_one(&pool)
    .await
    .expect("certificate template file fixture should insert");
    assert_constraint_error(
        sqlx::query(
            "INSERT INTO certificate_template_file_uploads
                (file_id, template_id, purpose_code, uploaded_by)
             VALUES ($1, $2, 'certificate_template_image', $3)",
        )
        .bind(related_file_id)
        .bind(template_one)
        .bind(actor_id)
        .execute(&pool)
        .await,
        "certificate_template_file_uploads_file_purpose_fkey",
    );
    let locked_candidate = insert_candidate(&pool, campaign_one, template_one).await;
    let first_request: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_issue_requests (campaign_id, submitted_by)
         VALUES ($1, $2)
         RETURNING id",
    )
    .bind(campaign_one)
    .bind(actor_id)
    .fetch_one(&pool)
    .await
    .expect("first pending request should insert");
    let second_request: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_issue_requests (campaign_id, submitted_by)
         VALUES ($1, $2)
         RETURNING id",
    )
    .bind(campaign_one)
    .bind(actor_id)
    .fetch_one(&pool)
    .await
    .expect("second pending request should insert");

    for request_id in [first_request, second_request] {
        sqlx::query(
            "INSERT INTO certificate_issue_request_items (
                request_id, candidate_id, campaign_id
             ) VALUES ($1, $2, $3)",
        )
        .bind(request_id)
        .bind(locked_candidate)
        .bind(campaign_one)
        .execute(&pool)
        .await
        .expect("request item should insert");
    }

    sqlx::query(
        "INSERT INTO certificate_candidate_issue_locks (candidate_id, request_id)
         VALUES ($1, $2)",
    )
    .bind(locked_candidate)
    .bind(first_request)
    .execute(&pool)
    .await
    .expect("first active candidate lock should insert");
    assert_constraint_error(
        sqlx::query(
            "INSERT INTO certificate_candidate_issue_locks (candidate_id, request_id)
             VALUES ($1, $2)",
        )
        .bind(locked_candidate)
        .bind(second_request)
        .execute(&pool)
        .await,
        "certificate_candidate_issue_locks_pkey",
    );

    let first_candidate = insert_candidate(&pool, campaign_one, template_one).await;
    let first_run = insert_issued_run(&pool, campaign_one, first_candidate, actor_id, 1).await;
    insert_certificate(
        &pool,
        CertificateInsert {
            campaign_id: campaign_one,
            template_id: template_one,
            candidate_id: first_candidate,
            issue_run_id: first_run,
            academic_year_id,
            activity_sequence: 1,
            certificate_sequence: 1,
            certificate_number: "2999-0001-000001-0",
            proof_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        },
    )
    .await
    .expect("first certificate should insert");

    let campaign_two = insert_campaign(&pool, academic_year_id, actor_id, "Campaign two", 2).await;
    let template_two = insert_template(&pool, campaign_two, "two").await;
    let duplicate_number_candidate = insert_candidate(&pool, campaign_two, template_two).await;
    let duplicate_number_run =
        insert_issued_run(&pool, campaign_two, duplicate_number_candidate, actor_id, 1).await;
    let duplicate_number = insert_certificate(
        &pool,
        CertificateInsert {
            campaign_id: campaign_two,
            template_id: template_two,
            candidate_id: duplicate_number_candidate,
            issue_run_id: duplicate_number_run,
            academic_year_id,
            activity_sequence: 1,
            certificate_sequence: 1,
            certificate_number: "2999-0001-000001-0",
            proof_hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        },
    )
    .await;
    assert_constraint_error(duplicate_number, "certificates_certificate_number_key");

    let duplicate_proof_candidate = insert_candidate(&pool, campaign_two, template_two).await;
    let duplicate_proof_run =
        insert_issued_run(&pool, campaign_two, duplicate_proof_candidate, actor_id, 2).await;
    let duplicate_proof = insert_certificate(
        &pool,
        CertificateInsert {
            campaign_id: campaign_two,
            template_id: template_two,
            candidate_id: duplicate_proof_candidate,
            issue_run_id: duplicate_proof_run,
            academic_year_id,
            activity_sequence: 2,
            certificate_sequence: 2,
            certificate_number: "2999-0002-000002-0",
            proof_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        },
    )
    .await;
    assert_constraint_error(duplicate_proof, "certificates_qr_proof_hash_key");
}
