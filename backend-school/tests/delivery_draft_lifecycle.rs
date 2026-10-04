#[path = "../src/bin/cleanup_cancelled_academic_versions/cleanup.rs"]
mod cleanup_service;
#[path = "../src/modules/academic/cutover_test_preflight.rs"]
pub mod cutover_test_preflight;
#[path = "../src/modules/academic/cutover_test_support.rs"]
pub mod cutover_test_support;
use cutover_test_support::{apply_migrations_through, seed_release_two_predecessor};
use school_academic_delivery::{
    models::{versions::DeleteDeliveryVersionRequest, CreateAcademicTermChangeSetRequest},
    services::{change_sets, versions},
};
use school_academic_timetable::{
    models::timetable_version::{CloneTimetableVersionRequest, DeleteTimetableDraftRequest},
    services::{timetable_lifecycle, timetable_version_service as tables},
};
use school_authorization::ActorContext;
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(name: &str, version: i64) -> PgPool {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 5).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, version).await.unwrap();
    pool
}
async fn context(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    sqlx::query_as("SELECT id,academic_year_id,academic_term_id,published_by FROM academic_timetable_versions WHERE status='published' AND EXISTS(SELECT 1 FROM academic_timetable_blocks WHERE timetable_version_id=academic_timetable_versions.id) ORDER BY id LIMIT 1").fetch_one(pool).await.unwrap()
}
async fn cancel_table(pool: &PgPool, source_id: Uuid, actor: Uuid) -> Uuid {
    let source = tables::get_version(pool, source_id, chrono::Utc::now().date_naive())
        .await
        .unwrap();
    let draft = tables::clone_draft(
        pool,
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
    sqlx::query("UPDATE academic_timetable_versions SET status='cancelled',row_version=row_version+1 WHERE id=$1").bind(draft.id).execute(pool).await.unwrap();
    draft.id
}
async fn cancel_opening(pool: &PgPool, term: Uuid, actor: Uuid) -> Uuid {
    let draft = change_sets::create_change_set(
        pool,
        actor,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: term,
            reason: "legacy cancelled fixture".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE academic_term_change_sets SET status='cancelled',cancelled_by=$2,cancelled_at=now(),row_version=row_version+1 WHERE id=$1").bind(draft.id).bind(actor).execute(pool).await.unwrap();
    sqlx::query("UPDATE academic_delivery_versions SET status='cancelled',row_version=row_version+1 WHERE id=$1").bind(draft.target_delivery_version_id).execute(pool).await.unwrap();
    draft.target_delivery_version_id
}
fn actor(id: Uuid) -> ActorContext {
    ActorContext {
        user_id: id,
        permissions: vec![
            codes::LEARNING_OFFERING_MANAGE_SCHOOL.into(),
            codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL.into(),
        ],
    }
}

#[tokio::test]
async fn migration_preserves_published_graphs_and_moves_only_draft_dates() {
    let pool = fixture("delivery_dates_migration", 88).await;
    let (_, year, term, actor) = context(&pool).await;
    let (source,date):(Uuid,chrono::NaiveDate)=sqlx::query_as("SELECT id,effective_from FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC LIMIT 1").bind(term).fetch_one(&pool).await.unwrap();
    let draft = Uuid::new_v4();
    let journal = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,source_version_id,effective_from,snapshot,created_by) SELECT $1,academic_term_id,academic_year_id,id,effective_from+1,snapshot,$2 FROM academic_delivery_versions WHERE id=$3").bind(draft).bind(actor).bind(source).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_term_change_sets(id,academic_term_id,academic_year_id,base_delivery_version_id,target_delivery_version_id,effective_from,reason,idempotency_key,creation_request_hash,created_by) VALUES($1,$2,$3,$4,$5,$6,'date migration fixture',$1::text,repeat('a',64),$7)").bind(journal).bind(term).bind(year).bind(source).bind(draft).bind(date.succ_opt()).bind(actor).execute(&pool).await.unwrap();
    let before:String=sqlx::query_scalar("SELECT md5(jsonb_agg(jsonb_build_object('id',id,'snapshot',snapshot,'date',effective_from,'revision',row_version) ORDER BY id)::text) FROM academic_delivery_versions WHERE status='published'").fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 89).await.unwrap();
    let after:String=sqlx::query_scalar("SELECT md5(jsonb_agg(jsonb_build_object('id',id,'snapshot',snapshot,'date',effective_from,'revision',row_version) ORDER BY id)::text) FROM academic_delivery_versions WHERE status='published'").fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    let version = versions::get_version(&pool, draft).await.unwrap();
    assert_eq!(version.effective_from, None);
    assert_eq!(version.reference_date, date.succ_opt().unwrap());
    assert_eq!(version.row_version, 1);
    let changed = change_sets::get_change_set(&pool, journal).await.unwrap();
    assert_eq!(changed.effective_from, None);
    assert_eq!(changed.reference_date, version.reference_date);
    assert!(
        sqlx::query("DELETE FROM academic_delivery_versions WHERE id=$1")
            .bind(source)
            .execute(&pool)
            .await
            .unwrap_err()
            .to_string()
            .contains("IMMUTABLE")
    );
}

