use super::*;
use school_academic_delivery::models::{
    versions::{DeliverySnapshot, DeliveryVersion},
    RemoveDeliveryDraftResourceRequest,
};
use school_academic_delivery::services::versions;
use serde_json::{json, Value};

async fn removal_request(pool: &PgPool, id: Uuid) -> RemoveDeliveryDraftResourceRequest {
    let revision = change_sets::get_change_set(pool, id).await.unwrap();
    let version = versions::get_version(pool, revision.target_delivery_version_id)
        .await
        .unwrap();
    RemoveDeliveryDraftResourceRequest {
        change_set_row_version: revision.row_version,
        target_delivery_version_row_version: version.row_version,
    }
}

async fn draft_version(pool: &PgPool, id: Uuid) -> DeliveryVersion {
    let revision = change_sets::get_change_set(pool, id).await.unwrap();
    versions::get_version(pool, revision.target_delivery_version_id)
        .await
        .unwrap()
}

async fn preserved_facts(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object(
        'published',(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM academic_delivery_versions v WHERE status='published'),
        'offerings',(SELECT jsonb_agg(to_jsonb(o) ORDER BY id) FROM learning_offerings o),
        'groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY id) FROM learning_groups g),
        'teachers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_teachers t),
        'students',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learning_group_students s),
        'tables',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_timetable_versions t),
        'blocks',(SELECT jsonb_agg(to_jsonb(b) ORDER BY id) FROM academic_timetable_blocks b),
        'attendance',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM attendance_sessions a),
        'courseResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_course_results r),
        'activityResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_activity_results r))")
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn delivery_draft_removal_migration_preserves_published_and_unexcluded_projection() {
    let pool = prepare_delivery_fixture("delivery_removal_migration", false).await;
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 141).await.unwrap();
    let context = operational_change_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "removal-migration",
    )
    .await;
    let before = preserved_facts(&pool).await;
    let projection: Value = sqlx::query_scalar("SELECT academic_delivery_revision_snapshot($1)")
        .bind(revision.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 142).await.unwrap();
    apply_migrations_through(&pool, 142).await.unwrap();
    assert_eq!(before, preserved_facts(&pool).await);
    let after: Value = sqlx::query_scalar("SELECT academic_delivery_revision_snapshot($1)")
        .bind(revision.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(projection, after);
}

