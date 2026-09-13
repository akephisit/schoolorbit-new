use super::super::models::*;
use super::{bell_schedules, years_terms};
use crate::{
    error::AppError,
    modules::academic::cutover_test_support::{
        apply_migrations_through, apply_phase_b_runtime_migrations,
    },
};
use chrono::{Duration, NaiveDate, NaiveTime};
use sqlx::PgPool;
use uuid::Uuid;

pub(crate) async fn empty_school(name: &str) -> (PgPool, Uuid) {
    let pool = crate::test_helpers::create_named_test_pool_with_max_connections(name, 3).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 77).await.unwrap();
    let actor: Uuid = sqlx::query_scalar("INSERT INTO users(username,password_hash,first_name,last_name,user_type) VALUES('E2E-LIFECYCLE-opening','!','ฝ่ายวิชาการ','ทดสอบ','staff') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    (pool, actor)
}

pub(crate) async fn ready_year(
    pool: &PgPool,
    actor: Uuid,
    year_number: i32,
    start: NaiveDate,
) -> (AcademicYear, AcademicTerm) {
    let year = years_terms::create_year(
        pool,
        actor,
        CreateAcademicYearRequest {
            year: year_number,
            custom_name: None,
            start_date: start,
            end_date: start + Duration::days(364),
            school_days: vec!["MON".into()],
        },
    )
    .await
    .unwrap();
    let term = ready_term_for_existing_year(pool, actor, year.id, start).await;
    (year, term)
}

