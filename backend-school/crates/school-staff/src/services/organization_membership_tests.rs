use super::{
    organization_member_service as members, organization_unit_service as units, staff_service,
};
use crate::models::{CreateStaffRequest, UpdateOrganizationUnitRequest, UpdateStaffRequest};
use school_authorization::{get_cached_user_permissions, PermissionCache};
use school_errors::AppError;
use school_test_db::{
    create_named_test_pool_with_max_connections, create_test_user, run_test_migrations,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(name: &str) -> PgPool {
    let pool = create_named_test_pool_with_max_connections(name, 5).await;
    run_test_migrations(&pool).await;
    pool
}
async fn person(pool: &PgPool) -> Uuid {
    create_test_user(
        pool,
        &format!("{}@example.test", Uuid::now_v7()),
        "Test1234!",
    )
    .await
    .unwrap()
}
async fn unit(pool: &PgPool, parent: Option<Uuid>) -> Uuid {
    sqlx::query_scalar("INSERT INTO organization_units(id,code,name,unit_type,parent_unit_id) VALUES($1,$2,'Test administrative unit','division',$3) RETURNING id")
        .bind(Uuid::now_v7()).bind(format!("T-{}",Uuid::now_v7())).bind(parent).fetch_one(pool).await.unwrap()
}
async fn add(pool: &PgPool, user: Uuid, unit: Uuid, primary: bool) {
    members::add_member(pool, user, unit, "member", None, primary, None)
        .await
        .unwrap();
}
async fn learning_area(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT id FROM organization_units WHERE unit_type='subject_group' AND is_active ORDER BY id LIMIT 1").fetch_one(pool).await.unwrap()
}

async fn observe_blocked_backend(pool: &PgPool, blocker: i32, expected_waiter: Option<i32>) -> i32 {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiter: Option<i32> = sqlx::query_scalar(
                "SELECT pid FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid))
                 AND ($2::int IS NULL OR pid = $2) ORDER BY pid LIMIT 1",
            )
            .bind(blocker)
            .bind(expected_waiter)
            .fetch_optional(pool)
            .await
            .unwrap();
            if let Some(waiter) = waiter {
                return waiter;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("second backend must actually wait on the held transaction")
}

async fn workflow_selector_reference_cases(pool: &PgPool, actor: Uuid) {
    use sqlx::types::Json;
    let mut targets = Vec::new();
    for _ in 0..5 {
        targets.push(unit(pool, None).await);
    }
    let selector =
        |target: Uuid| serde_json::json!({"kind":"organization_unit","organizationUnitId":target});
    let template = Uuid::now_v7();
    let current = serde_json::json!({"steps":[{"recipients":[selector(targets[0])]}]});
    sqlx::query("INSERT INTO process_templates(id,code,version,definition,created_by) VALUES($1,$2,1,$3,$4)")
        .bind(template).bind(format!("selector-{template}")).bind(Json(&current)).bind(actor).execute(pool).await.unwrap();
    let historical = serde_json::json!({"steps":[{"recipients":[selector(targets[1])]}]});
    sqlx::query("INSERT INTO process_template_versions(template_id,version,definition,created_by) VALUES($1,1,$2,$3)")
        .bind(template).bind(Json(&historical)).bind(actor).execute(pool).await.unwrap();
    let instance = Uuid::now_v7();
    let directed =
        serde_json::json!({"directed":{"assignments":[{"selector":selector(targets[2])}]}});
    sqlx::query("INSERT INTO process_instances(id,source_module,source_type,source_id,revision,definition,created_by) VALUES($1,'fixture','fixture',$2,1,$3,$4)")
        .bind(instance).bind(Uuid::now_v7()).bind(Json(&directed)).bind(actor).execute(pool).await.unwrap();
    let revision =
        serde_json::json!({"steps":[{"definition":{"recipients":[selector(targets[3])]}}]});
    sqlx::query(
        "INSERT INTO process_instance_revisions(instance_id,revision,definition) VALUES($1,1,$2)",
    )
    .bind(instance)
    .bind(Json(&revision))
    .execute(pool)
    .await
    .unwrap();
    let compact = targets[4].simple().to_string().to_uppercase();
    let settings = serde_json::json!({"defaultFirstRecipient":{"kind":"organization_unit","organizationUnitId":compact}});
    sqlx::query("INSERT INTO correspondence_routing_settings(register,revision,definition,updated_by) VALUES('incoming',1,$1,$2)")
        .bind(Json(&settings)).bind(actor).execute(pool).await.unwrap();
    for target in &targets {
        assert!(
            matches!(units::delete_organization_unit_permanently(pool,*target,actor).await,Err(AppError::Conflict(message)) if message.contains("กระบวนงาน")),
            "every functional JSON owner prevents orphaning its unit selector"
        );
    }
    let stored: Json<serde_json::Value> = sqlx::query_scalar(
        "SELECT definition FROM correspondence_routing_settings WHERE register='incoming'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        stored.0, settings,
        "typed UUID comparison never rewrites preserved JSON bytes"
    );
    let missing = serde_json::json!({"defaultReviewer":selector(Uuid::now_v7())});
    let missing_error = sqlx::query(
        "UPDATE correspondence_routing_settings SET definition=$1 WHERE register='incoming'",
    )
    .bind(Json(&missing))
    .execute(pool)
    .await
    .unwrap_err();
    assert_eq!(
        missing_error.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    let invalid = sqlx::query(
        "SELECT * FROM organization_selector_unit_ids('{\"organizationUnitId\":42}'::jsonb)",
    )
    .fetch_all(pool)
    .await
    .unwrap_err();
    assert_eq!(
        invalid.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    for invalid in [
        serde_json::json!({"organizationUnitId":{}}),
        serde_json::json!({"organizationUnitId":[]}),
        serde_json::json!({"organizationUnitId":true}),
    ] {
        let error = sqlx::query("SELECT * FROM organization_selector_unit_ids($1)")
            .bind(Json(&invalid))
            .fetch_all(pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    let null_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT * FROM organization_selector_unit_ids('{\"organizationUnitId\":null}'::jsonb)",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert!(null_ids.is_empty());
    let urn = serde_json::json!({"organizationUnitId":format!("urn:uuid:{}",targets[0])});
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT * FROM organization_selector_unit_ids($1)")
        .bind(Json(&urn))
        .fetch_all(pool)
        .await
        .unwrap();
    assert_eq!(ids, vec![targets[0]]);

    // Delete-first: the JSON writer must really wait on the unit lock, then reject absence.
    let delete_first = unit(pool, None).await;
    let mut deleter = pool.begin().await.unwrap();
    let deletion_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *deleter)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM organization_units WHERE id=$1 FOR UPDATE")
        .bind(delete_first)
        .execute(&mut *deleter)
        .await
        .unwrap();
    let mut writer_connection = pool.acquire().await.unwrap();
    let writer_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *writer_connection)
        .await
        .unwrap();
    let definition = serde_json::json!({"defaultReviewer":selector(delete_first)});
    let writer = tokio::spawn(async move {
        sqlx::query("INSERT INTO correspondence_routing_settings(register,revision,definition,updated_by) VALUES('internal',1,$1,$2)").bind(Json(&definition)).bind(actor).execute(&mut *writer_connection).await
    });
    observe_blocked_backend(pool, deletion_pid, Some(writer_pid)).await;
    assert!(!writer.is_finished());
    sqlx::query("DELETE FROM organization_units WHERE id=$1")
        .bind(delete_first)
        .execute(&mut *deleter)
        .await
        .unwrap();
    deleter.commit().await.unwrap();
    assert_eq!(
        writer
            .await
            .unwrap()
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23503")
    );

    // Writer-first: the service waits for the KEY SHARE guard and then sees the committed JSON.
    let writer_first = unit(pool, None).await;
    let mut settings_writer = pool.begin().await.unwrap();
    let settings_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *settings_writer)
        .await
        .unwrap();
    let definition = serde_json::json!({"defaultReviewer":selector(writer_first)});
    sqlx::query("INSERT INTO correspondence_routing_settings(register,revision,definition,updated_by) VALUES('internal',1,$1,$2)").bind(Json(&definition)).bind(actor).execute(&mut *settings_writer).await.unwrap();
    let delete_pool = pool.clone();
    let deletion = tokio::spawn(async move {
        units::delete_organization_unit_permanently(&delete_pool, writer_first, actor).await
    });
    observe_blocked_backend(pool, settings_pid, None).await;
    assert!(!deletion.is_finished());
    settings_writer.commit().await.unwrap();
    assert!(
        matches!(deletion.await.unwrap(),Err(AppError::Conflict(message)) if message.contains("กระบวนงาน"))
    );
}

#[tokio::test]
async fn organization_membership_recursive_display_is_cycle_safe_current_and_permission_neutral() {
    let pool = fixture("org_member_recursive").await;
    let root = unit(&pool, None).await;
    let mut chain = vec![root];
    for _ in 0..5 {
        chain.push(unit(&pool, chain.last().copied()).await);
    }
    let user = person(&pool).await;
    add(&pool, user, chain[5], false).await;
    add(&pool, user, chain[3], false).await;
    let direct = person(&pool).await;
    add(&pool, direct, root, false).await;
    for (start, end, status, inactive_unit) in [
        ("CURRENT_DATE+1", "NULL", "active", false),
        ("CURRENT_DATE-2", "CURRENT_DATE", "active", false),
        ("CURRENT_DATE-2", "NULL", "inactive", false),
        ("CURRENT_DATE-2", "NULL", "active", true),
    ] {
        let excluded = person(&pool).await;
        sqlx::query("UPDATE users SET status=$2 WHERE id=$1")
            .bind(excluded)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        let leaf = unit(&pool, Some(chain[4])).await;
        sqlx::query(sqlx::AssertSqlSafe(format!("INSERT INTO organization_members(user_id,organization_unit_id,position_code,started_at,ended_at) VALUES($1,$2,'head',{start},{end})"))).bind(excluded).bind(leaf).execute(&pool).await.unwrap();
        if inactive_unit {
            sqlx::query("UPDATE organization_units SET is_active=false WHERE id=$1")
                .bind(leaf)
                .execute(&pool)
                .await
                .unwrap();
        }
    }
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM organization_members")
        .fetch_one(&pool)
        .await
        .unwrap();
    let result = members::list_members(&pool, root, true).await.unwrap();
    assert_eq!(
        result.len(),
        3,
        "real source assignments remain available for person grouping"
    );
    assert_eq!(result[0].user_id, direct, "direct assignments sort first");
    assert_eq!(result.iter().filter(|r| r.user_id == user).count(), 2);
    assert_eq!(
        members::list_members(&pool, root, false)
            .await
            .unwrap()
            .len(),
        1
    );
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM organization_members")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "display cannot synthesize persisted memberships"
    );
    sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id) SELECT $1,id FROM permissions WHERE code='roles.create.all'").bind(root).execute(&pool).await.unwrap();
    let permissions = get_cached_user_permissions("org", user, &pool, &PermissionCache::new())
        .await
        .unwrap();
    assert!(
        !permissions.iter().any(|p| p == "roles.create.all"),
        "ancestor display must not grant exact parent rights"
    );
    let update: UpdateOrganizationUnitRequest =
        serde_json::from_value(serde_json::json!({"parent_unit_id":chain[5]})).unwrap();
    assert!(matches!(
        units::update_organization_unit(&pool, root, update, direct).await,
        Err(AppError::ValidationError(_))
    ));
    sqlx::query("UPDATE organization_units SET parent_unit_id=$2 WHERE id=$1")
        .bind(root)
        .bind(chain[5])
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        members::list_members(&pool, root, true)
            .await
            .unwrap()
            .len(),
        3,
        "cycle guard terminates without repeating source rows"
    );
    pool.close().await;
}

