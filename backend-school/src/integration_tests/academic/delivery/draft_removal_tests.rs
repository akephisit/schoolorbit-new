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
    let revision = super::room_scope_tests::historical_revision(&pool, &context, 12).await;
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
    // Both explicit empty audience parents retain the offering identity for dated eligibility.
    assert!(offerings::get(&pool, offering.id).await.is_ok());
    let empty_parents:i64=sqlx::query_scalar("SELECT count(*) FROM academic_delivery_revision_audiences a WHERE learning_offering_id=$1 AND NOT EXISTS(SELECT 1 FROM academic_delivery_revision_audience_rooms r WHERE r.change_set_id=a.change_set_id AND r.learning_offering_id=a.learning_offering_id)").bind(offering.id).fetch_one(&pool).await.unwrap();
    assert_eq!(empty_parents, 2);
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
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
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

pub(super) fn delivery_router(pool: &PgPool, actor: Uuid, permissions: &[&str]) -> axum::Router {
    academic_router(
        pool,
        actor,
        permissions,
        school_academic_http::delivery::routes(),
    )
}

fn academic_router(
    pool: &PgPool,
    actor: Uuid,
    permissions: &[&str],
    routes: axum::Router<school_academic_http::state::AcademicHttpState>,
) -> axum::Router {
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
    let websocket_manager = Arc::new(school_academic_http::websockets::WebSocketManager::new());
    let state = school_academic_http::state::AcademicHttpState {
        staff_directory: &crate::adapters::staff_directory::STAFF_DIRECTORY,
        placement_rosters: &crate::http_state::PLACEMENT_ROSTERS,
        auth_runtime: runtime,
        websocket_manager: websocket_manager.clone(),
        collaboration: websocket_manager.collaboration(),
        result_locks: &crate::adapters::academic_result_locks::RESULT_LOCKS,
        assessment_publication: &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
        lifecycle_providers: &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS,
    };
    axum::Router::new()
        .nest("/api/academic", routes)
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
        &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
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
            starts_on: chrono::Utc::now()
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive(),
            delivery_version_id: Some(revision.target_delivery_version_id),
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
            starts_on: chrono::Utc::now()
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive(),
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
    if draft_version(&pool, revision.id)
        .await
        .snapshot
        .offerings
        .iter()
        .any(|o| o.id == offering.id)
    {
        change_sets::remove_draft_offering(
            &pool,
            context.teacher_id,
            revision.id,
            offering.id,
            removal_request(&pool, revision.id).await,
        )
        .await
        .unwrap();
    }
    assert!(
        offerings::get(&pool, offering.id).await.is_ok(),
        "explicit audience intent and preparation receipt preserve registry resource"
    );
    let preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            starts_on: chrono::Utc::now()
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive(),
            delivery_version_id: Some(revision.target_delivery_version_id),
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
            starts_on: chrono::Utc::now()
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive(),
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
    assert!(
        restored[0].groups.iter().any(|g| g.id == group),
        "explicit reviewed re-add restores its current-curriculum group only"
    );
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
        AND version.scheduling_mode='synchronized' AND version.effective_from<=(now() AT TIME ZONE 'Asia/Bangkok')::date
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
                starts_on: chrono::Utc::now()
                    .with_timezone(&chrono_tz::Asia::Bangkok)
                    .date_naive(),
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
    assert!(
        offerings::get(&pool, offering.id).await.is_ok(),
        "empty scope pins the activity identity"
    );
    assert!(draft_version(&pool, revision.id)
        .await
        .snapshot
        .offerings
        .iter()
        .all(|o| o.id != offering.id));
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM activity_versions WHERE id=$1)"
    )
    .bind(activity_version_id)
    .fetch_one(&pool)
    .await
    .unwrap());
}