#[tokio::test]
async fn cancelled_cleanup_preflights_all_and_keeps_active_drafts_and_published_rows() {
    let pool = fixture("cancelled_cleanup_atomic", 89).await;
    let (source, year, term, user) = context(&pool).await;
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    let table = cancel_table(&pool, source, user).await;
    let opening = cancel_opening(&pool, term, user).await;
    assert!(
        sqlx::query("UPDATE academic_timetable_versions SET status='draft' WHERE id=$1")
            .bind(table)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("UPDATE academic_timetable_blocks SET note='cannot edit cancelled' WHERE timetable_version_id=$1").bind(table).execute(&pool).await.is_err());
    let active = change_sets::create_change_set(
        &pool,
        user,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: term,
            reason: "keep active draft".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let first = cleanup_service::cleanup(&pool, &actor(user), year, term, None)
        .await
        .unwrap();
    assert_eq!((first.timetable_versions, first.delivery_versions), (1, 1));
    assert!(!first.applied);
    let table_row = tables::get_version(&pool, table, chrono::Utc::now().date_naive())
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=$1",
    )
    .bind(table)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(count > 0);
    let blocked = timetable_lifecycle::delete_draft(
        &pool,
        user,
        source,
        DeleteTimetableDraftRequest {
            row_version: 1,
            expected_block_count: 0,
        },
    )
    .await
    .unwrap_err();
    assert!(blocked.to_string().contains("ลบได้เฉพาะ"));
    // A newly observed external FK causes a rollback of every preceding deletion and audit.
    sqlx::query("CREATE TABLE cleanup_external_reference(version_id UUID REFERENCES academic_delivery_versions(id) ON DELETE RESTRICT)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO cleanup_external_reference VALUES($1)")
        .bind(opening)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        cleanup_service::cleanup(&pool, &actor(user), year, term, Some(&first.preview_hash))
            .await
            .is_err()
    );
    assert!(
        tables::get_version(&pool, table, chrono::Utc::now().date_naive())
            .await
            .is_ok()
    );
    assert!(versions::get_version(&pool, opening).await.is_ok());
    assert_eq!(
        tables::get_version(&pool, table, chrono::Utc::now().date_naive())
            .await
            .unwrap()
            .row_version,
        table_row.row_version
    );
    sqlx::query("DROP TABLE cleanup_external_reference")
        .execute(&pool)
        .await
        .unwrap();
    let result =
        cleanup_service::cleanup(&pool, &actor(user), year, term, Some(&first.preview_hash))
            .await
            .unwrap();
    assert!(result.applied);
    assert_eq!(result.published_fingerprint, first.published_fingerprint);
    assert!(
        tables::get_version(&pool, table, chrono::Utc::now().date_naive())
            .await
            .is_err()
    );
    assert!(versions::get_version(&pool, opening).await.is_err());
    assert!(change_sets::get_change_set(&pool, active.id).await.is_ok());
    assert!(
        tables::get_version(&pool, source, chrono::Utc::now().date_naive())
            .await
            .is_ok()
    );
    let again = cleanup_service::cleanup(&pool, &actor(user), year, term, None)
        .await
        .unwrap();
    assert_eq!((again.timetable_versions, again.delivery_versions), (0, 0));
    let mut read_only = actor(user);
    read_only.permissions = vec![
        codes::LEARNING_OFFERING_READ_SCHOOL.into(),
        codes::ACADEMIC_TIMETABLE_READ_SCHOOL.into(),
    ];
    assert!(
        cleanup_service::cleanup(&pool, &read_only, year, term, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn cancelled_opening_is_immutable_until_hard_deleted_and_stale_revision_is_rejected() {
    let pool = fixture("cancelled_opening_guards", 89).await;
    let (_, _, term, user) = context(&pool).await;
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    let id = cancel_opening(&pool, term, user).await;
    assert!(
        sqlx::query("UPDATE academic_delivery_versions SET status='draft' WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap_err()
            .to_string()
            .contains("IMMUTABLE")
    );
    let version = versions::get_version(&pool, id).await.unwrap();
    let (journal,revision,items):(Uuid,i64,i64)=sqlx::query_as("SELECT journal.id,journal.row_version,(SELECT count(*) FROM academic_term_change_items WHERE change_set_id=journal.id) FROM academic_term_change_sets journal WHERE target_delivery_version_id=$1").bind(id).fetch_one(&pool).await.unwrap();
    let request = DeleteDeliveryVersionRequest {
        row_version: version.row_version,
        change_set_row_version: revision,
        expected_item_count: items,
        expected_offering_count: version.snapshot.offerings.len() as i64,
        expected_group_count: version
            .snapshot
            .offerings
            .iter()
            .map(|o| o.groups.len() as i64)
            .sum(),
    };
    let mut stale = request.clone();
    stale.row_version -= 1;
    assert!(change_sets::delete_version(&pool, user, id, stale)
        .await
        .is_err());
    change_sets::delete_version(&pool, user, id, request)
        .await
        .unwrap();
    assert!(versions::get_version(&pool, id).await.is_err());
    assert!(change_sets::get_change_set(&pool, journal).await.is_err());
}
