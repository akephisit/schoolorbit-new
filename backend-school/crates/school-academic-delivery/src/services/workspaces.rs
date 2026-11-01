use school_database::DbSession;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::time::{Duration, Instant};

use sqlx::PgPool;
use uuid::Uuid;

use school_academic_core::models::{
    GradeLevelLookupItem, HomeroomLookupItem, RequirementKind, StudyProgramOption,
};
use school_academic_core::services::curriculum;
use school_authorization::AcademicResourceListFilter;
use school_errors::AppError;

use super::super::models::{
    ActivitySchedulingMode, CurriculumDeliveryAlignmentState, CurriculumDeliveryExtraOffering,
    DeliveryCatalogVersionOption, DeliveryManagementOptions, DeliveryPrerequisite,
    HomeroomDeliveryGroupSummary, HomeroomDeliveryItem, HomeroomDeliveryRoom,
    HomeroomDeliveryWorkspace, HomeroomGroupMode, HomeroomOfferingState, HomeroomTeacherState,
    LearningDeliveryOverview, LearningOfferingKind, LearningOfferingOverviewItem,
    LearningOfferingQuery, LearningOfferingStatus, Room, RosterStatus, StaffLookupItem,
    UnlinkedDeliveryItem,
};
use super::{groups, offerings};

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
struct HomeroomOptionRow {
    id: Uuid,
    name: String,
    level_type: Option<String>,
    year: Option<i32>,
    grade_level_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
struct StaffOptionRow {
    id: Uuid,
    title: Option<String>,
    display_name: String,
}

#[derive(Debug)]
struct OfferingAggregateRow {
    group_count: i64,
    teacher_assignment_count: i64,
    groups_without_primary_teacher: i64,
    published_roster_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct OverviewGroupRow {
    id: Uuid,
    learning_offering_id: Uuid,
    status: LearningOfferingStatus,
    roster_status: RosterStatus,
    registry_teacher_count: i64,
    registry_has_primary: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct CatalogVersionRow {
    subject_type: Option<String>,
    id: Uuid,
    scheduling_mode: Option<ActivitySchedulingMode>,
    kind: LearningOfferingKind,
    code: String,
    name: String,
    version_no: i32,
    standard_periods_per_week: Option<i32>,
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
    study_program_display_order: i32,
    study_program_name: String,
    edition_id: Uuid,
    level_name: String,
    curriculum_level_id: Uuid,
    edition_name: String,
    revision_year: Option<i32>,
    grade_level_ids: Vec<Uuid>,
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
    subject_type: Option<String>,
    standard_periods_per_week: Option<i32>,
    scheduling_mode: Option<ActivitySchedulingMode>,
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
    starts_on: Option<chrono::NaiveDate>,
    ends_on: Option<chrono::NaiveDate>,
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
}

#[derive(Debug, sqlx::FromRow)]
struct WorkspaceDeliveryVersionRow {
    id: Uuid,
    status: super::super::models::versions::DeliveryVersionStatus,
    effective_from: chrono::NaiveDate,
}

#[derive(Debug, sqlx::FromRow)]
struct WorkspaceContextRow {
    term_type: String,
    type_occurrence: i64,
    delivery_version_id: Option<Uuid>,
    delivery_version_status: Option<super::super::models::versions::DeliveryVersionStatus>,
    delivery_version_effective_from: Option<chrono::NaiveDate>,
}

#[derive(Clone, Copy, Debug)]
pub struct HomeroomDeliveryTiming {
    pub context_and_homerooms: Duration,
    pub resources: Duration,
    pub assembly: Duration,
}

async fn join_context_and_homeroom_reads<Context, Homerooms>(
    context: Context,
    homerooms: Homerooms,
) -> Result<(Option<WorkspaceContextRow>, Vec<HomeroomDeliveryBaseRow>), sqlx::Error>
where
    Context: Future<Output = Result<Option<WorkspaceContextRow>, sqlx::Error>>,
    Homerooms: Future<Output = Result<Vec<HomeroomDeliveryBaseRow>, sqlx::Error>>,
{
    futures::try_join!(context, homerooms)
}

async fn join_workspace_resource_reads<ExpectedRows, OfferingRows, GroupRows>(
    expected_rows: ExpectedRows,
    offering_rows: OfferingRows,
    group_rows: GroupRows,
) -> Result<
    (
        Vec<ExpectedDeliveryRow>,
        Vec<OfferingTargetRow>,
        Vec<DeliveryGroupRow>,
    ),
    sqlx::Error,
>
where
    ExpectedRows: Future<Output = Result<Vec<ExpectedDeliveryRow>, sqlx::Error>>,
    OfferingRows: Future<Output = Result<Vec<OfferingTargetRow>, sqlx::Error>>,
    GroupRows: Future<Output = Result<Vec<DeliveryGroupRow>, sqlx::Error>>,
{
    futures::try_join!(expected_rows, offering_rows, group_rows)
}

pub async fn homeroom_delivery_workspace_for_version(
    pool: &PgPool,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    requested_delivery_version_id: Option<Uuid>,
    filter: &AcademicResourceListFilter,
) -> Result<HomeroomDeliveryWorkspace, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.workspaces.homeroom_delivery_workspace_for_version",
    )
    .await?;
    homeroom_delivery_workspace_for_version_on_connection(
        &mut connection,
        academic_year_id,
        academic_term_id,
        requested_delivery_version_id,
        filter,
    )
    .await
}

pub async fn homeroom_delivery_workspace_for_version_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    requested_delivery_version_id: Option<Uuid>,
    filter: &AcademicResourceListFilter,
) -> Result<HomeroomDeliveryWorkspace, AppError> {
    Ok(homeroom_delivery_workspace_with_timing_on_connection(
        connection,
        academic_year_id,
        academic_term_id,
        requested_delivery_version_id,
        filter,
    )
    .await?
    .0)
}

