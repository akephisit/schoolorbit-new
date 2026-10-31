use super::catalog;
use crate::models::{CreateSubjectGroupRequest, UpdateSubjectGroupRequest};
use school_test_db::{create_named_test_pool_with_max_connections, run_test_migrations};
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(name: &str) -> PgPool {
    let pool = create_named_test_pool_with_max_connections(name, 4).await;
    run_test_migrations(&pool).await;
    pool
}
fn request(name: &str) -> CreateSubjectGroupRequest {
    CreateSubjectGroupRequest {
        name_th: name.into(),
        name_en: String::new(),
        display_order: None,
    }
}

#[tokio::test]
async fn learning_area_create_rename_and_archive_share_one_stable_owner() {
    let pool = fixture("learning_area_lifecycle").await;
    let group = catalog::create_subject_group(&pool, request("ภาษาจีน"))
        .await
        .unwrap();
    assert!(group.code.starts_with("SG-"));
    assert_eq!(group.id.get_version_num(), 7);
    assert_eq!(
        catalog::resolve_subject_group_owner(&pool, group.id)
            .await
            .unwrap(),
        group.organization_unit_id
    );
    let updated = catalog::update_subject_group(
        &pool,
        group.id,
        UpdateSubjectGroupRequest {
            name_th: "ภาษาจีนเพิ่มเติม".into(),
            name_en: "Chinese".into(),
            display_order: 20,
            is_active: Some(false),
            row_version: group.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.code, group.code);
    assert_eq!(updated.organization_unit_id, group.organization_unit_id);
    let unit: (String, Option<String>, bool, i32, bool) = sqlx::query_as(
        "SELECT u.name, u.name_en, u.is_active, u.display_order, root.code = 'SCHOOL'
         FROM organization_units u JOIN organization_units root ON root.id = u.parent_unit_id WHERE u.id = $1")
        .bind(group.organization_unit_id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        unit,
        (
            "ภาษาจีนเพิ่มเติม".into(),
            Some("Chinese".into()),
            false,
            20,
            true
        )
    );
    assert!(catalog::resolve_subject_group_owner(&pool, group.id)
        .await
        .is_err());
    assert!(catalog::update_subject_group(
        &pool,
        group.id,
        UpdateSubjectGroupRequest {
            name_th: "stale".into(),
            name_en: String::new(),
            display_order: 1,
            is_active: Some(true),
            row_version: group.row_version,
        }
    )
    .await
    .is_err());
}

#[tokio::test]
async fn learning_area_concurrent_creates_have_unique_codes_and_complete_pairs() {
    let pool = fixture("learning_area_concurrent").await;
    let mut workers = Vec::new();
    for index in 0..8 {
        let pool = pool.clone();
        workers.push(tokio::spawn(async move {
            catalog::create_subject_group(&pool, request(&format!("กลุ่ม {index}")))
                .await
                .unwrap()
        }));
    }
    let mut codes = std::collections::HashSet::new();
    let mut orders = std::collections::HashSet::new();
    for worker in workers {
        let group = worker.await.unwrap();
        assert!(codes.insert(group.code));
        assert!(orders.insert(group.display_order));
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM organization_units WHERE subject_group_id = $1",
        )
        .bind(group.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }
}

#[tokio::test]
async fn learning_area_owner_failure_rolls_back_registry_and_counter() {
    let pool = fixture("learning_area_rollback").await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM subject_groups")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_learning_area_owner() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF NEW.unit_type = 'subject_group' THEN RAISE EXCEPTION 'fixture rejection'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER reject_learning_area_owner BEFORE INSERT ON organization_units FOR EACH ROW EXECUTE FUNCTION reject_learning_area_owner();")
        .execute(&pool).await.unwrap();
    assert!(catalog::create_subject_group(&pool, request("ต้องย้อนกลับ"))
        .await
        .is_err());
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM subject_groups")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, count);
    sqlx::query("DROP TRIGGER reject_learning_area_owner ON organization_units")
        .execute(&pool)
        .await
        .unwrap();
    let group = catalog::create_subject_group(&pool, request("สำเร็จ"))
        .await
        .unwrap();
    assert_eq!(group.code, "SG-000001");
}

#[tokio::test]
async fn learning_area_in_use_and_activity_owner_cannot_be_disabled() {
    let pool = fixture("learning_area_dependencies").await;
    let group = catalog::create_subject_group(&pool, request("กลุ่มใช้งาน"))
        .await
        .unwrap();
    let subject_id: Uuid = sqlx::query_scalar("INSERT INTO subjects(id, code, identity_key, subject_group_id, owning_organization_unit_id) VALUES (uuidv7(),'TEST','TEST',$1,$2) RETURNING id")
        .bind(group.id).bind(group.organization_unit_id).fetch_one(&pool).await.unwrap();
    let update = UpdateSubjectGroupRequest {
        name_th: "กลุ่มใช้งาน".into(),
        name_en: String::new(),
        display_order: 10,
        is_active: Some(false),
        row_version: group.row_version,
    };
    assert!(
        catalog::update_subject_group(&pool, group.id, update.clone())
            .await
            .is_err()
    );
    let active: bool = sqlx::query_scalar("SELECT is_active FROM organization_units WHERE id = $1")
        .bind(group.organization_unit_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(active);
    sqlx::query("UPDATE subjects SET archived_at = now() WHERE id=$1")
        .bind(subject_id)
        .execute(&pool)
        .await
        .unwrap();
    catalog::update_subject_group(&pool, group.id, update)
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE subject_groups SET is_active=false WHERE code='AC'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("UPDATE organization_units SET parent_unit_id=(SELECT id FROM organization_units WHERE unit_type='management_group' LIMIT 1) WHERE id=$1")
        .bind(group.organization_unit_id).execute(&pool).await.is_err());
    assert!(
        sqlx::query("UPDATE subject_groups SET code='OTHER' WHERE id=$1")
            .bind(group.id)
            .execute(&pool)
            .await
            .is_err()
    );
}

#[test]
fn learning_area_creation_rejects_caller_codes() {
    assert!(serde_json::from_str::<CreateSubjectGroupRequest>(
        r#"{"code":"MANUAL","nameTh":"ทดสอบ"}"#
    )
    .is_err());
    assert!(serde_json::from_str::<CreateSubjectGroupRequest>(r#"{"nameTh":"ทดสอบ"}"#).is_ok());
}

async fn pre_separation_fixture(name: &str) -> PgPool {
    let pool = create_named_test_pool_with_max_connections(name, 4).await;
    let base = sqlx::migrate!("../../migrations");
    let migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            base.iter().filter(|m| m.version <= 115).cloned().collect(),
        ),
        ignore_missing: base.ignore_missing,
        locking: false,
        no_tx: base.no_tx,
        table_name: base.table_name,
        create_schemas: base.create_schemas,
    };
    school_test_db::run_fixture_migrator(&pool, &migrator)
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn learning_area_migration_rejects_ambiguous_states_and_reconciles_missing_owner() {
    let pool = pre_separation_fixture("learning_area_migration").await;
    let migration = include_str!("../../../../migrations/116_organization_learning_areas.sql");
    for (setup, expected) in [
        (
            "UPDATE organization_units SET is_active=false WHERE code='SCHOOL'",
            "ROOT_INVALID",
        ),
        (
            "UPDATE subject_groups SET is_active=NULL WHERE id=(SELECT id FROM subject_groups LIMIT 1)",
            "STATUS_CONFLICT",
        ),
        (
            "INSERT INTO organization_units(code,name,unit_type,subject_group_id,is_active) SELECT 'BAD-TYPE','ประเภทผิด','division',id,false FROM subject_groups LIMIT 1",
            "UNMAPPED_UNIT",
        ),
        (
            "INSERT INTO subjects(id,code,identity_key,subject_group_id,owning_organization_unit_id) SELECT uuidv7(),'WRONG-OWNER','WRONG-OWNER',g.id,u.id FROM subject_groups g CROSS JOIN organization_units u WHERE u.code='SCHOOL' LIMIT 1",
            "SUBJECT_OWNER_CONFLICT",
        ),
        (
            "UPDATE subject_groups SET is_active=false WHERE id=(SELECT subject_group_id FROM organization_units WHERE code='SUBJ-MA'); UPDATE organization_units SET is_active=false WHERE code='SUBJ-MA'; INSERT INTO subjects(id,code,identity_key,subject_group_id,owning_organization_unit_id) SELECT uuidv7(),'INACTIVE-OWNER','INACTIVE-OWNER',subject_group_id,id FROM organization_units WHERE code='SUBJ-MA'",
            "SUBJECT_OWNER_CONFLICT",
        ),
        (
            "UPDATE subject_groups SET name_th='' WHERE id=(SELECT id FROM subject_groups LIMIT 1)",
            "NAME_INVALID",
        ),
        (
            "INSERT INTO organization_units(code,name,unit_type) VALUES ('LOOSE','ไม่เชื่อม','subject_group')",
            "UNMAPPED_UNIT",
        ),
        (
            "INSERT INTO organization_units(code,name,unit_type,subject_group_id,is_active) SELECT 'DUP','ซ้ำ','subject_group',subject_group_id,false FROM organization_units WHERE unit_type='subject_group' LIMIT 1",
            "DUPLICATE_MAPPING",
        ),
        (
            "UPDATE organization_units SET is_active=false WHERE code='SUBJ-MA'",
            "STATUS_CONFLICT",
        ),
        (
            "UPDATE organization_units SET subject_group_id=NULL WHERE code='ACAD-ACT'",
            "ACTIVITY_MAPPING_INVALID",
        ),
        (
            "INSERT INTO organization_units(code,name,parent_unit_id) SELECT 'CHILD','หน่วยงานลูก',id FROM organization_units WHERE code='SUBJ-MA'",
            "SUBJECT_GROUP_HAS_CHILDREN",
        ),
    ] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::raw_sql(setup).execute(&mut *tx).await.unwrap();
        let error = sqlx::raw_sql(migration)
            .execute(&mut *tx)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected}: {error}"
        );
        tx.rollback().await.unwrap();
    }
    let before: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM organization_members), (SELECT count(*) FROM organization_permission_grants), (SELECT count(*) FROM organization_units)")
        .fetch_one(&pool).await.unwrap();
    let group_id: Uuid = sqlx::query_scalar("INSERT INTO subject_groups(code,name_th,name_en) VALUES ('ORPHAN','ทะเบียนยังไม่เชื่อม','') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    run_test_migrations(&pool).await;
    run_test_migrations(&pool).await;
    let after: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM organization_members), (SELECT count(*) FROM organization_permission_grants), (SELECT count(*) FROM organization_units)")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(after, (before.0, before.1, before.2 + 1));
    assert!(catalog::resolve_subject_group_owner(&pool, group_id)
        .await
        .is_ok());
    let misplaced: i64 = sqlx::query_scalar("SELECT count(*) FROM organization_units WHERE unit_type='subject_group' AND parent_unit_id IS DISTINCT FROM (SELECT id FROM organization_units WHERE code='SCHOOL')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(misplaced, 0);
}

