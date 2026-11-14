//! Sanitized 155 -> current cutover and scoped Results deletion rehearsals.
use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

#[path = "current_academic_operations/source_fences.rs"]
mod source_fences;

#[path = "current_academic_operations/group_cutover.rs"]
mod group_cutover;

#[path = "current_academic_operations/inactive_placements.rs"]
mod inactive_placements;

async fn predecessor(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 155).await.unwrap();
    pool
}

async fn canonical(pool: &PgPool) {
    if let Err(error) = school_migrations::run_tenant_migrations_detailed(pool).await {
        let mut tx = pool.begin().await.unwrap();
        let native = sqlx::raw_sql(include_str!(
            "../../../migrations/157_current_academic_operations.sql"
        ))
        .execute(&mut *tx)
        .await;
        tx.rollback().await.unwrap();
        panic!(
            "canonical current migration: {error:?}; rollback-only native diagnostic: {native:?}"
        );
    }
}

async fn current_facts(pool: &PgPool) -> Value {
    sqlx::query_scalar(
        r#"SELECT jsonb_build_object(
        'offerings',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_offerings r),
        'groups',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_groups r),
        'teachers',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_group_teachers r),
        'timetables',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_timetables r),
        'blocks',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_timetable_blocks r),
        'memberships',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_group_students r))"#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn populated_cutover_preserves_current_ids_facts_and_retries_without_history() {
    let pool = predecessor("current_ops_preservation").await;
    let chosen: Vec<Uuid> = sqlx::query_scalar("SELECT academic_timetable_applicable_publication(id,(now() AT TIME ZONE 'Asia/Bangkok')::date) FROM academic_terms WHERE academic_timetable_applicable_publication(id,(now() AT TIME ZONE 'Asia/Bangkok')::date) IS NOT NULL ORDER BY id")
        .fetch_all(&pool).await.unwrap();
    let supervision: Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r)-'updated_at'-'timetable_block_group_id'-'learning_group_id' ORDER BY id) FROM supervision_observations r")
        .fetch_one(&pool).await.unwrap();
    canonical(&pool).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_timetables WHERE id=ANY($1)")
            .bind(&chosen)
            .fetch_one(&pool)
            .await
            .unwrap(),
        chosen.len() as i64
    );

    assert_eq!(sqlx::query_scalar::<_, Value>("SELECT jsonb_agg(to_jsonb(r)-'updated_at'-'timetable_block_group_id'-'learning_group_id' ORDER BY id) FROM supervision_observations r")
        .fetch_one(&pool).await.unwrap(), supervision);
    let legacy: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_class WHERE relnamespace=current_schema()::regnamespace AND relname=ANY(ARRAY['academic_delivery_versions','academic_timetable_versions','academic_term_change_sets','academic_term_change_items','academic_teacher_handoff_runs','academic_delivery_revision_audiences','academic_delivery_revision_audience_rooms','academic_delivery_offering_effectivity','academic_delivery_teacher_cutoffs'])")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(legacy, 0);
    let before = current_facts(&pool).await;
    canonical(&pool).await;
    assert_eq!(before, current_facts(&pool).await);
    let checks: Value = sqlx::query_scalar("SELECT checks FROM academic_current_operations_migration_audit WHERE migration_version=157 AND passed")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(checks.as_array().unwrap().len(), 8);
    assert!(checks
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["passed"] == true && c["mismatchCount"] == 0));
}

#[tokio::test]
async fn future_publication_never_replaces_the_applicable_current_graph() {
    let pool = predecessor("current_ops_future").await;
    let (version, offering, periods): (Uuid, Uuid, i32) = sqlx::query_as("SELECT v.id,(o->>'id')::uuid,(o->>'weeklyPeriodTarget')::int FROM academic_delivery_versions v CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') o JOIN academic_terms t ON t.id=v.academic_term_id WHERE t.status='active' LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    for (number, days, increment) in [(1_i64, 0_i32, 1_i32), (2, 0, 2), (3, 30, 7)] {
        sqlx::query("INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,source_version_id,effective_from,status,snapshot,row_version,created_by,published_by,published_at,publication_idempotency_key,publication_request_hash,publication_no) SELECT $2,academic_term_id,academic_year_id,id,(now() AT TIME ZONE 'Asia/Bangkok')::date+$4,'published',jsonb_set(snapshot,'{offerings,0,weeklyPeriodTarget}',to_jsonb($3::int)),1,created_by,published_by,clock_timestamp(),$2,repeat('a',64),publication_no+$5 FROM academic_delivery_versions WHERE id=$1")
            .bind(version).bind(Uuid::new_v4()).bind(periods+increment).bind(days).bind(number).execute(&pool).await.unwrap();
    }
    canonical(&pool).await;
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT weekly_period_target FROM learning_offerings WHERE id=$1"
        )
        .bind(offering)
        .fetch_one(&pool)
        .await
        .unwrap(),
        periods + 2
    );
}