pub async fn homeroom_delivery_workspace_with_timing(
    pool: &PgPool,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    requested_delivery_version_id: Option<Uuid>,
    filter: &AcademicResourceListFilter,
) -> Result<(HomeroomDeliveryWorkspace, HomeroomDeliveryTiming), AppError> {
    let started = Instant::now();
    // Each independent branch releases its checkout before the join completes, including with a one-connection pool.
    let (context, homerooms) = join_context_and_homeroom_reads(
        async {
            let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
                pool,
                "academic_delivery.workspaces.context",
            )
            .await?;
            load_workspace_context(
                &mut connection,
                academic_year_id,
                academic_term_id,
                requested_delivery_version_id,
            )
            .await
        },
        async {
            let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
                pool,
                "academic_delivery.workspaces.homerooms",
            )
            .await?;
            load_workspace_homeroom_rows(&mut connection, academic_year_id).await
        },
    )
    .await?;
    let elapsed = started.elapsed();
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.workspaces.finish",
    )
    .await?;
    finish_homeroom_delivery_workspace(
        &mut connection,
        academic_year_id,
        academic_term_id,
        requested_delivery_version_id,
        filter,
        context,
        homerooms,
        elapsed,
    )
    .await
}

pub async fn homeroom_delivery_workspace_with_timing_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    requested_delivery_version_id: Option<Uuid>,
    filter: &AcademicResourceListFilter,
) -> Result<(HomeroomDeliveryWorkspace, HomeroomDeliveryTiming), AppError> {
    let started = Instant::now();
    let context = load_workspace_context(
        connection,
        academic_year_id,
        academic_term_id,
        requested_delivery_version_id,
    )
    .await?;
    let homerooms = load_workspace_homeroom_rows(connection, academic_year_id).await?;
    finish_homeroom_delivery_workspace(
        connection,
        academic_year_id,
        academic_term_id,
        requested_delivery_version_id,
        filter,
        context,
        homerooms,
        started.elapsed(),
    )
    .await
}

async fn load_workspace_context(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    requested_delivery_version_id: Option<Uuid>,
) -> Result<Option<WorkspaceContextRow>, sqlx::Error> {
    DbSession::new(connection).query_as("academic_delivery.services.workspaces.homeroom_delivery_workspace_with_timing_on_connection",
        r#"WITH selected_term AS (
               SELECT selected.id,
                      selected.academic_year_id,
                      selected.term_type,
                      count(sibling.id)::bigint AS type_occurrence,
                      selected.status AS term_status
               FROM academic_terms selected
               JOIN academic_terms sibling
                 ON sibling.academic_year_id = selected.academic_year_id
                AND sibling.term_type = selected.term_type
                AND sibling.sequence_no <= selected.sequence_no
               WHERE selected.id = $1
                 AND selected.academic_year_id = $2
               GROUP BY selected.id, selected.academic_year_id,
                        selected.term_type, selected.status
           )
           SELECT term.term_type,
                  term.type_occurrence,
                  version.id AS delivery_version_id,
                  version.status AS delivery_version_status,
                  COALESCE(version.effective_from,version.reference_date) AS delivery_version_effective_from
           FROM selected_term term
           LEFT JOIN LATERAL (
               SELECT candidate.id, candidate.status, candidate.effective_from,candidate.reference_date
               FROM academic_delivery_versions candidate
               WHERE candidate.academic_term_id = term.id
                 AND candidate.academic_year_id = term.academic_year_id
                 AND (
                     ($3::uuid IS NOT NULL AND candidate.id = $3)
                     OR ($3::uuid IS NULL AND candidate.status IN ('draft', 'published'))
                 )
               ORDER BY CASE WHEN candidate.status='draft' THEN 0 ELSE 1 END,
                   candidate.publication_no DESC NULLS FIRST,candidate.created_at DESC,candidate.id
               LIMIT 1
           ) version ON TRUE"#,
    )
    .bind(academic_term_id)
    .bind(academic_year_id)
    .bind(requested_delivery_version_id)
    .fetch_optional().await
}

