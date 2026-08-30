use std::collections::{HashMap, HashSet};

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::core::models::{RequirementKind, StudyProgramOption};
use crate::modules::academic::core::services::curriculum;
use crate::modules::academic::models::timetable_version::TimetableVersionStatus;
use crate::modules::lookup::models::{
    AcademicLookupQuery, GradeLevelLookupItem, HomeroomLookupItem, LookupQuery,
};
use crate::modules::lookup::services as lookup_services;
use crate::policies::learning_offering_access_policy::learning_offering_owner_allowed;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

use super::super::models::{
    DeliveryCatalogVersionOption, DeliveryManagementOptions, DeliveryPrerequisite,
    HomeroomDeliveryGroupSummary, HomeroomDeliveryItem, HomeroomDeliveryRoom,
    HomeroomDeliveryWorkspace, HomeroomGroupMode, HomeroomOfferingState, HomeroomTeacherState,
    HomeroomTimetableState, LearningDeliveryOverview, LearningOfferingKind,
    LearningOfferingOverviewItem, LearningOfferingQuery, LearningOfferingStatus, RosterStatus,
    UnlinkedDeliveryItem,
};
use super::offerings;

const MAX_WORKSPACE_GROUPS: i64 = 2_000;
const MAX_WORKSPACE_HOMEROOMS: usize = 500;
const MAX_WORKSPACE_ITEMS: usize = 50_000;
const MAX_WORKSPACE_TARGET_ROWS: usize = 20_000;
const MAX_CATALOG_OPTIONS: usize = 2_000;
const MAX_LOOKUP_OPTIONS: usize = 500;

