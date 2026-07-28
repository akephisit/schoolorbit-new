use crate::test_helpers::{create_test_pool, run_test_migrations};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const PRE_FILE_PLATFORM_MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_baseline.sql",
        "ee504bda3d290474b0aec46703aeb5e8ff3536142876e4055f03723f0219c774",
    ),
    (
        "002_menu_workspace_code.sql",
        "3a934a93438007f9e07f290810b116a5c6dc9244126543436615f698ec44c825",
    ),
    (
        "003_workflow_windows.sql",
        "97a7d0bd89dc59e090ee17c62fe4caef845ddafd23c9d3263ac17b356c7b6616",
    ),
    (
        "004_work_items.sql",
        "c60afe02b0b021e8930fbbba2f9f768230550e19ce30d717f82a2c3dc9494186",
    ),
    (
        "005_teaching_supervision.sql",
        "6532b41cc1f5ffc19907f33c7ef20225988fa55e2910c04b28bba3e3ced07b1b",
    ),
    (
        "006_teaching_supervision_default_permissions.sql",
        "a22339758a5ad7a94f516efc51914e3e90f44e87b69ce771a9ca7bf1cc10cf35",
    ),
    (
        "007_teaching_supervision_scoped_management_permissions.sql",
        "1135b43976ef228929fbfb0c16fb7e26572cb2b7aac07359cc12461911980e42",
    ),
    (
        "008_supervision_observed_at.sql",
        "2fce53a13b99e32154aaad134ac97d6c37194cc53205f8673e96b88314a5a9a7",
    ),
    (
        "009_supervision_simplified_approval_flow.sql",
        "0d9ffdb6991daea9904af4a5eab3075dbb074aada3b440f05ce8fab773af79ff",
    ),
    (
        "010_supervision_academic_affairs_approval_grant.sql",
        "a0c9a39d494d51cf1f7b79560f4f2d1a778df19d7506a224d85ca2fc3bb2d8d3",
    ),
    (
        "011_daily_teaching_overview_permission.sql",
        "40e4d69c0bcb6e5e6bac9524f4b16274680d1f0b61f9733e2d8565bc1426c48f",
    ),
    (
        "012_academic_assessment_plans.sql",
        "4357e907426d0ab97c3fedf485da9a587b6e95fcdd723585e193104516b1c59d",
    ),
    (
        "013_academic_assessment_teacher_access.sql",
        "265306f153b8ee0ebf8565480d623a09e71d3eddfc1c9b9173ee6aba962d33c8",
    ),
    (
        "014_academic_assessment_subject_plans.sql",
        "0bb1e627bbccb0f8dde30a2879b743facb8c80c4c0036243eb863ffc198172f1",
    ),
    (
        "015_academic_assessment_subject_group_read.sql",
        "0ad122e5768598d230b88c0ea785242e262657bd5febb4f26b163884ef4b1ea0",
    ),
    (
        "016_academic_assessment_saved_status.sql",
        "300c8b6d43795f5ec5672e42f056964abe971e8becc2bd00c4b5d2aa8206a018",
    ),
    (
        "017_drop_subject_default_instructor_id.sql",
        "e34bb522bbf8e752dab10fb6d2baebd4f67264fab687a1087956475f3098611b",
    ),
    (
        "018_school_calendar.sql",
        "b6662c44d8918c63ed38044169de2aa9ef934963406915e87404b8a53fdbc668",
    ),
    (
        "019_academic_exam_schedule.sql",
        "11300afdcdc2ea8f398f80c888c5ceb3edb1ec517a65e8dd886a6054ea32444e",
    ),
    (
        "020_academic_exam_invigilator_live_range_conflicts.sql",
        "df1a4672442cc71966cae078c05aa2d82b6f8d7a6a1118e95450318fc9a8ab5d",
    ),
    (
        "021_academic_exam_day_drop_sort_order.sql",
        "c513de3273ca258157e7791a8c73acfddbbe8b9b9626a3bc95bba40db4e3dcb7",
    ),
    (
        "022_academic_exam_round_exam_kind.sql",
        "3202ffdd4e4d88c5de8a3465366709878e24e2998435a5d795a5a5efcde22376",
    ),
    (
        "023_academic_question_bank.sql",
        "e43b63421992435c8418c23afbd4730b8cf6f82ca5731bf28bf40fd87bf9e87d",
    ),
    (
        "024_question_bank_subject_contract_and_search.sql",
        "faf78cd83ee37e2e92948d5e9c9487d491bda714abe5b02018da15d29729df87",
    ),
    (
        "025_question_bank_rich_document.sql",
        "3c4759a46ab640a73d708e38cad004787ddac2b7530633b9ea9a3918887d0e43",
    ),
    (
        "026_calendar_event_tags.sql",
        "1e98ce4a80dda01c3a83450f40dd219679f9bc766ce037580e8d60fb5b5293e0",
    ),
    (
        "027_role_organization_system_flags.sql",
        "ee5c9b097c31dd96185bd73575d043dcef8916cf3d1a14642a1fb3de77a40983",
    ),
    (
        "028_remove_auto_scheduler.sql",
        "1f1f2b58c109ea03f4c91286716e00418f92abc6ded9771725ec8601e7df62a4",
    ),
    (
        "029_configurable_menu_workspaces.sql",
        "9c800320fcc3b47a400736b64bff3bed27c8654c2f77b75bcaa6dd784ce58ee5",
    ),
];

fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

fn has_test_database_url() -> bool {
    env::var("TEST_DATABASE_URL")
        .map(|url| !url.trim().is_empty())
        .unwrap_or(false)
}

#[test]
fn pre_file_platform_migrations_are_unchanged() {
    for (filename, expected_sha256) in PRE_FILE_PLATFORM_MIGRATIONS {
        let contents = fs::read(migrations_dir().join(filename))
            .unwrap_or_else(|error| panic!("could not read migration {filename}: {error}"));
        let actual_sha256 = format!("{:x}", Sha256::digest(contents));

        assert_eq!(
            actual_sha256, *expected_sha256,
            "applied migration {filename} must remain immutable"
        );
    }
}

#[test]
fn file_platform_migration_is_present() {
    assert!(
        migrations_dir().join("030_file_platform.sql").is_file(),
        "File Platform requires its forward-only 030 migration"
    );
}

#[test]
fn file_platform_migration_declares_exact_domains_and_relationship_guards() {
    let migration = fs::read_to_string(migrations_dir().join("030_file_platform.sql"))
        .expect("File Platform migration should be readable");

    for required_fragment in [
        "storage_status VARCHAR(32) NOT NULL DEFAULT 'pending'",
        "CONSTRAINT file_versions_storage_status_check",
        "CONSTRAINT file_derivatives_storage_status_check",
        "FOREIGN KEY (current_version_id, id)",
        "FOREIGN KEY (source_version_id, file_id)",
        "FOREIGN KEY (file_version_id, file_id)",
        "FOREIGN KEY (file_derivative_id, file_id)",
        "status = 'leased'",
        "status <> 'leased'",
        "lease_owner ~ '[^[:space:]]'",
        "BEFORE DELETE ON file_versions",
        "BEFORE DELETE ON file_derivatives",
        "provider_code ~ '[^[:space:]]'",
        "object_key ~ '[^[:space:]]'",
        "NEW.id IS DISTINCT FROM OLD.id",
    ] {
        assert!(
            migration.contains(required_fragment),
            "migration 030 must include {required_fragment:?}"
        );
    }
}

