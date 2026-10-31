use chrono::NaiveDate;
use school_test_db::academic_context_receipt_fixture::seed_legacy_preparation_receipt;
use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use sqlx::PgPool;
use uuid::Uuid;

const YEAR: Uuid = Uuid::from_u128(0x10000000000000000000000000000025);
const NEXT_YEAR: Uuid = Uuid::from_u128(0x10000000000000000000000000000026);
const TERM: Uuid = Uuid::from_u128(0x11000000000000000000000000000251);
const ACTOR: Uuid = Uuid::from_u128(0x50000000000000000000000000000001);

async fn predecessor(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 146).await.unwrap();
    pool
}
async fn scoped_target(pool: &PgPool, date: &str, year: Option<Uuid>) -> Uuid {
    let event:Uuid=sqlx::query_scalar("INSERT INTO calendar_events(title,start_date,end_date,all_day,created_by,updated_at) VALUES('Migration event',$1::text::date,$1::text::date,true,$2,'2001-01-01') RETURNING id")
        .bind(date).bind(ACTOR).fetch_one(pool).await.unwrap();
    sqlx::query_scalar("INSERT INTO calendar_event_targets(event_id,academic_year_id,audience_type,grade_level_id,updated_at) SELECT $1,$2,'student',id,'2001-01-02' FROM grade_levels ORDER BY id LIMIT 1 RETURNING id")
        .bind(event).bind(year).fetch_one(pool).await.unwrap()
}
async fn assert_refusal(pool: &PgPool, code: &str) {
    let error = apply_migrations_through(pool, 148).await.unwrap_err();
    assert!(
        error.to_string().contains(code),
        "expected {code}, received {error}"
    );
    let retained:i64=sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='academic_terms' AND column_name IN('start_date','planned_end_date','closed_on')").fetch_one(pool).await.unwrap();
    assert_eq!(retained, 3, "destructive cleanup must roll back atomically");
    let applied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE version=148")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(applied, 0);
    let audit: Option<String> = sqlx::query_scalar(
        "SELECT to_regclass(current_schema()||'.academic_context_cutover_audit')::text",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(audit.is_none());
}

#[tokio::test]
async fn populated_cutover_preserves_actual_facts_links_calendars_defaults_and_replays() {
    let pool = predecessor("context148_populated").await;
    let target = scoped_target(&pool, "2025-05-02", None).await;
    let explicit = scoped_target(&pool, "2025-05-03", Some(NEXT_YEAR)).await;
    sqlx::query("INSERT INTO attendance_settings(academic_term_id,configuration,updated_by) VALUES($1,'{\"enabled\":true,\"weekdays\":[],\"lateAfter\":\"08:00:00\",\"digestTimes\":[],\"evidenceDays\":30,\"faceDistance\":0.45,\"faceMargin\":0.1,\"activityCountsAsPresent\":true}',$2)").bind(TERM).bind(ACTOR).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_days(academic_term_id,date,counted,note) VALUES($1,'2025-05-02',true,'explicit included'),($1,'2025-05-03',false,'explicit excluded')").bind(TERM).execute(&pool).await.unwrap();
    sqlx::query("UPDATE behavior_settings SET merit_reset='term',deduction_reset='year'")
        .execute(&pool)
        .await
        .unwrap();
    let expected_resets:Vec<(String,NaiveDate)>=sqlx::query_as("SELECT 'merit'::text,COALESCE(t.closed_on,t.planned_end_date,y.end_date) FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.status IN('active','closing','closed') AND y.status IN('active','closing','closed','archived') UNION SELECT 'deduction',end_date FROM academic_years WHERE status IN('active','closing','closed','archived') ORDER BY 1,2").fetch_all(&pool).await.unwrap();
    let policy = Uuid::new_v4();
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_promotion_policy_versions(id,name,rules,progression_row_version,reviewed_by) VALUES($1,'Migration policy','[{}]',1,$2)").bind(policy).bind(ACTOR).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_promotion_runs(id,source_year_id,target_year_id,policy_id,request_id,request_checksum,created_by) VALUES($1,$2,$3,$4,$5,repeat('a',64),$6)").bind(run).bind(YEAR).bind(NEXT_YEAR).bind(policy).bind(Uuid::new_v4()).bind(ACTOR).execute(&pool).await.unwrap();
    let preserved:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM homeroom_placements),(SELECT count(*) FROM learning_offerings),(SELECT count(*) FROM academic_timetable_versions)").fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 148).await.unwrap();
    let mapped: (Uuid, String) = sqlx::query_as(
        "SELECT academic_year_id,updated_at::date::text FROM calendar_event_targets WHERE id=$1",
    )
    .bind(target)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mapped, (YEAR, "2001-01-02".into()));
    let explicit_year: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM calendar_event_targets WHERE id=$1")
            .bind(explicit)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        explicit_year, NEXT_YEAR,
        "existing explicit links never retarget by date"
    );
    let days:Vec<(NaiveDate,bool,String)>=sqlx::query_as("SELECT date,counted,note FROM attendance_days WHERE academic_term_id=$1 AND date BETWEEN '2025-05-01' AND '2025-05-04' ORDER BY date").bind(TERM).fetch_all(&pool).await.unwrap();
    assert_eq!(
        days.iter().map(|row| row.1).collect::<Vec<_>>(),
        vec![false, true, false, false]
    );
    assert_eq!(days[1].2, "explicit included");
    assert_eq!(days[2].2, "explicit excluded");
    let reset_dates: Vec<(String, NaiveDate)> =
        sqlx::query_as("SELECT side,resets_on FROM behavior_reset_dates ORDER BY 1,2")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(reset_dates, expected_resets);
    let intent: NaiveDate = sqlx::query_scalar(
        "SELECT target_placement_starts_on FROM academic_promotion_runs WHERE id=$1",
    )
    .bind(run)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(intent, NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
    let after:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM homeroom_placements),(SELECT count(*) FROM learning_offerings),(SELECT count(*) FROM academic_timetable_versions)").fetch_one(&pool).await.unwrap();
    assert_eq!(after, preserved);
    let defaults: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT default_academic_year_id,default_academic_term_id FROM academic_context_settings",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(defaults, (Some(YEAR), Some(TERM)));
    let hidden: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_years WHERE status='planning' AND is_visible",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(hidden, 0);
    let snapshot: serde_json::Value =
        sqlx::query_scalar("SELECT retained_facts FROM academic_context_cutover_audit")
            .fetch_one(&pool)
            .await
            .unwrap();
    apply_migrations_through(&pool, 148).await.unwrap();
    let replay: serde_json::Value =
        sqlx::query_scalar("SELECT retained_facts FROM academic_context_cutover_audit")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(snapshot, replay);
}

