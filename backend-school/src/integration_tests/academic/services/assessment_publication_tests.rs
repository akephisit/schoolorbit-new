use school_academic_assessment::assessment::services;
use school_test_db::cutover_test_support::{
    CutoverFixture, apply_migrations_through, record_passing_phase_a_reconciliation_marker,
    seed_academic_cutover_fixture,
};
use sqlx::PgPool;
use uuid::Uuid;

const PRIMARY: Uuid = Uuid::from_u128(0x50000000000000000000000000000002);
const OTHER: Uuid = Uuid::from_u128(0x50000000000000000000000000000003);

async fn predecessor(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 44).await.unwrap();
    record_passing_phase_a_reconciliation_marker(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 145).await.unwrap();
    pool
}

#[derive(Clone, Copy)]
enum FixtureSchema {
    Before146,
    Before155,
    Current,
}

async fn current_pool(name: &str) -> PgPool {
    let pool = predecessor(name).await;
    school_test_db::run_test_migrations(&pool).await;
    pool
}

/// Each scenario has its own explicit context and draft teaching groups.
/// Only the historical146 input retains retired academic boundary fields.
async fn offering(
    pool: &PgPool,
    number: i32,
    date_offset: i32,
    schema: FixtureSchema,
) -> (Uuid, Vec<Uuid>, chrono::NaiveDate, chrono::NaiveDate) {
    let mut tx = pool.begin().await.unwrap();
    let source: Uuid = sqlx::query_scalar(
        "SELECT learning_offering_id FROM course_assessment_plans ORDER BY id LIMIT 1",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let term = Uuid::now_v7();
    let id = Uuid::now_v7();
    let (start, end) = match schema {
        FixtureSchema::Before146 => {
            sqlx::query("UPDATE academic_years SET start_date=LEAST(start_date,current_date-400),end_date=GREATEST(end_date,current_date+400) WHERE id=(SELECT academic_year_id FROM learning_offerings WHERE id=$1)")
        .bind(source).execute(&mut *tx).await.unwrap();
            let (start, end): (chrono::NaiveDate, chrono::NaiveDate) = sqlx::query_as(
        "INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,start_date,planned_end_date,bell_schedule_id)
         SELECT $1,offering.academic_year_id,'Coordinator fixture',$2,'COORD-'||$2::text,'custom',false,false,'planning',current_date+$3,current_date+$3+20,source_term.bell_schedule_id
         FROM learning_offerings offering JOIN academic_terms source_term ON source_term.id=offering.academic_term_id
         WHERE offering.id=$4 RETURNING start_date,planned_end_date",
    ).bind(term).bind(1000+number).bind(date_offset).bind(source).fetch_one(&mut *tx).await.unwrap();
            (start, end)
        }
        FixtureSchema::Before155 | FixtureSchema::Current => {
            let start = chrono::Utc::now()
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive()
                + chrono::Duration::days(i64::from(date_offset));
            let term_sql = if matches!(schema, FixtureSchema::Before155) {
                "INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT $1,offering.academic_year_id,'Coordinator fixture',$2,'COORD-'||$2::text,'custom',false,false,'planning',source_term.bell_schedule_id FROM learning_offerings offering JOIN academic_terms source_term ON source_term.id=offering.academic_term_id WHERE offering.id=$3"
            } else {
                "INSERT INTO academic_terms(school_days,id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT 'MON,TUE,WED,THU,FRI',$1,offering.academic_year_id,'Coordinator fixture',$2,'COORD-'||$2::text,'custom',false,false,'planning',source_term.bell_schedule_id FROM learning_offerings offering JOIN academic_terms source_term ON source_term.id=offering.academic_term_id WHERE offering.id=$3"
            };
            sqlx::query(sqlx::AssertSqlSafe(term_sql))
                .bind(term)
                .bind(1000 + number)
                .bind(source)
                .execute(&mut *tx)
                .await
                .unwrap();
            (start, start + chrono::Duration::days(20))
        }
    };
    if matches!(schema, FixtureSchema::Current) {
        sqlx::query("INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,owning_organization_unit_id,weekly_period_target) SELECT $1,$2,academic_year_id,'course','COORD-'||$3::text,'Coordinator fixture',owning_organization_unit_id,1 FROM learning_offerings WHERE id=$4")
            .bind(id).bind(term).bind(number).bind(source).execute(&mut *tx).await.unwrap();
    } else {
        sqlx::query(
        "INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,status,published_at,owning_organization_unit_id,starts_on)
         SELECT $1,$2,academic_year_id,'course','COORD-'||$3::text,'Coordinator fixture','draft',NULL,owning_organization_unit_id,$4 FROM learning_offerings WHERE id=$5",
    ).bind(id).bind(term).bind(number).bind(start).bind(source).execute(&mut *tx).await.unwrap();
    }
    sqlx::query(
        "INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score)
         SELECT $1,$2,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score FROM course_offering_details WHERE learning_offering_id=$3",
    ).bind(id).bind(term).bind(source).execute(&mut *tx).await.unwrap();
    let mut groups = Vec::new();
    for index in 0..2 {
        let group = Uuid::now_v7();
        if matches!(schema, FixtureSchema::Current) {
            sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,registration_open) SELECT $1,id,academic_term_id,academic_year_id,$2,$2,false FROM learning_offerings WHERE id=$3").bind(group).bind(format!("COORD-{number}-{index}")).bind(id).execute(&mut *tx).await.unwrap();
        } else {
            sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status)
            SELECT $1,id,academic_term_id,academic_year_id,$2,$2,'draft','draft' FROM learning_offerings WHERE id=$3")
            .bind(group).bind(format!("COORD-{number}-{index}")).bind(id).execute(&mut *tx).await.unwrap();
        }
        groups.push(group);
    }
    if !matches!(schema, FixtureSchema::Current) {
        sqlx::query(
            "UPDATE learning_offerings SET status='published',published_at=now() WHERE id=$1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    (id, groups, start, end)
}

async fn teacher(
    pool: &PgPool,
    groups: &[Uuid],
    user: Uuid,
    start: chrono::NaiveDate,
    end: Option<chrono::NaiveDate>,
) {
    let historic:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='learning_group_teachers' AND column_name='starts_on')").fetch_one(pool).await.unwrap();
    for group in groups {
        if historic {
            sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,ends_on) SELECT uuidv7(),id,academic_term_id,academic_year_id,$2,'primary',$3,$4 FROM learning_groups WHERE id=$1").bind(group).bind(user).bind(start).bind(end).execute(pool).await.unwrap();
        } else {
            sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role) SELECT uuidv7(),id,academic_term_id,academic_year_id,$2,'primary' FROM learning_groups WHERE id=$1").bind(group).bind(user).execute(pool).await.unwrap();
        }
    }
}

