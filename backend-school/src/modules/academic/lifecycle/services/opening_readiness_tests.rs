use crate::modules::academic::{
    core::services::activation_context_tests::{
        empty_school, ready_term_for_existing_year, ready_year,
    },
    delivery,
    lifecycle::models::{ExecutePromotionRunInput, PromotionDecisionOutcome},
    results::{models as result_models, services as result_services},
    services::timetable_version_service,
};
use crate::{middleware::permission::ActorContext, permissions::registry::codes};
use chrono::{Duration, NaiveDate};
use uuid::Uuid;

fn lifecycle_actor(user_id: Uuid) -> ActorContext {
    ActorContext {
        user_id,
        permissions: vec![codes::WILDCARD.into()],
    }
}

#[tokio::test]
async fn opening_publication_evidence_tracks_only_usable_target_term_publications() {
    let (pool, actor) = empty_school("opening_publication_evidence").await;
    let (year, term) = ready_year(
        &pool,
        actor,
        2569,
        NaiveDate::from_ymd_opt(2026, 5, 16).unwrap(),
    )
    .await;
    let mut tx = pool.begin().await.unwrap();
    let offerings =
        delivery::services::opening::published_for_term(&mut tx, year.id, term.id, term.start_date)
            .await
            .unwrap();
    let timetables = timetable_version_service::opening::published_for_term(
        &mut tx,
        year.id,
        term.id,
        term.start_date,
        term.bell_schedule_id,
    )
    .await
    .unwrap();
    assert_eq!(offerings.count(), 0);
    assert_eq!(timetables.count(), 0);
    let empty_offering_hash = offerings.source_checksum.clone();
    let empty_timetable_hash = timetables.source_checksum.clone();
    tx.rollback().await.unwrap();

    let offering = Uuid::new_v4();
    let owner: Uuid = sqlx::query_scalar(
        "SELECT id FROM organization_units WHERE is_active IS TRUE ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let activity = Uuid::new_v4();
    let activity_version = Uuid::new_v4();
    let mut setup = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO activities(id,code,identity_key,activity_type,owning_organization_unit_id)
         VALUES($1,'E2E-OPEN','e2e-opening','other',$2)",
    )
    .bind(activity)
    .bind(owner)
    .execute(&mut *setup)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO activity_versions(id,activity_id,version_no,name,activity_type,periods_per_week,hours_per_week,scheduling_mode,is_active,grade_level_ids,start_academic_year_id,effective_from,status,published_at)
         VALUES($1,$2,1,'E2E-LIFECYCLE-opening','other',1,1,'synchronized',true,'[]'::jsonb,$3,$4,'published',now())",
    )
    .bind(activity_version)
    .bind(activity)
    .bind(year.id)
    .bind(term.start_date)
    .execute(&mut *setup)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,status,starts_on,owning_organization_unit_id)
         VALUES($1,$2,$3,'activity','E2E-OPEN','E2E-LIFECYCLE-opening','draft',$4,$5)",
    )
    .bind(offering)
    .bind(term.id)
    .bind(year.id)
    .bind(term.start_date)
    .bind(owner)
    .execute(&mut *setup)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO activity_offering_details(learning_offering_id,academic_term_id,academic_year_id,activity_version_id,activity_id,registration_type,scheduling_mode,hours)
         VALUES($1,$2,$3,$4,$5,'assigned','synchronized',20)",
    )
    .bind(offering)
    .bind(term.id)
    .bind(year.id)
    .bind(activity_version)
    .bind(activity)
    .execute(&mut *setup)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE learning_offerings SET status='published',published_at=now(),row_version=row_version+1 WHERE id=$1",
    )
    .bind(offering)
    .execute(&mut *setup)
    .await
    .unwrap();
    let version = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,effective_from,status,bell_schedule_id,published_by,published_at)
         VALUES($1,$2,$3,$4,'published',$5,$6,now())",
    )
    .bind(version)
    .bind(term.id)
    .bind(year.id)
    .bind(term.start_date)
    .bind(term.bell_schedule_id)
    .bind(actor)
    .execute(&mut *setup)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,effective_from,status,bell_schedule_id,published_by,published_at)
         VALUES($1,$2,$3,$4,'published',$5,$6,now())",
    )
    .bind(Uuid::new_v4())
    .bind(term.id)
    .bind(year.id)
    .bind(term.start_date.succ_opt().unwrap())
    .bind(term.bell_schedule_id)
    .bind(actor)
    .execute(&mut *setup)
    .await
    .unwrap();
    setup.commit().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let offerings =
        delivery::services::opening::published_for_term(&mut tx, year.id, term.id, term.start_date)
            .await
            .unwrap();
    let timetables = timetable_version_service::opening::published_for_term(
        &mut tx,
        year.id,
        term.id,
        term.start_date,
        term.bell_schedule_id,
    )
    .await
    .unwrap();
    assert_eq!(offerings.count(), 1);
    assert_eq!(timetables.count(), 1);
    assert_ne!(offerings.source_checksum, empty_offering_hash);
    assert_ne!(timetables.source_checksum, empty_timetable_hash);

    sqlx::query(
        "UPDATE learning_offerings SET status='cancelled',row_version=row_version+1 WHERE id=$1",
    )
    .bind(offering)
    .execute(&mut *tx)
    .await
    .unwrap();
    let cancelled =
        delivery::services::opening::published_for_term(&mut tx, year.id, term.id, term.start_date)
            .await
            .unwrap();
    assert_eq!(cancelled.count(), 0);
}