#[derive(Debug, sqlx::FromRow)]
struct GradeLevelRow {
    id: Uuid,
    level_type: String,
    year: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct OfferingAggregateRow {
    learning_offering_id: Uuid,
    group_count: i64,
    teacher_assignment_count: i64,
    groups_without_primary_teacher: i64,
    published_roster_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct CatalogVersionRow {
    id: Uuid,
    kind: LearningOfferingKind,
    code: String,
    name: String,
    version_no: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct HomeroomDeliveryBaseRow {
    homeroom_id: Uuid,
    homeroom_name: String,
    grade_level_id: Uuid,
    grade_level_type: String,
    grade_level_year: i32,
    study_program_id: Uuid,
    study_program_code: String,
    study_program_name: String,
    curriculum_id: Uuid,
    curriculum_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ExpectedDeliveryRow {
    homeroom_id: Uuid,
    requirement_id: Uuid,
    resource_kind: LearningOfferingKind,
    catalog_version_id: Uuid,
    code: String,
    name: String,
    requirement_kind: RequirementKind,
    standard_periods_per_week: Option<i32>,
}

#[derive(Debug, sqlx::FromRow)]
struct OfferingTargetRow {
    offering_id: Uuid,
    resource_kind: LearningOfferingKind,
    catalog_version_id: Uuid,
    status: LearningOfferingStatus,
    code: String,
    name: String,
    weekly_period_target: Option<i32>,
    target_kind: String,
    homeroom_id: Option<Uuid>,
    grade_level_id: Uuid,
    study_program_id: Uuid,
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct DeliveryGroupRow {
    id: Uuid,
    learning_offering_id: Uuid,
    code: String,
    name: String,
    status: LearningOfferingStatus,
    roster_status: RosterStatus,
    homeroom_ids: Vec<Uuid>,
    homeroom_names: Vec<String>,
    primary_teacher_count: i64,
    timetable_entry_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct WorkspaceTimetableVersionRow {
    id: Uuid,
    status: TimetableVersionStatus,
}

pub async fn homeroom_delivery_workspace(
    pool: &PgPool,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<HomeroomDeliveryWorkspace, AppError> {
    let (term_type, type_occurrence, term_status): (String, i64, String) = sqlx::query_as(
        r#"SELECT selected.term_type,
                  count(sibling.id)::bigint AS type_occurrence,
                  selected.status
           FROM academic_terms selected
           JOIN academic_terms sibling
             ON sibling.academic_year_id = selected.academic_year_id
            AND sibling.term_type = selected.term_type
            AND sibling.sequence_no <= selected.sequence_no
           WHERE selected.id = $1
             AND selected.academic_year_id = $2
           GROUP BY selected.id, selected.term_type, selected.status"#,
    )
    .bind(academic_term_id)
    .bind(academic_year_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนในปีการศึกษาที่เลือก".to_string()))?;

    let timetable_version: Option<WorkspaceTimetableVersionRow> = match term_status.as_str() {
        "planning" | "ready" => {
            sqlx::query_as(
                r#"SELECT id, status
               FROM academic_timetable_versions
               WHERE academic_term_id = $1 AND status = 'published'
               ORDER BY effective_from, id LIMIT 1"#,
            )
            .bind(academic_term_id)
            .fetch_optional(pool)
            .await?
        }
        "active" => {
            let current: Option<WorkspaceTimetableVersionRow> = sqlx::query_as(
                r#"SELECT id, status
                   FROM academic_timetable_versions
                   WHERE academic_term_id = $1
                     AND status = 'published'
                     AND effective_from <= CURRENT_DATE
                   ORDER BY effective_from DESC, id LIMIT 1"#,
            )
            .bind(academic_term_id)
            .fetch_optional(pool)
            .await?;
            if current.is_some() {
                current
            } else {
                sqlx::query_as(
                    r#"SELECT id, status
                       FROM academic_timetable_versions
                       WHERE academic_term_id = $1 AND status = 'published'
                       ORDER BY effective_from, id LIMIT 1"#,
                )
                .bind(academic_term_id)
                .fetch_optional(pool)
                .await?
            }
        }
        _ => {
            sqlx::query_as(
                r#"SELECT id, status
                   FROM academic_timetable_versions
                   WHERE academic_term_id = $1 AND status = 'published'
                   ORDER BY effective_from DESC, id DESC LIMIT 1"#,
            )
            .bind(academic_term_id)
            .fetch_optional(pool)
            .await?
        }
    };
    let timetable_version_id = timetable_version.as_ref().map(|version| version.id);

    let homeroom_rows: Vec<HomeroomDeliveryBaseRow> = sqlx::query_as(
        r#"SELECT homeroom.id AS homeroom_id,
                  homeroom.name AS homeroom_name,
                  grade.id AS grade_level_id,
                  grade.level_type AS grade_level_type,
                  grade.year AS grade_level_year,
                  program.id AS study_program_id,
                  program.code AS study_program_code,
                  program.name_th AS study_program_name,
                  curriculum.id AS curriculum_id,
                  curriculum.name_th AS curriculum_name
           FROM homerooms homeroom
           JOIN grade_levels grade ON grade.id = homeroom.grade_level_id
           JOIN study_programs program ON program.id = homeroom.study_program_id
           JOIN curriculum_versions version ON version.id = program.curriculum_version_id
           JOIN curricula curriculum ON curriculum.id = version.curriculum_id
           WHERE homeroom.academic_year_id = $1
             AND homeroom.is_active
           ORDER BY CASE grade.level_type
                        WHEN 'kindergarten' THEN 1
                        WHEN 'primary' THEN 2
                        WHEN 'secondary' THEN 3
                        ELSE 4
                    END,
                    grade.year,
                    homeroom.name,
                    homeroom.id
           LIMIT $2"#,
    )
    .bind(academic_year_id)
    .bind((MAX_WORKSPACE_HOMEROOMS + 1) as i64)
    .fetch_all(pool)
    .await?;
    ensure_option_size(
        homeroom_rows.len(),
        MAX_WORKSPACE_HOMEROOMS,
        "ห้องประจำชั้นในพื้นที่ทำงาน",
    )?;
    let homeroom_ids = homeroom_rows
        .iter()
        .map(|homeroom| homeroom.homeroom_id)
        .collect::<Vec<_>>();

    let expected_rows: Vec<ExpectedDeliveryRow> = sqlx::query_as(
        r#"SELECT homeroom.id AS homeroom_id,
                  requirement.id AS requirement_id,
                  'course'::text AS resource_kind,
                  requirement.subject_version_id AS catalog_version_id,
                  subject.code,
                  version.name_th AS name,
                  requirement.requirement_kind,
                  version.periods_per_week AS standard_periods_per_week,
                  requirement.display_order
           FROM homerooms homeroom
           JOIN study_programs program ON program.id = homeroom.study_program_id
           JOIN curriculum_course_requirements requirement
             ON requirement.study_program_id = program.id
            AND requirement.grade_level_id = homeroom.grade_level_id
           JOIN curriculum_term_slots slot
             ON slot.id = requirement.term_slot_id
            AND slot.curriculum_version_id = program.curriculum_version_id
            AND slot.term_type = $2
            AND slot.type_occurrence = $3
           JOIN subject_versions version ON version.id = requirement.subject_version_id
           JOIN subjects subject ON subject.id = version.subject_id
           WHERE homeroom.id = ANY($1)
           UNION ALL
           SELECT homeroom.id AS homeroom_id,
                  requirement.id AS requirement_id,
                  'activity'::text AS resource_kind,
                  requirement.activity_version_id AS catalog_version_id,
                  activity.code,
                  version.name,
                  requirement.requirement_kind,
                  NULL::integer AS standard_periods_per_week,
                  requirement.display_order
           FROM homerooms homeroom
           JOIN study_programs program ON program.id = homeroom.study_program_id
           JOIN curriculum_activity_requirements requirement
             ON requirement.study_program_id = program.id
            AND requirement.grade_level_id = homeroom.grade_level_id
           JOIN curriculum_term_slots slot
             ON slot.id = requirement.term_slot_id
            AND slot.curriculum_version_id = program.curriculum_version_id
            AND slot.term_type = $2
            AND slot.type_occurrence = $3
           JOIN activity_versions version ON version.id = requirement.activity_version_id
           JOIN activities activity ON activity.id = version.activity_id
           WHERE homeroom.id = ANY($1)
           ORDER BY homeroom_id, display_order, resource_kind, catalog_version_id
           LIMIT $4"#,
    )
    .bind(&homeroom_ids)
    .bind(&term_type)
    .bind(type_occurrence as i32)
    .bind((MAX_WORKSPACE_ITEMS + 1) as i64)
    .fetch_all(pool)
    .await?;
    ensure_option_size(
        expected_rows.len(),
        MAX_WORKSPACE_ITEMS,
        "รายการตามโครงสร้างในพื้นที่ทำงาน",
    )?;

    let owner_ids = filter.allowed_organization_unit_ids();
    let offering_rows: Vec<OfferingTargetRow> = sqlx::query_as(
        r#"SELECT offering.id AS offering_id,
                  offering.kind AS resource_kind,
                  CASE offering.kind
                      WHEN 'course' THEN course_detail.subject_version_id
                      ELSE activity_detail.activity_version_id
                  END AS catalog_version_id,
                  offering.status,
                  offering.code_snapshot AS code,
                  offering.name_snapshot AS name,
                  timetable_target.weekly_period_target,
                  target.target_kind,
                  target.homeroom_id,
                  target.grade_level_id,
                  target.study_program_id
           FROM learning_offerings offering
           LEFT JOIN course_offering_details course_detail
             ON course_detail.learning_offering_id = offering.id
           LEFT JOIN activity_offering_details activity_detail
             ON activity_detail.learning_offering_id = offering.id
           JOIN learning_offering_targets target
             ON target.learning_offering_id = offering.id
            AND target.academic_term_id = offering.academic_term_id
            AND target.academic_year_id = offering.academic_year_id
           LEFT JOIN academic_timetable_version_targets timetable_target
             ON timetable_target.learning_offering_id = offering.id
            AND timetable_target.timetable_version_id = $5
           WHERE offering.academic_term_id = $1
             AND offering.academic_year_id = $2
             AND ($3 OR offering.owning_organization_unit_id = ANY($4))
           ORDER BY offering.code_snapshot, offering.id, target.id
           LIMIT $6"#,
    )
    .bind(academic_term_id)
    .bind(academic_year_id)
    .bind(filter.includes_school_owned)
    .bind(&owner_ids)
    .bind(timetable_version_id)
    .bind((MAX_WORKSPACE_TARGET_ROWS + 1) as i64)
    .fetch_all(pool)
    .await?;
    ensure_option_size(
        offering_rows.len(),
        MAX_WORKSPACE_TARGET_ROWS,
        "เป้าหมายรายการเปิดสอนในพื้นที่ทำงาน",
    )?;

    let group_rows: Vec<DeliveryGroupRow> = sqlx::query_as(
        r#"SELECT learning_group.id,
                  learning_group.learning_offering_id,
                  learning_group.code,
                  learning_group.name,
                  learning_group.status,
                  learning_group.roster_status,
                  coalesce(
                      array_agg(homeroom.id ORDER BY homeroom.name, homeroom.id)
                          FILTER (WHERE homeroom.id IS NOT NULL),
                      ARRAY[]::uuid[]
                  ) AS homeroom_ids,
                  coalesce(
                      array_agg(homeroom.name ORDER BY homeroom.name, homeroom.id)
                          FILTER (WHERE homeroom.id IS NOT NULL),
                      ARRAY[]::text[]
                  ) AS homeroom_names,
                  (SELECT count(*)::bigint
                   FROM learning_group_teachers teacher
                   JOIN users teacher_user ON teacher_user.id = teacher.teacher_id
                   WHERE teacher.learning_group_id = learning_group.id
                     AND teacher.role = 'primary'
                     AND teacher_user.user_type = 'staff'
                     AND teacher_user.status = 'active') AS primary_teacher_count,
                  (SELECT count(*)::bigint
                   FROM academic_timetable_entries entry
                   WHERE entry.learning_group_id = learning_group.id
                     AND entry.academic_term_id = learning_group.academic_term_id
                     AND entry.academic_year_id = learning_group.academic_year_id
                     AND entry.timetable_version_id = $5
                     AND entry.is_active) AS timetable_entry_count
           FROM learning_groups learning_group
           JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
           LEFT JOIN learning_group_homerooms coverage
             ON coverage.learning_group_id = learning_group.id
           LEFT JOIN homerooms homeroom ON homeroom.id = coverage.homeroom_id
           WHERE learning_group.academic_term_id = $1
             AND learning_group.academic_year_id = $2
             AND ($3 OR offering.owning_organization_unit_id = ANY($4))
           GROUP BY learning_group.id
           ORDER BY learning_group.code, learning_group.id
           LIMIT $6"#,
    )
    .bind(academic_term_id)
    .bind(academic_year_id)
    .bind(filter.includes_school_owned)
    .bind(&owner_ids)
    .bind(timetable_version_id)
    .bind(MAX_WORKSPACE_GROUPS + 1)
    .fetch_all(pool)
    .await?;
    ensure_option_size(
        group_rows.len(),
        MAX_WORKSPACE_GROUPS as usize,
        "กลุ่มเรียนในพื้นที่ทำงาน",
    )?;

    let mut expected_by_homeroom: HashMap<Uuid, Vec<ExpectedDeliveryRow>> = HashMap::new();
    for expected in expected_rows {
        expected_by_homeroom
            .entry(expected.homeroom_id)
            .or_default()
            .push(expected);
    }
    let mut offerings_by_resource: HashMap<(LearningOfferingKind, Uuid), Vec<&OfferingTargetRow>> =
        HashMap::new();
    for offering in &offering_rows {
        offerings_by_resource
            .entry((offering.resource_kind, offering.catalog_version_id))
            .or_default()
            .push(offering);
    }
    let mut groups_by_offering: HashMap<Uuid, Vec<&DeliveryGroupRow>> = HashMap::new();
    for group in &group_rows {
        groups_by_offering
            .entry(group.learning_offering_id)
            .or_default()
            .push(group);
    }

    let mut matched_offering_ids = HashSet::new();
    let mut rooms = Vec::with_capacity(homeroom_rows.len());
    for room in homeroom_rows {
        let grade_level = grade_level_lookup_item(GradeLevelRow {
            id: room.grade_level_id,
            level_type: room.grade_level_type,
            year: room.grade_level_year,
        });
        let homeroom = HomeroomLookupItem {
            id: room.homeroom_id,
            name: room.homeroom_name,
            grade_level: Some(grade_level.name.clone()),
            grade_level_id: Some(grade_level.id),
        };
        let study_program = StudyProgramOption {
            id: room.study_program_id,
            code: room.study_program_code,
            name: room.study_program_name,
            curriculum_id: room.curriculum_id,
            curriculum_name: room.curriculum_name,
        };
        let mut items = Vec::new();
        for expected in expected_by_homeroom
            .remove(&homeroom.id)
            .unwrap_or_default()
        {
            let applicable_offering = offerings_by_resource
                .get(&(expected.resource_kind, expected.catalog_version_id))
                .and_then(|candidates| {
                    candidates.iter().copied().find(|candidate| {
                        candidate.target_kind == "homeroom"
                            && candidate.homeroom_id == Some(homeroom.id)
                            || candidate.target_kind == "grade_program"
                                && candidate.grade_level_id == grade_level.id
                                && candidate.study_program_id == study_program.id
                    })
                });
            let offering_id = applicable_offering.map(|offering| offering.offering_id);
            if let Some(id) = offering_id {
                matched_offering_ids.insert(id);
            }
            let applicable_groups: Vec<&DeliveryGroupRow> = offering_id
                .and_then(|id| groups_by_offering.get(&id))
                .into_iter()
                .flatten()
                .copied()
                .filter(|group| group.homeroom_ids.contains(&homeroom.id))
                .collect();
            let coverage_counts = applicable_groups
                .iter()
                .map(|group| group.homeroom_ids.len() as i64)
                .collect::<Vec<_>>();
            let primary_counts = applicable_groups
                .iter()
                .map(|group| group.primary_teacher_count)
                .collect::<Vec<_>>();
            let timetable_counts = applicable_groups
                .iter()
                .map(|group| group.timetable_entry_count)
                .collect::<Vec<_>>();
            items.push(HomeroomDeliveryItem {
                requirement_id: expected.requirement_id,
                resource_kind: expected.resource_kind,
                catalog_version_id: expected.catalog_version_id,
                code: expected.code,
                name: expected.name,
                requirement_kind: expected.requirement_kind,
                standard_periods_per_week: expected.standard_periods_per_week,
                weekly_period_target: applicable_offering
                    .and_then(|offering| offering.weekly_period_target),
                offering_id,
                offering_state: applicable_offering
                    .map(|offering| offering_state(offering.status))
                    .unwrap_or(HomeroomOfferingState::Missing),
                group_mode: classify_group_mode(expected.requirement_kind, &coverage_counts),
                teacher_state: classify_teacher_state(&primary_counts),
                timetable_state: classify_timetable_state(&timetable_counts),
                groups: applicable_groups
                    .into_iter()
                    .map(delivery_group_summary)
                    .collect(),
            });
        }
        let ready_count = items
            .iter()
            .filter(|item| item.offering_id.is_some() && !item.groups.is_empty())
            .count();
        let blockers = if items.is_empty() {
            vec![DeliveryPrerequisite {
                code: "curriculum_structure_empty".to_string(),
                message: "ยังไม่มีโครงสร้างรายวิชาหรือกิจกรรมสำหรับห้องนี้ในภาคเรียนที่เลือก".to_string(),
                recovery_path: "/staff/academic/curricula".to_string(),
            }]
        } else {
            Vec::new()
        };
        rooms.push(HomeroomDeliveryRoom {
            homeroom,
            grade_level,
            study_program,
            expected_count: items.len(),
            ready_count,
            items,
            blockers,
        });
    }

    let mut unlinked = Vec::new();
    for group in &group_rows {
        if group.homeroom_ids.is_empty() {
            if let Some(offering) = offering_rows
                .iter()
                .find(|offering| offering.offering_id == group.learning_offering_id)
            {
                unlinked.push(UnlinkedDeliveryItem {
                    offering_id: offering.offering_id,
                    group_id: Some(group.id),
                    code: group.code.clone(),
                    name: group.name.clone(),
                    reason: "กลุ่มเรียนยังไม่ได้เชื่อมกับห้องประจำชั้น".to_string(),
                });
            }
        }
    }
    for offering in &offering_rows {
        if !matched_offering_ids.contains(&offering.offering_id)
            && !unlinked
                .iter()
                .any(|item| item.offering_id == offering.offering_id)
        {
            unlinked.push(UnlinkedDeliveryItem {
                offering_id: offering.offering_id,
                group_id: None,
                code: offering.code.clone(),
                name: offering.name.clone(),
                reason: "รายการเปิดสอนไม่ตรงกับโครงสร้างหรือห้องเป้าหมายในภาคเรียนนี้".to_string(),
            });
        }
    }
    unlinked.sort_by(|left, right| left.code.cmp(&right.code).then(left.name.cmp(&right.name)));

    Ok(HomeroomDeliveryWorkspace {
        academic_term_id,
        academic_year_id,
        timetable_version_id,
        timetable_version_status: timetable_version.map(|version| version.status),
        homerooms: rooms,
        unlinked,
    })
}

fn offering_state(status: LearningOfferingStatus) -> HomeroomOfferingState {
    match status {
        LearningOfferingStatus::Draft => HomeroomOfferingState::Draft,
        LearningOfferingStatus::Published => HomeroomOfferingState::Published,
        LearningOfferingStatus::Closed => HomeroomOfferingState::Closed,
    }
}

fn classify_group_mode(
    requirement_kind: RequirementKind,
    homeroom_counts: &[i64],
) -> HomeroomGroupMode {
    if homeroom_counts.is_empty() {
        return match requirement_kind {
            RequirementKind::Required => HomeroomGroupMode::Missing,
            RequirementKind::Elective | RequirementKind::Optional => HomeroomGroupMode::Deferred,
        };
    }
    if homeroom_counts.len() > 1 {
        HomeroomGroupMode::Split
    } else if homeroom_counts[0] > 1 {
        HomeroomGroupMode::Combined
    } else {
        HomeroomGroupMode::Normal
    }
}

fn classify_teacher_state(primary_teacher_counts: &[i64]) -> HomeroomTeacherState {
    if !primary_teacher_counts.is_empty() && primary_teacher_counts.iter().all(|count| *count > 0) {
        HomeroomTeacherState::Assigned
    } else {
        HomeroomTeacherState::MissingPrimary
    }
}

fn classify_timetable_state(timetable_entry_counts: &[i64]) -> HomeroomTimetableState {
    if !timetable_entry_counts.is_empty() && timetable_entry_counts.iter().all(|count| *count > 0) {
        HomeroomTimetableState::Scheduled
    } else if timetable_entry_counts.iter().any(|count| *count > 0) {
        HomeroomTimetableState::PartlyScheduled
    } else {
        HomeroomTimetableState::Unscheduled
    }
}

fn delivery_group_summary(group: &DeliveryGroupRow) -> HomeroomDeliveryGroupSummary {
    HomeroomDeliveryGroupSummary {
        id: group.id,
        code: group.code.clone(),
        name: group.name.clone(),
        status: group.status,
        teachers_locked: group.status != LearningOfferingStatus::Draft,
        roster_status: group.roster_status,
        homeroom_ids: group.homeroom_ids.clone(),
        homeroom_names: group.homeroom_names.clone(),
        primary_teacher_count: group.primary_teacher_count,
        timetable_entry_count: group.timetable_entry_count,
    }
}

pub async fn delivery_overview(
    pool: &PgPool,
    academic_term_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<LearningDeliveryOverview, AppError> {
    let offerings =
        offerings::list(pool, LearningOfferingQuery { academic_term_id }, filter).await?;
    if offerings.is_empty() {
        return Ok(LearningDeliveryOverview {
            academic_term_id,
            offerings: Vec::new(),
        });
    }

    let offering_ids: Vec<Uuid> = offerings.iter().map(|offering| offering.id).collect();
    let grade_level_ids: Vec<Uuid> = offerings
        .iter()
        .flat_map(|offering| offering.targets.iter().map(|target| target.grade_level_id))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let study_program_ids: Vec<Uuid> = offerings
        .iter()
        .flat_map(|offering| {
            offering
                .targets
                .iter()
                .map(|target| target.study_program_id)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let grade_rows: Vec<GradeLevelRow> = sqlx::query_as(
        r#"
        SELECT id, level_type, year
        FROM grade_levels
        WHERE id = ANY($1)
        ORDER BY CASE level_type
            WHEN 'kindergarten' THEN 1
            WHEN 'primary' THEN 2
            WHEN 'secondary' THEN 3
            ELSE 4
        END, year, id
        "#,
    )
    .bind(&grade_level_ids)
    .fetch_all(pool)
    .await?;
    let grades_by_id: HashMap<Uuid, GradeLevelLookupItem> = grade_rows
        .into_iter()
        .map(grade_level_lookup_item)
        .map(|item| (item.id, item))
        .collect();

    let program_rows: Vec<StudyProgramOption> = sqlx::query_as(
        r#"
        SELECT program.id, program.code, program.name_th AS name,
               curriculum.id AS curriculum_id, curriculum.name_th AS curriculum_name
        FROM study_programs program
        JOIN curriculum_versions version ON version.id = program.curriculum_version_id
        JOIN curricula curriculum ON curriculum.id = version.curriculum_id
        WHERE program.id = ANY($1)
        ORDER BY curriculum.code, program.code, program.id
        "#,
    )
    .bind(&study_program_ids)
    .fetch_all(pool)
    .await?;
    let programs_by_id: HashMap<Uuid, StudyProgramOption> = program_rows
        .into_iter()
        .map(|item| (item.id, item))
        .collect();

    if grades_by_id.len() != grade_level_ids.len()
        || programs_by_id.len() != study_program_ids.len()
    {
        return Err(AppError::InternalServerError(
            "ไม่สามารถแสดงชื่อระดับชั้นหรือแผนการเรียนของรายการเปิดสอนได้".to_string(),
        ));
    }

    let aggregates: Vec<OfferingAggregateRow> = sqlx::query_as(
        r#"
        WITH selected_offerings AS (
            SELECT unnest($1::uuid[]) AS learning_offering_id
        ),
        group_summary AS (
            SELECT learning_group.learning_offering_id,
                   count(*)::bigint AS group_count,
                   count(*) FILTER (
                       WHERE NOT EXISTS (
                           SELECT 1
                           FROM learning_group_teachers primary_teacher
                           JOIN users teacher ON teacher.id = primary_teacher.teacher_id
                           WHERE primary_teacher.learning_group_id = learning_group.id
                             AND primary_teacher.role = 'primary'
                             AND teacher.user_type = 'staff'
                             AND teacher.status = 'active'
                       )
                   )::bigint AS groups_without_primary_teacher,
                   count(*) FILTER (
                       WHERE learning_group.roster_status = 'published'
                   )::bigint AS published_roster_count
            FROM learning_groups learning_group
            WHERE learning_group.learning_offering_id = ANY($1)
            GROUP BY learning_group.learning_offering_id
        ),
        teacher_summary AS (
            SELECT learning_group.learning_offering_id,
                   count(*)::bigint AS teacher_assignment_count
            FROM learning_groups learning_group
            JOIN learning_group_teachers teacher
              ON teacher.learning_group_id = learning_group.id
            WHERE learning_group.learning_offering_id = ANY($1)
            GROUP BY learning_group.learning_offering_id
        )
        SELECT selected.learning_offering_id,
               coalesce(groups.group_count, 0)::bigint AS group_count,
               coalesce(teachers.teacher_assignment_count, 0)::bigint
                   AS teacher_assignment_count,
               coalesce(groups.groups_without_primary_teacher, 0)::bigint
                   AS groups_without_primary_teacher,
               coalesce(groups.published_roster_count, 0)::bigint AS published_roster_count
        FROM selected_offerings selected
        LEFT JOIN group_summary groups
          ON groups.learning_offering_id = selected.learning_offering_id
        LEFT JOIN teacher_summary teachers
          ON teachers.learning_offering_id = selected.learning_offering_id
        ORDER BY selected.learning_offering_id
        "#,
    )
    .bind(&offering_ids)
    .fetch_all(pool)
    .await?;
    let total_groups: i64 = aggregates.iter().map(|item| item.group_count).sum();
    if total_groups > MAX_WORKSPACE_GROUPS {
        return Err(AppError::ValidationError(
            "จำนวนกลุ่มเรียนในพื้นที่ทำงานเกิน 2000 กลุ่ม กรุณาแบ่งข้อมูลก่อนเปิดพื้นที่ทำงาน".to_string(),
        ));
    }
    let mut aggregates_by_id: HashMap<Uuid, OfferingAggregateRow> = aggregates
        .into_iter()
        .map(|item| (item.learning_offering_id, item))
        .collect();

    let mut overview_items = Vec::with_capacity(offerings.len());
    for offering in offerings {
        let aggregate = aggregates_by_id.remove(&offering.id).ok_or_else(|| {
            AppError::InternalServerError("ไม่สามารถสรุปความพร้อมของรายการเปิดสอนได้".to_string())
        })?;
        let mut offering_grade_ids: Vec<Uuid> = offering
            .targets
            .iter()
            .map(|target| target.grade_level_id)
            .collect();
        offering_grade_ids.sort_unstable();
        offering_grade_ids.dedup();
        let mut grade_levels: Vec<GradeLevelLookupItem> = offering_grade_ids
            .into_iter()
            .filter_map(|id| grades_by_id.get(&id).cloned())
            .collect();
        grade_levels.sort_by_key(|item| (item.level_order, item.id));

        let mut offering_program_ids: Vec<Uuid> = offering
            .targets
            .iter()
            .map(|target| target.study_program_id)
            .collect();
        offering_program_ids.sort_unstable();
        offering_program_ids.dedup();
        let mut study_programs: Vec<StudyProgramOption> = offering_program_ids
            .into_iter()
            .filter_map(|id| programs_by_id.get(&id).cloned())
            .collect();
        study_programs.sort_by(|left, right| {
            left.curriculum_name
                .cmp(&right.curriculum_name)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.id.cmp(&right.id))
        });

        overview_items.push(LearningOfferingOverviewItem {
            offering,
            grade_levels,
            study_programs,
            group_count: aggregate.group_count,
            teacher_assignment_count: aggregate.teacher_assignment_count,
            groups_without_primary_teacher: aggregate.groups_without_primary_teacher,
            published_roster_count: aggregate.published_roster_count,
        });
    }

    Ok(LearningDeliveryOverview {
        academic_term_id,
        offerings: overview_items,
    })
}

pub async fn delivery_management_options(
    pool: &PgPool,
    academic_term_id: Uuid,
    actor_user_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<DeliveryManagementOptions, AppError> {
    let (academic_year_id, term_start): (Uuid, chrono::NaiveDate) =
        sqlx::query_as("SELECT academic_year_id, start_date FROM academic_terms WHERE id = $1")
            .bind(academic_term_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนที่เลือก".to_string()))?;
    let owner_ids = filter.allowed_organization_unit_ids();
    let catalog_rows: Vec<CatalogVersionRow> = sqlx::query_as(
        r#"
        SELECT option.id, option.kind, option.code, option.name, option.version_no
        FROM (
            SELECT version.id, 'course'::text AS kind, subject.code,
                   version.name_th AS name, version.version_no
            FROM subject_versions version
            JOIN subjects subject ON subject.id = version.subject_id
            WHERE version.status = 'published'
              AND version.effective_from <= $1
              AND (version.effective_until IS NULL OR version.effective_until > $1)
              AND ($2 OR subject.owning_organization_unit_id = ANY($3))
            UNION ALL
            SELECT version.id, 'activity'::text AS kind, activity.code,
                   version.name, version.version_no
            FROM activity_versions version
            JOIN activities activity ON activity.id = version.activity_id
            WHERE version.status = 'published'
              AND version.effective_from <= $1
              AND (version.effective_until IS NULL OR version.effective_until > $1)
              AND ($2 OR activity.owning_organization_unit_id = ANY($3))
        ) option
        ORDER BY option.kind, option.code, option.version_no DESC, option.id
        LIMIT $4
        "#,
    )
    .bind(term_start)
    .bind(filter.includes_school_owned)
    .bind(&owner_ids)
    .bind((MAX_CATALOG_OPTIONS + 1) as i64)
    .fetch_all(pool)
    .await?;
    ensure_option_size(
        catalog_rows.len(),
        MAX_CATALOG_OPTIONS,
        "รายการวิชาและกิจกรรม",
    )?;
    let catalog_versions = catalog_rows
        .into_iter()
        .map(|row| DeliveryCatalogVersionOption {
            id: row.id,
            kind: row.kind,
            label: format!("{} — {} (ฉบับ {})", row.code, row.name, row.version_no),
            code: row.code,
            name: row.name,
            version_no: row.version_no,
        })
        .collect();

    let academic_lookup = || AcademicLookupQuery {
        academic_year_id,
        active_only: Some(true),
        search: None,
        limit: Some(MAX_LOOKUP_OPTIONS as i32),
        level_type: None,
        subject_type: None,
    };
    let grade_rows: Vec<GradeLevelRow> = sqlx::query_as(
        r#"
        SELECT id, level_type, year
        FROM grade_levels
        WHERE is_active
        ORDER BY CASE level_type
            WHEN 'kindergarten' THEN 1
            WHEN 'primary' THEN 2
            WHEN 'secondary' THEN 3
            ELSE 4
        END, year, id
        LIMIT $1
        "#,
    )
    .bind((MAX_LOOKUP_OPTIONS + 1) as i64)
    .fetch_all(pool)
    .await?;
    ensure_option_size(grade_rows.len(), MAX_LOOKUP_OPTIONS, "ระดับชั้น")?;
    let grade_levels = grade_rows
        .into_iter()
        .map(grade_level_lookup_item)
        .collect();
    let study_programs =
        curriculum::list_study_program_options_for_year(pool, academic_year_id, filter).await?;
    let homerooms = lookup_services::lookup_homerooms(pool, academic_lookup()).await?;
    let lookup_query = || LookupQuery {
        active_only: Some(true),
        search: None,
        limit: Some(MAX_LOOKUP_OPTIONS as i32),
        member_only: Some(false),
    };
    let organization_units =
        lookup_services::lookup_organization_units(pool, actor_user_id, lookup_query())
            .await?
            .into_iter()
            .filter(|unit| learning_offering_owner_allowed(filter, unit.id))
            .collect();
    let teachers = lookup_services::lookup_staff(pool, lookup_query()).await?;
    let rooms = lookup_services::lookup_rooms(pool).await?;
    ensure_option_size(rooms.len(), MAX_LOOKUP_OPTIONS, "ห้องเรียน")?;

    Ok(DeliveryManagementOptions {
        academic_term_id,
        academic_year_id,
        catalog_versions,
        grade_levels,
        study_programs,
        organization_units,
        homerooms,
        teachers,
        rooms,
    })
}

fn ensure_option_size(actual: usize, maximum: usize, label: &str) -> Result<(), AppError> {
    if actual > maximum {
        Err(AppError::ValidationError(format!(
            "จำนวนตัวเลือก{label}เกิน {maximum} รายการ กรุณาลดข้อมูลก่อนเปิดตัวเลือก"
        )))
    } else {
        Ok(())
    }
}

fn grade_level_lookup_item(row: GradeLevelRow) -> GradeLevelLookupItem {
    let (name, code, short_name, order_base) = match row.level_type.as_str() {
        "kindergarten" => (
            format!("อนุบาลปีที่ {}", row.year),
            format!("K{}", row.year),
            format!("อ.{}", row.year),
            1,
        ),
        "primary" => (
            format!("ประถมศึกษาปีที่ {}", row.year),
            format!("P{}", row.year),
            format!("ป.{}", row.year),
            2,
        ),
        "secondary" => (
            format!("มัธยมศึกษาปีที่ {}", row.year),
            format!("M{}", row.year),
            format!("ม.{}", row.year),
            3,
        ),
        _ => (
            format!("Other {}", row.year),
            format!("O{}", row.year),
            format!("?{}", row.year),
            4,
        ),
    };
    GradeLevelLookupItem {
        id: row.id,
        code,
        name,
        short_name: Some(short_name),
        level_type: row.level_type,
        level_order: order_base * 100 + row.year,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::core::models::RequirementKind;
    use crate::modules::academic::delivery::models::{
        HomeroomGroupMode, HomeroomTeacherState, HomeroomTimetableState,
    };

    #[test]
    fn grade_level_labels_are_human_readable_and_stably_ordered() {
        let item = grade_level_lookup_item(GradeLevelRow {
            id: Uuid::nil(),
            level_type: "secondary".to_string(),
            year: 2,
        });
        assert_eq!(item.name, "มัธยมศึกษาปีที่ 2");
        assert_eq!(item.short_name.as_deref(), Some("ม.2"));
        assert_eq!(item.level_order, 302);
    }

    #[test]
    fn group_mode_exposes_missing_deferred_combined_and_split_states() {
        assert_eq!(
            classify_group_mode(RequirementKind::Required, &[]),
            HomeroomGroupMode::Missing
        );
        assert_eq!(
            classify_group_mode(RequirementKind::Elective, &[]),
            HomeroomGroupMode::Deferred
        );
        assert_eq!(
            classify_group_mode(RequirementKind::Required, &[1]),
            HomeroomGroupMode::Normal
        );
        assert_eq!(
            classify_group_mode(RequirementKind::Required, &[2]),
            HomeroomGroupMode::Combined
        );
        assert_eq!(
            classify_group_mode(RequirementKind::Required, &[1, 1]),
            HomeroomGroupMode::Split
        );
    }

    #[test]
    fn staffing_and_timetable_states_require_every_applicable_group() {
        assert_eq!(
            classify_teacher_state(&[1, 2]),
            HomeroomTeacherState::Assigned
        );
        assert_eq!(
            classify_teacher_state(&[1, 0]),
            HomeroomTeacherState::MissingPrimary
        );
        assert_eq!(
            classify_timetable_state(&[2, 1]),
            HomeroomTimetableState::Scheduled
        );
        assert_eq!(
            classify_timetable_state(&[2, 0]),
            HomeroomTimetableState::PartlyScheduled
        );
        assert_eq!(
            classify_timetable_state(&[0, 0]),
            HomeroomTimetableState::Unscheduled
        );
    }
}
