use chrono::NaiveDate;
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
    apply_migrations_through(&pool, 145).await.unwrap();
    pool
}
async fn scoped_target(pool: &PgPool, date: &str, year: Option<Uuid>) -> Uuid {
    let event:Uuid=sqlx::query_scalar("INSERT INTO calendar_events(title,start_date,end_date,all_day,created_by,updated_at) VALUES('Migration event',$1::text::date,$1::text::date,true,$2,'2001-01-01') RETURNING id")
        .bind(date).bind(ACTOR).fetch_one(pool).await.unwrap();
    sqlx::query_scalar("INSERT INTO calendar_event_targets(event_id,academic_year_id,audience_type,grade_level_id,updated_at) SELECT $1,$2,'student',id,'2001-01-02' FROM grade_levels ORDER BY id LIMIT 1 RETURNING id")
        .bind(event).bind(year).fetch_one(pool).await.unwrap()
}
async fn assert_refusal(pool: &PgPool, code: &str) {
    let error = apply_migrations_through(pool, 147).await.unwrap_err();
    assert!(
        error.to_string().contains(code),
        "expected {code}, received {error}"
    );
    let retained:i64=sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='academic_terms' AND column_name IN('start_date','planned_end_date','closed_on')").fetch_one(pool).await.unwrap();
    assert_eq!(retained, 3, "destructive cleanup must roll back atomically");
    let applied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE version=147")
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
    let pool = predecessor("context147_populated").await;
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
    apply_migrations_through(&pool, 147).await.unwrap();
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
    apply_migrations_through(&pool, 147).await.unwrap();
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
            "context147_ambiguous"
        } else {
            "context147_missing"
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
        assert_refusal(&pool, "ACADEMIC_CONTEXT_147_CALENDAR_YEAR_UNMAPPABLE").await;
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
    for kind in ["function", "check", "view"] {
        let pool = predecessor(&format!("context147_unknown_{kind}")).await;
        apply_migrations_through(&pool, 146).await.unwrap();
        match kind {
            "function" => {
                sqlx::raw_sql("CREATE FUNCTION surprise_boundary() RETURNS date LANGUAGE plpgsql AS $$ BEGIN RETURN (SELECT END_DATE FROM ACADEMIC_YEARS LIMIT 1); END $$").execute(&pool).await.unwrap();
            }
            "check" => {
                sqlx::raw_sql("ALTER TABLE academic_years ADD CONSTRAINT surprise_boundary CHECK(end_date-start_date<400)").execute(&pool).await.unwrap();
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
                "ACADEMIC_CONTEXT_147_UNKNOWN_DATE_DEPENDENCY"
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
            _ => {
                sqlx::raw_sql("DROP VIEW surprise_boundary")
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        apply_migrations_through(&pool, 147).await.unwrap();
    }
}

#[tokio::test]
async fn transformed_target_reassignment_refuses_even_when_foreign_keys_are_valid() {
    let pool = predecessor("context147_wrong_target").await;
    let id = scoped_target(&pool, "2025-05-02", None).await;
    apply_migrations_through(&pool, 146).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION surprise_mapping() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.academic_year_id:='10000000-0000-0000-0000-000000000026'; RETURN NEW; END $$; CREATE TRIGGER surprise_mapping BEFORE UPDATE ON calendar_event_targets FOR EACH ROW EXECUTE FUNCTION surprise_mapping()").execute(&pool).await.unwrap();
    assert_refusal(&pool, "ACADEMIC_CONTEXT_147_RECONCILIATION_FAILED").await;
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
    apply_migrations_through(&pool, 147).await.unwrap();
}
