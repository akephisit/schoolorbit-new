use super::models::{
    AcademicTermStatus, AcademicTermType, AcademicYearStatus, BellSchedulePeriodInput,
    CatalogDisplayState, CreateAcademicTermRequest, CreateActivityVersionRequest,
    CreateBellScheduleRequest, CreateCatalogActivityRequest, CreateCatalogSubjectRequest,
    CreateCurriculumRequest, CreateCurriculumVersionRequest, CreateHomeroomPlacementRequest,
    CreateHomeroomRequest, CreateStudentAcademicYearRequest, CreateStudyProgramRequest,
    CreateSubjectGroupRequest, CreateSubjectVersionRequest, CurriculumDisplayState,
    CurriculumStructureRequirementInput, CurriculumTermSlotInput, HomeroomPlacementStatus,
    PublishVersionRequest, ReplaceBellSchedulePeriodsRequest, ReplaceCurriculumStructureRequest,
    ReplaceCurriculumTermSlotsRequest, ReplaceGradeProgressionsRequest, RequirementKind,
    RequirementResourceKind, StudentAcademicYearFilter, StudentYearCandidateQuery,
    TransferHomeroomPlacementRequest, UpdateAcademicTermRequest, UpdateAcademicYearRequest,
    UpdateActivityVersionRequest, UpdateCatalogActivityRequest, UpdateCatalogSubjectRequest,
    UpdateStudyProgramRequest, UpdateSubjectGroupRequest, UpdateSubjectVersionRequest,
    VersionStatus,
};
use super::services::{
    bell_schedules, catalog, context, curriculum, curriculum_structure, ensure_draft_version,
    ensure_planning_delete, parse_row_version, progressions, student_years,
    validate_canonical_decimal, validate_date_containment, workspaces, years_terms,
};
use crate::policies::resource_access_policy::AcademicResourceListFilter;
use crate::{
    middleware::permission::ActorContext,
    modules::academic::cutover_test_support::{
        apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
        CutoverFixture,
    },
    permissions::registry::codes,
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
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 48).await.unwrap();
    pool
}

async fn fixture_actor(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT id FROM users WHERE user_type = 'staff' ORDER BY id LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn create_published_program_option_fixture(
    pool: &PgPool,
    owner_id: Uuid,
    start_academic_year_id: Uuid,
    end_academic_year_id: Option<Uuid>,
    code: &str,
) -> (Uuid, Uuid) {
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(pool)
            .await
            .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM subject_versions WHERE status = 'published' ORDER BY id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        pool,
        CreateCurriculumRequest {
            code: format!("OPTION-{code}"),
            name_th: format!("หลักสูตรตัวเลือก {code}"),
            name_en: None,
            description: None,
            grade_level_ids: vec![grade_level_id],
            owning_organization_unit_id: Some(owner_id),
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_version(
        pool,
        curriculum_row.id,
        CreateCurriculumVersionRequest {
            version_name: format!("ฉบับ {code}"),
            start_academic_year_id,
            end_academic_year_id,
            description: None,
        },
    )
    .await
    .unwrap();
    let program = curriculum::create_program(
        pool,
        version.id,
        CreateStudyProgramRequest {
            code: format!("PROGRAM-{code}"),
            name_th: format!("แผนการเรียน {code}"),
            name_en: None,
            is_default: true,
            owning_organization_unit_id: Some(owner_id),
        },
    )
    .await
    .unwrap();
    let mut workspace = curriculum_structure::get_workspace(pool, version.id)
        .await
        .unwrap();
    if workspace.term_slots.is_empty() {
        workspace = curriculum_structure::replace_term_slots(
            pool,
            version.id,
            ReplaceCurriculumTermSlotsRequest {
                slots: vec![CurriculumTermSlotInput {
                    id: None,
                    sequence: 1,
                    term_type: AcademicTermType::Regular,
                    type_occurrence: 1,
                    name: "ภาคเรียนที่ 1".to_string(),
                }],
                row_version: workspace.row_version,
            },
        )
        .await
        .unwrap();
    }
    let term_slot_id = workspace.term_slots[0].id;
    curriculum_structure::replace_program_structure(
        pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Course,
                catalog_version_id: subject_version_id,
                grade_level_id,
                term_slot_id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
            row_version: program.row_version,
        },
    )
    .await
    .unwrap();
    curriculum::publish_version(
        pool,
        version.id,
        PublishVersionRequest {
            row_version: workspace.row_version,
        },
    )
    .await
    .unwrap();
    (curriculum_row.id, program.id)
}

async fn create_curriculum_overview_fixture(
    pool: &PgPool,
    owner_id: Uuid,
    grade_level_id: Uuid,
    subject_version_id: Uuid,
    code: &str,
    start_academic_year_id: Uuid,
    end_academic_year_id: Option<Uuid>,
    publish: bool,
    program_count: usize,
) -> (Uuid, Uuid) {
    let curriculum_row = curriculum::create(
        pool,
        CreateCurriculumRequest {
            code: code.to_string(),
            name_th: format!("หลักสูตร {code}"),
            name_en: None,
            description: None,
            grade_level_ids: vec![grade_level_id],
            owning_organization_unit_id: Some(owner_id),
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_version(
        pool,
        curriculum_row.id,
        CreateCurriculumVersionRequest {
            version_name: format!("ฉบับ {code}"),
            start_academic_year_id,
            end_academic_year_id,
            description: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(pool)
    .await
    .unwrap();
    let mut workspace = curriculum_structure::get_workspace(pool, version.id)
        .await
        .unwrap();
    if workspace.term_slots.is_empty() {
        workspace = curriculum_structure::replace_term_slots(
            pool,
            version.id,
            ReplaceCurriculumTermSlotsRequest {
                slots: vec![CurriculumTermSlotInput {
                    id: None,
                    sequence: 1,
                    term_type: AcademicTermType::Regular,
                    type_occurrence: 1,
                    name: "ภาคเรียนที่ 1".to_string(),
                }],
                row_version: workspace.row_version,
            },
        )
        .await
        .unwrap();
    }
    let term_slot_id = workspace.term_slots[0].id;
    for index in 0..program_count {
        let program = curriculum::create_program(
            pool,
            version.id,
            CreateStudyProgramRequest {
                code: format!("{code}-P{}", index + 1),
                name_th: format!("แผน {}", index + 1),
                name_en: None,
                is_default: index == 0,
                owning_organization_unit_id: Some(owner_id),
            },
        )
        .await
        .unwrap();
        curriculum_structure::replace_program_structure(
            pool,
            program.id,
            ReplaceCurriculumStructureRequest {
                requirements: vec![CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Course,
                    catalog_version_id: subject_version_id,
                    grade_level_id,
                    term_slot_id,
                    requirement_kind: RequirementKind::Required,
                    display_order: 1,
                }],
                row_version: program.row_version,
            },
        )
        .await
        .unwrap();
    }
    if publish {
        curriculum::publish_version(
            pool,
            version.id,
            PublishVersionRequest {
                row_version: workspace.row_version,
            },
        )
        .await
        .unwrap();
    }
    (curriculum_row.id, version.id)
}

#[tokio::test]
async fn curriculum_structure_workspace_reads_catalog_metrics_and_dynamic_term_slots() {
    let pool = prepare_core_fixture("academic_core_structure_workspace").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        r#"SELECT id
           FROM subject_versions
           WHERE status = 'published'
             AND periods_per_week IS NOT NULL
             AND hours_per_semester IS NOT NULL
           ORDER BY version_no DESC, id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2)
           ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            code: "STRUCTURE".to_string(),
            name_th: "หลักสูตรโครงสร้าง".to_string(),
            name_en: None,
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
            version_name: "ฉบับโครงสร้าง".to_string(),
            start_academic_year_id: CURRENT_YEAR_ID,
            end_academic_year_id: None,
            description: None,
        },
    )
    .await
    .unwrap();
    let program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            code: "GENERAL".to_string(),
            name_th: "แผนทั่วไป".to_string(),
            name_en: None,
            is_default: true,
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();

    let empty_workspace = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    let first_regular_slot = empty_workspace
        .term_slots
        .iter()
        .find(|slot| slot.term_type == AcademicTermType::Regular && slot.type_occurrence == 1)
        .expect("the start academic year must seed its first regular curriculum slot");

    let workspace = curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Course,
                catalog_version_id: subject_version_id,
                grade_level_id,
                term_slot_id: first_regular_slot.id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
            row_version: program.row_version,
        },
    )
    .await
    .unwrap();

    assert!(workspace.term_slots.len() >= 2);
    assert_eq!(workspace.programs.len(), 1);
    assert_eq!(workspace.requirements.len(), 1);
    assert_eq!(
        workspace.requirements[0].metrics.credit.as_deref(),
        Some("1.50")
    );
    assert_eq!(
        workspace.requirements[0].metrics.total_hours.as_deref(),
        Some("60")
    );
    assert!(workspace.validation.blockers.is_empty());
}

