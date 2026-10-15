use super::{personnel_tests::migrate_through, staff_info_service, staff_service};
use school_test_db::create_named_test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool(name: &str, version: i64) -> PgPool {
    let pool = create_named_test_pool(name).await;
    migrate_through(&pool, version).await.unwrap();
    pool
}
async fn person(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic','เดิม','ทดสอบ','staff','active')").bind(id).bind(id.to_string()).execute(pool).await.unwrap();
    id
}
async fn patch(
    pool: &PgPool,
    user: Uuid,
    value: serde_json::Value,
) -> Result<(), school_errors::AppError> {
    staff_service::update_staff(pool, user, serde_json::from_value(value).unwrap(), user)
        .await
        .map(|_| ())
}

#[tokio::test]
async fn staff_profile_bilingual_validation_selection_search_and_pagination() {
    use crate::models::{StaffListAccess, StaffListFilter};
    let pool = pool("staff_profile_names", 115).await;
    let legacy = person(&pool).await;
    // Existing incomplete names remain readable and permit unrelated edits.
    patch(&pool, legacy, serde_json::json!({"nickname":"เดิม"}))
        .await
        .unwrap();
    assert!(
        patch(&pool, legacy, serde_json::json!({"first_name":"ใหม่"}))
            .await
            .is_err()
    );
    patch(
        &pool,
        legacy,
        serde_json::json!({"title_en":" Ms. ","first_name_en":" Alice ","last_name_en":"Example"}),
    )
    .await
    .unwrap();
    let english = staff_service::get_staff_profile(&pool, legacy, false)
        .await
        .unwrap();
    assert_eq!(english.display_name, "Ms. Alice Example");
    assert_eq!(english.first_name, "เดิม");
    assert!(english.has_complete_name);
    let other = person(&pool).await;
    patch(&pool,other,serde_json::json!({"title":"นาย","first_name":"กมล","last_name":"ทดสอบ","title_en":"Mr.","first_name_en":"Zed","last_name_en":"Example"})).await.unwrap();
    assert_eq!(
        staff_service::get_staff_profile(&pool, other, false)
            .await
            .unwrap()
            .display_name,
        "นายกมล ทดสอบ"
    );
    let (items, total, _, _) = staff_service::list_staff(
        &pool,
        serde_json::from_value::<StaffListFilter>(serde_json::json!({"search":"Zed"})).unwrap(),
        StaffListAccess::School,
    )
    .await
    .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].id, other);
    let (accounts, total, _, _) = staff_service::list_staff(
        &pool,
        serde_json::from_value(serde_json::json!({"search":legacy.to_string()})).unwrap(),
        StaffListAccess::School,
    )
    .await
    .unwrap();
    assert_eq!(total, 1);
    assert_eq!(accounts[0].id, legacy);
    let mut ids = Vec::new();
    for page in 1..=2 {
        let (items, total, _, _) = staff_service::list_staff(
            &pool,
            serde_json::from_value(serde_json::json!({"page":page,"page_size":1})).unwrap(),
            StaffListAccess::School,
        )
        .await
        .unwrap();
        assert_eq!(total, 2);
        ids.push(items[0].id);
    }
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    for value in [
        serde_json::json!({"first_name":"Incomplete","last_name":"Name"}),
        serde_json::json!({"title":"นาย","first_name":"ไทย","last_name_en":"Mixed"}),
    ] {
        let mut request = value;
        request["username"] = serde_json::json!(Uuid::now_v7().to_string());
        request["password"] = serde_json::json!("Synthetic-123!");
        request["role_ids"] = serde_json::json!([]);
        assert!(staff_service::create_staff(
            &pool,
            serde_json::from_value(request).unwrap(),
            legacy
        )
        .await
        .is_err());
    }
    let created=staff_service::create_staff(&pool,serde_json::from_value(serde_json::json!({"username":"english-create","password":"Synthetic-123!","role_ids":[],"title_en":"Dr.","first_name_en":"Ben","last_name_en":"Example"})).unwrap(),legacy).await.unwrap();
    assert_eq!(
        staff_service::get_staff_profile(&pool, created, false)
            .await
            .unwrap()
            .display_name,
        "Dr. Ben Example"
    );
}