async fn existing_plan(pool: &PgPool, offering: Uuid, coordinator: Option<Uuid>) -> Uuid {
    let plan = Uuid::now_v7();
    sqlx::query("INSERT INTO course_assessment_plans(id,academic_term_id,academic_year_id,learning_offering_id,subject_version_id,assessment_coordinator_id,row_version)
        SELECT $1,academic_term_id,academic_year_id,learning_offering_id,subject_version_id,$2,7 FROM course_offering_details WHERE learning_offering_id=$3")
        .bind(plan).bind(coordinator).bind(offering).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO course_assessment_phases(id,plan_id,phase_code,max_score,exam_arrangement,exam_duration_minutes,row_version,created_by,updated_by)
        SELECT uuidv7(),$1,code,25,CASE WHEN code='midterm' THEN 'in_timetable' ELSE 'none' END,CASE WHEN code='midterm' THEN 80 ELSE NULL END,9,$2,$2
        FROM unnest(ARRAY['before_midterm','midterm','after_midterm','final']) code")
        .bind(plan).bind(PRIMARY).execute(pool).await.unwrap();
    plan
}

async fn fill(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, offering_ids: &[Uuid]) {
    services::fill_missing_coordinators(
        &crate::adapters::academic_result_locks::RESULT_LOCKS,
        tx,
        PRIMARY,
        offering_ids,
    )
    .await
    .unwrap();
}

async fn coordinator_cases(
    pool: &PgPool,
    schema: FixtureSchema,
) -> Vec<(Uuid, &'static str, Option<Uuid>, Option<Uuid>)> {
    let inactive = Uuid::from_u128(0x50000000000000000000000000000004);
    let inserted = sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status)
        VALUES($1,'coordinator-inactive-fixture','fixture-not-a-login','Inactive','Coordinator','staff','inactive')")
        .bind(inactive)
        .execute(pool)
        .await
        .unwrap();
    assert_eq!(inserted.rows_affected(), 1);
    let mut cases = Vec::new();
    for (number, scenario, offset, expected) in [
        (1, "common", -10, Some(PRIMARY)),
        (2, "subset_primary", -10, Some(PRIMARY)),
        (3, "split_primary", -10, None),
        (4, "two_common", -10, None),
        (5, "missing_group_teacher", -10, None),
        (6, "inactive", -10, None),
        (7, "expired", -10, None),
        (8, "term_start_clamp", 30, Some(PRIMARY)),
        (9, "term_end_clamp", -40, Some(PRIMARY)),
        (10, "existing_null", -10, Some(PRIMARY)),
        (11, "existing_invalid_owner", -10, Some(PRIMARY)),
        (12, "closed_group_ignored", -10, Some(PRIMARY)),
    ] {
        let (id, groups, start, end) = offering(pool, number, offset, schema).await;
        match scenario {
            "split_primary" => {
                teacher(pool, &groups[..1], PRIMARY, start, None).await;
                teacher(pool, &groups[1..], OTHER, start, None).await;
            }
            "missing_group_teacher" => teacher(pool, &groups[..1], PRIMARY, start, None).await,
            "inactive" => teacher(pool, &groups, inactive, start, None).await,
            "expired" => teacher(pool, &groups, PRIMARY, start, Some(start)).await,
            "term_end_clamp" => teacher(pool, &groups, PRIMARY, start, Some(end)).await,
            _ => teacher(pool, &groups, PRIMARY, start, None).await,
        }
        if scenario == "subset_primary" {
            teacher(pool, &groups[..1], OTHER, start, None).await;
        }
        if scenario == "two_common" {
            teacher(pool, &groups, OTHER, start, None).await;
        }
        if scenario == "closed_group_ignored" {
            if matches!(schema, FixtureSchema::Current) {
                sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,registration_open) SELECT uuidv7(),id,academic_term_id,academic_year_id,'EMPTY-COORD','Current empty group',false FROM learning_offerings WHERE id=$1").bind(id).execute(pool).await.unwrap();
            } else {
                sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status)
                SELECT uuidv7(),id,academic_term_id,academic_year_id,'CLOSED-COORD','Ignored closed group','closed','closed' FROM learning_offerings WHERE id=$1")
                .bind(id).execute(pool).await.unwrap();
            }
        }
        let preserved_owner = if scenario == "existing_invalid_owner" {
            Some(OTHER)
        } else {
            None
        };
        if matches!(scenario, "existing_null" | "existing_invalid_owner") {
            existing_plan(pool, id, preserved_owner).await;
        }
        let expected = if matches!(schema, FixtureSchema::Current) {
            match scenario {
                "expired" => Some(PRIMARY),
                "closed_group_ignored" => None,
                _ => expected,
            }
        } else {
            expected
        };
        cases.push((id, scenario, expected, preserved_owner));
    }
    cases
}

