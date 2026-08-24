use super::models::{
    AcademicTermStatus, AcademicTermType, AcademicYearStatus, BellSchedulePeriodInput,
    CreateAcademicTermRequest, CreateActivityVersionRequest, CreateBellScheduleRequest,
    CreateCatalogActivityRequest, CreateCatalogSubjectRequest, CreateCurriculumRequest,
    CreateCurriculumVersionRequest, CreateHomeroomPlacementRequest, CreateHomeroomRequest,
    CreateStudentAcademicYearRequest, CreateStudyProgramRequest, CreateSubjectGroupRequest,
    CreateSubjectVersionRequest, HomeroomPlacementStatus, ProgramRequirementInput,
    PublishVersionRequest, ReplaceBellSchedulePeriodsRequest, ReplaceGradeProgressionsRequest,
    ReplaceProgramRequirementsRequest, RequirementKind, RequirementResourceKind,
    TransferHomeroomPlacementRequest, UpdateAcademicTermRequest, UpdateAcademicYearRequest,
    UpdateActivityVersionRequest, UpdateCatalogActivityRequest, UpdateCatalogSubjectRequest,
    UpdateStudyProgramRequest, UpdateSubjectGroupRequest, UpdateSubjectVersionRequest,
    VersionStatus,
};
use super::services::{
    bell_schedules, catalog, context, curriculum, ensure_draft_version, ensure_planning_delete,
    parse_row_version, progressions, student_years, validate_canonical_decimal,
    validate_date_containment, validate_term_definitions, years_terms,
};
use crate::{
    modules::academic::cutover_test_support::{
        apply_migrations_through, seed_academic_cutover_fixture, CutoverFixture,
    },
    test_helpers::create_named_test_pool,
};
use chrono::{NaiveDate, NaiveTime};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const CURRENT_YEAR_ID: Uuid = Uuid::from_u128(0x1000_0000_0000_0000_0000_0000_0000_0025);
const FUTURE_YEAR_ID: Uuid = Uuid::from_u128(0x1000_0000_0000_0000_0000_0000_0000_0026);

async fn prepare_core_fixture(name: &str) -> PgPool {
    let pool = create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 44).await.unwrap();
    pool
}