#[tokio::test]
async fn activity_zero_targets_http_preserve_scope_revisions_and_negative_validation() {
    use axum::http::StatusCode;
    use school_permissions::registry::codes;
    use tower::ServiceExt;
    let pool = prepare_delivery_runtime_fixture("activity_zero_http").await;
    apply_migrations_through(&pool, 143).await.unwrap();
    let context = operational_change_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "activity-zero-http",
    )
    .await;
    // Open draft activities through the public operation (which owns inclusion),
    // since the historical fixture has activities without legacy timetable lessons.
    for mode in [
        ActivitySchedulingMode::Independent,
        ActivitySchedulingMode::Synchronized,
    ] {
        let opened = open_activity_with_catalog_target(
            &pool,
            &context,
            revision.target_delivery_version_id,
            mode,
            false,
        )
        .await;
        assert_eq!(
            draft_version(&pool, revision.id)
                .await
                .snapshot
                .offerings
                .iter()
                .find(|offering| offering.id == opened.id)
                .unwrap()
                .weekly_period_target,
            1
        );
    }
    let initial = draft_version(&pool, revision.id).await;
    for mode in [
        ActivitySchedulingMode::Independent,
        ActivitySchedulingMode::Synchronized,
    ] {
        let offering = initial
            .snapshot
            .offerings
            .iter()
            .find(|offering| {
                matches!(&offering.catalog,
            LearningOfferingSnapshot::Activity(activity) if activity.scheduling_mode == mode)
            })
            .unwrap();
        let current = change_sets::get_change_set(&pool, revision.id)
            .await
            .unwrap();
        let path = format!("/api/academic/term-change-sets/{}/items", revision.id);
        for (permissions, target, row_version, expected) in [
            (
                vec![codes::LEARNING_OFFERING_READ_SCHOOL],
                0,
                current.row_version,
                StatusCode::FORBIDDEN,
            ),
            (
                vec![codes::LEARNING_OFFERING_MANAGE_SCHOOL],
                -1,
                current.row_version,
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                vec![codes::LEARNING_OFFERING_MANAGE_SCHOOL],
                0,
                current.row_version + 1,
                StatusCode::CONFLICT,
            ),
            (
                vec![codes::LEARNING_OFFERING_MANAGE_SCHOOL],
                0,
                current.row_version,
                StatusCode::OK,
            ),
        ] {
            let before = draft_version(&pool, revision.id).await;
            let request = json!({
                "action": "adjust_weekly_period_target", "changeSetRowVersion": row_version,
                "itemRowVersion": null, "learningOfferingId": offering.id, "weeklyPeriodTarget": target,
            });
            let _: UpsertAcademicTermChangeItemRequest =
                serde_json::from_value(request.clone()).unwrap();
            let response = delivery_router(&pool, context.teacher_id, &permissions)
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri(&path)
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&request).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let body = axum::body::to_bytes(response.into_body(), 1_000_000)
                .await
                .unwrap();
            assert_eq!(
                status,
                expected,
                "{mode:?}: {}",
                String::from_utf8_lossy(&body)
            );
            let after = draft_version(&pool, revision.id).await;
            if expected == StatusCode::OK {
                let changed = after
                    .snapshot
                    .offerings
                    .iter()
                    .find(|row| row.id == offering.id)
                    .unwrap();
                assert_eq!(changed.weekly_period_target, 0);
                assert_eq!(
                    serde_json::to_value(&changed.groups).unwrap(),
                    serde_json::to_value(&offering.groups).unwrap()
                );
                assert_eq!(
                    serde_json::to_value(&changed.catalog).unwrap(),
                    serde_json::to_value(&offering.catalog).unwrap()
                );
            } else {
                assert_eq!(
                    serde_json::to_value(before).unwrap(),
                    serde_json::to_value(after).unwrap()
                );
            }
        }
        let adjusted = change_sets::get_change_set(&pool, revision.id)
            .await
            .unwrap();
        let (item_id, item_version) = adjusted.items.iter().find_map(|item| match item {
            school_academic_delivery::models::AcademicTermChangeItem::AdjustWeeklyPeriodTarget { id, row_version, learning_offering_id, .. } if *learning_offering_id == offering.id => Some((*id,*row_version)),
            _ => None,
        }).unwrap();
        let request = json!({"action":"adjust_weekly_period_target", "changeSetRowVersion": adjusted.row_version,
            "itemRowVersion":item_version, "learningOfferingId":offering.id, "weeklyPeriodTarget":2});
        let _: UpsertAcademicTermChangeItemRequest =
            serde_json::from_value(request.clone()).unwrap();
        let response = delivery_router(
            &pool,
            context.teacher_id,
            &[codes::LEARNING_OFFERING_MANAGE_SCHOOL],
        )
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(&path)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            draft_version(&pool, revision.id)
                .await
                .snapshot
                .offerings
                .iter()
                .find(|row| row.id == offering.id)
                .unwrap()
                .weekly_period_target,
            2
        );
        let current = change_sets::get_change_set(&pool, revision.id)
            .await
            .unwrap();
        let request =
            json!({"changeSetRowVersion":current.row_version,"itemRowVersion":item_version+1});
        let _: DeleteAcademicTermChangeItemRequest =
            serde_json::from_value(request.clone()).unwrap();
        let response = delivery_router(
            &pool,
            context.teacher_id,
            &[codes::LEARNING_OFFERING_MANAGE_SCHOOL],
        )
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("{path}/{item_id}"))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            serde_json::to_value(&draft_version(&pool, revision.id).await.snapshot).unwrap(),
            serde_json::to_value(&initial.snapshot).unwrap(),
            "deleting the adjustment restores Add1 without changing any other graph fact"
        );
    }
    // AddActivity has an explicit actual target, independent of its positive catalog default.
    let (_, activity_version_id, scheduling_mode) =
        add_operational_change_catalog_fixture(&pool, &context).await;
    let catalog_target: i32 =
        sqlx::query_scalar("SELECT periods_per_week FROM activity_versions WHERE id=$1")
            .bind(activity_version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(catalog_target > 0);
    let current = change_sets::get_change_set(&pool, revision.id)
        .await
        .unwrap();
    for (target, expected) in [(-1, StatusCode::UNPROCESSABLE_ENTITY), (0, StatusCode::OK)] {
        let request = json!({
            "action": "add_activity", "changeSetRowVersion": current.row_version, "weeklyPeriodTarget": target,
            "offering": {
                "createHomeroomGroups": false, "academicTermId": context.term_id, "activityVersionId": activity_version_id,
                "curriculumActivityRequirementId": null,
                "targets": [{"targetKind": "homeroom", "homeroomId": context.homeroom_id,
                    "gradeLevelId": context.grade_level_id, "studyProgramId": context.study_program_id}],
                "registrationType": "assigned", "schedulingMode": scheduling_mode, "capacity": null,
                "attendanceRequirement": {"minimumPercent": null, "requiredSessions": null},
                "passCriteria": {"requireAttendance": false, "requireTeacherConfirmation": true, "outcomes": ["pass", "fail"]},
            },
        });
        let _: UpsertAcademicTermChangeItemRequest =
            serde_json::from_value(request.clone()).unwrap();
        let before = draft_version(&pool, revision.id).await;
        let response = delivery_router(
            &pool,
            context.teacher_id,
            &[codes::LEARNING_OFFERING_MANAGE_SCHOOL],
        )
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/academic/term-change-sets/{}/items",
                    revision.id
                ))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .unwrap();
        assert_eq!(status, expected, "{}", String::from_utf8_lossy(&body));
        let after = draft_version(&pool, revision.id).await;
        if expected == StatusCode::OK {
            let added = after.snapshot.offerings.iter().find(|offering| matches!(&offering.catalog,
                LearningOfferingSnapshot::Activity(activity) if activity.activity_version_id == activity_version_id)).unwrap();
            assert_eq!(added.weekly_period_target, 0);
            assert_eq!(
                after.snapshot.offerings.len(),
                before.snapshot.offerings.len() + 1
            );
        } else {
            assert_eq!(
                serde_json::to_value(before).unwrap(),
                serde_json::to_value(after).unwrap()
            );
        }
    }
}

