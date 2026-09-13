use crate::modules::academic::{
    core, delivery,
    services::{exam_schedule_service, timetable_version_service},
};
use uuid::Uuid;

#[tokio::test]
async fn lifecycle_exam_readiness_changes_when_child_assignments_change() {
    let pool = core::services_tests::prepare_core_fixture("lifecycle_exam_fingerprint").await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 69)
        .await
        .unwrap();
    let (year, term, date): (Uuid, Uuid, chrono::NaiveDate) = sqlx::query_as(
        "SELECT academic_year_id,id,start_date FROM academic_terms WHERE status='active'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let round:Uuid=sqlx::query_scalar("INSERT INTO academic_exam_rounds(academic_year_id,academic_term_id,name,status) VALUES($1,$2,'E2E-LIFECYCLE-details','draft') RETURNING id").bind(year).bind(term).fetch_one(&pool).await.unwrap();
    let day:Uuid=sqlx::query_scalar("INSERT INTO academic_exam_days(exam_round_id,academic_year_id,academic_term_id,exam_date,start_time,end_time) VALUES($1,$2,$3,$4,'08:00','16:00') RETURNING id").bind(round).bind(year).bind(term).bind(date).fetch_one(&pool).await.unwrap();
    let homeroom: Uuid =
        sqlx::query_scalar("SELECT id FROM homerooms WHERE academic_year_id=$1 LIMIT 1")
            .bind(year)
            .fetch_one(&pool)
            .await
            .unwrap();
    let room: Uuid = sqlx::query_scalar("SELECT id FROM rooms LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let staff: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let student: Uuid =
        sqlx::query_scalar("SELECT id FROM users WHERE user_type='student' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let assignment:Uuid=sqlx::query_scalar("INSERT INTO academic_exam_day_room_assignments(exam_day_id,academic_year_id,academic_term_id,homeroom_id,room_id,capacity_override) VALUES($1,$2,$3,$4,$5,20) RETURNING id").bind(day).bind(year).bind(term).bind(homeroom).bind(room).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_exam_day_blocked_windows(exam_day_id,label,start_time,end_time) VALUES($1,'พัก','12:00','13:00')").bind(day).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_exam_day_invigilators(exam_day_id,day_room_assignment_id,staff_id,role_label) VALUES($1,$2,$3,'กรรมการ')").bind(day).bind(assignment).bind(staff).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_exam_seat_assignments(day_room_assignment_id,student_id,seat_number) VALUES($1,$2,'1')").bind(assignment).bind(student).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_exam_day_grade_levels(exam_day_id,grade_level_id) SELECT $1,id FROM grade_levels ORDER BY id LIMIT 1").bind(day).execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let mut before = exam_schedule_service::pending_term_work(&mut tx, year, term)
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == round)
        .unwrap()
        .revision;
    for (sql,id) in [
        ("UPDATE academic_exam_day_blocked_windows SET label='พักกลางวัน' WHERE exam_day_id=$1",day),
        ("UPDATE academic_exam_day_room_assignments SET capacity_override=25 WHERE id=$1",assignment),
        ("UPDATE academic_exam_day_invigilators SET role_label='หัวหน้าห้องสอบ' WHERE exam_day_id=$1",day),
        ("UPDATE academic_exam_seat_assignments SET seat_number='2' WHERE day_room_assignment_id=$1",assignment),
        ("DELETE FROM academic_exam_day_grade_levels WHERE exam_day_id=$1",day),
    ] {
        assert_eq!(sqlx::query(sql).bind(id).execute(&mut *tx).await.unwrap().rows_affected(),1);
        let after=exam_schedule_service::pending_term_work(&mut tx,year,term).await.unwrap().into_iter().find(|row|row.id==round).unwrap().revision;
        assert_ne!(before,after,"changed exam child data must invalidate readiness: {sql}");
        before=after;
    }
}