pub(crate) async fn ready_term_for_existing_year(
    pool: &PgPool,
    actor: Uuid,
    academic_year_id: Uuid,
    start: NaiveDate,
) -> AcademicTerm {
    let bell = bell_schedules::create(
        pool,
        actor,
        CreateBellScheduleRequest {
            academic_year_id,
            name: "E2E-LIFECYCLE-opening".into(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    bell_schedules::replace_periods(
        pool,
        actor,
        bell.id,
        ReplaceBellSchedulePeriodsRequest {
            row_version: bell.row_version,
            periods: vec![BellSchedulePeriodInput {
                name: None,
                start_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                order_index: 1,
                applicable_days: vec!["MON".into()],
                is_active: true,
            }],
        },
    )
    .await
    .unwrap();
    let term = years_terms::create_term(
        pool,
        actor,
        CreateAcademicTermRequest {
            academic_year_id,
            term_type: AcademicTermType::Regular,
            custom_name: None,
            start_date: start,
            planned_end_date: None,
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id: bell.id,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE academic_terms SET status='ready',row_version=row_version+1 WHERE id=$1")
        .bind(term.id)
        .execute(pool)
        .await
        .unwrap();
    term
}

async fn inspect(pool: &PgPool, year: Uuid, term: Uuid) -> ActivationState {
    let mut tx = pool.begin().await.unwrap();
    let state = super::activation_context::read_activation_state(&mut tx, year, term)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    state
}

#[tokio::test]
async fn activation_context_initial_school_has_no_fabricated_predecessor_and_hashes_bell_changes() {
    let (pool, actor) = empty_school("activation_context_initial").await;
    let (year, term) = ready_year(
        &pool,
        actor,
        2569,
        NaiveDate::from_ymd_opt(2026, 5, 16).unwrap(),
    )
    .await;
    let before = inspect(&pool, year.id, term.id).await;
    assert!(before.opens_year);
    assert!(before.predecessor.is_none());
    assert!(before.issues.is_empty(), "{:?}", before.issues);
    assert!(before.students.is_empty());
    assert!(before.placements.is_empty());
    assert!(before.context.planned_end_date.is_none());
    assert_eq!(
        before.source_checksum,
        inspect(&pool, year.id, term.id).await.source_checksum
    );
    sqlx::query("UPDATE bell_schedule_periods SET start_time='08:15' WHERE bell_schedule_id=$1")
        .bind(term.bell_schedule_id)
        .execute(&pool)
        .await
        .unwrap();
    let later = inspect(&pool, year.id, term.id).await;
    assert!(later.issues.is_empty());
    assert_ne!(before.source_checksum, later.source_checksum);
    sqlx::query("UPDATE bell_schedule_periods SET is_active=false WHERE bell_schedule_id=$1")
        .bind(term.bell_schedule_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(inspect(&pool, year.id, term.id)
        .await
        .issues
        .iter()
        .any(|row| row.code == ActivationIssueCode::BellSchedule));
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        super::activation_context::read_activation_state(&mut tx, year.id, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn activation_context_requires_closed_prior_context_and_first_future_term() {
    let (pool, actor) = empty_school("activation_context_sequence").await;
    let start = NaiveDate::from_ymd_opt(2026, 5, 16).unwrap();
    let (source, first) = ready_year(&pool, actor, 2569, start).await;
    let (target, next) = ready_year(&pool, actor, 2570, start + Duration::days(365)).await;
    let blocked = inspect(&pool, target.id, next.id).await;
    assert_eq!(
        blocked.predecessor.as_ref().unwrap().academic_year_id,
        source.id
    );
    assert!(blocked
        .issues
        .iter()
        .any(|row| row.code == ActivationIssueCode::EarlierYearOpen));
    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(source.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='active' WHERE id=$1")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    let blocked = inspect(&pool, target.id, next.id).await;
    assert!(blocked
        .issues
        .iter()
        .any(|row| row.code == ActivationIssueCode::OtherRunningYear));
    assert!(blocked
        .issues
        .iter()
        .any(|row| row.code == ActivationIssueCode::OtherRunningTerm));
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=start_date WHERE id=$1")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(source.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(inspect(&pool, target.id, next.id).await.issues.is_empty());
    let summer = years_terms::create_term(
        &pool,
        actor,
        CreateAcademicTermRequest {
            academic_year_id: target.id,
            term_type: AcademicTermType::Summer,
            custom_name: None,
            start_date: target.start_date + Duration::days(250),
            planned_end_date: None,
            included_in_year_result: false,
            blocks_year_closure: false,
            bell_schedule_id: next.bell_schedule_id,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE academic_terms SET status='ready' WHERE id=$1")
        .bind(summer.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(inspect(&pool, target.id, summer.id)
        .await
        .issues
        .iter()
        .any(|row| row.code == ActivationIssueCode::NotFirstTerm));
    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(target.id)
        .execute(&pool)
        .await
        .unwrap();
    let same_year = inspect(&pool, target.id, summer.id).await;
    assert!(!same_year.opens_year);
    assert!(same_year
        .issues
        .iter()
        .any(|row| row.code == ActivationIssueCode::EarlierTermOpen));
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=start_date WHERE id=$1")
        .bind(next.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(inspect(&pool, target.id, summer.id).await.issues.is_empty());
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        super::activation_context::read_activation_state(&mut tx, source.id, summer.id).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn activation_context_validates_planned_curriculum_placement_capacity_and_future_dates() {
    let (pool, actor) = empty_school("activation_context_placements").await;
    let (year, term) = ready_year(
        &pool,
        actor,
        2569,
        NaiveDate::from_ymd_opt(2026, 5, 16).unwrap(),
    )
    .await;
    let grade: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type='secondary' AND year=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let curriculum: Uuid = sqlx::query_scalar("INSERT INTO curricula(code,identity_key,name_th,grade_level_ids) VALUES('E2E-OPEN','E2E-OPEN','E2E-LIFECYCLE-opening',$1) RETURNING id")
        .bind(sqlx::types::Json(vec![grade])).fetch_one(&pool).await.unwrap();
    let version: Uuid = sqlx::query_scalar("INSERT INTO curriculum_versions(curriculum_id,version_name,start_academic_year_id,status) VALUES($1,'E2E-OPEN',$2,'draft') RETURNING id")
        .bind(curriculum).bind(year.id).fetch_one(&pool).await.unwrap();
    let program: Uuid = sqlx::query_scalar("INSERT INTO study_programs(id,curriculum_version_id,code,name_th,status) VALUES(uuid_generate_v4(),$1,'E2E-OPEN','E2E-LIFECYCLE-opening','published') RETURNING id")
        .bind(version).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE curriculum_versions SET status='published',published_at=now() WHERE id=$1")
        .bind(version)
        .execute(&pool)
        .await
        .unwrap();
    let room: Uuid = sqlx::query_scalar("INSERT INTO homerooms(code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active) VALUES('E2E-OPEN','ม.1/1',$1,$2,$3,'1',1,true) RETURNING id")
        .bind(year.id).bind(grade).bind(program).fetch_one(&pool).await.unwrap();
    for index in 0..2 {
        let student: Uuid = sqlx::query_scalar("INSERT INTO users(username,password_hash,first_name,last_name,user_type) VALUES($1,'!','นักเรียน','ทดสอบ','student') RETURNING id")
            .bind(format!("E2E-LIFECYCLE-opening-{index}")).fetch_one(&pool).await.unwrap();
        let sy: Uuid = sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES(uuid_generate_v4(),$1,$2,$3,$4,'planned') RETURNING id")
            .bind(student).bind(year.id).bind(grade).bind(program).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) VALUES(uuid_generate_v4(),$1,$2,$3,$4,'planned','new')")
            .bind(sy).bind(year.id).bind(room).bind(year.start_date + Duration::days(20)).execute(&pool).await.unwrap();
        if index == 0 {
            let future = inspect(&pool, year.id, term.id).await;
            assert!(future.issues.is_empty(), "{:?}", future.issues);
            assert_eq!(future.students.len(), 1);
            assert!(!future.placements[0].eligible);
            sqlx::query(
                "UPDATE homeroom_placements SET start_date=$1 WHERE student_academic_year_id=$2",
            )
            .bind(year.start_date)
            .bind(sy)
            .execute(&pool)
            .await
            .unwrap();
            let present = inspect(&pool, year.id, term.id).await;
            assert!(present.issues.is_empty());
            assert!(present.placements[0].eligible);
            assert_ne!(present.source_checksum, future.source_checksum);
            sqlx::query("UPDATE homerooms SET row_version=row_version+1 WHERE id=$1")
                .bind(room)
                .execute(&pool)
                .await
                .unwrap();
            let room_revised = inspect(&pool, year.id, term.id).await;
            assert_ne!(room_revised.source_checksum, present.source_checksum);
            sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
                .bind(student)
                .execute(&pool)
                .await
                .unwrap();
            assert!(inspect(&pool, year.id, term.id)
                .await
                .issues
                .iter()
                .any(|issue| issue.code == ActivationIssueCode::StudentReference));
            sqlx::query("UPDATE users SET status='active' WHERE id=$1")
                .bind(student)
                .execute(&pool)
                .await
                .unwrap();
        }
    }
    sqlx::query("UPDATE homeroom_placements SET start_date=$1 WHERE academic_year_id=$2")
        .bind(year.start_date)
        .bind(year.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(inspect(&pool, year.id, term.id)
        .await
        .issues
        .iter()
        .any(|issue| issue.code == ActivationIssueCode::RoomCapacity));
    sqlx::query("UPDATE homerooms SET is_active=false WHERE id=$1")
        .bind(room)
        .execute(&pool)
        .await
        .unwrap();
    assert!(inspect(&pool, year.id, term.id)
        .await
        .issues
        .iter()
        .any(|issue| issue.code == ActivationIssueCode::PlacementReference));
}