#[tokio::test]
async fn organization_membership_primary_domains_isolate_selection_and_raw_concurrency() {
    let pool = fixture("org_member_primary").await;
    let user = person(&pool).await;
    let area = learning_area(&pool).await;
    let a = unit(&pool, None).await;
    let b = unit(&pool, None).await;
    add(&pool, user, area, true).await;
    add(&pool, user, a, true).await;
    add(&pool, user, b, true).await;
    let primaries: Vec<Uuid> = sqlx::query_scalar(
        "SELECT organization_unit_id FROM organization_members WHERE user_id=$1 AND is_primary",
    )
    .bind(user)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(primaries.len(), 2);
    assert!(primaries.contains(&area) && primaries.contains(&b));
    let moving_user = person(&pool).await;
    add(&pool, moving_user, a, true).await;
    sqlx::query("UPDATE organization_members SET started_at=CURRENT_DATE-10 WHERE user_id=$1")
        .bind(moving_user)
        .execute(&pool)
        .await
        .unwrap();
    let source_id: Uuid =
        sqlx::query_scalar("SELECT id FROM organization_members WHERE user_id=$1")
            .bind(moving_user)
            .fetch_one(&pool)
            .await
            .unwrap();
    let input = |target, position: &str| members::UpdateMemberInput {
        organization_unit_id: a,
        user_id: moving_user,
        position_code: position.into(),
        position_title: Some("ตำแหน่งเฉพาะ".into()),
        is_primary: true,
        responsibilities: Some("ความรับผิดชอบเฉพาะ".into()),
        new_organization_unit_id: target,
    };
    members::update_member(&pool, input(a, "head"))
        .await
        .unwrap();
    let retained:bool=sqlx::query_scalar("SELECT id=$2 AND started_at=CURRENT_DATE-10 AND position_code='head' FROM organization_members WHERE user_id=$1").bind(moving_user).bind(source_id).fetch_one(&pool).await.unwrap();
    assert!(
        retained,
        "same-unit role edit preserves source identity and start date"
    );
    let fields: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT position_title,responsibilities FROM organization_members WHERE id=$1",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        fields,
        (Some("ตำแหน่งเฉพาะ".into()), Some("ความรับผิดชอบเฉพาะ".into()))
    );
    add(&pool, moving_user, b, false).await;
    assert!(matches!(
        members::update_member(&pool, input(b, "member")).await,
        Err(AppError::Conflict(_))
    ));
    let unchanged: bool = sqlx::query_scalar(
        "SELECT is_primary AND ended_at IS NULL FROM organization_members WHERE id=$1",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        unchanged,
        "duplicate-target rejection precedes source/primary mutations"
    );
    let target = unit(&pool, None).await;
    members::update_member(&pool, input(target, "member"))
        .await
        .unwrap();
    let preserved:bool=sqlx::query_scalar("SELECT organization_unit_id=$2 AND ended_at=CURRENT_DATE AND started_at=CURRENT_DATE-10 AND is_primary FROM organization_members WHERE id=$1").bind(source_id).bind(a).fetch_one(&pool).await.unwrap();
    assert!(
        preserved,
        "moving preserves the original source affiliation as dated history"
    );
    let ambiguous_user = person(&pool).await;
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,is_primary,started_at,ended_at) VALUES($1,$2,'member',true,CURRENT_DATE-5,CURRENT_DATE+5),($1,$2,'head',true,CURRENT_DATE+5,NULL)")
        .bind(ambiguous_user).bind(a).execute(&pool).await.unwrap();
    let before: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(m)::text FROM organization_members m WHERE user_id=$1 ORDER BY id",
    )
    .bind(ambiguous_user)
    .fetch_all(&pool)
    .await
    .unwrap();
    let ambiguous = members::UpdateMemberInput {
        organization_unit_id: a,
        user_id: ambiguous_user,
        position_code: "member".into(),
        position_title: None,
        is_primary: true,
        responsibilities: None,
        new_organization_unit_id: b,
    };
    assert!(matches!(
        members::update_member(&pool, ambiguous).await,
        Err(AppError::Conflict(_))
    ));
    let after: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(m)::text FROM organization_members m WHERE user_id=$1 ORDER BY id",
    )
    .bind(ambiguous_user)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        before, after,
        "unit-keyed edits must not guess between current and scheduled facts"
    );
    let scheduled_user = person(&pool).await;
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,is_primary,started_at,ended_at) VALUES($1,$2,'member',true,CURRENT_DATE-5,CURRENT_DATE+5),($1,$3,'member',true,CURRENT_DATE+5,NULL)")
        .bind(scheduled_user).bind(a).bind(b).execute(&pool).await.unwrap();
    let future_before:String=sqlx::query_scalar("SELECT row_to_json(m)::text FROM organization_members m WHERE user_id=$1 AND organization_unit_id=$2").bind(scheduled_user).bind(b).fetch_one(&pool).await.unwrap();
    members::update_member(
        &pool,
        members::UpdateMemberInput {
            organization_unit_id: a,
            user_id: scheduled_user,
            position_code: "head".into(),
            position_title: None,
            is_primary: true,
            responsibilities: None,
            new_organization_unit_id: a,
        },
    )
    .await
    .unwrap();
    let future_after:String=sqlx::query_scalar("SELECT row_to_json(m)::text FROM organization_members m WHERE user_id=$1 AND organization_unit_id=$2").bind(scheduled_user).bind(b).fetch_one(&pool).await.unwrap();
    assert_eq!(
        future_before, future_after,
        "bounded same-unit edit must preserve a nonoverlapping future primary fact"
    );
    let bounded_target = unit(&pool, None).await;
    members::update_member(
        &pool,
        members::UpdateMemberInput {
            organization_unit_id: a,
            user_id: scheduled_user,
            position_code: "head".into(),
            position_title: None,
            is_primary: true,
            responsibilities: None,
            new_organization_unit_id: bounded_target,
        },
    )
    .await
    .unwrap();
    let bounded:bool=sqlx::query_scalar("SELECT started_at=CURRENT_DATE AND ended_at=CURRENT_DATE+5 AND is_primary FROM organization_members WHERE user_id=$1 AND organization_unit_id=$2").bind(scheduled_user).bind(bounded_target).fetch_one(&pool).await.unwrap();
    assert!(bounded, "moving a bounded fact preserves its planned end");
    let future_target = unit(&pool, None).await;
    members::update_member(
        &pool,
        members::UpdateMemberInput {
            organization_unit_id: b,
            user_id: scheduled_user,
            position_code: "member".into(),
            position_title: None,
            is_primary: true,
            responsibilities: None,
            new_organization_unit_id: future_target,
        },
    )
    .await
    .unwrap();
    let scheduled:bool=sqlx::query_scalar("SELECT started_at=CURRENT_DATE+5 AND ended_at IS NULL AND is_primary FROM organization_members WHERE user_id=$1 AND organization_unit_id=$2").bind(scheduled_user).bind(future_target).fetch_one(&pool).await.unwrap();
    assert!(
        scheduled,
        "moving a future fact preserves its planned start"
    );
    let service_user = person(&pool).await;
    let add_primary = |pool: PgPool, target| async move {
        members::add_member(&pool, service_user, target, "member", None, true, None).await
    };
    let (left, right) = tokio::join!(add_primary(pool.clone(), a), add_primary(pool.clone(), b));
    left.unwrap();
    right.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM organization_members WHERE user_id=$1 AND is_primary",
    )
    .bind(service_user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "service selections serialize without cross-domain demotion"
    );
    let raw_user = person(&pool).await;
    let mut first = pool.begin().await.unwrap();
    let first_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *first)
        .await
        .unwrap();
    sqlx::query("INSERT INTO organization_members(id,user_id,organization_unit_id,position_code,is_primary,started_at,ended_at) VALUES($1,$2,$3,'member',true,CURRENT_DATE-1,CURRENT_DATE+10)")
        .bind(Uuid::now_v7()).bind(raw_user).bind(a).execute(&mut *first).await.unwrap();
    let mut second = pool.acquire().await.unwrap();
    let second_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *second)
        .await
        .unwrap();
    let second_insert = tokio::spawn(async move {
        sqlx::query("INSERT INTO organization_members(id,user_id,organization_unit_id,position_code,is_primary,started_at,ended_at) VALUES($1,$2,$3,'member',true,CURRENT_DATE-1,CURRENT_DATE+10)")
            .bind(Uuid::now_v7()).bind(raw_user).bind(b).execute(&mut *second).await
    });
    assert_eq!(
        observe_blocked_backend(&pool, first_pid, Some(second_pid)).await,
        second_pid
    );
    assert!(
        !second_insert.is_finished(),
        "losing raw writer is blocked before winner commits"
    );
    first.commit().await.unwrap();
    let error = second_insert.await.unwrap().unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,is_primary,started_at,ended_at) VALUES($1,$2,'member',true,CURRENT_DATE-10,CURRENT_DATE-1)").bind(raw_user).bind(area).execute(&pool).await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn organization_membership_bulk_save_preserves_history_dates_and_primary_flags() {
    let pool = fixture("org_member_bulk_history").await;
    let actor = person(&pool).await;
    let area = learning_area(&pool).await;
    let admin = unit(&pool, None).await;
    let historical = unit(&pool, None).await;
    let future = unit(&pool, None).await;
    let payload:CreateStaffRequest=serde_json::from_value(serde_json::json!({"username":format!("bulk{}",Uuid::now_v7()),"password":"Test1234!","title":"นาย","first_name":"ทดสอบ","last_name":"บุคลากร","role_ids":[],"organization_assignments":[{"organization_unit_id":area,"position_code":"member","is_primary":true},{"organization_unit_id":admin,"position_code":"member","position_title":"ตำแหน่งเดิม","responsibilities":"ภาระงานเดิม","is_primary":true}]})).unwrap();
    let user = staff_service::create_staff(&pool, payload, actor)
        .await
        .unwrap();
    sqlx::query("UPDATE organization_members SET started_at=CURRENT_DATE-20 WHERE user_id=$1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    let history_id:Uuid=sqlx::query_scalar("INSERT INTO organization_members(user_id,organization_unit_id,position_code,is_primary,started_at,ended_at) VALUES($1,$2,'member',false,CURRENT_DATE-30,CURRENT_DATE-21) RETURNING id").bind(user).bind(historical).fetch_one(&pool).await.unwrap();
    let future_id:Uuid=sqlx::query_scalar("INSERT INTO organization_members(user_id,organization_unit_id,position_code,started_at) VALUES($1,$2,'member',CURRENT_DATE+10) RETURNING id").bind(user).bind(future).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE organization_units SET is_active=false WHERE id=$1")
        .bind(admin)
        .execute(&pool)
        .await
        .unwrap();
    let assignments = serde_json::json!([{"organization_unit_id":area,"position_code":"member","is_primary":true},{"organization_unit_id":admin,"position_code":"member","position_title":"ตำแหน่งเดิม","responsibilities":"ภาระงานเดิม","is_primary":true},{"organization_unit_id":future,"position_code":"member","is_primary":false}]);
    let original:Vec<(Uuid,chrono::NaiveDate,Option<chrono::NaiveDate>,bool)>=sqlx::query_as("SELECT id,started_at,ended_at,is_primary FROM organization_members WHERE user_id=$1 ORDER BY id").bind(user).fetch_all(&pool).await.unwrap();
    for _ in 0..2 {
        let update: UpdateStaffRequest = serde_json::from_value(
            serde_json::json!({"nickname":"edited","organization_assignments":assignments}),
        )
        .unwrap();
        staff_service::update_staff(&pool, user, update, actor)
            .await
            .unwrap();
    }
    let saved:Vec<(Uuid,chrono::NaiveDate,Option<chrono::NaiveDate>,bool)>=sqlx::query_as("SELECT id,started_at,ended_at,is_primary FROM organization_members WHERE user_id=$1 ORDER BY id").bind(user).fetch_all(&pool).await.unwrap();
    assert_eq!(
        original, saved,
        "unchanged saves preserve IDs, dates, flags and ended history"
    );
    let fields:(Option<String>,Option<String>)=sqlx::query_as("SELECT position_title,responsibilities FROM organization_members WHERE user_id=$1 AND organization_unit_id=$2").bind(user).bind(admin).fetch_one(&pool).await.unwrap();
    assert_eq!(
        fields,
        (Some("ตำแหน่งเดิม".into()), Some("ภาระงานเดิม".into())),
        "whole-staff round trips retain custom assignment facts, including inactive sources"
    );
    let update:UpdateStaffRequest=serde_json::from_value(serde_json::json!({"organization_assignments":[{"organization_unit_id":area,"position_code":"member","is_primary":false},{"organization_unit_id":admin,"position_code":"member","position_title":"ตำแหน่งเดิม","responsibilities":"ภาระงานเดิม","is_primary":true}]})).unwrap();
    staff_service::update_staff(&pool, user, update, actor)
        .await
        .unwrap();
    let cancelled: bool =
        sqlx::query_scalar("SELECT ended_at=started_at FROM organization_members WHERE id=$1")
            .bind(future_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(cancelled);
    // A cancelled future empty interval must stay absent on the next edit/save.
    let repeat:UpdateStaffRequest=serde_json::from_value(serde_json::json!({"organization_assignments":[{"organization_unit_id":area,"position_code":"member","is_primary":false},{"organization_unit_id":admin,"position_code":"member","position_title":"ตำแหน่งเดิม","responsibilities":"ภาระงานเดิม","is_primary":true}]})).unwrap();
    staff_service::update_staff(&pool, user, repeat, actor)
        .await
        .unwrap();
    let history_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organization_members WHERE id=$1)")
            .bind(history_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(history_exists);
    let future_primary: bool = sqlx::query_scalar(
        "SELECT is_primary FROM organization_members WHERE user_id=$1 AND organization_unit_id=$2",
    )
    .bind(user)
    .bind(area)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!future_primary);
    let same_day = unit(&pool, None).await;
    add(&pool, user, same_day, false).await;
    let same_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM organization_members WHERE user_id=$1 AND organization_unit_id=$2",
    )
    .bind(user)
    .bind(same_day)
    .fetch_one(&pool)
    .await
    .unwrap();
    members::remove_member(&pool, same_day, user).await.unwrap();
    add(&pool, user, same_day, false).await;
    let restored_id:Uuid=sqlx::query_scalar("SELECT id FROM organization_members WHERE user_id=$1 AND organization_unit_id=$2 AND ended_at IS NULL").bind(user).bind(same_day).fetch_one(&pool).await.unwrap();
    assert_eq!(
        same_id, restored_id,
        "same-day explicit reassignment reuses stable unique identity"
    );
    let old_admin:(Uuid,chrono::NaiveDate,Option<chrono::NaiveDate>,Option<String>,Option<String>)=sqlx::query_as("SELECT id,started_at,ended_at,position_title,responsibilities FROM organization_members WHERE user_id=$1 AND organization_unit_id=$2").bind(user).bind(admin).fetch_one(&pool).await.unwrap();
    let active_admin = unit(&pool, None).await;
    let takeover:UpdateStaffRequest=serde_json::from_value(serde_json::json!({"organization_assignments":[{"organization_unit_id":area,"position_code":"member","is_primary":false},{"organization_unit_id":admin,"position_code":"member","position_title":"ตำแหน่งเดิม","responsibilities":"ภาระงานเดิม","is_primary":false},{"organization_unit_id":active_admin,"position_code":"member","is_primary":true},{"organization_unit_id":same_day,"position_code":"member","is_primary":false}]})).unwrap();
    staff_service::update_staff(&pool, user, takeover, actor)
        .await
        .unwrap();
    let retained_admin:(Uuid,chrono::NaiveDate,Option<chrono::NaiveDate>,Option<String>,Option<String>)=sqlx::query_as("SELECT id,started_at,ended_at,position_title,responsibilities FROM organization_members WHERE user_id=$1 AND organization_unit_id=$2").bind(user).bind(admin).fetch_one(&pool).await.unwrap();
    assert_eq!(
        old_admin, retained_admin,
        "active primary takeover retains inactive source identity, dates and custom facts"
    );
    let flags:Vec<(Uuid,bool)>=sqlx::query_as("SELECT organization_unit_id,is_primary FROM organization_members WHERE user_id=$1 AND organization_unit_id=ANY($2)").bind(user).bind([admin,active_admin].as_slice()).fetch_all(&pool).await.unwrap();
    assert!(flags.contains(&(admin, false)) && flags.contains(&(active_admin, true)));
    let promote_inactive:UpdateStaffRequest=serde_json::from_value(serde_json::json!({"organization_assignments":[{"organization_unit_id":admin,"position_code":"member","position_title":"ตำแหน่งเดิม","responsibilities":"ภาระงานเดิม","is_primary":true}]})).unwrap();
    assert!(
        matches!(
            staff_service::update_staff(&pool, user, promote_inactive, actor).await,
            Err(AppError::Conflict(_))
        ),
        "retaining an inactive assignment never permits a new promotion"
    );
    pool.close().await;
}

