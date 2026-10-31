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

/// Each scenario has its own term and draft teaching groups, preserving all original episodes.
async fn offering(
    pool: &PgPool,
    number: i32,
    date_offset: i32,
) -> (Uuid, Vec<Uuid>, chrono::NaiveDate, chrono::NaiveDate) {
    let source: Uuid = sqlx::query_scalar(
        "SELECT learning_offering_id FROM course_assessment_plans ORDER BY id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let term = Uuid::now_v7();
    let id = Uuid::now_v7();
    sqlx::query("UPDATE academic_years SET start_date=LEAST(start_date,current_date-400),end_date=GREATEST(end_date,current_date+400) WHERE id=(SELECT academic_year_id FROM learning_offerings WHERE id=$1)")
        .bind(source).execute(pool).await.unwrap();
    let (start, end): (chrono::NaiveDate, chrono::NaiveDate) = sqlx::query_as(
        "INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,start_date,planned_end_date)
         SELECT $1,academic_year_id,'Coordinator fixture',$2,'COORD-'||$2::text,'custom',false,false,'planning',current_date+$3,current_date+$3+20
         FROM learning_offerings WHERE id=$4 RETURNING start_date,planned_end_date",
    ).bind(term).bind(1000+number).bind(date_offset).bind(source).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,status,published_at,owning_organization_unit_id,starts_on)
         SELECT $1,$2,academic_year_id,'course','COORD-'||$3::text,'Coordinator fixture','published',now(),owning_organization_unit_id,$4 FROM learning_offerings WHERE id=$5",
    ).bind(id).bind(term).bind(number).bind(start).bind(source).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,standard_periods_per_week,assessment_total_score)
         SELECT $1,$2,academic_year_id,subject_version_id,subject_id,credit,hours,standard_periods_per_week,assessment_total_score FROM course_offering_details WHERE learning_offering_id=$3",
    ).bind(id).bind(term).bind(source).execute(pool).await.unwrap();
    let mut groups = Vec::new();
    for index in 0..2 {
        let group = Uuid::now_v7();
        sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status)
            SELECT $1,id,academic_term_id,academic_year_id,$2,$2,'draft','draft' FROM learning_offerings WHERE id=$3")
            .bind(group).bind(format!("COORD-{number}-{index}")).bind(id).execute(pool).await.unwrap();
        groups.push(group);
    }
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

#[tokio::test]
async fn assessment_publication_and_migration_share_suggestion_parity_and_preserve_accepted_rows() {
    let pool = predecessor("assessment_publication_migration_parity").await;
    // Explicit expired/inactive users never become candidates.
    let inactive = Uuid::from_u128(0x50000000000000000000000000000004);
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(inactive)
        .execute(&pool)
        .await
        .unwrap();
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
        let (id, groups, start, end) = offering(&pool, number, offset).await;
        match scenario {
            "split_primary" => {
                teacher(&pool, &groups[..1], PRIMARY, start, None).await;
                teacher(&pool, &groups[1..], OTHER, start, None).await;
            }
            "missing_group_teacher" => teacher(&pool, &groups[..1], PRIMARY, start, None).await,
            "inactive" => teacher(&pool, &groups, inactive, start, None).await,
            "expired" => teacher(&pool, &groups, PRIMARY, start, Some(start)).await,
            "term_end_clamp" => teacher(&pool, &groups, PRIMARY, start, Some(end)).await,
            _ => teacher(&pool, &groups, PRIMARY, start, None).await,
        }
        if scenario == "subset_primary" {
            teacher(&pool, &groups[..1], OTHER, start, None).await;
        }
        if scenario == "two_common" {
            teacher(&pool, &groups, OTHER, start, None).await;
        }
        if scenario == "closed_group_ignored" {
            sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status)
                SELECT uuidv7(),id,academic_term_id,academic_year_id,'CLOSED-COORD','Ignored closed group','closed','closed' FROM learning_offerings WHERE id=$1")
                .bind(id).execute(&pool).await.unwrap();
        }
        let preserved_owner = if scenario == "existing_invalid_owner" {
            Some(OTHER)
        } else {
            None
        };
        if matches!(scenario, "existing_null" | "existing_invalid_owner") {
            existing_plan(&pool, id, preserved_owner).await;
        }
        let detail = services::get_plan_detail(&pool, id).await.unwrap();
        assert_eq!(
            detail.suggested_coordinator_id, expected,
            "manual suggestion: {scenario}"
        );
        cases.push((id, scenario, expected, preserved_owner));
    }
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
    let ids = cases.iter().map(|case| case.0).collect::<Vec<_>>();
    let mut tx = pool.begin().await.unwrap();
    fill(&mut tx, &ids).await;
    for (id, scenario, expected, preserved) in &cases {
        let actual: Option<Uuid> = sqlx::query_scalar("SELECT assessment_coordinator_id FROM course_assessment_plans WHERE learning_offering_id=$1").bind(id).fetch_optional(&mut *tx).await.unwrap().flatten();
        assert_eq!(actual, preserved.or(*expected), "runtime: {scenario}");
    }
    tx.rollback().await.unwrap();
    apply_migrations_through(&pool, 146).await.unwrap();
    apply_migrations_through(&pool, 146).await.unwrap();
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
    let pool = predecessor("assessment_publication_scope_exclusions").await;
    let (id,groups,start,_) = offering(&pool,1,-10).await;
    teacher(&pool,&groups,PRIMARY,start,None).await;
    let (year,term,subject): (Uuid,Uuid,Uuid) = sqlx::query_as("SELECT academic_year_id,academic_term_id,subject_id FROM course_offering_details WHERE learning_offering_id=$1").bind(id).fetch_one(&pool).await.unwrap();
    for (year_status,term_status,offering_status) in [
        ("closed","planning","published"),("archived","planning","published"),
        ("active","closed","published"),("active","cancelled","published"),
        ("active","planning","draft"),("active","planning","closed"),
    ] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1").bind(year).bind(year_status).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2,closed_on=CASE WHEN $2='closed' THEN planned_end_date ELSE NULL END WHERE id=$1").bind(term).bind(term_status).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE learning_offerings SET status=$2 WHERE id=$1").bind(id).bind(offering_status).execute(&mut *tx).await.unwrap();
        fill(&mut tx,&[id]).await;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM course_assessment_plans WHERE learning_offering_id=$1").bind(id).fetch_one(&mut *tx).await.unwrap();
        assert_eq!(count,0,"{year_status}/{term_status}/{offering_status}");
        tx.rollback().await.unwrap();
    }
    let policy: Uuid = sqlx::query_scalar("INSERT INTO academic_grading_policy_versions(version_no,name,lifecycle) VALUES(999,'Coordinator lock fixture','draft') RETURNING id").fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_course_result_locks(subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by)
        VALUES($1,$2,$3,$4,'{}',repeat('a',64),repeat('b',64),'{}',$5)")
        .bind(subject).bind(term).bind(year).bind(policy).bind(PRIMARY).execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    fill(&mut tx,&[id]).await;
    tx.commit().await.unwrap();
    assert!(services::get_plan_detail(&pool,id).await.unwrap().id.is_none());
    apply_migrations_through(&pool,146).await.unwrap();
    assert!(services::get_plan_detail(&pool,id).await.unwrap().id.is_none());
}