#[tokio::test]
async fn missing_calendar_year_refuses_zero_or_ambiguous_historical_matches() {
    for ambiguous in [false, true] {
        let pool = predecessor(if ambiguous {
            "context148_ambiguous"
        } else {
            "context148_missing"
        })
        .await;
        if ambiguous {
            sqlx::query("INSERT INTO academic_years(year,name,start_date,end_date,status) VALUES(2901,'Overlapping historical year','2025-05-01','2026-04-30','planning')").execute(&pool).await.unwrap();
        }
        let id = scoped_target(
            &pool,
            if ambiguous {
                "2025-05-02"
            } else {
                "2199-01-01"
            },
            None,
        )
        .await;
        assert_refusal(&pool, "ACADEMIC_CONTEXT_148_CALENDAR_YEAR_UNMAPPABLE").await;
        let year: Option<Uuid> =
            sqlx::query_scalar("SELECT academic_year_id FROM calendar_event_targets WHERE id=$1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(year, None);
    }
}

#[tokio::test]
async fn unknown_date_functions_checks_and_views_refuse_cleanup() {
    for kind in [
        "function",
        "check",
        "tampered_known_check",
        "tampered_known_view",
        "view",
    ] {
        let pool = predecessor(&format!("context148_unknown_{kind}")).await;
        apply_migrations_through(&pool, 147).await.unwrap();
        let original_exam_view: String = sqlx::query_scalar(
            "SELECT pg_get_viewdef('academic_exam_eligible_sources'::regclass,false)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        match kind {
            "function" => {
                sqlx::raw_sql("CREATE FUNCTION surprise_boundary() RETURNS date LANGUAGE plpgsql AS $$ BEGIN RETURN (SELECT END_DATE FROM ACADEMIC_YEARS LIMIT 1); END $$").execute(&pool).await.unwrap();
            }
            "check" => {
                sqlx::raw_sql("ALTER TABLE academic_years ADD CONSTRAINT surprise_boundary CHECK(end_date-start_date<400)").execute(&pool).await.unwrap();
            }
            "tampered_known_view" => {
                sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                    "CREATE OR REPLACE VIEW academic_exam_eligible_sources AS {} AND false",
                    original_exam_view.trim_end().trim_end_matches(';')
                )))
                .execute(&pool)
                .await
                .unwrap();
            }
            "tampered_known_check" => {
                sqlx::raw_sql("ALTER TABLE academic_years DROP CONSTRAINT academic_years_date_order_check; ALTER TABLE academic_years ADD CONSTRAINT academic_years_date_order_check CHECK(start_date<=end_date AND year>2000)").execute(&pool).await.unwrap();
            }
            _ => {
                sqlx::raw_sql(
                    "CREATE VIEW surprise_boundary AS SELECT start_date FROM academic_years",
                )
                .execute(&pool)
                .await
                .unwrap();
            }
        }
        assert_refusal(
            &pool,
            if kind == "view" {
                "depend"
            } else {
                "ACADEMIC_CONTEXT_148_UNKNOWN_DATE_DEPENDENCY"
            },
        )
        .await;
        match kind {
            "function" => {
                sqlx::raw_sql("DROP FUNCTION surprise_boundary()")
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            "check" => {
                sqlx::raw_sql("ALTER TABLE academic_years DROP CONSTRAINT surprise_boundary")
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            "tampered_known_view" => {
                sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                    "CREATE OR REPLACE VIEW academic_exam_eligible_sources AS {original_exam_view}"
                )))
                .execute(&pool)
                .await
                .unwrap();
            }
            "tampered_known_check" => {
                sqlx::raw_sql("ALTER TABLE academic_years DROP CONSTRAINT academic_years_date_order_check; ALTER TABLE academic_years ADD CONSTRAINT academic_years_date_order_check CHECK(start_date<=end_date)").execute(&pool).await.unwrap();
            }
            _ => {
                sqlx::raw_sql("DROP VIEW surprise_boundary")
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        apply_migrations_through(&pool, 148).await.unwrap();
    }
}