#[tokio::test]
async fn file_platform_schema_is_additive_and_constrained() {
    if !has_test_database_url() {
        eprintln!("SKIPPED: TEST_DATABASE_URL is not set; File Platform migration assertions require an isolated test database.");
        return;
    }

    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    assert_columns(
        &pool,
        "files",
        &[
            "purpose_code",
            "visibility",
            "lifecycle_status",
            "current_version_id",
            "retention_class",
            "delete_requested_at",
            "deleted_at",
        ],
    )
    .await;

    for table in ["file_versions", "file_derivatives", "file_operations"] {
        assert!(
            relation_exists(&pool, table).await,
            "expected {table} table after File Platform migration"
        );
    }

    assert_unique_constraint(&pool, "file_versions", &["file_id", "version_number"]).await;
    assert_unique_constraint(
        &pool,
        "file_versions",
        &["provider_code", "storage_class", "object_key"],
    )
    .await;

    assert_check_domain(
        &pool,
        "files",
        "files_lifecycle_status_check",
        &[
            "pending",
            "processing",
            "ready",
            "delete_requested",
            "deleted",
            "failed",
            "quarantined",
        ],
    )
    .await;
    assert_check_domain(
        &pool,
        "file_versions",
        "file_versions_storage_class_check",
        &["public", "private"],
    )
    .await;
    assert_check_domain(
        &pool,
        "file_versions",
        "file_versions_storage_status_check",
        &[
            "pending",
            "stored",
            "delete_requested",
            "deleted",
            "missing",
            "failed",
        ],
    )
    .await;
    assert_check_domain(
        &pool,
        "file_versions",
        "file_versions_scan_status_check",
        &["pending", "clean", "infected", "failed", "skipped"],
    )
    .await;
    assert_check_domain(
        &pool,
        "file_derivatives",
        "file_derivatives_lifecycle_status_check",
        &["pending", "processing", "ready", "failed", "deleted"],
    )
    .await;
    assert_check_domain(
        &pool,
        "file_derivatives",
        "file_derivatives_storage_class_check",
        &["public", "private"],
    )
    .await;
    assert_check_domain(
        &pool,
        "file_derivatives",
        "file_derivatives_storage_status_check",
        &[
            "pending",
            "stored",
            "delete_requested",
            "deleted",
            "missing",
            "failed",
        ],
    )
    .await;
    assert_check_domain(
        &pool,
        "file_operations",
        "file_operations_operation_type_check",
        &["scan", "generate_derivative", "delete_object", "reconcile"],
    )
    .await;
    assert_check_domain(
        &pool,
        "file_operations",
        "file_operations_status_check",
        &[
            "pending",
            "leased",
            "succeeded",
            "retryable_failure",
            "failed",
            "cancelled",
        ],
    )
    .await;
}

#[tokio::test]
async fn file_platform_schema_rejects_inconsistent_leases_and_cross_file_targets() {
    let Some(pool) = test_pool_or_skip().await else {
        return;
    };

    let file_a = insert_file(&pool).await;
    let file_b = insert_file(&pool).await;
    let version_a = insert_version(&pool, file_a, "stored")
        .await
        .expect("valid version should insert");
    let derivative_a = insert_derivative(&pool, file_a, version_a, "stored")
        .await
        .expect("valid derivative should insert");

    assert_sql_rejected(
        sqlx::query(
            "INSERT INTO file_operations (file_id, operation_type, status)
             VALUES ($1, 'reconcile', 'leased')",
        )
        .bind(file_a)
        .execute(&pool)
        .await,
        "leased work without a lease",
    );
    assert_sql_rejected(
        sqlx::query(
            "INSERT INTO file_operations (
                file_id, operation_type, status, lease_owner, leased_at, lease_expires_at
             ) VALUES ($1, 'reconcile', 'pending', 'worker', now(), now() + interval '1 minute')",
        )
        .bind(file_a)
        .execute(&pool)
        .await,
        "non-leased work with active lease fields",
    );
    sqlx::query(
        "INSERT INTO file_operations (
            file_id, operation_type, status, lease_owner, leased_at, lease_expires_at
         ) VALUES ($1, 'reconcile', 'leased', 'worker', now() - interval '2 minutes', now() - interval '1 minute')",
    )
    .bind(file_a)
    .execute(&pool)
    .await
    .expect("expired leased work should remain reclaimable");

    assert_sql_rejected(
        sqlx::query("UPDATE files SET current_version_id = $1 WHERE id = $2")
            .bind(version_a)
            .bind(file_b)
            .execute(&pool)
            .await,
        "a current version owned by another file",
    );
    assert_sql_rejected(
        insert_derivative(&pool, file_b, version_a, "stored").await,
        "a derivative whose source version belongs to another file",
    );
    assert_sql_rejected(
        sqlx::query(
            "INSERT INTO file_operations (file_id, file_version_id, operation_type)
             VALUES ($1, $2, 'scan')",
        )
        .bind(file_b)
        .bind(version_a)
        .execute(&pool)
        .await,
        "an operation version target owned by another file",
    );
    assert_sql_rejected(
        sqlx::query(
            "INSERT INTO file_operations (file_id, file_derivative_id, operation_type)
             VALUES ($1, $2, 'delete_object')",
        )
        .bind(file_b)
        .bind(derivative_a)
        .execute(&pool)
        .await,
        "an operation derivative target owned by another file",
    );
}