#[tokio::test]
async fn learning_area_position_grants_do_not_cross_groups_or_administrative_tree() {
    use school_authorization::{
        accessible_exact_units_for_permission, accessible_tree_units_for_permission,
    };
    use school_permissions::registry::codes;
    let pool = fixture("learning_area_position_scope").await;
    let head = school_test_db::create_test_user(&pool, "area-head@example.test", "Fixture123!")
        .await
        .unwrap();
    let admin = school_test_db::create_test_user(&pool, "admin-head@example.test", "Fixture123!")
        .await
        .unwrap();
    let a = catalog::create_subject_group(&pool, request("กลุ่ม ก"))
        .await
        .unwrap();
    let b = catalog::create_subject_group(&pool, request("กลุ่ม ข"))
        .await
        .unwrap();
    for unit in [a.organization_unit_id, b.organization_unit_id] {
        sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code) SELECT $1,id,'head' FROM permissions WHERE code=$2")
            .bind(unit).bind(codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,started_at) VALUES ($1,$2,'head',CURRENT_DATE),($1,$3,'member',CURRENT_DATE)")
        .bind(head).bind(a.organization_unit_id).bind(b.organization_unit_id).execute(&pool).await.unwrap();
    let exact = accessible_exact_units_for_permission(
        &pool,
        head,
        codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT,
    )
    .await
    .unwrap();
    assert!(exact.contains(&a.organization_unit_id));
    assert!(!exact.contains(&b.organization_unit_id));
    let admin_unit: Uuid =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE code='ACAD-01'")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,started_at) VALUES ($1,$2,'head',CURRENT_DATE)")
        .bind(admin).bind(admin_unit).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code) SELECT $1,id,'head' FROM permissions WHERE code=$2 ON CONFLICT DO NOTHING")
        .bind(admin_unit).bind(codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE).execute(&pool).await.unwrap();
    let tree = accessible_tree_units_for_permission(
        &pool,
        admin,
        codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE,
    )
    .await
    .unwrap();
    assert!(!tree.contains(&a.organization_unit_id));
    assert!(!tree.contains(&b.organization_unit_id));
    assert!(tree.contains(&admin_unit));
    sqlx::query("UPDATE organization_members SET position_code='member' WHERE user_id=$1 AND organization_unit_id=$2")
        .bind(head).bind(a.organization_unit_id).execute(&pool).await.unwrap();
    assert!(accessible_exact_units_for_permission(
        &pool,
        head,
        codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT
    )
    .await
    .unwrap()
    .is_empty());
}

