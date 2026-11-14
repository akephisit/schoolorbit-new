//! Historical 089 date migration and cancelled-resource SQL guards.
use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
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
    let (version_effective_from,version_reference_date,version_row_version):(Option<chrono::NaiveDate>,chrono::NaiveDate,i64)=sqlx::query_as(
        "SELECT effective_from,reference_date,row_version FROM academic_delivery_versions WHERE id=$1",
    ).bind(draft).fetch_one(&pool).await.unwrap();
    assert_eq!(version_effective_from, None);
    assert_eq!(version_reference_date, date.succ_opt().unwrap());
    assert_eq!(version_row_version, 1);
    // Assert the 089 journal contract directly: current hydration requires later schema views.
    let (effective_from, reference_date): (Option<chrono::NaiveDate>, chrono::NaiveDate) =
        sqlx::query_as(
            "SELECT effective_from,reference_date FROM academic_term_change_sets WHERE id=$1",
        )
        .bind(journal)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(effective_from, None);
    assert_eq!(reference_date, version_reference_date);
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
async fn historical_cancelled_delivery_timetable_and_children_are_immutable_at_089() {
    let pool = fixture("cancelled_089_sql_guards", 89).await;
    let (source, _, term, actor) = context(&pool).await;
    let timetable = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_timetable_versions SELECT (jsonb_populate_record(NULL::academic_timetable_versions,to_jsonb(v)||jsonb_build_object('id',$2::uuid,'source_version_id',v.id,'status','draft','effective_from',NULL,'reference_date',v.effective_from,'published_by',NULL,'published_at',NULL,'publication_idempotency_key',NULL,'publication_request_hash',NULL,'created_by',$3::uuid,'row_version',1))).* FROM academic_timetable_versions v WHERE id=$1").bind(source).bind(timetable).bind(actor).execute(&pool).await.unwrap();
    let block = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_timetable_blocks SELECT (jsonb_populate_record(NULL::academic_timetable_blocks,to_jsonb(b)||jsonb_build_object('id',$2::uuid,'timetable_version_id',$3::uuid))).* FROM academic_timetable_blocks b WHERE timetable_version_id=$1 ORDER BY id LIMIT 1").bind(source).bind(block).bind(timetable).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_timetable_versions SET status='cancelled',row_version=row_version+1 WHERE id=$1").bind(timetable).execute(&pool).await.unwrap();
    let delivery = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_delivery_versions SELECT (jsonb_populate_record(NULL::academic_delivery_versions,to_jsonb(v)||jsonb_build_object('id',$2::uuid,'source_version_id',v.id,'status','draft','effective_from',NULL,'reference_date',v.effective_from,'published_by',NULL,'published_at',NULL,'publication_idempotency_key',NULL,'publication_request_hash',NULL,'created_by',$3::uuid,'row_version',1))).* FROM academic_delivery_versions v WHERE academic_term_id=$1 AND status='published' ORDER BY id LIMIT 1").bind(term).bind(delivery).bind(actor).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_delivery_versions SET status='cancelled',row_version=row_version+1 WHERE id=$1").bind(delivery).execute(&pool).await.unwrap();
    let before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('table',(SELECT to_jsonb(v) FROM academic_timetable_versions v WHERE id=$1),'block',(SELECT to_jsonb(b) FROM academic_timetable_blocks b WHERE id=$2),'delivery',(SELECT to_jsonb(v) FROM academic_delivery_versions v WHERE id=$3))").bind(timetable).bind(block).bind(delivery).fetch_one(&pool).await.unwrap();
    for (statement, id) in [
        (
            "UPDATE academic_timetable_versions SET status='draft' WHERE id=$1",
            timetable,
        ),
        (
            "UPDATE academic_timetable_blocks SET note='cannot edit cancelled' WHERE id=$1",
            block,
        ),
        (
            "UPDATE academic_delivery_versions SET status='draft' WHERE id=$1",
            delivery,
        ),
    ] {
        let error = sqlx::query(statement)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
        assert!(error
            .as_database_error()
            .unwrap()
            .message()
            .contains("IMMUTABLE"));
    }
    let after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('table',(SELECT to_jsonb(v) FROM academic_timetable_versions v WHERE id=$1),'block',(SELECT to_jsonb(b) FROM academic_timetable_blocks b WHERE id=$2),'delivery',(SELECT to_jsonb(v) FROM academic_delivery_versions v WHERE id=$3))").bind(timetable).bind(block).bind(delivery).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&pool)
            .await
            .unwrap(),
        89
    );
}
