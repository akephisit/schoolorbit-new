//! Sanitized 150 -> 151 rehearsals using the canonical tenant migration runner.
use school_academic_delivery::services::publication;
use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::collections::BTreeMap;
use uuid::Uuid;

const FACT_TABLES: &[&str] = &[
    "academic_delivery_versions",
    "academic_timetable_versions",
    "academic_term_change_sets",
    "academic_term_change_items",
    "learning_offerings",
    "course_offering_details",
    "activity_offering_details",
    "learning_offering_targets",
    "learning_groups",
    "learning_group_teachers",
    "learning_group_students",
    "learning_group_homerooms",
    "learning_group_preferred_rooms",
    "academic_timetable_blocks",
    "academic_timetable_block_groups",
    "academic_timetable_block_group_instructors",
    "academic_timetable_block_homerooms",
    "academic_timetable_block_teachers",
    "academic_timetable_block_group_sync",
    "academic_teacher_handoff_runs",
    "attendance_sessions",
    "attendance_records",
    "learning_group_score_items",
    "learning_group_student_scores",
    "learning_group_phase_confirmations",
    "academic_course_result_locks",
    "academic_course_results",
    "academic_activity_result_locks",
    "academic_activity_results",
    "academic_audit_events",
];

async fn predecessor(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 150).await.unwrap();
    pool
}

async fn canonical(pool: &PgPool) {
    school_migrations::run_tenant_migrations(pool)
        .await
        .expect("canonical tenant runner must accept the reconciled predecessor");
}

async fn facts(pool: &PgPool) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    for table in FACT_TABLES {
        let projection = if matches!(
            *table,
            "academic_delivery_versions" | "academic_timetable_versions"
        ) {
            "to_jsonb(row)-'publication_no'"
        } else {
            "to_jsonb(row)"
        };
        // Both identifiers and expressions come exclusively from the fixed fixture list.
        let query = format!(
            "SELECT COALESCE(jsonb_agg(fact ORDER BY fact::text),'[]'::jsonb) FROM \
             (SELECT {projection} AS fact FROM {table} row) preserved"
        );
        result.insert(
            (*table).into(),
            sqlx::query_scalar(sqlx::AssertSqlSafe(query))
                .fetch_one(pool)
                .await
                .unwrap(),
        );
    }
    result
}