async fn wait_until_blocked(pool: &PgPool, blocker: i32) {
    for _ in 0..200 {
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))",
        )
        .bind(blocker)
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("expected concurrent catalog/registry decision to wait for its parent lock");
}

#[tokio::test]
async fn learning_area_catalog_creation_and_deactivation_cannot_race() {
    let pool = fixture("learning_area_catalog_race").await;
    let a = catalog::create_subject_group(&pool, request("กลุ่มเริ่มรายวิชาก่อน"))
        .await
        .unwrap();
    let mut insert_tx = pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *insert_tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO subjects(id,code,identity_key,subject_group_id,owning_organization_unit_id) VALUES (uuidv7(),'RACE-A','RACE-A',$1,$2)")
        .bind(a.id).bind(a.organization_unit_id).execute(&mut *insert_tx).await.unwrap();
    let worker_pool = pool.clone();
    let deactivate = tokio::spawn(async move {
        catalog::update_subject_group(
            &worker_pool,
            a.id,
            UpdateSubjectGroupRequest {
                name_th: a.name_th,
                name_en: a.name_en,
                display_order: 1,
                is_active: Some(false),
                row_version: a.row_version,
            },
        )
        .await
    });
    wait_until_blocked(&pool, blocker).await;
    insert_tx.commit().await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(10), deactivate)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );

    let b = catalog::create_subject_group(&pool, request("กลุ่มปิดก่อนเพิ่มรายวิชา"))
        .await
        .unwrap();
    let mut archive_tx = pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *archive_tx)
        .await
        .unwrap();
    sqlx::query("UPDATE subject_groups SET is_active=false WHERE id=$1")
        .bind(b.id)
        .execute(&mut *archive_tx)
        .await
        .unwrap();
    let worker_pool = pool.clone();
    let insert = tokio::spawn(async move {
        sqlx::query("INSERT INTO subjects(id,code,identity_key,subject_group_id,owning_organization_unit_id) VALUES (uuidv7(),'RACE-B','RACE-B',$1,$2)")
        .bind(b.id).bind(b.organization_unit_id).execute(&worker_pool).await
    });
    wait_until_blocked(&pool, blocker).await;
    archive_tx.commit().await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(10), insert)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    let invalid: i64 = sqlx::query_scalar("SELECT count(*) FROM subjects s JOIN subject_groups g ON g.id=s.subject_group_id WHERE s.archived_at IS NULL AND g.is_active IS NOT TRUE")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(invalid, 0);
}