async fn load_workspace_homeroom_rows(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
) -> Result<Vec<HomeroomDeliveryBaseRow>, sqlx::Error> {
    DbSession::new(connection).query_as("academic_delivery.services.workspaces.homeroom_delivery_workspace_with_timing_on_connection",
        r#"SELECT homeroom.id AS homeroom_id,
                  homeroom.name AS homeroom_name,
                  grade.id AS grade_level_id,
                  grade.level_type AS grade_level_type,
                  grade.year AS grade_level_year,
                  program.id AS study_program_id,
                  program.code AS study_program_code,
                  program.display_order AS study_program_display_order,
                  program.name_th AS study_program_name,
                  curriculum.id AS edition_id,
                  version.name_th AS level_name,
                  version.id AS curriculum_level_id,
                  curriculum.name AS edition_name, curriculum.revision_year,
                  ARRAY(SELECT DISTINCT requirement.grade_level_id FROM (
                      SELECT grade_level_id FROM published_curriculum_course_requirements WHERE study_program_id = program.id
                      UNION SELECT grade_level_id FROM published_curriculum_activity_requirements WHERE study_program_id = program.id
                  ) requirement ORDER BY requirement.grade_level_id) AS grade_level_ids
           FROM homerooms homeroom
           JOIN grade_levels grade ON grade.id = homeroom.grade_level_id
           JOIN published_study_programs program ON program.id = homeroom.study_program_id
           JOIN published_curriculum_levels version ON version.id = program.curriculum_level_id
           JOIN curriculum_editions curriculum ON curriculum.id=version.edition_id
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
    .fetch_all().await
}

async fn load_workspace_expected_rows(
    connection: &mut sqlx::PgConnection,
    homeroom_ids: &[Uuid],
    term_type: &str,
    type_occurrence: i64,
) -> Result<Vec<ExpectedDeliveryRow>, sqlx::Error> {
    DbSession::new(connection).query_as("academic_delivery.services.workspaces.homeroom_delivery_workspace_with_timing_on_connection",
        r#"SELECT homeroom.id AS homeroom_id,
                  requirement.id AS requirement_id,
                  'course'::text AS resource_kind,
                  requirement.subject_version_id AS catalog_version_id,
                  subject.code,
                  version.name_th AS name,
                  requirement.requirement_kind,
                  version.type AS subject_type,
                  version.periods_per_week AS standard_periods_per_week,
                  NULL::text AS scheduling_mode,
                  requirement.display_order
           FROM homerooms homeroom
           JOIN published_study_programs program ON program.id = homeroom.study_program_id
           JOIN published_curriculum_course_requirements requirement
             ON requirement.study_program_id = program.id
            AND requirement.grade_level_id = homeroom.grade_level_id
           JOIN published_curriculum_term_slots slot
             ON slot.id = requirement.term_slot_id
            AND slot.curriculum_level_id = program.curriculum_level_id
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
                  NULL::text AS subject_type,
                  NULL::integer AS standard_periods_per_week,
                  version.scheduling_mode,
                  requirement.display_order
           FROM homerooms homeroom
           JOIN published_study_programs program ON program.id = homeroom.study_program_id
           JOIN published_curriculum_activity_requirements requirement
             ON requirement.study_program_id = program.id
            AND requirement.grade_level_id = homeroom.grade_level_id
           JOIN published_curriculum_term_slots slot
             ON slot.id = requirement.term_slot_id
            AND slot.curriculum_level_id = program.curriculum_level_id
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
    .fetch_all().await
}

async fn finish_homeroom_delivery_workspace(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    requested_delivery_version_id: Option<Uuid>,
    filter: &AcademicResourceListFilter,
    context: Option<WorkspaceContextRow>,
    homeroom_rows: Vec<HomeroomDeliveryBaseRow>,
    context_and_homerooms: Duration,
) -> Result<(HomeroomDeliveryWorkspace, HomeroomDeliveryTiming), AppError> {
    let context =
        context.ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนในปีการศึกษาที่เลือก".to_string()))?;
    if requested_delivery_version_id.is_some() && context.delivery_version_id.is_none() {
        return Err(AppError::ValidationError(
            "รุ่นเปิดสอนที่เลือกไม่ได้อยู่ในปีการศึกษาและภาคเรียนนี้".to_string(),
        ));
    }
    let delivery_version = match context.delivery_version_id {
        Some(id) => Some(WorkspaceDeliveryVersionRow {
            id,
            status: context
                .delivery_version_status
                .ok_or_else(|| AppError::InternalServerError("สถานะรุ่นเปิดสอนไม่สมบูรณ์".to_string()))?,
            effective_from: context.delivery_version_effective_from.ok_or_else(|| {
                AppError::InternalServerError("วันที่มีผลของรุ่นเปิดสอนไม่สมบูรณ์".to_string())
            })?,
        }),
        None => None,
    };
    let term_type = context.term_type;
    let type_occurrence = context.type_occurrence;
    let delivery_version_id = delivery_version.as_ref().map(|version| version.id);
    let delivery_version_effective_from = delivery_version.as_ref().and_then(|version| {
        (version.status == crate::models::versions::DeliveryVersionStatus::Published)
            .then_some(version.effective_from)
    });

    ensure_option_size(
        homeroom_rows.len(),
        MAX_WORKSPACE_HOMEROOMS,
        "ห้องประจำชั้นในพื้นที่ทำงาน",
    )?;
    let homeroom_ids = homeroom_rows
        .iter()
        .map(|homeroom| homeroom.homeroom_id)
        .collect::<Vec<_>>();

    let resources_started_at = Instant::now();

    let source = match delivery_version_id {
        Some(id) => {
            Some(super::versions::visible_version_on_connection(connection, id, filter).await?)
        }
        None => None,
    };
    let source_group_ids = source
        .as_ref()
        .map(|source| {
            source
                .snapshot
                .offerings
                .iter()
                .flat_map(|offering| offering.groups.iter().map(|group| group.id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let roster_statuses =
        super::versions::roster_statuses_on_connection(connection, &source_group_ids).await?;
    let source_status = source.as_ref().map(|source| match source.status {
        super::super::models::versions::DeliveryVersionStatus::Draft => {
            LearningOfferingStatus::Draft
        }
        super::super::models::versions::DeliveryVersionStatus::Published => {
            LearningOfferingStatus::Published
        }
        super::super::models::versions::DeliveryVersionStatus::Cancelled => {
            LearningOfferingStatus::Cancelled
        }
    });
    let mut source_offerings = Vec::new();
    let mut source_groups = Vec::new();
    let homeroom_names = homeroom_rows
        .iter()
        .map(|room| (room.homeroom_id, room.homeroom_name.clone()))
        .collect::<HashMap<_, _>>();
    if let Some(source) = &source {
        let status = source_status
            .ok_or_else(|| AppError::InternalServerError("สถานะรุ่นเปิดสอนไม่ครบ".into()))?;
        for offering in &source.snapshot.offerings {
            let catalog_version_id = match &offering.catalog {
                super::super::models::LearningOfferingSnapshot::Course(course) => {
                    course.subject_version_id
                }
                super::super::models::LearningOfferingSnapshot::Activity(activity) => {
                    activity.activity_version_id
                }
            };
            for target in &offering.targets {
                source_offerings.push(OfferingTargetRow {
                    offering_id: offering.id,
                    resource_kind: offering.kind,
                    catalog_version_id,
                    status,
                    code: offering.code.clone(),
                    name: offering.name.clone(),
                    weekly_period_target: Some(offering.weekly_period_target),
                    starts_on: source.effective_from,
                    ends_on: source.effective_until,
                    target_kind: match target.target_kind {
                        super::super::models::OfferingTargetKind::Homeroom => "homeroom",
                        super::super::models::OfferingTargetKind::GradeProgram => "grade_program",
                    }
                    .into(),
                    homeroom_id: target.homeroom_id,
                    grade_level_id: target.grade_level_id,
                    study_program_id: target.study_program_id,
                });
            }
            for group in &offering.groups {
                source_groups.push(DeliveryGroupRow {
                    id: group.id,
                    learning_offering_id: offering.id,
                    code: group.code.clone(),
                    name: group.name.clone(),
                    status,
                    roster_status: *roster_statuses
                        .get(&group.id)
                        .ok_or_else(|| AppError::Conflict("ไม่พบทะเบียนกลุ่มเรียนของรุ่นเปิดสอน".into()))?,
                    homeroom_ids: group.homeroom_ids.clone(),
                    homeroom_names: group
                        .homeroom_ids
                        .iter()
                        .filter_map(|id| homeroom_names.get(id).cloned())
                        .collect(),
                    primary_teacher_count: group
                        .teachers
                        .iter()
                        .filter(|teacher| {
                            teacher.role == super::super::models::LearningTeacherRole::Primary
                        })
                        .count() as i64,
                });
            }
        }
    }
    let offering_rows = async { Ok::<_, sqlx::Error>(source_offerings) };
    let group_rows = async { Ok::<_, sqlx::Error>(source_groups) };

    let (expected_rows, offering_rows, group_rows) = join_workspace_resource_reads(
        load_workspace_expected_rows(connection, &homeroom_ids, &term_type, type_occurrence),
        offering_rows,
        group_rows,
    )
    .await?;
    let resources = resources_started_at.elapsed();
    ensure_option_size(
        expected_rows.len(),
        MAX_WORKSPACE_ITEMS,
        "รายการตามโครงสร้างในพื้นที่ทำงาน",
    )?;
    ensure_option_size(
        offering_rows.len(),
        MAX_WORKSPACE_TARGET_ROWS,
        "เป้าหมายรายการเปิดสอนในพื้นที่ทำงาน",
    )?;
    ensure_option_size(
        group_rows.len(),
        MAX_WORKSPACE_GROUPS as usize,
        "กลุ่มเรียนในพื้นที่ทำงาน",
    )?;

    let assembly_started_at = Instant::now();
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
            display_order: room.study_program_display_order,
            name: room.study_program_name,
            edition_id: room.edition_id,
            level_name: room.level_name,
            curriculum_level_id: room.curriculum_level_id,
            edition_name: room.edition_name,
            revision_year: room.revision_year,
            grade_level_ids: room.grade_level_ids,
        };
        let mut items = Vec::new();
        let expected_rows_for_room = expected_by_homeroom
            .remove(&homeroom.id)
            .unwrap_or_default();
        let expected_resources = expected_rows_for_room
            .iter()
            .map(|expected| (expected.resource_kind, expected.catalog_version_id))
            .collect::<HashSet<_>>();
        for expected in expected_rows_for_room {
            let applicable_offering = offerings_by_resource
                .get(&(expected.resource_kind, expected.catalog_version_id))
                .and_then(|candidates| {
                    select_applicable_offering(
                        candidates,
                        homeroom.id,
                        grade_level.id,
                        study_program.id,
                        delivery_version_effective_from,
                    )
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
            items.push(HomeroomDeliveryItem {
                requirement_id: expected.requirement_id,
                resource_kind: expected.resource_kind,
                catalog_version_id: expected.catalog_version_id,
                code: expected.code,
                name: expected.name,
                requirement_kind: expected.requirement_kind,
                subject_type: expected.subject_type,
                standard_periods_per_week: expected.standard_periods_per_week,
                weekly_period_target: applicable_offering
                    .and_then(|offering| offering.weekly_period_target),
                scheduling_mode: expected.scheduling_mode,
                offering_id,
                offering_state: applicable_offering
                    .map(|offering| offering_state(offering.status))
                    .unwrap_or(HomeroomOfferingState::Missing),
                group_mode: classify_opening_group_mode(
                    offering_id.is_some(),
                    expected.scheduling_mode,
                    expected.requirement_kind,
                    &coverage_counts,
                ),
                teacher_state: classify_opening_teacher_state(
                    offering_id.is_some(),
                    expected.scheduling_mode,
                    &primary_counts,
                ),
                alignment_states: classify_expected_alignment(
                    applicable_offering,
                    expected.standard_periods_per_week,
                    delivery_version_effective_from,
                ),
                groups: applicable_groups
                    .into_iter()
                    .map(delivery_group_summary)
                    .collect(),
            });
        }
        let mut extra_offerings = Vec::new();
        let mut extra_offering_ids = HashSet::new();
        for offering in &offering_rows {
            if expected_resources.contains(&(offering.resource_kind, offering.catalog_version_id))
                || !target_applies_to_room(offering, homeroom.id, grade_level.id, study_program.id)
                || !offering_has_started(offering, delivery_version_effective_from)
                || !extra_offering_ids.insert(offering.offering_id)
            {
                continue;
            }
            extra_offerings.push(CurriculumDeliveryExtraOffering {
                offering_id: offering.offering_id,
                resource_kind: offering.resource_kind,
                catalog_version_id: offering.catalog_version_id,
                code: offering.code.clone(),
                name: offering.name.clone(),
                weekly_period_target: offering.weekly_period_target,
                starts_on: offering.starts_on,
                ends_on: offering.ends_on,
                alignment_states: classify_extra_alignment(
                    offering,
                    delivery_version_effective_from,
                ),
            });
        }
        extra_offerings.sort_by(|left, right| {
            left.code
                .cmp(&right.code)
                .then(left.name.cmp(&right.name))
                .then(left.offering_id.cmp(&right.offering_id))
        });
        let ready_count = items
            .iter()
            .filter(|item| {
                item.offering_id.is_some()
                    && (!item.groups.is_empty() || item.group_mode == HomeroomGroupMode::Central)
            })
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
            curriculum_level_id: room.curriculum_level_id,
            expected_count: items.len(),
            ready_count,
            items,
            extra_offerings,
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
                    resource_kind: offering.resource_kind,
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
                resource_kind: offering.resource_kind,
                group_id: None,
                code: offering.code.clone(),
                name: offering.name.clone(),
                reason: "รายการเปิดสอนไม่ตรงกับโครงสร้างหรือห้องเป้าหมายในภาคเรียนนี้".to_string(),
            });
        }
    }
    unlinked.sort_by(|left, right| left.code.cmp(&right.code).then(left.name.cmp(&right.name)));

    let workspace = HomeroomDeliveryWorkspace {
        academic_term_id,
        academic_year_id,
        delivery_version_id,
        delivery_version_status: delivery_version.map(|version| version.status),
        delivery_version_effective_from,
        homerooms: rooms,
        unlinked,
    };
    let assembly = assembly_started_at.elapsed();
    Ok((
        workspace,
        HomeroomDeliveryTiming {
            context_and_homerooms,
            resources,
            assembly,
        },
    ))
}

fn target_applies_to_room(
    offering: &OfferingTargetRow,
    homeroom_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
) -> bool {
    (offering.target_kind == "homeroom" && offering.homeroom_id == Some(homeroom_id))
        || (offering.target_kind == "grade_program"
            && offering.grade_level_id == grade_level_id
            && offering.study_program_id == study_program_id)
}

fn offering_has_started(
    offering: &OfferingTargetRow,
    effective_from: Option<chrono::NaiveDate>,
) -> bool {
    effective_from.is_none_or(|date| offering.starts_on.is_none_or(|start| start <= date))
}

fn offering_ended_before(
    offering: &OfferingTargetRow,
    effective_from: Option<chrono::NaiveDate>,
) -> bool {
    effective_from.is_some_and(|date| offering.ends_on.is_some_and(|end| end < date))
}

fn select_applicable_offering<'a>(
    candidates: &'a [&'a OfferingTargetRow],
    homeroom_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
    effective_from: Option<chrono::NaiveDate>,
) -> Option<&'a OfferingTargetRow> {
    candidates
        .iter()
        .copied()
        .filter(|offering| {
            target_applies_to_room(offering, homeroom_id, grade_level_id, study_program_id)
                && offering_has_started(offering, effective_from)
        })
        .max_by_key(|offering| {
            (
                !offering_ended_before(offering, effective_from),
                offering.starts_on,
                offering.offering_id,
            )
        })
}

