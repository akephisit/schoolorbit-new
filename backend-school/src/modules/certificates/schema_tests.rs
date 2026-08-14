use std::path::Path;

use sqlx::PgPool;
use uuid::Uuid;

use crate::test_helpers::{create_named_test_pool, create_test_user, run_test_migrations};

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

async fn insert_academic_year(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO academic_years (year, name, start_date, end_date)
         VALUES (2999, 'Certificate schema test', '2999-01-01', '2999-12-31')
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("academic year fixture should insert")
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

#[tokio::test]
async fn migrated_schema_enforces_certificate_invariants() {
    let pool = create_named_test_pool("certificate_schema_invariants").await;
    run_test_migrations(&pool).await;

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
