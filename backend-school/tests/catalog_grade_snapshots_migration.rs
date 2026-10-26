//! Synthetic predecessor-to-141 rehearsal; no tenant database or migration history edits.
use school_test_db::cutover_test_support::{apply_migrations_through, seed_release_two_predecessor};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 140).await.unwrap();
    pool
}

async fn stable(pool: &PgPool, activity: bool, archived: bool) -> Uuid {
    let id = Uuid::new_v4();
    let query = if activity {
        "INSERT INTO activities SELECT (jsonb_populate_record(NULL::activities,to_jsonb(s)||jsonb_build_object('id',$1::uuid,'code',$1::text,'identity_key',$1::text,'archived_at',CASE WHEN $2 THEN now() END))).* FROM activities s ORDER BY id LIMIT 1"
    } else {
        "INSERT INTO subjects SELECT (jsonb_populate_record(NULL::subjects,to_jsonb(s)||jsonb_build_object('id',$1::uuid,'code',$1::text,'identity_key',$1::text,'archived_at',CASE WHEN $2 THEN now() END))).* FROM subjects s ORDER BY id LIMIT 1"
    };
    sqlx::query(query).bind(id).bind(archived).execute(pool).await.unwrap();
    id
}

async fn version(pool: &PgPool, activity: bool, stable: Uuid, revision: i32, date: &str, status: &str, grades: Vec<Uuid>) -> Uuid {
    let id = Uuid::new_v4();
    let query = if activity {
        "INSERT INTO activity_versions SELECT (jsonb_populate_record(NULL::activity_versions,to_jsonb(v)||jsonb_build_object('id',$1::uuid,'activity_id',$2::uuid,'version_no',$3::int,'effective_from',$4::text,'status','draft','published_at',NULL,'row_version',1,'grade_level_ids',$5::jsonb,'created_at','2001-01-01T00:00:00Z','updated_at','2001-01-01T00:00:00Z'))).* FROM activity_versions v ORDER BY id LIMIT 1"
    } else {
        "INSERT INTO subject_versions SELECT (jsonb_populate_record(NULL::subject_versions,to_jsonb(v)||jsonb_build_object('id',$1::uuid,'subject_id',$2::uuid,'version_no',$3::int,'effective_from',$4::text,'status','draft','published_at',NULL,'row_version',1,'created_at','2001-01-01T00:00:00Z','updated_at','2001-01-01T00:00:00Z'))).* FROM subject_versions v WHERE $5::jsonb IS NOT NULL ORDER BY id LIMIT 1"
    };
    sqlx::query(query).bind(id).bind(stable).bind(revision).bind(date).bind(json!(grades)).execute(pool).await.unwrap();
    if !activity {
        for grade in grades {
            sqlx::query("INSERT INTO subject_version_grade_levels(subject_id,grade_level_id) VALUES($1,$2)")
                .bind(id).bind(grade).execute(pool).await.unwrap();
        }
    }
    let query = if activity { "UPDATE activity_versions SET status=$2,published_at=CASE WHEN $2='published' THEN '2001-01-01'::timestamptz END WHERE id=$1" } else { "UPDATE subject_versions SET status=$2,published_at=CASE WHEN $2='published' THEN '2001-01-01'::timestamptz END WHERE id=$1" };
    sqlx::query(query).bind(id).bind(status).execute(pool).await.unwrap();
    id
}

async fn facts(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('subjects',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM subjects s),'activities',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM activities a),'versions',(SELECT jsonb_agg(facts ORDER BY id) FROM (SELECT id,to_jsonb(v) AS facts FROM subject_versions v UNION ALL SELECT id,to_jsonb(v) FROM activity_versions v) v),'links',(SELECT jsonb_agg(to_jsonb(g) ORDER BY subject_id,grade_level_id) FROM subject_version_grade_levels g),'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM academic_audit_events a),'references',jsonb_build_object('courses',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r),'activities',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_activity_requirements r),'publications',(SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM curriculum_publication_courses p),'activityPublications',(SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM curriculum_publication_activities p),'offerings',(SELECT jsonb_agg(to_jsonb(o) ORDER BY learning_offering_id) FROM course_offering_details o),'activityOfferings',(SELECT jsonb_agg(to_jsonb(o) ORDER BY learning_offering_id) FROM activity_offering_details o)))")
        .fetch_one(pool).await.unwrap()
}