#[tokio::test]
async fn file_platform_schema_exercises_every_status_domain() {
    let Some(pool) = test_pool_or_skip().await else {
        return;
    };

    let file = insert_file(&pool).await;
    for status in [
        "pending",
        "processing",
        "ready",
        "delete_requested",
        "deleted",
        "failed",
        "quarantined",
    ] {
        sqlx::query("UPDATE files SET lifecycle_status = $1 WHERE id = $2")
            .bind(status)
            .bind(file)
            .execute(&pool)
            .await
            .expect("allowed file lifecycle status should update");
    }
    assert_sql_rejected(
        sqlx::query("UPDATE files SET lifecycle_status = 'invalid' WHERE id = $1")
            .bind(file)
            .execute(&pool)
            .await,
        "an invalid file lifecycle status",
    );

    let version = insert_version(&pool, file, "pending")
        .await
        .expect("valid version should insert");
    for status in ["pending", "clean", "infected", "failed", "skipped"] {
        sqlx::query("UPDATE file_versions SET scan_status = $1 WHERE id = $2")
            .bind(status)
            .bind(version)
            .execute(&pool)
            .await
            .expect("allowed scan status should update");
    }
    assert_sql_rejected(
        sqlx::query("UPDATE file_versions SET scan_status = 'invalid' WHERE id = $1")
            .bind(version)
            .execute(&pool)
            .await,
        "an invalid scan status",
    );
    for status in [
        "pending",
        "stored",
        "delete_requested",
        "deleted",
        "missing",
        "failed",
    ] {
        sqlx::query(
            "UPDATE file_versions
             SET storage_status = $1,
                 deleted_at = CASE WHEN $1 = 'deleted' THEN now() ELSE NULL END
             WHERE id = $2",
        )
        .bind(status)
        .bind(version)
        .execute(&pool)
        .await
        .expect("allowed version storage status should update");
    }
    assert_sql_rejected(
        sqlx::query("UPDATE file_versions SET storage_status = 'invalid' WHERE id = $1")
            .bind(version)
            .execute(&pool)
            .await,
        "an invalid version storage status",
    );

    let derivative = insert_derivative(&pool, file, version, "pending")
        .await
        .expect("valid derivative should insert");
    for status in ["pending", "processing", "ready", "failed", "deleted"] {
        sqlx::query("UPDATE file_derivatives SET lifecycle_status = $1 WHERE id = $2")
            .bind(status)
            .bind(derivative)
            .execute(&pool)
            .await
            .expect("allowed derivative lifecycle status should update");
    }
    assert_sql_rejected(
        sqlx::query("UPDATE file_derivatives SET lifecycle_status = 'invalid' WHERE id = $1")
            .bind(derivative)
            .execute(&pool)
            .await,
        "an invalid derivative lifecycle status",
    );
    for status in [
        "pending",
        "stored",
        "delete_requested",
        "deleted",
        "missing",
        "failed",
    ] {
        sqlx::query(
            "UPDATE file_derivatives
             SET storage_status = $1,
                 deleted_at = CASE WHEN $1 = 'deleted' THEN now() ELSE NULL END
             WHERE id = $2",
        )
        .bind(status)
        .bind(derivative)
        .execute(&pool)
        .await
        .expect("allowed derivative storage status should update");
    }
    assert_sql_rejected(
        sqlx::query("UPDATE file_derivatives SET storage_status = 'invalid' WHERE id = $1")
            .bind(derivative)
            .execute(&pool)
            .await,
        "an invalid derivative storage status",
    );

    for operation_type in ["scan", "generate_derivative", "delete_object", "reconcile"] {
        sqlx::query("INSERT INTO file_operations (file_id, operation_type) VALUES ($1, $2)")
            .bind(file)
            .bind(operation_type)
            .execute(&pool)
            .await
            .expect("allowed operation type should insert");
    }
    assert_sql_rejected(
        sqlx::query("INSERT INTO file_operations (file_id, operation_type) VALUES ($1, 'invalid')")
            .bind(file)
            .execute(&pool)
            .await,
        "an invalid operation type",
    );
    for status in [
        "pending",
        "succeeded",
        "retryable_failure",
        "failed",
        "cancelled",
    ] {
        sqlx::query(
            "INSERT INTO file_operations (file_id, operation_type, status)
             VALUES ($1, 'reconcile', $2)",
        )
        .bind(file)
        .bind(status)
        .execute(&pool)
        .await
        .expect("allowed non-leased operation status should insert");
    }
    sqlx::query(
        "INSERT INTO file_operations (
            file_id, operation_type, status, lease_owner, leased_at, lease_expires_at
         ) VALUES ($1, 'reconcile', 'leased', 'worker', now(), now() + interval '1 minute')",
    )
    .bind(file)
    .execute(&pool)
    .await
    .expect("allowed leased operation status should insert");
    assert_sql_rejected(
        sqlx::query(
            "INSERT INTO file_operations (file_id, operation_type, status)
             VALUES ($1, 'reconcile', 'invalid')",
        )
        .bind(file)
        .execute(&pool)
        .await,
        "an invalid operation status",
    );
}