#[tokio::test]
async fn organization_membership_permanent_delete_blocks_all_reference_kinds_and_guards_races() {
    let pool = fixture("org_member_delete").await;
    let actor = person(&pool).await;
    let empty = unit(&pool, None).await;
    units::delete_organization_unit_permanently(&pool, empty, actor)
        .await
        .unwrap();
    assert!(matches!(
        units::get_organization_unit(&pool, empty).await,
        Err(AppError::NotFound(_))
    ));
    let parent = unit(&pool, None).await;
    let child = unit(&pool, Some(parent)).await;
    assert!(matches!(
        units::delete_organization_unit_permanently(&pool, parent, actor).await,
        Err(AppError::Conflict(_))
    ));
    let u = person(&pool).await;
    add(&pool, u, child, false).await;
    members::remove_member(&pool, child, u).await.unwrap();
    members::remove_member(&pool, child, u).await.unwrap();
    members::remove_member(&pool, child, Uuid::now_v7())
        .await
        .unwrap();
    assert!(
        matches!(
            units::delete_organization_unit_permanently(&pool, child, actor).await,
            Err(AppError::Conflict(_))
        ),
        "ended memberships still protect history"
    );
    let grant = unit(&pool, None).await;
    sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id) SELECT $1,id FROM permissions LIMIT 1").bind(grant).execute(&pool).await.unwrap();
    assert!(matches!(
        units::delete_organization_unit_permanently(&pool, grant, actor).await,
        Err(AppError::Conflict(_))
    ));
    let role_unit = unit(&pool, None).await;
    sqlx::query("INSERT INTO user_roles(user_id,role_id,organization_unit_id) SELECT $1,id,$2 FROM roles LIMIT 1").bind(actor).bind(role_unit).execute(&pool).await.unwrap();
    assert!(
        matches!(units::delete_organization_unit_permanently(&pool,role_unit,actor).await,Err(AppError::Conflict(message)) if message.contains("บทบาท"))
    );
    let delegation_unit = unit(&pool, None).await;
    sqlx::query("INSERT INTO organization_permission_delegations(from_user_id,to_user_id,permission_id,organization_unit_id,revoked_at) SELECT $1,$2,id,$3,now() FROM permissions LIMIT 1").bind(actor).bind(u).bind(delegation_unit).execute(&pool).await.unwrap();
    assert!(
        matches!(units::delete_organization_unit_permanently(&pool,delegation_unit,actor).await,Err(AppError::Conflict(message)) if message.contains("มอบหมาย"))
    );
    let root: Uuid = sqlx::query_scalar("SELECT id FROM organization_units WHERE code='SCHOOL'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let area = learning_area(&pool).await;
    for protected in [root, area] {
        assert!(matches!(
            units::delete_organization_unit_permanently(&pool, protected, actor).await,
            Err(AppError::Conflict(_))
        ));
    }
    // Catalog-backed coverage includes CASCADE and SET NULL without deleting either record.
    let linked = unit(&pool, None).await;
    sqlx::query("CREATE TABLE test_org_delete_reference(id uuid PRIMARY KEY, unit_id uuid REFERENCES organization_units(id) ON DELETE CASCADE)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO test_org_delete_reference VALUES($1,$2)")
        .bind(Uuid::now_v7())
        .bind(linked)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        units::delete_organization_unit_permanently(&pool, linked, actor).await,
        Err(AppError::Conflict(_))
    ));
    let raced = unit(&pool, None).await;
    let mut tx = pool.begin().await.unwrap();
    let deletion_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM organization_units WHERE id=$1 FOR UPDATE")
        .bind(raced)
        .execute(&mut *tx)
        .await
        .unwrap();
    let mut writer_connection = pool.acquire().await.unwrap();
    let writer_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *writer_connection)
        .await
        .unwrap();
    let writer = tokio::spawn(async move {
        sqlx::query("INSERT INTO test_org_delete_reference VALUES($1,$2)")
            .bind(Uuid::now_v7())
            .bind(raced)
            .execute(&mut *writer_connection)
            .await
    });
    observe_blocked_backend(&pool, deletion_pid, Some(writer_pid)).await;
    assert!(!writer.is_finished());
    let category: Option<String> =
        sqlx::query_scalar("SELECT organization_unit_reference_category($1)")
            .bind(raced)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(category, None);
    sqlx::query("DELETE FROM organization_units WHERE id=$1")
        .bind(raced)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        writer
            .await
            .unwrap()
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23503")
    );
    // In the other lock order, the service waits for the existing FK writer then refuses deletion.
    let writer_first_unit = unit(&pool, None).await;
    let mut reference_writer = pool.begin().await.unwrap();
    let reference_writer_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *reference_writer)
        .await
        .unwrap();
    sqlx::query("INSERT INTO test_org_delete_reference VALUES($1,$2)")
        .bind(Uuid::now_v7())
        .bind(writer_first_unit)
        .execute(&mut *reference_writer)
        .await
        .unwrap();
    let deletion_pool = pool.clone();
    let deletion = tokio::spawn(async move {
        units::delete_organization_unit_permanently(&deletion_pool, writer_first_unit, actor).await
    });
    let waiter = observe_blocked_backend(&pool, reference_writer_pid, None).await;
    assert_ne!(waiter, reference_writer_pid);
    assert!(!deletion.is_finished());
    reference_writer.commit().await.unwrap();
    assert!(matches!(
        deletion.await.unwrap(),
        Err(AppError::Conflict(_))
    ));
    let retained: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM test_org_delete_reference WHERE unit_id=$1)",
    )
    .bind(writer_first_unit)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(retained);
    // Same-actor membership writer holds the user first; deletion must wait before unit lock.
    let own_unit = unit(&pool, None).await;
    let mut member_writer = pool.begin().await.unwrap();
    let member_writer_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *member_writer)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(actor)
        .execute(&mut *member_writer)
        .await
        .unwrap();
    let delete_pool = pool.clone();
    let own_deletion = tokio::spawn(async move {
        units::delete_organization_unit_permanently(&delete_pool, own_unit, actor).await
    });
    let waiter = observe_blocked_backend(&pool, member_writer_pid, None).await;
    assert_ne!(waiter, member_writer_pid);
    assert!(!own_deletion.is_finished());
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES($1,$2,'member')").bind(actor).bind(own_unit).execute(&mut *member_writer).await.unwrap();
    member_writer.commit().await.unwrap();
    assert!(
        matches!(own_deletion.await.unwrap(), Err(AppError::Conflict(_))),
        "same-actor lock order resolves as a linked-reference conflict, never deadlock"
    );
    workflow_selector_reference_cases(&pool, actor).await;
    pool.close().await;
}