async fn grade_ids(pool: &PgPool) -> Vec<Uuid> {
    sqlx::query_scalar("SELECT id FROM grade_levels WHERE is_active IS TRUE ORDER BY id").fetch_all(pool).await.unwrap()
}

#[tokio::test]
async fn catalog_grade_snapshot_migration_preserves_history_and_each_date_winner() {
    let pool = fixture("catalog_grade_snapshot_migration_preserve").await;
    let grades = grade_ids(&pool).await;
    let mut drafts = vec![];
    let mut repaired = vec![];
    let mut untouched = vec![];
    for activity in [false,true] {
        let catalog = stable(&pool, activity, false).await;
        untouched.push(version(&pool,activity,catalog,1,"2020-01-01","published",vec![]).await);
        untouched.push(version(&pool,activity,catalog,2,"2020-01-01","published",vec![grades[0]]).await);
        repaired.push(version(&pool,activity,catalog,3,"2026-01-01","published",vec![]).await);
        repaired.push(version(&pool,activity,catalog,4,"2040-01-01","published",vec![]).await);
        drafts.push(version(&pool,activity,catalog,5,"2030-01-01","draft",vec![]).await);
        untouched.push(version(&pool,activity,catalog,6,"2010-01-01","archived",vec![]).await);
        let archived = stable(&pool, activity, true).await;
        untouched.push(version(&pool,activity,archived,1,"2020-01-01","published",vec![]).await);
        untouched.push(version(&pool,activity,archived,2,"2020-01-01","draft",vec![]).await);
    }
    // Preserve an actual offering/curriculum pin even when its old scope was empty.
    let pinned: Uuid = sqlx::query_scalar("SELECT subject_version_id FROM course_offering_details ORDER BY learning_offering_id LIMIT 1").fetch_one(&pool).await.unwrap();
    sqlx::query("DELETE FROM subject_version_grade_levels WHERE subject_id=$1").bind(pinned).execute(&pool).await.unwrap();
    let before = facts(&pool).await;
    apply_migrations_through(&pool,141).await.unwrap();
    let after = facts(&pool).await;
    assert_eq!(before["subjects"],after["subjects"]);
    assert_eq!(before["activities"],after["activities"]);
    assert_eq!(before["references"],after["references"]);
    for old in before["versions"].as_array().unwrap() {
        let id: Uuid = serde_json::from_value(old["id"].clone()).unwrap();
        let actual = after["versions"].as_array().unwrap().iter().find(|v|v["id"]==old["id"]).unwrap();
        if drafts.contains(&id) {
            assert_eq!(actual["row_version"],json!(old["row_version"].as_i64().unwrap()+1));
            assert_eq!(actual["migration_provenance"]["catalogGradeSnapshot141"]["gradeLevelIds"],json!(grades));
        } else {
            assert_eq!(old,actual,"every preexisting non-repaired row is immutable");
        }
    }
    for source in repaired {
        let corrected = after["versions"].as_array().unwrap().iter().find(|v|v["migration_provenance"]["catalogGradeSnapshot141"]["sourceVersionId"]==json!(source)).unwrap();
        assert_eq!(corrected["status"],"published");
        assert_eq!(corrected["row_version"],2);
        assert_eq!(corrected["migration_provenance"]["catalogGradeSnapshot141"]["gradeLevelIds"],json!(grades));
        assert_ne!(corrected["published_at"],"2001-01-01T00:00:00+00:00");
        let source_row = before["versions"].as_array().unwrap().iter().find(|v|v["id"]==json!(source)).unwrap();
        assert_eq!(source_row["effective_from"],corrected["effective_from"]);
        assert!(corrected["version_no"].as_i64().unwrap()>6);
    }
    for source in untouched {
        assert!(!after["versions"].as_array().unwrap().iter().any(|v|v["migration_provenance"]["catalogGradeSnapshot141"]["action"]=="published_correction" && v["migration_provenance"]["catalogGradeSnapshot141"]["sourceVersionId"]==json!(source)));
    }
    for old in before["links"].as_array().unwrap() {
        assert!(after["links"].as_array().unwrap().contains(old));
    }
    let audit: Value = sqlx::query_scalar("SELECT payload FROM academic_audit_events WHERE event_code='catalog.grade_snapshots_reconciled'").fetch_one(&pool).await.unwrap();
    assert_eq!(audit["passed"],true);
    assert_eq!(audit["draftsRepaired"],2);
    assert!(audit["publishedCorrections"].as_i64().unwrap()>=4);
    school_migrations::run_tenant_migrations(&pool).await.unwrap();
    assert_eq!(after,facts(&pool).await,"canonical rerun is a no-op");
    assert!(sqlx::query("UPDATE subject_versions SET name_th='forbidden' WHERE id=$1").bind(pinned).execute(&pool).await.is_err());
}