async fn assessment_facts(pool: &PgPool) -> serde_json::Value {
    sqlx::query_scalar("SELECT jsonb_build_object('plans',(SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM course_assessment_plans p),'phases',(SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM course_assessment_phases p),'controls',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM academic_assessment_phase_controls c))")
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn assessment_publication_146_migration_preserves_initialized_owners_and_edit_controls() {
    let pool = predecessor("assessment_publication_migration_parity").await;
    // Explicit expired/inactive users never become candidates.
    let cases = coordinator_cases(&pool, FixtureSchema::Before146).await;
    let phases_before: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(phase) ORDER BY id) FROM course_assessment_phases phase",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let control_before: serde_json::Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(control) ORDER BY id) FROM academic_gradebook_phase_controls control").fetch_one(&pool).await.unwrap();
    // Exercise both legacy settings on all phases before the split.
    sqlx::query("UPDATE academic_assessment_phase_controls SET plan_editing_enabled=phase_code IN ('before_midterm','midterm')").execute(&pool).await.unwrap();
    let assessment_before: serde_json::Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(control) ORDER BY id) FROM academic_assessment_phase_controls control").fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 146).await.unwrap();
    apply_migrations_through(&pool, 146).await.unwrap();
    let initialized = assessment_facts(&pool).await;
    apply_migrations_through(&pool, 149).await.unwrap();
    assert_eq!(
        assessment_facts(&pool).await,
        initialized,
        "147..149 preserve146 initialized owners/phase facts and controls"
    );
    apply_migrations_through(&pool, 149).await.unwrap();
    for (id, scenario, expected, preserved) in &cases {
        let row:Option<(Option<Uuid>,i64)>=sqlx::query_as("SELECT assessment_coordinator_id,row_version FROM course_assessment_plans WHERE learning_offering_id=$1").bind(id).fetch_optional(&pool).await.unwrap();
        if expected.is_none() && preserved.is_none() {
            assert!(row.is_none(), "{scenario}");
            continue;
        }
        let (owner, revision) = row.unwrap();
        assert_eq!(owner, preserved.or(*expected), "migration:{scenario}");
        if matches!(*scenario, "existing_null" | "existing_invalid_owner") {
            assert_eq!(revision, if preserved.is_some() { 7 } else { 8 });
            assert_eq!(sqlx::query_scalar::<_,bool>("SELECT bool_and(row_version=9) FROM course_assessment_phases WHERE plan_id=(SELECT id FROM course_assessment_plans WHERE learning_offering_id=$1)").bind(id).fetch_one(&pool).await.unwrap(),true);
        } else {
            assert_eq!(revision, 1);
            let defaults:(i64,bool)=sqlx::query_as("SELECT count(*),bool_and(max_score=0 AND exam_arrangement='none' AND exam_duration_minutes IS NULL) FROM course_assessment_phases WHERE plan_id=(SELECT id FROM course_assessment_plans WHERE learning_offering_id=$1)").bind(id).fetch_one(&pool).await.unwrap();
            assert_eq!(defaults, (4, true));
        }
    }
    let old_phases_after: serde_json::Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(phase) ORDER BY id) FROM course_assessment_phases phase WHERE id IN (SELECT (value->>'id')::uuid FROM jsonb_array_elements($1::jsonb))").bind(&phases_before).fetch_one(&pool).await.unwrap();
    assert_eq!(phases_before, old_phases_after);
    let gradebook_after: serde_json::Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(control) ORDER BY id) FROM academic_gradebook_phase_controls control").fetch_one(&pool).await.unwrap();
    assert_eq!(control_before, gradebook_after);
    let assessment_after: serde_json::Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(control)-'exam_arrangement_editing_enabled' ORDER BY id) FROM academic_assessment_phase_controls control").fetch_one(&pool).await.unwrap();
    assert_eq!(assessment_before, assessment_after);
    let controls: Vec<(String,bool,bool)> = sqlx::query_as("SELECT phase_code,plan_editing_enabled,exam_arrangement_editing_enabled FROM academic_assessment_phase_controls").fetch_all(&pool).await.unwrap();
    assert!(
        controls
            .into_iter()
            .all(|(code, score, exam)| exam
                == (score && matches!(code.as_str(), "midterm" | "final")))
    );
}