#[tokio::test]
async fn curriculum_term_slots_are_draft_only_and_cannot_remove_a_referenced_slot() {
    let pool = prepare_core_fixture("academic_core_term_slot_replace").await;
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
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            code: "TERM-SLOTS".to_string(),
            name_th: "หลักสูตรภาคเรียนยืดหยุ่น".to_string(),
            name_en: None,
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
            version_name: "ฉบับภาคเรียนยืดหยุ่น".to_string(),
            start_academic_year_id: CURRENT_YEAR_ID,
            end_academic_year_id: None,
            description: None,
        },
    )
    .await
    .unwrap();
    let initial = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    let first_slot = initial.term_slots[0].clone();
    let mut slot_inputs = initial
        .term_slots
        .iter()
        .map(|slot| CurriculumTermSlotInput {
            id: Some(slot.id),
            sequence: slot.sequence,
            term_type: slot.term_type,
            type_occurrence: slot.type_occurrence,
            name: slot.name.clone(),
        })
        .collect::<Vec<_>>();
    slot_inputs.push(CurriculumTermSlotInput {
        id: None,
        sequence: slot_inputs.len() as i32 + 1,
        term_type: AcademicTermType::Custom,
        type_occurrence: 1,
        name: "ภาคเรียนโครงงาน".to_string(),
    });
    let with_custom = curriculum_structure::replace_term_slots(
        &pool,
        version.id,
        ReplaceCurriculumTermSlotsRequest {
            slots: slot_inputs,
            row_version: initial.row_version,
        },
    )
    .await
    .unwrap();
    assert!(with_custom
        .term_slots
        .iter()
        .any(|slot| slot.name == "ภาคเรียนโครงงาน"));

    let program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            code: "GENERAL".to_string(),
            name_th: "แผนทั่วไป".to_string(),
            name_en: None,
            is_default: true,
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Course,
                catalog_version_id: subject_version_id,
                grade_level_id,
                term_slot_id: first_slot.id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
            row_version: program.row_version,
        },
    )
    .await
    .unwrap();

    let without_referenced = with_custom
        .term_slots
        .iter()
        .filter(|slot| slot.id != first_slot.id)
        .map(|slot| CurriculumTermSlotInput {
            id: Some(slot.id),
            sequence: slot.sequence,
            term_type: slot.term_type,
            type_occurrence: slot.type_occurrence,
            name: slot.name.clone(),
        })
        .collect();
    let removal = curriculum_structure::replace_term_slots(
        &pool,
        version.id,
        ReplaceCurriculumTermSlotsRequest {
            slots: without_referenced,
            row_version: with_custom.row_version,
        },
    )
    .await;
    assert!(matches!(removal, Err(crate::error::AppError::Conflict(_))));
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
fn academic_foundation_identity_derives_standard_and_custom_labels() {
    assert_eq!(
        years_terms::derive_academic_year_name(2571, None).unwrap(),
        "ปีการศึกษา 2571"
    );
    assert_eq!(
        years_terms::derive_academic_year_name(2571, Some("ปีแห่งการอ่าน")).unwrap(),
        "ปีแห่งการอ่าน"
    );
    assert!(years_terms::derive_academic_year_name(2571, Some("   ")).is_err());

    let regular = years_terms::derive_term_identity(AcademicTermType::Regular, 2, None).unwrap();
    assert_eq!(regular.code, "2");
    assert_eq!(regular.name, "ภาคเรียนที่ 2");

    let summer =
        years_terms::derive_term_identity(AcademicTermType::Summer, 3, Some("ภาคฤดูร้อนเพิ่มเติม"))
            .unwrap();
    assert_eq!(summer.code, "SUMMER");
    assert_eq!(summer.name, "ภาคฤดูร้อนเพิ่มเติม");
}