#[tokio::test]
async fn organization_membership_migration_preserves_flags_and_refuses_ambiguous_history() {
    use super::personnel_tests::migrate_through;
    let pool = create_named_test_pool_with_max_connections("org_member_migration", 2).await;
    migrate_through(&pool, 131).await.unwrap();
    let user = person(&pool).await;
    let a = unit(&pool, None).await;
    let b = unit(&pool, None).await;
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,is_primary,started_at,ended_at) VALUES($1,$2,'member',true,CURRENT_DATE-10,CURRENT_DATE-5),($1,$3,'member',true,CURRENT_DATE-8,CURRENT_DATE-2)").bind(user).bind(a).bind(b).execute(&pool).await.unwrap();
    let before: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(m)::text FROM organization_members m WHERE user_id=$1 ORDER BY id",
    )
    .bind(user)
    .fetch_all(&pool)
    .await
    .unwrap();
    let error = migrate_through(&pool, 132).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("ORGANIZATION_132_PRIMARY_OVERLAP"));
    let after: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(m)::text FROM organization_members m WHERE user_id=$1 ORDER BY id",
    )
    .bind(user)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after, "failed migration preserves all source rows");
    sqlx::query("UPDATE organization_members SET started_at=CURRENT_DATE-5 WHERE user_id=$1 AND organization_unit_id=$2").bind(user).bind(b).execute(&pool).await.unwrap();
    let reconciled: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(m)::text FROM organization_members m WHERE user_id=$1 ORDER BY id",
    )
    .bind(user)
    .fetch_all(&pool)
    .await
    .unwrap();
    let orphan_id = Uuid::now_v7();
    sqlx::query("INSERT INTO process_templates(id,code,version,definition) VALUES($1,'legacy-selector',1,$2)")
        .bind(orphan_id).bind(sqlx::types::Json(serde_json::json!({"steps":[{"recipients":[{"kind":"organization_unit","organizationUnitId":Uuid::now_v7()}]}]}))).execute(&pool).await.unwrap();
    let orphan_error = migrate_through(&pool, 132).await.unwrap_err();
    assert!(orphan_error
        .to_string()
        .contains("ORGANIZATION_132_WORKFLOW_SELECTOR_ORPHANS"));
    sqlx::query("UPDATE process_templates SET definition=$2 WHERE id=$1").bind(orphan_id)
        .bind(sqlx::types::Json(serde_json::json!({"steps":[{"recipients":[{"kind":"organization_unit","organizationUnitId":a.simple().to_string().to_uppercase()}]}]}))).execute(&pool).await.unwrap();
    migrate_through(&pool, 132).await.unwrap();
    migrate_through(&pool, 132).await.unwrap();
    let after: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(m)::text FROM organization_members m WHERE user_id=$1 ORDER BY id",
    )
    .bind(user)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        reconciled, after,
        "migration/retry never infers new chosen primaries"
    );
    pool.close().await;
}