fn classify_expected_alignment(
    offering: Option<&OfferingTargetRow>,
    standard_periods_per_week: Option<i32>,
    effective_from: Option<chrono::NaiveDate>,
) -> Vec<CurriculumDeliveryAlignmentState> {
    let Some(offering) = offering else {
        return vec![CurriculumDeliveryAlignmentState::CurriculumRequirementNotOffered];
    };
    let mut states = Vec::new();
    if offering_ended_before(offering, effective_from) {
        states.push(CurriculumDeliveryAlignmentState::EndedEarly);
    }
    if standard_periods_per_week.is_some()
        && offering.weekly_period_target.is_some()
        && standard_periods_per_week != offering.weekly_period_target
    {
        states.push(CurriculumDeliveryAlignmentState::OperationalPeriodsDiffer);
    }
    if states.is_empty() {
        states.push(CurriculumDeliveryAlignmentState::MatchesCurriculum);
    }
    states
}

fn classify_extra_alignment(
    offering: &OfferingTargetRow,
    effective_from: Option<chrono::NaiveDate>,
) -> Vec<CurriculumDeliveryAlignmentState> {
    let mut states = vec![CurriculumDeliveryAlignmentState::ExtraOffering];
    if offering_ended_before(offering, effective_from) {
        states.push(CurriculumDeliveryAlignmentState::EndedEarly);
    }
    states
}