#[tokio::test]
async fn delivery_draft_removal_is_local_persistent_and_clears_pending_commands() {
    let pool = prepare_delivery_runtime_fixture("delivery_removal_scope").await;
    let context = operational_change_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "removal-scope",
    )
    .await;
    let sibling = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "removal-sibling",
    )
    .await;
    let original = draft_version(&pool, revision.id).await;
    let offering = original
        .snapshot
        .offerings
        .iter()
        .find(|o| !o.groups.is_empty())
        .unwrap();
    let offering_id = offering.id;
    let group_id = offering.groups[0].id;
    let sibling_before = serde_json::to_value(draft_version(&pool, sibling.id).await).unwrap();
    let facts = preserved_facts(&pool).await;
    let new_teacher = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status)
        VALUES($1,$1::text,'not-a-login','Synthetic','Removal','staff','active')",
    )
    .bind(new_teacher)
    .execute(&pool)
    .await
    .unwrap();
    change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        revision.id,
        UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
            change_set_row_version: revision.row_version,
            item_row_version: None,
            learning_group_id: group_id,
            teacher_id: new_teacher,
            teacher_role: LearningTeacherRole::Secondary,
        },
    )
    .await
    .unwrap();
    let request = removal_request(&pool, revision.id).await;
    let removed =
        change_sets::remove_draft_group(&pool, context.teacher_id, revision.id, group_id, request)
            .await
            .unwrap();
    assert!(removed.items.is_empty());
    let version_after_remove = draft_version(&pool, revision.id).await.row_version;
    assert!(matches!(
        change_sets::upsert_change_item(
            &pool,
            context.teacher_id,
            revision.id,
            UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
                change_set_row_version: removed.row_version,
                item_row_version: None,
                learning_group_id: group_id,
                teacher_id: new_teacher,
                teacher_role: LearningTeacherRole::Secondary,
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        version_after_remove,
        draft_version(&pool, revision.id).await.row_version
    );
    assert!(removed
        .changes
        .iter()
        .any(|change| change.resource_id == group_id));
    assert_eq!(facts, preserved_facts(&pool).await);
    assert_eq!(
        sibling_before,
        serde_json::to_value(draft_version(&pool, sibling.id).await).unwrap()
    );
    let refreshed = change_sets::update_change_set(
        &pool,
        context.teacher_id,
        revision.id,
        UpdateAcademicTermChangeSetRequest {
            row_version: removed.row_version,
            reference_date: removed.reference_date,
            reason: "Unrelated draft edit".into(),
        },
    )
    .await
    .unwrap();
    let projected: sqlx::types::Json<DeliverySnapshot> =
        sqlx::query_scalar("SELECT academic_delivery_revision_snapshot($1,$2)")
            .bind(revision.id)
            .bind(refreshed.reference_date + Duration::days(1))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!projected
        .offerings
        .iter()
        .flat_map(|o| &o.groups)
        .any(|g| g.id == group_id));
    publication_preview(&pool, revision.id).await.unwrap();
    let current = draft_version(&pool, revision.id).await;
    assert!(!current
        .snapshot
        .offerings
        .iter()
        .flat_map(|o| &o.groups)
        .any(|g| g.id == group_id));
    change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        revision.id,
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            change_set_row_version: refreshed.row_version,
            item_row_version: None,
            learning_offering_id: offering_id,
            weekly_period_target: offering.weekly_period_target + 1,
        },
    )
    .await
    .unwrap();
    let request = removal_request(&pool, revision.id).await;
    let removed = change_sets::remove_draft_offering(
        &pool,
        context.teacher_id,
        revision.id,
        offering_id,
        request,
    )
    .await
    .unwrap();
    assert!(removed.items.is_empty());
    assert!(!draft_version(&pool, revision.id)
        .await
        .snapshot
        .offerings
        .iter()
        .any(|o| o.id == offering_id));
    assert_eq!(facts, preserved_facts(&pool).await);
    assert_eq!(
        sibling_before,
        serde_json::to_value(draft_version(&pool, sibling.id).await).unwrap()
    );
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_audit_events WHERE entity_id=$1
        AND event_code='academic_delivery_draft_resource.removed'",
    )
    .bind(revision.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2);
    // Repeated removal is a conflict and never replays a destructive mutation.
    assert!(matches!(
        change_sets::remove_draft_offering(
            &pool,
            context.teacher_id,
            revision.id,
            offering_id,
            removal_request(&pool, revision.id).await
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn delivery_draft_removal_deletes_exclusive_resources_and_retains_shared_drafts() {
    let pool = prepare_delivery_runtime_fixture("delivery_removal_exclusive").await;
    let context = planning_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "removal-exclusive",
    )
    .await;
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(revision.target_delivery_version_id),
        course_request(&context),
    )
    .await
    .unwrap();
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "Exclusive group".into(),
            description: None,
            capacity: None,
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    change_sets::remove_draft_group(
        &pool,
        context.teacher_id,
        revision.id,
        group.id,
        removal_request(&pool, revision.id).await,
    )
    .await
    .unwrap();
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM learning_groups WHERE id=$1)")
            .bind(group.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!exists);
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "Second exclusive group".into(),
            description: None,
            capacity: None,
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    // A sibling includes the same draft registry resource through the provider operation.
    let sibling = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "removal-shared",
    )
    .await;
    let mut tx = pool.begin().await.unwrap();
    // The same authoritative inclusion used by curriculum preparation is exercised by SQL
    // journal creation here because its provider helper is private to Delivery.
    sqlx::query("INSERT INTO academic_term_change_items(id,change_set_id,academic_term_id,academic_year_id,
        action_kind,learning_offering_id,weekly_period_target,created_by)
        SELECT gen_random_uuid(),$1,$2,$3,'add_offering',$4,periods_per_week,$5 FROM subject_versions WHERE id=$6")
        .bind(sibling.id).bind(context.term_id).bind(context.year_id).bind(offering.id)
        .bind(context.teacher_id).bind(context.subject_version_id).execute(&mut *tx).await.unwrap();
    sqlx::query(
        "UPDATE academic_delivery_versions SET snapshot=academic_delivery_revision_snapshot($1)
        WHERE id=$2",
    )
    .bind(sibling.id)
    .bind(sibling.target_delivery_version_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let sibling_before = serde_json::to_value(draft_version(&pool, sibling.id).await).unwrap();
    change_sets::remove_draft_offering(
        &pool,
        context.teacher_id,
        revision.id,
        offering.id,
        removal_request(&pool, revision.id).await,
    )
    .await
    .unwrap();
    assert_eq!(
        sibling_before,
        serde_json::to_value(draft_version(&pool, sibling.id).await).unwrap()
    );
    assert!(offerings::get(&pool, offering.id).await.is_ok());
    assert!(groups::get(&pool, group.id).await.is_ok());
    change_sets::remove_draft_offering(
        &pool,
        context.teacher_id,
        sibling.id,
        offering.id,
        removal_request(&pool, sibling.id).await,
    )
    .await
    .unwrap();
    // The first draft's exclusion contains no source membership and does not pin registry rows.
    assert!(matches!(
        offerings::get(&pool, offering.id).await,
        Err(AppError::NotFound(_))
    ));
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM subject_versions WHERE id=$1)")
            .bind(context.subject_version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(exists);
}