#[tokio::test]
async fn transformed_target_reassignment_refuses_even_when_foreign_keys_are_valid() {
    let pool = predecessor("context148_wrong_target").await;
    let id = scoped_target(&pool, "2025-05-02", None).await;
    apply_migrations_through(&pool, 147).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION surprise_mapping() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.academic_year_id:='10000000-0000-0000-0000-000000000026'; RETURN NEW; END $$; CREATE TRIGGER surprise_mapping BEFORE UPDATE ON calendar_event_targets FOR EACH ROW EXECUTE FUNCTION surprise_mapping()").execute(&pool).await.unwrap();
    assert_refusal(&pool, "ACADEMIC_CONTEXT_148_RECONCILIATION_FAILED").await;
    let year: Option<Uuid> =
        sqlx::query_scalar("SELECT academic_year_id FROM calendar_event_targets WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(year, None);
    sqlx::raw_sql(
        "DROP TRIGGER surprise_mapping ON calendar_event_targets; DROP FUNCTION surprise_mapping()",
    )
    .execute(&pool)
    .await
    .unwrap();
    apply_migrations_through(&pool, 148).await.unwrap();
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OldTermRequest {
    academic_year_id: Uuid,
    request_id: Uuid,
    action: String,
    expected_year_version: i64,
    expected_term_version: i64,
    readiness_checksum: String,
    acknowledged_warning_codes: Vec<String>,
    closed_on: Option<NaiveDate>,
    reason: Option<String>,
}
async fn old_term_receipt(
    pool: &PgPool,
    audit: bool,
) -> (Uuid, serde_json::Value, serde_json::Value, String) {
    use sha2::{Digest, Sha256};
    let id = Uuid::new_v4();
    let request = OldTermRequest {
        academic_year_id: YEAR,
        request_id: id,
        action: "reopen".into(),
        expected_year_version: 2,
        expected_term_version: 3,
        readiness_checksum: "a".repeat(64),
        acknowledged_warning_codes: vec![],
        closed_on: None,
        reason: Some("แก้ไข  ผล\nสองบรรทัด \"quoted\"".into()),
    };
    let checksum = hex::encode(Sha256::digest(
        serde_json::to_vec(&(ACTOR, TERM, &request)).unwrap(),
    ));
    let request = serde_json::to_value(request).unwrap();
    let bell: Uuid = sqlx::query_scalar("SELECT bell_schedule_id FROM academic_terms WHERE id=$1")
        .bind(TERM)
        .fetch_one(pool)
        .await
        .unwrap();
    let outcome = serde_json::json!({"requestId":id,"action":"reopen","completedAt":"2025-05-01T17:30:00Z","context":{
        "academicYearId":YEAR,"academicTermId":TERM,"yearName":"Historical year","termName":"Historical term","yearStatus":"active","termStatus":"closing",
        "yearRowVersion":3,"termRowVersion":4,"yearStartDate":"2025-05-01","yearEndDate":"2026-04-30","termStartDate":"2025-05-01","plannedEndDate":"2025-10-31","closedOn":null,
        "sequence":1,"bellScheduleId":bell,"includedInYearResult":true,"blocksYearClosure":true
    }});
    sqlx::query("INSERT INTO academic_term_transition_receipts(request_id,academic_year_id,academic_term_id,actor_user_id,action,request_checksum,accepted_readiness,outcome,created_at) VALUES($1,$2,$3,$4,'reopen',$5,$6,$7,'2025-05-01T17:30:00Z')")
        .bind(id).bind(YEAR).bind(TERM).bind(ACTOR).bind(&checksum).bind(sqlx::types::Json(serde_json::json!({"context":outcome["context"]}))).bind(sqlx::types::Json(&outcome)).execute(pool).await.unwrap();
    if audit {
        sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload) VALUES('academic.term.reopen','academic_term',$1,$2,$1,$3,$4)")
            .bind(TERM).bind(YEAR).bind(ACTOR).bind(sqlx::types::Json(serde_json::json!([request,outcome]))).execute(pool).await.unwrap();
    }
    (id, request, outcome, checksum)
}

#[tokio::test]
async fn historical_term_request_mapping_preserves_full_checksum_and_real_event_day() {
    let pool = predecessor("context148_term_receipt").await;
    let (id, mut request, old_outcome, old_checksum) = old_term_receipt(&pool, true).await;
    apply_migrations_through(&pool, 148).await.unwrap();
    request.as_object_mut().unwrap().remove("closedOn");
    let(canonical,outcome,checksum,evidence):(serde_json::Value,serde_json::Value,String,serde_json::Value)=sqlx::query_as("SELECT canonical_request,outcome,request_checksum,accepted_readiness FROM academic_term_transition_receipts WHERE request_id=$1").bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(canonical, request);
    assert_eq!(checksum, old_checksum);
    assert_eq!(outcome["context"]["operationDate"], "2025-05-02");
    assert_eq!(outcome["completedAt"], old_outcome["completedAt"]);
    assert!(outcome["context"].get("plannedEndDate").is_none());
    assert_eq!(
        evidence["context"], old_outcome["context"],
        "original accepted readiness remains historical evidence"
    );
    let audit:serde_json::Value=sqlx::query_scalar("SELECT payload FROM academic_audit_events WHERE event_code='academic.term.reopen' AND entity_id=$1").bind(TERM).fetch_one(&pool).await.unwrap();
    assert_eq!(
        audit[1], old_outcome,
        "original audit is not replaced by a fabricated current result"
    );
    apply_migrations_through(&pool, 148).await.unwrap();
}

#[tokio::test]
async fn missing_or_changed_original_term_request_refuses_before_cleanup() {
    for missing in [true, false] {
        let pool = predecessor(if missing {
            "context148_receipt_missing"
        } else {
            "context148_receipt_changed"
        })
        .await;
        let (id, _, _, _) = old_term_receipt(&pool, !missing).await;
        if !missing {
            sqlx::query("UPDATE academic_audit_events SET payload=jsonb_set(payload,'{0,reason}','\"changed evidence\"') WHERE entity_id=$1 AND event_code='academic.term.reopen'").bind(TERM).execute(&pool).await.unwrap();
        }
        assert_refusal(&pool, "ACADEMIC_CONTEXT_148_TERM_RECEIPT_SOURCE_UNMAPPABLE").await;
        let same: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM academic_term_transition_receipts WHERE request_id=$1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(same, 1);
    }
}

#[tokio::test]
async fn preparation_mapping_checksum_and_independent_revision_dates_are_preserved() {
    let pool = predecessor("context148_preparation_valid").await;
    let (nonce, version, revision, checksum) = seed_legacy_preparation_receipt(&pool, 0).await;
    let before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_array((SELECT to_jsonb(v) FROM academic_delivery_versions v WHERE id=$1),(SELECT to_jsonb(r) FROM academic_term_change_sets r WHERE id=$2))")
        .bind(version).bind(revision).fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 148).await.unwrap();
    let(date,stored):(Option<NaiveDate>,String)=sqlx::query_as("SELECT offering_starts_on,request_checksum FROM academic_term_preparation_runs WHERE request_id=$1").bind(nonce).fetch_one(&pool).await.unwrap();
    assert_eq!(
        date, None,
        "the legacy request has no accepted explicit date constraint; its revision remains dated"
    );
    assert_eq!(stored, checksum);
    let after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_array((SELECT to_jsonb(v) FROM academic_delivery_versions v WHERE id=$1),(SELECT to_jsonb(r) FROM academic_term_change_sets r WHERE id=$2))")
        .bind(version).bind(revision).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn preparation_tampered_checksum_mapping_or_version_proof_refuses_cleanup() {
    for kind in ["checksum", "mapping", "version"] {
        let pool = predecessor(&format!("context148_preparation_{kind}")).await;
        let (nonce, version, _, _) = seed_legacy_preparation_receipt(&pool, 0).await;
        sqlx::raw_sql("ALTER TABLE academic_term_preparation_runs DISABLE TRIGGER academic_term_preparation_runs_immutable").execute(&pool).await.unwrap();
        match kind {
            "checksum" => {
                sqlx::query("UPDATE academic_term_preparation_runs SET request_checksum=repeat('e',64) WHERE request_id=$1").bind(nonce).execute(&pool).await.unwrap();
            }
            "mapping" => {
                sqlx::query("UPDATE academic_term_preparation_runs SET mappings=jsonb_set(mappings,'{dates,0,targetDate}','\"2026-05-04\"') WHERE request_id=$1").bind(nonce).execute(&pool).await.unwrap();
            }
            _ => {
                sqlx::query("DELETE FROM academic_audit_events WHERE entity_id=$1 AND event_code='academic_delivery_version.created'").bind(version).execute(&pool).await.unwrap();
            }
        }
        sqlx::raw_sql("ALTER TABLE academic_term_preparation_runs ENABLE TRIGGER academic_term_preparation_runs_immutable").execute(&pool).await.unwrap();
        assert_refusal(
            &pool,
            if kind == "version" {
                "ACADEMIC_CONTEXT_148_PREPARATION_CREATED_INTENT_UNMAPPABLE"
            } else {
                "ACADEMIC_CONTEXT_148_PREPARATION_REQUEST_UNMAPPABLE"
            },
        )
        .await;
    }
}

#[tokio::test]
async fn preparation_mixed_retained_and_created_offerings_keep_distinct_dates() {
    let pool = predecessor("context148_preparation_mixed").await;
    let (nonce, version, revision, checksum) = seed_legacy_preparation_receipt(&pool, 1).await;
    let before:Vec<(Uuid,NaiveDate)>=sqlx::query_as("SELECT id,starts_on FROM learning_offerings WHERE id IN(SELECT value::uuid FROM academic_term_preparation_runs ledger CROSS JOIN LATERAL jsonb_array_elements_text(ledger.outcome->0->'targetIds') value WHERE ledger.request_id=$1) ORDER BY id").bind(nonce).fetch_all(&pool).await.unwrap();
    assert_eq!(before.len(), 2);
    assert_ne!(before[0].1, before[1].1);
    apply_migrations_through(&pool, 148).await.unwrap();
    let(date,stored):(Option<NaiveDate>,String)=sqlx::query_as("SELECT offering_starts_on,request_checksum FROM academic_term_preparation_runs WHERE request_id=$1").bind(nonce).fetch_one(&pool).await.unwrap();
    assert_eq!(
        date, None,
        "legacy implicit creation dates never become accepted explicit constraints"
    );
    assert_eq!(stored, checksum);
    let after: Vec<(Uuid, NaiveDate)> =
        sqlx::query_as("SELECT id,starts_on FROM learning_offerings WHERE id=ANY($1) ORDER BY id")
            .bind(before.iter().map(|item| item.0).collect::<Vec<_>>())
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    let references: (NaiveDate,NaiveDate)=sqlx::query_as("SELECT version.reference_date,revision.reference_date FROM academic_delivery_versions version CROSS JOIN academic_term_change_sets revision WHERE version.id=$1 AND revision.id=$2").bind(version).bind(revision).fetch_one(&pool).await.unwrap();
    assert_eq!(
        references,
        (
            NaiveDate::from_ymd_opt(2026, 5, 10).unwrap(),
            NaiveDate::from_ymd_opt(2026, 5, 10).unwrap()
        )
    );
    apply_migrations_through(&pool, 148).await.unwrap();
}

fn camel_record(value: serde_json::Value) -> serde_json::Value {
    serde_json::Value::Object(
        value
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| {
                let mut parts = key.split('_');
                let mut camel = parts.next().unwrap().to_owned();
                for part in parts {
                    let mut characters = part.chars();
                    if let Some(first) = characters.next() {
                        camel.extend(first.to_uppercase());
                        camel.extend(characters);
                    }
                }
                (camel, value.clone())
            })
            .collect(),
    )
}
async fn old_promotion_history(pool: &PgPool) -> (Uuid, Vec<(Uuid, Option<NaiveDate>)>) {
    let policy = Uuid::new_v4();
    let run = Uuid::new_v4();
    let approval = Uuid::new_v4();
    let request = Uuid::new_v4();
    let (grade,program):(Uuid,Uuid)=sqlx::query_as("SELECT grade_level_id,study_program_id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 1").bind(YEAR).fetch_one(pool).await.unwrap();
    let room:Uuid=sqlx::query_scalar("INSERT INTO homerooms(code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity) VALUES($1,'Historical promotion room',$2,$3,$4,$1,40) RETURNING id").bind(format!("MIG-{}",&run.simple().to_string()[..12])).bind(NEXT_YEAR).bind(grade).bind(program).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO academic_promotion_policy_versions(id,name,rules,progression_row_version,reviewed_by) VALUES($1,'Historical mixed policy','[{}]',1,$2)").bind(policy).bind(ACTOR).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO academic_promotion_runs(id,source_year_id,target_year_id,policy_id,request_id,request_checksum,created_by) VALUES($1,$2,$3,$4,$5,repeat('a',64),$6)").bind(run).bind(YEAR).bind(NEXT_YEAR).bind(policy).bind(Uuid::new_v4()).bind(ACTOR).execute(pool).await.unwrap();
    let mut rows = Vec::new();
    let mut receipts = Vec::new();
    let mut snapshots = Vec::new();
    for index in 0..4 {
        let student = Uuid::new_v4();
        let source = Uuid::new_v4();
        let item = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type) VALUES($1,$1::text,'!','Historical','Promotion','student')").bind(student).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES($1,$2,$3,$4,$5,'active')").bind(source).bind(student).bind(YEAR).bind(grade).bind(program).execute(pool).await.unwrap();
        let placed = index < 2;
        let held = index == 2;
        let decision = serde_json::json!({"outcome":if held{"hold"}else{"promote"},"targetGradeLevelId":if held{None}else{Some(grade)},"targetStudyProgramId":if held{None}else{Some(program)},"targetHomeroomId":if held{None}else{Some(room)},"reason":if held{Some("Historical hold")}else{None},"condition":null});
        let snapshot:serde_json::Value=sqlx::query_scalar("INSERT INTO academic_promotion_run_items(id,run_id,source_year_id,target_year_id,student_academic_year_id,student_id,source_grade_level_id,source_study_program_id,source_row_version,source_checksum,recommendation,decision,reviewed_by,reviewed_at,status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,1,repeat('b',64),'{}',$9,$10,now(),$11) RETURNING to_jsonb(academic_promotion_run_items)")
            .bind(item).bind(run).bind(YEAR).bind(NEXT_YEAR).bind(source).bind(student).bind(grade).bind(program).bind(sqlx::types::Json(&decision)).bind(ACTOR).bind(if index<3{"executed"}else{"reviewed"}).fetch_one(pool).await.unwrap();
        let mut snapshot = camel_record(snapshot);
        snapshot["status"] = serde_json::json!("reviewed");
        snapshots.push(snapshot);
        let date = if placed {
            NaiveDate::from_ymd_opt(2026, 5 + index, 1)
        } else if held {
            None
        } else {
            NaiveDate::from_ymd_opt(2026, 5, 1)
        };
        if index < 3 {
            let (target_student, placement) = if placed {
                let target_student = Uuid::new_v4();
                let placement = Uuid::new_v4();
                sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES($1,$2,$3,$4,$5,'planned')").bind(target_student).bind(student).bind(NEXT_YEAR).bind(grade).bind(program).execute(pool).await.unwrap();
                sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) VALUES($1,$2,$3,$4,$5,'planned','promotion')").bind(placement).bind(target_student).bind(NEXT_YEAR).bind(room).bind(date).execute(pool).await.unwrap();
                (Some(target_student), Some(placement))
            } else {
                (None, None)
            };
            receipts.push((item, student, target_student, placement));
        }
        rows.push((item, date));
    }
    sqlx::query("INSERT INTO academic_promotion_run_approvals(id,run_id,approved_run_version,source_checksum,intent,approved_by) VALUES($1,$2,1,repeat('c',64),$3,$4)").bind(approval).bind(run).bind(sqlx::types::Json(serde_json::json!({"items":snapshots}))).bind(ACTOR).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO academic_promotion_execution_batches(request_id,run_id,approval_id,actor_user_id,request_checksum,item_ids) VALUES($1,$2,$3,$4,repeat('d',64),$5)").bind(request).bind(run).bind(approval).bind(ACTOR).bind(receipts.iter().map(|row|row.0).collect::<Vec<_>>()).execute(pool).await.unwrap();
    for (item, student, target_student, placement) in receipts {
        sqlx::query("INSERT INTO academic_promotion_execution_receipts(item_id,run_id,request_id,approval_id,source_year_id,target_year_id,student_id,target_student_year_id,target_placement_id,source_row_version,executed_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,1,$10)")
            .bind(item).bind(run).bind(request).bind(approval).bind(YEAR).bind(NEXT_YEAR).bind(student).bind(target_student).bind(placement).bind(ACTOR).execute(pool).await.unwrap();
    }
    (run, rows)
}
#[tokio::test]
async fn promotion_mixed_executed_dates_hold_and_pending_default_are_preserved() {
    let pool = predecessor("context148_promotion_mixed").await;
    let (run, expected) = old_promotion_history(&pool).await;
    let placements:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(placement) ORDER BY placement.id) FROM homeroom_placements placement").fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 148).await.unwrap();
    for (item, date) in expected {
        let actual: Option<NaiveDate> = sqlx::query_scalar(
            "SELECT target_placement_starts_on FROM academic_promotion_run_items WHERE id=$1",
        )
        .bind(item)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(actual, date);
        let approved:Option<NaiveDate>=sqlx::query_scalar("SELECT (item->>'targetPlacementStartsOn')::date FROM academic_promotion_run_approvals approval CROSS JOIN LATERAL jsonb_array_elements(approval.intent->'items') item WHERE approval.run_id=$1 AND item->>'id'=$2").bind(run).bind(item.to_string()).fetch_one(&pool).await.unwrap();
        assert_eq!(approved, date);
    }
    let after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(placement) ORDER BY placement.id) FROM homeroom_placements placement").fetch_one(&pool).await.unwrap();
    assert_eq!(placements, after);
    let checksum: String =
        sqlx::query_scalar("SELECT request_checksum FROM academic_promotion_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(checksum, "a".repeat(64));
}
#[tokio::test]
async fn promotion_receipt_for_nonexecuted_item_refuses_cleanup() {
    let pool = predecessor("context148_promotion_receipt_status").await;
    let (_, items) = old_promotion_history(&pool).await;
    sqlx::raw_sql("ALTER TABLE academic_promotion_run_items DISABLE TRIGGER academic_promotion_run_items_identity").execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_promotion_run_items SET status='reviewed' WHERE id=$1")
        .bind(items[0].0)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("ALTER TABLE academic_promotion_run_items ENABLE TRIGGER academic_promotion_run_items_identity").execute(&pool).await.unwrap();
    assert_refusal(
        &pool,
        "ACADEMIC_CONTEXT_148_PROMOTION_ITEM_INTENT_UNMAPPABLE",
    )
    .await;
}