#[tokio::test]
async fn staff_profile_education_collection_revision_ownership_order_and_rollback() {
    let pool = pool("staff_profile_educations", 115).await;
    let user = person(&pool).await;
    let foreign = person(&pool).await;
    patch(&pool,user,serde_json::json!({"staff_info":{"education":{"expected_revision":0,"items":[{"education_level":"master","major":"Science"},{"education_level":"bachelor"},{"education_level":"other"},{"education_level":"doctorate"}]},"teaching_license_number":"synthetic-license"}})).await.unwrap();
    let info = staff_info_service::read_staff_info(&pool, user)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(info.education_revision, 1);
    assert_eq!(
        info.educations
            .iter()
            .map(|row| row.education_level.unwrap().as_str())
            .collect::<Vec<_>>(),
        vec!["bachelor", "master", "doctorate", "other"]
    );
    assert_eq!(info.highest_education_level.unwrap().as_str(), "doctorate");
    assert!(staff_service::get_staff_profile(&pool, user, false)
        .await
        .unwrap()
        .staff_info
        .unwrap()
        .teaching_license_number
        .is_none());
    assert!(staff_service::get_staff_profile(&pool, user, true)
        .await
        .unwrap()
        .staff_info
        .unwrap()
        .teaching_license_number
        .is_some());
    assert!(matches!(patch(&pool,user,serde_json::json!({"nickname":"must rollback","staff_info":{"education":{"expected_revision":0,"items":[]}}})).await,Err(school_errors::AppError::Conflict(_))));
    assert!(staff_service::get_staff_profile(&pool, user, false)
        .await
        .unwrap()
        .nickname
        .is_none());
    let id = info.educations[0].id;
    for items in [
        serde_json::json!([{"id":id,"education_level":"bachelor"}]),
        serde_json::json!([{"education_level":null}]),
    ] {
        assert!(patch(
            &pool,
            foreign,
            serde_json::json!({"staff_info":{"education":{"expected_revision":0,"items":items}}})
        )
        .await
        .is_err());
    }
    assert!(patch(&pool,user,serde_json::json!({"staff_info":{"education":{"expected_revision":1,"items":[{"id":id,"education_level":"bachelor"},{"id":id,"education_level":"master"}]}}})).await.is_err());
    patch(&pool,user,serde_json::json!({"staff_info":{"education":{"expected_revision":1,"items":[{"id":id,"education_level":"bachelor","major":"Mathematics"}]}}})).await.unwrap();
    let after = staff_info_service::read_staff_info(&pool, user)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.educations.len(), 1);
    assert_eq!(after.educations[0].id, id);
    assert_eq!(after.education_revision, 2);
    patch(
        &pool,
        user,
        serde_json::json!({"staff_info":{"education":{"expected_revision":2,"items":[]}}}),
    )
    .await
    .unwrap();
    assert!(staff_info_service::read_staff_info(&pool, user)
        .await
        .unwrap()
        .unwrap()
        .highest_education_level
        .is_none());
    patch(&pool,user,serde_json::json!({"staff_info":{"education":{"expected_revision":3,"items":[{"education_level":"other"}]}}})).await.unwrap();
    assert_eq!(
        staff_info_service::read_staff_info(&pool, user)
            .await
            .unwrap()
            .unwrap()
            .highest_education_level
            .unwrap()
            .as_str(),
        "other"
    );
}

