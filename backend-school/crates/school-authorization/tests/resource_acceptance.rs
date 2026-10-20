use school_authorization::resource_access_policy::{
    eligible_active_staff_for_resource_on_connection, ResourceAccessPermissions,
    ResourceAccessTarget,
};
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;
async fn user(pool: &PgPool, name: &str, read: Option<&str>, unit: Option<Uuid>) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic',$3,'Fixture','staff','active')").bind(id).bind(id.to_string()).bind(name).execute(pool).await.unwrap();
    let role = Uuid::now_v7();
    sqlx::query("INSERT INTO roles(id,code,name,user_type) VALUES($1,$2,$2,'staff')")
        .bind(role)
        .bind(format!("fixture_{role}"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2 OR code=$3").bind(role).bind(codes::BUDGET_APPROVE_ASSIGNED).bind(read).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(id,user_id,role_id,organization_unit_id,started_at) VALUES($1,$2,$3,$4,'2020-01-01')").bind(Uuid::now_v7()).bind(id).bind(role).bind(unit).execute(pool).await.unwrap();
    id
}
async fn member(pool: &PgPool, user: Uuid, unit: Uuid) {
    sqlx::query("INSERT INTO organization_members(id,user_id,organization_unit_id,position_code,started_at) VALUES($1,$2,$3,'member','2020-01-01')").bind(Uuid::now_v7()).bind(user).bind(unit).execute(pool).await.unwrap();
}
#[tokio::test]
async fn resource_acceptance_preserves_direct_scope_union_and_permission_origin() {
    let pool = school_test_db::create_named_test_pool("resource_acceptance_scope_union").await;
    school_test_db::run_test_migrations(&pool).await;
    let root: Uuid = sqlx::query_scalar("SELECT id FROM organization_units WHERE code='BUD-01'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let child: Uuid = sqlx::query_scalar("SELECT id FROM organization_units WHERE code='BUD-PLA'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let elsewhere: Uuid =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE code='BUD-FIN'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let owner = user(&pool, "Own", Some(codes::BUDGET_READ_OWN), None).await;
    let assigned = user(&pool, "Assigned", Some(codes::BUDGET_READ_ASSIGNED), None).await;
    let school = user(&pool, "School", Some(codes::BUDGET_READ_SCHOOL), None).await;
    let exact = user(
        &pool,
        "Exact",
        Some(codes::BUDGET_READ_ORGANIZATION_UNIT),
        Some(child),
    )
    .await;
    member(&pool, exact, child).await;
    member(&pool, exact, elsewhere).await;
    let tree = user(
        &pool,
        "Tree",
        Some(codes::BUDGET_READ_ORGANIZATION_TREE),
        Some(root),
    )
    .await;
    member(&pool, tree, root).await;
    let no_read = user(&pool, "No read", None, None).await;
    let wrong_position = user(&pool, "Wrong position", None, None).await;
    member(&pool, wrong_position, child).await;
    sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code) SELECT $1,id,'head' FROM permissions WHERE code=$2 ON CONFLICT DO NOTHING").bind(child).bind(codes::BUDGET_READ_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    let users = vec![
        owner,
        assigned,
        school,
        exact,
        tree,
        no_read,
        wrong_position,
    ];
    let permissions = ResourceAccessPermissions {
        own: Some(codes::BUDGET_READ_OWN),
        assigned: Some(codes::BUDGET_READ_ASSIGNED),
        school: Some(codes::BUDGET_READ_SCHOOL),
        organization_unit: Some(codes::BUDGET_READ_ORGANIZATION_UNIT),
        organization_tree: Some(codes::BUDGET_READ_ORGANIZATION_TREE),
    };
    let mut target = ResourceAccessTarget {
        owner_user_id: Some(owner),
        assigned_user_ids: vec![assigned],
        organization_unit_ids: vec![child],
    };
    let mut tx = pool.begin().await.unwrap();
    let eligible = eligible_active_staff_for_resource_on_connection(
        &mut tx,
        &users,
        &[codes::BUDGET_APPROVE_ASSIGNED],
        permissions,
        &target,
    )
    .await
    .unwrap();
    for id in [owner, assigned, school, exact, tree] {
        assert!(eligible.contains(&id));
    }
    assert!(!eligible.contains(&no_read));
    assert!(!eligible.contains(&wrong_position));
    target.organization_unit_ids = vec![elsewhere];
    let eligible = eligible_active_staff_for_resource_on_connection(
        &mut tx,
        &users,
        &[codes::BUDGET_APPROVE_ASSIGNED],
        permissions,
        &target,
    )
    .await
    .unwrap();
    assert!(
        !eligible.contains(&exact),
        "membership at another unit does not transfer its originating grant"
    );
    assert!(eligible.contains(&tree));
    tx.commit().await.unwrap();
}