#[tokio::test]
async fn opening_readiness_does_not_invent_optional_work_and_rechecks_enabled_publication_gates() {
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
            require_published_offerings: true,
            require_published_timetable: true,
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
        vec![
            "opening.published_offering_missing",
            "opening.published_timetable_missing"
        ]
    );

    let read_only_actor = ActorContext {
        user_id: actor_id,
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    let read_only = super::get_activation_workspace(&pool, &read_only_actor, year.id, term.id)
        .await
        .unwrap();
    assert_eq!(read_only.source_checksum, gated.source_checksum);
    assert!(read_only
        .findings
        .iter()
        .all(|finding| finding.resolution_url.is_none()));
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
    let curriculum: Uuid = sqlx::query_scalar(
        "INSERT INTO curricula(code,identity_key,name_th,grade_level_ids)
         VALUES('E2E-OPEN-ROOM','E2E-OPEN-ROOM','E2E-LIFECYCLE-room',$1) RETURNING id",
    )
    .bind(sqlx::types::Json(vec![grade]))
    .fetch_one(&pool)
    .await
    .unwrap();
    let version: Uuid = sqlx::query_scalar(
        "INSERT INTO curriculum_versions(curriculum_id,version_name,start_academic_year_id,status)
         VALUES($1,'E2E-OPEN-ROOM',$2,'draft') RETURNING id",
    )
    .bind(curriculum)
    .bind(year.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let program: Uuid = sqlx::query_scalar(
        "INSERT INTO study_programs(id,curriculum_version_id,code,name_th,status)
         VALUES(uuid_generate_v4(),$1,'E2E-OPEN-ROOM','E2E-LIFECYCLE-room','published') RETURNING id",
    )
    .bind(version)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE curriculum_versions SET status='published',published_at=now() WHERE id=$1")
        .bind(version)
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
            require_published_offerings: false,
            require_published_timetable: false,
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
    .bind(term.start_date)
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
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 78)
        .await
        .unwrap();
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
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 78)
        .await
        .unwrap();
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
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 78)
        .await
        .unwrap();
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
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 78)
        .await
        .unwrap();
    let source_end: NaiveDate =
        sqlx::query_scalar("SELECT end_date FROM academic_years WHERE id=$1")
            .bind(calculation.run.source_year_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let target_end: NaiveDate =
        sqlx::query_scalar("SELECT end_date FROM academic_years WHERE id=$1")
            .bind(calculation.run.target_year_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let target_start: NaiveDate =
        sqlx::query_scalar("SELECT start_date FROM academic_years WHERE id=$1")
            .bind(calculation.run.target_year_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let intervening: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM academic_years
         WHERE id<>$1 AND id<>$2 AND start_date>$3 AND start_date<$4 ORDER BY start_date,id",
    )
    .bind(calculation.run.source_year_id)
    .bind(calculation.run.target_year_id)
    .bind(source_end)
    .bind(target_start)
    .fetch_all(&pool)
    .await
    .unwrap();
    for (index, year_id) in intervening.into_iter().enumerate() {
        let start = target_end + Duration::days(1 + (index as i64 * 366));
        sqlx::query("UPDATE academic_years SET start_date=$1,end_date=$2 WHERE id=$3")
            .bind(start)
            .bind(start + Duration::days(364))
            .bind(year_id)
            .execute(&pool)
            .await
            .unwrap();
    }
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
    assert!(before
        .findings
        .iter()
        .any(|finding| finding.code == "opening.promotion_decision_missing" && finding.count == 1));

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
    assert!(!covered
        .findings
        .iter()
        .any(|finding| finding.code == "opening.promotion_decision_missing"));
    assert_ne!(covered.source_checksum, before.source_checksum);
}