#[test]
fn academic_foundation_period_overlap_is_scoped_to_shared_school_days() {
    let period =
        |order_index, start_hour, end_hour, days: &[&str], is_active| BellSchedulePeriodInput {
            name: Some(format!("คาบ {order_index}")),
            start_time: NaiveTime::from_hms_opt(start_hour, 0, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(end_hour, 0, 0).unwrap(),
            order_index,
            applicable_days: days.iter().map(|day| (*day).to_string()).collect(),
            is_active,
        };

    assert!(bell_schedules::validate_periods(&[
        period(1, 8, 10, &["MON"], true),
        period(2, 9, 11, &["TUE"], true),
    ])
    .is_ok());
    assert!(bell_schedules::validate_periods(&[
        period(1, 8, 10, &["MON"], true),
        period(2, 9, 11, &["MON"], true),
    ])
    .is_err());
    assert!(bell_schedules::validate_periods(&[
        period(1, 8, 10, &["MON"], true),
        period(2, 9, 11, &["MON"], false),
    ])
    .is_ok());
    assert!(bell_schedules::validate_periods(&[period(1, 8, 10, &["MON", "MON"], true,)]).is_err());
    assert!(bell_schedules::validate_periods(&[period(1, 8, 10, &["HOLIDAY"], true)]).is_err());
}

#[test]
fn academic_foundation_homeroom_identity_uses_grade_and_room_number() {
    let secondary = student_years::derive_homeroom_identity("secondary", 1, "3", None).unwrap();
    assert_eq!(secondary.code, "M1-3");
    assert_eq!(secondary.name, "ม.1/3");

    let primary =
        student_years::derive_homeroom_identity("primary", 2, " 1 ", Some("ห้องส่งเสริมวิทยาศาสตร์"))
            .unwrap();
    assert_eq!(primary.code, "P2-1");
    assert_eq!(primary.name, "ห้องส่งเสริมวิทยาศาสตร์");

    assert!(student_years::derive_homeroom_identity("secondary", 1, " ", None).is_err());
    assert!(student_years::derive_homeroom_identity("other", 1, "1", None).is_err());
    assert!(student_years::derive_homeroom_identity("primary", 1, "1", Some("   "),).is_err());
}

#[test]
fn request_dtos_use_camel_case_and_reject_unknown_fields() {
    let payload = json!({
        "year": 2570,
        "customName": "ปีแห่งการอ่าน",
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
    });
    assert!(serde_json::from_value::<UpdateAcademicYearRequest>(unknown).is_err());

    let term = json!({
        "academicYearId": Uuid::new_v4(),
        "termType": "regular",
        "customName": null,
        "startDate": "2027-05-01",
        "endDate": "2027-09-30",
        "includedInYearResult": true,
        "blocksYearClosure": true,
        "bellScheduleId": Uuid::new_v4()
    });
    assert!(serde_json::from_value::<CreateAcademicTermRequest>(term).is_ok());

    let schedule = json!({
        "academicYearId": Uuid::new_v4(),
        "name": "ตารางเวลาปกติ",
        "owningOrganizationUnitId": null
    });
    assert!(serde_json::from_value::<CreateBellScheduleRequest>(schedule).is_ok());

    let homeroom = json!({
        "academicYearId": Uuid::new_v4(),
        "customName": null,
        "gradeLevelId": Uuid::new_v4(),
        "roomNumber": "1",
        "studyProgramId": Uuid::new_v4(),
        "capacity": 30
    });
    assert!(serde_json::from_value::<CreateHomeroomRequest>(homeroom).is_ok());

    let legacy_homeroom = json!({
        "academicYearId": Uuid::new_v4(),
        "code": "M1-1",
        "name": "ม.1/1",
        "gradeLevelId": Uuid::new_v4(),
        "roomNumber": "1",
        "studyProgramId": Uuid::new_v4(),
        "capacity": 30
    });
    assert!(serde_json::from_value::<CreateHomeroomRequest>(legacy_homeroom).is_err());
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
        custom_name: Some("ปีการศึกษา 2026 เตรียมการ".to_string()),
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
            custom_name: Some("ข้อมูลเก่า".to_string()),
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
            term_type: AcademicTermType::Remedial,
            custom_name: None,
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
        term_type: AcademicTermType::Custom,
        custom_name: Some("ภาคซ่อมเสริมปรับปรุง".to_string()),
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
    assert_eq!(updated_term.term_type, AcademicTermType::Custom);
    assert_eq!(updated_term.code, created.code);
    let stale_term = years_terms::update_term(
        &pool,
        actor,
        created.id,
        UpdateAcademicTermRequest {
            term_type: created.term_type,
            custom_name: Some("ข้อมูลเก่า".to_string()),
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
async fn student_year_read_models_are_human_readable() {
    let pool = prepare_core_fixture("academic_core_student_year_read_model").await;
    let records = student_years::list_student_years(
        &pool,
        StudentAcademicYearFilter {
            academic_year_id: CURRENT_YEAR_ID,
            student_id: None,
            grade_level_id: None,
            study_program_id: None,
            homeroom_id: None,
            status: None,
        },
    )
    .await
    .unwrap();
    let record = records.first().expect("fixture student-year record");
    assert!(!record.student_name.is_empty());
    assert!(!record.grade_level_name.is_empty());
    assert!(!record.study_program_name.is_empty());
    assert_ne!(record.student_name, record.student_id.to_string());
}

#[tokio::test]
async fn student_year_candidates_include_only_students_missing_the_target_year() {
    let pool = prepare_core_fixture("academic_core_student_year_candidates").await;
    let candidate_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, first_name, last_name, user_type, status) \
         VALUES ($1, $2, $3, $4, $5, 'student', 'active')",
    )
    .bind(candidate_id)
    .bind(format!("candidate-{candidate_id}"))
    .bind("test-password-hash")
    .bind("พร้อม")
    .bind("เรียน")
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO student_info (user_id, student_id) VALUES ($1, '67099')")
        .bind(candidate_id)
        .execute(&pool)
        .await
        .unwrap();

    let query = StudentYearCandidateQuery {
        academic_year_id: FUTURE_YEAR_ID,
        search: Some("67099".to_string()),
        limit: Some(10),
    };
    let candidates = student_years::list_student_year_candidates(&pool, query.clone())
        .await
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].id, candidate_id);
    assert_eq!(candidates[0].student_code.as_deref(), Some("67099"));
    assert_eq!(candidates[0].name, "พร้อม เรียน");

    let (grade_level_id, study_program_id): (Uuid, Uuid) = sqlx::query_as(
        r#"
        SELECT grade.id, program.id
        FROM grade_levels grade
        CROSS JOIN study_programs program
        JOIN curriculum_versions version ON version.id = program.curriculum_version_id
        JOIN academic_years starts ON starts.id = version.start_academic_year_id
        JOIN academic_years target ON target.id = $1
        LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
        WHERE starts.start_date <= target.start_date
          AND (ends.end_date IS NULL OR ends.end_date >= target.end_date)
        ORDER BY grade.level_type, grade.year, program.id
        LIMIT 1
        "#,
    )
    .bind(FUTURE_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    student_years::create_student_year(
        &pool,
        fixture_actor(&pool).await,
        CreateStudentAcademicYearRequest {
            academic_year_id: FUTURE_YEAR_ID,
            student_id: candidate_id,
            grade_level_id,
            study_program_id,
        },
    )
    .await
    .unwrap();

    let after = student_years::list_student_year_candidates(&pool, query)
        .await
        .unwrap();
    assert!(after.is_empty());
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
            name: "ตารางคาบทางเลือก".to_string(),
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
            custom_name: Some("ห้องอนาคต A".to_string()),
            grade_level_id: current.2,
            room_number: "A".to_string(),
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
            custom_name: Some("ห้องอนาคต B".to_string()),
            grade_level_id: current.2,
            room_number: "B".to_string(),
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
async fn year_relationship_collections_do_not_leak_across_years() {
    let pool = prepare_core_fixture("academic_core_year_relationship_collections").await;
    let (grade_level_id, study_program_id, current_homeroom_id): (Uuid, Uuid, Uuid) =
        sqlx::query_as(
            "SELECT grade_level_id, study_program_id, id FROM homerooms \
             WHERE academic_year_id = $1 ORDER BY id LIMIT 1",
        )
        .bind(CURRENT_YEAR_ID)
        .fetch_one(&pool)
        .await
        .unwrap();
    let future_homeroom_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id = $1 ORDER BY id LIMIT 1",
    )
    .bind(FUTURE_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let staff_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type = 'staff' AND status = 'active' ORDER BY id LIMIT 2",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(staff_ids.len(), 2, "fixture must provide two active staff");

    let second_current_homeroom_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO homerooms (
               id, code, name, academic_year_id, grade_level_id, room_number,
               study_program_id, capacity, is_active
           ) VALUES ($1, 'BATCH-ADVISOR', 'ห้องทดสอบครูที่ปรึกษา', $2, $3, 'BATCH', $4, 40, true)"#,
    )
    .bind(second_current_homeroom_id)
    .bind(CURRENT_YEAR_ID)
    .bind(grade_level_id)
    .bind(study_program_id)
    .execute(&pool)
    .await
    .unwrap();
    let advisor_ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    sqlx::query(
        r#"INSERT INTO homeroom_advisors (id, homeroom_id, user_id, role)
           VALUES ($1, $2, $3, 'primary'),
                  ($4, $5, $6, 'secondary'),
                  ($7, $8, $3, 'primary')"#,
    )
    .bind(advisor_ids[0])
    .bind(current_homeroom_id)
    .bind(staff_ids[0])
    .bind(advisor_ids[1])
    .bind(second_current_homeroom_id)
    .bind(staff_ids[1])
    .bind(advisor_ids[2])
    .bind(future_homeroom_id)
    .execute(&pool)
    .await
    .unwrap();

    let student_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name, user_type, status
           ) VALUES ($1, $2, $2, 'fixture-not-a-login', 'นักเรียน', 'ชุดข้อมูล', 'student', 'active')"#,
    )
    .bind(student_id)
    .bind(format!("batch-year-{student_id}@example.invalid"))
    .execute(&pool)
    .await
    .unwrap();
    let student_year_id = Uuid::new_v4();
    let placement_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO student_academic_years (
               id, student_id, academic_year_id, grade_level_id, study_program_id, status
           ) VALUES ($1, $2, $3, $4, $5, 'active')"#,
    )
    .bind(student_year_id)
    .bind(student_id)
    .bind(CURRENT_YEAR_ID)
    .bind(grade_level_id)
    .bind(study_program_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO homeroom_placements (
               id, student_academic_year_id, academic_year_id, homeroom_id,
               start_date, status, enrollment_type, class_number
           ) VALUES ($1, $2, $3, $4, '2025-05-01', 'current', 'regular', 98)"#,
    )
    .bind(placement_id)
    .bind(student_year_id)
    .bind(CURRENT_YEAR_ID)
    .bind(second_current_homeroom_id)
    .execute(&pool)
    .await
    .unwrap();

    let placements = student_years::list_placements_for_year(&pool, CURRENT_YEAR_ID)
        .await
        .unwrap();
    let expected_placement_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM homeroom_placements WHERE academic_year_id = $1")
            .bind(CURRENT_YEAR_ID)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(placements.len() as i64, expected_placement_count);
    assert!(placements.len() >= 2);
    assert!(placements
        .iter()
        .all(|placement| placement.academic_year_id == CURRENT_YEAR_ID));
    assert!(placements
        .iter()
        .any(|placement| placement.id == placement_id));

    let future_placements = student_years::list_placements_for_year(&pool, FUTURE_YEAR_ID)
        .await
        .unwrap();
    assert!(future_placements
        .iter()
        .all(|placement| placement.academic_year_id == FUTURE_YEAR_ID));
    assert!(future_placements
        .iter()
        .all(|future| { placements.iter().all(|current| current.id != future.id) }));

    let advisors = student_years::list_advisors_for_year(&pool, CURRENT_YEAR_ID)
        .await
        .unwrap();
    assert!(advisors.iter().any(|advisor| advisor.id == advisor_ids[0]));
    assert!(advisors.iter().any(|advisor| advisor.id == advisor_ids[1]));
    assert!(!advisors.iter().any(|advisor| advisor.id == advisor_ids[2]));
    let current_homeroom_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM homerooms WHERE academic_year_id = $1")
            .bind(CURRENT_YEAR_ID)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(advisors
        .iter()
        .all(|advisor| current_homeroom_ids.contains(&advisor.homeroom_id)));

    let unknown_year_id = Uuid::new_v4();
    assert!(matches!(
        student_years::list_placements_for_year(&pool, unknown_year_id).await,
        Err(crate::error::AppError::NotFound(_))
    ));
    assert!(matches!(
        student_years::list_advisors_for_year(&pool, unknown_year_id).await,
        Err(crate::error::AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn year_relationship_collections_reject_oversized_workspaces() {
    let pool = prepare_core_fixture("academic_core_year_relationship_limits").await;
    let (homeroom_id, grade_level_id, study_program_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT id, grade_level_id, study_program_id FROM homerooms \
         WHERE academic_year_id = $1 ORDER BY id LIMIT 1",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name, user_type, status
           )
           SELECT gen_random_uuid(),
                  'batch-limit-staff-' || sequence || '@example.invalid',
                  'batch-limit-staff-' || sequence,
                  'fixture-not-a-login', 'ครู', sequence::text, 'staff', 'active'
           FROM generate_series(1, 2001) sequence"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO homeroom_advisors (id, homeroom_id, user_id, role)
           SELECT gen_random_uuid(), $1, id, 'secondary'
           FROM users WHERE username LIKE 'batch-limit-staff-%'"#,
    )
    .bind(homeroom_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name, user_type, status
           )
           SELECT gen_random_uuid(),
                  'batch-limit-student-' || sequence || '@example.invalid',
                  'batch-limit-student-' || sequence,
                  'fixture-not-a-login', 'นักเรียน', sequence::text, 'student', 'active'
           FROM generate_series(1, 2001) sequence"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO student_academic_years (
               id, student_id, academic_year_id, grade_level_id, study_program_id, status
           )
           SELECT gen_random_uuid(), id, $1, $2, $3, 'active'
           FROM users WHERE username LIKE 'batch-limit-student-%'"#,
    )
    .bind(CURRENT_YEAR_ID)
    .bind(grade_level_id)
    .bind(study_program_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO homeroom_placements (
               id, student_academic_year_id, academic_year_id, homeroom_id,
               start_date, status, enrollment_type
           )
           SELECT gen_random_uuid(), student_year.id, $1, $2,
                  '2025-05-01', 'current', 'regular'
           FROM student_academic_years student_year
           JOIN users student ON student.id = student_year.student_id
           WHERE student.username LIKE 'batch-limit-student-%'"#,
    )
    .bind(CURRENT_YEAR_ID)
    .bind(homeroom_id)
    .execute(&pool)
    .await
    .unwrap();

    let advisors = student_years::list_advisors_for_year(&pool, CURRENT_YEAR_ID).await;
    let placements = student_years::list_placements_for_year(&pool, CURRENT_YEAR_ID).await;
    assert!(matches!(
        advisors,
        Err(crate::error::AppError::ValidationError(_))
    ));
    assert!(matches!(
        placements,
        Err(crate::error::AppError::ValidationError(_))
    ));
}

