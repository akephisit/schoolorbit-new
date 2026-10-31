use school_academic_assessment::assessment::services;
use school_test_db::cutover_test_support::{
    apply_migrations_through, record_passing_phase_a_reconciliation_marker,
    seed_academic_cutover_fixture, CutoverFixture,
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
        FixtureSchema::Current => {
            let start = chrono::Utc::now()
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive()
                + chrono::Duration::days(i64::from(date_offset));
            sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT $1,offering.academic_year_id,'Coordinator fixture',$2,'COORD-'||$2::text,'custom',false,false,'planning',source_term.bell_schedule_id FROM learning_offerings offering JOIN academic_terms source_term ON source_term.id=offering.academic_term_id WHERE offering.id=$3")
                .bind(term).bind(1000+number).bind(source).execute(&mut *tx).await.unwrap();
            (start, start + chrono::Duration::days(20))
        }
    };
    sqlx::query(
        "INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,status,published_at,owning_organization_unit_id,starts_on)
         SELECT $1,$2,academic_year_id,'course','COORD-'||$3::text,'Coordinator fixture','draft',NULL,owning_organization_unit_id,$4 FROM learning_offerings WHERE id=$5",
    ).bind(id).bind(term).bind(number).bind(start).bind(source).execute(&mut *tx).await.unwrap();
    sqlx::query(
        "INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score)
         SELECT $1,$2,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score FROM course_offering_details WHERE learning_offering_id=$3",
    ).bind(id).bind(term).bind(source).execute(&mut *tx).await.unwrap();
    let mut groups = Vec::new();
    for index in 0..2 {
        let group = Uuid::now_v7();
        sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status)
            SELECT $1,id,academic_term_id,academic_year_id,$2,$2,'draft','draft' FROM learning_offerings WHERE id=$3")
            .bind(group).bind(format!("COORD-{number}-{index}")).bind(id).execute(&mut *tx).await.unwrap();
        groups.push(group);
    }
    sqlx::query("UPDATE learning_offerings SET status='published',published_at=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
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
    for group in groups {
        sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,ends_on)
            SELECT uuidv7(),id,academic_term_id,academic_year_id,$2,'primary',$3,$4 FROM learning_groups WHERE id=$1")
            .bind(group).bind(user).bind(start).bind(end).execute(pool).await.unwrap();
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
            sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status)
                SELECT uuidv7(),id,academic_term_id,academic_year_id,'CLOSED-COORD','Ignored closed group','closed','closed' FROM learning_offerings WHERE id=$1")
                .bind(id).execute(pool).await.unwrap();
        }
        let preserved_owner = if scenario == "existing_invalid_owner" {
            Some(OTHER)
        } else {
            None
        };
        if matches!(scenario, "existing_null" | "existing_invalid_owner") {
            existing_plan(pool, id, preserved_owner).await;
        }
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
    school_test_db::run_test_migrations(&pool).await;
    assert_eq!(assessment_facts(&pool).await,initialized,"context147/148 preserves the applied146 owner initialization, newly created plans/phases and independent control flags exactly");
    school_test_db::run_test_migrations(&pool).await;
    assert_eq!(assessment_facts(&pool).await, initialized);
    for (id, scenario, expected, preserved) in &cases {
        let detail = services::get_plan_detail(&pool, *id).await.unwrap();
        assert_eq!(
            detail.assessment_coordinator_id,
            preserved.or(*expected),
            "migration: {scenario}"
        );
        if matches!(*scenario, "existing_null" | "existing_invalid_owner") {
            assert_eq!(
                detail.row_version,
                Some(if preserved.is_some() { 7 } else { 8 })
            );
            assert!(detail
                .phases
                .iter()
                .all(|phase| phase.row_version == Some(9)));
        } else if expected.is_some() {
            assert_eq!(detail.row_version, Some(1));
            assert_eq!(detail.phases.len(), 4);
            assert!(detail.phases.iter().all(|phase|phase.max_score=="0" && phase.exam_arrangement==school_academic_assessment::assessment::models::AssessmentExamArrangement::None && phase.exam_duration_minutes.is_none()));
        } else {
            assert!(detail.id.is_none());
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
        for (year_status, term_status, offering_status) in [
            ("closed", "planning", "published"),
            ("archived", "planning", "published"),
            ("active", "closed", "published"),
            ("active", "cancelled", "published"),
            ("active", "planning", "draft"),
            ("active", "planning", "closed"),
        ] {
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
            sqlx::query("UPDATE learning_offerings SET status=$2 WHERE id=$1")
                .bind(id)
                .bind(offering_status)
                .execute(&mut *tx)
                .await
                .unwrap();
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
            school_test_db::run_test_migrations(&pool).await;
        }
        assert!(services::get_plan_detail(&pool, id)
            .await
            .unwrap()
            .id
            .is_none());
        school_test_db::run_test_migrations(&pool).await;
        assert!(services::get_plan_detail(&pool, id)
            .await
            .unwrap()
            .id
            .is_none());
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
        "SELECT learning_offering_id FROM activity_offering_details ORDER BY learning_offering_id LIMIT 1",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let inserted = sqlx::query(
        "INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,status,owning_organization_unit_id,starts_on)
         SELECT batch.id,target.academic_term_id,target.academic_year_id,'activity','COORD-ACT-'||batch.number::text,'Coordinator activity fixture','draft',source.owning_organization_unit_id,target.starts_on
         FROM unnest($1::uuid[]) WITH ORDINALITY batch(id,number)
         JOIN learning_offerings target ON target.id=$2
         JOIN learning_offerings source ON source.id=$3",
    )
    .bind(&activity_ids)
    .bind(missing)
    .bind(activity_source)
    .execute(&mut *tx)
    .await
    .unwrap();
    assert_eq!(inserted.rows_affected(), 499);
    sqlx::query(
        "INSERT INTO activity_offering_details(learning_offering_id,academic_term_id,academic_year_id,activity_version_id,activity_id,registration_type,scheduling_mode,hours,capacity,attendance_requirement,pass_criteria)
         SELECT offering.id,offering.academic_term_id,offering.academic_year_id,source.activity_version_id,source.activity_id,source.registration_type,source.scheduling_mode,source.hours,source.capacity,source.attendance_requirement,source.pass_criteria
         FROM learning_offerings offering JOIN activity_offering_details source ON source.learning_offering_id=$2
         WHERE offering.id=ANY($1)",
    )
    .bind(&activity_ids)
    .bind(activity_source)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE learning_offerings SET status='published',published_at=now() WHERE id=ANY($1)",
    )
    .bind(&activity_ids)
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
async fn assessment_publication_current_suggestions_use_actual_operation_day() {
    let pool = current_pool("assessment_publication_actual_day").await;
    let cases = coordinator_cases(&pool, FixtureSchema::Current).await;
    let before: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM course_assessment_phases p",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let ids = cases.iter().map(|case| case.0).collect::<Vec<_>>();
    for (id, scenario, legacy_expected, _) in &cases {
        let expected = if matches!(*scenario, "term_start_clamp" | "term_end_clamp") {
            None
        } else {
            *legacy_expected
        };
        assert_eq!(
            services::get_plan_detail(&pool, *id)
                .await
                .unwrap()
                .suggested_coordinator_id,
            expected,
            "current operation day: {scenario}"
        );
    }
    let mut tx = pool.begin().await.unwrap();
    fill(&mut tx, &ids).await;
    tx.commit().await.unwrap();
    for (id, scenario, legacy_expected, preserved) in &cases {
        let expected = if matches!(*scenario, "term_start_clamp" | "term_end_clamp") {
            None
        } else {
            *legacy_expected
        };
        let detail = services::get_plan_detail(&pool, *id).await.unwrap();
        assert_eq!(
            detail.assessment_coordinator_id,
            preserved.or(expected),
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
    school_test_db::run_test_migrations(&pool).await;
    let detail = services::get_plan_detail(&pool, id).await.unwrap();
    assert_eq!(detail.assessment_coordinator_id, Some(PRIMARY));
    assert_eq!(detail.phases.len(), 4);
    let consistent:bool=sqlx::query_scalar("SELECT bool_and(exam_arrangement_editing_enabled=(phase_code IN('midterm','final') AND plan_editing_enabled)) FROM academic_assessment_phase_controls").fetch_one(&pool).await.unwrap();
    assert!(consistent);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND ((table_name='academic_years' AND column_name IN('start_date','end_date')) OR(table_name='academic_terms' AND column_name IN('start_date','planned_end_date','closed_on')))").fetch_one(&pool).await.unwrap(),0);
    let before = assessment_facts(&pool).await;
    school_test_db::run_test_migrations(&pool).await;
    assert_eq!(assessment_facts(&pool).await, before);
}