async fn assert_retimed_legacy_receipts_preserved(created: usize) {
    use school_test_db::academic_context_receipt_fixture::{
        legacy_offering_receipt_facts, retime_legacy_created_offerings,
        seed_legacy_curriculum_receipt,
    };
    for preparation in [false, true] {
        let pool = predecessor(&format!("context148_retimed_{created}_{preparation}")).await;
        let (nonce, target, original_hash) = if preparation {
            let (nonce, _, _, checksum) = seed_legacy_preparation_receipt(&pool, created).await;
            let target:Uuid=sqlx::query_scalar("SELECT target_academic_term_id FROM academic_term_preparation_runs WHERE request_id=$1").bind(nonce).fetch_one(&pool).await.unwrap();
            (nonce, target, checksum)
        } else {
            let receipt = seed_legacy_curriculum_receipt(&pool, created).await;
            (
                receipt.request_id,
                receipt.target_term_id,
                receipt.request_hash,
            )
        };
        let actual_dates = retime_legacy_created_offerings(&pool, nonce, preparation).await;
        assert_eq!(actual_dates.len(), created);
        if created == 2 {
            assert_ne!(actual_dates[0].1, actual_dates[1].1);
        }
        let before = legacy_offering_receipt_facts(&pool, target).await;
        apply_migrations_through(&pool, 148).await.unwrap();
        let (bound, checksum): (Option<NaiveDate>, String) = if preparation {
            sqlx::query_as("SELECT offering_starts_on,request_checksum FROM academic_term_preparation_runs WHERE request_id=$1").bind(nonce).fetch_one(&pool).await.unwrap()
        } else {
            sqlx::query_as("SELECT starts_on,request_hash::text FROM learning_delivery_apply_runs WHERE idempotency_key=$1").bind(nonce).fetch_one(&pool).await.unwrap()
        };
        assert_eq!(bound,None,"legacy DTOs accepted no explicit offering date, even when the receipt created offerings");
        assert_eq!(checksum, original_hash);
        assert_eq!(legacy_offering_receipt_facts(&pool,target).await,before,"all evolved output dates, exact identities, request hashes and original audit facts remain unchanged");
        apply_migrations_through(&pool, 148).await.unwrap();
        assert_eq!(legacy_offering_receipt_facts(&pool, target).await, before);
    }
}