fn offering_state(status: LearningOfferingStatus) -> HomeroomOfferingState {
    match status {
        LearningOfferingStatus::Draft => HomeroomOfferingState::Draft,
        LearningOfferingStatus::Published => HomeroomOfferingState::Published,
        LearningOfferingStatus::Cancelled => HomeroomOfferingState::Closed,
        LearningOfferingStatus::Closed => HomeroomOfferingState::Closed,
    }
}

fn classify_opening_group_mode(
    offered: bool,
    scheduling_mode: Option<ActivitySchedulingMode>,
    requirement_kind: RequirementKind,
    counts: &[i64],
) -> HomeroomGroupMode {
    if offered && counts.is_empty() && scheduling_mode == Some(ActivitySchedulingMode::Synchronized)
    {
        HomeroomGroupMode::Central
    } else {
        classify_group_mode(requirement_kind, counts)
    }
}

fn classify_opening_teacher_state(
    offered: bool,
    scheduling_mode: Option<ActivitySchedulingMode>,
    counts: &[i64],
) -> HomeroomTeacherState {
    if offered && counts.is_empty() && scheduling_mode == Some(ActivitySchedulingMode::Synchronized)
    {
        HomeroomTeacherState::Deferred
    } else {
        classify_teacher_state(counts)
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
    }
}

pub async fn delivery_overview(
    pool: &PgPool,
    academic_term_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<LearningDeliveryOverview, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.workspaces.delivery_overview",
    )
    .await?;
    delivery_overview_on_connection(&mut connection, academic_term_id, filter).await
}