#[tokio::test]
async fn conflicting_preparation_refuses_atomically_and_corrected_source_can_retry() {
    let pool = predecessor("current_ops_ambiguous").await;
    let term = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id,school_days) SELECT $1,academic_year_id,'Current preparation',99,'CURRENT-PREP','regular',false,false,'planning',bell_schedule_id,school_days FROM academic_terms WHERE status='active'")
        .bind(term).execute(&pool).await.unwrap();
    let first = Uuid::new_v4();
    for id in [first, Uuid::new_v4()] {
        sqlx::query("INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,status,snapshot,reference_date) SELECT $1,id,academic_year_id,'draft','{\"offerings\":[]}',(now() AT TIME ZONE 'Asia/Bangkok')::date FROM academic_terms WHERE id=$2")
            .bind(id).bind(term).execute(&pool).await.unwrap();
    }
    let error = school_migrations::run_tenant_migrations_detailed(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "ACADEMIC_CURRENT_DELIVERY_SOURCE_AMBIGUOUS"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        156
    );
    assert!(sqlx::query_scalar::<_, bool>("SELECT to_regclass('academic_delivery_versions') IS NOT NULL AND to_regclass('academic_timetables') IS NULL")
        .fetch_one(&pool).await.unwrap());
    sqlx::query("DELETE FROM academic_delivery_versions WHERE academic_term_id=$1 AND id<>$2")
        .bind(term)
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();
    canonical(&pool).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_delivery_state WHERE academic_term_id=$1"
        )
        .bind(term)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn mismatched_current_timetable_refuses_without_dropping_placements() {
    let pool = predecessor("current_ops_mismatch").await;
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_timetable_blocks")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("ALTER TABLE academic_delivery_versions DISABLE TRIGGER academic_delivery_versions_immutable; UPDATE academic_delivery_versions SET snapshot=jsonb_set(snapshot,'{offerings,0,groups}','[]') WHERE academic_term_id=(SELECT id FROM academic_terms WHERE status='active'); ALTER TABLE academic_delivery_versions ENABLE TRIGGER academic_delivery_versions_immutable;")
        .execute(&pool).await.unwrap();
    let error = school_migrations::run_tenant_migrations_detailed(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "ACADEMIC_CURRENT_TIMETABLE_DELIVERY_MISMATCH"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_timetable_blocks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn malformed_retained_aggregate_items_refuse_before_filtering() {
    let pool = predecessor("current_ops_malformed").await;
    apply_migrations_through(&pool, 156).await.unwrap();
    let offering = Uuid::new_v4();
    let subject = Uuid::new_v4();
    let good = json!({"subjectId":subject,"learningOfferingId":offering,"resultId":Uuid::new_v4(),"effectiveVersion":1,"credits":"1.00","outcome":"numeric","numericGrade":"4.00"});
    for bad in [
        json!({"credits":"1.00"}),
        json!({"learningOfferingId":null,"subjectId":Uuid::new_v4(),"resultId":null,"effectiveVersion":null,"credits":"1.00","outcome":null,"numericGrade":null}),
    ] {
        let snapshot = json!({"policy":{"passingGrade":"1.00","minimumLearnerLevel":1,"allowReviewedHolds":true},"results":{"courses":[good,bad],"activities":[]},"learnerEvaluations":{"policyVersionId":"06000000-0000-0000-0000-000000000002","domains":[]}});
        let error = sqlx::query_scalar::<_, Value>(
            "SELECT academic_result_without_offering($1,$2,'{}','{}','{}',true)",
        )
        .bind(snapshot)
        .bind(offering)
        .fetch_one(&pool)
        .await
        .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().message(),
            "ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE"
        );
    }
    let snapshot = json!({"policy":{"passingGrade":"1.00","minimumLearnerLevel":1,"allowReviewedHolds":true},"results":{"courses":[],"activities":[{"learningGroupId":Uuid::new_v4(),"learningOfferingId":offering,"resultId":Uuid::new_v4(),"effectiveVersion":null,"outcome":"pass"}]},"learnerEvaluations":{"policyVersionId":"06000000-0000-0000-0000-000000000002","domains":[]}});
    let error = sqlx::query_scalar::<_, Value>(
        "SELECT academic_result_without_offering($1,$2,'{}','{}','{}',true)",
    )
    .bind(snapshot)
    .bind(offering)
    .fetch_one(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().message(),
        "ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE"
    );
}

#[tokio::test]
async fn scoped_removal_recalculates_mixed_aggregates_and_requires_review_for_new_holds() {
    let pool = predecessor("current_ops_result_holds").await;
    canonical(&pool).await;
    let (offering, group, term, year, student, subject): (Uuid, Uuid, Uuid, Uuid, Uuid, Uuid) = sqlx::query_as("SELECT o.id,g.id,o.academic_term_id,o.academic_year_id,m.student_academic_year_id,d.subject_id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id JOIN learning_group_students m ON m.learning_group_id=g.id JOIN course_offering_details d ON d.learning_offering_id=o.id JOIN academic_terms t ON t.id=o.academic_term_id WHERE t.status='active' LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let actor: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type='staff' AND status='active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let second = Uuid::new_v4();
    let second_group = Uuid::new_v4();
    let mut setup = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,to_jsonb(o)||jsonb_build_object('id',$2::uuid,'code_snapshot','CURRENT-SCI','source_requirement_kind',NULL,'source_requirement_id',NULL))).* FROM learning_offerings o WHERE id=$1")
        .bind(offering).bind(second).execute(&mut *setup).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details SELECT (jsonb_populate_record(NULL::course_offering_details,to_jsonb(d)||jsonb_build_object('learning_offering_id',$2::uuid,'subject_version_id',v.id,'subject_id',v.subject_id,'curriculum_course_requirement_id',NULL))).* FROM course_offering_details d CROSS JOIN subject_versions v JOIN subjects s ON s.id=v.subject_id WHERE d.learning_offering_id=$1 AND s.code='SCI-CORE' ORDER BY v.version_no LIMIT 1")
        .bind(offering).bind(second).execute(&mut *setup).await.unwrap();
    sqlx::query("INSERT INTO learning_groups SELECT (jsonb_populate_record(NULL::learning_groups,to_jsonb(g)||jsonb_build_object('id',$2::uuid,'learning_offering_id',$3::uuid))).* FROM learning_groups g WHERE id=$1")
        .bind(group).bind(second_group).bind(second).execute(&mut *setup).await.unwrap();
    setup.commit().await.unwrap();
    let second_subject: Uuid = sqlx::query_scalar(
        "SELECT subject_id FROM course_offering_details WHERE learning_offering_id=$1",
    )
    .bind(second)
    .fetch_one(&pool)
    .await
    .unwrap();
    let first_result = Uuid::new_v4();
    let second_result = Uuid::new_v4();
    for (off, grp, sub, result, grade) in [
        (offering, group, subject, first_result, 4),
        (second, second_group, second_subject, second_result, 2),
    ] {
        let lock = Uuid::new_v4();
        sqlx::query("INSERT INTO academic_course_result_locks(id,subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES($1,$2,$3,$4,'06000000-0000-0000-0000-000000000001','{}',repeat('0',64),repeat('0',64),$5,$6)")
            .bind(lock).bind(sub).bind(term).bind(year).bind(json!({"groups":[{"learningGroupId":grp,"learningOfferingId":off}]})).bind(actor).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_course_results(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,course_result_lock_id,student_academic_year_id,calculated_score,calculated_grade,outcome,numeric_grade,selection_source) VALUES($1,$2,$3,$4,$5,$6,$7,$8,80,$9,'numeric',$9,'derived')")
            .bind(result).bind(grp).bind(off).bind(term).bind(year).bind(sub).bind(lock).bind(student).bind(grade).execute(&pool).await.unwrap();
    }
    let policy = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_aggregate_policy_versions(id,name,passing_grade,minimum_learner_level,allow_reviewed_holds,approved_by) VALUES($1,'Reviewed current fixture',1,1,true,$2)")
        .bind(policy).bind(actor).execute(&pool).await.unwrap();
    let domains: Vec<Value> = ["desirable_characteristic","reading_thinking_writing"].into_iter().map(|domain| json!({
        "domain":domain,"average":{"numerator":"3","denominator":"2","decimal":"1.5"},"qualityLevel":2,"complete":true,"missingSubjects":[],"catalogCriteria":[],
        "subjects":[{"subjectId":subject,"average":{"numerator":"3","denominator":"1","decimal":"3"},"criteria":[{"id":Uuid::new_v4(),"qualityLevel":3,"schoolCriterionId":null}]},
            {"subjectId":second_subject,"average":{"numerator":"0","denominator":"1","decimal":"0"},"criteria":[{"id":Uuid::new_v4(),"qualityLevel":0,"schoolCriterionId":null}]}]
    })).collect();
    let snapshot = json!({"policy":{"id":policy,"name":"Reviewed current fixture","passingGrade":"1.00","minimumLearnerLevel":1,"allowReviewedHolds":true,"approvedBy":actor,"approvedAt":"2025-01-01T00:00:00Z"},
        "results":{"academicYearId":year,"academicTermId":term,"studentAcademicYearId":student,"passingGrade":"1.00",
            "courses":[{"subjectId":subject,"learningOfferingId":offering,"resultId":first_result,"effectiveVersion":1,"credits":"1.00","outcome":"numeric","numericGrade":"4.00"},
                {"subjectId":second_subject,"learningOfferingId":second,"resultId":second_result,"effectiveVersion":1,"credits":"1.00","outcome":"numeric","numericGrade":"2.00"}],
            "totals":{},"activities":[],"activityTotals":{},"sourceChecksum":"0".repeat(64)},
        "learnerEvaluations":{"studentAcademicYearId":student,"policyVersionId":"06000000-0000-0000-0000-000000000002","domains":domains},
        "blockers":[],"holdFindings":[],"canLock":true,"sourceChecksum":"0".repeat(64)});
    let revision = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_term_aggregate_revisions(id,student_academic_year_id,academic_year_id,academic_term_id,revision,policy_id,source_checksum,request_id,request_checksum,snapshot,official_gpa,locked_by) VALUES($1,$2,$3,$4,1,$5,repeat('0',64),$6,repeat('0',64),$7,3,$8)")
        .bind(revision).bind(student).bind(year).bind(term).bind(policy).bind(Uuid::new_v4()).bind(&snapshot).bind(actor).execute(&pool).await.unwrap();
    let annual = Uuid::new_v4();
    let annual_snapshot = json!({"academicYearId":year,"studentAcademicYearId":student,
        "terms":[{"academicTermId":term,"termName":"Current fixture","sequence":1,"isCurrent":true,
            "revision":{"id":revision,"revision":1,"snapshot":snapshot,"officialGpa":"3.00","holdReason":null,
                "lockedBy":actor,"lockedAt":"2025-01-01T00:00:00Z","isCurrent":true}}],
        "totals":{},"canLock":true,"needsHold":false,"sourceChecksum":"0".repeat(64)});
    sqlx::query("INSERT INTO academic_annual_result_revisions(id,student_academic_year_id,academic_year_id,revision,source_checksum,request_id,request_checksum,snapshot,official_gpa,locked_by) VALUES($1,$2,$3,1,repeat('0',64),$4,repeat('0',64),$5,3,$6)")
        .bind(annual).bind(student).bind(year).bind(Uuid::new_v4()).bind(annual_snapshot).bind(actor).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_annual_result_term_sources(annual_revision_id,academic_year_id,academic_term_id,student_academic_year_id,term_aggregate_revision_id) VALUES($1,$2,$3,$4,$5)")
        .bind(annual).bind(year).bind(term).bind(student).bind(revision).execute(&pool).await.unwrap();
    let before_other: Value =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM academic_course_results r WHERE id=$1")
            .bind(second_result)
            .fetch_one(&pool)
            .await
            .unwrap();
    let impact: Value = sqlx::query_scalar(
        "SELECT academic_current_results_deletion_impact($1,ARRAY[$2]::uuid[],true)",
    )
    .bind(offering)
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(impact["officialResults"], 1);
    assert_eq!(impact["termAggregates"], 1);
    assert_eq!(impact["annualAggregates"], 1);
    sqlx::query("SELECT academic_remove_offering_results($1)")
        .bind(offering)
        .execute(&pool)
        .await
        .unwrap();
    let (saved, gpa): (Value, Option<String>) = sqlx::query_as(
        "SELECT snapshot,official_gpa::text FROM academic_term_aggregate_revisions WHERE id=$1",
    )
    .bind(revision)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        gpa.is_none(),
        "new hold findings require review despite canLock=true"
    );
    assert_eq!(saved["holdFindings"], json!(["failed_learner_evaluations"]));
    assert_eq!(saved["results"]["totals"]["provisionalGpa"], "2.00");
    assert_eq!(saved["results"]["courses"].as_array().unwrap().len(), 1);
    assert!(!saved.to_string().contains(&offering.to_string()));
    let (annual_saved, annual_gpa): (Value, Option<String>) = sqlx::query_as(
        "SELECT snapshot,official_gpa::text FROM academic_annual_result_revisions WHERE id=$1",
    )
    .bind(annual)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(annual_gpa.is_none());
    assert_eq!(annual_saved["totals"]["provisionalGpa"], "2.00");
    assert_eq!(annual_saved["terms"][0]["revision"]["snapshot"], saved);
    assert!(!annual_saved.to_string().contains(&offering.to_string()));
    assert_eq!(
        before_other,
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(r) FROM academic_course_results r WHERE id=$1"
        )
        .bind(second_result)
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_result_removal_scope")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let forbidden = sqlx::query("DELETE FROM academic_course_results WHERE id=$1")
        .bind(second_result)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        forbidden.as_database_error().unwrap().message(),
        "ACADEMIC_OFFICIAL_RESULT_IMMUTABLE"
    );
}