#[tokio::test]
async fn file_platform_schema_rejects_invalid_locator_values_and_physical_identity_deletion() {
    let Some(pool) = test_pool_or_skip().await else {
        return;
    };

    let standalone_file = insert_file(&pool).await;
    let standalone_version = insert_version(&pool, standalone_file, "stored")
        .await
        .expect("valid version should insert");
    let derivative_file = insert_file(&pool).await;
    let derivative_version = insert_version(&pool, derivative_file, "stored")
        .await
        .expect("valid version should insert");
    let derivative = insert_derivative(&pool, derivative_file, derivative_version, "stored")
        .await
        .expect("valid derivative should insert");

    assert_sql_rejected(
        sqlx::query("UPDATE file_versions SET id = $1 WHERE id = $2")
            .bind(Uuid::new_v4())
            .bind(standalone_version)
            .execute(&pool)
            .await,
        "a version UUID update",
    );
    assert_sql_rejected(
        sqlx::query("DELETE FROM file_versions WHERE id = $1")
            .bind(standalone_version)
            .execute(&pool)
            .await,
        "physical deletion of a version without derivatives",
    );
    assert_sql_rejected(
        insert_version_with_id(&pool, standalone_file, standalone_version).await,
        "reuse of a protected version UUID",
    );
    assert_sql_rejected(
        sqlx::query("UPDATE file_derivatives SET id = $1 WHERE id = $2")
            .bind(Uuid::new_v4())
            .bind(derivative)
            .execute(&pool)
            .await,
        "a derivative UUID update",
    );
    assert_sql_rejected(
        sqlx::query("DELETE FROM file_derivatives WHERE id = $1")
            .bind(derivative)
            .execute(&pool)
            .await,
        "physical derivative deletion",
    );
    assert_sql_rejected(
        insert_derivative_with_id(&pool, derivative_file, derivative_version, derivative).await,
        "reuse of a protected derivative UUID",
    );

    for (provider_code, object_key, scenario) in [
        ("\t", "another-object", "a tab-only version provider code"),
        ("test", "\n", "a newline-only version object key"),
    ] {
        assert_sql_rejected(
            insert_version_with_locator(
                &pool,
                insert_file(&pool).await,
                provider_code,
                object_key,
                "stored",
            )
            .await,
            scenario,
        );
    }
    for (provider_code, object_key, scenario) in [
        (
            "\t",
            "another-object",
            "a tab-only derivative provider code",
        ),
        ("test", "\n", "a newline-only derivative object key"),
    ] {
        let file = insert_file(&pool).await;
        let version = insert_version(&pool, file, "stored")
            .await
            .expect("valid source version should insert");
        assert_sql_rejected(
            insert_derivative_with_locator(
                &pool,
                file,
                version,
                provider_code,
                object_key,
                "stored",
            )
            .await,
            scenario,
        );
    }
    assert_sql_rejected(
        sqlx::query(
            "INSERT INTO file_operations (
                file_id, operation_type, status, lease_owner, leased_at, lease_expires_at
             ) VALUES ($1, 'reconcile', 'leased', E'\\t\\n', now(), now() + interval '1 minute')",
        )
        .bind(standalone_file)
        .execute(&pool)
        .await,
        "a tab/newline-only lease owner",
    );
}

