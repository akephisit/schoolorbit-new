use crate::integration_tests::academic::{
    core::services::activation_context_tests::{
        empty_school, ready_term_for_existing_year, ready_year,
    },
    delivery,
    lifecycle::models::{ExecutePromotionRunInput, PromotionDecisionOutcome},
    results::{models as result_models, services as result_services},
    services::timetable_service,
};
use chrono::NaiveDate;
use school_authorization::ActorContext;
use school_permissions::registry::codes;
use uuid::Uuid;

fn lifecycle_actor(user_id: Uuid) -> ActorContext {
    ActorContext {
        user_id,
        permissions: vec![codes::WILDCARD.into()],
    }
}

#[tokio::test]
async fn opening_current_evidence_tracks_exact_term_and_current_revision() {
    let (pool, actor) = empty_school("opening_current_evidence").await;
    let (year, term) = ready_year(
        &pool,
        actor,
        2569,
        NaiveDate::from_ymd_opt(2026, 5, 16).unwrap(),
    )
    .await;
    let mut tx = pool.begin().await.unwrap();
    let empty = delivery::services::opening::current_for_term(&mut tx, year.id, term.id)
        .await
        .unwrap();
    let absent = timetable_service::opening::current_for_term(
        &mut tx,
        year.id,
        term.id,
        term.bell_schedule_id,
    )
    .await
    .unwrap();
    assert_eq!((empty.count(), absent.count()), (0, 0));
    let offering:Uuid=sqlx::query_scalar("INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,owning_organization_unit_id,weekly_period_target) SELECT uuidv7(),$1,$2,'activity','CURRENT','Current presence',id,1 FROM organization_units WHERE is_active ORDER BY id LIMIT 1 RETURNING id").bind(term.id).bind(year.id).fetch_one(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_timetables(academic_term_id,academic_year_id,bell_schedule_id) VALUES($1,$2,$3)").bind(term.id).bind(year.id).bind(term.bell_schedule_id).execute(&mut *tx).await.unwrap();
    let current = delivery::services::opening::current_for_term(&mut tx, year.id, term.id)
        .await
        .unwrap();
    let table = timetable_service::opening::current_for_term(
        &mut tx,
        year.id,
        term.id,
        term.bell_schedule_id,
    )
    .await
    .unwrap();
    assert_eq!((current.count(), table.count()), (1, 1));
    assert_ne!(empty.source_checksum, current.source_checksum);
    assert_ne!(absent.source_checksum, table.source_checksum);
    sqlx::query("UPDATE learning_offerings SET weekly_period_target=2,row_version=row_version+1 WHERE id=$1").bind(offering).execute(&mut *tx).await.unwrap();
    assert_ne!(
        current.source_checksum,
        delivery::services::opening::current_for_term(&mut tx, year.id, term.id)
            .await
            .unwrap()
            .source_checksum
    );
    assert_eq!(
        timetable_service::opening::current_for_term(&mut tx, year.id, term.id, Uuid::new_v4())
            .await
            .unwrap()
            .count(),
        0
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn opening_readiness_does_not_invent_optional_work_and_rechecks_enabled_current_presence_gates()
 {
    let (pool, actor_id) = empty_school("opening_optional_gates").await;
    let (year, term) = ready_year(
        &pool,
        actor_id,
        2569,
        NaiveDate::from_ymd_opt(2026, 5, 16).unwrap(),
    )
    .await;
    let actor = lifecycle_actor(actor_id);
    let initial = super::get_activation_workspace(&pool, &actor, year.id, term.id)
        .await
        .unwrap();
    assert!(initial.opens_year);
    assert!(initial.can_activate);
    assert!(initial.findings.is_empty());

    let policy = super::update_opening_policy(
        &pool,
        &actor,
        super::super::models::UpdateOpeningPolicyInput {
            row_version: initial.policy.row_version,
            require_homeroom_placements: false,
            require_offerings: true,
            require_timetable: true,
        },
    )
    .await
    .unwrap();
    let gated = super::get_activation_workspace(&pool, &actor, year.id, term.id)
        .await
        .unwrap();
    assert_eq!(gated.policy, policy);
    assert!(!gated.can_activate);
    assert_ne!(gated.source_checksum, initial.source_checksum);
    assert_eq!(
        gated
            .findings
            .iter()
            .map(|finding| finding.code.as_str())
            .collect::<Vec<_>>(),
        vec!["opening.offering_missing", "opening.timetable_missing"]
    );

    let read_only_actor = ActorContext {
        user_id: actor_id,
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    let read_only = super::get_activation_workspace(&pool, &read_only_actor, year.id, term.id)
        .await
        .unwrap();
    assert_eq!(read_only.source_checksum, gated.source_checksum);
    assert!(
        read_only
            .findings
            .iter()
            .all(|finding| finding.resolution_url.is_none())
    );
}

#[tokio::test]
async fn opening_readiness_counts_planned_students_without_an_eligible_homeroom() {
    let (pool, actor_id) = empty_school("opening_homeroom_gate").await;
    let (year, term) = ready_year(
        &pool,
        actor_id,
        2569,
        NaiveDate::from_ymd_opt(2026, 5, 16).unwrap(),
    )
    .await;
    let grade: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type='secondary' AND year=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let edition:Uuid=sqlx::query_scalar("INSERT INTO curriculum_editions(name,revision_year) VALUES('E2E-LIFECYCLE-test',2569) RETURNING id").fetch_one(&pool).await.unwrap();
    let version:Uuid=sqlx::query_scalar("INSERT INTO curriculum_levels(edition_id,code,name_th,grade_level_ids,is_active) VALUES($1,$2,'E2E-LIFECYCLE-target',$3,true) RETURNING id").bind(edition).bind(Uuid::new_v4().to_string()).bind(sqlx::types::Json(vec![grade])).fetch_one(&pool).await.unwrap();
    let program: Uuid = sqlx::query_scalar(
        "INSERT INTO study_programs(id,curriculum_level_id,code,name_th,status)
         VALUES(uuid_generate_v4(),$1,'E2E-OPEN-ROOM','E2E-LIFECYCLE-room','published') RETURNING id",
    )
    .bind(version)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("SELECT capture_curriculum_publication($1,$2,'Synthetic placement publication')")
        .bind(edition)
        .bind(actor_id)
        .execute(&pool)
        .await
        .unwrap();
    let room: Uuid = sqlx::query_scalar(
        "INSERT INTO homerooms(code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active)
         VALUES('E2E-OPEN-ROOM','ม.1/1',$1,$2,$3,'1',30,true) RETURNING id",
    )
    .bind(year.id)
    .bind(grade)
    .bind(program)
    .fetch_one(&pool)
    .await
    .unwrap();
    let student: Uuid = sqlx::query_scalar(
        "INSERT INTO users(username,password_hash,first_name,last_name,user_type)
         VALUES('E2E-LIFECYCLE-room','!','นักเรียน','เปิดปี','student') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let student_year: Uuid = sqlx::query_scalar(
        "INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status)
         VALUES(uuid_generate_v4(),$1,$2,$3,$4,'planned') RETURNING id",
    )
    .bind(student)
    .bind(year.id)
    .bind(grade)
    .bind(program)
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor = lifecycle_actor(actor_id);
    let initial = super::get_activation_workspace(&pool, &actor, year.id, term.id)
        .await
        .unwrap();
    assert_eq!(initial.planned_students, 1);
    assert_eq!(initial.eligible_placements, 0);
    assert!(initial.can_activate);
    super::update_opening_policy(
        &pool,
        &actor,
        super::super::models::UpdateOpeningPolicyInput {
            row_version: initial.policy.row_version,
            require_homeroom_placements: true,
            require_offerings: false,
            require_timetable: false,
        },
    )
    .await
    .unwrap();
    let blocked = super::get_activation_workspace(&pool, &actor, year.id, term.id)
        .await
        .unwrap();
    assert!(!blocked.can_activate);
    assert_eq!(blocked.findings.len(), 1);
    assert_eq!(
        blocked.findings[0].code,
        "opening.homeroom_placement_missing"
    );
    assert_eq!(blocked.findings[0].count, 1);

    sqlx::query(
        "INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type)
         VALUES(uuid_generate_v4(),$1,$2,$3,$4,'planned','new')",
    )
    .bind(student_year)
    .bind(year.id)
    .bind(room)
    .bind(chrono::Utc::now().with_timezone(&chrono_tz::Asia::Bangkok).date_naive())
    .execute(&pool)
    .await
    .unwrap();
    let ready = super::get_activation_workspace(&pool, &actor, year.id, term.id)
        .await
        .unwrap();
    assert!(ready.can_activate);
    assert!(ready.findings.is_empty());
    assert_eq!(ready.eligible_placements, 1);
    assert_ne!(ready.source_checksum, blocked.source_checksum);
}

#[tokio::test]
async fn promotion_opening_requires_executed_coverage_and_surfaces_later_result_corrections() {
    let (pool, executor, results_actor, context, calculation, approved) =
        super::promotion_execution::tests::approved_fixture(
            "promotion_opening_coverage",
            PromotionDecisionOutcome::Hold,
        )
        .await;
    let mut tx = pool.begin().await.unwrap();
    let before = super::promotion_opening::read_in_transaction(
        &mut tx,
        calculation.run.source_year_id,
        calculation.run.target_year_id,
    )
    .await
    .unwrap();
    assert_eq!(before.source_students, 1);
    assert_eq!(before.covered_students, 0);
    assert_eq!(before.missing_students, 1);
    assert_eq!(before.executing_runs, 0);
    assert_eq!(before.inconsistent_receipts, 0);
    assert_eq!(before.unresolved_impacts, 0);
    tx.rollback().await.unwrap();

    super::execute_run(
        &pool,
        &executor,
        calculation.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved.row_version,
            limit: 100,
        },
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let covered = super::promotion_opening::read_in_transaction(
        &mut tx,
        calculation.run.source_year_id,
        calculation.run.target_year_id,
    )
    .await
    .unwrap();
    assert_eq!(covered.covered_students, 1);
    assert_eq!(covered.missing_students, 0);
    assert_eq!(covered.inconsistent_receipts, 0);
    assert_eq!(covered.unresolved_impacts, 0);
    assert_ne!(covered.source_checksum, before.source_checksum);
    tx.rollback().await.unwrap();

    let course: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(calculation.items[0].student_academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    result_services::correct_result(
        &pool,
        &results_actor,
        &context,
        result_models::ResultCorrectionInput::Course {
            course_result_id: course,
            outcome: result_models::CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let corrected = super::promotion_opening::read_in_transaction(
        &mut tx,
        calculation.run.source_year_id,
        calculation.run.target_year_id,
    )
    .await
    .unwrap();
    assert_eq!(corrected.unresolved_impacts, 1);
    assert_ne!(corrected.source_checksum, covered.source_checksum);
}

#[tokio::test]
async fn promotion_opening_reports_inflight_runs_and_receipt_owned_target_drift() {
    let (pool, _, calculation, _) =
        super::promotion_execution::tests::started_run_fixture("promotion_opening_inflight").await;
    let mut tx = pool.begin().await.unwrap();
    let inflight = super::promotion_opening::read_in_transaction(
        &mut tx,
        calculation.run.source_year_id,
        calculation.run.target_year_id,
    )
    .await
    .unwrap();
    assert_eq!(inflight.executing_runs, 1);
    tx.rollback().await.unwrap();

    let (pool, executor, _, _, calculation, approved) =
        super::promotion_execution::tests::approved_fixture(
            "promotion_opening_receipt_drift",
            PromotionDecisionOutcome::Promote,
        )
        .await;
    let executed = super::execute_run(
        &pool,
        &executor,
        calculation.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved.row_version,
            limit: 100,
        },
    )
    .await
    .unwrap();
    let target = executed.receipts[0].target_student_year_id.unwrap();
    sqlx::query("UPDATE student_academic_years SET status='active' WHERE id=$1")
        .bind(target)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let drifted = super::promotion_opening::read_in_transaction(
        &mut tx,
        calculation.run.source_year_id,
        calculation.run.target_year_id,
    )
    .await
    .unwrap();
    assert_eq!(drifted.inconsistent_receipts, 1);
    assert_eq!(drifted.covered_students, 0);
    assert_eq!(drifted.missing_students, 1);
}

#[tokio::test]
async fn first_term_opening_readiness_requires_promotion_coverage_but_not_for_a_new_school() {
    let (pool, executor, _, _, calculation, approved) =
        super::promotion_execution::tests::approved_fixture(
            "opening_workspace_promotion",
            PromotionDecisionOutcome::Hold,
        )
        .await;
    let target_number: i32 = sqlx::query_scalar("SELECT year FROM academic_years WHERE id=$1")
        .bind(calculation.run.target_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let intervening:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM academic_years WHERE year>(SELECT year FROM academic_years WHERE id=$1) AND year<$2 ORDER BY year,id").bind(calculation.run.source_year_id).bind(target_number).fetch_all(&pool).await.unwrap();
    for (index, year) in intervening.into_iter().enumerate() {
        sqlx::query("UPDATE academic_years SET year=$2 WHERE id=$1")
            .bind(year)
            .bind(target_number + 1 + index as i32)
            .execute(&pool)
            .await
            .unwrap();
    }
    let target_start = calculation.run.target_placement_starts_on.unwrap();
    let term = ready_term_for_existing_year(
        &pool,
        executor.user_id,
        calculation.run.target_year_id,
        target_start,
    )
    .await;
    let reader = lifecycle_actor(executor.user_id);
    let before =
        super::get_activation_workspace(&pool, &reader, calculation.run.target_year_id, term.id)
            .await
            .unwrap();
    assert!(
        before.findings.iter().any(
            |finding| finding.code == "opening.promotion_decision_missing" && finding.count == 1
        )
    );

    super::execute_run(
        &pool,
        &executor,
        calculation.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved.row_version,
            limit: 100,
        },
    )
    .await
    .unwrap();
    let covered =
        super::get_activation_workspace(&pool, &reader, calculation.run.target_year_id, term.id)
            .await
            .unwrap();
    assert!(
        !covered
            .findings
            .iter()
            .any(|finding| finding.code == "opening.promotion_decision_missing")
    );
    assert_ne!(covered.source_checksum, before.source_checksum);
}