#[tokio::test]
async fn staff_profile_migration_preserves_partial_education_and_retries() {
    let pool = pool("staff_profile_migration_preserve", 112).await;
    let user = person(&pool).await;
    sqlx::query("INSERT INTO staff_info(user_id,major,teaching_license_number,metadata) VALUES($1,'Legacy partial','synthetic-license','{\"synthetic\":true}')").bind(user).execute(&pool).await.unwrap();
    let before: String = sqlx::query_scalar("SELECT (to_jsonb(u))::text FROM users u WHERE id=$1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    let old:(Uuid,String)=sqlx::query_as("SELECT id,(to_jsonb(i)-ARRAY['education_level','major','university'])::text FROM staff_info i WHERE user_id=$1").bind(user).fetch_one(&pool).await.unwrap();
    migrate_through(&pool, 115).await.unwrap();
    migrate_through(&pool, 115).await.unwrap();
    let after:String=sqlx::query_scalar("SELECT (to_jsonb(u)-ARRAY['title_en','first_name_en','last_name_en'])::text FROM users u WHERE id=$1").bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    let preserved: String = sqlx::query_scalar(
        "SELECT (to_jsonb(i)-ARRAY['education_revision','teaching_license_file_id'])::text FROM staff_info i WHERE user_id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(old.1, preserved);
    let info = staff_info_service::read_staff_info(&pool, user)
        .await
        .unwrap()
        .unwrap();
    assert!(info.teaching_license_file_id.is_none());
    assert_eq!(info.educations[0].id, old.0);
    assert_eq!(info.educations[0].major.as_deref(), Some("Legacy partial"));
    assert!(info.highest_education_level.is_none());
    assert!(
        crate::profile_cutover::read_profile_cutover(&pool)
            .await
            .unwrap()
            .passed
    );
}

#[tokio::test]
async fn staff_profile_migration_blocks_missing_stale_or_failed_reconciliation() {
    for (index, damage) in [
        "DELETE FROM staff_profile_cutover_audit",
        "UPDATE staff_info SET major='Changed'",
        "DELETE FROM staff_educations",
    ]
    .into_iter()
    .enumerate()
    {
        let pool = pool(&format!("staff_profile_migration_block_{index}"), 112).await;
        let user = person(&pool).await;
        sqlx::query(
            "INSERT INTO staff_info(user_id,education_level,major) VALUES($1,'bachelor','Science')",
        )
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
        migrate_through(&pool, 113).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(damage))
            .execute(&pool)
            .await
            .unwrap();
        let error = migrate_through(&pool, 114).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("STAFF_PROFILE_RECONCILIATION_REQUIRED"));
        let columns:i64=sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='staff_info' AND column_name IN('education_level','major','university')").fetch_one(&pool).await.unwrap();
        assert_eq!(columns, 3);
    }
}