pub(super) async fn open_activity_with_catalog_target(
    pool: &PgPool,
    context: &RuntimeContext,
    target_id: Uuid,
    mode: ActivitySchedulingMode,
    create_groups: bool,
) -> school_academic_delivery::models::LearningOffering {
    let activity_version_id: Uuid = sqlx::query_scalar("SELECT activity.id FROM activity_versions activity JOIN academic_terms term ON term.id=$1 WHERE activity.status='published' AND activity.scheduling_mode=$2 AND activity.effective_from<=(now() AT TIME ZONE 'Asia/Bangkok')::date ORDER BY activity.id LIMIT 1")
        .bind(context.term_id).bind(mode).fetch_one(pool).await.unwrap();
    offerings::create_for_delivery(
        pool,
        context.teacher_id,
        Some(target_id),
        CreateLearningOfferingRequest::Activity(CreateActivityOfferingRequest {
            starts_on: chrono::Utc::now()
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive(),
            create_homeroom_groups: create_groups,
            academic_term_id: context.term_id,
            activity_version_id,
            curriculum_activity_requirement_id: None,
            targets: vec![OfferingTargetInput {
                target_kind: OfferingTargetKind::Homeroom,
                homeroom_id: Some(context.homeroom_id),
                grade_level_id: context.grade_level_id,
                study_program_id: context.study_program_id,
            }],
            registration_type: ActivityRegistrationType::Assigned,
            scheduling_mode: mode,
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
        }),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn added_activity_target_migration_reconciles_only_cached_weekly_fields_and_revisions() {
    let pool = prepare_delivery_runtime_fixture("activity_target_migration").await;
    let context = operational_change_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "activity-target-migration",
    )
    .await;
    let sibling = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        13,
        "activity-target-unchanged",
    )
    .await;
    let initial = draft_version(&pool, revision.id).await;
    let excluded_group_id = initial
        .snapshot
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .next()
        .unwrap()
        .id;
    change_sets::remove_draft_group(
        &pool,
        context.teacher_id,
        revision.id,
        excluded_group_id,
        removal_request(&pool, revision.id).await,
    )
    .await
    .unwrap();
    let mut targets = Vec::new();
    for mode in [
        ActivitySchedulingMode::Independent,
        ActivitySchedulingMode::Synchronized,
    ] {
        for target in [0, 2] {
            let opened = open_activity_with_catalog_target(
                &pool,
                &context,
                revision.target_delivery_version_id,
                mode,
                true,
            )
            .await;
            for group in groups::list(&pool, opened.id).await.unwrap() {
                groups::replace_teachers(
                    &pool,
                    context.teacher_id,
                    group.id,
                    ReplaceLearningGroupTeachersRequest {
                        row_version: group.row_version,
                        teachers: vec![TeacherAssignmentInput {
                            teacher_id: context.teacher_id,
                            role: LearningTeacherRole::Primary,
                        }],
                    },
                )
                .await
                .unwrap();
            }
            let current = change_sets::get_change_set(&pool, revision.id)
                .await
                .unwrap();
            change_sets::upsert_change_item(
                &pool,
                context.teacher_id,
                revision.id,
                UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
                    change_set_row_version: current.row_version,
                    item_row_version: None,
                    learning_offering_id: opened.id,
                    weekly_period_target: target,
                },
            )
            .await
            .unwrap();
            targets.push((opened.id, target));
        }
    }
    // A terminal revision can contain the same old ignored command; never repair its recorded graph.
    let cancelled = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        14,
        "activity-target-cancelled",
    )
    .await;
    let terminal_activity = open_activity_with_catalog_target(
        &pool,
        &context,
        cancelled.target_delivery_version_id,
        ActivitySchedulingMode::Synchronized,
        false,
    )
    .await;
    let terminal_current = change_sets::get_change_set(&pool, cancelled.id)
        .await
        .unwrap();
    change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        cancelled.id,
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            change_set_row_version: terminal_current.row_version,
            item_row_version: None,
            learning_offering_id: terminal_activity.id,
            weekly_period_target: 2,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE academic_delivery_versions SET status='cancelled' WHERE id=$1")
        .bind(cancelled.target_delivery_version_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_term_change_sets SET status='cancelled',cancelled_by=$2,cancelled_at=now() WHERE id=$1").bind(cancelled.id).bind(context.teacher_id).execute(&pool).await.unwrap();
    let terminal_before:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(version) ORDER BY id) FROM academic_delivery_versions version WHERE status<>'draft'").fetch_one(&pool).await.unwrap();
    let parent_before: Value = sqlx::query_scalar(
        "SELECT to_jsonb(revision) FROM academic_term_change_sets revision WHERE id=$1",
    )
    .bind(revision.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let cached = draft_version(&pool, revision.id).await;
    for (id, _) in &targets {
        assert_eq!(
            cached
                .snapshot
                .offerings
                .iter()
                .find(|row| row.id == *id)
                .unwrap()
                .weekly_period_target,
            1,
            "pre143 reproduces ignored adjustments"
        );
    }
    let revision_before = change_sets::get_change_set(&pool, revision.id)
        .await
        .unwrap();
    let sibling_before = draft_version(&pool, sibling.id).await;
    let sibling_revision_before = change_sets::get_change_set(&pool, sibling.id)
        .await
        .unwrap();
    let history_before = preserved_facts(&pool).await;
    let journal_before: Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(item) ORDER BY id) FROM academic_term_change_items item",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let cached_record: Value = sqlx::query_scalar(
        "SELECT to_jsonb(version) FROM academic_delivery_versions version WHERE id=$1",
    )
    .bind(cached.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut expected = cached_record["snapshot"].clone();
    for offering in expected["offerings"].as_array_mut().unwrap() {
        for (id, target) in &targets {
            if offering["id"] == json!(id) {
                offering["weeklyPeriodTarget"] = json!(target);
            }
        }
    }
    apply_migrations_through(&pool, 143).await.unwrap();
    let repaired = draft_version(&pool, revision.id).await;
    let current = change_sets::get_change_set(&pool, revision.id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&repaired.snapshot).unwrap(),
        expected,
        "only weekly scalars change; all ordered graph facts and exclusions remain"
    );
    assert_eq!(repaired.row_version, cached.row_version + 1);
    assert_eq!(current.row_version, revision_before.row_version + 1);
    let mut expected_version = cached_record;
    let repaired_record: Value = sqlx::query_scalar(
        "SELECT to_jsonb(version) FROM academic_delivery_versions version WHERE id=$1",
    )
    .bind(cached.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for field in ["row_version", "updated_at"] {
        expected_version[field] = repaired_record[field].clone();
    }
    expected_version["snapshot"] = expected.clone();
    assert_eq!(
        expected_version, repaired_record,
        "only the exact raw weekly fields and revision guard/timestamp change"
    );
    let parent_after: Value = sqlx::query_scalar(
        "SELECT to_jsonb(revision) FROM academic_term_change_sets revision WHERE id=$1",
    )
    .bind(revision.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut expected_parent = parent_before;
    for field in ["row_version", "updated_at"] {
        expected_parent[field] = parent_after[field].clone();
    }
    assert_eq!(
        expected_parent, parent_after,
        "no parent metadata changes beyond the revision guard and timestamp"
    );
    assert_eq!(terminal_before,sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(version) ORDER BY id) FROM academic_delivery_versions version WHERE status<>'draft'").fetch_one(&pool).await.unwrap());
    assert_eq!(
        serde_json::to_value(sibling_before).unwrap(),
        serde_json::to_value(draft_version(&pool, sibling.id).await).unwrap()
    );
    assert_eq!(
        serde_json::to_value(sibling_revision_before).unwrap(),
        serde_json::to_value(
            change_sets::get_change_set(&pool, sibling.id)
                .await
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        history_before,
        preserved_facts(&pool).await,
        "published bytes/receipts and all downstream facts remain unchanged"
    );
    assert_eq!(
        journal_before,
        sqlx::query_scalar::<_, Value>(
            "SELECT jsonb_agg(to_jsonb(item) ORDER BY id) FROM academic_term_change_items item"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    let projection: Value = sqlx::query_scalar("SELECT academic_delivery_revision_snapshot($1,$2)")
        .bind(revision.id)
        .bind(current.reference_date)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(projection, expected);
    let preview = publication_preview(&pool, revision.id).await.unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|finding| finding.code == AcademicChangeFindingCode::ResourceStale),
        "{:?}",
        preview.findings
    );
    use tower::ServiceExt;
    let stale = json!({"action":"adjust_weekly_period_target","changeSetRowVersion":revision_before.row_version,"itemRowVersion":1,"learningOfferingId":targets[0].0,"weeklyPeriodTarget":2});
    let response = delivery_router(
        &pool,
        context.teacher_id,
        &[school_permissions::registry::codes::LEARNING_OFFERING_MANAGE_SCHOOL],
    )
    .oneshot(
        axum::http::Request::builder()
            .method("PUT")
            .uri(format!(
                "/api/academic/term-change-sets/{}/items",
                revision.id
            ))
            .header("content-type", "application/json")
            .body(axum::body::Body::from(stale.to_string()))
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    let stale_target = change_sets::remove_draft_offering(
        &pool,
        context.teacher_id,
        revision.id,
        targets[0].0,
        RemoveDeliveryDraftResourceRequest {
            change_set_row_version: current.row_version,
            target_delivery_version_row_version: cached.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(stale_target, AppError::Conflict(_)));
    sqlx::raw_sql(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/143_delivery_added_target_adjustments.sql"
    )))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(&repaired).unwrap(),
        serde_json::to_value(draft_version(&pool, revision.id).await).unwrap(),
        "reconciliation retry is a no-op"
    );
    assert_eq!(
        serde_json::to_value(&current).unwrap(),
        serde_json::to_value(
            change_sets::get_change_set(&pool, revision.id)
                .await
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(history_before, preserved_facts(&pool).await);
}

async fn pending_removal_facts(pool: &PgPool) -> Value {
    json!({
        "preserved": preserved_facts(pool).await,
        "commands": sqlx::query_scalar::<_, Value>("SELECT jsonb_build_object(
            'revisions',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_term_change_sets r),
            'versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM academic_delivery_versions v),
            'items',(SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM academic_term_change_items i),
            'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM academic_audit_events a),
            'plans',(SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM course_assessment_plans p),
            'scores',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learning_group_student_scores s))").fetch_one(pool).await.unwrap(),
    })
}

#[tokio::test]
async fn delivery_draft_removal_http_pending_add_with_three_groups() {
    http_pending_three_group_removal("add").await;
}

#[tokio::test]
async fn delivery_draft_removal_http_pending_period_with_three_groups() {
    http_pending_three_group_removal("period").await;
}

#[tokio::test]
async fn delivery_draft_removal_http_pending_teacher_with_three_groups() {
    http_pending_three_group_removal("teacher").await;
}

async fn http_pending_three_group_removal(command: &str) {
    use axum::http::StatusCode;
    use school_permissions::registry::codes;
    let pool = prepare_delivery_runtime_fixture(&format!("removal_http_pending_{command}")).await;
    let context = planning_runtime_context(&pool).await;
    let initial = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "pending-removal-initial",
    )
    .await;
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(initial.target_delivery_version_id),
        course_request(&context),
    )
    .await
    .unwrap();
    let mut course_groups = groups::list(&pool, offering.id).await.unwrap();
    while course_groups.len() < 3 {
        let group = groups::create(
            &pool,
            context.teacher_id,
            offering.id,
            CreateLearningGroupRequest {
                name: format!("กลุ่มเรียนที่ {}", course_groups.len() + 1),
                description: None,
                capacity: None,
                preferred_room_ids: vec![],
            },
        )
        .await
        .unwrap();
        course_groups.push(group);
    }
    assert_eq!(course_groups.len(), 3);
    for group in &course_groups {
        let current = groups::get(&pool, group.id).await.unwrap();
        groups::replace_homerooms(
            &pool,
            context.teacher_id,
            group.id,
            ReplaceLearningGroupHomeroomsRequest {
                row_version: current.row_version,
                homeroom_ids: vec![context.homeroom_id],
            },
        )
        .await
        .unwrap();
        let current = groups::get(&pool, group.id).await.unwrap();
        groups::replace_teachers(
            &pool,
            context.teacher_id,
            group.id,
            ReplaceLearningGroupTeachersRequest {
                row_version: current.row_version,
                teachers: vec![TeacherAssignmentInput {
                    teacher_id: context.teacher_id,
                    role: LearningTeacherRole::Primary,
                }],
            },
        )
        .await
        .unwrap();
    }
    let revision = if command == "add" {
        initial
    } else {
        let preview = publication_preview(&pool, initial.id).await.unwrap();
        assert!(
            !preview
                .findings
                .iter()
                .any(|f| f.severity == AcademicChangeFindingSeverity::Blocking),
            "{:?}",
            preview.findings
        );
        change_sets::publish_change_set(
        &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
        &pool,
            context.teacher_id,
            initial.id,
            PublishAcademicTermChangeSetRequest {
                effective_from: preview.effective_from,
                row_version: preview.change_set_row_version,
                target_delivery_version_row_version: preview.target_delivery_version_row_version,
                preview_hash: preview.preview_hash,
                acknowledged_warning_codes: vec![],
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .unwrap();
        create_runtime_change_set(
            &pool,
            context.teacher_id,
            context.term_id,
            2,
            "pending-removal-next",
        )
        .await
    };
    if command == "period" {
        let current = change_sets::get_change_set(&pool, revision.id)
            .await
            .unwrap();
        change_sets::upsert_change_item(
            &pool,
            context.teacher_id,
            revision.id,
            UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
                change_set_row_version: current.row_version,
                item_row_version: None,
                learning_offering_id: offering.id,
                weekly_period_target: 2,
            },
        )
        .await
        .unwrap();
    } else if command == "teacher" {
        let teacher:Uuid=sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' AND status='active' AND id<>$1 ORDER BY id LIMIT 1").bind(context.teacher_id).fetch_one(&pool).await.unwrap();
        for group in course_groups.iter().take(2) {
            let current = change_sets::get_change_set(&pool, revision.id)
                .await
                .unwrap();
            change_sets::upsert_change_item(
                &pool,
                context.teacher_id,
                revision.id,
                UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
                    change_set_row_version: current.row_version,
                    item_row_version: None,
                    learning_group_id: group.id,
                    teacher_id: teacher,
                    teacher_role: LearningTeacherRole::Secondary,
                },
            )
            .await
            .unwrap();
        }
    }
    let unrelated = open_activity_with_catalog_target(
        &pool,
        &context,
        revision.target_delivery_version_id,
        ActivitySchedulingMode::Synchronized,
        false,
    )
    .await;
    let sibling = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        2,
        "pending-removal-sibling",
    )
    .await;
    if command == "add" {
        seed_shared_draft_inclusion(&pool, &sibling, &context, offering.id).await;
    }
    let wrong = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        2,
        "pending-removal-wrong-draft",
    )
    .await;
    if draft_version(&pool, wrong.id)
        .await
        .snapshot
        .offerings
        .iter()
        .any(|row| row.id == offering.id)
    {
        change_sets::remove_draft_offering(
            &pool,
            context.teacher_id,
            wrong.id,
            offering.id,
            removal_request(&pool, wrong.id).await,
        )
        .await
        .unwrap();
    }
    let sibling_before = serde_json::to_value(draft_version(&pool, sibling.id).await).unwrap();
    let preserved_before = preserved_facts(&pool).await;
    let group_id = course_groups[0].id;
    let group_path = format!(
        "/api/academic/term-change-sets/{}/groups/{group_id}",
        revision.id
    );
    let offering_path = format!(
        "/api/academic/term-change-sets/{}/offerings/{}",
        revision.id, offering.id
    );
    let permissions = &[codes::LEARNING_OFFERING_MANAGE_SCHOOL];
    let mut statuses = Vec::new();
    for path in [&group_path, &offering_path] {
        let before = pending_removal_facts(&pool).await;
        let request = removal_request(&pool, revision.id).await;
        let audit_before:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='academic_delivery_draft_resource.removed'").bind(revision.id).fetch_one(&pool).await.unwrap();
        let (status, body) = http_remove(
            delivery_router(&pool, context.teacher_id, permissions),
            path,
            &request,
        )
        .await;
        statuses.push(status);
        if status != StatusCode::OK {
            assert_eq!(
                before,
                pending_removal_facts(&pool).await,
                "failed HTTP removal changes no graph, revision, journal or audit facts"
            );
            eprintln!(
                "pending {command}: {path} returned {status}: {body}; all persisted facts unchanged"
            );
            continue;
        }
        let after = removal_request(&pool, revision.id).await;
        assert_eq!(
            after.change_set_row_version,
            request.change_set_row_version + 1
        );
        assert_eq!(
            after.target_delivery_version_row_version,
            request.target_delivery_version_row_version + 1
        );
        let audit_after:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='academic_delivery_draft_resource.removed'").bind(revision.id).fetch_one(&pool).await.unwrap();
        assert_eq!(audit_after, audit_before + 1);
        let current = draft_version(&pool, revision.id).await;
        if path == &group_path {
            let remaining = &current
                .snapshot
                .offerings
                .iter()
                .find(|row| row.id == offering.id)
                .unwrap()
                .groups;
            assert_eq!(remaining.len(), 2);
            assert!(remaining.iter().all(|group| group.id != group_id));
            let cleared_group:i64=sqlx::query_scalar("SELECT count(*) FROM academic_term_change_items WHERE change_set_id=$1 AND learning_group_id=$2").bind(revision.id).bind(group_id).fetch_one(&pool).await.unwrap();
            assert_eq!(cleared_group, 0);
            let excluded_groups: Vec<Uuid> = sqlx::query_scalar(
                "SELECT excluded_group_ids FROM academic_term_change_sets WHERE id=$1",
            )
            .bind(revision.id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(excluded_groups.contains(&group_id));
            if command == "teacher" {
                let retained_teacher:i64=sqlx::query_scalar("SELECT count(*) FROM academic_term_change_items WHERE change_set_id=$1 AND learning_group_id=$2").bind(revision.id).bind(course_groups[1].id).fetch_one(&pool).await.unwrap();
                assert_eq!(
                    retained_teacher, 1,
                    "another group's pending teacher command survives group removal"
                );
            }
            let before_wrong = pending_removal_facts(&pool).await;
            let wrong_path = format!(
                "/api/academic/term-change-sets/{}/offerings/{}",
                wrong.id, offering.id
            );
            let (wrong_status, _) = http_remove(
                delivery_router(&pool, context.teacher_id, permissions),
                &wrong_path,
                &removal_request(&pool, wrong.id).await,
            )
            .await;
            assert_eq!(wrong_status, StatusCode::CONFLICT);
            assert_eq!(before_wrong, pending_removal_facts(&pool).await);
            // Once the duplicate preflight is repaired, ordinary permission and stale guards must surface.
            for (permission, stale_parent, stale_target, expected) in [
                (vec![], false, false, StatusCode::FORBIDDEN),
                (permissions.to_vec(), true, false, StatusCode::CONFLICT),
                (permissions.to_vec(), false, true, StatusCode::CONFLICT),
            ] {
                let before = pending_removal_facts(&pool).await;
                let mut request = removal_request(&pool, revision.id).await;
                request.change_set_row_version += i64::from(stale_parent);
                request.target_delivery_version_row_version += i64::from(stale_target);
                let (status, _) = http_remove(
                    delivery_router(&pool, context.teacher_id, &permission),
                    &offering_path,
                    &request,
                )
                .await;
                assert_eq!(status, expected);
                assert_eq!(before, pending_removal_facts(&pool).await);
            }
        } else {
            assert!(current
                .snapshot
                .offerings
                .iter()
                .all(|row| row.id != offering.id));
            let excluded_offerings: Vec<Uuid> = sqlx::query_scalar(
                "SELECT excluded_offering_ids FROM academic_term_change_sets WHERE id=$1",
            )
            .bind(revision.id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(excluded_offerings.contains(&offering.id));
            let retained:i64=sqlx::query_scalar("SELECT count(*) FROM academic_term_change_items WHERE change_set_id=$1 AND learning_offering_id=$2").bind(revision.id).bind(unrelated.id).fetch_one(&pool).await.unwrap();
            assert_eq!(retained, 1, "unrelated pending commands survive");
            let cleared:i64=sqlx::query_scalar("SELECT count(*) FROM academic_term_change_items WHERE change_set_id=$1 AND (learning_offering_id=$2 OR learning_group_id=ANY($3))").bind(revision.id).bind(offering.id).bind(course_groups.iter().map(|group|group.id).collect::<Vec<_>>()).fetch_one(&pool).await.unwrap();
            assert_eq!(cleared, 0);
        }
    }
    assert_eq!(
        statuses,
        vec![StatusCode::OK, StatusCode::OK],
        "both real HTTP routes must accept pending {command} commands"
    );
    assert_eq!(
        preserved_before,
        preserved_facts(&pool).await,
        "shared/published registry, rosters, timetable and results stay intact"
    );
    assert_eq!(
        sibling_before,
        serde_json::to_value(draft_version(&pool, sibling.id).await).unwrap()
    );
    assert_eq!(
        groups::list(&pool, offering.id).await.unwrap().len(),
        3,
        "only selected snapshot membership is removed"
    );
}

#[tokio::test]
async fn delivery_room_scope_last_course_group_unlinks_its_room() {
    let pool = prepare_delivery_runtime_fixture("room_scope_last_group").await;
    let context = planning_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "room-scope-last-group",
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
            name: "กลุ่มห้องที่นำออก".into(),
            description: None,
            capacity: None,
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    groups::replace_homerooms(
        &pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: group.row_version,
            homeroom_ids: vec![context.homeroom_id],
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
    let after = draft_version(&pool, revision.id).await;
    assert!(
        after
            .snapshot
            .offerings
            .iter()
            .all(|row| row.id != offering.id),
        "removing the last course group must unlink its only room and omit the empty-audience offering"
    );
}

#[tokio::test]
async fn timetable_placement_preview_http_authorizes_persisted_source_resources() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::post,
    };
    use school_academic_timetable::{
        models::timetable_block::{
            CreateStructuralTimetableBlocksRequest, TimetableStructuralKind,
            TimetableStructuralSlotInput,
        },
        services::timetable_block_service,
    };
    use school_permissions::registry::codes;
    use tower::ServiceExt;

    let pool = prepare_delivery_runtime_fixture("timetable_preview_source_http").await;
    let actor = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (source_version, term, bell, delivery): (Uuid, Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT version.id,version.academic_term_id,version.bell_schedule_id,version.delivery_version_id
         FROM academic_timetable_versions version
         JOIN academic_terms term ON term.id=version.academic_term_id
         WHERE version.status='published' AND term.status='active'
         ORDER BY version.effective_from,version.id LIMIT 1",
    ).fetch_one(&pool).await.unwrap();
    let version = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,status,source_version_id,bell_schedule_id,delivery_version_id,created_by)
        SELECT $1,academic_term_id,academic_year_id,'draft',id,bell_schedule_id,delivery_version_id,$3 FROM academic_timetable_versions WHERE id=$2")
        .bind(version).bind(source_version).bind(actor).execute(&pool).await.unwrap();
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM academic_delivery_versions WHERE id=$1)"
    )
    .bind(delivery)
    .fetch_one(&pool)
    .await
    .unwrap());
    sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days)
        SELECT gen_random_uuid(),$1,'คาบทดสอบ',TIME '09:00',TIME '09:50',1,'MON-FRI'
        WHERE NOT EXISTS(SELECT 1 FROM bell_schedule_periods WHERE bell_schedule_id=$1 AND order_index=1)")
        .bind(bell).execute(&pool).await.unwrap();
    let period: Uuid = sqlx::query_scalar("SELECT id FROM bell_schedule_periods WHERE bell_schedule_id=$1 AND is_active ORDER BY order_index,id LIMIT 1")
        .bind(bell).fetch_one(&pool).await.unwrap();
    let block = timetable_block_service::create_structural_blocks(
        &pool,
        &school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
            &pool, actor,
        ),
        CreateStructuralTimetableBlocksRequest {
            timetable_version_id: version,
            academic_term_id: term,
            structural_kind: TimetableStructuralKind::Other,
            title: "คาบพิเศษต้องใช้สิทธิ์ระดับโรงเรียน".into(),
            note: None,
            slots: vec![TimetableStructuralSlotInput {
                day_of_week: "MON".into(),
                bell_schedule_period_id: period,
            }],
            homeroom_ids: vec![],
            teacher_ids: vec![actor],
            all_homerooms: false,
            all_teachers: false,
            room_id: None,
        },
    )
    .await
    .unwrap()
    .remove(0);
    let before = serde_json::to_value(&block).unwrap();
    // The supplied course candidate hides the actual source's structural scope.
    // Its version and own teacher would pass the assigned resource policy alone.
    let payload = json!({
        "timetableVersionId":version,"academicTermId":term,
        "source":{"kind":"existing_block","blockId":block.id,"rowVersion":block.row_version},
        "candidate":{"blockKind":"course","learningGroupId":null,"learningOfferingId":null,
            "roomId":null,"homeroomIds":[],"teacherIds":[actor],"instructorIds":[]},
        "targetDayOfWeek":"TUE","targetBellSchedulePeriodId":period,
        "expectedTargetBlockId":null,"expectedTargetRowVersion":null,
    });
    for (permission, expected) in [
        (
            codes::ACADEMIC_TIMETABLE_MANAGE_ASSIGNED,
            StatusCode::FORBIDDEN,
        ),
        (codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL, StatusCode::OK),
    ] {
        let routes = axum::Router::new().route(
            "/timetable-blocks/placement-preview",
            post(school_academic_http::handlers::timetable_blocks::preview_placement),
        );
        let response = academic_router(&pool, actor, &[permission], routes)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/academic/timetable-blocks/placement-preview")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .unwrap();
        assert_eq!(status, expected, "{}", String::from_utf8_lossy(&body));
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["success"], expected == StatusCode::OK);
        if expected == StatusCode::OK {
            assert_eq!(body["data"]["state"], "move");
            assert_eq!(body["data"]["conflicts"], json!([]));
        } else {
            assert!(body.get("data").is_none_or(Value::is_null));
        }
        assert_eq!(
            before,
            serde_json::to_value(
                timetable_block_service::get_block(&pool, block.id)
                    .await
                    .unwrap()
            )
            .unwrap()
        );
    }
}
