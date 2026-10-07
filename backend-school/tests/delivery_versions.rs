#[path = "../src/modules/academic/cutover_test_preflight.rs"]
pub mod cutover_test_preflight;
#[path = "../src/modules/academic/cutover_test_support.rs"]
pub mod cutover_test_support;

use school_academic_delivery::services::versions;
use school_test_db::create_named_test_pool_with_max_connections;
use sqlx::PgPool;
use uuid::Uuid;

use cutover_test_support::{apply_migrations_through, seed_release_two_predecessor};

async fn predecessor(name: &str) -> PgPool {
    let pool = create_named_test_pool_with_max_connections(name, 1).await;
    seed_release_two_predecessor(&pool)
        .await
        .expect("legacy fixture reaches 059");
    apply_migrations_through(&pool, 85)
        .await
        .expect("legacy fixture reaches 085");
    pool
}

async fn add_placement_version(pool: &PgPool, published: bool, source: bool) -> Uuid {
    let id = Uuid::new_v4();
    let source_id: Uuid = sqlx::query_scalar("SELECT id FROM academic_timetable_versions v WHERE status='published' AND EXISTS(SELECT 1 FROM academic_timetable_version_targets t WHERE t.timetable_version_id=v.id) AND EXISTS(SELECT 1 FROM academic_timetable_blocks b WHERE b.timetable_version_id=v.id) ORDER BY effective_from,id LIMIT 1")
        .fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,effective_from,status,source_version_id,bell_schedule_id,created_by)
        SELECT $1,academic_term_id,academic_year_id,(SELECT max(existing.effective_from)+7 FROM academic_timetable_versions existing WHERE existing.academic_term_id=academic_timetable_versions.academic_term_id),'draft',CASE WHEN $2 THEN id ELSE NULL END,bell_schedule_id,published_by
        FROM academic_timetable_versions WHERE id=$3")
        .bind(id).bind(source).bind(source_id).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_version_targets(timetable_version_id,learning_offering_id,academic_term_id,academic_year_id,weekly_period_target)
        SELECT $1,t.learning_offering_id,t.academic_term_id,t.academic_year_id,t.weekly_period_target
        FROM academic_timetable_version_targets t JOIN academic_timetable_versions v ON v.id=t.timetable_version_id
        WHERE v.id=$2 ORDER BY t.learning_offering_id")
        .bind(id).bind(source_id).execute(pool).await.unwrap();
    if published {
        sqlx::query("UPDATE academic_timetable_versions SET status='published',published_by=created_by,published_at=now() WHERE id=$1")
            .bind(id).execute(pool).await.unwrap();
    }
    id
}