async fn schema(pool: &PgPool) -> Value {
    sqlx::query_scalar(
        r#"SELECT jsonb_build_object(
            'relations',(SELECT jsonb_agg(jsonb_build_array(oid,relname,relkind,relowner,relrowsecurity) ORDER BY oid)
                FROM pg_class WHERE relnamespace=current_schema()::regnamespace),
            'columns',(SELECT jsonb_agg(to_jsonb(a) ORDER BY a.attrelid,a.attnum)
                FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid
                WHERE c.relnamespace=current_schema()::regnamespace AND a.attnum>0),
            'defaults',(SELECT jsonb_agg(to_jsonb(d) ORDER BY d.oid)
                FROM pg_attrdef d JOIN pg_class c ON c.oid=d.adrelid
                WHERE c.relnamespace=current_schema()::regnamespace),
            'constraints',(SELECT jsonb_agg(to_jsonb(c) ORDER BY c.oid)
                FROM pg_constraint c WHERE c.connamespace=current_schema()::regnamespace),
            'indexes',(SELECT jsonb_agg(to_jsonb(i) ORDER BY i.indexrelid)
                FROM pg_index i JOIN pg_class c ON c.oid=i.indrelid
                WHERE c.relnamespace=current_schema()::regnamespace),
            'triggers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY t.oid)
                FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid
                WHERE c.relnamespace=current_schema()::regnamespace),
            'functions',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.oid)
                FROM pg_proc p WHERE p.pronamespace=current_schema()::regnamespace))"#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn history(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(m) ORDER BY version) FROM _sqlx_migrations m")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audit(pool: &PgPool) -> Value {
    sqlx::query_scalar(
        "SELECT to_jsonb(a) FROM academic_publication_revisions_audit a WHERE migration_version=151",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn assert_reconciled(pool: &PgPool) {
    let receipt = audit(pool).await;
    assert_eq!(receipt["passed"], true);
    let checks = receipt["checks"].as_array().unwrap();
    let expected = [
        "delivery_publications_preserved",
        "downstream_sources_preserved",
        "offering_registry_preserved",
        "projection_integrity_reconciled",
        "publication_numbers_reconciled",
        "publication_receipts_preserved",
        "teacher_episodes_preserved",
        "timetable_publications_preserved",
    ];
    assert_eq!(checks.len(), expected.len());
    assert_eq!(
        checks
            .iter()
            .map(|check| check["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    for check in checks {
        assert_eq!(check["passed"], true);
        assert_eq!(check["count"], 0);
    }
    let decoded = publication::read_revisions_audit(pool).await.unwrap();
    assert!(decoded.completed);
    assert_eq!(decoded.checks.len(), 8);
    let ledgers: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM academic_delivery_teacher_cutoffs),\
         (SELECT count(*) FROM academic_delivery_offering_effectivity)",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        ledgers,
        (0, 0),
        "backfill must not invent effectivity evidence"
    );
}

async fn date_winners(pool: &PgPool, numbered: bool) -> Value {
    let ranking = if numbered {
        "n::bigint DESC"
    } else {
        "effective_from DESC,published_at DESC,id DESC"
    };
    let query = format!(
        "WITH publications AS (\
            SELECT 'delivery'::text domain,id,academic_term_id,effective_from,published_at,to_jsonb(v)->>'publication_no' AS n FROM academic_delivery_versions v WHERE status='published' \
            UNION ALL SELECT 'timetable',id,academic_term_id,effective_from,published_at,to_jsonb(v)->>'publication_no' FROM academic_timetable_versions v WHERE status='published'),\
         probes AS (SELECT DISTINCT academic_term_id,effective_from+offset_days AS day FROM publications CROSS JOIN generate_series(-1,1) offset_days),\
         candidates AS (SELECT domain,p.id,p.academic_term_id,probes.day,p.effective_from,p.published_at,n::bigint publication_no,\
             row_number() OVER(PARTITION BY domain,p.academic_term_id,probes.day ORDER BY {ranking}) AS rank \
             FROM publications p JOIN probes ON probes.academic_term_id=p.academic_term_id AND p.effective_from<=probes.day) \
         SELECT COALESCE(jsonb_agg(jsonb_build_array(domain,academic_term_id,day,id) ORDER BY domain,academic_term_id,day),'[]'::jsonb) FROM candidates WHERE rank=1"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(query))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn delivery(
    pool: &PgPool,
    term: Uuid,
    actor: Uuid,
    source: Option<Uuid>,
    date: &str,
    status: &str,
    published_at: &str,
    snapshot: Value,
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        r#"INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,source_version_id,
            effective_from,reference_date,status,snapshot,created_by,published_by,published_at,
            publication_idempotency_key,publication_request_hash,created_at,updated_at)
           SELECT $1,id,academic_year_id,$4,CASE WHEN $6='published' THEN $5::date END,$5::date,$6,$8,$3,
             CASE WHEN $6='published' THEN $3 END,CASE WHEN $6='published' THEN $7::timestamptz END,
             CASE WHEN $6='published' THEN $1 END,CASE WHEN $6='published' THEN repeat('a',64) END,
             '2000-02-03T04:05:06Z','2001-02-03T04:05:06Z'
           FROM academic_terms WHERE id=$2"#,
    )
    .bind(id).bind(term).bind(actor).bind(source).bind(date).bind(status)
    .bind(published_at).bind(snapshot).execute(pool).await.unwrap();
    id
}

async fn timetable(
    pool: &PgPool,
    term: Uuid,
    actor: Uuid,
    source: Option<Uuid>,
    delivery: Uuid,
    date: &str,
    status: &str,
    published_at: &str,
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        r#"INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,source_version_id,
            delivery_version_id,bell_schedule_id,effective_from,status,created_by,published_by,published_at,
            publication_idempotency_key,publication_request_hash,created_at,updated_at)
           SELECT $1,id,academic_year_id,$4,$5,bell_schedule_id,CASE WHEN $7='published' THEN $6::date END,$7,$3,
             CASE WHEN $7='published' THEN $3 END,CASE WHEN $7='published' THEN $8::timestamptz END,
             CASE WHEN $7='published' THEN $1 END,CASE WHEN $7='published' THEN repeat('b',64) END,
             '2000-03-04T05:06:07Z','2001-03-04T05:06:07Z'
           FROM academic_terms WHERE id=$2"#,
    )
    .bind(id).bind(term).bind(actor).bind(source).bind(delivery).bind(date).bind(status)
    .bind(published_at).execute(pool).await.unwrap();
    id
}