pub async fn delivery_overview_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_term_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<LearningDeliveryOverview, AppError> {
    let offerings = offerings::list_on_connection(
        connection,
        LearningOfferingQuery { academic_term_id },
        filter,
    )
    .await?;
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

    let grade_rows: Vec<GradeLevelRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.workspaces.delivery_overview_on_connection",
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
        .fetch_all()
        .await?;
    let grades_by_id: HashMap<Uuid, GradeLevelLookupItem> = grade_rows
        .into_iter()
        .map(grade_level_lookup_item)
        .map(|item| (item.id, item))
        .collect();

    let program_rows: Vec<StudyProgramOption> = DbSession::new(connection).query_as("academic_delivery.services.workspaces.delivery_overview_on_connection",
        r#"
        SELECT program.id, program.code, program.display_order, program.name_th AS name,
               curriculum.id AS edition_id, version.name_th AS level_name,
               version.id AS curriculum_level_id, curriculum.name AS edition_name, curriculum.revision_year,
               ARRAY(SELECT DISTINCT requirement.grade_level_id FROM (
                   SELECT grade_level_id FROM published_curriculum_course_requirements WHERE study_program_id = program.id
                   UNION SELECT grade_level_id FROM published_curriculum_activity_requirements WHERE study_program_id = program.id
               ) requirement ORDER BY requirement.grade_level_id) AS grade_level_ids
        FROM published_study_programs program
        JOIN published_curriculum_levels version ON version.id = program.curriculum_level_id
        JOIN curriculum_editions curriculum ON curriculum.id=version.edition_id
        WHERE program.id = ANY($1)
        ORDER BY version.code, version.id, program.display_order, program.code, program.id
        "#,
    )
    .bind(&study_program_ids)
    .fetch_all()
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

    let group_rows: Vec<OverviewGroupRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.workspaces.delivery_overview_on_connection",
            r#"
        SELECT g.id,g.learning_offering_id,g.status,g.roster_status,
               count(assignment.id)::bigint AS registry_teacher_count,
               coalesce(bool_or(assignment.role='primary' AND account.user_type='staff'
                   AND account.status='active'),false) AS registry_has_primary
        FROM learning_groups g
        LEFT JOIN learning_group_teachers assignment ON assignment.learning_group_id=g.id
            AND g.status<>'published'
        LEFT JOIN users account ON account.id=assignment.teacher_id
        WHERE g.learning_offering_id=ANY($1)
        GROUP BY g.id ORDER BY g.id LIMIT 2001
        "#,
        )
        .bind(&offering_ids)
        .fetch_all()
        .await?;
    if group_rows.len() > MAX_WORKSPACE_GROUPS as usize {
        return Err(AppError::ValidationError(
            "จำนวนกลุ่มเรียนในพื้นที่ทำงานเกิน 2000 กลุ่ม กรุณาแบ่งข้อมูลก่อนเปิดพื้นที่ทำงาน".to_string(),
        ));
    }
    let published_groups = group_rows
        .iter()
        .filter(|group| group.status == LearningOfferingStatus::Published)
        .map(|group| group.id)
        .collect::<Vec<_>>();
    let year = offerings
        .first()
        .ok_or_else(|| AppError::InternalServerError("ไม่พบปีของรายการเปิดสอน".into()))?
        .academic_year_id;
    let published = super::versions::group_teacher_assignments_on_connection(
        connection,
        year,
        academic_term_id,
        &published_groups,
    )
    .await?
    .groups
    .into_iter()
    .map(|group| (group.learning_group_id, group))
    .collect::<HashMap<_, _>>();
    let mut aggregates_by_id: HashMap<Uuid, OfferingAggregateRow> = offering_ids
        .iter()
        .map(|id| {
            (
                *id,
                OfferingAggregateRow {
                    group_count: 0,
                    teacher_assignment_count: 0,
                    groups_without_primary_teacher: 0,
                    published_roster_count: 0,
                },
            )
        })
        .collect();
    for group in group_rows {
        let aggregate = aggregates_by_id
            .get_mut(&group.learning_offering_id)
            .ok_or_else(|| AppError::InternalServerError("ไม่พบรายการเปิดสอนของกลุ่ม".into()))?;
        let (teacher_count, has_primary) = match group.status {
            LearningOfferingStatus::Published => published
                .get(&group.id)
                .map(|source| {
                    (
                        source.teacher_ids.len() as i64,
                        !source.primary_teacher_ids.is_empty(),
                    )
                })
                .unwrap_or((0, false)),
            LearningOfferingStatus::Draft => {
                (group.registry_teacher_count, group.registry_has_primary)
            }
            // Closed/cancelled registry rows remain archival facts, not a fallback
            // for a published group's current planning membership.
            LearningOfferingStatus::Closed | LearningOfferingStatus::Cancelled => {
                (group.registry_teacher_count, group.registry_has_primary)
            }
        };
        aggregate.group_count += 1;
        aggregate.teacher_assignment_count += teacher_count;
        aggregate.groups_without_primary_teacher += i64::from(!has_primary);
        aggregate.published_roster_count +=
            i64::from(group.roster_status == RosterStatus::Published);
    }

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
            left.level_name
                .cmp(&right.level_name)
                .then_with(|| left.curriculum_level_id.cmp(&right.curriculum_level_id))
                .then_with(|| left.display_order.cmp(&right.display_order))
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
    filter: &AcademicResourceListFilter,
    curriculum_filter: &AcademicResourceListFilter,
) -> Result<DeliveryManagementOptions, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.workspaces.delivery_management_options",
    )
    .await?;
    delivery_management_options_on_connection(
        &mut connection,
        academic_term_id,
        filter,
        curriculum_filter,
    )
    .await
}