#[tokio::test]
async fn delivery_version_migration_preserves_graph_and_supports_many_timetables() {
    let pool = predecessor("delivery_version_many_tables").await;
    let other = add_placement_version(&pool, true, true).await;
    let before_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM academic_timetable_blocks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    apply_migrations_through(&pool, 87)
        .await
        .expect("delivery graphs reconcile");
    let term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_timetable_versions WHERE id=$1")
            .bind(other)
            .fetch_one(&pool)
            .await
            .unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
    let deliveries = versions::list_versions(
        &pool,
        term_id,
        &school_authorization::AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        deliveries.len(),
        1,
        "only positions changed, so both tables share a graph"
    );
    let delivery = versions::get_version(&pool, deliveries[0].id)
        .await
        .unwrap();
    assert!(!delivery.snapshot.offerings.is_empty());
    assert!(delivery
        .snapshot
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .all(|group| !group.teachers.is_empty()));
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_versions WHERE delivery_version_id=$1",
    )
    .bind(deliveries[0].id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables, 2);
    let after_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM academic_timetable_blocks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before_ids, after_ids);
    let passing: bool = sqlx::query_scalar("SELECT passed AND jsonb_array_length(checks)=15 FROM academic_delivery_version_migration_audit WHERE migration_version=87")
        .fetch_one(&pool).await.unwrap();
    assert!(passing);
    let immutable =
        sqlx::query("UPDATE academic_delivery_versions SET snapshot='{}'::jsonb WHERE id=$1")
            .bind(deliveries[0].id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert!(immutable
        .to_string()
        .contains("ACADEMIC_DELIVERY_VERSION_IMMUTABLE"));
    apply_migrations_through(&pool, 89)
        .await
        .expect("canonical cleanup has fresh evidence");
    let canonical: bool=sqlx::query_scalar("SELECT cutover_completed AND to_regclass('academic_timetable_version_targets') IS NULL FROM academic_delivery_version_migration_audit WHERE migration_version=87")
        .fetch_one(&pool).await.unwrap();
    assert!(canonical);
    let audit = school_academic_delivery::services::versions::read_cutover_audit(&pool)
        .await
        .unwrap();
    assert!(audit.completed);
    assert_eq!(audit.checks.len(), 30);
    sqlx::query("UPDATE academic_delivery_version_migration_audit SET passed=false WHERE migration_version=87")
        .execute(&pool).await.unwrap();
    assert!(
        !school_academic_delivery::services::versions::read_cutover_audit(&pool)
            .await
            .unwrap()
            .completed
    );
    sqlx::query("DELETE FROM academic_delivery_version_migration_audit WHERE migration_version=87")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        !school_academic_delivery::services::versions::read_cutover_audit(&pool)
            .await
            .unwrap()
            .completed
    );
    let versions = school_academic_timetable::services::timetable_version_service::list_versions(
        &pool, term_id,
    )
    .await
    .unwrap();
    assert_eq!(versions.len(), 2);
    assert!(versions
        .iter()
        .all(|version| version.delivery_version_id == delivery.id));
    assert_eq!(
        before_ids,
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM academic_timetable_blocks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn delivery_version_migration_keeps_a_placement_draft_independent() {
    let pool = predecessor("delivery_version_pure_draft").await;
    let draft = add_placement_version(&pool, false, true).await;
    apply_migrations_through(&pool, 87)
        .await
        .expect("draft has explicit published source evidence");
    let pair: (Uuid,Uuid) = sqlx::query_as("SELECT draft.delivery_version_id,source.delivery_version_id
        FROM academic_timetable_versions draft JOIN academic_timetable_versions source ON source.id=draft.source_version_id WHERE draft.id=$1")
        .bind(draft).fetch_one(&pool).await.unwrap();
    assert_eq!(pair.0, pair.1);
    let delivery_drafts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM academic_delivery_versions WHERE status='draft'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(delivery_drafts, 0);
    apply_migrations_through(&pool, 88)
        .await
        .expect("placement drafts cut over independently");
    let null_date: bool = sqlx::query_scalar(
        "SELECT effective_from IS NULL FROM academic_timetable_versions WHERE id=$1",
    )
    .bind(draft)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(null_date);
}

#[tokio::test]
async fn delivery_version_migration_rejects_unmapped_source_without_partial_backfill() {
    let pool = predecessor("delivery_version_unmapped").await;
    let draft = add_placement_version(&pool, false, false).await;
    apply_migrations_through(&pool, 86).await.unwrap();
    let failure = apply_migrations_through(&pool, 87).await.unwrap_err();
    assert!(failure
        .to_string()
        .contains("DELIVERY_VERSION_SOURCE_UNMAPPABLE"));
    let untouched: bool = sqlx::query_scalar(
        "SELECT delivery_version_id IS NULL FROM academic_timetable_versions WHERE id=$1",
    )
    .bind(draft)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(untouched);
    let canonical_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM academic_delivery_versions")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(canonical_count, 0);
}

#[tokio::test]
async fn delivery_version_cutover_refuses_missing_and_stale_evidence() {
    let pool = predecessor("delivery_cutover_stale").await;
    let draft = add_placement_version(&pool, false, true).await;
    apply_migrations_through(&pool, 87).await.unwrap();
    sqlx::query("UPDATE academic_timetable_versions SET row_version=row_version+1 WHERE id=$1")
        .bind(draft)
        .execute(&pool)
        .await
        .unwrap();
    let failure = apply_migrations_through(&pool, 88).await.unwrap_err();
    assert!(failure
        .to_string()
        .contains("DELIVERY_VERSION_FRESH_RECONCILIATION_REQUIRED"));
    let legacy_still_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('academic_timetable_version_targets') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(legacy_still_exists);
    sqlx::query("UPDATE academic_delivery_version_migration_audit SET passed=false WHERE migration_version=87").execute(&pool).await.unwrap();
    let failure = apply_migrations_through(&pool, 88).await.unwrap_err();
    assert!(failure
        .to_string()
        .contains("DELIVERY_VERSION_FRESH_RECONCILIATION_REQUIRED"));
}

#[tokio::test]
async fn independent_timetable_clone_reuses_source_and_deletes_only_draft_children() {
    use school_academic_timetable::models::timetable_version::{
        CloneTimetableVersionRequest, DeleteTimetableDraftRequest,
    };
    use school_academic_timetable::services::{
        timetable_lifecycle, timetable_version_service as tables,
    };
    let pool = predecessor("independent_table_delete").await;
    apply_migrations_through(&pool, 89).await.unwrap();
    let (source_id,actor): (Uuid,Uuid)=sqlx::query_as("SELECT id,published_by FROM academic_timetable_versions version WHERE status='published' AND EXISTS(SELECT 1 FROM academic_timetable_blocks block WHERE block.timetable_version_id=version.id) ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap();
    let source = tables::get_version(&pool, source_id, chrono::Utc::now().date_naive())
        .await
        .unwrap();
    let draft = tables::clone_draft(
        &pool,
        actor,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(draft.source_version_id, Some(source_id));
    assert_eq!(draft.effective_from, None);
    assert_eq!(draft.delivery_version_id, source.delivery_version_id);
    let cloned_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=$1",
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(cloned_count > 0);
    let resumed = tables::clone_draft(
        &pool,
        actor,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(resumed.id, draft.id);
    let old_graph = versions::get_version(&pool, source.delivery_version_id)
        .await
        .unwrap();
    let failure = timetable_lifecycle::delete_draft(
        &pool,
        actor,
        draft.id,
        DeleteTimetableDraftRequest {
            expected_block_count: cloned_count,
            row_version: draft.row_version - 1,
        },
    )
    .await
    .unwrap_err();
    assert!(
        failure.to_string().contains("rowVersion") || failure.to_string().contains("แบบร่างเปลี่ยนไป")
    );
    let incorrect_count = timetable_lifecycle::delete_draft(
        &pool,
        actor,
        draft.id,
        DeleteTimetableDraftRequest {
            row_version: draft.row_version,
            expected_block_count: cloned_count - 1,
        },
    )
    .await
    .unwrap_err();
    assert!(incorrect_count.to_string().contains("จำนวนคาบ"));
    // An external reference not known to the lifecycle service must also fail atomically.
    sqlx::query("CREATE TABLE external_placement_reference (block_id uuid PRIMARY KEY REFERENCES academic_timetable_blocks(id))").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO external_placement_reference SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=$1 ORDER BY id LIMIT 1").bind(draft.id).execute(&pool).await.unwrap();
    let children_before: serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('blocks',(SELECT jsonb_agg(to_jsonb(b) ORDER BY id) FROM academic_timetable_blocks b WHERE timetable_version_id=$1),'groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY g.id) FROM academic_timetable_block_groups g JOIN academic_timetable_blocks b ON b.id=g.block_id WHERE b.timetable_version_id=$1),'sync',(SELECT jsonb_agg(to_jsonb(s) ORDER BY s.id) FROM academic_timetable_block_group_sync s JOIN academic_timetable_blocks b ON b.id=s.block_id WHERE b.timetable_version_id=$1))").bind(draft.id).fetch_one(&pool).await.unwrap();
    let referenced = timetable_lifecycle::delete_draft(
        &pool,
        actor,
        draft.id,
        DeleteTimetableDraftRequest {
            row_version: draft.row_version,
            expected_block_count: cloned_count,
        },
    )
    .await
    .unwrap_err();
    assert!(referenced.to_string().contains("ข้อมูลอื่นอ้างอิง"));
    let children_after: serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('blocks',(SELECT jsonb_agg(to_jsonb(b) ORDER BY id) FROM academic_timetable_blocks b WHERE timetable_version_id=$1),'groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY g.id) FROM academic_timetable_block_groups g JOIN academic_timetable_blocks b ON b.id=g.block_id WHERE b.timetable_version_id=$1),'sync',(SELECT jsonb_agg(to_jsonb(s) ORDER BY s.id) FROM academic_timetable_block_group_sync s JOIN academic_timetable_blocks b ON b.id=s.block_id WHERE b.timetable_version_id=$1))").bind(draft.id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        children_before, children_after,
        "all draft child deletion rolls back on a referenced placement"
    );
    assert_eq!(
        tables::get_version(&pool, draft.id, chrono::Utc::now().date_naive())
            .await
            .unwrap()
            .row_version,
        draft.row_version
    );
    sqlx::query("DROP TABLE external_placement_reference")
        .execute(&pool)
        .await
        .unwrap();
    let deleted = timetable_lifecycle::delete_draft(
        &pool,
        actor,
        draft.id,
        DeleteTimetableDraftRequest {
            expected_block_count: cloned_count,
            row_version: draft.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(deleted.source_version_id, Some(source_id));
    assert_eq!(deleted.deleted_block_count, cloned_count);
    assert!(
        tables::get_version(&pool, draft.id, chrono::Utc::now().date_naive())
            .await
            .is_err()
    );
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=$1",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, cloned_count);
    let unchanged = versions::get_version(&pool, old_graph.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(old_graph.snapshot).unwrap(),
        serde_json::to_value(unchanged.snapshot).unwrap()
    );
    assert!(timetable_lifecycle::delete_draft(
        &pool,
        actor,
        source_id,
        DeleteTimetableDraftRequest {
            expected_block_count: cloned_count,
            row_version: source.row_version
        }
    )
    .await
    .unwrap_err()
    .to_string()
    .contains("แบบร่าง"));
    let audit_count: i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='academic_timetable_version.draft_deleted'").bind(draft.id).fetch_one(&pool).await.unwrap();
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn independent_delivery_publication_does_not_publish_or_rewrite_timetables() {
    use chrono::Duration;
    use school_academic_delivery::models::{
        CreateAcademicTermChangeSetRequest, PublishAcademicTermChangeSetRequest,
        UpsertAcademicTermChangeItemRequest,
    };
    use school_academic_delivery::services::change_sets;
    use school_academic_timetable::models::timetable_version::{
        CloneTimetableVersionRequest, UpdateTimetableDeliverySourceRequest,
    };
    use school_academic_timetable::services::{
        timetable_lifecycle, timetable_version_service as tables,
    };
    let pool = predecessor("independent_delivery_publish").await;
    apply_migrations_through(&pool, 96).await.unwrap();
    let (source_id,actor): (Uuid,Uuid)=sqlx::query_as("SELECT version.id,version.published_by FROM academic_timetable_versions version WHERE status='published' AND EXISTS(SELECT 1 FROM academic_timetable_blocks block WHERE block.timetable_version_id=version.id) ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap();
    let source = tables::get_version(&pool, source_id, chrono::Utc::now().date_naive())
        .await
        .unwrap();
    let base = versions::get_version(&pool, source.delivery_version_id)
        .await
        .unwrap();
    let draft = tables::clone_draft(
        &pool,
        actor,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    let before: Vec<(Uuid, i64)> =
        sqlx::query_as("SELECT id,row_version FROM academic_timetable_blocks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    // Exercise an active lifecycle with the preserved historical fixture IDs.
    // Publication dates follow the current clock rather than a stale fixture date.
    let publish_date = std::cmp::max(
        chrono::Utc::now().date_naive(),
        base.effective_from.unwrap() + Duration::days(1),
    );
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$2) WHERE id=$1")
        .bind(source.academic_year_id)
        .bind(publish_date + Duration::days(30))
        .execute(&pool)
        .await
        .unwrap();
    let revision = change_sets::create_change_set(
        &pool,
        actor,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: source.academic_term_id,
            reason: "ปรับจำนวนคาบ โดยยังไม่จัดตารางใหม่".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let offering = &base.snapshot.offerings[0];
    let updated = change_sets::upsert_change_item(
        &pool,
        actor,
        revision.id,
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            change_set_row_version: revision.row_version,
            item_row_version: None,
            learning_offering_id: offering.id,
            weekly_period_target: offering.weekly_period_target + 1,
        },
    )
    .await
    .unwrap();
    let preview = publication_preview(&pool, revision.id).await.unwrap();
    assert!(
        preview.findings.iter().all(|finding| finding.severity
            != school_academic_delivery::models::AcademicChangeFindingSeverity::Blocking),
        "delivery readiness must not demand placed lessons: {:?}",
        preview.findings
    );
    let receipt = change_sets::publish_change_set(
        &pool,
        actor,
        updated.id,
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
    let after: Vec<(Uuid, i64)> =
        sqlx::query_as("SELECT id,row_version FROM academic_timetable_blocks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        source.delivery_version_id,
        tables::get_version(&pool, source.id, chrono::Utc::now().date_naive())
            .await
            .unwrap()
            .delivery_version_id
    );
    assert_eq!(
        draft.delivery_version_id,
        tables::get_version(&pool, draft.id, chrono::Utc::now().date_naive())
            .await
            .unwrap()
            .delivery_version_id,
        "a running draft never silently upgrades"
    );
    let reloaded = tables::get_version(&pool, draft.id, chrono::Utc::now().date_naive())
        .await
        .unwrap();
    let updated = timetable_lifecycle::update_source(
        &pool,
        actor,
        draft.id,
        UpdateTimetableDeliverySourceRequest {
            row_version: reloaded.row_version,
            delivery_version_id: receipt.target_delivery_version_id,
        },
    )
    .await
    .unwrap();
    assert_ne!(updated.delivery_version_id, source.delivery_version_id);
    assert_eq!(
        before,
        sqlx::query_as::<_, (Uuid, i64)>(
            "SELECT id,row_version FROM academic_timetable_blocks ORDER BY id"
        )
        .fetch_all(&pool)
        .await
        .unwrap()
    );
    let base_after = versions::get_version(&pool, base.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(base.snapshot).unwrap(),
        serde_json::to_value(base_after.snapshot).unwrap()
    );
}

#[tokio::test]
async fn changed_delivery_graphs_preserve_dated_source_chain_including_a_b_a() {
    let pool = predecessor("delivery_source_chain").await;
    let middle = add_placement_version(&pool, false, true).await;
    sqlx::query("UPDATE academic_timetable_version_targets SET weekly_period_target=weekly_period_target+1 WHERE timetable_version_id=$1").bind(middle).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_timetable_versions SET status='published',published_by=created_by,published_at=now() WHERE id=$1").bind(middle).execute(&pool).await.unwrap();
    let last = add_placement_version(&pool, false, true).await;
    sqlx::query("UPDATE academic_timetable_versions SET effective_from=effective_from+7,status='published',published_by=created_by,published_at=now() WHERE id=$1").bind(last).execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
    let term: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_timetable_versions WHERE id=$1")
            .bind(last)
            .fetch_one(&pool)
            .await
            .unwrap();
    let chain:Vec<(Uuid,Option<Uuid>,serde_json::Value)>=sqlx::query_as("SELECT id,source_version_id,snapshot FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from,id").bind(term).fetch_all(&pool).await.unwrap();
    assert_eq!(chain.len(), 3);
    assert_eq!(chain[0].1, None);
    assert_eq!(chain[1].1, Some(chain[0].0));
    assert_eq!(chain[2].1, Some(chain[1].0));
    assert_eq!(chain[0].2, chain[2].2);
    assert_ne!(
        chain[0].0, chain[2].0,
        "returning to earlier content is a new dated source"
    );
    assert_ne!(chain[0].2, chain[1].2);
}

#[tokio::test]
async fn migration_refuses_changed_pure_draft_targets_then_retries_after_evidence_is_repaired() {
    let pool = predecessor("delivery_changed_pure_targets").await;
    let draft = add_placement_version(&pool, false, true).await;
    sqlx::query("UPDATE academic_timetable_version_targets SET weekly_period_target=weekly_period_target+1 WHERE timetable_version_id=$1").bind(draft).execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 86).await.unwrap();
    let error = apply_migrations_through(&pool, 87).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("DELIVERY_DRAFT_TARGETS_UNMAPPABLE"));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_delivery_versions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    // Repair from the explicitly recorded source; no name-based matching.
    sqlx::query("UPDATE academic_timetable_version_targets target SET weekly_period_target=original.weekly_period_target FROM academic_timetable_versions draft,academic_timetable_version_targets original WHERE draft.id=$1 AND target.timetable_version_id=draft.id AND original.timetable_version_id=draft.source_version_id AND original.learning_offering_id=target.learning_offering_id").bind(draft).execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
    let audits:i64=sqlx::query_scalar("SELECT count(*) FROM academic_delivery_version_migration_audit WHERE passed AND cutover_completed").fetch_one(&pool).await.unwrap();
    assert_eq!(audits, 2);
}

#[tokio::test]
async fn historical_placed_groups_without_dated_assignment_evidence_block_migration() {
    let pool = predecessor("delivery_missing_history").await;
    let group:Uuid=sqlx::query_scalar("SELECT g.learning_group_id FROM academic_timetable_block_groups g JOIN academic_timetable_blocks b ON b.id=g.block_id JOIN academic_timetable_versions v ON v.id=b.timetable_version_id WHERE g.is_active AND b.is_active AND v.status='published' ORDER BY g.id LIMIT 1").fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE learning_groups SET created_at=now()+interval '1 day' WHERE id=$1")
        .bind(group)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("ALTER TABLE learning_group_teachers DISABLE TRIGGER learning_group_teachers_published_immutable").execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE learning_group_teachers SET starts_on=(SELECT max(version.effective_from)+1 FROM academic_timetable_versions version JOIN academic_timetable_blocks block ON block.timetable_version_id=version.id JOIN academic_timetable_block_groups placement ON placement.block_id=block.id WHERE placement.learning_group_id=$1 AND version.status='published') WHERE learning_group_id=$1")
        .bind(group).execute(&mut *tx).await.unwrap();
    sqlx::query("ALTER TABLE learning_group_teachers ENABLE TRIGGER learning_group_teachers_published_immutable").execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    apply_migrations_through(&pool, 86).await.unwrap();
    let error = apply_migrations_through(&pool, 87).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("DELIVERY_VERSION_HISTORICAL_GROUP_UNMAPPABLE"));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_delivery_versions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let legacy: bool =
        sqlx::query_scalar("SELECT to_regclass('academic_timetable_version_targets') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(legacy);
}

#[tokio::test]
async fn mixed_legacy_revision_preserves_opening_commands_and_its_placed_draft() {
    let pool = predecessor("delivery_mixed_revision").await;
    let draft = add_placement_version(&pool, false, true).await;
    let revision = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_term_change_sets(id,academic_term_id,academic_year_id,effective_from,reason,base_timetable_version_id,target_timetable_version_id,idempotency_key,creation_request_hash,created_by) SELECT $2,academic_term_id,academic_year_id,effective_from,'ปรับจำนวนคาบและตำแหน่ง',source_version_id,id,$2::text,repeat('a',64),created_by FROM academic_timetable_versions WHERE id=$1").bind(draft).bind(revision).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_timetable_versions SET change_set_id=$2 WHERE id=$1")
        .bind(draft)
        .bind(revision)
        .execute(&pool)
        .await
        .unwrap();
    let offering:Uuid=sqlx::query_scalar("SELECT learning_offering_id FROM academic_timetable_version_targets WHERE timetable_version_id=$1 ORDER BY learning_offering_id LIMIT 1").bind(draft).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE academic_timetable_version_targets SET weekly_period_target=weekly_period_target+1 WHERE timetable_version_id=$1 AND learning_offering_id=$2").bind(draft).bind(offering).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_term_change_items(change_set_id,academic_term_id,academic_year_id,action_kind,learning_offering_id,weekly_period_target,created_by) SELECT $2,version.academic_term_id,version.academic_year_id,'adjust_weekly_period_target',$3,weekly_period_target,version.created_by FROM academic_timetable_versions version JOIN academic_timetable_version_targets target ON target.timetable_version_id=version.id WHERE version.id=$1 AND target.learning_offering_id=$3").bind(draft).bind(revision).bind(offering).execute(&pool).await.unwrap();
    // Copy a real placement into the mixed draft. Its identity must survive cutover.
    let block = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_timetable_blocks(id,timetable_version_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,structural_kind,scheduling_mode,learning_offering_id,title,note,created_by) SELECT $2,$1,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,structural_kind,scheduling_mode,learning_offering_id,title,note,created_by FROM academic_timetable_blocks WHERE timetable_version_id=(SELECT source_version_id FROM academic_timetable_versions WHERE id=$1) AND is_active ORDER BY id LIMIT 1").bind(draft).bind(block).execute(&pool).await.unwrap();
    let item_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM academic_term_change_items WHERE change_set_id=$1 ORDER BY id",
    )
    .bind(revision)
    .fetch_all(&pool)
    .await
    .unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
    let (pinned,opening_draft,status):(Uuid,Uuid,String)=sqlx::query_as("SELECT timetable.delivery_version_id,revision.target_delivery_version_id,opening.status FROM academic_timetable_versions timetable JOIN academic_term_change_sets revision ON revision.id=$2 JOIN academic_delivery_versions opening ON opening.id=revision.target_delivery_version_id WHERE timetable.id=$1").bind(draft).bind(revision).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "draft");
    assert_ne!(pinned, opening_draft);
    let source = versions::get_version(&pool, pinned).await.unwrap();
    let pending = versions::get_version(&pool, opening_draft).await.unwrap();
    assert_eq!(pending.source_version_id, Some(pinned));
    let original = source
        .snapshot
        .offerings
        .iter()
        .find(|row| row.id == offering)
        .unwrap();
    let changed = pending
        .snapshot
        .offerings
        .iter()
        .find(|row| row.id == offering)
        .unwrap();
    assert_eq!(
        changed.weekly_period_target,
        original.weekly_period_target + 1
    );
    assert_eq!(
        item_ids,
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM academic_term_change_items WHERE change_set_id=$1 ORDER BY id"
        )
        .bind(revision)
        .fetch_all(&pool)
        .await
        .unwrap()
    );
    let preserved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_timetable_blocks WHERE id=$1 AND timetable_version_id=$2)").bind(block).bind(draft).fetch_one(&pool).await.unwrap();
    assert!(preserved);
}

#[tokio::test]
async fn explicit_legacy_target_inclusions_become_opening_commands_without_rewriting_tables() {
    let pool = predecessor("delivery_direct_target_inclusion").await;
    let draft = add_placement_version(&pool, false, true).await;
    let revision = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_term_change_sets(id,academic_term_id,academic_year_id,effective_from,reason,base_timetable_version_id,target_timetable_version_id,idempotency_key,creation_request_hash,created_by) SELECT $2,academic_term_id,academic_year_id,effective_from,'รายการเพิ่มด้วย ID จากเส้นทางเดิม',source_version_id,id,$2::text,repeat('b',64),created_by FROM academic_timetable_versions WHERE id=$1")
        .bind(draft).bind(revision).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_timetable_versions SET change_set_id=$2 WHERE id=$1")
        .bind(draft)
        .bind(revision)
        .execute(&pool)
        .await
        .unwrap();
    let original:Uuid=sqlx::query_scalar("SELECT target.learning_offering_id FROM academic_timetable_version_targets target JOIN course_offering_details course ON course.learning_offering_id=target.learning_offering_id WHERE target.timetable_version_id=$1 ORDER BY target.learning_offering_id LIMIT 1")
        .bind(draft).fetch_one(&pool).await.unwrap();
    let added = Uuid::new_v4();
    let subject = Uuid::new_v4();
    let subject_version = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO subjects SELECT (jsonb_populate_record(NULL::subjects,to_jsonb(source)||jsonb_build_object('id',$2::text,'code','INCLUSION-TEST','identity_key','inclusion-test'))).* FROM subjects source JOIN course_offering_details detail ON detail.subject_id=source.id WHERE detail.learning_offering_id=$1")
        .bind(original).bind(subject).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO subject_versions SELECT (jsonb_populate_record(NULL::subject_versions,to_jsonb(source)||jsonb_build_object('id',$2::text,'subject_id',$3::text,'code','INCLUSION-TEST'))).* FROM subject_versions source JOIN course_offering_details detail ON detail.subject_version_id=source.id WHERE detail.learning_offering_id=$1")
        .bind(original).bind(subject_version).bind(subject).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,to_jsonb(original)||jsonb_build_object('id',$2::text,'code_snapshot','INCLUSION-TEST','status','draft'))).* FROM learning_offerings original WHERE id=$1")
        .bind(original).bind(added).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details SELECT (jsonb_populate_record(NULL::course_offering_details,to_jsonb(original)||jsonb_build_object('learning_offering_id',$2::text,'subject_id',$3::text,'subject_version_id',$4::text))).* FROM course_offering_details original WHERE learning_offering_id=$1")
        .bind(original).bind(added).bind(subject).bind(subject_version).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_version_targets(timetable_version_id,learning_offering_id,academic_term_id,academic_year_id,weekly_period_target) SELECT timetable_version_id,$2,academic_term_id,academic_year_id,weekly_period_target FROM academic_timetable_version_targets WHERE timetable_version_id=$1 AND learning_offering_id=$3")
        .bind(draft).bind(added).bind(original).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let before_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM academic_timetable_blocks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
    let (pinned,pending):(Uuid,Uuid)=sqlx::query_as("SELECT timetable.delivery_version_id,revision.target_delivery_version_id FROM academic_timetable_versions timetable JOIN academic_term_change_sets revision ON revision.id=$2 WHERE timetable.id=$1")
        .bind(draft).bind(revision).fetch_one(&pool).await.unwrap();
    assert_ne!(pinned, pending);
    assert!(!versions::get_version(&pool, pinned)
        .await
        .unwrap()
        .snapshot
        .offerings
        .iter()
        .any(|offering| offering.id == added));
    assert!(versions::get_version(&pool, pending)
        .await
        .unwrap()
        .snapshot
        .offerings
        .iter()
        .any(|offering| offering.id == added));
    let item:(Uuid,Uuid,String)=sqlx::query_as("SELECT id,learning_offering_id,action_kind FROM academic_term_change_items WHERE change_set_id=$1")
        .bind(revision).fetch_one(&pool).await.unwrap();
    assert_eq!(
        item.0,
        Uuid::new_v5(
            &revision,
            format!("legacy-included-offering:{added}").as_bytes()
        )
    );
    assert_eq!(item.1, added);
    assert_eq!(item.2, "add_offering");
    let fresh:bool=sqlx::query_scalar("SELECT snapshot=academic_delivery_revision_snapshot($2) FROM academic_delivery_versions WHERE id=$1")
        .bind(pending).bind(revision).fetch_one(&pool).await.unwrap();
    assert!(
        fresh,
        "migration preserves a usable opening draft, including exact targets"
    );
    assert_eq!(
        before_ids,
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM academic_timetable_blocks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap()
    );
    apply_migrations_through(&pool, 89).await.unwrap();
    assert!(versions::read_cutover_audit(&pool).await.unwrap().completed);
}

#[tokio::test]
async fn pre_effective_groups_are_preserved_when_a_timetable_was_published_early() {
    let pool = predecessor("delivery_early_publication").await;
    let (table,group):(Uuid,Uuid)=sqlx::query_as("SELECT version.id,placed.learning_group_id FROM academic_timetable_versions version JOIN academic_timetable_blocks block ON block.timetable_version_id=version.id JOIN academic_timetable_block_groups placed ON placed.block_id=block.id WHERE version.status='published' AND block.is_active AND placed.is_active ORDER BY version.id,placed.id LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("ALTER TABLE academic_timetable_versions DISABLE TRIGGER academic_timetable_versions_published_immutable").execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE academic_timetable_versions SET published_at=(effective_from-14)::timestamp AT TIME ZONE 'Asia/Bangkok' WHERE id=$1")
        .bind(table).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE learning_groups SET created_at=(SELECT (effective_from-1)::timestamp AT TIME ZONE 'Asia/Bangkok' FROM academic_timetable_versions WHERE id=$1) WHERE id=$2")
        .bind(table).bind(group).execute(&mut *tx).await.unwrap();
    sqlx::query("ALTER TABLE academic_timetable_versions ENABLE TRIGGER academic_timetable_versions_published_immutable").execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
    let source: Uuid = sqlx::query_scalar(
        "SELECT delivery_version_id FROM academic_timetable_versions WHERE id=$1",
    )
    .bind(table)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(versions::get_version(&pool, source)
        .await
        .unwrap()
        .snapshot
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .any(|item| item.id == group));
    assert!(versions::read_cutover_audit(&pool).await.unwrap().completed);
}

#[tokio::test]
async fn late_imported_groups_use_exact_dated_placement_evidence_and_preserve_identity() {
    let pool = predecessor("delivery_dated_late_group").await;
    let (table,group):(Uuid,Uuid)=sqlx::query_as("SELECT version.id,placed.learning_group_id FROM academic_timetable_versions version JOIN academic_timetable_blocks block ON block.timetable_version_id=version.id JOIN academic_timetable_block_groups placed ON placed.block_id=block.id WHERE version.status='published' AND block.is_active AND placed.is_active ORDER BY version.id,placed.id LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let before: Vec<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT id,block_id,learning_group_id FROM academic_timetable_block_groups ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let assignments:Vec<(Uuid,Uuid,String)>=sqlx::query_as("SELECT id,teacher_id,role FROM learning_group_teachers WHERE learning_group_id=$1 ORDER BY id")
        .bind(group).fetch_all(&pool).await.unwrap();
    sqlx::query("UPDATE learning_groups SET created_at=(SELECT (effective_from+30)::timestamp AT TIME ZONE 'Asia/Bangkok' FROM academic_timetable_versions WHERE id=$1) WHERE id=$2")
        .bind(table).bind(group).execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
    let source: Uuid = sqlx::query_scalar(
        "SELECT delivery_version_id FROM academic_timetable_versions WHERE id=$1",
    )
    .bind(table)
    .fetch_one(&pool)
    .await
    .unwrap();
    let source = versions::get_version(&pool, source).await.unwrap();
    let imported = source
        .snapshot
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .find(|item| item.id == group)
        .unwrap();
    assert_eq!(imported.teachers.len(), assignments.len());
    for (id, teacher, role) in assignments {
        assert!(imported
            .teachers
            .iter()
            .any(|assignment| assignment.assignment_id == id
                && assignment.teacher_id == teacher
                && serde_json::to_value(&assignment.role).unwrap() == role));
    }
    let after: Vec<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT id,block_id,learning_group_id FROM academic_timetable_block_groups ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    assert!(versions::read_cutover_audit(&pool).await.unwrap().completed);
}

#[tokio::test]
async fn historical_instructors_require_exact_roles_and_unambiguous_dated_episodes() {
    for ambiguous in [false, true] {
        let pool = predecessor(if ambiguous {
            "delivery_ambiguous_teacher_history"
        } else {
            "delivery_wrong_teacher_role"
        })
        .await;
        let (group,instructor):(Uuid,Uuid)=sqlx::query_as("SELECT placed.learning_group_id,instructor.id FROM academic_timetable_versions version JOIN academic_timetable_blocks block ON block.timetable_version_id=version.id JOIN academic_timetable_block_groups placed ON placed.block_id=block.id JOIN academic_timetable_block_group_instructors instructor ON instructor.block_group_id=placed.id WHERE version.status='published' AND block.is_active AND placed.is_active ORDER BY version.id,placed.id,instructor.id LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        if ambiguous {
            let mut tx = pool.begin().await.unwrap();
            // Exercise corrupt legacy input without weakening the production guard.
            sqlx::query("ALTER TABLE learning_group_teachers DISABLE TRIGGER learning_group_teachers_interval_guard").execute(&mut *tx).await.unwrap();
            sqlx::query("ALTER TABLE learning_group_teachers DISABLE TRIGGER learning_group_teachers_published_immutable").execute(&mut *tx).await.unwrap();
            sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,ends_on) SELECT uuid_generate_v4(),assignment.learning_group_id,assignment.academic_term_id,assignment.academic_year_id,assignment.teacher_id,assignment.role,assignment.starts_on-1,assignment.ends_on FROM learning_group_teachers assignment JOIN academic_timetable_block_group_instructors instructor ON instructor.instructor_id=assignment.teacher_id AND instructor.role=assignment.role WHERE instructor.id=$1 AND assignment.learning_group_id=$2 LIMIT 1")
                .bind(instructor).bind(group).execute(&mut *tx).await.unwrap();
            sqlx::query("ALTER TABLE learning_group_teachers ENABLE TRIGGER learning_group_teachers_interval_guard").execute(&mut *tx).await.unwrap();
            sqlx::query("ALTER TABLE learning_group_teachers ENABLE TRIGGER learning_group_teachers_published_immutable").execute(&mut *tx).await.unwrap();
            tx.commit().await.unwrap();
        } else {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query("ALTER TABLE academic_timetable_block_group_instructors DISABLE TRIGGER academic_timetable_block_group_instructors_version_immutable").execute(&mut *tx).await.unwrap();
            sqlx::query("UPDATE academic_timetable_block_group_instructors SET role=CASE WHEN role='primary' THEN 'secondary' ELSE 'primary' END WHERE id=$1")
                .bind(instructor).execute(&mut *tx).await.unwrap();
            sqlx::query("ALTER TABLE academic_timetable_block_group_instructors ENABLE TRIGGER academic_timetable_block_group_instructors_version_immutable").execute(&mut *tx).await.unwrap();
            tx.commit().await.unwrap();
        }
        apply_migrations_through(&pool, 86).await.unwrap();
        let error = apply_migrations_through(&pool, 87).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("DELIVERY_VERSION_HISTORICAL_INSTRUCTOR_UNMAPPABLE"),
            "{error}"
        );
        let mapped: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_delivery_versions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(mapped, 0);
        let legacy: bool = sqlx::query_scalar(
            "SELECT to_regclass('academic_timetable_version_targets') IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(legacy);
    }
}

async fn publication_preview(
    pool: &sqlx::PgPool,
    id: uuid::Uuid,
) -> Result<school_academic_delivery::models::AcademicTermChangeSetPreview, school_errors::AppError>
{
    let revision =
        school_academic_delivery::services::change_sets::get_change_set(pool, id).await?;
    school_academic_delivery::services::change_sets::preview_change_set_at(
        pool,
        id,
        Some(revision.reference_date),
    )
    .await
}

#[tokio::test]
async fn zero_course_periods_preserve_delivery_and_require_removing_existing_lessons() {
    use chrono::Duration;
    use school_academic_delivery::models::{
        CreateAcademicTermChangeSetRequest, PublishAcademicTermChangeSetRequest,
        UpsertAcademicTermChangeItemRequest,
    };
    use school_academic_delivery::services::change_sets;
    use school_academic_timetable::models::timetable_version::{
        CloneTimetableVersionRequest, UpdateTimetableDeliverySourceRequest,
    };
    use school_academic_timetable::services::{
        timetable_lifecycle, timetable_version_service as tables,
    };
    let pool = predecessor("zero_course_periods").await;
    apply_migrations_through(&pool, 96).await.unwrap();
    let (source_id,actor): (Uuid,Uuid)=sqlx::query_as("SELECT version.id,version.published_by FROM academic_timetable_versions version WHERE status='published' AND EXISTS(SELECT 1 FROM academic_timetable_blocks block WHERE block.timetable_version_id=version.id) ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap();
    let source = tables::get_version(&pool, source_id, chrono::Utc::now().date_naive())
        .await
        .unwrap();
    let base = versions::get_version(&pool, source.delivery_version_id)
        .await
        .unwrap();
    let draft = tables::clone_draft(
        &pool,
        actor,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    let before: Vec<(Uuid, i64)> =
        sqlx::query_as("SELECT id,row_version FROM academic_timetable_blocks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    // Exercise an active lifecycle with the preserved historical fixture IDs.
    // Publication dates follow the current clock rather than a stale fixture date.
    let publish_date = std::cmp::max(
        chrono::Utc::now().date_naive(),
        base.effective_from.unwrap() + Duration::days(1),
    );
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$2) WHERE id=$1")
        .bind(source.academic_year_id)
        .bind(publish_date + Duration::days(30))
        .execute(&pool)
        .await
        .unwrap();
    let revision = change_sets::create_change_set(
        &pool,
        actor,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: source.academic_term_id,
            reason: "ปรับจำนวนคาบ โดยยังไม่จัดตารางใหม่".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let group_id: Uuid = sqlx::query_scalar(
        "SELECT placed.learning_group_id FROM academic_timetable_block_groups placed
        JOIN academic_timetable_blocks block ON block.id=placed.block_id
        WHERE block.timetable_version_id=$1 AND block.is_active AND placed.is_active
          AND block.block_kind='COURSE' ORDER BY block.id,placed.id LIMIT 1",
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let offering = base
        .snapshot
        .offerings
        .iter()
        .find(|offering| offering.groups.iter().any(|group| group.id == group_id))
        .unwrap();
    let rejected = change_sets::upsert_change_item(
        &pool,
        actor,
        revision.id,
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            change_set_row_version: revision.row_version,
            item_row_version: None,
            learning_offering_id: offering.id,
            weekly_period_target: -1,
        },
    )
    .await
    .unwrap_err();
    assert!(rejected.to_string().contains("ศูนย์"));
    let updated = change_sets::upsert_change_item(
        &pool,
        actor,
        revision.id,
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            change_set_row_version: revision.row_version,
            item_row_version: None,
            learning_offering_id: offering.id,
            weekly_period_target: 0,
        },
    )
    .await
    .unwrap();
    let zero_draft = versions::get_version(&pool, updated.target_delivery_version_id)
        .await
        .unwrap();
    let zero_offering = zero_draft
        .snapshot
        .offerings
        .iter()
        .find(|row| row.id == offering.id)
        .unwrap();
    assert_eq!(zero_offering.weekly_period_target, 0);
    assert_eq!(
        serde_json::to_value(&zero_offering.catalog).unwrap(),
        serde_json::to_value(&offering.catalog).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&zero_offering.groups).unwrap(),
        serde_json::to_value(&offering.groups).unwrap()
    );
    let constraint = sqlx::query("UPDATE academic_term_change_items SET weekly_period_target=-1 WHERE change_set_id=$1 AND learning_offering_id=$2")
        .bind(updated.id).bind(offering.id).execute(&pool).await.unwrap_err();
    assert!(constraint
        .to_string()
        .contains("academic_term_change_items_weekly_period_target_check"));
    let preview = publication_preview(&pool, revision.id).await.unwrap();
    assert!(
        preview.findings.iter().all(|finding| finding.severity
            != school_academic_delivery::models::AcademicChangeFindingSeverity::Blocking),
        "delivery readiness must not demand placed lessons: {:?}",
        preview.findings
    );
    let receipt = change_sets::publish_change_set(
        &pool,
        actor,
        updated.id,
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
    let after: Vec<(Uuid, i64)> =
        sqlx::query_as("SELECT id,row_version FROM academic_timetable_blocks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        source.delivery_version_id,
        tables::get_version(&pool, source.id, chrono::Utc::now().date_naive())
            .await
            .unwrap()
            .delivery_version_id
    );
    assert_eq!(
        draft.delivery_version_id,
        tables::get_version(&pool, draft.id, chrono::Utc::now().date_naive())
            .await
            .unwrap()
            .delivery_version_id,
        "a running draft never silently upgrades"
    );
    let reloaded = tables::get_version(&pool, draft.id, chrono::Utc::now().date_naive())
        .await
        .unwrap();
    let updated = timetable_lifecycle::update_source(
        &pool,
        actor,
        draft.id,
        UpdateTimetableDeliverySourceRequest {
            row_version: reloaded.row_version,
            delivery_version_id: receipt.target_delivery_version_id,
        },
    )
    .await
    .unwrap();
    assert_ne!(updated.delivery_version_id, source.delivery_version_id);
    assert_eq!(
        before,
        sqlx::query_as::<_, (Uuid, i64)>(
            "SELECT id,row_version FROM academic_timetable_blocks ORDER BY id"
        )
        .fetch_all(&pool)
        .await
        .unwrap()
    );
    let base_after = versions::get_version(&pool, base.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(&base.snapshot).unwrap(),
        serde_json::to_value(base_after.snapshot).unwrap()
    );

    use school_academic_timetable::{
        models::{timetable_block::*, timetable_publication::*},
        services::timetable_block_service as blocks,
    };
    let workspace = blocks::get_workspace(
        &pool,
        TimetableBlockWorkspaceQuery {
            academic_year_id: source.academic_year_id,
            academic_term_id: source.academic_term_id,
            timetable_version_id: draft.id,
        },
        &school_academic_timetable::policy::TimetableAccessFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let demand = workspace
        .ordinary_demands
        .iter()
        .find(|demand| demand.learning_group_id == group_id)
        .unwrap();
    assert_eq!(demand.required_periods, 0);
    assert_eq!(demand.remaining_periods, 0);
    assert!(
        demand.scheduled_periods > 0,
        "existing lessons preserved for explicit removal"
    );
    // Positive targets still require an exact count; zero does not bypass the rule.
    let mut positive_source = zero_draft.snapshot.clone();
    let positive_offering = positive_source
        .offerings
        .iter_mut()
        .find(|row| row.id == offering.id)
        .unwrap();
    positive_offering.weekly_period_target = demand.scheduled_periods;
    assert!(
        !timetable_lifecycle::readiness(&positive_source, &workspace.blocks)
            .iter()
            .any(|finding| finding.learning_group_id == Some(group_id)
                && finding.code == TimetablePublicationFindingCode::PeriodCountMismatch)
    );
    positive_source
        .offerings
        .iter_mut()
        .find(|row| row.id == offering.id)
        .unwrap()
        .weekly_period_target += 1;
    assert!(
        timetable_lifecycle::readiness(&positive_source, &workspace.blocks)
            .iter()
            .any(|finding| finding.learning_group_id == Some(group_id)
                && finding.code == TimetablePublicationFindingCode::PeriodCountMismatch)
    );
    let block = workspace
        .blocks
        .iter()
        .find(|block| {
            block
                .groups
                .iter()
                .any(|group| group.learning_group_id == group_id)
        })
        .unwrap();
    let create_error = blocks::create_ordinary_block(
        &pool,
        actor,
        CreateOrdinaryTimetableBlockRequest {
            timetable_version_id: draft.id,
            academic_term_id: source.academic_term_id,
            learning_group_id: group_id,
            day_of_week: block.day_of_week.clone(),
            bell_schedule_period_id: block.bell_schedule_period_id,
            room_id: None,
            instructor_ids: offering
                .groups
                .iter()
                .find(|group| group.id == group_id)
                .unwrap()
                .teachers
                .iter()
                .map(|teacher| teacher.teacher_id)
                .collect(),
            note: None,
        },
    )
    .await
    .unwrap_err();
    assert!(create_error.to_string().contains("0 คาบ"));
    let preview = timetable_lifecycle::preview(
        &pool,
        draft.id,
        PreviewTimetablePublicationRequest {
            row_version: updated.row_version,
            effective_from: publish_date,
        },
    )
    .await
    .unwrap();
    assert!(preview
        .findings
        .iter()
        .any(|finding| finding.learning_offering_id == Some(offering.id)
            && finding.message.contains("ถอดคาบเดิม")));
    for block in workspace.blocks.iter().filter(|block| {
        block.is_active
            && block
                .groups
                .iter()
                .any(|group| group.learning_offering_id == offering.id && group.is_active)
    }) {
        let target = block
            .groups
            .iter()
            .find(|group| group.learning_offering_id == offering.id && group.is_active)
            .unwrap();
        blocks::remove_target(
            &pool,
            actor,
            block.id,
            RemoveTimetableBlockTargetRequest {
                timetable_version_id: draft.id,
                block_row_version: block.row_version,
                target_kind: TimetableTargetKind::Group,
                target_id: target.id,
                target_row_version: target.row_version,
            },
        )
        .await
        .unwrap();
    }
    let reloaded = tables::get_version(&pool, draft.id, chrono::Utc::now().date_naive())
        .await
        .unwrap();
    let preview = timetable_lifecycle::preview(
        &pool,
        draft.id,
        PreviewTimetablePublicationRequest {
            row_version: reloaded.row_version,
            effective_from: publish_date,
        },
    )
    .await
    .unwrap();
    assert!(!preview
        .findings
        .iter()
        .any(|finding| finding.learning_offering_id == Some(offering.id)
            && finding.code == TimetablePublicationFindingCode::PeriodCountMismatch));
}