#[tokio::test]
async fn delivery_draft_removal_rejects_stale_versions_and_closed_lifecycle() {
    let pool = prepare_delivery_runtime_fixture("delivery_removal_guards").await;
    let context = operational_change_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "removal-guards",
    )
    .await;
    let version = draft_version(&pool, revision.id).await;
    let offering = &version.snapshot.offerings[0];
    let request = removal_request(&pool, revision.id).await;
    let before = preserved_facts(&pool).await;
    for stale in [
        RemoveDeliveryDraftResourceRequest {
            change_set_row_version: request.change_set_row_version + 1,
            target_delivery_version_row_version: request.target_delivery_version_row_version,
        },
        RemoveDeliveryDraftResourceRequest {
            change_set_row_version: request.change_set_row_version,
            target_delivery_version_row_version: request.target_delivery_version_row_version + 1,
        },
    ] {
        assert!(matches!(
            change_sets::remove_draft_offering(
                &pool,
                context.teacher_id,
                revision.id,
                offering.id,
                stale
            )
            .await,
            Err(AppError::Conflict(_))
        ));
    }
    assert_eq!(before, preserved_facts(&pool).await);
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=start_date WHERE id=$1")
        .bind(context.term_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        change_sets::remove_draft_offering(
            &pool,
            context.teacher_id,
            revision.id,
            offering.id,
            request
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        version.row_version,
        draft_version(&pool, revision.id).await.row_version
    );
}