async fn revision(pool: &PgPool, target: Uuid, source: Uuid, actor: Uuid, status: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        r#"INSERT INTO academic_term_change_sets(id,academic_term_id,academic_year_id,
            base_delivery_version_id,target_delivery_version_id,effective_from,reference_date,
            reason,idempotency_key,creation_request_hash,created_by,status,published_by,published_at,cancelled_by,cancelled_at,
            created_at,updated_at)
           SELECT $1,academic_term_id,academic_year_id,$3,id,
             CASE WHEN $5='published' THEN effective_from END,reference_date,
             'sanitized publication migration fixture',$1::text,repeat('c',64),$4,$5,
             CASE WHEN $5='published' THEN $4 END,CASE WHEN $5='published' THEN '2002-01-01'::timestamptz END,
             CASE WHEN $5='cancelled' THEN $4 END,CASE WHEN $5='cancelled' THEN '2002-01-02'::timestamptz END,
             '2000-04-05T06:07:08Z','2001-04-05T06:07:08Z'
           FROM academic_delivery_versions WHERE id=$2"#,
    )
    .bind(id).bind(target).bind(source).bind(actor).bind(status)
    .execute(pool).await.unwrap();
    id
}

async fn seed_history(pool: &PgPool) {
    sqlx::raw_sql(
        r#"INSERT INTO learning_group_student_scores(learning_group_id,learning_offering_id,academic_term_id,
               academic_year_id,score_item_id,student_academic_year_id,score,updated_by,created_at,updated_at)
           SELECT item.learning_group_id,item.learning_offering_id,item.academic_term_id,item.academic_year_id,
               item.id,student.student_academic_year_id,0.10,'50000000-0000-0000-0000-000000000002',
               '2003-01-01','2003-01-02'
           FROM learning_group_score_items item JOIN learning_group_students student
             ON student.learning_group_id=item.learning_group_id ORDER BY item.id,student.id LIMIT 1;
           WITH accepted_lock AS (
             INSERT INTO academic_course_result_locks(subject_id,academic_term_id,academic_year_id,policy_version_id,
                 policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by,locked_at)
             SELECT detail.subject_id,detail.academic_term_id,detail.academic_year_id,policy.id,
                 '{"fixture":"accepted-policy"}',repeat('d',64),repeat('e',64),'{"fixture":"accepted-course"}',
                 '50000000-0000-0000-0000-000000000002','2003-02-01'
             FROM course_offering_details detail CROSS JOIN academic_grading_policy_versions policy
             WHERE EXISTS(SELECT 1 FROM learning_groups g JOIN learning_group_students s ON s.learning_group_id=g.id
                 WHERE g.learning_offering_id=detail.learning_offering_id)
             ORDER BY detail.learning_offering_id,policy.id LIMIT 1 RETURNING *)
           INSERT INTO academic_course_results(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,
               subject_id,course_result_lock_id,student_academic_year_id,calculated_score,calculated_grade,outcome,numeric_grade,selection_source)
           SELECT g.id,g.learning_offering_id,g.academic_term_id,g.academic_year_id,accepted_lock.subject_id,accepted_lock.id,
               student.student_academic_year_id,77.25,3.5,'numeric',3.5,'derived'
           FROM accepted_lock JOIN course_offering_details detail ON detail.subject_id=accepted_lock.subject_id AND detail.academic_term_id=accepted_lock.academic_term_id
           JOIN learning_groups g ON g.learning_offering_id=detail.learning_offering_id
           JOIN learning_group_students student ON student.learning_group_id=g.id ORDER BY g.id,student.id LIMIT 1;
           WITH accepted_lock AS (
             INSERT INTO academic_activity_result_locks(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,
                 roster_checksum,source_checksum,source_snapshot,locked_by,locked_at)
             SELECT g.id,g.learning_offering_id,g.academic_term_id,g.academic_year_id,
                 repeat('f',64),repeat('a',64),'{"fixture":"accepted-activity"}',
                 '50000000-0000-0000-0000-000000000002','2003-03-01'
             FROM learning_groups g JOIN activity_offering_details detail ON detail.learning_offering_id=g.learning_offering_id
             WHERE EXISTS(SELECT 1 FROM learning_group_students s WHERE s.learning_group_id=g.id)
             ORDER BY g.id LIMIT 1 RETURNING *)
           INSERT INTO academic_activity_results(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,
               activity_result_lock_id,student_academic_year_id,outcome)
           SELECT accepted_lock.learning_group_id,accepted_lock.learning_offering_id,accepted_lock.academic_term_id,accepted_lock.academic_year_id,
               accepted_lock.id,student.student_academic_year_id,'pass'
           FROM accepted_lock JOIN learning_group_students student ON student.learning_group_id=accepted_lock.learning_group_id
           ORDER BY student.id LIMIT 1;
           WITH session AS (
             INSERT INTO attendance_sessions(academic_term_id,date,kind,source_key,title,teacher_ids,
                 learning_group_id,offering_id,start_time,end_time,saved_at,saved_by,roster)
             SELECT g.academic_term_id,term.start_date+1,'lesson','publication151-fixture','Sanitized saved lesson',
                 ARRAY['50000000-0000-0000-0000-000000000002'::uuid],g.id,g.learning_offering_id,
                 '08:30','09:20','2003-04-01','50000000-0000-0000-0000-000000000002','[]'
             FROM learning_groups g JOIN academic_terms term ON term.id=g.academic_term_id
             WHERE EXISTS(SELECT 1 FROM learning_group_students s WHERE s.learning_group_id=g.id)
             ORDER BY g.id LIMIT 1 RETURNING *)
           INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin,observed_at,updated_by)
           SELECT session.id,year.student_id,student.student_academic_year_id,'late','teacher','2003-04-01',
               '50000000-0000-0000-0000-000000000002'
           FROM session JOIN learning_group_students student ON student.learning_group_id=session.learning_group_id
           JOIN student_academic_years year ON year.id=student.student_academic_year_id ORDER BY student.id LIMIT 1;"#,
    )
    .execute(pool).await.unwrap();
}