#[tokio::test]
async fn assessment_publication_skips_closed_contexts_draft_offerings_and_locked_results() {
    for current in [false, true] {
        let pool = if current {
            current_pool("assessment_publication_current_scope").await
        } else {
            predecessor("assessment_publication_scope_exclusions").await
        };
        let schema = if current {
            FixtureSchema::Current
        } else {
            FixtureSchema::Before146
        };
        let (id, groups, start, _) = offering(&pool, 1, -10, schema).await;
        teacher(&pool, &groups, PRIMARY, start, None).await;

        let (year,term,subject): (Uuid,Uuid,Uuid) = sqlx::query_as("SELECT academic_year_id,academic_term_id,subject_id FROM course_offering_details WHERE learning_offering_id=$1").bind(id).fetch_one(&pool).await.unwrap();
        let scenarios = if current {
            vec![
                ("closed", "planning", "current"),
                ("archived", "planning", "current"),
                ("active", "closed", "current"),
                ("active", "cancelled", "current"),
            ]
        } else {
            vec![
                ("closed", "planning", "published"),
                ("archived", "planning", "published"),
                ("active", "closed", "published"),
                ("active", "cancelled", "published"),
                ("active", "planning", "draft"),
                ("active", "planning", "closed"),
            ]
        };
        for (year_status, term_status, offering_status) in scenarios {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
                .bind(year)
                .bind(year_status)
                .execute(&mut *tx)
                .await
                .unwrap();
            if current {
                sqlx::query("UPDATE academic_terms SET status=$2 WHERE id=$1")
                    .bind(term)
                    .bind(term_status)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            } else {
                sqlx::query("UPDATE academic_terms SET status=$2,closed_on=CASE WHEN $2='closed' THEN planned_end_date ELSE NULL END WHERE id=$1").bind(term).bind(term_status).execute(&mut *tx).await.unwrap();
            }
            if !current {
                sqlx::query("UPDATE learning_offerings SET status=$2 WHERE id=$1")
                    .bind(id)
                    .bind(offering_status)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            }
            if current {
                fill(&mut tx, &[id]).await;
            }
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM course_assessment_plans WHERE learning_offering_id=$1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            assert_eq!(count, 0, "{year_status}/{term_status}/{offering_status}");
            // Migration eligibility is independent SQL; prove each excluded source also stays
            // untouched during deployment, then roll back its DDL with this scenario.
            if !current {
                sqlx::raw_sql(include_str!(
                    "../../../../migrations/146_assessment_coordinators_and_edit_controls.sql"
                ))
                .execute(&mut *tx)
                .await
                .unwrap();
            }
            let migrated_count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM course_assessment_plans WHERE learning_offering_id=$1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            assert_eq!(
                migrated_count, 0,
                "migration: {year_status}/{term_status}/{offering_status}"
            );
            tx.rollback().await.unwrap();
        }
        let policy: Uuid = sqlx::query_scalar("INSERT INTO academic_grading_policy_versions(version_no,name,lifecycle) VALUES(999,'Coordinator lock fixture','draft') RETURNING id").fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_course_result_locks(subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by)
        VALUES($1,$2,$3,$4,'{}',repeat('a',64),repeat('b',64),'{}',$5)")
        .bind(subject).bind(term).bind(year).bind(policy).bind(PRIMARY).execute(&pool).await.unwrap();
        if current {
            let mut tx = pool.begin().await.unwrap();
            fill(&mut tx, &[id]).await;
            tx.commit().await.unwrap();
        } else {
            apply_migrations_through(&pool, 146).await.unwrap();
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM course_assessment_plans WHERE learning_offering_id=$1"
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }
}

#[tokio::test]
async fn assessment_publication_accepts_large_mixed_targets_and_preserves_owned_plans() {
    let pool = current_pool("assessment_publication_large_mixed_targets").await;
    let (missing, groups, start, _) = offering(&pool, 1, -10, FixtureSchema::Current).await;
    teacher(&pool, &groups, PRIMARY, start, None).await;

    let (owned, owned_groups, owned_start, _) =
        offering(&pool, 2, -10, FixtureSchema::Current).await;
    teacher(&pool, &owned_groups, PRIMARY, owned_start, None).await;

    existing_plan(&pool, owned, Some(OTHER)).await;

    // Delivery includes retained activities and courses with accepted owners in its targets.
    // Persist every target so deduplication or nonexistent IDs cannot hide a smaller input.
    let activity_ids = (0..499).map(|_| Uuid::now_v7()).collect::<Vec<_>>();
    let mut tx = pool.begin().await.unwrap();
    let activity_source: Uuid = sqlx::query_scalar(
        "SELECT id FROM activity_versions WHERE status='published' ORDER BY id LIMIT 1",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let inserted = sqlx::query(
        "INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,owning_organization_unit_id,weekly_period_target)
         SELECT batch.id,target.academic_term_id,target.academic_year_id,'activity','COORD-ACT-'||batch.number::text,'Coordinator activity fixture',source.owning_organization_unit_id,1
         FROM unnest($1::uuid[]) WITH ORDINALITY batch(id,number)
         JOIN learning_offerings target ON target.id=$2
         JOIN activity_versions version ON version.id=$3 JOIN activities source ON source.id=version.activity_id",
    )
    .bind(&activity_ids)
    .bind(missing)
    .bind(activity_source)
    .execute(&mut *tx)
    .await
    .unwrap();
    assert_eq!(inserted.rows_affected(), 499);
    sqlx::query(
        r#"INSERT INTO activity_offering_details(learning_offering_id,academic_term_id,academic_year_id,activity_version_id,activity_id,registration_type,scheduling_mode,hours,capacity,attendance_requirement,pass_criteria)
         SELECT offering.id,offering.academic_term_id,offering.academic_year_id,source.id,source.activity_id,'assigned',source.scheduling_mode,source.hours_per_week,NULL,'{"minimumPercent":null,"requiredSessions":null}'::jsonb,'{"requireAttendance":false,"requireTeacherConfirmation":true,"outcomes":["pass","fail"]}'::jsonb
         FROM learning_offerings offering JOIN activity_versions source ON source.id=$2
         WHERE offering.id=ANY($1)"#,
    )
    .bind(&activity_ids)
    .bind(activity_source)
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    let mut target_ids = activity_ids.clone();
    target_ids.extend([owned, missing]);
    target_ids.reverse();
    assert_eq!(
        target_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        501
    );
    let persisted_targets: i64 =
        sqlx::query_scalar("SELECT count(*) FROM learning_offerings WHERE id=ANY($1)")
            .bind(&target_ids)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(persisted_targets, 501);
    let snapshot_sql = "SELECT jsonb_agg(jsonb_build_object('plan',to_jsonb(plan),'phases',(SELECT jsonb_agg(to_jsonb(phase) ORDER BY phase.id) FROM course_assessment_phases phase WHERE phase.plan_id=plan.id)) ORDER BY plan.id) FROM course_assessment_plans plan WHERE plan.learning_offering_id=ANY($1)";
    let owned_before: serde_json::Value = sqlx::query_scalar(snapshot_sql)
        .bind(vec![owned])
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    fill(&mut tx, &target_ids).await;
    tx.commit().await.unwrap();

    let assigned = services::get_plan_detail(&pool, missing).await.unwrap();
    assert_eq!(assigned.assessment_coordinator_id, Some(PRIMARY));
    assert_eq!(assigned.phases.len(), 4);
    assert!(assigned.phases.iter().all(|phase| phase.max_score == "0"
        && phase.exam_arrangement
            == school_academic_assessment::assessment::models::AssessmentExamArrangement::None
        && phase.exam_duration_minutes.is_none()));
    let owned_after: serde_json::Value = sqlx::query_scalar(snapshot_sql)
        .bind(vec![owned])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owned_before, owned_after);
    let activity_plans: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM course_assessment_plans WHERE learning_offering_id=ANY($1)",
    )
    .bind(&activity_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(activity_plans, 0);
    let first_fill: serde_json::Value = sqlx::query_scalar(snapshot_sql)
        .bind(&target_ids)
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    fill(&mut tx, &target_ids).await;
    tx.commit().await.unwrap();
    let repeated_fill: serde_json::Value = sqlx::query_scalar(snapshot_sql)
        .bind(&target_ids)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(first_fill, repeated_fill);
}

#[tokio::test]
async fn assessment_suggestions_use_current_normalized_teachers() {
    let pool = current_pool("assessment_publication_actual_day").await;
    let cases = coordinator_cases(&pool, FixtureSchema::Current).await;
    let before: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM course_assessment_phases p",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let ids = cases.iter().map(|case| case.0).collect::<Vec<_>>();
    for (id, scenario, expected, _) in &cases {
        assert_eq!(
            services::get_plan_detail(&pool, *id)
                .await
                .unwrap()
                .suggested_coordinator_id,
            *expected,
            "published allocation: {scenario}"
        );
    }
    let mut tx = pool.begin().await.unwrap();
    fill(&mut tx, &ids).await;
    tx.commit().await.unwrap();
    for (id, scenario, expected, preserved) in &cases {
        let detail = services::get_plan_detail(&pool, *id).await.unwrap();
        assert_eq!(
            detail.assessment_coordinator_id,
            preserved.or(*expected),
            "current publication: {scenario}"
        );
    }
    let after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM course_assessment_phases p WHERE id IN(SELECT (value->>'id')::uuid FROM jsonb_array_elements($1::jsonb))").bind(&before).fetch_one(&pool).await.unwrap();
    assert_eq!(
        after, before,
        "accepted phase scores, exam settings and revisions remain unchanged"
    );
}

#[tokio::test]
async fn assessment_publication_146_and_context_cutover_apply_all_pending_together() {
    let pool = predecessor("assessment_146_context_all_pending").await;
    let (id, groups, start, _) = offering(&pool, 1, -10, FixtureSchema::Before146).await;
    teacher(&pool, &groups, PRIMARY, start, None).await;
    sqlx::query("UPDATE academic_assessment_phase_controls SET plan_editing_enabled=phase_code IN('before_midterm','midterm')").execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 149).await.unwrap();
    let owner:Option<Uuid>=sqlx::query_scalar("SELECT assessment_coordinator_id FROM course_assessment_plans WHERE learning_offering_id=$1").bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(owner, Some(PRIMARY));
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM course_assessment_phases WHERE plan_id=(SELECT id FROM course_assessment_plans WHERE learning_offering_id=$1)").bind(id).fetch_one(&pool).await.unwrap(),4);
    let consistent:bool=sqlx::query_scalar("SELECT bool_and(exam_arrangement_editing_enabled=(phase_code IN('midterm','final') AND plan_editing_enabled)) FROM academic_assessment_phase_controls").fetch_one(&pool).await.unwrap();
    assert!(consistent);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND ((table_name='academic_years' AND column_name IN('start_date','end_date')) OR(table_name='academic_terms' AND column_name IN('start_date','planned_end_date','closed_on')))").fetch_one(&pool).await.unwrap(),0);
    let before = assessment_facts(&pool).await;
    apply_migrations_through(&pool, 149).await.unwrap();
    assert_eq!(assessment_facts(&pool).await, before);
}

