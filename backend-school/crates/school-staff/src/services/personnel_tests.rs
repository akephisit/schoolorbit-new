use super::personnel_cutover_service::{read_personnel_cutover_audit, read_personnel_preflight};
use school_test_db::create_named_test_pool;
use sqlx::{migrate::Migrator, PgPool};
use std::borrow::Cow;
use uuid::Uuid;

#[tokio::test]
async fn reference_catalog_is_bounded_searchable_and_unique() {
    use super::reference_service::*;
    use crate::personnel::*;
    let pool = legacy_pool("reference_catalog").await;
    migrate_through(&pool, 81).await.unwrap();
    let created = create_reference_item(
        &pool,
        CreateReferenceRequest {
            kind: StaffReferenceKind::Major,
            name: "  วิทยาศาสตร์   Science  ".into(),
            display_order: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(created.name, "วิทยาศาสตร์ Science");
    assert!(matches!(
        create_reference_item(
            &pool,
            CreateReferenceRequest {
                kind: StaffReferenceKind::Major,
                name: "วิทยาศาสตร์ science".into(),
                display_order: None
            }
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    for name in [" ".to_string(), "ก".repeat(201), "Bad\nName".into()] {
        assert!(matches!(
            create_reference_item(
                &pool,
                CreateReferenceRequest {
                    kind: StaffReferenceKind::Major,
                    name,
                    display_order: None
                }
            )
            .await,
            Err(school_errors::AppError::BadRequest(_))
        ));
    }
    let query = ReferenceListQuery {
        kind: StaffReferenceKind::Major,
        search: Some("Science".into()),
        status: None,
        page: None,
        page_size: Some(500),
    };
    let page = list_reference_items(&pool, query).await.unwrap();
    assert_eq!(page.page_size, 50);
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, created.id);
    update_reference_item(
        &pool,
        created.id,
        UpdateReferenceRequest {
            name: None,
            is_active: Some(false),
            display_order: None,
        },
    )
    .await
    .unwrap();
    let active = list_reference_items(
        &pool,
        ReferenceListQuery {
            kind: StaffReferenceKind::Major,
            search: None,
            status: None,
            page: None,
            page_size: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(active.total, 0);
    let inactive = list_reference_items(
        &pool,
        ReferenceListQuery {
            kind: StaffReferenceKind::Major,
            search: None,
            status: Some(ReferenceStatusFilter::Inactive),
            page: None,
            page_size: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(inactive.items[0].id, created.id);
    assert_eq!(inactive.items[0].code, created.code);
}

#[tokio::test]
async fn reference_deactivation_race_rejects_new_assignment() {
    use super::staff_info_service::patch_staff_info;
    use crate::personnel::*;
    let pool = legacy_pool("reference_deactivation_race").await;
    migrate_through(&pool, 81).await.unwrap();
    let user = canonical_person(&pool).await;
    let position: Uuid =
        sqlx::query_scalar("SELECT id FROM staff_reference_items WHERE code='teacher'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut deactivation = pool.begin().await.unwrap();
    sqlx::query("UPDATE staff_reference_items SET is_active=false WHERE id=$1")
        .bind(position)
        .execute(&mut *deactivation)
        .await
        .unwrap();
    let worker_pool = pool.clone();
    let assignment = tokio::spawn(async move {
        let mut tx = worker_pool.begin().await.unwrap();
        patch_staff_info(
            &mut tx,
            user,
            &UpdateStaffInfoRequest {
                job_position_id: Some(Some(position)),
                ..Default::default()
            },
        )
        .await
    });
    tokio::task::yield_now().await;
    deactivation.commit().await.unwrap();
    assert!(matches!(
        assignment.await.unwrap(),
        Err(school_errors::AppError::BadRequest(_))
    ));
}

pub(super) async fn migrate_through(
    pool: &PgPool,
    version: i64,
) -> Result<(), sqlx::migrate::MigrateError> {
    let source = sqlx::migrate!("../../migrations");
    let migrator = Migrator {
        migrations: Cow::Owned(
            source
                .iter()
                .filter(|m| m.version <= version)
                .cloned()
                .collect(),
        ),
        ignore_missing: false,
        locking: false,
        no_tx: source.no_tx,
        table_name: source.table_name,
        create_schemas: source.create_schemas,
    };
    migrator.run(pool).await
}

async fn legacy_pool(name: &str) -> PgPool {
    let pool = create_named_test_pool(name).await;
    migrate_through(&pool, 80)
        .await
        .expect("predecessor migrations pass");
    pool
}

async fn insert_legacy_info(pool: &PgPool, degree: &str, major: &str, university: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, username, password_hash, first_name, last_name, user_type, status) VALUES ($1, $2, 'synthetic-hash', 'Fixture', 'Person', 'staff', 'active')")
        .bind(user_id).bind(format!("personnel-{}", user_id.simple())).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO staff_info (user_id, employment_type, education_level, major, university, teaching_license_number, teaching_license_expiry, metadata) VALUES ($1, 'permanent', $2, $3, $4, 'synthetic-license', '2030-01-01', '{\"fixture\":true}')")
        .bind(user_id).bind(degree).bind(major).bind(university).execute(pool).await.unwrap();
    user_id
}

async fn preservation_snapshot(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT jsonb_build_array(id, user_id, employment_type, teaching_license_number, teaching_license_expiry, metadata, created_at)::text FROM staff_info ORDER BY user_id")
        .fetch_all(pool).await.unwrap()
}

fn require_personnel_migration() {
    assert!(
        sqlx::migrate!("../../migrations").version_exists(81),
        "canonical personnel migration 081 must exist"
    );
}

#[tokio::test]
async fn migration_081_preserves_and_normalizes_personnel() {
    require_personnel_migration();
    let pool = legacy_pool("personnel_081_preserve").await;
    let first = insert_legacy_info(&pool, "ปริญญาตรี", "  Mathematics  ", "Fixture University").await;
    let second = insert_legacy_info(&pool, "ป.ตรี", "mathematics", " Fixture   University ").await;
    let third = insert_legacy_info(&pool, "ปริญญาโท", "Science", "Second University").await;
    let fourth = insert_legacy_info(&pool, "  ", " ", " ").await;
    let before = preservation_snapshot(&pool).await;
    migrate_through(&pool, 81).await.unwrap();
    assert_eq!(before, preservation_snapshot(&pool).await);
    let degrees: Vec<Option<String>> = sqlx::query_scalar("SELECT education_level FROM staff_info WHERE user_id = ANY($1) ORDER BY array_position($1::uuid[], user_id)")
        .bind(vec![first, second, third, fourth]).fetch_all(&pool).await.unwrap();
    assert_eq!(
        degrees,
        vec![
            Some("bachelor".into()),
            Some("bachelor".into()),
            Some("master".into()),
            None
        ]
    );
    let refs: Vec<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as("SELECT major_id, university_id FROM staff_info WHERE user_id = ANY($1) ORDER BY array_position($1::uuid[], user_id)")
        .bind(vec![first, second]).fetch_all(&pool).await.unwrap();
    assert!(refs[0].0.is_some());
    assert_eq!(refs[0], refs[1]);
    let positions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_reference_items WHERE kind = 'job_position'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(positions, 7);
    let old_columns: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'staff_info' AND column_name IN ('major', 'university')").fetch_one(&pool).await.unwrap();
    assert_eq!(old_columns, 0);
    let audit: serde_json::Value = sqlx::query_scalar(
        "SELECT checks FROM staff_personnel_cutover_audit WHERE migration_version = 81",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!audit.as_array().unwrap().is_empty());
    assert!(audit
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["passed"] == true));
}

async fn assert_failed_mapping(name: &str, degree: &str) {
    require_personnel_migration();
    let pool = legacy_pool(name).await;
    insert_legacy_info(&pool, degree, "Preserved Major", "Preserved University").await;
    let before = preservation_snapshot(&pool).await;
    let error = migrate_through(&pool, 81).await.unwrap_err();
    assert!(error.to_string().contains("PERSONNEL_EDUCATION_UNMAPPED"));
    assert_eq!(before, preservation_snapshot(&pool).await);
    let value: String = sqlx::query_scalar("SELECT education_level FROM staff_info")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(value, degree);
    let version: i64 =
        sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version, 80);
    let old_columns: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'staff_info' AND column_name IN ('major', 'university')").fetch_one(&pool).await.unwrap();
    assert_eq!(old_columns, 2);
    let partial: Option<String> = sqlx::query_scalar(
        "SELECT to_regclass(current_schema() || '.staff_personnel_cutover_audit')::text",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(partial.is_none());
}

#[tokio::test]
async fn migration_081_rejects_unknown_without_cleanup() {
    assert_failed_mapping("personnel_081_unknown", "Unverified level").await;
}

#[tokio::test]
async fn migration_081_rejects_ambiguous_degree() {
    assert_failed_mapping("personnel_081_ambiguous", "ปริญญาตรี / ปริญญาโท").await;
}

#[tokio::test]
async fn migration_081_failed_transaction_is_retryable() {
    require_personnel_migration();
    let pool = legacy_pool("personnel_081_retry").await;
    insert_legacy_info(&pool, "Unverified", "Science", "Fixture University").await;
    assert!(migrate_through(&pool, 81).await.is_err());
    sqlx::query("UPDATE staff_info SET education_level = 'ปริญญาตรี'")
        .execute(&pool)
        .await
        .unwrap();
    migrate_through(&pool, 81).await.unwrap();
    let refs_before: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM staff_reference_items ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    migrate_through(&pool, 81).await.unwrap();
    let refs_after: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM staff_reference_items ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(refs_before, refs_after);
}

#[tokio::test]
async fn migration_081_preserves_empty_state() {
    require_personnel_migration();
    let pool = legacy_pool("personnel_081_empty").await;
    migrate_through(&pool, 81).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM staff_info")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let checked: bool = sqlx::query_scalar(
        "SELECT passed FROM staff_personnel_cutover_audit WHERE migration_version = 81",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(checked);
}

#[tokio::test]
async fn migration_081_enforces_reference_kind() {
    require_personnel_migration();
    let pool = legacy_pool("personnel_081_kind").await;
    let user = insert_legacy_info(&pool, "ปริญญาตรี", "Science", "Fixture University").await;
    migrate_through(&pool, 81).await.unwrap();
    let major: Uuid = sqlx::query_scalar("SELECT major_id FROM staff_info WHERE user_id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    let result = sqlx::query("UPDATE staff_info SET job_position_id = $1 WHERE user_id = $2")
        .bind(major)
        .bind(user)
        .execute(&pool)
        .await;
    assert_eq!(
        result
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23503")
    );
}

#[tokio::test]
async fn migration_081_rechecks_drift() {
    let pool = legacy_pool("personnel_081_drift").await;
    insert_legacy_info(&pool, "ปริญญาตรี", "Science", "Fixture University").await;
    let report = read_personnel_preflight(&pool).await.unwrap();
    assert!(report.passed);
    assert_eq!(report.migration_version, 80);
    let version: i64 =
        sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version, 80, "preflight must not migrate");
    sqlx::query("UPDATE staff_info SET education_level = 'Unverified after preflight'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!read_personnel_preflight(&pool).await.unwrap().passed);
    assert!(migrate_through(&pool, 81).await.is_err());
}

#[tokio::test]
async fn migration_081_cutover_audit_is_required() {
    let pool = legacy_pool("personnel_081_audit").await;
    migrate_through(&pool, 81).await.unwrap();
    assert!(read_personnel_cutover_audit(&pool).await.unwrap().passed);
    sqlx::query("DELETE FROM staff_personnel_cutover_audit")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!read_personnel_cutover_audit(&pool).await.unwrap().passed);
    assert!(!read_personnel_preflight(&pool).await.unwrap().passed);
}

#[tokio::test]
async fn migration_081_aliases_match_preflight() {
    let pool = legacy_pool("personnel_081_aliases").await;
    let examples = [
        (" primary ", "primary"),
        ("ประถมศึกษา", "primary"),
        ("ม.3", "lower_secondary"),
        ("มัธยมศึกษาตอนต้น", "lower_secondary"),
        ("ม.6", "upper_secondary"),
        ("มัธยมศึกษาตอนปลาย", "upper_secondary"),
        ("ปวช.", "vocational_certificate"),
        ("ประกาศนียบัตรวิชาชีพ", "vocational_certificate"),
        ("ปวส.", "higher_vocational"),
        ("ประกาศนียบัตรวิชาชีพชั้นสูง", "higher_vocational"),
        ("อนุปริญญา", "diploma"),
        ("ปริญญาตรี", "bachelor"),
        ("ป.ตรี", "bachelor"),
        ("Bachelor's   Degree", "bachelor"),
        ("Bachelor Degree", "bachelor"),
        ("ปริญญาโท", "master"),
        ("ป.โท", "master"),
        ("Master's Degree", "master"),
        ("Master Degree", "master"),
        ("ปริญญาเอก", "doctorate"),
        ("ป.เอก", "doctorate"),
        ("Doctoral Degree", "doctorate"),
        ("PHD", "doctorate"),
        ("PH.D.", "doctorate"),
        ("อื่น ๆ", "other"),
        ("อื่นๆ", "other"),
        ("lower_secondary", "lower_secondary"),
        ("upper_secondary", "upper_secondary"),
        ("vocational_certificate", "vocational_certificate"),
        ("higher_vocational", "higher_vocational"),
        ("diploma", "diploma"),
        ("bachelor", "bachelor"),
        ("master", "master"),
        ("doctorate", "doctorate"),
        ("other", "other"),
    ];
    let mut ids = Vec::new();
    for (raw, _) in examples {
        ids.push(insert_legacy_info(&pool, raw, "Science", "Fixture University").await);
    }
    assert!(read_personnel_preflight(&pool).await.unwrap().passed);
    migrate_through(&pool, 81).await.unwrap();
    let actual: Vec<String> = sqlx::query_scalar("SELECT education_level FROM staff_info WHERE user_id = ANY($1) ORDER BY array_position($1::uuid[], user_id)")
        .bind(&ids).fetch_all(&pool).await.unwrap();
    let expected: Vec<String> = examples
        .iter()
        .map(|(_, code)| (*code).to_string())
        .collect();
    assert_eq!(actual, expected);
    assert!(read_personnel_preflight(&pool).await.unwrap().passed);
}

async fn canonical_person(pool: &PgPool) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, username, password_hash, first_name, last_name, user_type, status) VALUES ($1, $2, 'synthetic-hash', 'Fixture', 'Person', 'staff', 'active')")
        .bind(user_id).bind(format!("personnel-{}", user_id.simple())).execute(pool).await.unwrap();
    user_id
}

#[tokio::test]
async fn personnel_patch_distinguishes_missing_null_value() {
    let pool = legacy_pool("personnel_patch_states").await;
    let user = insert_legacy_info(&pool, "ปริญญาตรี", "Science", "Fixture University").await;
    migrate_through(&pool, 81).await.unwrap();
    let no_change = serde_json::from_value(serde_json::json!({"staff_info":{}})).unwrap();
    super::staff_service::update_staff(&pool, user, no_change)
        .await
        .unwrap();
    let degree: Option<String> =
        sqlx::query_scalar("SELECT education_level FROM staff_info WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(degree.as_deref(), Some("bachelor"));
    let clear = serde_json::from_value(
        serde_json::json!({"staff_info":{"education_level":null,"major_id":null}}),
    )
    .unwrap();
    super::staff_service::update_staff(&pool, user, clear)
        .await
        .unwrap();
    let cleared: (Option<String>, Option<Uuid>) =
        sqlx::query_as("SELECT education_level, major_id FROM staff_info WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(cleared, (None, None));
    let explicit = serde_json::from_value(
        serde_json::json!({"staff_info":{"academic_rank":"none","education_level":"master"}}),
    )
    .unwrap();
    super::staff_service::update_staff(&pool, user, explicit)
        .await
        .unwrap();
    let values: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT academic_rank, education_level FROM staff_info WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(values, (Some("none".into()), Some("master".into())));
}

#[test]
fn personnel_rejects_unknown_enum() {
    for invalid in [
        serde_json::json!({"academic_rank":"unverified"}),
        serde_json::json!({"education_level":"free text"}),
        serde_json::json!({"major":"free text"}),
    ] {
        let result = serde_json::from_value::<crate::models::UpdateStaffRequest>(
            serde_json::json!({"staff_info":invalid}),
        );
        assert!(
            result.is_err(),
            "noncanonical personnel data must be rejected"
        );
    }
}

#[tokio::test]
async fn personnel_patch_preserves_license_and_employment() {
    let pool = legacy_pool("personnel_patch_preserve").await;
    let user = insert_legacy_info(&pool, "ปริญญาตรี", "Science", "Fixture University").await;
    migrate_through(&pool, 81).await.unwrap();
    let before = preservation_snapshot(&pool).await;
    let teacher: Uuid =
        sqlx::query_scalar("SELECT id FROM staff_reference_items WHERE code='teacher'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let request = serde_json::from_value(serde_json::json!({"staff_info":{"job_position_id":teacher,"academic_rank":"proficient","education_level":"doctorate"}})).unwrap();
    super::staff_service::update_staff(&pool, user, request)
        .await
        .unwrap();
    assert_eq!(before, preservation_snapshot(&pool).await);
    let profile = super::staff_service::get_staff_profile(&pool, user, false)
        .await
        .unwrap();
    let info = serde_json::to_value(profile.staff_info.unwrap()).unwrap();
    assert_eq!(info["job_position"]["id"], teacher.to_string());
    assert_eq!(info["job_position"]["name"], "ครู");
    assert_eq!(info["academic_rank"], "proficient");
    assert_eq!(info["education_level"], "doctorate");
}

#[tokio::test]
async fn personnel_patch_creates_missing_info_row() {
    let pool = legacy_pool("personnel_patch_missing").await;
    migrate_through(&pool, 81).await.unwrap();
    let user = canonical_person(&pool).await;
    let request = serde_json::from_value(serde_json::json!({"staff_info":{"academic_rank":"not_applicable","education_level":"bachelor"}})).unwrap();
    super::staff_service::update_staff(&pool, user, request)
        .await
        .unwrap();
    let values: (Uuid, String, String) = sqlx::query_as(
        "SELECT user_id, academic_rank, education_level FROM staff_info WHERE user_id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(values, (user, "not_applicable".into(), "bachelor".into()));
}

#[tokio::test]
async fn personnel_patch_rejects_wrong_kind_and_missing_reference() {
    let pool = legacy_pool("personnel_patch_refs").await;
    let user = insert_legacy_info(&pool, "ปริญญาตรี", "Science", "Fixture University").await;
    migrate_through(&pool, 81).await.unwrap();
    let major: Uuid = sqlx::query_scalar("SELECT major_id FROM staff_info WHERE user_id=$1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    for reference in [major, Uuid::new_v4()] {
        let request =
            serde_json::from_value(serde_json::json!({"staff_info":{"job_position_id":reference}}))
                .unwrap();
        let outcome = super::staff_service::update_staff(&pool, user, request).await;
        assert!(matches!(
            outcome,
            Err(school_errors::AppError::BadRequest(_))
        ));
    }
}

#[tokio::test]
async fn personnel_patch_keeps_existing_inactive_reference() {
    let pool = legacy_pool("personnel_patch_inactive").await;
    let user = insert_legacy_info(&pool, "ปริญญาตรี", "Science", "Fixture University").await;
    migrate_through(&pool, 81).await.unwrap();
    let major: Uuid = sqlx::query_scalar("SELECT major_id FROM staff_info WHERE user_id=$1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE staff_reference_items SET is_active=false WHERE id=$1")
        .bind(major)
        .execute(&pool)
        .await
        .unwrap();
    let keep =
        serde_json::from_value(serde_json::json!({"staff_info":{"major_id":major}})).unwrap();
    super::staff_service::update_staff(&pool, user, keep)
        .await
        .unwrap();
    let other = canonical_person(&pool).await;
    let new_assignment =
        serde_json::from_value(serde_json::json!({"staff_info":{"major_id":major}})).unwrap();
    assert!(matches!(
        super::staff_service::update_staff(&pool, other, new_assignment).await,
        Err(school_errors::AppError::BadRequest(_))
    ));
}

#[tokio::test]
async fn personnel_create_persists_canonical_fields_and_license() {
    let pool = legacy_pool("personnel_create").await;
    migrate_through(&pool, 81).await.unwrap();
    let teacher: Uuid =
        sqlx::query_scalar("SELECT id FROM staff_reference_items WHERE code='teacher'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let request = serde_json::from_value(serde_json::json!({
        "username":"personnel-create-fixture", "password":"Synthetic-test-123!",
        "first_name":"Fixture", "last_name":"Person", "role_ids":[],
        "staff_info":{"job_position_id":teacher,"academic_rank":"none","education_level":"bachelor", "teaching_license_number":"synthetic-license", "teaching_license_expiry":"2030-01-01"}
    })).unwrap();
    let user = super::staff_service::create_staff(&pool, request)
        .await
        .unwrap();
    let row: (Uuid, String, String, String) = sqlx::query_as("SELECT job_position_id, academic_rank, education_level, teaching_license_number FROM staff_info WHERE user_id=$1").bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!(
        row,
        (
            teacher,
            "none".into(),
            "bachelor".into(),
            "synthetic-license".into()
        )
    );
}

#[tokio::test]
async fn personnel_directory_filters_match_missing_values() {
    use super::staff_service::list_staff;
    use crate::models::*;
    let pool = legacy_pool("personnel_directory_filters").await;
    migrate_through(&pool, 81).await.unwrap();
    let first = canonical_person(&pool).await;
    let second = canonical_person(&pool).await;
    sqlx::query("INSERT INTO staff_info (user_id, job_position_id, academic_rank, education_level) SELECT $1,id,'none','bachelor' FROM staff_reference_items WHERE code='teacher'").bind(first).execute(&pool).await.unwrap();
    for (field, value, expected) in [
        ("job_position_id", "unspecified", second),
        ("academic_rank", "unspecified", second),
        ("education_level", "unspecified", second),
        ("academic_rank", "none", first),
        ("education_level", "bachelor", first),
    ] {
        let filter: StaffListFilter =
            serde_json::from_value(serde_json::json!({field:value})).unwrap();
        let (items, total, _, _) = list_staff(&pool, filter, StaffListAccess::School)
            .await
            .unwrap();
        assert_eq!(total, 1, "{field}={value}");
        assert_eq!(items[0].id, expected);
    }
    for (field, value) in [
        ("job_position_id", "broken"),
        ("academic_rank", "unknown"),
        ("education_level", "ป.ตรี"),
        ("subject_group_id", "broken"),
        ("status", "unknown"),
    ] {
        let filter: StaffListFilter =
            serde_json::from_value(serde_json::json!({field:value})).unwrap();
        assert!(matches!(
            list_staff(&pool, filter, StaffListAccess::School).await,
            Err(school_errors::AppError::BadRequest(_))
        ));
    }
}

async fn personnel_group_unit(pool: &PgPool, group: Uuid, active: bool) -> Uuid {
    if active {
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM organization_units WHERE subject_group_id=$1 AND is_active",
        )
        .bind(group)
        .fetch_optional(pool)
        .await
        .unwrap();
        if let Some(id) = existing {
            return id;
        }
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO organization_units (id,code,name,category,unit_type,subject_group_id,is_active) VALUES ($1,$2,'Fixture unit','academic','subject_group',$3,$4)").bind(id).bind(id.simple().to_string()).bind(group).bind(active).execute(pool).await.unwrap();
    id
}
async fn personnel_membership(
    pool: &PgPool,
    user: Uuid,
    unit: Uuid,
    start: &str,
    end: Option<&str>,
) {
    sqlx::query("INSERT INTO organization_members (user_id,organization_unit_id,position_code,started_at,ended_at) VALUES ($1,$2,'member',$3::text::date,$4::text::date)").bind(user).bind(unit).bind(start).bind(end).execute(pool).await.unwrap();
}
async fn personnel_group(pool: &PgPool, name: &str, active: bool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO subject_groups(id,code,name_th,name_en,is_active) VALUES ($1,$2,$3,$3,$4)",
    )
    .bind(id)
    .bind(id.simple().to_string()[..20].to_string())
    .bind(name)
    .bind(active)
    .execute(pool)
    .await
    .unwrap();
    id
}
#[tokio::test]
async fn personnel_groups_deduplicate_current_membership() {
    use super::staff_service::{get_staff_profile, list_staff};
    use crate::models::*;
    let pool = legacy_pool("personnel_groups_current").await;
    migrate_through(&pool, 81).await.unwrap();
    let person = canonical_person(&pool).await;
    let empty = canonical_person(&pool).await;
    let math = personnel_group(&pool, "Fixture Math", true).await;
    let science = personnel_group(&pool, "Fixture Science", true).await;
    let disabled = personnel_group(&pool, "Fixture Disabled", false).await;
    for (group, active, start, end) in [
        (math, true, "2000-01-01", None),
        (math, true, "2001-01-01", None),
        (science, true, "2100-01-01", None),
        (science, true, "2000-01-01", Some("2001-01-01")),
        (science, false, "2000-01-01", None),
        (disabled, true, "2000-01-01", None),
    ] {
        let unit = personnel_group_unit(&pool, group, active).await;
        personnel_membership(&pool, person, unit, start, end).await;
    }
    let profile = get_staff_profile(&pool, person, false).await.unwrap();
    assert_eq!(profile.subject_groups.len(), 1);
    assert_eq!(profile.subject_groups[0].id, math);
    let (_, total, _, _) = list_staff(
        &pool,
        StaffListFilter {
            subject_group_id: Some(math.to_string()),
            ..Default::default()
        },
        StaffListAccess::School,
    )
    .await
    .unwrap();
    assert_eq!(total, 1);
    let (items, total, _, _) = list_staff(
        &pool,
        StaffListFilter {
            subject_group_id: Some("unassigned".into()),
            ..Default::default()
        },
        StaffListAccess::School,
    )
    .await
    .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].id, empty);
}

#[tokio::test]
async fn personnel_overview_matches_directory_counts() {
    use super::{personnel_overview_service::get_personnel_overview, staff_service::list_staff};
    use crate::models::*;
    let pool = legacy_pool("personnel_overview_counts").await;
    migrate_through(&pool, 82).await.unwrap();
    let a = canonical_person(&pool).await;
    let b = canonical_person(&pool).await;
    let c = canonical_person(&pool).await;
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(c)
        .execute(&pool)
        .await
        .unwrap();
    for (user, position, rank, degree) in [
        (a, "teacher", "proficient", "bachelor"),
        (b, "assistant_teacher", "none", "master"),
    ] {
        sqlx::query("INSERT INTO staff_info(user_id,job_position_id,academic_rank,education_level) SELECT $1,id,$3,$4 FROM staff_reference_items WHERE code=$2").bind(user).bind(position).bind(rank).bind(degree).execute(&pool).await.unwrap();
    }
    let math = personnel_group(&pool, "Fixture Math", true).await;
    let science = personnel_group(&pool, "Fixture Science", true).await;
    for (user, group, start) in [
        (a, math, "2000-01-01"),
        (a, math, "2001-01-01"),
        (a, science, "2000-01-01"),
        (b, math, "2000-01-01"),
        (b, science, "2100-01-01"),
    ] {
        let unit = personnel_group_unit(&pool, group, true).await;
        personnel_membership(&pool, user, unit, start, None).await;
    }
    let root: Uuid=sqlx::query_scalar("INSERT INTO organization_units(code,name) VALUES ('fixture_root','Fixture root') RETURNING id").fetch_one(&pool).await.unwrap();
    let child: Uuid=sqlx::query_scalar("INSERT INTO organization_units(code,name,parent_unit_id) VALUES ('fixture_child','Fixture child',$1) RETURNING id").bind(root).fetch_one(&pool).await.unwrap();
    personnel_membership(&pool, a, root, "2000-01-01", None).await;
    personnel_membership(&pool, c, child, "2000-01-01", None).await;
    for (scope, total) in [
        (StaffListAccess::OrganizationUnit(a), 2),
        (StaffListAccess::OrganizationTree(a), 3),
    ] {
        let overview = get_personnel_overview(
            &pool,
            PersonnelOverviewQuery {
                status: Some(PersonnelStatusFilter::All),
            },
            scope,
        )
        .await
        .unwrap();
        let (_, directory_total, _, _) = list_staff(
            &pool,
            StaffListFilter {
                status: Some("all".into()),
                ..Default::default()
            },
            scope,
        )
        .await
        .unwrap();
        assert_eq!(overview.total, total);
        assert_eq!(overview.total, directory_total);
    }
    for status in [PersonnelStatusFilter::Active, PersonnelStatusFilter::All] {
        let overview = get_personnel_overview(
            &pool,
            PersonnelOverviewQuery {
                status: Some(status),
            },
            StaffListAccess::School,
        )
        .await
        .unwrap();
        assert_eq!(
            (overview.total, overview.active, overview.other_statuses),
            (3, 2, 1)
        );
        let expected = if status == PersonnelStatusFilter::All {
            3
        } else {
            2
        };
        assert_eq!(overview.filtered_total, expected);
        assert_eq!(
            overview
                .subject_groups
                .iter()
                .find(|v| v.key == math.to_string())
                .unwrap()
                .count,
            2
        );
        assert_eq!(
            overview
                .subject_groups
                .iter()
                .find(|v| v.key == science.to_string())
                .unwrap()
                .count,
            1
        );
        for (dimension, buckets) in [
            ("job_position_id", &overview.job_positions),
            ("academic_rank", &overview.academic_ranks),
            ("education_level", &overview.education_levels),
            ("subject_group_id", &overview.subject_groups),
        ] {
            if dimension != "subject_group_id" {
                assert_eq!(buckets.iter().map(|b| b.count).sum::<i64>(), expected);
            }
            for bucket in buckets {
                let filter: StaffListFilter = serde_json::from_value(
                    serde_json::json!({dimension:bucket.key,"status":status.as_str()}),
                )
                .unwrap();
                let (_, total, _, _) = list_staff(&pool, filter, StaffListAccess::School)
                    .await
                    .unwrap();
                assert_eq!(total, bucket.count, "{dimension}: {}", bucket.key);
            }
        }
    }
    let own = get_personnel_overview(
        &pool,
        PersonnelOverviewQuery { status: None },
        StaffListAccess::Own(c),
    )
    .await
    .unwrap();
    assert_eq!((own.total, own.filtered_total), (1, 0));
    for status in ["suspended", "resigned", "retired"] {
        let user = canonical_person(&pool).await;
        sqlx::query("UPDATE users SET status=$2 WHERE id=$1")
            .bind(user)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
    }
    let all = get_personnel_overview(
        &pool,
        PersonnelOverviewQuery {
            status: Some(PersonnelStatusFilter::All),
        },
        StaffListAccess::School,
    )
    .await
    .unwrap();
    assert_eq!(all.total, 6);
    assert_eq!(all.statuses.iter().map(|b| b.count).sum::<i64>(), 6);
    let empty = get_personnel_overview(
        &pool,
        PersonnelOverviewQuery { status: None },
        StaffListAccess::Own(Uuid::new_v4()),
    )
    .await
    .unwrap();
    assert_eq!(empty.total, 0);
}
