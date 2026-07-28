use crate::test_helpers::{create_test_pool, run_test_migrations};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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

    for (table, column) in [
        ("files", "lifecycle_status"),
        ("file_versions", "storage_class"),
        ("file_versions", "scan_status"),
        ("file_derivatives", "lifecycle_status"),
        ("file_derivatives", "storage_class"),
        ("file_operations", "operation_type"),
        ("file_operations", "status"),
    ] {
        assert_check_constraint(&pool, table, column).await;
    }
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
        "SELECT pg_get_constraintdef(constraint.oid)
         FROM pg_constraint AS constraint
         WHERE constraint.conrelid = $1::regclass
           AND constraint.contype = 'u'",
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

async fn assert_check_constraint(pool: &PgPool, table: &str, column: &str) {
    let constraints = sqlx::query_scalar::<_, String>(
        "SELECT pg_get_constraintdef(constraint.oid)
         FROM pg_constraint AS constraint
         WHERE constraint.conrelid = $1::regclass
           AND constraint.contype = 'c'",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .expect("check constraint metadata query should execute");

    assert!(
        constraints
            .iter()
            .any(|definition| definition.contains(column)),
        "expected a CHECK constraint for {table}.{column}; found {constraints:?}"
    );
}