pub async fn delivery_management_options_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_term_id: Uuid,
    filter: &AcademicResourceListFilter,
    curriculum_filter: &AcademicResourceListFilter,
) -> Result<DeliveryManagementOptions, AppError> {
    let (academic_year_id, catalog_date): (Uuid, chrono::NaiveDate) = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.workspaces.delivery_management_options_on_connection",
            "SELECT academic_year_id, (now() AT TIME ZONE 'Asia/Bangkok')::date FROM academic_terms WHERE id = $1",
        )
        .bind(academic_term_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนที่เลือก".to_string()))?;
    let owner_ids = filter.allowed_organization_unit_ids();
    let catalog_rows: Vec<CatalogVersionRow> = DbSession::new(connection).query_as("academic_delivery.services.workspaces.delivery_management_options_on_connection",
        r#"
        SELECT option.id, option.kind, option.code, option.name, option.version_no,
               option.standard_periods_per_week, option.scheduling_mode, option.subject_type
        FROM (
            SELECT version.id, 'course'::text AS kind, subject.code,
                   version.name_th AS name, version.version_no,
                   version.periods_per_week AS standard_periods_per_week, NULL::text AS scheduling_mode, version.type AS subject_type
            FROM subject_versions version
            JOIN subjects subject ON subject.id = version.subject_id
            WHERE version.status = 'published'
              AND version.effective_from <= $1
              AND ($2 OR subject.owning_organization_unit_id = ANY($3))
            UNION ALL
            SELECT version.id, 'activity'::text AS kind, activity.code,
                   version.name, version.version_no,
                   NULL::integer AS standard_periods_per_week, version.scheduling_mode, NULL::text AS subject_type
            FROM activity_versions version
            JOIN activities activity ON activity.id = version.activity_id
            WHERE version.status = 'published'
              AND version.effective_from <= $1
              AND ($2 OR activity.owning_organization_unit_id = ANY($3))
        ) option
        ORDER BY option.kind, option.code, option.version_no DESC, option.id
        LIMIT $4
        "#,
    )
    .bind(catalog_date)
    .bind(filter.includes_school_owned)
    .bind(&owner_ids)
    .bind((MAX_CATALOG_OPTIONS + 1) as i64)
    .fetch_all()
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
            subject_type: row.subject_type,
            kind: row.kind,
            scheduling_mode: row.scheduling_mode,
            label: format!("{} — {} (ฉบับ {})", row.code, row.name, row.version_no),
            code: row.code,
            name: row.name,
            version_no: row.version_no,
            standard_periods_per_week: row.standard_periods_per_week,
        })
        .collect();

    let grade_rows: Vec<GradeLevelRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.workspaces.delivery_management_options_on_connection",
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
        .fetch_all()
        .await?;
    ensure_option_size(grade_rows.len(), MAX_LOOKUP_OPTIONS, "ระดับชั้น")?;
    let grade_levels = grade_rows
        .into_iter()
        .map(grade_level_lookup_item)
        .collect();
    let study_programs = if curriculum_filter.includes_school_owned {
        curriculum::list_study_program_options_for_year_on_connection(
            connection,
            academic_year_id,
            curriculum_filter,
        )
        .await?
    } else {
        Vec::new()
    };
    let homeroom_rows: Vec<HomeroomOptionRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.workspaces.delivery_management_options_on_connection",
            r#"SELECT homeroom.id, homeroom.name, grade.level_type, grade.year,
                  homeroom.grade_level_id
           FROM homerooms homeroom
           LEFT JOIN grade_levels grade ON grade.id = homeroom.grade_level_id
           WHERE homeroom.academic_year_id = $1 AND homeroom.is_active
           ORDER BY grade.year, homeroom.name
           LIMIT $2"#,
        )
        .bind(academic_year_id)
        .bind((MAX_LOOKUP_OPTIONS + 1) as i64)
        .fetch_all()
        .await?;
    ensure_option_size(homeroom_rows.len(), MAX_LOOKUP_OPTIONS, "ห้องประจำชั้น")?;
    let homerooms = homeroom_rows
        .into_iter()
        .map(|row| HomeroomLookupItem {
            id: row.id,
            name: row.name,
            grade_level: grade_level_label(row.level_type.as_deref(), row.year),
            grade_level_id: row.grade_level_id,
        })
        .collect();
    let learning_groups =
        groups::list_for_term_on_connection(connection, academic_term_id, filter).await?;
    let staff_rows: Vec<StaffOptionRow> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.workspaces.delivery_management_options_on_connection",
            r#"SELECT id, title, display_name
           FROM user_display_names
           WHERE user_type = 'staff' AND status = 'active'
           ORDER BY first_name, last_name
           LIMIT $1"#,
        )
        .bind((MAX_LOOKUP_OPTIONS + 1) as i64)
        .fetch_all()
        .await?;
    ensure_option_size(staff_rows.len(), MAX_LOOKUP_OPTIONS, "ครู")?;
    let teachers = staff_rows
        .into_iter()
        .map(|row| StaffLookupItem {
            id: row.id,
            name: row.display_name,
            title: row.title,
        })
        .collect();
    let rooms: Vec<Room> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.workspaces.delivery_management_options_on_connection",
            r#"SELECT room.id, room.building_id, room.name_th, room.name_en, room.code,
                  room.room_type, room.capacity, room.floor, room.status, room.description,
                  room.created_at, room.updated_at, building.name_th AS building_name
           FROM rooms room
           LEFT JOIN buildings building ON building.id = room.building_id
           WHERE room.status = 'ACTIVE'
           ORDER BY building.name_th NULLS LAST, room.floor NULLS FIRST, room.name_th, room.id
           LIMIT $1"#,
        )
        .bind((MAX_LOOKUP_OPTIONS + 1) as i64)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                error_class = school_database::error_class(&error),
                "delivery room option query failed"
            );
            AppError::InternalServerError("Failed to fetch rooms".to_string())
        })?;
    ensure_option_size(rooms.len(), MAX_LOOKUP_OPTIONS, "ห้องเรียน")?;

    Ok(DeliveryManagementOptions {
        academic_term_id,
        academic_year_id,
        catalog_versions,
        grade_levels,
        study_programs,
        homerooms,
        learning_groups,
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

fn grade_level_label(level_type: Option<&str>, year: Option<i32>) -> Option<String> {
    match (level_type, year) {
        (Some("kindergarten"), Some(year)) => Some(format!("อ.{year}")),
        (Some("primary"), Some(year)) => Some(format!("ป.{year}")),
        (Some("secondary"), Some(year)) => Some(format!("ม.{year}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{HomeroomGroupMode, HomeroomTeacherState};
    use school_academic_core::models::RequirementKind;
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use tokio::time::{timeout, Duration};

    #[test]
    fn central_states_require_an_open_synchronized_activity_without_applicable_groups() {
        let mode = Some(ActivitySchedulingMode::Synchronized);
        assert_eq!(
            classify_opening_group_mode(true, mode, RequirementKind::Required, &[]),
            HomeroomGroupMode::Central
        );
        assert_eq!(
            classify_opening_teacher_state(true, mode, &[]),
            HomeroomTeacherState::Deferred
        );
        assert_eq!(
            classify_opening_group_mode(false, mode, RequirementKind::Required, &[]),
            HomeroomGroupMode::Missing
        );
        assert_eq!(
            classify_opening_group_mode(
                true,
                Some(ActivitySchedulingMode::Independent),
                RequirementKind::Required,
                &[]
            ),
            HomeroomGroupMode::Missing
        );
        assert_eq!(
            classify_opening_group_mode(true, mode, RequirementKind::Required, &[1]),
            HomeroomGroupMode::Normal
        );
        assert_eq!(
            classify_opening_teacher_state(true, mode, &[0]),
            HomeroomTeacherState::MissingPrimary
        );
    }

    #[tokio::test]
    async fn public_workspace_parallel_reads_release_the_only_connection_before_finishing() {
        let pool = school_test_db::create_named_test_pool_with_max_connections(
            "delivery_workspace_single_connection",
            1,
        )
        .await;
        school_test_db::run_test_migrations(&pool).await;
        let year = Uuid::new_v4();
        let term = Uuid::new_v4();
        let schedule = Uuid::new_v4();
        sqlx::query("INSERT INTO academic_years (id,year,name,status) VALUES ($1,2569,'Synthetic workspace year','planning')")
            .bind(year).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO bell_schedules (id,academic_year_id,code,name) VALUES ($1,$2,'SYNTHETIC','Synthetic workspace schedule')")
            .bind(schedule).bind(year).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_terms (id,academic_year_id,sequence_no,code,name,term_type,included_in_year_result,blocks_year_closure,bell_schedule_id,status) VALUES ($1,$2,1,'SYNTHETIC','Synthetic workspace term','regular',true,true,$3,'planning')")
            .bind(term).bind(year).bind(schedule).execute(&pool).await.unwrap();
        let filter = AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        };
        let (workspace, _) = timeout(
            Duration::from_secs(5),
            homeroom_delivery_workspace_with_timing(&pool, year, term, None, &filter),
        )
        .await
        .expect("parallel branches must release their checkout before the dependent phase")
        .unwrap();
        assert_eq!(workspace.academic_year_id, year);
        assert_eq!(workspace.academic_term_id, term);
        assert!(workspace.homerooms.is_empty());
        assert!(workspace.unlinked.is_empty());
        // The same SQL helpers also run entirely on a connection owned by a caller.
        let mut connection = pool.acquire().await.unwrap();
        let (borrowed, _) = timeout(
            Duration::from_secs(5),
            homeroom_delivery_workspace_with_timing_on_connection(
                &mut connection,
                year,
                term,
                None,
                &filter,
            ),
        )
        .await
        .expect("connection helpers must not acquire the pool again")
        .unwrap();
        assert_eq!(borrowed.academic_term_id, term);
        assert!(borrowed.homerooms.is_empty());
    }

    #[tokio::test]
    async fn context_and_homeroom_reads_start_concurrently() {
        let barrier = Arc::new(Barrier::new(3));
        let context_barrier = Arc::clone(&barrier);
        let homeroom_barrier = Arc::clone(&barrier);

        let joined = tokio::spawn(join_context_and_homeroom_reads(
            async move {
                context_barrier.wait().await;
                Ok::<Option<WorkspaceContextRow>, sqlx::Error>(None)
            },
            async move {
                homeroom_barrier.wait().await;
                Ok::<Vec<HomeroomDeliveryBaseRow>, sqlx::Error>(Vec::new())
            },
        ));

        timeout(Duration::from_secs(1), barrier.wait())
            .await
            .expect("context and homeroom reads must start before either completes");
        let (context, homerooms) = joined
            .await
            .expect("context and homeroom join task must complete")
            .expect("empty context and homeroom reads must succeed");
        assert!(context.is_none());
        assert!(homerooms.is_empty());
    }

    #[tokio::test]
    async fn workspace_resource_reads_start_concurrently() {
        let barrier = Arc::new(Barrier::new(4));
        let expected_barrier = Arc::clone(&barrier);
        let offering_barrier = Arc::clone(&barrier);
        let group_barrier = Arc::clone(&barrier);

        let joined = tokio::spawn(join_workspace_resource_reads(
            async move {
                expected_barrier.wait().await;
                Ok::<Vec<ExpectedDeliveryRow>, sqlx::Error>(Vec::new())
            },
            async move {
                offering_barrier.wait().await;
                Ok::<Vec<OfferingTargetRow>, sqlx::Error>(Vec::new())
            },
            async move {
                group_barrier.wait().await;
                Ok::<Vec<DeliveryGroupRow>, sqlx::Error>(Vec::new())
            },
        ));

        timeout(Duration::from_secs(1), barrier.wait())
            .await
            .expect("all independent workspace reads must start before any one completes");
        let (expected, offerings, groups) = joined
            .await
            .expect("workspace read join task must complete")
            .expect("empty workspace reads must succeed");
        assert!(expected.is_empty());
        assert!(offerings.is_empty());
        assert!(groups.is_empty());
    }

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
    fn staffing_state_requires_every_applicable_group() {
        assert_eq!(
            classify_teacher_state(&[1, 2]),
            HomeroomTeacherState::Assigned
        );
        assert_eq!(
            classify_teacher_state(&[1, 0]),
            HomeroomTeacherState::MissingPrimary
        );
    }

    fn alignment_offering(
        weekly_period_target: Option<i32>,
        starts_on: Option<chrono::NaiveDate>,
        ends_on: Option<chrono::NaiveDate>,
    ) -> OfferingTargetRow {
        OfferingTargetRow {
            offering_id: Uuid::new_v4(),
            resource_kind: LearningOfferingKind::Course,
            catalog_version_id: Uuid::new_v4(),
            status: LearningOfferingStatus::Published,
            code: "TEST101".to_string(),
            name: "รายวิชาทดสอบ".to_string(),
            weekly_period_target,
            starts_on,
            ends_on,
            target_kind: "homeroom".to_string(),
            homeroom_id: Some(Uuid::new_v4()),
            grade_level_id: Uuid::new_v4(),
            study_program_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn alignment_states_distinguish_missing_matching_ended_period_difference_and_extra() {
        let effective_from = chrono::NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        assert_eq!(
            classify_expected_alignment(None, Some(2), Some(effective_from)),
            vec![CurriculumDeliveryAlignmentState::CurriculumRequirementNotOffered]
        );

        let matching = alignment_offering(
            Some(2),
            Some(chrono::NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
            None,
        );
        assert_eq!(
            classify_expected_alignment(Some(&matching), Some(2), Some(effective_from)),
            vec![CurriculumDeliveryAlignmentState::MatchesCurriculum]
        );

        let ended_with_different_periods = alignment_offering(
            Some(3),
            Some(chrono::NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
            Some(chrono::NaiveDate::from_ymd_opt(2026, 8, 14).unwrap()),
        );
        assert_eq!(
            classify_expected_alignment(
                Some(&ended_with_different_periods),
                Some(2),
                Some(effective_from)
            ),
            vec![
                CurriculumDeliveryAlignmentState::EndedEarly,
                CurriculumDeliveryAlignmentState::OperationalPeriodsDiffer,
            ]
        );
        assert_eq!(
            classify_extra_alignment(&ended_with_different_periods, Some(effective_from)),
            vec![
                CurriculumDeliveryAlignmentState::ExtraOffering,
                CurriculumDeliveryAlignmentState::EndedEarly,
            ]
        );
    }
}