async fn assert_coordinator_membership(
    pool: &PgPool,
    offering: Uuid,
    mut expected_teachers: Vec<Uuid>,
    expected_ready: bool,
    expected_listed: bool,
) -> school_academic_assessment::assessment::models::AssessmentPlanDetail {
    let detail = services::get_plan_detail(pool, offering).await.unwrap();
    let mut teachers = detail
        .coordinator_candidates
        .iter()
        .map(|candidate| candidate.teacher_id)
        .collect::<Vec<_>>();
    teachers.sort_unstable();
    expected_teachers.sort_unstable();
    assert_eq!(teachers, expected_teachers);
    assert_eq!(detail.readiness.ready, expected_ready);
    let summaries = services::list_assessment_plans(
        pool,
        &school_academic_assessment::assessment::models::AssessmentPlanListQuery {
            academic_term_id: detail.academic_term_id,
            subject_id: None,
            instructor_id: None,
            ready: None,
            exam_arrangement: None,
        },
        &school_authorization::AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
        PRIMARY,
    )
    .await
    .unwrap();
    let summary = summaries
        .iter()
        .find(|summary| summary.offering_id == offering);
    assert_eq!(summary.is_some(), expected_listed);
    if let Some(summary) = summary {
        assert_eq!(
            summary.assessment_coordinator_id,
            detail.assessment_coordinator_id
        );
        assert_eq!(summary.readiness.ready, detail.readiness.ready);
        assert_eq!(summary.readiness.findings, detail.readiness.findings);
    }
    let exam_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_exam_eligible_sources WHERE learning_offering_id=$1",
    )
    .bind(offering)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(exam_count > 0, expected_ready);
    detail
}