#[tokio::test]
async fn legacy_single_publication_retimed_offering_preserves_unbound_receipt() {
    assert_retimed_legacy_receipts_preserved(1).await;
}

#[tokio::test]
async fn legacy_divergent_publication_subsets_preserve_unbound_receipts() {
    assert_retimed_legacy_receipts_preserved(2).await;
}

#[tokio::test]
async fn attendance_captured_dates_after_later_closure_keep_their_old_counting_meaning() {
    let pool = predecessor("context148_attendance_captured_dates").await;
    let captured = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap(); // Thursday, captured before the later backdated closure.
    let finalized = captured.succ_opt().unwrap();
    let (student_year,student):(Uuid,Uuid)=sqlx::query_as("SELECT id,student_id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 1").bind(YEAR).fetch_one(&pool).await.unwrap();
    let teacher: Uuid =
        sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,start_time,end_time,roster,saved_at,saved_by) VALUES($1,$2,$3,'arrival','preserved-arrival','Captured arrival',$4,'08:00','08:30',$5,'2025-10-09T01:00:00Z',$6)")
        .bind(session).bind(TERM).bind(captured).bind(vec![teacher])
        .bind(sqlx::types::Json(serde_json::json!([{"studentId":student,"studentAcademicYearId":student_year,"classNumber":1}])))
        .bind(teacher).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin,updated_by) VALUES($1,$2,$3,'present','teacher',$4)")
        .bind(session).bind(student).bind(student_year).bind(teacher).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_finalized_dates(generation,academic_term_id,date,processed_at) SELECT generation,$1,$2,'2025-10-10T17:00:00Z' FROM attendance_module_state")
        .bind(TERM).bind(finalized).execute(&pool).await.unwrap();
    // Old closure allowed an earlier date than already captured sessions; the
    // reporting/score predicates still counted the real captured weekday.
    sqlx::query("UPDATE academic_terms SET closed_on='2025-10-01',status='closed' WHERE id=$1")
        .bind(TERM)
        .execute(&pool)
        .await
        .unwrap();
    let old_counted:bool=sqlx::query_scalar("SELECT COALESCE(session.count_override,day.counted,COALESCE(settings.configuration->'weekdays','[1,2,3,4,5]'::jsonb) @> to_jsonb(EXTRACT(ISODOW FROM session.date)::integer)) FROM attendance_sessions session LEFT JOIN attendance_days day ON day.academic_term_id=session.academic_term_id AND day.date=session.date LEFT JOIN attendance_settings settings ON settings.academic_term_id=session.academic_term_id WHERE session.id=$1")
        .bind(session).fetch_one(&pool).await.unwrap();
    assert!(old_counted);
    let facts_sql = r#"SELECT jsonb_build_object(
        'sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM attendance_sessions s),
        'records',(SELECT jsonb_agg(to_jsonb(r) ORDER BY session_id,student_id) FROM attendance_records r),
        'finalization',(SELECT jsonb_agg(to_jsonb(f) ORDER BY generation,academic_term_id,date) FROM attendance_finalized_dates f))"#;
    let before: serde_json::Value = sqlx::query_scalar(facts_sql)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_days WHERE academic_term_id=$1 AND date IN($2,$3)"
        )
        .bind(TERM)
        .bind(captured)
        .bind(finalized)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    apply_migrations_through(&pool, 148).await.unwrap();
    for date in [captured, finalized] {
        let decision: Option<bool> = sqlx::query_scalar(
            "SELECT counted FROM attendance_days WHERE academic_term_id=$1 AND date=$2",
        )
        .bind(TERM)
        .bind(date)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(decision,Some(true),"persisted fact dates keep the old weekday decision after academic boundaries disappear");
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_days WHERE academic_term_id=$1 AND date='2025-10-16'"
        )
        .bind(TERM)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0,
        "an unrelated weekday beyond the closed range is not invented"
    );
    let after: serde_json::Value = sqlx::query_scalar(facts_sql)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, before);
    apply_migrations_through(&pool, 148).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(facts_sql)
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
}