#[tokio::test]
async fn delivery_draft_catalog_options_expose_authoritative_subject_type() {
    let pool = prepare_delivery_runtime_fixture("delivery_removal_catalog").await;
    let context = planning_runtime_context(&pool).await;
    let filter = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let options = workspaces::delivery_management_options(&pool, context.term_id, &filter, &filter)
        .await
        .unwrap();
    assert!(options
        .catalog_versions
        .iter()
        .any(|o| o.kind == LearningOfferingKind::Course));
    assert!(options
        .catalog_versions
        .iter()
        .any(|o| o.kind == LearningOfferingKind::Activity));
    for option in options.catalog_versions {
        let expected: Option<String> = if option.kind == LearningOfferingKind::Course {
            Some(
                sqlx::query_scalar("SELECT type FROM subject_versions WHERE id=$1")
                    .bind(option.id)
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        assert_eq!(option.subject_type, expected);
        assert!(serde_json::to_value(option)
            .unwrap()
            .get("subjectType")
            .is_some());
    }
}

fn delivery_router(pool: &PgPool, actor: Uuid, permissions: &[&str]) -> axum::Router {
    use school_auth::{
        config::SessionConfig, runtime::AuthRuntime, session_crypto::SessionHmacKey,
        session_service::AuthenticatedSession,
    };
    use school_authorization::PermissionCache;
    use school_tenancy::{AdminClient, AdminClientConfig, PoolManager, TenantContext};
    use std::sync::Arc;
    let permission_cache = Arc::new(PermissionCache::new());
    let revision = permission_cache.snapshot_revision("delivery-removal-http", actor);
    assert!(permission_cache.fill_if_current(
        "delivery-removal-http",
        actor,
        revision,
        permissions.iter().map(|p| p.to_string()).collect()
    ));
    let runtime = AuthRuntime {
        admin_client: Arc::new(AdminClient::new(
            "http://127.0.0.1:9".into(),
            "test-secret".into(),
            AdminClientConfig::from_env().unwrap(),
        )),
        pool_manager: Arc::new(PoolManager::new()),
        permission_cache,
        identity_cache: Arc::new(school_auth::session_cache::SessionCache::new()),
        config: Arc::new(SessionConfig::for_tests(SessionHmacKey::for_tests(
            [81; 32],
        ))),
        session_events: tokio::sync::broadcast::channel(8).0,
        permission_events: tokio::sync::broadcast::channel(8).0,
    };
    let session = AuthenticatedSession::for_tests(
        runtime.identity_cache.clone(),
        TenantContext {
            tenant_id: Uuid::new_v4(),
            subdomain: "delivery-removal-http".into(),
            pool: pool.clone().into(),
        },
        Uuid::new_v4(),
        actor,
        "staff",
    );
    let state = school_academic_http::state::AcademicHttpState {
        staff_directory: &crate::adapters::staff_directory::STAFF_DIRECTORY,
        placement_rosters: &crate::http_state::PLACEMENT_ROSTERS,
        auth_runtime: runtime,
        websocket_manager: Arc::new(school_academic_http::websockets::WebSocketManager::new()),
        result_locks: &crate::adapters::academic_result_locks::RESULT_LOCKS,
        lifecycle_providers: &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS,
    };
    axum::Router::new()
        .nest("/api/academic", school_academic_http::delivery::routes())
        .layer(axum::Extension(session))
        .with_state(state)
}

async fn http_remove(
    app: axum::Router,
    path: &str,
    request: &RemoveDeliveryDraftResourceRequest,
) -> (axum::http::StatusCode, Value) {
    use tower::ServiceExt;
    let response = app.oneshot(axum::http::Request::builder().method("DELETE").uri(path)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(json!({
            "changeSetRowVersion": request.change_set_row_version,
            "targetDeliveryVersionRowVersion": request.target_delivery_version_row_version,
        }).to_string())).unwrap()).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1000000)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn delivery_draft_removal_http_enforces_target_scope_and_selected_draft() {
    use axum::http::StatusCode;
    use school_permissions::registry::codes;
    let pool = prepare_delivery_runtime_fixture("delivery_removal_http").await;
    let context = operational_change_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "removal-http",
    )
    .await;
    let version = draft_version(&pool, revision.id).await;
    let offering = version
        .snapshot
        .offerings
        .iter()
        .find(|o| !o.groups.is_empty())
        .unwrap();
    let group = &offering.groups[0];
    let actor = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status)
        VALUES($1,$1::text,'not-a-login','Synthetic','Scoped actor','staff','active')",
    )
    .bind(actor)
    .execute(&pool)
    .await
    .unwrap();
    let offering_path = format!(
        "/api/academic/term-change-sets/{}/offerings/{}",
        revision.id, offering.id
    );
    let group_path = format!(
        "/api/academic/term-change-sets/{}/groups/{}",
        revision.id, group.id
    );
    let request = removal_request(&pool, revision.id).await;
    for path in [&offering_path, &group_path] {
        for permissions in [
            vec![],
            vec![codes::LEARNING_OFFERING_READ_SCHOOL],
            vec![codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT],
            vec![codes::LEARNING_OFFERING_MANAGE_ASSIGNED],
        ] {
            let (status, _) =
                http_remove(delivery_router(&pool, actor, &permissions), path, &request).await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "target management scope must be enforced: {permissions:?}"
            );
            assert_eq!(
                serde_json::to_value(&version).unwrap(),
                serde_json::to_value(draft_version(&pool, revision.id).await).unwrap()
            );
        }
    }
    let permissions = &[codes::LEARNING_OFFERING_MANAGE_SCHOOL];
    let wrong_offering: Uuid =
        sqlx::query_scalar("SELECT id FROM learning_offerings WHERE academic_term_id<>$1 LIMIT 1")
            .bind(context.term_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let wrong_path = format!(
        "/api/academic/term-change-sets/{}/offerings/{wrong_offering}",
        revision.id
    );
    assert_eq!(
        http_remove(
            delivery_router(&pool, actor, permissions),
            &wrong_path,
            &request
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut stale = removal_request(&pool, revision.id).await;
    stale.target_delivery_version_row_version += 1;
    assert_eq!(
        http_remove(
            delivery_router(&pool, actor, permissions),
            &offering_path,
            &stale
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        http_remove(
            delivery_router(&pool, actor, permissions),
            &group_path,
            &request
        )
        .await
        .0,
        StatusCode::OK
    );
    let current = removal_request(&pool, revision.id).await;
    let (status, data) = http_remove(
        delivery_router(&pool, actor, permissions),
        &offering_path,
        &current,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{data}");
    assert!(
        offerings::get(&pool, offering.id).await.is_ok(),
        "published registry survives HTTP removal"
    );
}

async fn seed_shared_draft_inclusion(
    pool: &PgPool,
    sibling: &super::super::models::AcademicTermChangeSet,
    context: &RuntimeContext,
    offering_id: Uuid,
) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO academic_term_change_items(id,change_set_id,academic_term_id,academic_year_id,
        action_kind,learning_offering_id,weekly_period_target,created_by)
        SELECT gen_random_uuid(),$1,$2,$3,'add_offering',$4,periods_per_week,$5 FROM subject_versions WHERE id=$6")
        .bind(sibling.id).bind(context.term_id).bind(context.year_id).bind(offering_id)
        .bind(context.teacher_id).bind(context.subject_version_id).execute(&mut *tx).await.unwrap();
    sqlx::query(
        "UPDATE academic_delivery_versions SET snapshot=academic_delivery_revision_snapshot($1)
        WHERE id=$2",
    )
    .bind(sibling.id)
    .bind(sibling.target_delivery_version_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn delivery_draft_removal_publication_only_promotes_groups_in_the_graph() {
    let pool = prepare_delivery_runtime_fixture("delivery_removal_publish_groups").await;
    let context = planning_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "removal-publish-groups",
    )
    .await;
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(revision.target_delivery_version_id),
        course_request(&context),
    )
    .await
    .unwrap();
    let kept = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "Published group".into(),
            description: None,
            capacity: None,
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    let kept = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        kept.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: kept.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();
    groups::replace_teachers(
        &pool,
        context.teacher_id,
        kept.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: kept.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let removed = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "Other draft group".into(),
            description: None,
            capacity: None,
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    let sibling = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "removal-publish-sibling",
    )
    .await;
    seed_shared_draft_inclusion(&pool, &sibling, &context, offering.id).await;
    change_sets::remove_draft_group(
        &pool,
        context.teacher_id,
        revision.id,
        removed.id,
        removal_request(&pool, revision.id).await,
    )
    .await
    .unwrap();
    let current = draft_version(&pool, revision.id).await;
    let facts = preserved_facts(&pool).await;
    // Direct apply recomputes membership without requiring a prior preview request.
    let hidden_bulk = school_academic_delivery::services::bulk_group_setup::apply(
        &pool,
        context.teacher_id,
        offering.id,
        school_academic_delivery::models::BulkGroupSetupRequest {
            delivery_version_id: current.id,
            delivery_version_row_version: current.row_version,
            groups: vec![school_academic_delivery::models::BulkGroupSetupTarget {
                learning_group_id: removed.id,
                row_version: removed.row_version,
            }],
            primary_teacher: Some(
                school_academic_delivery::models::BulkGroupSetupPrimaryTeacher {
                    teacher_id: context.teacher_id,
                    replace_existing_primary: false,
                },
            ),
            roster_tracking: None,
        },
    )
    .await;
    assert!(matches!(hidden_bulk, Err(AppError::ValidationError(_))));
    assert_eq!(facts, preserved_facts(&pool).await);
    assert_eq!(
        current.row_version,
        draft_version(&pool, revision.id).await.row_version
    );
    let preview = publication_preview(&pool, revision.id).await.unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|f| f.severity == AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        revision.id,
        PublishAcademicTermChangeSetRequest {
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            effective_from: preview.effective_from,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: vec![],
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        groups::get(&pool, kept.id).await.unwrap().status,
        LearningOfferingStatus::Published
    );
    assert_eq!(
        groups::get(&pool, removed.id).await.unwrap().status,
        LearningOfferingStatus::Draft
    );
    let published = draft_version(&pool, revision.id).await;
    assert!(!published
        .snapshot
        .offerings
        .iter()
        .flat_map(|o| &o.groups)
        .any(|g| g.id == removed.id));
    assert!(matches!(
        change_sets::remove_draft_offering(
            &pool,
            context.teacher_id,
            revision.id,
            offering.id,
            removal_request(&pool, revision.id).await
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn delivery_draft_removal_explicit_curriculum_inclusion_restores_only_the_offering() {
    let pool = prepare_delivery_runtime_fixture("delivery_removal_reinclude").await;
    let context = planning_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "removal-reinclude",
    )
    .await;
    let preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    let applied = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
            delivery_version_id: Some(revision.target_delivery_version_id),
            source_hash: preview.source_hash.clone(),
            choices: default_preparation_choices(&preview),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let graph = draft_version(&pool, revision.id).await;
    let offering = graph
        .snapshot
        .offerings
        .iter()
        .find(|o| applied.offering_ids.contains(&o.id) && !o.groups.is_empty())
        .unwrap();
    let group = offering.groups[0].id;
    change_sets::remove_draft_group(
        &pool,
        context.teacher_id,
        revision.id,
        group,
        removal_request(&pool, revision.id).await,
    )
    .await
    .unwrap();
    change_sets::remove_draft_offering(
        &pool,
        context.teacher_id,
        revision.id,
        offering.id,
        removal_request(&pool, revision.id).await,
    )
    .await
    .unwrap();
    assert!(
        offerings::get(&pool, offering.id).await.is_ok(),
        "preparation receipt preserves registry resource"
    );
    let preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
            delivery_version_id: Some(revision.target_delivery_version_id),
            source_hash: preview.source_hash.clone(),
            choices: default_preparation_choices(&preview),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let graph = draft_version(&pool, revision.id).await;
    let restored: Vec<_> = graph
        .snapshot
        .offerings
        .iter()
        .filter(|o| o.id == offering.id)
        .collect();
    assert_eq!(restored.len(), 1);
    assert!(!restored[0].groups.iter().any(|g| g.id == group));
}

#[tokio::test]
async fn delivery_draft_removal_handles_synchronized_activity_groups_and_offerings() {
    let pool = prepare_delivery_runtime_fixture("delivery_removal_activity").await;
    let context = operational_change_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "removal-activity",
    )
    .await;
    let activity_version_id: Uuid = sqlx::query_scalar(
        "SELECT version.id FROM activity_versions version
        JOIN academic_terms term ON term.id=$1 WHERE version.status='published'
        AND version.scheduling_mode='synchronized' AND version.effective_from<=term.start_date
        ORDER BY version.id LIMIT 1",
    )
    .bind(context.term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let scheduling_mode = ActivitySchedulingMode::Synchronized;
    let original_ids = draft_version(&pool, revision.id)
        .await
        .snapshot
        .offerings
        .into_iter()
        .map(|o| o.id)
        .collect::<Vec<_>>();
    let current = change_sets::get_change_set(&pool, revision.id)
        .await
        .unwrap();
    change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        revision.id,
        UpsertAcademicTermChangeItemRequest::AddActivity {
            change_set_row_version: current.row_version,
            weekly_period_target: 2,
            offering: CreateActivityOfferingRequest {
                academic_term_id: context.term_id,
                activity_version_id,
                curriculum_activity_requirement_id: None,
                create_homeroom_groups: false,
                targets: vec![OfferingTargetInput {
                    target_kind: OfferingTargetKind::Homeroom,
                    homeroom_id: Some(context.homeroom_id),
                    grade_level_id: context.grade_level_id,
                    study_program_id: context.study_program_id,
                }],
                registration_type: ActivityRegistrationType::Assigned,
                scheduling_mode,
                capacity: None,
                attendance_requirement: ActivityAttendanceRequirement {
                    minimum_percent: None,
                    required_sessions: None,
                },
                pass_criteria: ActivityPassCriteria {
                    require_attendance: false,
                    require_teacher_confirmation: true,
                    outcomes: vec!["pass".into(), "fail".into()],
                },
            },
        },
    )
    .await
    .unwrap();
    let graph = draft_version(&pool, revision.id).await;
    let offering = graph
        .snapshot
        .offerings
        .iter()
        .find(|o| {
            !original_ids.contains(&o.id)
                && matches!(&o.catalog,
        LearningOfferingSnapshot::Activity(a) if a.activity_version_id == activity_version_id)
        })
        .unwrap();
    let group = offering.groups[0].id;
    change_sets::remove_draft_group(
        &pool,
        context.teacher_id,
        revision.id,
        group,
        removal_request(&pool, revision.id).await,
    )
    .await
    .unwrap();
    assert!(matches!(
        groups::get(&pool, group).await,
        Err(AppError::NotFound(_))
    ));
    change_sets::remove_draft_offering(
        &pool,
        context.teacher_id,
        revision.id,
        offering.id,
        removal_request(&pool, revision.id).await,
    )
    .await
    .unwrap();
    assert!(matches!(
        offerings::get(&pool, offering.id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM activity_versions WHERE id=$1)"
    )
    .bind(activity_version_id)
    .fetch_one(&pool)
    .await
    .unwrap());
}
