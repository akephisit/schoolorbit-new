//! Immutable 087–089 migration contracts and current zero-workload behavior.
#[path = "../src/adapters/lesson_sources.rs"]
mod lesson_sources;
use school_test_db::create_named_test_pool_with_max_connections;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};

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

// Historical JSON is intentionally untyped: no retired runtime DTO is retained.
async fn cutover_snapshot(pool: &PgPool, id: Uuid) -> Value {
    sqlx::query_scalar("SELECT snapshot FROM academic_delivery_versions WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn cutover_complete(pool: &PgPool) -> bool {
    let rows:Vec<(i32,bool,bool,Value)>=sqlx::query_as("SELECT migration_version,passed,cutover_completed,checks FROM academic_delivery_version_migration_audit WHERE migration_version IN(87,88) ORDER BY migration_version").fetch_all(pool).await.unwrap();
    rows.len() == 2
        && rows.iter().map(|r| r.0).eq([87, 88])
        && rows.iter().all(|(_, passed, completed, checks)| {
            let checks = checks.as_array().unwrap();
            *passed
                && *completed
                && checks.len() == 15
                && checks
                    .iter()
                    .all(|c| c["passed"] == true && c["count"].as_i64().unwrap() >= 0)
                && checks
                    .iter()
                    .map(|c| c["code"].as_str().unwrap())
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == 15
        })
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
    let deliveries: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 ORDER BY id",
    )
    .bind(term_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        deliveries.len(),
        1,
        "only positions changed, so both tables share a graph"
    );
    let historical_snapshot = cutover_snapshot(&pool, deliveries[0]).await;
    assert!(
        !historical_snapshot["offerings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        historical_snapshot["offerings"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|offering| offering["groups"].as_array().unwrap())
            .all(|group| !group["teachers"].as_array().unwrap().is_empty())
    );
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_versions WHERE delivery_version_id=$1",
    )
    .bind(deliveries[0])
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
            .bind(deliveries[0])
            .execute(&pool)
            .await
            .unwrap_err();
    assert!(
        immutable
            .to_string()
            .contains("ACADEMIC_DELIVERY_VERSION_IMMUTABLE")
    );
    apply_migrations_through(&pool, 89)
        .await
        .expect("canonical cleanup has fresh evidence");
    let canonical: bool=sqlx::query_scalar("SELECT cutover_completed AND to_regclass('academic_timetable_version_targets') IS NULL FROM academic_delivery_version_migration_audit WHERE migration_version=87")
        .fetch_one(&pool).await.unwrap();
    assert!(canonical);
    assert!(cutover_complete(&pool).await);
    let checks:i64=sqlx::query_scalar("SELECT sum(jsonb_array_length(checks))::bigint FROM academic_delivery_version_migration_audit WHERE migration_version IN(87,88)").fetch_one(&pool).await.unwrap();
    assert_eq!(checks, 30);
    sqlx::query("UPDATE academic_delivery_version_migration_audit SET passed=false WHERE migration_version=87").execute(&pool).await.unwrap();
    assert!(!cutover_complete(&pool).await);
    sqlx::query("DELETE FROM academic_delivery_version_migration_audit WHERE migration_version=87")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!cutover_complete(&pool).await);
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
    assert!(
        failure
            .to_string()
            .contains("DELIVERY_VERSION_SOURCE_UNMAPPABLE")
    );
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
    assert!(
        failure
            .to_string()
            .contains("DELIVERY_VERSION_FRESH_RECONCILIATION_REQUIRED")
    );
    let legacy_still_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('academic_timetable_version_targets') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(legacy_still_exists);
    sqlx::query("UPDATE academic_delivery_version_migration_audit SET passed=false WHERE migration_version=87").execute(&pool).await.unwrap();
    let failure = apply_migrations_through(&pool, 88).await.unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("DELIVERY_VERSION_FRESH_RECONCILIATION_REQUIRED")
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
    assert!(
        error
            .to_string()
            .contains("DELIVERY_DRAFT_TARGETS_UNMAPPABLE")
    );
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
    assert!(
        error
            .to_string()
            .contains("DELIVERY_VERSION_HISTORICAL_GROUP_UNMAPPABLE")
    );
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
    let source = cutover_snapshot(&pool, pinned).await;
    let pending = cutover_snapshot(&pool, opening_draft).await;
    let source_id: Option<Uuid> =
        sqlx::query_scalar("SELECT source_version_id FROM academic_delivery_versions WHERE id=$1")
            .bind(opening_draft)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(source_id, Some(pinned));
    let original = source["offerings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == json!(offering))
        .unwrap();
    let changed = pending["offerings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == json!(offering))
        .unwrap();
    assert_eq!(
        changed["weeklyPeriodTarget"].as_i64().unwrap(),
        original["weeklyPeriodTarget"].as_i64().unwrap() + 1
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
    assert!(
        !cutover_snapshot(&pool, pinned).await["offerings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|offering| offering["id"] == json!(added))
    );
    assert!(
        cutover_snapshot(&pool, pending).await["offerings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|offering| offering["id"] == json!(added))
    );
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
    assert!(cutover_complete(&pool).await);
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
    assert!(
        cutover_snapshot(&pool, source).await["offerings"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|offering| offering["groups"].as_array().unwrap())
            .any(|item| item["id"] == json!(group))
    );
    assert!(cutover_complete(&pool).await);
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
    let source = cutover_snapshot(&pool, source).await;
    let imported = source["offerings"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|offering| offering["groups"].as_array().unwrap())
        .find(|item| item["id"] == json!(group))
        .unwrap();
    assert_eq!(
        imported["teachers"].as_array().unwrap().len(),
        assignments.len()
    );
    for (id, teacher, role) in assignments {
        assert!(imported["teachers"].as_array().unwrap().iter().any(
            |assignment| assignment["assignmentId"] == json!(id)
                && assignment["teacherId"] == json!(teacher)
                && assignment["role"] == json!(role)
        ));
    }
    let after: Vec<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT id,block_id,learning_group_id FROM academic_timetable_block_groups ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    assert!(cutover_complete(&pool).await);
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

#[path = "../src/adapters/staff_directory.rs"]
mod staff_directory;

#[tokio::test]
async fn zero_course_periods_preserve_current_facts_until_explicit_placement_removal() {
    current_zero_periods("course").await;
}
#[tokio::test]
async fn zero_independent_activity_periods_preserve_current_facts_until_explicit_placement_removal()
{
    current_zero_periods("independent").await;
}
#[tokio::test]
async fn zero_synchronized_activity_periods_preserve_current_facts_until_explicit_placement_removal()
 {
    current_zero_periods("synchronized").await;
}

async fn current_zero_periods(mode: &str) {
    use school_academic_delivery::{
        models::current::UpdateWeeklyPeriodTargetRequest,
        services::{current, offerings},
    };
    use school_academic_timetable::{
        collaboration::TimetableMutationContext,
        models::timetable_block::TimetableBlockWorkspaceQuery,
        policy::TimetableAccessFilter,
        services::{timetable_block_service as blocks, timetable_service},
    };
    let pool = if mode == "course" {
        predecessor("zero_current_course").await
    } else {
        let pool =
            create_named_test_pool_with_max_connections(&format!("zero_current_{mode}"), 1).await;
        apply_migrations_through(&pool, 40).await.unwrap();
        school_test_db::cutover_test_support::seed_academic_cutover_fixture(
            &pool,
            school_test_db::cutover_test_support::CutoverFixture::Passing,
        )
        .await
        .unwrap();
        sqlx::query("INSERT INTO academic_timetable_entries(id,day_of_week,period_id,entry_type,classroom_id,academic_semester_id,activity_slot_id,created_by) SELECT gen_random_uuid(),CASE WHEN activity.scheduling_mode='synchronized' THEN 'TUE' ELSE 'WED' END,'12000000-0000-0000-0000-000000000001','ACTIVITY',coverage.classroom_id,slot.semester_id,slot.id,slot.created_by FROM activity_slots slot JOIN activity_catalog activity ON activity.id=slot.activity_catalog_id JOIN activity_slot_classrooms coverage ON coverage.slot_id=slot.id").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO timetable_entry_instructors(id,entry_id,instructor_id,role) SELECT gen_random_uuid(),id,created_by,'primary' FROM academic_timetable_entries WHERE entry_type='ACTIVITY'").execute(&pool).await.unwrap();
        apply_migrations_through(&pool, 44).await.unwrap();
        school_test_db::cutover_test_support::record_passing_phase_a_reconciliation_marker(&pool)
            .await
            .unwrap();
        apply_migrations_through(&pool, 85).await.unwrap();
        pool
    };
    apply_migrations_through(&pool, 157).await.unwrap();
    let (table,block,group,offering,term,year,actor):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT t.id,b.id,g.learning_group_id,g.learning_offering_id,t.academic_term_id,t.academic_year_id,(SELECT id FROM users WHERE user_type='staff' AND status='active' ORDER BY id LIMIT 1) FROM academic_timetables t JOIN academic_terms term ON term.id=t.academic_term_id JOIN academic_timetable_blocks b ON b.timetable_id=t.id JOIN academic_timetable_block_groups g ON g.block_id=b.id WHERE term.status='active' AND (($1='course' AND b.block_kind='COURSE') OR (b.block_kind='ACTIVITY' AND b.scheduling_mode=$1)) ORDER BY b.id,g.id LIMIT 1").bind(mode).fetch_one(&pool).await.unwrap();
    let (student,student_year):(Uuid,Uuid)=sqlx::query_as("SELECT student_id,id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 1").bind(year).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_students(id,learning_group_id,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at) SELECT uuidv7(),$1,$2,$3,$4,$5,'active','manual','2025-05-01' WHERE NOT EXISTS(SELECT 1 FROM learning_group_students WHERE learning_group_id=$1 AND student_id=$5 AND membership_status='active')").bind(group).bind(term).bind(year).bind(student_year).bind(student).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role) SELECT uuidv7(),$1,$2,$3,$4,'primary' WHERE NOT EXISTS(SELECT 1 FROM learning_group_teachers WHERE learning_group_id=$1)").bind(group).bind(term).bind(year).bind(actor).execute(&pool).await.unwrap();
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time,roster) VALUES($1,$2,'2025-08-25','lesson',$3,'Saved zero-target lesson',ARRAY[$4]::uuid[],$5,$6,'08:00','09:00',jsonb_build_array(jsonb_build_object('studentId',$7::uuid,'studentAcademicYearId',$8::uuid)))").bind(session).bind(term).bind(session.to_string()).bind(actor).bind(group).bind(offering).bind(student).bind(student_year).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin) VALUES($1,$2,$3,'present','teacher')").bind(session).bind(student).bind(student_year).execute(&pool).await.unwrap();
    if mode == "course" {
        let item = Uuid::new_v4();
        sqlx::query("INSERT INTO learning_group_score_items(id,learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) SELECT $1,$2,$3,p.id,phase.id,$4,$5,'Saved zero-target score',100 FROM course_assessment_plans p JOIN course_assessment_phases phase ON phase.plan_id=p.id WHERE p.learning_offering_id=$3 ORDER BY phase.id LIMIT 1").bind(item).bind(group).bind(offering).bind(term).bind(year).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO learning_group_student_scores(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score) VALUES($1,$2,$3,$4,$5,$6,80)").bind(group).bind(offering).bind(term).bind(year).bind(item).bind(student_year).execute(&pool).await.unwrap();
    } else {
        sqlx::query("INSERT INTO academic_activity_evaluations(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,student_academic_year_id,outcome,evidence) VALUES($1,$2,$3,$4,$5,'pass','{\"fixture\":\"saved-zero-target-evaluation\"}') ON CONFLICT(learning_group_id,student_academic_year_id) DO NOTHING").bind(group).bind(offering).bind(term).bind(year).bind(student_year).execute(&pool).await.unwrap();
    }
    if mode == "course" {
        sqlx::query("WITH accepted AS (INSERT INTO academic_course_result_locks(subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by) SELECT detail.subject_id,$2,$3,policy.id,'{}',repeat('a',64),repeat('b',64),jsonb_build_object('groups',jsonb_build_array(jsonb_build_object('learningGroupId',$4::uuid,'learningOfferingId',$1::uuid))),$5 FROM course_offering_details detail CROSS JOIN academic_grading_policy_versions policy WHERE detail.learning_offering_id=$1 ORDER BY policy.id LIMIT 1 RETURNING *) INSERT INTO academic_course_results(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,course_result_lock_id,student_academic_year_id,calculated_score,calculated_grade,outcome,numeric_grade,selection_source) SELECT $4,$1,$2,$3,subject_id,id,$6,80,4,'numeric',4,'derived' FROM accepted")
            .bind(offering).bind(term).bind(year).bind(group).bind(actor).bind(student_year).execute(&pool).await.unwrap();
    } else {
        sqlx::query("WITH accepted AS (INSERT INTO academic_activity_result_locks(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES($1,$2,$3,$4,repeat('a',64),repeat('b',64),'{}',$5) RETURNING id) INSERT INTO academic_activity_results(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,activity_result_lock_id,student_academic_year_id,outcome) SELECT $1,$2,$3,$4,id,$6,'pass' FROM accepted")
            .bind(group).bind(offering).bind(term).bind(year).bind(actor).bind(student_year).execute(&pool).await.unwrap();
    }
    let before = current::workspace(
        &pool,
        year,
        term,
        &school_authorization::AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let source = before.offerings.iter().find(|o| o.id == offering).unwrap();
    assert!(!source.groups.is_empty());
    assert!(source.groups.iter().any(|g| !g.teachers.is_empty()));
    let facts = zero_target_facts(&pool).await;
    for resource in [
        "groups",
        "teachers",
        "rosters",
        "attendance",
        "records",
        if mode == "course" {
            "scores"
        } else {
            "evaluations"
        },
        if mode == "course" {
            "courseResults"
        } else {
            "activityResults"
        },
    ] {
        assert!(
            !facts[resource].as_array().unwrap().is_empty(),
            "fixture needs actual saved {resource}"
        );
    }

    let placed:Value=sqlx::query_scalar("SELECT jsonb_build_object('blocks',(SELECT jsonb_agg(to_jsonb(b) ORDER BY id) FROM academic_timetable_blocks b WHERE timetable_id=$1),'groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY g.id) FROM academic_timetable_block_groups g JOIN academic_timetable_blocks b ON b.id=g.block_id WHERE b.timetable_id=$1),'instructors',(SELECT jsonb_agg(to_jsonb(i) ORDER BY i.id) FROM academic_timetable_block_group_instructors i JOIN academic_timetable_block_groups g ON g.id=i.block_group_id JOIN academic_timetable_blocks b ON b.id=g.block_id WHERE b.timetable_id=$1),'sync',(SELECT jsonb_agg(to_jsonb(s) ORDER BY s.id) FROM academic_timetable_block_group_sync s JOIN academic_timetable_blocks b ON b.id=s.block_id WHERE b.timetable_id=$1))").bind(table).fetch_one(&pool).await.unwrap();
    assert!(
        offerings::update_weekly_period_target(
            &pool,
            offering,
            UpdateWeeklyPeriodTargetRequest {
                row_version: source.row_version,
                weekly_period_target: -1
            }
        )
        .await
        .is_err()
    );
    assert_eq!(zero_target_facts(&pool).await, facts);
    offerings::update_weekly_period_target(
        &pool,
        offering,
        UpdateWeeklyPeriodTargetRequest {
            row_version: source.row_version,
            weekly_period_target: 0,
        },
    )
    .await
    .unwrap();
    let after = current::workspace(
        &pool,
        year,
        term,
        &school_authorization::AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let changed = after.offerings.iter().find(|o| o.id == offering).unwrap();
    let mut expected = serde_json::to_value(source).unwrap();
    expected["weeklyPeriodTarget"] = json!(0);
    expected["rowVersion"] = json!(source.row_version + 1);
    assert_eq!(serde_json::to_value(changed).unwrap(), expected);
    assert!(after.row_version > before.row_version);
    assert_eq!(zero_target_facts(&pool).await, facts);
    assert_eq!(placed,sqlx::query_scalar::<_,Value>("SELECT jsonb_build_object('blocks',(SELECT jsonb_agg(to_jsonb(b) ORDER BY id) FROM academic_timetable_blocks b WHERE timetable_id=$1),'groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY g.id) FROM academic_timetable_block_groups g JOIN academic_timetable_blocks b ON b.id=g.block_id WHERE b.timetable_id=$1),'instructors',(SELECT jsonb_agg(to_jsonb(i) ORDER BY i.id) FROM academic_timetable_block_group_instructors i JOIN academic_timetable_block_groups g ON g.id=i.block_group_id JOIN academic_timetable_blocks b ON b.id=g.block_id WHERE b.timetable_id=$1),'sync',(SELECT jsonb_agg(to_jsonb(s) ORDER BY s.id) FROM academic_timetable_block_group_sync s JOIN academic_timetable_blocks b ON b.id=s.block_id WHERE b.timetable_id=$1))").bind(table).fetch_one(&pool).await.unwrap());
    let workspace = blocks::get_workspace(
        &pool,
        TimetableBlockWorkspaceQuery {
            academic_year_id: year,
            academic_term_id: term,
            timetable_id: table,
        },
        &TimetableAccessFilter {
            includes_school_owned: true,
            ..Default::default()
        },
        &staff_directory::STAFF_DIRECTORY,
    )
    .await
    .unwrap();
    if mode == "synchronized" {
        let demand = workspace
            .synchronized_demands
            .iter()
            .find(|d| d.learning_offering_id == offering)
            .unwrap();
        assert_eq!(demand.required_periods, 0);
        assert!(demand.scheduled_periods > 0);
    } else {
        let demand = workspace
            .ordinary_demands
            .iter()
            .find(|d| d.learning_group_id == group)
            .unwrap();
        assert_eq!((demand.required_periods, demand.remaining_periods), (0, 0));
        assert!(demand.scheduled_periods > 0);
    }
    let version: i64 =
        sqlx::query_scalar("SELECT row_version FROM academic_timetable_blocks WHERE id=$1")
            .bind(block)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut mutation = TimetableMutationContext::for_tests(&pool, actor);
    mutation.lesson_source_detachment = &lesson_sources::LESSON_SOURCE_DETACHMENT;
    blocks::deactivate_block(&pool, &mutation, block, table, version)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_timetable_blocks WHERE id=$1")
            .bind(block)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_timetable_block_groups WHERE block_id=$1"
        )
        .bind(block)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        zero_target_facts(&pool).await,
        facts,
        "explicit placement removal preserves actual academic facts"
    );
    assert_eq!(
        timetable_service::get_for_term(&pool, term)
            .await
            .unwrap()
            .unwrap()
            .id,
        table
    );
}

async fn zero_target_facts(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('courseDetails',(SELECT jsonb_agg(to_jsonb(r) ORDER BY learning_offering_id) FROM course_offering_details r),'activityDetails',(SELECT jsonb_agg(to_jsonb(r) ORDER BY learning_offering_id) FROM activity_offering_details r),'courseLocks',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_course_result_locks r),'activityLocks',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_activity_result_locks r),'groups',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_groups r),'teachers',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_group_teachers r),'rosters',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_group_students r),'scores',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_group_student_scores r),'evaluations',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_activity_evaluations r),'courseResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_course_results r),'activityResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_activity_results r),'attendance',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM attendance_sessions r),'records',(SELECT jsonb_agg(to_jsonb(r) ORDER BY session_id,student_id) FROM attendance_records r))").fetch_one(pool).await.unwrap()
}
