use super::*;

const RETIRED: [&str; 2] = [
    "academic_timetable.publish.school",
    "academic_exam_schedule.publish.school",
];

async fn grants(pool: &PgPool) -> Value {
    sqlx::query_scalar(
        r#"SELECT jsonb_build_object(
            'roles',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM roles r),
            'rolePermissions',(SELECT jsonb_agg(to_jsonb(r) ORDER BY role_id,permission_id) FROM role_permissions r),
            'userRoles',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM user_roles r),
            'members',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM organization_members r),
            'organizationGrants',(SELECT jsonb_agg(to_jsonb(r) ORDER BY organization_unit_id,permission_id,position_code) FROM organization_permission_grants r),
            'delegations',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM organization_permission_delegations r))"#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn retired_publication_grants_remain_inactive_without_management_conversion_or_receipt_changes(
) {
    let pool = predecessor("current_ops_retired_permissions").await;
    for (module, code) in [
        ("academic_timetable", RETIRED[0]),
        ("academic_exam_schedule", RETIRED[1]),
    ] {
        sqlx::query("INSERT INTO permissions(code,name,module,action,scope,is_active) VALUES($1,$1,$2,'publish','school',true) ON CONFLICT(code) DO UPDATE SET is_active=true")
            .bind(code).bind(module).execute(&pool).await.unwrap();
    }
    let users: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE status='active' ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(users.len(), 2);
    let role: Uuid = sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES('CURRENT_LEGACY_PUBLICATION','Retained legacy grants','staff') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let unit: Uuid = sqlx::query_scalar("INSERT INTO organization_units(code,name) VALUES('CURRENT_LEGACY_PUBLICATION','Retained legacy grants') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)")
        .bind(users[0])
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES($1,$2,'member')")
        .bind(users[0]).bind(unit).execute(&pool).await.unwrap();
    let codes = [RETIRED[0], RETIRED[1], "academic_curriculum.manage.school"];
    assert_eq!(sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=ANY($2)")
        .bind(role).bind(&codes).execute(&pool).await.unwrap().rows_affected(), 3);
    assert_eq!(sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code,created_by) SELECT $1,id,'member',$3 FROM permissions WHERE code=ANY($2)")
        .bind(unit).bind(&codes).bind(users[1]).execute(&pool).await.unwrap().rows_affected(), 3);
    assert_eq!(sqlx::query("INSERT INTO organization_permission_delegations(from_user_id,to_user_id,permission_id,organization_unit_id,reason) SELECT $1,$2,id,$3,'Retained legacy grant' FROM permissions WHERE code=ANY($4)")
        .bind(users[1]).bind(users[0]).bind(unit).bind(&codes).execute(&pool).await.unwrap().rows_affected(), 3);
    let grants_before = grants(&pool).await;
    let unrelated: Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM permissions p WHERE NOT(code=ANY($1))",
    )
    .bind(&RETIRED)
    .fetch_one(&pool)
    .await
    .unwrap();
    let retired_metadata: Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p)-'is_active' ORDER BY id) FROM permissions p WHERE code=ANY($1)")
        .bind(&RETIRED).fetch_one(&pool).await.unwrap();

    // The forward SQL itself must retire the rights, before canonical sync runs.
    apply_migrations_through(&pool, 157).await.unwrap();
    assert_eq!(
        unrelated,
        sqlx::query_scalar::<_, Value>(
            "SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM permissions p WHERE NOT(code=ANY($1))"
        )
        .bind(&RETIRED)
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    assert_eq!(retired_metadata, sqlx::query_scalar::<_, Value>("SELECT jsonb_agg(to_jsonb(p)-'is_active' ORDER BY id) FROM permissions p WHERE code=ANY($1)")
        .bind(&RETIRED).fetch_one(&pool).await.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM permissions WHERE code=ANY($1) AND NOT is_active"
        )
        .bind(&RETIRED)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        grants_before,
        grants(&pool).await,
        "all role, organization and delegation grants retain their exact identities; none becomes Manage"
    );
    let receipt: Value = sqlx::query_scalar("SELECT to_jsonb(r) FROM academic_current_operations_migration_audit r WHERE migration_version=157")
        .fetch_one(&pool).await.unwrap();
    let actual_codes: Vec<&str> = receipt["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|check| {
            assert_eq!(check["passed"], true);
            assert_eq!(check["mismatchCount"], 0);
            check["code"].as_str().unwrap()
        })
        .collect();
    let mut expected_codes = vec![
        "CURRENT_DELIVERY_GRAPH_RECONCILED",
        "CURRENT_TEACHERS_RECONCILED",
        "CURRENT_TIMETABLE_IDS_RECONCILED",
        "CURRENT_FACTS_PRESERVED",
        "CURRENT_SUPERVISION_PRESERVED",
        "CURRENT_CATALOG_PINS_PRESERVED",
        "CURRENT_LEGACY_SCHEMA_REMOVED",
        "CURRENT_SCOPE_GUARDS_ENABLED",
    ];
    expected_codes.sort_unstable();
    assert_eq!(actual_codes, expected_codes);
    assert_eq!(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM permissions p WHERE p.code=ANY(ARRAY['academic_timetable.manage.school','academic_exam_schedule.manage.school']) AND (EXISTS(SELECT 1 FROM role_permissions rp WHERE rp.role_id=$1 AND rp.permission_id=p.id) OR EXISTS(SELECT 1 FROM organization_permission_grants g WHERE g.organization_unit_id=$2 AND g.permission_id=p.id) OR EXISTS(SELECT 1 FROM organization_permission_delegations d WHERE d.to_user_id=$3 AND d.organization_unit_id=$2 AND d.permission_id=p.id))"
    ).bind(role).bind(unit).bind(users[0]).fetch_one(&pool).await.unwrap(), 0);

    // Canonical upserts legitimately update updated_at on registry entries.
    // All grants and the immutable cutover receipt remain byte-exact on replay.
    let mut canonical_metadata = None;
    for _ in 0..2 {
        school_migrations::sync_permissions(&pool).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM permissions WHERE code=ANY($1) AND NOT is_active"
            )
            .bind(&RETIRED)
            .fetch_one(&pool)
            .await
            .unwrap(),
            2
        );
        assert_eq!(grants_before, grants(&pool).await);
        assert_eq!(retired_metadata, sqlx::query_scalar::<_, Value>("SELECT jsonb_agg(to_jsonb(p)-'is_active' ORDER BY id) FROM permissions p WHERE code=ANY($1)")
            .bind(&RETIRED).fetch_one(&pool).await.unwrap());
        let metadata: Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p)-'updated_at' ORDER BY id) FROM permissions p WHERE NOT(code=ANY($1))")
            .bind(&RETIRED).fetch_one(&pool).await.unwrap();
        if let Some(previous) = &canonical_metadata {
            assert_eq!(previous, &metadata);
        }
        canonical_metadata = Some(metadata);
        assert_eq!(receipt, sqlx::query_scalar::<_, Value>("SELECT to_jsonb(r) FROM academic_current_operations_migration_audit r WHERE migration_version=157")
            .fetch_one(&pool).await.unwrap());
    }
}
