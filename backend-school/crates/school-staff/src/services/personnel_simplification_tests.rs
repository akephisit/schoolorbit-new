use super::personnel_cutover_service::{read_personnel_cutover_audit, read_personnel_preflight};
use super::personnel_tests::migrate_through;
use school_test_db::create_named_test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn source_pool(name: &str) -> PgPool {
    assert!(
        sqlx::migrate!("../../migrations").version_exists(84),
        "personnel simplification migrations must exist"
    );
    let pool = create_named_test_pool(name).await;
    migrate_through(&pool, 82).await.unwrap();
    pool
}

async fn source_person(pool: &PgPool) -> (Uuid, Uuid) {
    let user = Uuid::new_v4();
    let position = Uuid::new_v4();
    let major = Uuid::new_v4();
    let university = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES ($1,$2,'synthetic','Fixture','Person','staff','active')")
        .bind(user).bind(user.to_string()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO staff_reference_items(id,kind,code,name,is_active,display_order) VALUES ($1,'job_position',$2,'ตำแหน่งเฉพาะเดิม',false,99),($3,'major',$4,'คณิตศาสตร์',false,0),($5,'university',$6,'สถาบันทดสอบ',false,0)")
        .bind(position).bind(format!("ref_{}",position.simple())).bind(major).bind(format!("ref_{}",major.simple())).bind(university).bind(format!("ref_{}",university.simple())).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO staff_info(user_id,job_position_id,major_id,university_id,academic_rank,education_level,employment_type,teaching_license_number,teaching_license_expiry,metadata) VALUES ($1,$2,$3,$4,'none','bachelor','contract','synthetic-license','2030-01-01','{\"synthetic\":true}')")
        .bind(user).bind(position).bind(major).bind(university).execute(pool).await.unwrap();
    (user, position)
}

async fn invariant_snapshot(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT jsonb_build_array(id,user_id,job_position_id,academic_rank,education_level,employment_type,teaching_license_number,teaching_license_expiry,metadata,created_at,updated_at)::text FROM staff_info ORDER BY user_id")
        .fetch_all(pool).await.unwrap()
}

async fn source_exists(pool: &PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT to_regclass(current_schema() || '.staff_reference_items') IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn personnel_simplification_preserves_reference_names_and_ids() {
    let pool = source_pool("personnel_simplification_preserves").await;
    let (user, position) = source_person(&pool).await;
    let before = invariant_snapshot(&pool).await;
    let positions: Vec<String> = sqlx::query_scalar("SELECT (to_jsonb(r)-ARRAY['kind','normalized_name'])::text FROM staff_reference_items r WHERE kind='job_position' ORDER BY id").fetch_all(&pool).await.unwrap();
    migrate_through(&pool, 84).await.unwrap();
    assert_eq!(before, invariant_snapshot(&pool).await);
    let actual: (Option<Uuid>, Option<String>, Option<String>) =
        sqlx::query_as("SELECT job_position_id,major,university FROM staff_info WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        actual,
        (
            Some(position),
            Some("คณิตศาสตร์".into()),
            Some("สถาบันทดสอบ".into())
        )
    );
    let after: Vec<String> = sqlx::query_scalar(
        "SELECT (to_jsonb(p)-'is_selectable')::text FROM staff_job_positions p ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(positions, after);
    let selectable: bool =
        sqlx::query_scalar("SELECT is_selectable FROM staff_job_positions WHERE id=$1")
            .bind(position)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!selectable);
    assert!(!source_exists(&pool).await);
    let audit = read_personnel_cutover_audit(&pool).await.unwrap();
    assert!(audit.passed);
    assert_eq!(audit.migration_version, 84);
    assert_eq!(audit.checks.len(), 7);
    migrate_through(&pool, 84).await.unwrap();
    assert_eq!(before, invariant_snapshot(&pool).await);
}

#[tokio::test]
async fn personnel_simplification_empty_source_succeeds() {
    let pool = source_pool("personnel_simplification_empty").await;
    migrate_through(&pool, 84).await.unwrap();
    assert!(read_personnel_cutover_audit(&pool).await.unwrap().passed);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM staff_info")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let selectable: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_job_positions WHERE is_selectable AND is_active",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(selectable, 7);
}

#[tokio::test]
async fn personnel_simplification_blocks_missing_or_wrong_kind_reference() {
    for wrong_kind in [false, true] {
        let pool = source_pool(if wrong_kind {
            "personnel_simplification_wrong_kind"
        } else {
            "personnel_simplification_missing"
        })
        .await;
        let (user, position) = source_person(&pool).await;
        sqlx::query("ALTER TABLE staff_info DROP CONSTRAINT staff_info_major_reference_fkey")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE staff_info SET major_id=$1 WHERE user_id=$2")
            .bind(if wrong_kind { position } else { Uuid::new_v4() })
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
        assert!(!read_personnel_preflight(&pool).await.unwrap().passed);
        let before = invariant_snapshot(&pool).await;
        let error = migrate_through(&pool, 83).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("PERSONNEL_SIMPLIFICATION_SOURCE_INVALID"));
        assert!(source_exists(&pool).await);
        assert_eq!(before, invariant_snapshot(&pool).await);
        let partial: bool = sqlx::query_scalar(
            "SELECT to_regclass(current_schema() || '.staff_job_positions') IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!partial);
    }
}

#[tokio::test]
async fn personnel_simplification_blocks_stale_or_partial_audit() {
    let mutations = [
        "DELETE FROM staff_personnel_simplification_audit",
        "UPDATE staff_personnel_simplification_audit SET checks='[]'::jsonb",
        "UPDATE staff_personnel_simplification_audit SET checks=jsonb_set(checks,'{1}',checks->0)",
        "UPDATE staff_personnel_simplification_audit SET checks=jsonb_set(checks,'{0,count}','999'::jsonb)",
        "UPDATE staff_personnel_simplification_audit SET checks=jsonb_set(checks,'{0,count}','-1'::jsonb)",
        "UPDATE staff_personnel_simplification_audit SET checks=jsonb_set(checks,'{0,passed}','false'::jsonb)",
        "UPDATE staff_personnel_simplification_audit SET checks=jsonb_set(checks,'{0,code}',to_jsonb('wrong'::text))",
        "UPDATE staff_personnel_simplification_audit SET source_fingerprint='stale'",
        "UPDATE staff_info SET major='changed target'",
        "UPDATE staff_reference_items SET name='changed source' WHERE kind='major'",
    ];
    for (index, mutation) in mutations.iter().enumerate() {
        let pool = source_pool(&format!("personnel_simplification_stale_{index}")).await;
        source_person(&pool).await;
        migrate_through(&pool, 83).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(*mutation))
            .execute(&pool)
            .await
            .unwrap();
        assert!(!read_personnel_preflight(&pool).await.unwrap().passed);
        assert!(migrate_through(&pool, 84).await.is_err());
        assert!(source_exists(&pool).await);
        let version: i64 =
            sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE success")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(version, 83);
    }
}

#[tokio::test]
async fn personnel_simplification_failed_cleanup_retries_atomically() {
    let pool = source_pool("personnel_simplification_retry").await;
    let (user, _) = source_person(&pool).await;
    migrate_through(&pool, 83).await.unwrap();
    sqlx::query("UPDATE staff_personnel_simplification_audit SET target_fingerprint='stale'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(migrate_through(&pool, 84).await.is_err());
    assert!(source_exists(&pool).await);
    sqlx::query("UPDATE staff_personnel_simplification_audit SET target_fingerprint=md5(staff_personnel_simplification_snapshot(false)::text)").execute(&pool).await.unwrap();
    migrate_through(&pool, 84).await.unwrap();
    let major: String = sqlx::query_scalar("SELECT major FROM staff_info WHERE user_id=$1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(major, "คณิตศาสตร์");
    assert!(read_personnel_cutover_audit(&pool).await.unwrap().passed);
}

#[tokio::test]
async fn personnel_simplification_completed_audit_survives_valid_edits() {
    let pool = source_pool("personnel_simplification_edit_audit").await;
    source_person(&pool).await;
    migrate_through(&pool, 84).await.unwrap();
    sqlx::query("UPDATE staff_info SET major='วิทยาศาสตร์'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(read_personnel_cutover_audit(&pool).await.unwrap().passed);
    sqlx::query("ALTER TABLE staff_info DROP CONSTRAINT staff_info_job_position_fkey")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!read_personnel_cutover_audit(&pool).await.unwrap().passed);
}

#[tokio::test]
async fn personnel_simplification_null_and_shared_names_are_preserved() {
    let pool = source_pool("personnel_simplification_shared").await;
    let (first, _) = source_person(&pool).await;
    let second = Uuid::new_v4();
    let third = Uuid::new_v4();
    for user in [second, third] {
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES ($1,$2,'synthetic','Fixture','Person','staff','active')").bind(user).bind(user.to_string()).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO staff_info(user_id,major_id,university_id) SELECT $1,major_id,university_id FROM staff_info WHERE user_id=$2").bind(second).bind(first).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO staff_info(user_id) VALUES ($1)")
        .bind(third)
        .execute(&pool)
        .await
        .unwrap();
    migrate_through(&pool, 84).await.unwrap();
    let values: Vec<(Option<String>,Option<String>)> = sqlx::query_as("SELECT major,university FROM staff_info WHERE user_id=ANY($1) ORDER BY array_position($1::uuid[],user_id)").bind(vec![first,second,third]).fetch_all(&pool).await.unwrap();
    assert_eq!(
        values,
        vec![
            (Some("คณิตศาสตร์".into()), Some("สถาบันทดสอบ".into())),
            (Some("คณิตศาสตร์".into()), Some("สถาบันทดสอบ".into())),
            (None, None)
        ]
    );
}