#[tokio::test]
async fn definer_scope_cannot_be_forged_or_shadowed_even_with_temporary_table_permission() {
    let pool = predecessor("current_ops_definer_scope").await;
    canonical(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let role = format!("current_scope_{}", Uuid::new_v4().simple());
    assert!(schema
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_'));
    let setup = format!("CREATE ROLE {role}; GRANT USAGE ON SCHEMA {schema} TO {role}; GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA {schema} TO {role}; REVOKE ALL ON {schema}.academic_result_removal_scope FROM {role};");
    sqlx::raw_sql(sqlx::AssertSqlSafe(setup))
        .execute(&mut *tx)
        .await
        .unwrap();
    let (year,term,student,actor):(Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT t.academic_year_id,t.id,s.id,(SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1) FROM academic_terms t JOIN student_academic_years s ON s.academic_year_id=t.academic_year_id WHERE t.status='active' LIMIT 1")
        .fetch_one(&mut *tx).await.unwrap();
    let policy = Uuid::new_v4();
    let revision = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_aggregate_policy_versions(id,name,passing_grade,minimum_learner_level,allow_reviewed_holds,approved_by) VALUES($1,'Shadow fixture',1,1,true,$2)")
        .bind(policy).bind(actor).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_term_aggregate_revisions(id,student_academic_year_id,academic_year_id,academic_term_id,revision,policy_id,source_checksum,request_id,request_checksum,snapshot,official_gpa,locked_by) VALUES($1,$2,$3,$4,1,$5,repeat('0',64),$6,repeat('0',64),'{}',1,$7)")
        .bind(revision).bind(student).bind(year).bind(term).bind(policy).bind(Uuid::new_v4()).bind(actor).execute(&mut *tx).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("SET LOCAL ROLE {role}; CREATE TEMP TABLE academic_result_removal_scope(transaction_id xid8,backend_pid integer,relation_name text,operation text,before_hash text,after_hash text); CREATE TEMP TABLE learning_offerings(id uuid,academic_year_id uuid,academic_term_id uuid);")))
        .execute(&mut *tx).await.unwrap();
    let has_temp: bool =
        sqlx::query_scalar("SELECT has_database_privilege(current_user,current_database(),'TEMP')")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert!(
        has_temp,
        "the shadow fixture deliberately has stronger TEMP privilege than production runtime"
    );
    sqlx::query("INSERT INTO pg_temp.academic_result_removal_scope SELECT pg_current_xact_id(),pg_backend_pid(),'academic_term_aggregate_revisions','DELETE',encode(sha256(convert_to(to_jsonb(r)::text,'UTF8')),'hex'),NULL FROM academic_term_aggregate_revisions r WHERE id=$1")
        .bind(revision).execute(&mut *tx).await.unwrap();
    sqlx::query("SAVEPOINT shadow_attempt")
        .execute(&mut *tx)
        .await
        .unwrap();
    let denied = sqlx::query("DELETE FROM academic_term_aggregate_revisions WHERE id=$1")
        .bind(revision)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        denied.as_database_error().unwrap().message(),
        "ACADEMIC_OFFICIAL_RESULT_IMMUTABLE"
    );
    sqlx::query("ROLLBACK TO SAVEPOINT shadow_attempt")
        .execute(&mut *tx)
        .await
        .unwrap();
    let fake = Uuid::new_v4();
    sqlx::query("INSERT INTO pg_temp.learning_offerings VALUES($1,$2,$3)")
        .bind(fake)
        .bind(year)
        .bind(term)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SAVEPOINT source_shadow")
        .execute(&mut *tx)
        .await
        .unwrap();
    let refused = sqlx::query("SELECT academic_remove_offering_results($1)")
        .bind(fake)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        refused.as_database_error().unwrap().message(),
        "ACADEMIC_CURRENT_OFFERING_NOT_FOUND"
    );
    sqlx::query("ROLLBACK TO SAVEPOINT source_shadow")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SAVEPOINT helper_attempt")
        .execute(&mut *tx)
        .await
        .unwrap();
    let denied = sqlx::query(
        "SELECT academic_result_removal_allow('academic_term_aggregate_revisions','{}',NULL)",
    )
    .execute(&mut *tx)
    .await
    .unwrap_err();
    assert_eq!(
        denied.as_database_error().unwrap().code().as_deref(),
        Some("42501")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT helper_attempt")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn discarded_lesson_evidence_queues_owned_file_cleanup_and_refuses_unverified_routing() {
    let pool = predecessor("current_ops_file_manifest").await;
    let (offering,group,term,student,student_year):(Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT o.id,g.id,o.academic_term_id,m.student_id,m.student_academic_year_id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id JOIN learning_group_students m ON m.learning_group_id=g.id WHERE NOT EXISTS(SELECT 1 FROM academic_delivery_versions v CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') graph WHERE v.id=academic_delivery_applicable_publication(o.academic_term_id,(now() AT TIME ZONE 'Asia/Bangkok')::date) AND (graph->>'id')::uuid=o.id) LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let actor: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type='staff' AND status='active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let file = Uuid::new_v4();
    let version = Uuid::new_v4();
    let unrelated = Uuid::new_v4();
    for id in [file, unrelated] {
        sqlx::query("INSERT INTO files(id,owner_user_id,display_filename,created_by,purpose_code,visibility,lifecycle_status) VALUES($1,$2,'sanitized-evidence.jpg',$2,'attendance_evidence','private','ready')")
            .bind(id).bind(actor).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO file_versions(id,file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status,created_by) VALUES($1,$2,1,'r2','private','stored',$3,'image/jpeg','jpg',1024,repeat('0',64),'clean',$4)")
        .bind(version).bind(file).bind(format!("attendance/sanitized/{file}.jpg")).bind(actor).execute(&pool).await.unwrap();
    sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
        .bind(file)
        .bind(version)
        .execute(&pool)
        .await
        .unwrap();
    let session = Uuid::new_v4();
    let device = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time) VALUES($1,$2,'2025-08-25','lesson',$3,'Sanitized lesson',ARRAY[$4]::uuid[],$5,$6,'10:00','11:00')")
        .bind(session).bind(term).bind(session.to_string()).bind(actor).bind(group).bind(offering).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin) VALUES($1,$2,$3,'present','teacher')")
        .bind(session).bind(student).bind(student_year).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_devices(id,name,paired_by,operator_id) VALUES($1,'Sanitized device',$2,$2)").bind(device).bind(actor).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_scan_events(id,session_id,student_id,device_id,evidence_file_id,captured_at) VALUES($1,$2,$3,$4,$5,now())")
        .bind(Uuid::new_v4()).bind(session).bind(student).bind(device).bind(file).execute(&pool).await.unwrap();
    let other_before: Value = sqlx::query_scalar("SELECT to_jsonb(f) FROM files f WHERE id=$1")
        .bind(unrelated)
        .fetch_one(&pool)
        .await
        .unwrap();
    let error = school_migrations::run_tenant_migrations_detailed(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "ACADEMIC_CURRENT_ATTENDANCE_EVIDENCE_UNMAPPABLE"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT lifecycle_status FROM files WHERE id=$1")
            .bind(file)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "ready"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_scan_events WHERE evidence_file_id=$1"
        )
        .bind(file)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('schoolorbit.storage_binding_attestation',repeat('b',64),true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE file_storage_cleanup_watches SET namespace_fingerprint=repeat('a',64),namespace_bound_at=now(),binding_evidence_digest=repeat('b',64) WHERE target_kind='version' AND target_id=$1")
        .bind(version).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    canonical(&pool).await;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT lifecycle_status FROM files WHERE id=$1")
            .bind(file)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "delete_requested"
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM file_operations WHERE file_id=$1 AND file_version_id=$2 AND operation_type='delete_object' AND status='pending'").bind(file).bind(version).fetch_one(&pool).await.unwrap(),1);
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT revoked_at IS NOT NULL FROM file_storage_cleanup_watches WHERE target_id=$1"
    )
    .bind(version)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_scan_events WHERE evidence_file_id=$1"
        )
        .bind(file)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        other_before,
        sqlx::query_scalar::<_, Value>("SELECT to_jsonb(f) FROM files f WHERE id=$1")
            .bind(unrelated)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn group_removal_keeps_other_groups_official_results_and_shared_lock_membership() {
    let pool = predecessor("current_ops_group_results").await;
    canonical(&pool).await;
    let (offering,group,term,year,student,subject,student_user):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT o.id,g.id,o.academic_term_id,o.academic_year_id,m.student_academic_year_id,d.subject_id,m.student_id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id JOIN learning_group_students m ON m.learning_group_id=g.id JOIN course_offering_details d ON d.learning_offering_id=o.id JOIN academic_terms t ON t.id=o.academic_term_id WHERE t.status='active' LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let other_group: Uuid = sqlx::query_scalar(
        "SELECT id FROM learning_groups WHERE learning_offering_id=$1 AND id<>$2 LIMIT 1",
    )
    .bind(offering)
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_student = Uuid::new_v4();
    let other_user = Uuid::new_v4();
    sqlx::query("INSERT INTO users SELECT (jsonb_populate_record(NULL::users,to_jsonb(u)||jsonb_build_object('id',$2::uuid,'national_id',NULL,'national_id_hash',NULL,'email',NULL,'username',$3::text))).* FROM users u WHERE id=$1")
        .bind(student_user).bind(other_user).bind(format!("current-fixture-{other_user}")).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years SELECT (jsonb_populate_record(NULL::student_academic_years,to_jsonb(s)||jsonb_build_object('id',$2::uuid,'student_id',$3::uuid))).* FROM student_academic_years s WHERE id=$1")
        .bind(student).bind(other_student).bind(other_user).execute(&pool).await.unwrap();
    let actor: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type='staff' AND status='active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let lock = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_course_result_locks(id,subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES($1,$2,$3,$4,'06000000-0000-0000-0000-000000000001','{}',repeat('0',64),repeat('0',64),$5,$6)")
        .bind(lock).bind(subject).bind(term).bind(year).bind(json!({"groups":[{"learningGroupId":group,"learningOfferingId":offering},{"learningGroupId":other_group,"learningOfferingId":offering}]})).bind(actor).execute(&pool).await.unwrap();
    let selected_result = Uuid::new_v4();
    let other_result = Uuid::new_v4();
    for (id, grp, learner) in [
        (selected_result, group, student),
        (other_result, other_group, other_student),
    ] {
        sqlx::query("INSERT INTO academic_course_results(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,course_result_lock_id,student_academic_year_id,calculated_score,calculated_grade,outcome,numeric_grade,selection_source) VALUES($1,$2,$3,$4,$5,$6,$7,$8,80,4,'numeric',4,'derived')")
            .bind(id).bind(grp).bind(offering).bind(term).bind(year).bind(subject).bind(lock).bind(learner).execute(&pool).await.unwrap();
    }
    let other_before: Value =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM academic_course_results r WHERE id=$1")
            .bind(other_result)
            .fetch_one(&pool)
            .await
            .unwrap();
    let impact: Value = sqlx::query_scalar(
        "SELECT academic_current_results_deletion_impact($1,ARRAY[$2]::uuid[],false)",
    )
    .bind(offering)
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(impact["officialResults"], 1);
    sqlx::query("SELECT academic_remove_group_results($1)")
        .bind(group)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_course_results WHERE id=$1")
            .bind(selected_result)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        other_before,
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(r) FROM academic_course_results r WHERE id=$1"
        )
        .bind(other_result)
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    let remaining: Value =
        sqlx::query_scalar("SELECT source_snapshot FROM academic_course_result_locks WHERE id=$1")
            .bind(lock)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        remaining["groups"],
        json!([{"learningGroupId":other_group,"learningOfferingId":offering}])
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM learning_offerings WHERE id=$1")
            .bind(offering)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn exact_legacy_lesson_source_adopts_placement_key_without_changing_any_saved_fact() {
    let pool = predecessor("current_ops_session_identity").await;
    let session = Uuid::new_v4();
    let placed:Uuid=sqlx::query_scalar("SELECT g.id FROM academic_timetable_block_groups g JOIN academic_timetable_blocks b ON b.id=g.block_id JOIN learning_group_students m ON m.learning_group_id=g.learning_group_id WHERE b.timetable_version_id=academic_timetable_applicable_publication(b.academic_term_id,(now() AT TIME ZONE 'Asia/Bangkok')::date) LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time,roster) SELECT $1,b.academic_term_id,'2025-08-25'::date+array_position(ARRAY['MON','TUE','WED','THU','FRI','SAT','SUN'],b.day_of_week)-1,'lesson',g.learning_group_id::text||':'||b.bell_schedule_period_id::text,'Retained lesson',ARRAY[]::uuid[],g.learning_group_id,g.learning_offering_id,p.start_time,p.end_time,jsonb_build_array(jsonb_build_object('studentId',m.student_id,'studentAcademicYearId',m.student_academic_year_id,'classNumber',NULL)) FROM academic_timetable_block_groups g JOIN academic_timetable_blocks b ON b.id=g.block_id JOIN bell_schedule_periods p ON p.id=b.bell_schedule_period_id JOIN learning_group_students m ON m.learning_group_id=g.learning_group_id WHERE g.id=$2 LIMIT 1")
        .bind(session).bind(placed).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin) SELECT s.id,(p->>'studentId')::uuid,(p->>'studentAcademicYearId')::uuid,'present','teacher' FROM attendance_sessions s CROSS JOIN LATERAL jsonb_array_elements(s.roster) p WHERE s.id=$1")
        .bind(session).execute(&pool).await.unwrap();
    let before: Value = sqlx::query_scalar(
        "SELECT to_jsonb(s)-'source_key' FROM attendance_sessions s WHERE id=$1",
    )
    .bind(session)
    .fetch_one(&pool)
    .await
    .unwrap();
    let records:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r) ORDER BY student_id) FROM attendance_records r WHERE session_id=$1").bind(session).fetch_one(&pool).await.unwrap();
    canonical(&pool).await;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT source_key FROM attendance_sessions WHERE id=$1")
            .bind(session)
            .fetch_one(&pool)
            .await
            .unwrap(),
        format!("placement:{placed}")
    );
    assert_eq!(
        before,
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(s)-'source_key' FROM attendance_sessions s WHERE id=$1"
        )
        .bind(session)
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    assert_eq!(records,sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(r) ORDER BY student_id) FROM attendance_records r WHERE session_id=$1").bind(session).fetch_one(&pool).await.unwrap());
}