fn saved_payload(
    detail: &school_academic_assessment::assessment::models::AssessmentPlanDetail,
) -> school_academic_assessment::assessment::models::SaveAssessmentPlanRequest {
    school_academic_assessment::assessment::models::SaveAssessmentPlanRequest {
        row_version: detail.row_version,
        assessment_coordinator_id: detail.assessment_coordinator_id,
        phases: detail
            .phases
            .iter()
            .map(|phase| {
                school_academic_assessment::assessment::models::SaveAssessmentPhaseRequest {
                    id: phase.id,
                    phase_code: phase.phase_code,
                    max_score: phase.max_score.clone(),
                    exam_arrangement: phase.exam_arrangement,
                    exam_duration_minutes: phase.exam_duration_minutes,
                }
            })
            .collect(),
    }
}

#[tokio::test]
async fn assessment_current_teacher_replacement_keeps_read_save_and_exam_eligibility_in_agreement()
{
    use school_academic_delivery::{models as dm, services::groups as current_groups};
    let pool = current_pool("assessment_current_parity").await;
    let (id, groups, start, _) = offering(&pool, 1, -10, FixtureSchema::Current).await;
    teacher(&pool, &groups, PRIMARY, start, None).await;
    sqlx::query("INSERT INTO learning_group_homerooms(id,learning_group_id,academic_term_id,academic_year_id,homeroom_id,coverage_source) SELECT uuidv7(),g.id,g.academic_term_id,g.academic_year_id,(SELECT id FROM homerooms WHERE academic_year_id=g.academic_year_id AND is_active ORDER BY id LIMIT 1),'manual' FROM learning_groups g WHERE g.id=ANY($1)").bind(&groups).execute(&pool).await.unwrap();
    let plan = existing_plan(&pool, id, Some(PRIMARY)).await;
    let original = assert_coordinator_membership(&pool, id, vec![PRIMARY], true, true).await;
    crate::integration_tests::academic::assessment_adapter::save_plan(
        &pool,
        id,
        PRIMARY,
        true,
        saved_payload(&original),
    )
    .await
    .unwrap();
    let phases:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM course_assessment_phases p WHERE plan_id=$1").bind(plan).fetch_one(&pool).await.unwrap();
    for group in &groups {
        let row = current_groups::get(&pool, *group).await.unwrap();
        current_groups::replace_teachers(
            &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
            &pool,
            PRIMARY,
            *group,
            dm::ReplaceLearningGroupTeachersRequest {
                row_version: row.row_version,
                teachers: vec![dm::TeacherAssignmentInput {
                    teacher_id: OTHER,
                    role: dm::LearningTeacherRole::Primary,
                }],
            },
        )
        .await
        .unwrap();
    }
    let replaced = assert_coordinator_membership(&pool, id, vec![OTHER], false, true).await;
    assert_eq!(replaced.assessment_coordinator_id, Some(PRIMARY));
    assert!(matches!(
        crate::integration_tests::academic::assessment_adapter::save_plan(
            &pool,
            id,
            PRIMARY,
            true,
            saved_payload(&replaced)
        )
        .await,
        Err(school_errors::AppError::ValidationError(_))
    ));
    assert_eq!(phases,sqlx::query_scalar::<_,serde_json::Value>("SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM course_assessment_phases p WHERE plan_id=$1").bind(plan).fetch_one(&pool).await.unwrap());
    let mut replacement = saved_payload(&replaced);
    replacement.assessment_coordinator_id = Some(OTHER);
    crate::integration_tests::academic::assessment_adapter::save_plan(
        &pool,
        id,
        PRIMARY,
        true,
        replacement,
    )
    .await
    .unwrap();
    assert_coordinator_membership(&pool, id, vec![OTHER], true, true).await;
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(OTHER)
        .execute(&pool)
        .await
        .unwrap();
    assert_coordinator_membership(&pool, id, vec![], false, true).await;
}