#[tokio::test]
async fn fresh_publication_migration_has_exact_evidence_and_is_replay_stable() {
    let pool = school_test_db::create_named_test_pool("publication151_fresh").await;
    canonical(&pool).await;
    assert_reconciled(&pool).await;
    let before = (facts(&pool).await, audit(&pool).await, history(&pool).await);
    canonical(&pool).await;
    assert_eq!(
        (facts(&pool).await, audit(&pool).await, history(&pool).await),
        before
    );
}

#[tokio::test]
async fn populated_publication_migration_preserves_exact_history_pins_and_date_winners() {
    let pool = predecessor("publication151_preservation").await;
    seed_history(&pool).await;
    let (term, actor): (Uuid, Uuid) = sqlx::query_as(
        "UPDATE academic_terms SET bell_schedule_id=(SELECT bell_schedule_id FROM academic_terms WHERE id='11000000-0000-0000-0000-000000000251') \
         WHERE id='11000000-0000-0000-0000-000000000252' RETURNING id,'50000000-0000-0000-0000-000000000002'::uuid",
    ).fetch_one(&pool).await.unwrap();
    let a = delivery(
        &pool,
        term,
        actor,
        None,
        "2025-11-05",
        "published",
        "2005-01-01",
        json!({"offerings":[]}),
    )
    .await;
    let ta = timetable(
        &pool,
        term,
        actor,
        None,
        a,
        "2025-11-05",
        "published",
        "2005-01-01",
    )
    .await;
    // Accepted source provenance can point from November 5 to November 1.
    let b = delivery(
        &pool,
        term,
        actor,
        Some(a),
        "2025-11-01",
        "published",
        "2005-01-01",
        json!({"offerings":[]}),
    )
    .await;
    let tb = timetable(
        &pool,
        term,
        actor,
        Some(ta),
        b,
        "2025-11-01",
        "published",
        "2005-01-01",
    )
    .await;
    let c = delivery(
        &pool,
        term,
        actor,
        Some(b),
        "2025-11-03",
        "published",
        "2004-01-01",
        json!({"offerings":[]}),
    )
    .await;
    let tc = timetable(
        &pool,
        term,
        actor,
        Some(tb),
        c,
        "2025-11-04",
        "published",
        "2004-01-01",
    )
    .await;
    let d = delivery(
        &pool,
        term,
        actor,
        Some(c),
        "2025-11-02",
        "published",
        "2006-01-01",
        json!({"offerings":[]}),
    )
    .await;
    for status in ["draft", "cancelled"] {
        let target = delivery(
            &pool,
            term,
            actor,
            Some(a),
            "2025-11-01",
            status,
            "2007-01-01",
            json!({"offerings":[]}),
        )
        .await;
        timetable(
            &pool,
            term,
            actor,
            Some(ta),
            a,
            "2025-11-01",
            status,
            "2007-01-01",
        )
        .await;
        revision(&pool, target, a, actor, status).await;
    }
    revision(&pool, a, b, actor, "published").await;
    let before = facts(&pool).await;
    for table in [
        "learning_group_teachers",
        "academic_timetable_blocks",
        "academic_timetable_block_groups",
        "academic_timetable_block_group_instructors",
        "attendance_sessions",
        "attendance_records",
        "learning_group_student_scores",
        "academic_course_results",
        "academic_activity_results",
    ] {
        assert!(
            !before[table].as_array().unwrap().is_empty(),
            "fixture must preserve actual {table} facts"
        );
    }
    let winners = date_winners(&pool, false).await;
    canonical(&pool).await;
    assert_eq!(
        facts(&pool).await,
        before,
        "only publication_no may be added to accepted rows"
    );
    assert_eq!(date_winners(&pool, true).await, winners);
    assert_reconciled(&pool).await;
    let actual: Vec<(Uuid,Option<i64>)> = sqlx::query_as(
        "SELECT id,publication_no FROM academic_delivery_versions WHERE academic_term_id=$1 ORDER BY publication_no NULLS LAST,id",
    ).bind(term).fetch_all(&pool).await.unwrap();
    assert_eq!(
        actual
            .iter()
            .filter(|(_, n)| n.is_some())
            .copied()
            .collect::<Vec<_>>(),
        vec![(b, Some(1)), (d, Some(2)), (c, Some(3)), (a, Some(4))]
    );
    let actual_tables: Vec<(Uuid,Option<i64>)> = sqlx::query_as(
        "SELECT id,publication_no FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY publication_no",
    ).bind(term).fetch_all(&pool).await.unwrap();
    assert_eq!(
        actual_tables,
        vec![(tb, Some(1)), (tc, Some(2)), (ta, Some(3))]
    );
    let numbering_errors: i64=sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT status,publication_no,row_number() OVER(PARTITION BY academic_term_id,status ORDER BY effective_from,id) expected \
         FROM academic_delivery_versions UNION ALL SELECT status,publication_no,row_number() OVER(PARTITION BY academic_term_id,status ORDER BY effective_from,id) \
         FROM academic_timetable_versions) v WHERE (status='published' AND publication_no IS DISTINCT FROM expected) OR (status<>'published' AND publication_no IS NOT NULL)",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(numbering_errors, 0);
    let receipt = audit(&pool).await;
    canonical(&pool).await;
    assert_eq!(facts(&pool).await, before);
    assert_eq!(audit(&pool).await, receipt);
}