#[tokio::test]
async fn intentional_empty_current_graph_detaches_legacy_supervision_and_keeps_shared_exam_resources(
) {
    let pool = predecessor("current_ops_empty_audience").await;
    let old_observation:(Uuid,Uuid,Value)=sqlx::query_as("SELECT id,timetable_block_group_id,to_jsonb(s)-'updated_at'-'timetable_block_group_id'-'learning_group_id'-'lesson_snapshot' FROM supervision_observations s LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let rooms: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_exam_day_room_assignments")
        .fetch_one(&pool)
        .await
        .unwrap();
    let seats: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_exam_seat_assignments")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("ALTER TABLE academic_delivery_versions DISABLE TRIGGER academic_delivery_versions_immutable; UPDATE academic_delivery_versions SET snapshot='{\"offerings\":[]}' WHERE academic_term_id=(SELECT id FROM academic_terms WHERE status='active'); ALTER TABLE academic_delivery_versions ENABLE TRIGGER academic_delivery_versions_immutable;")
        .execute(&pool).await.unwrap();
    let current = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,effective_from,status,source_version_id,bell_schedule_id,created_by,published_by,published_at,delivery_version_id,publication_no) SELECT $1,academic_term_id,academic_year_id,(now() AT TIME ZONE 'Asia/Bangkok')::date,'published',id,bell_schedule_id,created_by,published_by,now(),delivery_version_id,publication_no+1 FROM academic_timetable_versions WHERE academic_term_id=(SELECT id FROM academic_terms WHERE status='active') ORDER BY publication_no DESC LIMIT 1")
        .bind(current).execute(&pool).await.unwrap();
    canonical(&pool).await;
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM learning_offerings WHERE academic_term_id=(SELECT id FROM academic_terms WHERE status='active')").fetch_one(&pool).await.unwrap(),0);
    assert_eq!(sqlx::query_scalar::<_,Uuid>("SELECT id FROM academic_timetables WHERE academic_term_id=(SELECT id FROM academic_terms WHERE status='active')").fetch_one(&pool).await.unwrap(),current);
    let (source,detached,original):(Value,bool,Value)=sqlx::query_as("SELECT lesson_snapshot,timetable_block_group_id IS NULL AND learning_group_id IS NULL,to_jsonb(s)-'updated_at'-'timetable_block_group_id'-'learning_group_id'-'lesson_snapshot' FROM supervision_observations s WHERE id=$1")
        .bind(old_observation.0).fetch_one(&pool).await.unwrap();
    assert!(detached);
    assert_eq!(source["source"], "removed_timetable");
    assert_eq!(
        source["timetableBlockGroupId"],
        old_observation.1.to_string()
    );
    assert_eq!(original, old_observation.2);
    assert_eq!(
        rooms,
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_exam_day_room_assignments")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(
        seats,
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_exam_seat_assignments")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
}