async fn relation_exists(pool: &PgPool, relation: &str) -> bool {
    sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(relation)
        .fetch_one(pool)
        .await
        .expect("relation existence query should execute")
}

async fn assert_columns(pool: &PgPool, table: &str, expected: &[&str]) {
    let actual = sqlx::query_scalar::<_, String>(
        "SELECT column_name
         FROM information_schema.columns
         WHERE table_schema = current_schema() AND table_name = $1
         ORDER BY column_name",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .expect("column metadata query should execute");

    for column in expected {
        assert!(
            actual.iter().any(|actual_column| actual_column == column),
            "expected {table}.{column} after File Platform migration; found {actual:?}"
        );
    }
}

async fn assert_unique_constraint(pool: &PgPool, table: &str, expected_columns: &[&str]) {
    let constraints = sqlx::query_scalar::<_, String>(
        "SELECT pg_get_constraintdef(c.oid)
         FROM pg_constraint AS c
         WHERE c.conrelid = $1::regclass
           AND c.contype = 'u'",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .expect("unique constraint metadata query should execute");
    let expected_definition = format!("UNIQUE ({})", expected_columns.join(", "));

    assert!(
        constraints
            .iter()
            .any(|definition| definition.contains(&expected_definition)),
        "expected {table} unique constraint {expected_definition}; found {constraints:?}"
    );
}

async fn assert_check_domain(
    pool: &PgPool,
    table_name: &str,
    constraint_name: &str,
    expected: &[&str],
) {
    let definition = sqlx::query_scalar::<_, String>(
        "SELECT pg_get_constraintdef(c.oid)
         FROM pg_constraint AS c
         JOIN pg_class AS rel ON rel.oid = c.conrelid
         JOIN pg_namespace AS nsp ON nsp.oid = rel.relnamespace
         WHERE c.conname = $1
           AND c.contype = 'c'
           AND rel.relname = $2
           AND nsp.nspname = current_schema()",
    )
    .bind(constraint_name)
    .bind(table_name)
    .fetch_optional(pool)
    .await
    .expect("check constraint metadata query should execute")
    .unwrap_or_else(|| panic!("missing CHECK constraint {constraint_name}"));
    let literal = regex::Regex::new(r"'([^']*)'").expect("literal pattern should compile");
    let actual = literal
        .captures_iter(&definition)
        .map(|capture| capture[1].to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        expected.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "unexpected domain for {constraint_name}: {definition}"
    );
}

async fn test_pool_or_skip() -> Option<PgPool> {
    if !has_test_database_url() {
        eprintln!("SKIPPED: TEST_DATABASE_URL is not set; File Platform migration assertions require an isolated test database.");
        return None;
    }

    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    Some(pool)
}

async fn insert_file(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO files (
            filename, original_filename, file_size, mime_type, storage_path, file_type
         ) VALUES ('platform-test.bin', 'platform-test.bin', 1, 'application/octet-stream', $1, 'other')
         RETURNING id",
    )
    .bind(format!("platform-test/{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .expect("test file should insert")
}

async fn insert_version(
    pool: &PgPool,
    file_id: Uuid,
    storage_status: &str,
) -> Result<Uuid, sqlx::Error> {
    insert_version_with_locator(
        pool,
        file_id,
        "test",
        &format!("platform-test/{}", Uuid::new_v4()),
        storage_status,
    )
    .await
}

async fn insert_version_with_locator(
    pool: &PgPool,
    file_id: Uuid,
    provider_code: &str,
    object_key: &str,
    storage_status: &str,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO file_versions (
            file_id, version_number, provider_code, storage_class, storage_status,
            object_key, detected_mime_type, canonical_extension, byte_size, checksum
         ) VALUES ($1, 1, $2, 'private', $3, $4, 'application/octet-stream', 'bin', 1, repeat('a', 64))
         RETURNING id",
    )
    .bind(file_id)
    .bind(provider_code)
    .bind(storage_status)
    .bind(object_key)
    .fetch_one(pool)
    .await
}

async fn insert_version_with_id(
    pool: &PgPool,
    file_id: Uuid,
    version_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO file_versions (
            id, file_id, version_number, provider_code, storage_class, storage_status,
            object_key, detected_mime_type, canonical_extension, byte_size, checksum
         ) VALUES ($1, $2, 1, 'test', 'private', 'stored', $3,
                   'application/octet-stream', 'bin', 1, repeat('a', 64))
         RETURNING id",
    )
    .bind(version_id)
    .bind(file_id)
    .bind(format!("platform-test/{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
}

async fn insert_derivative(
    pool: &PgPool,
    file_id: Uuid,
    source_version_id: Uuid,
    storage_status: &str,
) -> Result<Uuid, sqlx::Error> {
    insert_derivative_with_locator(
        pool,
        file_id,
        source_version_id,
        "test",
        &format!("platform-test/{}", Uuid::new_v4()),
        storage_status,
    )
    .await
}

async fn insert_derivative_with_locator(
    pool: &PgPool,
    file_id: Uuid,
    source_version_id: Uuid,
    provider_code: &str,
    object_key: &str,
    storage_status: &str,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO file_derivatives (
            file_id, source_version_id, derivative_kind, provider_code, storage_class,
            storage_status, object_key, detected_mime_type, canonical_extension, byte_size, checksum
         ) VALUES ($1, $2, 'thumbnail-256', $3, 'private', $4, $5,
                   'image/webp', 'webp', 1, repeat('b', 64))
         RETURNING id",
    )
    .bind(file_id)
    .bind(source_version_id)
    .bind(provider_code)
    .bind(storage_status)
    .bind(object_key)
    .fetch_one(pool)
    .await
}

async fn insert_derivative_with_id(
    pool: &PgPool,
    file_id: Uuid,
    source_version_id: Uuid,
    derivative_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO file_derivatives (
            id, file_id, source_version_id, derivative_kind, provider_code, storage_class,
            storage_status, object_key, detected_mime_type, canonical_extension, byte_size, checksum
         ) VALUES ($1, $2, $3, 'thumbnail-256', 'test', 'private', 'stored', $4,
                   'image/webp', 'webp', 1, repeat('b', 64))
         RETURNING id",
    )
    .bind(derivative_id)
    .bind(file_id)
    .bind(source_version_id)
    .bind(format!("platform-test/{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
}

fn assert_sql_rejected<T>(result: Result<T, sqlx::Error>, scenario: &str) {
    assert!(result.is_err(), "schema should reject {scenario}");
}