#[tokio::test]
async fn assessment_migration_150_preserves_populated_data_and_exam_source_identity() {
    let pool = predecessor("assessment_migration_150_preservation").await;
    apply_migrations_through(&pool, 149).await.unwrap();
    let (ready_offering, groups, start, _) =
        offering(&pool, 1, -10, FixtureSchema::Before155).await;
    teacher(&pool, &groups, PRIMARY, start, None).await;
    // This fixture is exactly schema149; publication numbers arrive in151.
    sqlx::query("INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,effective_from,status,snapshot,published_by,published_at,publication_idempotency_key,publication_request_hash) SELECT uuidv7(),academic_term_id,academic_year_id,$2,'published',academic_capture_delivery_snapshot(academic_term_id,$2,jsonb_build_array(jsonb_build_object('id',id,'weekly_period_target',1))),$3,now(),uuidv7(),repeat('a',64) FROM learning_offerings WHERE id=$1")
        .bind(ready_offering).bind(start).bind(PRIMARY).execute(&pool).await.unwrap();
    existing_plan(&pool, ready_offering, Some(PRIMARY)).await;
    sqlx::query("INSERT INTO learning_group_homerooms(id,learning_group_id,academic_term_id,academic_year_id,homeroom_id,coverage_source) SELECT uuidv7(),learning_group.id,learning_group.academic_term_id,learning_group.academic_year_id,(SELECT id FROM homerooms WHERE academic_year_id=learning_group.academic_year_id AND is_active ORDER BY id LIMIT 1),'manual' FROM learning_groups learning_group WHERE learning_group.id=ANY($1)")
        .bind(&groups).execute(&pool).await.unwrap();
    let facts_sql = "SELECT jsonb_build_object('plans',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM course_assessment_plans row),'phases',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM course_assessment_phases row),'episodes',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM learning_group_teachers row),'versions',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM academic_delivery_versions row),'exam_items',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM academic_exam_schedule_items row),'controls',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM academic_assessment_phase_controls row))";
    let before: serde_json::Value = sqlx::query_scalar(facts_sql)
        .fetch_one(&pool)
        .await
        .unwrap();
    for key in [
        "plans",
        "phases",
        "episodes",
        "versions",
        "exam_items",
        "controls",
    ] {
        assert!(
            !before[key].as_array().unwrap().is_empty(),
            "preservation has populated {key}"
        );
    }
    let view_before: String = sqlx::query_scalar(
        "SELECT pg_get_viewdef('academic_exam_eligible_sources'::regclass,false)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let sources_before: Vec<(Uuid, Uuid, Uuid, Uuid)> = sqlx::query_as("SELECT source_id,assessment_phase_id,learning_group_id,homeroom_id FROM academic_exam_eligible_sources ORDER BY source_id").fetch_all(&pool).await.unwrap();
    assert!(!sources_before.is_empty());
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../migrations/150_assessment_coordinator_delivery_versions.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    assert!(sqlx::query("SELECT 1/0").execute(&mut *tx).await.is_err());
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT to_regclass('academic_delivery_latest_published_teacher_allocations')::text"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        None
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT pg_get_viewdef('academic_exam_eligible_sources'::regclass,false)"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        view_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(facts_sql)
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
    apply_migrations_through(&pool, 150).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(facts_sql)
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
    let columns: Vec<String> = sqlx::query_scalar("SELECT column_name::text FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='academic_exam_eligible_sources' ORDER BY ordinal_position").fetch_all(&pool).await.unwrap();
    assert_eq!(
        columns,
        vec![
            "source_id",
            "academic_term_id",
            "academic_year_id",
            "assessment_phase_id",
            "course_assessment_plan_id",
            "learning_offering_id",
            "learning_group_id",
            "homeroom_id",
            "subject_id",
            "grade_level_id",
            "exam_kind",
            "duration_minutes"
        ]
    );
    let sources_after: Vec<(Uuid, Uuid, Uuid, Uuid)> = sqlx::query_as("SELECT source_id,assessment_phase_id,learning_group_id,homeroom_id FROM academic_exam_eligible_sources ORDER BY source_id").fetch_all(&pool).await.unwrap();
    assert_eq!(sources_after, sources_before);
    apply_migrations_through(&pool, 150).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(facts_sql)
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn assessment_current_projection_preserves_stable_assignment_identity_and_exact_context() {
    let pool = current_pool("assessment_current_identity").await;
    let (id, groups, start, _) = offering(&pool, 1, -10, FixtureSchema::Current).await;
    teacher(&pool, &groups, PRIMARY, start, None).await;
    let expected: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM learning_group_teachers WHERE learning_group_id=ANY($1) ORDER BY id",
    )
    .bind(&groups)
    .fetch_all(&pool)
    .await
    .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let allocations =
        school_academic_delivery::services::current::teacher_allocations_on_connection(
            &mut conn,
            &[id],
        )
        .await
        .unwrap();
    let mut actual: Vec<_> = allocations.iter().map(|a| a.assignment_id).collect();
    actual.sort();
    assert_eq!(actual, expected);
    assert!(allocations.iter().all(|a| a.learning_offering_id == id
        && groups.contains(&a.learning_group_id)
        && a.teacher_id == PRIMARY));
    assert!(
        school_academic_delivery::services::current::teacher_allocations_on_connection(
            &mut conn,
            &[]
        )
        .await
        .unwrap()
        .is_empty()
    );
    drop(conn);
    let duplicate=sqlx::query("INSERT INTO learning_group_teachers SELECT (jsonb_populate_record(NULL::learning_group_teachers,to_jsonb(t)||jsonb_build_object('id',uuidv7()))).* FROM learning_group_teachers t WHERE id=$1").bind(expected[0]).execute(&pool).await.unwrap_err();
    assert_eq!(
        duplicate.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(PRIMARY)
        .execute(&pool)
        .await
        .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        school_academic_delivery::services::current::teacher_allocations_on_connection(
            &mut conn,
            &[id]
        )
        .await
        .unwrap()
        .is_empty()
    );
    drop(conn);
    assert_eq!(
        expected,
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM learning_group_teachers WHERE learning_group_id=ANY($1) ORDER BY id"
        )
        .bind(&groups)
        .fetch_all(&pool)
        .await
        .unwrap()
    );
}