#[tokio::test]
async fn study_program_options_are_published_effective_and_authorized() {
    let pool = prepare_core_fixture("academic_core_program_options").await;
    let owner_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        owner_ids.len(),
        2,
        "fixture must provide two active organization units"
    );
    let owner_one_id = owner_ids[0];
    let owner_two_id = owner_ids[1];

    let (current_curriculum_id, current_program_id) = create_published_program_option_fixture(
        &pool,
        owner_two_id,
        CURRENT_YEAR_ID,
        None,
        "CURRENT",
    )
    .await;
    let (_, future_program_id) = create_published_program_option_fixture(
        &pool,
        owner_two_id,
        FUTURE_YEAR_ID,
        None,
        "FUTURE",
    )
    .await;
    let (_, expired_program_id) = create_published_program_option_fixture(
        &pool,
        owner_two_id,
        CURRENT_YEAR_ID,
        Some(CURRENT_YEAR_ID),
        "EXPIRED",
    )
    .await;

    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let draft_curriculum = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            code: "OPTION-DRAFT".to_string(),
            name_th: "หลักสูตรตัวเลือกร่าง".to_string(),
            name_en: None,
            description: None,
            grade_level_ids: vec![grade_level_id],
            owning_organization_unit_id: Some(owner_two_id),
        },
    )
    .await
    .unwrap();
    let draft_version = curriculum::create_version(
        &pool,
        draft_curriculum.id,
        CreateCurriculumVersionRequest {
            version_name: "ฉบับร่าง".to_string(),
            start_academic_year_id: CURRENT_YEAR_ID,
            end_academic_year_id: None,
            description: None,
        },
    )
    .await
    .unwrap();
    let draft_program = curriculum::create_program(
        &pool,
        draft_version.id,
        CreateStudyProgramRequest {
            code: "PROGRAM-DRAFT".to_string(),
            name_th: "แผนการเรียนร่าง".to_string(),
            name_en: None,
            is_default: true,
            owning_organization_unit_id: Some(owner_two_id),
        },
    )
    .await
    .unwrap();

    let school_options = curriculum::list_study_program_options_for_year(
        &pool,
        CURRENT_YEAR_ID,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    let current_option = school_options
        .iter()
        .find(|option| option.id == current_program_id)
        .expect("current published program must be selectable");
    assert_eq!(current_option.code, "PROGRAM-CURRENT");
    assert_eq!(current_option.name, "แผนการเรียน CURRENT");
    assert_eq!(current_option.curriculum_id, current_curriculum_id);
    assert_eq!(current_option.curriculum_name, "หลักสูตรตัวเลือก CURRENT");
    assert!(!school_options
        .iter()
        .any(|option| option.id == future_program_id));
    assert!(!school_options
        .iter()
        .any(|option| option.id == draft_program.id));

    let no_access = curriculum::list_study_program_options_for_year(
        &pool,
        CURRENT_YEAR_ID,
        &AcademicResourceListFilter::default(),
    )
    .await
    .unwrap();
    assert!(no_access.is_empty());

    let unrelated_unit = curriculum::list_study_program_options_for_year(
        &pool,
        CURRENT_YEAR_ID,
        &AcademicResourceListFilter {
            organization_unit_ids: vec![owner_one_id],
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    assert!(!unrelated_unit
        .iter()
        .any(|option| option.id == current_program_id));

    let owner_tree = curriculum::list_study_program_options_for_year(
        &pool,
        CURRENT_YEAR_ID,
        &AcademicResourceListFilter {
            organization_tree_unit_ids: vec![owner_two_id],
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    assert!(owner_tree
        .iter()
        .any(|option| option.id == current_program_id));
    assert!(!owner_tree
        .iter()
        .any(|option| option.id == future_program_id));
    assert!(!owner_tree
        .iter()
        .any(|option| option.id == draft_program.id));

    let future_options = curriculum::list_study_program_options_for_year(
        &pool,
        FUTURE_YEAR_ID,
        &AcademicResourceListFilter {
            organization_tree_unit_ids: vec![owner_two_id],
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    assert!(future_options
        .iter()
        .any(|option| option.id == future_program_id));
    assert!(!future_options
        .iter()
        .any(|option| option.id == expired_program_id));

    let union = curriculum::list_study_program_options_for_year(
        &pool,
        CURRENT_YEAR_ID,
        &AcademicResourceListFilter {
            organization_unit_ids: vec![owner_one_id],
            organization_tree_unit_ids: vec![owner_two_id],
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    let owner_tree_ids: Vec<Uuid> = owner_tree.iter().map(|option| option.id).collect();
    let union_ids: Vec<Uuid> = union.iter().map(|option| option.id).collect();
    assert_eq!(union_ids, owner_tree_ids);

    let unknown_year_id = Uuid::new_v4();
    assert!(matches!(
        curriculum::list_study_program_options_for_year(
            &pool,
            unknown_year_id,
            &AcademicResourceListFilter {
                includes_school_owned: true,
                ..AcademicResourceListFilter::default()
            },
        )
        .await,
        Err(crate::error::AppError::NotFound(_))
    ));
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
            hours_per_term: Some("30.00".to_string()),
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
    assert_eq!(version.hours_per_term.as_deref(), Some("30.00"));
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
            hours_per_term: published.hours_per_term,
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
async fn activity_catalog_requires_total_hours_before_publishing_a_new_version() {
    let pool = prepare_core_fixture("academic_core_activity_total_hours_publish").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY level_type, year, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "TOTAL-HOURS-REQUIRED".to_string(),
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
            name: "กิจกรรมที่ยังขาดชั่วโมงรวม".to_string(),
            description: None,
            hours_per_week: "1.00".to_string(),
            hours_per_term: None,
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();

    let result = catalog::publish_activity_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::AppError::ValidationError(message))
            if message.contains("ชั่วโมงรวมต่อภาคเรียน")
    ));
}

async fn create_overview_subject_version(
    pool: &PgPool,
    subject_id: Uuid,
    grade_level_id: Uuid,
    name: &str,
    effective_from: NaiveDate,
    effective_until: Option<NaiveDate>,
    publish: bool,
) {
    let version = catalog::create_subject_version(
        pool,
        subject_id,
        CreateSubjectVersionRequest {
            name_th: name.to_string(),
            name_en: None,
            credit: "1.00".to_string(),
            hours_per_semester: Some(40),
            subject_type: "BASIC".to_string(),
            group_id: None,
            description: None,
            effective_from,
            effective_until,
            term_code: None,
            periods_per_week: Some(2),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    if publish {
        catalog::publish_subject_version(
            pool,
            version.id,
            PublishVersionRequest {
                row_version: version.row_version,
            },
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn catalog_overview_selects_effective_versions_without_promoting_drafts() {
    let pool = prepare_core_fixture("catalog_overview_version_states").await;
    let today = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
    let grade_level_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active = true ORDER BY level_type, year, id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let current = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OVERVIEW-CURRENT".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    create_overview_subject_version(
        &pool,
        current.id,
        grade_level_id,
        "รุ่นที่ใช้อยู่",
        NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
        true,
    )
    .await;
    create_overview_subject_version(
        &pool,
        current.id,
        grade_level_id,
        "ร่างที่ยังไม่เผยแพร่",
        NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2026, 4, 30).unwrap()),
        false,
    )
    .await;

    let upcoming = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OVERVIEW-UPCOMING".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    create_overview_subject_version(
        &pool,
        upcoming.id,
        grade_level_id,
        "รุ่นอนาคต",
        NaiveDate::from_ymd_opt(2026, 12, 1).unwrap(),
        None,
        true,
    )
    .await;

    let expired = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OVERVIEW-EXPIRED".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    create_overview_subject_version(
        &pool,
        expired.id,
        grade_level_id,
        "รุ่นเดิม",
        NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()),
        true,
    )
    .await;

    let unpublished = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OVERVIEW-UNPUBLISHED".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();

    let school_filter = AcademicResourceListFilter {
        includes_school_owned: true,
        ..AcademicResourceListFilter::default()
    };
    let overview = catalog::list_subject_overview(&pool, &school_filter, &school_filter, today)
        .await
        .unwrap();

    let current_item = overview
        .items
        .iter()
        .find(|item| item.subject.id == current.id)
        .unwrap();
    assert_eq!(current_item.display_state, CatalogDisplayState::Current);
    assert_eq!(
        current_item.display_version.as_ref().unwrap().name_th,
        "รุ่นที่ใช้อยู่"
    );
    assert_eq!(current_item.draft_count, 1);
    assert!(current_item.can_manage);
    assert_eq!(current_item.grade_levels[0].id, grade_level_id);
    assert!(!current_item.grade_levels[0].name.is_empty());

    let upcoming_item = overview
        .items
        .iter()
        .find(|item| item.subject.id == upcoming.id)
        .unwrap();
    assert_eq!(upcoming_item.display_state, CatalogDisplayState::Upcoming);
    assert_eq!(upcoming_item.draft_count, 0);

    let expired_item = overview
        .items
        .iter()
        .find(|item| item.subject.id == expired.id)
        .unwrap();
    assert_eq!(expired_item.display_state, CatalogDisplayState::Expired);

    let unpublished_item = overview
        .items
        .iter()
        .find(|item| item.subject.id == unpublished.id)
        .unwrap();
    assert_eq!(
        unpublished_item.display_state,
        CatalogDisplayState::Unpublished
    );
    assert!(unpublished_item.display_version.is_none());
    assert!(unpublished_item.grade_levels.is_empty());
    assert!(overview
        .grade_level_options
        .iter()
        .any(|level| level.id == grade_level_id && level.short_name.is_some()));
    assert!(overview
        .owner_options
        .iter()
        .any(|owner| owner.organization_unit_id.is_none()));

    sqlx::query("UPDATE grade_levels SET is_active = false WHERE id = $1")
        .bind(grade_level_id)
        .execute(&pool)
        .await
        .unwrap();
    let overview_after_grade_archive =
        catalog::list_subject_overview(&pool, &school_filter, &school_filter, today)
            .await
            .unwrap();
    let archived_grade_item = overview_after_grade_archive
        .items
        .iter()
        .find(|item| item.subject.id == current.id)
        .unwrap();
    assert_eq!(archived_grade_item.grade_levels[0].id, grade_level_id);
    assert!(!overview_after_grade_archive
        .grade_level_options
        .iter()
        .any(|level| level.id == grade_level_id));
}

#[tokio::test]
async fn catalog_overview_keeps_activity_owner_scope_and_grade_options() {
    let pool = prepare_core_fixture("catalog_overview_activity_scope").await;
    let today = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
    let grade_level_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active = true ORDER BY level_type, year, id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let owner_id: Uuid =
        sqlx::query_scalar("SELECT id FROM organization_units ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "OVERVIEW-ACTIVITY".to_string(),
            activity_type: "guidance".to_string(),
            owning_organization_unit_id: Some(owner_id),
        },
    )
    .await
    .unwrap();
    let version = catalog::create_activity_version(
        &pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "แนะแนวที่ใช้อยู่".to_string(),
            description: None,
            hours_per_week: "1.00".to_string(),
            hours_per_term: Some("20.00".to_string()),
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    catalog::publish_activity_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap();

    let no_access = catalog::list_activity_overview(
        &pool,
        &AcademicResourceListFilter::default(),
        &AcademicResourceListFilter::default(),
        today,
    )
    .await
    .unwrap();
    assert!(!no_access
        .items
        .iter()
        .any(|item| item.activity.id == activity.id));

    let school_read_filter = AcademicResourceListFilter {
        includes_school_owned: true,
        ..AcademicResourceListFilter::default()
    };
    let school_scope = catalog::list_activity_overview(
        &pool,
        &school_read_filter,
        &AcademicResourceListFilter::default(),
        today,
    )
    .await
    .unwrap();
    let school_read_item = school_scope
        .items
        .iter()
        .find(|item| item.activity.id == activity.id)
        .unwrap();
    assert!(!school_read_item.can_manage);
    assert!(school_scope.owner_options.is_empty());

    let owner_filter = AcademicResourceListFilter {
        organization_unit_ids: vec![owner_id],
        ..AcademicResourceListFilter::default()
    };
    let owner_scope = catalog::list_activity_overview(&pool, &owner_filter, &owner_filter, today)
        .await
        .unwrap();
    let item = owner_scope
        .items
        .iter()
        .find(|item| item.activity.id == activity.id)
        .unwrap();
    assert_eq!(item.display_state, CatalogDisplayState::Current);
    assert!(item.can_manage);
    assert_eq!(item.grade_levels[0].id, grade_level_id);
    assert!(owner_scope
        .grade_level_options
        .iter()
        .any(|level| level.id == grade_level_id));
    assert!(owner_scope
        .owner_options
        .iter()
        .any(|owner| { owner.organization_unit_id == Some(owner_id) && owner.code.is_some() }));
    assert!(!owner_scope
        .owner_options
        .iter()
        .any(|owner| owner.organization_unit_id.is_none()));
}

#[tokio::test]
async fn curriculum_overview_resolves_display_versions_and_labels() {
    let pool = prepare_core_fixture("academic_core_curriculum_overview").await;
    let grade_level_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active = true AND level_type = 'secondary' \
         AND year = 1 ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM subject_versions WHERE status = 'published' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let owner_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM organization_units ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(owner_ids.len(), 2);
    let next_year_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_years (
               id, year, name, start_date, end_date, school_days, status
           ) VALUES ($1, 2027, 'ปีการศึกษา 2027', '2027-05-01', '2028-04-30',
                     ARRAY['MON','TUE','WED','THU','FRI'], 'planning')"#,
    )
    .bind(next_year_id)
    .execute(&pool)
    .await
    .unwrap();

    let (current_id, _) = create_curriculum_overview_fixture(
        &pool,
        owner_ids[0],
        grade_level_id,
        subject_version_id,
        "CUR-A",
        FUTURE_YEAR_ID,
        None,
        true,
        2,
    )
    .await;
    let current_draft = curriculum::create_version(
        &pool,
        current_id,
        CreateCurriculumVersionRequest {
            version_name: "ฉบับร่าง CUR-A".to_string(),
            start_academic_year_id: next_year_id,
            end_academic_year_id: None,
            description: None,
        },
    )
    .await
    .unwrap();
    curriculum::create_program(
        &pool,
        current_draft.id,
        CreateStudyProgramRequest {
            code: "CUR-A-DRAFT".to_string(),
            name_th: "แผนฉบับร่าง".to_string(),
            name_en: None,
            is_default: true,
            owning_organization_unit_id: Some(owner_ids[0]),
        },
    )
    .await
    .unwrap();
    create_curriculum_overview_fixture(
        &pool,
        owner_ids[0],
        grade_level_id,
        subject_version_id,
        "CUR-B",
        next_year_id,
        None,
        true,
        1,
    )
    .await;
    create_curriculum_overview_fixture(
        &pool,
        owner_ids[0],
        grade_level_id,
        subject_version_id,
        "CUR-C",
        CURRENT_YEAR_ID,
        Some(CURRENT_YEAR_ID),
        true,
        1,
    )
    .await;
    create_curriculum_overview_fixture(
        &pool,
        owner_ids[0],
        grade_level_id,
        subject_version_id,
        "CUR-D",
        next_year_id,
        None,
        false,
        0,
    )
    .await;
    create_curriculum_overview_fixture(
        &pool,
        owner_ids[1],
        grade_level_id,
        subject_version_id,
        "OUTSIDE",
        FUTURE_YEAR_ID,
        None,
        true,
        1,
    )
    .await;

    let overview = workspaces::curriculum_overview(
        &pool,
        &AcademicResourceListFilter {
            organization_unit_ids: vec![owner_ids[0]],
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .expect("overview should load");

    assert_eq!(overview.items.len(), 4);
    assert_eq!(overview.items[0].curriculum.code, "CUR-A");
    assert_eq!(
        overview.items[0].display_state,
        CurriculumDisplayState::Current
    );
    assert_eq!(overview.items[0].study_program_count, 2);
    assert_eq!(overview.items[0].draft_count, 1);
    assert_eq!(overview.items[0].grade_levels[0].name, "มัธยมศึกษาปีที่ 1");
    assert_eq!(
        overview.items[0].start_academic_year_name.as_deref(),
        Some("ปีการศึกษา 2026")
    );
    assert_eq!(
        overview.items[1].display_state,
        CurriculumDisplayState::Upcoming
    );
    assert_eq!(
        overview.items[2].display_state,
        CurriculumDisplayState::Expired
    );
    assert_eq!(
        overview.items[3].display_state,
        CurriculumDisplayState::Unpublished
    );
    assert!(!overview
        .items
        .iter()
        .any(|item| item.curriculum.code == "OUTSIDE"));
}

#[tokio::test]
async fn curriculum_management_options_are_published_scoped_and_ordered() {
    let pool = prepare_core_fixture("academic_core_curriculum_management_options").await;
    let owner_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM organization_units ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(owner_ids.len(), 2);
    let grade_level_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active = true ORDER BY level_type, year, id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            code: "OPTIONS".to_string(),
            name_th: "หลักสูตรตัวเลือก".to_string(),
            name_en: None,
            description: None,
            grade_level_ids: vec![grade_level_id],
            owning_organization_unit_id: Some(owner_ids[0]),
        },
    )
    .await
    .unwrap();
    let version = curriculum::create_version(
        &pool,
        curriculum_row.id,
        CreateCurriculumVersionRequest {
            version_name: "ฉบับตัวเลือก".to_string(),
            start_academic_year_id: FUTURE_YEAR_ID,
            end_academic_year_id: None,
            description: None,
        },
    )
    .await
    .unwrap();

    let subject = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OPTION-SUBJECT".to_string(),
            owning_organization_unit_id: Some(owner_ids[0]),
        },
    )
    .await
    .unwrap();
    let subject_version = catalog::create_subject_version(
        &pool,
        subject.id,
        CreateSubjectVersionRequest {
            name_th: "รายวิชาตัวเลือก".to_string(),
            name_en: None,
            credit: "1.00".to_string(),
            hours_per_semester: Some(40),
            subject_type: "BASIC".to_string(),
            group_id: None,
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            periods_per_week: Some(2),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let subject_version = catalog::publish_subject_version(
        &pool,
        subject_version.id,
        PublishVersionRequest {
            row_version: subject_version.row_version,
        },
    )
    .await
    .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "OPTION-ACTIVITY".to_string(),
            activity_type: "guidance".to_string(),
            owning_organization_unit_id: Some(owner_ids[0]),
        },
    )
    .await
    .unwrap();
    let activity_version = catalog::create_activity_version(
        &pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "กิจกรรมตัวเลือก".to_string(),
            description: None,
            hours_per_week: "1.00".to_string(),
            hours_per_term: Some("20.00".to_string()),
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let activity_version = catalog::publish_activity_version(
        &pool,
        activity_version.id,
        PublishVersionRequest {
            row_version: activity_version.row_version,
        },
    )
    .await
    .unwrap();
    let outside = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OPTION-OUTSIDE".to_string(),
            owning_organization_unit_id: Some(owner_ids[1]),
        },
    )
    .await
    .unwrap();
    let outside_version = catalog::create_subject_version(
        &pool,
        outside.id,
        CreateSubjectVersionRequest {
            name_th: "รายวิชานอกขอบเขต".to_string(),
            name_en: None,
            credit: "1.00".to_string(),
            hours_per_semester: Some(40),
            subject_type: "BASIC".to_string(),
            group_id: None,
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            periods_per_week: Some(2),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let outside_version = catalog::publish_subject_version(
        &pool,
        outside_version.id,
        PublishVersionRequest {
            row_version: outside_version.row_version,
        },
    )
    .await
    .unwrap();

    let owner_filter = AcademicResourceListFilter {
        organization_unit_ids: vec![owner_ids[0]],
        ..AcademicResourceListFilter::default()
    };
    let create_options = workspaces::curriculum_create_options(&pool, &owner_filter)
        .await
        .unwrap();
    assert!(!create_options.grade_levels.is_empty());
    let active_option_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM grade_levels WHERE id = ANY($1) AND is_active = true",
    )
    .bind(
        &create_options
            .grade_levels
            .iter()
            .map(|level| level.id)
            .collect::<Vec<_>>(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active_option_count,
        create_options.grade_levels.len() as i64
    );

    let options = workspaces::curriculum_management_options(&pool, version.id, &owner_filter)
        .await
        .unwrap();
    assert!(options
        .academic_years
        .windows(2)
        .all(|pair| pair[0].year >= pair[1].year));
    assert!(options
        .catalog_versions
        .iter()
        .any(|option| option.id == subject_version.id
            && option.resource_kind == RequirementResourceKind::Course));
    assert!(options
        .catalog_versions
        .iter()
        .any(|option| option.id == activity_version.id
            && option.resource_kind == RequirementResourceKind::Activity));
    assert!(!options
        .catalog_versions
        .iter()
        .any(|option| option.id == outside_version.id));
    assert!(matches!(
        workspaces::curriculum_management_options(
            &pool,
            version.id,
            &AcademicResourceListFilter::default(),
        )
        .await,
        Err(crate::error::AppError::Forbidden(_))
    ));
}

#[tokio::test]
async fn curriculum_program_workspace_resolves_requirement_labels() {
    let pool = prepare_core_fixture("academic_core_workspace_reads").await;
    let actor_user_id = fixture_actor(&pool).await;
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
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
    .await
    .unwrap();
    let activity = catalog::create_activity(
        &pool,
        CreateCatalogActivityRequest {
            code: "WORKSPACE-ACTIVITY".to_string(),
            activity_type: "guidance".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let activity_version = catalog::create_activity_version(
        &pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "กิจกรรม workspace".to_string(),
            description: None,
            hours_per_week: "1.00".to_string(),
            hours_per_term: Some("20.00".to_string()),
            scheduling_mode: "synchronized".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let activity_version = catalog::publish_activity_version(
        &pool,
        activity_version.id,
        PublishVersionRequest {
            row_version: activity_version.row_version,
        },
    )
    .await
    .unwrap();
    let activity_version_id = activity_version.id;
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            code: "WORKSPACE".to_string(),
            name_th: "หลักสูตรทดสอบ workspace".to_string(),
            name_en: None,
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
            version_name: "ฉบับ workspace".to_string(),
            start_academic_year_id: CURRENT_YEAR_ID,
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
            code: "DEFAULT".to_string(),
            name_th: "แผนหลัก".to_string(),
            name_en: None,
            is_default: true,
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let alternative_program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            code: "ALTERNATIVE".to_string(),
            name_th: "แผนทางเลือก".to_string(),
            name_en: None,
            is_default: false,
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let structure = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    let first_slot_id = structure.term_slots[0].id;
    let second_slot_id = structure
        .term_slots
        .get(1)
        .map_or(first_slot_id, |slot| slot.id);
    curriculum_structure::replace_program_structure(
        &pool,
        default_program.id,
        ReplaceCurriculumStructureRequest {
            requirements: vec![
                CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Activity,
                    catalog_version_id: activity_version_id,
                    grade_level_id,
                    term_slot_id: second_slot_id,
                    requirement_kind: RequirementKind::Required,
                    display_order: 2,
                },
                CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Course,
                    catalog_version_id: subject_version_id,
                    grade_level_id,
                    term_slot_id: first_slot_id,
                    requirement_kind: RequirementKind::Required,
                    display_order: 1,
                },
            ],
            row_version: default_program.row_version,
        },
    )
    .await
    .unwrap();
    curriculum_structure::replace_program_structure(
        &pool,
        alternative_program.id,
        ReplaceCurriculumStructureRequest {
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Course,
                catalog_version_id: subject_version_id,
                grade_level_id,
                term_slot_id: first_slot_id,
                requirement_kind: RequirementKind::Elective,
                display_order: 1,
            }],
            row_version: alternative_program.row_version,
        },
    )
    .await
    .unwrap();

    let other_curriculum = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            code: "WORKSPACE-OTHER".to_string(),
            name_th: "หลักสูตรอื่น".to_string(),
            name_en: None,
            description: None,
            grade_level_ids: vec![grade_level_id],
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let other_version = curriculum::create_version(
        &pool,
        other_curriculum.id,
        CreateCurriculumVersionRequest {
            version_name: "ฉบับอื่น".to_string(),
            start_academic_year_id: CURRENT_YEAR_ID,
            end_academic_year_id: None,
            description: None,
        },
    )
    .await
    .unwrap();
    let other_program = curriculum::create_program(
        &pool,
        other_version.id,
        CreateStudyProgramRequest {
            code: "OTHER".to_string(),
            name_th: "แผนอื่น".to_string(),
            name_en: None,
            is_default: true,
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();

    let program_workspace = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    assert_eq!(
        program_workspace
            .programs
            .iter()
            .map(|program| program.id)
            .collect::<Vec<_>>(),
        vec![default_program.id, alternative_program.id]
    );
    assert_eq!(program_workspace.requirements.len(), 3);
    assert_eq!(
        program_workspace
            .requirements
            .iter()
            .map(|requirement| requirement.study_program_id)
            .collect::<Vec<_>>(),
        vec![
            default_program.id,
            default_program.id,
            alternative_program.id
        ]
    );
    assert_eq!(program_workspace.requirements[0].display_order, 1);
    assert_eq!(program_workspace.requirements[1].display_order, 2);
    let course_requirement = &program_workspace.requirements[0];
    assert_eq!(course_requirement.grade_level.id, grade_level_id);
    assert!(!course_requirement.grade_level.name.is_empty());
    assert_eq!(course_requirement.catalog_version_id, subject_version_id);
    assert_eq!(
        course_requirement.resource_kind,
        RequirementResourceKind::Course
    );
    assert!(!course_requirement.code.is_empty());
    assert!(!course_requirement.name.is_empty());
    let activity_requirement = &program_workspace.requirements[1];
    assert_eq!(activity_requirement.catalog_version_id, activity_version_id);
    assert_eq!(
        activity_requirement.resource_kind,
        RequirementResourceKind::Activity
    );
    assert!(program_workspace
        .programs
        .iter()
        .all(|program| program.id != other_program.id));
    assert!(program_workspace
        .requirements
        .iter()
        .all(|requirement| requirement.study_program_id != other_program.id));
    let serialized_program_workspace = serde_json::to_value(&program_workspace).unwrap();
    let serialized_requirement = &serialized_program_workspace["requirements"][0];
    assert_eq!(
        serialized_requirement["studyProgramId"],
        default_program.id.to_string()
    );
    assert!(serialized_requirement.get("displayOrder").is_some());
    assert!(serialized_requirement.get("gradeLevel").is_some());
    assert!(serialized_requirement.get("metrics").is_some());

    let extra_schedule = bell_schedules::create(
        &pool,
        actor_user_id,
        CreateBellScheduleRequest {
            academic_year_id: FUTURE_YEAR_ID,
            name: "ตารางคาบ workspace".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    bell_schedules::replace_periods(
        &pool,
        actor_user_id,
        extra_schedule.id,
        ReplaceBellSchedulePeriodsRequest {
            periods: vec![BellSchedulePeriodInput {
                name: Some("คาบ workspace".to_string()),
                start_time: NaiveTime::from_hms_opt(7, 30, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(8, 20, 0).unwrap(),
                order_index: 1,
                applicable_days: vec!["MON".to_string()],
                is_active: true,
            }],
            row_version: extra_schedule.row_version,
        },
    )
    .await
    .unwrap();

    let full_access = ActorContext {
        user_id: actor_user_id,
        permissions: vec![
            codes::ACADEMIC_YEAR_READ_SCHOOL.to_string(),
            codes::ACADEMIC_TERM_READ_SCHOOL.to_string(),
        ],
    };
    let setup_workspace = workspaces::setup_workspace(&pool, &full_access)
        .await
        .unwrap();
    let expected_years = years_terms::list_years(&pool).await.unwrap();
    let mut expected_terms = Vec::new();
    let mut expected_schedules = Vec::new();
    for year in &expected_years {
        expected_terms.extend(years_terms::list_terms(&pool, year.id).await.unwrap());
        expected_schedules.extend(bell_schedules::list(&pool, year.id).await.unwrap());
    }
    assert_eq!(
        setup_workspace
            .years
            .iter()
            .map(|year| year.id)
            .collect::<Vec<_>>(),
        expected_years
            .iter()
            .map(|year| year.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        setup_workspace
            .terms
            .iter()
            .map(|term| term.id)
            .collect::<Vec<_>>(),
        expected_terms
            .iter()
            .map(|term| term.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        setup_workspace
            .bell_schedules
            .iter()
            .map(|schedule| schedule.id)
            .collect::<Vec<_>>(),
        expected_schedules
            .iter()
            .map(|schedule| schedule.id)
            .collect::<Vec<_>>()
    );
    assert!(setup_workspace
        .bell_schedules
        .iter()
        .any(|schedule| schedule.id == extra_schedule.id));
    let serialized_setup = serde_json::to_value(&setup_workspace).unwrap();
    assert!(serialized_setup["bellSchedules"]
        .as_array()
        .unwrap()
        .iter()
        .all(|schedule| schedule.get("periods").is_none()));

    for permissions in [
        vec![codes::ACADEMIC_YEAR_READ_SCHOOL.to_string()],
        vec![codes::ACADEMIC_TERM_READ_SCHOOL.to_string()],
    ] {
        let incomplete_actor = ActorContext {
            user_id: actor_user_id,
            permissions,
        };
        assert!(matches!(
            workspaces::setup_workspace(&pool, &incomplete_actor).await,
            Err(crate::error::AppError::Forbidden(_))
        ));
    }
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
    sqlx::query(
        r#"INSERT INTO subject_version_grade_levels (subject_id, grade_level_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
    )
    .bind(subject_version_id)
    .bind(grade_level_id)
    .execute(&pool)
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

    let mut structure = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    if structure.term_slots.is_empty() {
        structure = curriculum_structure::replace_term_slots(
            &pool,
            version.id,
            ReplaceCurriculumTermSlotsRequest {
                slots: vec![CurriculumTermSlotInput {
                    id: None,
                    sequence: 1,
                    term_type: AcademicTermType::Regular,
                    type_occurrence: 1,
                    name: "ภาคเรียนที่ 1".to_string(),
                }],
                row_version: structure.row_version,
            },
        )
        .await
        .unwrap();
    }
    let term_slot_id = structure.term_slots[0].id;

    for program in [&default_program, &science_program] {
        structure = curriculum_structure::replace_program_structure(
            &pool,
            program.id,
            ReplaceCurriculumStructureRequest {
                requirements: vec![CurriculumStructureRequirementInput {
                    resource_kind: RequirementResourceKind::Course,
                    catalog_version_id: subject_version_id,
                    grade_level_id,
                    term_slot_id,
                    requirement_kind: RequirementKind::Required,
                    display_order: 1,
                }],
                row_version: program.row_version,
            },
        )
        .await
        .unwrap();
        let requirement = structure
            .requirements
            .iter()
            .find(|requirement| requirement.study_program_id == program.id)
            .unwrap();
        assert!(requirement.metrics.credit.is_some());
        assert!(requirement.metrics.total_hours.is_some());
    }

    let published = curriculum::publish_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: structure.row_version,
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

#[tokio::test]
async fn curriculum_publication_rejects_missing_official_catalog_metrics() {
    let pool = prepare_core_fixture("academic_core_curriculum_metric_blocker").await;
    let (activity_version_id, grade_level_id): (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT version.id, grade.value::uuid
           FROM activity_versions version
           CROSS JOIN LATERAL jsonb_array_elements_text(version.grade_level_ids) grade(value)
           WHERE version.status = 'published' AND version.hours_per_term IS NULL
           ORDER BY version.id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let curriculum_row = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            code: "METRIC-BLOCKER".to_string(),
            name_th: "หลักสูตรทดสอบข้อมูลทางการ".to_string(),
            name_en: None,
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
            version_name: "ฉบับข้อมูลไม่ครบ".to_string(),
            start_academic_year_id: CURRENT_YEAR_ID,
            end_academic_year_id: None,
            description: None,
        },
    )
    .await
    .unwrap();
    let program = curriculum::create_program(
        &pool,
        version.id,
        CreateStudyProgramRequest {
            code: "DEFAULT".to_string(),
            name_th: "แผนหลัก".to_string(),
            name_en: None,
            is_default: true,
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let workspace = curriculum_structure::get_workspace(&pool, version.id)
        .await
        .unwrap();
    let workspace = curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Activity,
                catalog_version_id: activity_version_id,
                grade_level_id,
                term_slot_id: workspace.term_slots[0].id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
            row_version: program.row_version,
        },
    )
    .await
    .unwrap();

    let error = curriculum::publish_version(
        &pool,
        version.id,
        PublishVersionRequest {
            row_version: workspace.row_version,
        },
    )
    .await
    .unwrap_err();
    assert!(error.public_message().contains("ชั่วโมงรวม"));
}