#[tokio::test]
async fn catalog_grade_snapshot_migration_refuses_and_retries_without_partial_changes() {
    for (case,poison,repair,expected) in [
        ("grades","UPDATE grade_levels SET is_active=false","UPDATE grade_levels SET is_active=true","CATALOG_141_NO_ACTIVE_GRADES"),
        ("json","UPDATE activity_versions SET grade_level_ids='{}' WHERE status='draft'","UPDATE activity_versions SET grade_level_ids='[]' WHERE status='draft'","CATALOG_141_ACTIVITY_SCOPE_UNMAPPABLE"),
        ("provenance","UPDATE activity_versions SET migration_provenance=jsonb_build_object('catalogGradeSnapshot141',true) WHERE status='draft'","UPDATE activity_versions SET migration_provenance='{}' WHERE status='draft'","CATALOG_141_PROVENANCE_UNMAPPABLE"),
        ("guard","ALTER TABLE activity_versions DISABLE TRIGGER activity_versions_published_immutable","ALTER TABLE activity_versions ENABLE TRIGGER activity_versions_published_immutable","CATALOG_141_GUARD_STATE_INVALID"),
    ] {
        let pool = fixture(&format!("catalog_grade_snapshot_refuse_{case}")).await;
        let catalog = stable(&pool,true,false).await;
        version(&pool,true,catalog,1,"2020-01-01","published",vec![]).await;
        version(&pool,true,catalog,2,"2030-01-01","draft",vec![]).await;
        sqlx::raw_sql(poison).execute(&pool).await.unwrap();
        let before = facts(&pool).await;
        let error = apply_migrations_through(&pool,141).await.unwrap_err();
        assert!(error.to_string().contains(expected),"bounded refusal code: {expected}");
        assert_eq!(before,facts(&pool).await,"failure rolls back every catalog/audit change");
        sqlx::raw_sql(repair).execute(&pool).await.unwrap();
        apply_migrations_through(&pool,141).await.unwrap();
    }
}

#[tokio::test]
async fn catalog_grade_snapshot_migration_clean_empty_tenant_and_collision_overflow() {
    let pool = school_test_db::create_named_test_pool("catalog_grade_snapshot_clean").await;
    apply_migrations_through(&pool,140).await.unwrap();
    sqlx::query("UPDATE grade_levels SET is_active=false").execute(&pool).await.unwrap();
    apply_migrations_through(&pool,141).await.unwrap();
    apply_migrations_through(&pool,141).await.unwrap();
    for overflow in [false,true] {
        let pool = fixture(&format!("catalog_grade_snapshot_collision_{overflow}")).await;
        let catalog = stable(&pool,true,false).await;
        let source = version(&pool,true,catalog,if overflow {i32::MAX} else {1},"2020-01-01","published",vec![]).await;
        if !overflow {
            sqlx::query("INSERT INTO activity_versions SELECT (jsonb_populate_record(NULL::activity_versions,to_jsonb(v)||jsonb_build_object('id',uuid_generate_v5($1,'catalog-grade-snapshot-141'),'version_no',2,'status','archived','published_at',NULL))).* FROM activity_versions v WHERE id=$1")
                .bind(source).execute(&pool).await.unwrap();
        }
        let before = facts(&pool).await;
        let error = apply_migrations_through(&pool,141).await.unwrap_err();
        assert!(error.to_string().contains(if overflow {"CATALOG_141_REVISION_OVERFLOW"} else {"CATALOG_141_TARGET_ID_COLLISION"}));
        assert_eq!(before,facts(&pool).await);
    }
}