async fn fixture_actor(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT id FROM users WHERE user_type = 'staff' ORDER BY id LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[test]
fn academic_enums_serialize_as_snake_case() {
    assert_eq!(
        serde_json::to_value(AcademicYearStatus::Planning).unwrap(),
        json!("planning")
    );
    assert_eq!(
        serde_json::to_value(AcademicTermStatus::Cancelled).unwrap(),
        json!("cancelled")
    );
    assert_eq!(
        serde_json::to_value(AcademicTermType::Summer).unwrap(),
        json!("summer")
    );
}

#[test]
fn request_dtos_use_camel_case_and_reject_unknown_fields() {
    let payload = json!({
        "name": "ปีการศึกษา 2570",
        "year": 2570,
        "startDate": "2027-05-01",
        "endDate": "2028-03-31",
        "schoolDays": ["MON", "TUE"],
        "rowVersion": 3
    });
    let request: UpdateAcademicYearRequest = serde_json::from_value(payload).unwrap();
    assert_eq!(request.row_version, 3);

    let unknown = json!({
        "name": "ปีการศึกษา 2570",
        "year": 2570,
        "startDate": "2027-05-01",
        "endDate": "2028-03-31",
        "schoolDays": ["MON"],
        "rowVersion": 3,
        "status": "active"
    });
    assert!(serde_json::from_value::<UpdateAcademicYearRequest>(unknown).is_err());
}

#[test]
fn canonical_decimal_validation_rejects_ambiguous_wire_values() {
    assert_eq!(
        validate_canonical_decimal("2.50", 2).unwrap().to_string(),
        "2.50"
    );
    assert_eq!(validate_canonical_decimal("0", 2).unwrap().to_string(), "0");
    for invalid in ["02.50", "+2.5", "2.500", "2e1", " 2.5", "-0"] {
        assert!(validate_canonical_decimal(invalid, 2).is_err(), "{invalid}");
    }
}

#[test]
fn child_dates_must_be_contained_by_parent_dates() {
    let year_start = NaiveDate::from_ymd_opt(2027, 5, 1).unwrap();
    let year_end = NaiveDate::from_ymd_opt(2028, 3, 31).unwrap();
    let term_start = NaiveDate::from_ymd_opt(2027, 10, 1).unwrap();
    let term_end = NaiveDate::from_ymd_opt(2028, 1, 31).unwrap();
    assert!(validate_date_containment(year_start, year_end, term_start, term_end).is_ok());
    assert!(validate_date_containment(
        year_start,
        year_end,
        year_start.pred_opt().unwrap(),
        term_end
    )
    .is_err());
    assert!(validate_date_containment(
        year_start,
        year_end,
        term_start,
        year_end.succ_opt().unwrap()
    )
    .is_err());
}

#[test]
fn duplicate_term_code_and_sequence_have_stable_messages() {
    let year_id = Uuid::new_v4();
    let terms = vec![
        CreateAcademicTermRequest::fixture(year_id, 1, "T1"),
        CreateAcademicTermRequest::fixture(year_id, 1, "T2"),
    ];
    assert_eq!(
        validate_term_definitions(&terms)
            .unwrap_err()
            .public_message(),
        "ลำดับภาคเรียนซ้ำภายในปีการศึกษา"
    );

    let terms = vec![
        CreateAcademicTermRequest::fixture(year_id, 1, "T1"),
        CreateAcademicTermRequest::fixture(year_id, 2, "T1"),
    ];
    assert_eq!(
        validate_term_definitions(&terms)
            .unwrap_err()
            .public_message(),
        "รหัสภาคเรียนซ้ำภายในปีการศึกษา"
    );
}

#[test]
fn optimistic_versions_must_be_positive() {
    assert_eq!(parse_row_version(1).unwrap(), 1);
    assert!(parse_row_version(0).is_err());
    assert!(parse_row_version(-1).is_err());
}

#[test]
fn published_versions_are_immutable() {
    assert!(ensure_draft_version(VersionStatus::Draft).is_ok());
    assert!(ensure_draft_version(VersionStatus::Published).is_err());
    assert!(ensure_draft_version(VersionStatus::Archived).is_err());
}

#[test]
fn term_delete_requires_planning_and_no_dependencies() {
    assert!(ensure_planning_delete(AcademicTermStatus::Planning, 0).is_ok());
    assert!(ensure_planning_delete(AcademicTermStatus::Ready, 0).is_err());
    assert!(ensure_planning_delete(AcademicTermStatus::Planning, 1).is_err());
}

#[test]
fn draft_replacement_and_subject_group_updates_require_row_versions() {
    let progression = json!({ "progressions": [], "rowVersion": 4 });
    let request: ReplaceGradeProgressionsRequest = serde_json::from_value(progression).unwrap();
    assert_eq!(request.row_version, 4);
    assert!(
        serde_json::from_value::<ReplaceGradeProgressionsRequest>(json!({
            "progressions": []
        }))
        .is_err()
    );

    let group = json!({
        "code": "MA",
        "nameTh": "คณิตศาสตร์",
        "nameEn": "Mathematics",
        "displayOrder": 2,
        "isActive": true,
        "rowVersion": 3
    });
    let request: UpdateSubjectGroupRequest = serde_json::from_value(group).unwrap();
    assert_eq!(request.row_version, 3);
}

#[test]
fn flat_version_update_contract_rejects_unknown_fields() {
    let payload = json!({
        "nameTh": "คณิตศาสตร์พื้นฐาน",
        "nameEn": null,
        "credit": "1.50",
        "hoursPerSemester": 60,
        "subjectType": "BASIC",
        "groupId": null,
        "description": null,
        "effectiveFrom": "2027-05-01",
        "effectiveUntil": null,
        "termCode": "T1",
        "periodsPerWeek": 3,
        "gradeLevelIds": [],
        "rowVersion": 2
    });
    let request: UpdateSubjectVersionRequest = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(request.row_version, 2);

    let mut unknown = payload;
    unknown["legacyTerm"] = json!("1");
    assert!(serde_json::from_value::<UpdateSubjectVersionRequest>(unknown).is_err());
}

#[tokio::test]
async fn context_options_are_read_only_and_keep_active_state_unchanged() {
    let pool = prepare_core_fixture("academic_core_context_read_only").await;
    let audit_before: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_audit_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let active_before: (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT year.id, term.id
           FROM academic_years year
           JOIN academic_terms term ON term.academic_year_id = year.id
           WHERE year.status = 'active' AND term.status = 'active'"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let options = context::list_options(&pool).await.unwrap();

    let audit_after: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_audit_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let active_after: (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT year.id, term.id
           FROM academic_years year
           JOIN academic_terms term ON term.academic_year_id = year.id
           WHERE year.status = 'active' AND term.status = 'active'"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_after, audit_before);
    assert_eq!(active_after, active_before);
    assert_eq!(options.active_academic_year_id, Some(active_before.0));
    assert_eq!(options.active_academic_term_id, Some(active_before.1));
    assert_eq!(options.years.len(), 4);
    assert_eq!(options.terms.len(), 9);
}

#[tokio::test]
async fn planning_year_and_term_updates_reject_stale_versions_and_unused_term_deletes() {
    let pool = prepare_core_fixture("academic_core_year_term_mutations").await;
    let actor = fixture_actor(&pool).await;
    let future = years_terms::get_year(&pool, FUTURE_YEAR_ID).await.unwrap();
    let update_year = UpdateAcademicYearRequest {
        year: future.year,
        name: "ปีการศึกษา 2026 เตรียมการ".to_string(),
        start_date: future.start_date,
        end_date: future.end_date,
        school_days: future.school_days,
        row_version: future.row_version,
    };
    let updated = years_terms::update_year(&pool, actor, FUTURE_YEAR_ID, update_year)
        .await
        .unwrap();
    assert_eq!(updated.row_version, future.row_version + 1);
    let stale = years_terms::update_year(
        &pool,
        actor,
        FUTURE_YEAR_ID,
        UpdateAcademicYearRequest {
            year: future.year,
            name: "ข้อมูลเก่า".to_string(),
            start_date: future.start_date,
            end_date: future.end_date,
            school_days: vec!["MON".to_string()],
            row_version: future.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(stale.public_message().contains("ถูกแก้ไขแล้ว"));

    let bell_schedule_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM bell_schedules WHERE academic_year_id = $1 AND is_default",
    )
    .bind(FUTURE_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let created = years_terms::create_term(
        &pool,
        actor,
        CreateAcademicTermRequest {
            academic_year_id: FUTURE_YEAR_ID,
            sequence: 3,
            code: "REMEDIAL".to_string(),
            name: "ภาคซ่อมเสริม".to_string(),
            term_type: AcademicTermType::Remedial,
            start_date: NaiveDate::from_ymd_opt(2027, 4, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2027, 4, 15).unwrap(),
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id,
        },
    )
    .await
    .unwrap();
    let update_term = UpdateAcademicTermRequest {
        sequence: created.sequence,
        code: created.code.clone(),
        name: "ภาคซ่อมเสริมปรับปรุง".to_string(),
        term_type: created.term_type,
        start_date: created.start_date,
        end_date: created.end_date,
        included_in_year_result: created.included_in_year_result,
        blocks_year_closure: created.blocks_year_closure,
        bell_schedule_id: created.bell_schedule_id,
        row_version: created.row_version,
    };
    let updated_term = years_terms::update_term(&pool, actor, created.id, update_term)
        .await
        .unwrap();
    assert_eq!(updated_term.row_version, created.row_version + 1);
    let stale_term = years_terms::update_term(
        &pool,
        actor,
        created.id,
        UpdateAcademicTermRequest {
            sequence: created.sequence,
            code: created.code,
            name: "ข้อมูลเก่า".to_string(),
            term_type: created.term_type,
            start_date: created.start_date,
            end_date: created.end_date,
            included_in_year_result: created.included_in_year_result,
            blocks_year_closure: created.blocks_year_closure,
            bell_schedule_id: created.bell_schedule_id,
            row_version: created.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(stale_term.public_message().contains("ถูกแก้ไขแล้ว"));

    years_terms::delete_term(&pool, actor, created.id)
        .await
        .expect("an unused planning term must remain deletable after its audit events exist");
    assert!(years_terms::get_term(&pool, created.id).await.is_err());
    let durable_audit: i64 =
        sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id = $1")
            .bind(created.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(durable_audit, 3);
}

#[tokio::test]
async fn grade_progression_replacement_uses_one_optimistic_set_revision() {
    let pool = prepare_core_fixture("academic_core_progression_revision").await;
    let actor = fixture_actor(&pool).await;
    let before = progressions::list(&pool).await.unwrap();
    let after = progressions::replace(
        &pool,
        actor,
        ReplaceGradeProgressionsRequest {
            progressions: Vec::new(),
            row_version: before.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(after.row_version, before.row_version + 1);
    assert!(after.progressions.is_empty());

    let stale = progressions::replace(
        &pool,
        actor,
        ReplaceGradeProgressionsRequest {
            progressions: Vec::new(),
            row_version: before.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(stale.public_message().contains("ผู้ใช้อื่น"));
}

#[tokio::test]
async fn bell_schedule_period_replacement_is_atomic_and_rejects_stale_revisions() {
    let pool = prepare_core_fixture("academic_core_bell_schedule_runtime").await;
    let actor = fixture_actor(&pool).await;
    let schedule = bell_schedules::create(
        &pool,
        actor,
        CreateBellScheduleRequest {
            academic_year_id: FUTURE_YEAR_ID,
            code: "ALT".to_string(),
            name: "ตารางคาบทางเลือก".to_string(),
            is_default: false,
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let request = ReplaceBellSchedulePeriodsRequest {
        periods: vec![
            BellSchedulePeriodInput {
                name: Some("คาบ 1".to_string()),
                start_time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
                order_index: 1,
                applicable_days: vec!["MON".to_string(), "TUE".to_string()],
                is_active: true,
            },
            BellSchedulePeriodInput {
                name: Some("คาบ 2".to_string()),
                start_time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(10, 10, 0).unwrap(),
                order_index: 2,
                applicable_days: vec!["MON".to_string(), "TUE".to_string()],
                is_active: true,
            },
        ],
        row_version: schedule.row_version,
    };

    let periods = bell_schedules::replace_periods(&pool, actor, schedule.id, request.clone())
        .await
        .unwrap();
    assert_eq!(periods.len(), 2);
    assert_eq!(periods[0].applicable_days.as_deref(), Some("MON,TUE"));
    let current = bell_schedules::get(&pool, schedule.id).await.unwrap();
    assert_eq!(current.row_version, schedule.row_version + 1);
    let legacy_year_ids: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM bell_schedule_periods WHERE bell_schedule_id = $1 AND academic_year_id IS NOT NULL",
    )
    .bind(schedule.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy_year_ids, 0);

    let stale = bell_schedules::replace_periods(&pool, actor, schedule.id, request)
        .await
        .unwrap_err();
    assert!(stale.public_message().contains("ผู้ใช้อื่น"));
    assert_eq!(
        bell_schedules::list_periods(&pool, schedule.id)
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn subject_group_updates_use_optimistic_revisions() {
    let pool = prepare_core_fixture("academic_core_subject_group_revision").await;
    let group = catalog::create_subject_group(
        &pool,
        CreateSubjectGroupRequest {
            code: "TEST-GROUP".to_string(),
            name_th: "กลุ่มสาระทดสอบ".to_string(),
            name_en: "Test Group".to_string(),
            display_order: 99,
            is_active: true,
        },
    )
    .await
    .unwrap();
    let update = UpdateSubjectGroupRequest {
        code: group.code.clone(),
        name_th: "กลุ่มสาระทดสอบปรับปรุง".to_string(),
        name_en: group.name_en.clone(),
        display_order: 99,
        is_active: true,
        row_version: group.row_version,
    };
    let updated = catalog::update_subject_group(&pool, group.id, update.clone())
        .await
        .unwrap();
    assert_eq!(updated.row_version, group.row_version + 1);

    let stale = catalog::update_subject_group(&pool, group.id, update)
        .await
        .unwrap_err();
    assert!(stale.public_message().contains("ผู้ใช้อื่น"));
}

#[tokio::test]
async fn future_student_year_and_transfer_do_not_mutate_current_year_and_retries_are_idempotent() {
    let pool = prepare_core_fixture("academic_core_student_year_transfer").await;
    let actor = fixture_actor(&pool).await;
    let existing_context: (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT student_year.grade_level_id, student_year.study_program_id, placement.homeroom_id
           FROM student_academic_years student_year
           JOIN homeroom_placements placement
             ON placement.student_academic_year_id = student_year.id
            AND placement.status = 'current'
           WHERE student_year.academic_year_id = $1
           ORDER BY student_year.student_id
           LIMIT 1"#,
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let student_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name, user_type, status
           ) VALUES ($1, $2, $2, 'fixture-not-a-login', 'นักเรียน', 'ทดสอบ', 'student', 'active')"#,
    )
    .bind(student_id)
    .bind(format!("future-student-{student_id}@example.invalid"))
    .execute(&pool)
    .await
    .unwrap();
    let current_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO student_academic_years (
               id, student_id, academic_year_id, grade_level_id, study_program_id, status
           ) VALUES ($1, $2, $3, $4, $5, 'active')"#,
    )
    .bind(current_id)
    .bind(student_id)
    .bind(CURRENT_YEAR_ID)
    .bind(existing_context.0)
    .bind(existing_context.1)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO homeroom_placements (
               id, student_academic_year_id, academic_year_id, homeroom_id,
               start_date, status, enrollment_type, class_number
           ) VALUES ($1, $2, $3, $4, '2025-05-01', 'current', 'regular', 99)"#,
    )
    .bind(Uuid::new_v4())
    .bind(current_id)
    .bind(CURRENT_YEAR_ID)
    .bind(existing_context.2)
    .execute(&pool)
    .await
    .unwrap();
    let current = (
        current_id,
        student_id,
        existing_context.0,
        existing_context.1,
        1_i64,
    );
    let current_placement_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM homeroom_placements WHERE student_academic_year_id = $1",
    )
    .bind(current.0)
    .fetch_one(&pool)
    .await
    .unwrap();

    let future = student_years::create_student_year(
        &pool,
        actor,
        CreateStudentAcademicYearRequest {
            academic_year_id: FUTURE_YEAR_ID,
            student_id: current.1,
            grade_level_id: current.2,
            study_program_id: current.3,
        },
    )
    .await
    .unwrap();
    let current_after: i64 =
        sqlx::query_scalar("SELECT row_version FROM student_academic_years WHERE id = $1")
            .bind(current.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    let current_placements_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM homeroom_placements WHERE student_academic_year_id = $1",
    )
    .bind(current.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(current_after, current.4);
    assert_eq!(current_placements_after, current_placement_count);

    let homeroom_a = student_years::create_homeroom(
        &pool,
        CreateHomeroomRequest {
            academic_year_id: FUTURE_YEAR_ID,
            code: "FUTURE-A".to_string(),
            name: "ห้องอนาคต A".to_string(),
            grade_level_id: current.2,
            room_number: Some("A".to_string()),
            study_program_id: current.3,
            capacity: 40,
        },
    )
    .await
    .unwrap();
    let homeroom_b = student_years::create_homeroom(
        &pool,
        CreateHomeroomRequest {
            academic_year_id: FUTURE_YEAR_ID,
            code: "FUTURE-B".to_string(),
            name: "ห้องอนาคต B".to_string(),
            grade_level_id: current.2,
            room_number: Some("B".to_string()),
            study_program_id: current.3,
            capacity: 40,
        },
    )
    .await
    .unwrap();
    let wrong_year_homeroom: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id = $1 ORDER BY id LIMIT 1",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let wrong_year = student_years::create_placement(
        &pool,
        actor,
        future.id,
        CreateHomeroomPlacementRequest {
            homeroom_id: wrong_year_homeroom,
            start_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            status: HomeroomPlacementStatus::Planned,
            enrollment_type: "promotion".to_string(),
            class_number: None,
            row_version: future.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(wrong_year.public_message().contains("ปี"));

    let placement = student_years::create_placement(
        &pool,
        actor,
        future.id,
        CreateHomeroomPlacementRequest {
            homeroom_id: homeroom_a.id,
            start_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            status: HomeroomPlacementStatus::Current,
            enrollment_type: "promotion".to_string(),
            class_number: Some(1),
            row_version: future.row_version,
        },
    )
    .await
    .unwrap();
    let idempotency_key = Uuid::new_v4();
    let blank_reason = student_years::transfer_placement(
        &pool,
        actor,
        placement.id,
        TransferHomeroomPlacementRequest {
            target_homeroom_id: homeroom_b.id,
            transfer_date: NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            enrollment_type: "room_transfer".to_string(),
            class_number: Some(2),
            reason: "  ".to_string(),
            row_version: placement.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap_err();
    assert!(blank_reason.public_message().contains("เหตุผล"));

    let transfer_request = TransferHomeroomPlacementRequest {
        target_homeroom_id: homeroom_b.id,
        transfer_date: NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        enrollment_type: "room_transfer".to_string(),
        class_number: Some(2),
        reason: "ปรับห้องให้เหมาะกับแผนการเรียน".to_string(),
        row_version: placement.row_version,
        idempotency_key,
    };
    let first =
        student_years::transfer_placement(&pool, actor, placement.id, transfer_request.clone())
            .await
            .unwrap();
    let replay = student_years::transfer_placement(&pool, actor, placement.id, transfer_request)
        .await
        .unwrap();
    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(first.new_placement.id, replay.new_placement.id);
    let placement_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM homeroom_placements WHERE student_academic_year_id = $1",
    )
    .bind(future.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(placement_count, 2);

    let history = student_years::list_placements(&pool, future.id)
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].id, first.ended_placement.id);
    assert_eq!(history[1].id, first.new_placement.id);

    let audit_reason: String = sqlx::query_scalar(
        "SELECT payload->>'reason' FROM academic_audit_events \
         WHERE event_code = 'homeroom_placement.transferred' AND entity_id = $1",
    )
    .bind(first.ended_placement.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_reason, "ปรับห้องให้เหมาะกับแผนการเรียน");
}

#[tokio::test]
async fn catalog_versions_round_trip_exact_values_and_published_rows_are_immutable() {
    let pool = prepare_core_fixture("academic_core_catalog_runtime").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject_group_id: Uuid =
        sqlx::query_scalar("SELECT id FROM subject_groups WHERE code = 'MA'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "TEST-EXACT".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let version = catalog::create_subject_version(
        &pool,
        subject.id,
        CreateSubjectVersionRequest {
            name_th: "วิชาทดสอบค่าทศนิยม".to_string(),
            name_en: Some("Exact Decimal Test".to_string()),
            credit: "1.50".to_string(),
            hours_per_semester: Some(60),
            subject_type: "BASIC".to_string(),
            group_id: Some(subject_group_id),
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: Some("T1".to_string()),
            periods_per_week: Some(3),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    assert_eq!(version.credit, "1.50");
    assert_eq!(version.periods_per_week, Some(3));
    assert_eq!(version.grade_level_ids, vec![grade_level_id]);

    let published = catalog::publish_subject_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(published.status, VersionStatus::Published);
    let immutable = catalog::update_subject_version(
        &pool,
        version.id,
        UpdateSubjectVersionRequest {
            name_th: published.name_th,
            name_en: published.name_en,
            credit: published.credit,
            hours_per_semester: published.hours_per_semester,
            subject_type: published.subject_type,
            group_id: published.group_id,
            description: published.description,
            effective_from: published.effective_from,
            effective_until: published.effective_until,
            term_code: published.term_code,
            periods_per_week: published.periods_per_week,
            grade_level_ids: published.grade_level_ids,
            row_version: published.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(immutable.public_message().contains("แก้ไขไม่ได้"));

    let archived = catalog::update_subject(
        &pool,
        subject.id,
        UpdateCatalogSubjectRequest {
            code: subject.code,
            owning_organization_unit_id: subject.owning_organization_unit_id,
            archived: true,
            row_version: subject.row_version,
        },
    )
    .await
    .unwrap();
    assert!(archived.archived_at.is_some());
}

#[tokio::test]
async fn activity_catalog_versions_round_trip_exact_hours_and_archive_stably() {
    let pool = prepare_core_fixture("academic_core_activity_catalog_runtime").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "TEST-GUIDANCE".to_string(),
            activity_type: "guidance".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let version = catalog::create_activity_version(
        &pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "กิจกรรมแนะแนวทดสอบ".to_string(),
            description: None,
            hours_per_week: "1.50".to_string(),
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: Some("T1".to_string()),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    assert_eq!(version.hours_per_week, "1.50");
    assert_eq!(version.grade_level_ids, vec![grade_level_id]);

    let published = catalog::publish_activity_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap();
    let immutable = catalog::update_activity_version(
        &pool,
        published.id,
        UpdateActivityVersionRequest {
            name: published.name,
            description: published.description,
            hours_per_week: published.hours_per_week,
            scheduling_mode: published.scheduling_mode,
            effective_from: published.effective_from,
            effective_until: published.effective_until,
            term_code: published.term_code,
            grade_level_ids: published.grade_level_ids,
            row_version: published.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(immutable.public_message().contains("เผยแพร่แล้ว"));

    let archived = catalog::update_activity(
        &pool,
        activity.id,
        UpdateCatalogActivityRequest {
            code: activity.code,
            activity_type: activity.activity_type,
            owning_organization_unit_id: activity.owning_organization_unit_id,
            archived: true,
            row_version: activity.row_version,
        },
    )
    .await
    .unwrap();
    assert!(archived.archived_at.is_some());
}

#[tokio::test]
async fn curriculum_version_supports_multiple_programs_and_freezes_them_on_publish() {
    let pool = prepare_core_fixture("academic_core_curriculum_runtime").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM subject_versions WHERE status = 'published' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            code: "TEST-MULTI".to_string(),
            name_th: "หลักสูตรทดสอบหลายแผน".to_string(),
            name_en: Some("Multiple Program Test".to_string()),
            description: None,
            grade_level_ids: vec![grade_level_id],
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_version(
        &pool,
        curriculum_row.id,
        CreateCurriculumVersionRequest {
            version_name: "ฉบับ 2026".to_string(),
            start_academic_year_id: FUTURE_YEAR_ID,
            end_academic_year_id: None,
            description: None,
        },
    )
    .await
    .unwrap();
    let default_program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            code: "GENERAL".to_string(),
            name_th: "แผนทั่วไป".to_string(),
            name_en: Some("General".to_string()),
            is_default: true,
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let science_program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            code: "SCI-MATH".to_string(),
            name_th: "แผนวิทย์-คณิต".to_string(),
            name_en: Some("Science Mathematics".to_string()),
            is_default: false,
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        curriculum::list_programs(&pool, version.id)
            .await
            .unwrap()
            .len(),
        2
    );

    for program in [&default_program, &science_program] {
        let requirements = curriculum::replace_requirements(
            &pool,
            program.id,
            ReplaceProgramRequirementsRequest {
                requirements: vec![ProgramRequirementInput {
                    resource_kind: RequirementResourceKind::Course,
                    catalog_version_id: subject_version_id,
                    grade_level_id,
                    recommended_term_code: Some("T1".to_string()),
                    requirement_kind: RequirementKind::Required,
                    credit: Some("1.50".to_string()),
                    hours: Some("60.00".to_string()),
                    display_order: 1,
                }],
                row_version: program.row_version,
            },
        )
        .await
        .unwrap();
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].credit.as_deref(), Some("1.50"));
        assert_eq!(requirements[0].hours.as_deref(), Some("60.00"));
    }

    let published = curriculum::publish_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(published.status, VersionStatus::Published);
    let frozen_program = curriculum::get_program(&pool, science_program.id)
        .await
        .unwrap();
    let immutable = curriculum::update_program(
        &pool,
        frozen_program.id,
        UpdateStudyProgramRequest {
            code: frozen_program.code,
            name_th: frozen_program.name_th,
            name_en: frozen_program.name_en,
            is_default: frozen_program.is_default,
            owning_organization_unit_id: frozen_program.owning_organization_unit_id,
            row_version: frozen_program.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(immutable.public_message().contains("แก้ไขไม่ได้"));
}