#[tokio::test]
async fn lifecycle_providers_report_only_selected_term_drafts_and_consequential_delivery_items() {
    let pool = core::services_tests::prepare_core_fixture("lifecycle_operational_providers").await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 68)
        .await
        .unwrap();
    let (year,term,bell,start):(Uuid,Uuid,Uuid,chrono::NaiveDate) = sqlx::query_as(
        "SELECT academic_year_id,id,bell_schedule_id,start_date FROM academic_terms WHERE status='active'"
    ).fetch_one(&pool).await.unwrap();
    let actor: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let offering:Uuid = sqlx::query_scalar("SELECT id FROM learning_offerings WHERE academic_term_id=$1 AND status='published' LIMIT 1").bind(term).fetch_one(&pool).await.unwrap();
    let exam:Uuid = sqlx::query_scalar("INSERT INTO academic_exam_rounds(academic_year_id,academic_term_id,name,status) VALUES($1,$2,'E2E-LIFECYCLE-exam','draft') RETURNING id")
        .bind(year).bind(term).fetch_one(&pool).await.unwrap();
    let timetable:Uuid = sqlx::query_scalar("INSERT INTO academic_timetable_versions(academic_year_id,academic_term_id,bell_schedule_id,effective_from,status) VALUES($1,$2,$3,$4,'draft') RETURNING id")
        .bind(year).bind(term).bind(bell).bind(start.succ_opt().unwrap()).fetch_one(&pool).await.unwrap();
    let change:Uuid = sqlx::query_scalar("INSERT INTO academic_term_change_sets(academic_year_id,academic_term_id,effective_from,reason,idempotency_key,creation_request_hash,created_by) VALUES($1,$2,$3,'E2E-LIFECYCLE-change','lifecycle-provider',repeat('a',64),$4) RETURNING id")
        .bind(year).bind(term).bind(start).bind(actor).fetch_one(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let pending = delivery::services::pending_term_work(&mut tx, year, term)
        .await
        .unwrap();
    let before = pending
        .iter()
        .find(|row| row.id == change)
        .expect("draft change is visible");
    assert!(!before.blocks_closure);
    let before_revision = before.revision.clone();
    sqlx::query("INSERT INTO academic_term_change_items(change_set_id,academic_year_id,academic_term_id,action_kind,learning_offering_id,created_by) VALUES($1,$2,$3,'stop_offering',$4,$5)")
        .bind(change).bind(year).bind(term).bind(offering).bind(actor).execute(&mut *tx).await.unwrap();
    let pending = delivery::services::pending_term_work(&mut tx, year, term)
        .await
        .unwrap();
    let after = pending.iter().find(|row| row.id == change).unwrap();
    assert!(after.blocks_closure);
    assert_ne!(before_revision, after.revision);
    assert!(
        exam_schedule_service::pending_term_work(&mut tx, year, term)
            .await
            .unwrap()
            .iter()
            .any(|row| row.id == exam && !row.blocks_closure)
    );
    assert!(
        timetable_version_service::pending_term_work(&mut tx, year, term)
            .await
            .unwrap()
            .iter()
            .any(|row| row.id == timetable && !row.blocks_closure)
    );
    assert!(
        delivery::services::pending_term_work(&mut tx, Uuid::new_v4(), term)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        exam_schedule_service::pending_term_work(&mut tx, year, Uuid::new_v4())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        timetable_version_service::pending_term_work(&mut tx, Uuid::new_v4(), term)
            .await
            .unwrap()
            .is_empty()
    );
    let reader = crate::middleware::permission::ActorContext {
        user_id: actor,
        permissions: vec![crate::permissions::registry::codes::WILDCARD.into()],
    };
    let workspace = super::workspace_in_transaction(&mut tx, &reader, year, term)
        .await
        .unwrap();
    let supervision = workspace
        .findings
        .iter()
        .find(|finding| finding.code == "supervision.warning")
        .unwrap();
    assert_eq!(
        supervision.resolution_url.as_deref(),
        Some(
            format!("/staff/academic/supervision?academicYearId={year}&academicTermId={term}")
                .as_str()
        )
    );
    let results = workspace
        .findings
        .iter()
        .find(|finding| finding.code == "results.incomplete")
        .unwrap();
    assert_eq!(
        results.resolution_url.as_deref(),
        Some(
            format!(
                "/staff/academic/results/aggregates?academicYearId={year}&academicTermId={term}"
            )
            .as_str()
        )
    );
    let partial_reader = crate::middleware::permission::ActorContext {
        user_id: actor,
        permissions: vec![
            crate::permissions::registry::codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
            crate::permissions::registry::codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
        ],
    };
    let partial = super::workspace_in_transaction(&mut tx, &partial_reader, year, term)
        .await
        .unwrap();
    assert!(partial
        .findings
        .iter()
        .filter(|finding| finding.code.starts_with("results."))
        .all(|finding| finding.resolution_url.is_none()));
}

#[tokio::test]
async fn lifecycle_permission_defaults_only_grant_verified_system_administrators() {
    use crate::permissions::registry::codes;
    let pool = core::services_tests::prepare_core_fixture("lifecycle_permission_defaults").await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 68)
        .await
        .unwrap();
    let expected = vec![
        codes::ACADEMIC_LIFECYCLE_READ_SCHOOL,
        codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
        codes::ACADEMIC_LIFECYCLE_CLOSE_SCHOOL,
        codes::ACADEMIC_LIFECYCLE_REOPEN_SCHOOL,
        codes::ACADEMIC_LIFECYCLE_ACTIVATE_SCHOOL,
    ];
    let grants:Vec<(String,bool,String)>=sqlx::query_as("SELECT role.code,role.is_system,permission.code FROM role_permissions grant_row JOIN roles role ON role.id=grant_row.role_id JOIN permissions permission ON permission.id=grant_row.permission_id WHERE permission.code=ANY($1)")
        .bind(&expected).fetch_all(&pool).await.unwrap();
    for permission in expected {
        assert!(grants.iter().any(|(_, _, code)| code == permission));
    }
    assert!(grants
        .iter()
        .all(|(role, system, _)| role == "ADMIN" && *system));
}