#[tokio::test]
async fn staff_licence_attachment_scope_revision_lifecycle_rollback_and_private_read() {
    use super::staff_license_service as licence;
    use school_authorization::ActorContext;
    use school_file_platform::{
        access::FilePolicyAction,
        platform_types::{FileLifecycleStatus, FilePurpose, FileVisibility},
        repository::PlatformFile,
    };
    use school_permissions::registry::codes;
    let pool =
        school_test_db::create_named_test_pool_with_max_connections("staff_licence_attachment", 2)
            .await;
    migrate_through(&pool, 115).await.unwrap();
    let user = person(&pool).await;
    let other = person(&pool).await;
    let manager = ActorContext {
        user_id: user,
        permissions: vec![
            codes::STAFF_UPDATE_ALL.into(),
            codes::STAFF_PII_READ_SCHOOL.into(),
            codes::STAFF_PROFILE_READ_SCHOOL.into(),
        ],
    };
    let reader = ActorContext {
        user_id: other,
        permissions: vec![codes::STAFF_PROFILE_READ_SCHOOL.into()],
    };
    assert_eq!(
        licence::authorize_upload(&pool, &manager, Some(user))
            .await
            .unwrap(),
        user
    );
    assert!(licence::authorize_upload(&pool, &manager, None)
        .await
        .is_err());
    assert!(
        licence::authorize_upload(&pool, &manager, Some(Uuid::new_v4()))
            .await
            .is_err()
    );
    let insert = |owner| {
        let pool = &pool;
        async move {
            sqlx::query_scalar::<_,Uuid>("INSERT INTO files(owner_user_id,created_by,display_filename,purpose_code,visibility,lifecycle_status,retention_class,expires_at) VALUES($1,$2,'synthetic.pdf','teaching_license','private','ready','temporary',now()+interval '1 hour') RETURNING id")
            .bind(owner).bind(user).fetch_one(pool).await.unwrap()
        }
    };
    let first = insert(user).await;
    let second = insert(user).await;
    let unrelated = insert(other).await;
    let file = |id, owner| PlatformFile {
        id,
        owner_user_id: Some(owner),
        purpose: FilePurpose::TeachingLicense,
        visibility: FileVisibility::Private,
        lifecycle_status: FileLifecycleStatus::Ready,
        current_version: None,
        display_filename: "synthetic.pdf".into(),
        detected_mime_type: "application/pdf".into(),
        byte_size: 100,
    };
    assert!(licence::authorize_file(
        &pool,
        &manager,
        &file(first, user),
        FilePolicyAction::Read,
        Some(user)
    )
    .await
    .is_ok());
    assert!(licence::authorize_file(
        &pool,
        &reader,
        &file(first, user),
        FilePolicyAction::Read,
        Some(user)
    )
    .await
    .is_err());
    assert!(licence::authorize_file(
        &pool,
        &manager,
        &file(first, user),
        FilePolicyAction::Read,
        Some(other)
    )
    .await
    .is_err());
    let attach = |expected: Option<Uuid>, next: Option<Uuid>| serde_json::json!({"staff_info":{"teaching_license_file":{"expected_file_id":expected,"file_id":next}}});
    assert!(patch(&pool, user, attach(None, Some(unrelated)))
        .await
        .is_err());
    for (column, value) in [
        ("purpose_code", "'profile_image'"),
        ("created_by", "$2"),
        ("lifecycle_status", "'processing'"),
        ("expires_at", "now()-interval '1 minute'"),
    ] {
        let candidate = insert(user).await;
        let statement = format!("UPDATE files SET {column}={value} WHERE id=$1");
        let query = sqlx::query(sqlx::AssertSqlSafe(statement.as_str())).bind(candidate);
        if column == "created_by" {
            query.bind(other).execute(&pool).await.unwrap();
        } else {
            query.execute(&pool).await.unwrap();
        }
        assert!(patch(&pool, user, attach(None, Some(candidate)))
            .await
            .is_err());
    }
    patch(&pool, user, attach(None, Some(first))).await.unwrap();
    assert!(
        licence::delete_guard(&pool, &manager, &file(first, user), Some(user))
            .await
            .is_err()
    );
    assert_eq!(
        staff_service::get_staff_profile(&pool, user, true)
            .await
            .unwrap()
            .staff_info
            .unwrap()
            .teaching_license_file_id,
        Some(first)
    );
    assert_eq!(
        staff_service::get_staff_profile(&pool, user, false)
            .await
            .unwrap()
            .staff_info
            .unwrap()
            .teaching_license_file_id,
        None
    );
    assert!(patch(&pool, user, attach(None, Some(second)))
        .await
        .is_err());
    let mut invalid = attach(Some(first), Some(second));
    invalid["staff_info"]["education"] = serde_json::json!({"expected_revision":99,"items":[]});
    assert!(patch(&pool, user, invalid).await.is_err());
    let state:(Option<Uuid>,String)=sqlx::query_as("SELECT i.teaching_license_file_id,f.retention_class FROM staff_info i CROSS JOIN files f WHERE i.user_id=$1 AND f.id=$2").bind(user).bind(second).fetch_one(&pool).await.unwrap();
    assert_eq!(state, (Some(first), "temporary".into()));
    patch(&pool, user, attach(Some(first), Some(second)))
        .await
        .unwrap();
    assert!(licence::authorize_file(
        &pool,
        &manager,
        &file(first, user),
        FilePolicyAction::Read,
        Some(user)
    )
    .await
    .is_err());
    let old: (String, bool) =
        sqlx::query_as("SELECT retention_class,expires_at<=now() FROM files WHERE id=$1")
            .bind(first)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(old, ("temporary".into(), true));
    // The deletion guard shares the parent lock with attachment writes.
    let guard = licence::delete_guard(&pool, &manager, &file(unrelated, other), Some(other))
        .await
        .unwrap();
    let mut concurrent = pool.begin().await.unwrap();
    assert!(
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE NOWAIT")
            .bind(other)
            .fetch_one(&mut *concurrent)
            .await
            .is_err()
    );
    concurrent.rollback().await.unwrap();
    guard.rollback().await.unwrap();
    patch(&pool, user, attach(Some(second), None))
        .await
        .unwrap();
    assert!(staff_info_service::read_staff_info(&pool, user)
        .await
        .unwrap()
        .unwrap()
        .teaching_license_file_id
        .is_none());
}