#[derive(Clone, Copy, Debug)]
enum Malformed {
    MissingAssignment,
    UnknownAssignment,
    WrongTeacher,
    WrongRole,
    WrongGroup,
    WrongContext,
}

#[tokio::test]
async fn malformed_published_assignments_refuse_atomically_and_corrected_evidence_retries() {
    for malformed in [
        Malformed::MissingAssignment,
        Malformed::UnknownAssignment,
        Malformed::WrongTeacher,
        Malformed::WrongRole,
        Malformed::WrongGroup,
        Malformed::WrongContext,
    ] {
        let pool = predecessor(&format!("publication151_{malformed:?}")).await;
        let (source,term,actor,valid):(Uuid,Uuid,Uuid,Value)=sqlx::query_as(
            "SELECT id,academic_term_id,published_by,snapshot FROM academic_delivery_versions v WHERE status='published' \
             AND EXISTS(SELECT 1 FROM jsonb_array_elements(snapshot->'offerings') o,jsonb_array_elements(o->'groups') g,jsonb_array_elements(g->'teachers') t) ORDER BY id LIMIT 1",
        ).fetch_one(&pool).await.unwrap();
        let mut bad = valid.clone();
        let (oi, gi) = bad["offerings"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .find_map(|(oi, o)| {
                o["groups"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .find(|(_, g)| !g["teachers"].as_array().unwrap().is_empty())
                    .map(|(gi, _)| (oi, gi))
            })
            .unwrap();
        match malformed {
            Malformed::MissingAssignment => {
                bad["offerings"][oi]["groups"][gi]["teachers"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("assignmentId");
            }
            Malformed::UnknownAssignment => {
                bad["offerings"][oi]["groups"][gi]["teachers"][0]["assignmentId"] =
                    json!(Uuid::now_v7())
            }
            Malformed::WrongTeacher => {
                bad["offerings"][oi]["groups"][gi]["teachers"][0]["teacherId"] =
                    json!(Uuid::now_v7())
            }
            Malformed::WrongRole => {
                bad["offerings"][oi]["groups"][gi]["teachers"][0]["role"] =
                    json!("unmapped-fixture-role")
            }
            Malformed::WrongGroup => {
                bad["offerings"][oi]["groups"][gi]["id"] = json!(Uuid::now_v7())
            }
            Malformed::WrongContext => {
                let foreign:Value=sqlx::query_scalar("SELECT snapshot FROM academic_delivery_versions WHERE status='published' AND academic_term_id<>$1 AND EXISTS(SELECT 1 FROM jsonb_array_elements(snapshot->'offerings') o,jsonb_array_elements(o->'groups') g,jsonb_array_elements(g->'teachers') t) ORDER BY id LIMIT 1")
                    .bind(term).fetch_one(&pool).await.unwrap();
                bad = foreign;
            }
        }
        let date:String=sqlx::query_scalar("SELECT (max(effective_from)+7)::text FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published'")
            .bind(term).fetch_one(&pool).await.unwrap();
        let bad_id = delivery(
            &pool,
            term,
            actor,
            Some(source),
            &date,
            "published",
            "2008-01-01",
            bad,
        )
        .await;
        let before = facts(&pool).await;
        let original_schema = schema(&pool).await;
        let original_history = history(&pool).await;
        let error = school_migrations::run_tenant_migrations_detailed(&pool)
            .await
            .unwrap_err();
        assert_eq!(error.diagnostic.version, Some(151), "{malformed:?}");
        assert!(
            format!("{error:?}").contains("PUBLICATION_151_ASSIGNMENT_CONTEXT_INVALID"),
            "{malformed:?}: {error:?}"
        );
        assert_eq!(
            facts(&pool).await,
            before,
            "{malformed:?} must preserve all committed facts"
        );
        assert_eq!(
            schema(&pool).await,
            original_schema,
            "{malformed:?} must roll back columns, tables, indexes, functions and guards"
        );
        assert_eq!(history(&pool).await, original_history);
        let max_version: i64 =
            sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE success")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(max_version, 150);
        let immutable =
            sqlx::query("UPDATE academic_delivery_versions SET snapshot=snapshot WHERE id=$1")
                .bind(bad_id)
                .execute(&pool)
                .await
                .unwrap_err();
        assert!(immutable
            .to_string()
            .contains("ACADEMIC_DELIVERY_VERSION_IMMUTABLE"));
        // The fixture owner repairs only its deliberately malformed evidence. Keep the
        // original guard enabled at both transaction boundaries and retry the real runner.
        let mut correction = pool.begin().await.unwrap();
        sqlx::query("ALTER TABLE academic_delivery_versions DISABLE TRIGGER academic_delivery_versions_immutable")
            .execute(&mut *correction).await.unwrap();
        sqlx::query("UPDATE academic_delivery_versions SET snapshot=$2 WHERE id=$1")
            .bind(bad_id)
            .bind(valid)
            .execute(&mut *correction)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE academic_delivery_versions ENABLE TRIGGER academic_delivery_versions_immutable")
            .execute(&mut *correction).await.unwrap();
        correction.commit().await.unwrap();
        assert_eq!(schema(&pool).await, original_schema);
        let repaired = facts(&pool).await;
        canonical(&pool).await;
        assert_eq!(facts(&pool).await, repaired);
        assert_reconciled(&pool).await;
    }
}

#[tokio::test]
async fn actual_runtime_role_reads_audit_but_cannot_write_or_commit_unpublished_effectivity() {
    let admin = school_test_db::create_named_test_pool("publication151_runtime_admin").await;
    let identity = Uuid::now_v7().simple().to_string();
    let database = format!("publication151_{identity}");
    let role = format!("publication151_runtime_{identity}");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE \"{database}\""
    )))
    .execute(&admin)
    .await
    .unwrap();
    let options = admin.connect_options().as_ref().clone().database(&database);
    let owner = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    seed_release_two_predecessor(&owner).await.unwrap();
    apply_migrations_through(&owner, 150).await.unwrap();
    let (source,term,actor):(Uuid,Uuid,Uuid)=sqlx::query_as(
        "SELECT id,academic_term_id,published_by FROM academic_delivery_versions v WHERE status='published' AND academic_term_id='11000000-0000-0000-0000-000000000251' AND EXISTS(SELECT 1 FROM learning_group_teachers t WHERE t.academic_term_id=v.academic_term_id) ORDER BY id LIMIT 1",
    ).fetch_one(&owner).await.unwrap();
    let date: String = sqlx::query_scalar(
        "SELECT min(starts_on)::text FROM learning_group_teachers WHERE academic_term_id=$1",
    )
    .bind(term)
    .fetch_one(&owner)
    .await
    .unwrap();
    let draft = delivery(
        &owner,
        term,
        actor,
        Some(source),
        &date,
        "draft",
        "2009-01-01",
        json!({"offerings":[]}),
    )
    .await;
    let change = revision(&owner, draft, source, actor, "draft").await;
    canonical(&owner).await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        r#"CREATE ROLE "{role}" NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;
           REVOKE ALL ON DATABASE "{database}" FROM PUBLIC;
           GRANT CONNECT ON DATABASE "{database}" TO "{role}";
           REVOKE CREATE ON SCHEMA public FROM PUBLIC;
           GRANT USAGE ON SCHEMA public TO "{role}";
           GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA public TO "{role}";
           REVOKE INSERT,UPDATE,DELETE ON _sqlx_migrations FROM "{role}";
           GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO "{role}";"#,
    )))
    .execute(&owner)
    .await
    .unwrap();
    let runtime_role = role.clone();
    let runtime = PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| {
            let role = runtime_role.clone();
            Box::pin(async move {
                sqlx::query(sqlx::AssertSqlSafe(format!("SET ROLE \"{role}\"")))
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .unwrap();
    let (actual,is_owner,superuser,temporary,create_schema):(String,bool,bool,bool,bool)=sqlx::query_as(
        "SELECT current_user::text,(SELECT relowner=current_user::regrole FROM pg_class WHERE oid='academic_publication_revisions_audit'::regclass),\
         (SELECT rolsuper FROM pg_roles WHERE rolname=current_user),has_database_privilege(current_user,current_database(),'TEMP'),has_schema_privilege(current_user,'public','CREATE')",
    ).fetch_one(&runtime).await.unwrap();
    assert_eq!(actual, role);
    assert!(!is_owner && !superuser && !temporary && !create_schema);
    let receipt = audit(&runtime).await;
    assert_eq!(receipt, audit(&owner).await);
    assert!(
        publication::read_revisions_audit(&runtime)
            .await
            .unwrap()
            .completed
    );
    for statement in [
        "INSERT INTO academic_publication_revisions_audit(migration_version,passed,checks) VALUES(151,true,'[]')",
        "UPDATE academic_publication_revisions_audit SET passed=false WHERE migration_version=151",
        "DELETE FROM academic_publication_revisions_audit WHERE migration_version=151",
    ] {
        let denied=sqlx::query(statement).execute(&runtime).await.unwrap_err();
        assert_eq!(denied.as_database_error().unwrap().code().as_deref(),Some("23514"));
        assert!(denied.to_string().contains("PUBLICATION_151_AUDIT_IMMUTABLE"));
        assert_eq!(audit(&owner).await,receipt);
    }
    let before = facts(&owner).await;
    let mut transaction = runtime.begin().await.unwrap();
    let effects: Value = sqlx::query_scalar("SELECT academic_delivery_accept_effectivity($1,$2)")
        .bind(change)
        .bind(actor)
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
    assert!(
        !effects["cutoffAssignmentIds"]
            .as_array()
            .unwrap()
            .is_empty(),
        "exercise real teacher cutoff evidence"
    );
    let offerings: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_delivery_offering_effectivity WHERE change_set_id=$1",
    )
    .bind(change)
    .fetch_one(&mut *transaction)
    .await
    .unwrap();
    assert!(offerings > 0, "exercise real offering effectivity evidence");
    let denied = transaction.commit().await.unwrap_err();
    assert!(denied
        .to_string()
        .contains("ACADEMIC_DELIVERY_EFFECTIVITY_NOT_PUBLISHED"));
    assert_eq!(facts(&owner).await, before);
    assert_reconciled(&owner).await;
    runtime.close().await;
    owner.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE \"{database}\"")))
        .execute(&admin)
        .await
        .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP ROLE \"{role}\"")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
