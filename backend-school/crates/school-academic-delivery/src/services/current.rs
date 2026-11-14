//! Live Delivery projections. Normalized rows are the sole authority.
use crate::models::{
    LearningOfferingQuery, LearningOfferingSnapshot, LearningTeacherRole, current::*,
};
use school_authorization::AcademicResourceListFilter;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use uuid::Uuid;

pub async fn workspace(
    pool: &PgPool,
    year: Uuid,
    term: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<DeliveryWorkspace, AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "academic_delivery.current.workspace")
            .await?;
    DbSession::new(&mut *tx)
        .query(
            "academic_delivery.current.workspace",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let result = visible_workspace_on_connection(&mut tx, year, term, access).await?;
    tx.commit().await?;
    Ok(result)
}

const MAX_COMPLETE_OFFERINGS: usize = 20_000;
const MAX_COMPLETE_GROUPS: usize = 20_000;
const MAX_TARGET_RELATIONS: usize = 250_000;

#[derive(Clone, Copy)]
pub(super) enum WorkspacePurpose {
    Public,
    Complete,
}
impl WorkspacePurpose {
    pub(super) fn offering_limit(self) -> usize {
        match self {
            Self::Public => super::offerings::MAX_TERM_OFFERINGS,
            Self::Complete => MAX_COMPLETE_OFFERINGS,
        }
    }
    pub(super) fn group_limit(self) -> usize {
        match self {
            Self::Public => super::groups::MAX_TERM_GROUPS,
            Self::Complete => MAX_COMPLETE_GROUPS,
        }
    }
    pub(super) fn overflow(self, public_message: &'static str) -> AppError {
        match self {
            Self::Public => AppError::ValidationError(public_message.into()),
            Self::Complete => complete_projection_overflow(),
        }
    }
}

fn complete_projection_overflow() -> AppError {
    AppError::ConflictWithCode {
        code: "DELIVERY_CURRENT_GRAPH_TOO_LARGE",
        message: "ข้อมูลเปิดสอนปัจจุบันเกินขอบเขตที่ระบบรองรับ กรุณาตรวจสอบข้อมูลภาคเรียน".into(),
    }
}

pub async fn visible_workspace_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<DeliveryWorkspace, AppError> {
    workspace_projection_on_connection(connection, year, term, access, WorkspacePurpose::Public)
        .await
}

pub async fn workspace_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
) -> Result<DeliveryWorkspace, AppError> {
    workspace_projection_on_connection(
        connection,
        year,
        term,
        &all_access(),
        WorkspacePurpose::Public,
    )
    .await
}

/// Complete private authority projection for domain composition and canonical caches.
/// This does not apply a viewer's public caps or authorize an HTTP response. The
/// caller must project authorized data before exposing it and retain its public
/// budgets. Overflow refuses the whole projection; it never returns a partial graph.
/// Technical bounds are 20,000 offerings, 20,000 groups and 250,000 resolved target relations.
pub async fn complete_workspace_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
) -> Result<DeliveryWorkspace, AppError> {
    workspace_projection_on_connection(
        connection,
        year,
        term,
        &all_access(),
        WorkspacePurpose::Complete,
    )
    .await
}

fn all_access() -> AcademicResourceListFilter {
    AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    }
}

async fn workspace_projection_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
    access: &AcademicResourceListFilter,
    purpose: WorkspacePurpose,
) -> Result<DeliveryWorkspace, AppError> {
    let row_version: i64 = DbSession::new(connection).scalar("academic_delivery.current.workspace_context", "SELECT COALESCE(state.row_version,1) FROM academic_terms term LEFT JOIN academic_delivery_state state ON state.academic_term_id=term.id AND state.academic_year_id=term.academic_year_id WHERE term.id=$1 AND term.academic_year_id=$2").bind(term).bind(year).fetch_optional().await?.ok_or_else(|| AppError::NotFound("ไม่พบปีและภาคเรียนที่เลือก".into()))?;
    let rows = super::offerings::list_for_projection_on_connection(
        connection,
        LearningOfferingQuery {
            academic_term_id: term,
        },
        access,
        purpose,
    )
    .await?;
    let offering_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let groups =
        super::groups::list_for_projection_on_connection(connection, term, access, purpose).await?;
    let mut by_offering: HashMap<Uuid, Vec<DeliveryGroup>> = HashMap::new();
    for group in groups {
        by_offering
            .entry(group.learning_offering_id)
            .or_default()
            .push(DeliveryGroup {
                registration_open: group.registration_open,
                row_version: group.row_version,
                id: group.id,
                code: group.code,
                name: group.name,
                description: group.description,
                capacity: group.capacity,
                homeroom_ids: group.homeroom_ids,
                preferred_room_ids: group.preferred_room_ids,
                teachers: group
                    .teacher_assignments
                    .into_iter()
                    .map(|teacher| DeliveryTeacher {
                        is_active: teacher.is_active,
                        row_version: teacher.row_version,
                        assignment_id: teacher.id,
                        teacher_id: teacher.teacher_id,
                        display_name: teacher.display_name,
                        role: teacher.role,
                    })
                    .collect(),
            });
    }
    let sources: Vec<(Uuid,Uuid)> = DbSession::new(connection).query_as("academic_delivery.current.curriculum_source_ids", "SELECT source.learning_offering_id,source.publication_id FROM learning_offering_curriculum_sources source JOIN learning_offerings offering ON offering.id=source.learning_offering_id WHERE offering.academic_term_id=$1 AND offering.academic_year_id=$2 AND offering.id=ANY($3) ORDER BY source.learning_offering_id,source.publication_id").bind(term).bind(year).bind(&offering_ids).fetch_all().await?;
    let source_ids = sources.iter().map(|(_, id)| *id).collect::<Vec<_>>();
    let publications: Vec<crate::models::CurriculumPublicationSource> = DbSession::new(connection).query_as("academic_delivery.current.curriculum_sources", "SELECT publication.edition_id,publication.id AS publication_id,publication.publication_no,publication.revision_year,publication.name AS edition_name FROM curriculum_publications publication WHERE publication.id=ANY($1) ORDER BY publication.edition_id,publication.id").bind(source_ids).fetch_all().await?;
    let target_rooms:Vec<(Uuid,Uuid)>=DbSession::new(connection).query_as("academic_delivery.current.target_homerooms", "SELECT DISTINCT target.learning_offering_id,room.id FROM learning_offering_targets target JOIN homerooms room ON room.academic_year_id=target.academic_year_id AND room.is_active AND ((target.target_kind='homeroom' AND target.homeroom_id=room.id) OR (target.target_kind='grade_program' AND target.grade_level_id=room.grade_level_id AND target.study_program_id=room.study_program_id)) WHERE target.academic_year_id=$1 AND target.academic_term_id=$2 AND target.learning_offering_id=ANY($3) ORDER BY target.learning_offering_id,room.id LIMIT $4").bind(year).bind(term).bind(&offering_ids).bind((MAX_TARGET_RELATIONS+1) as i64).fetch_all().await?;
    if target_rooms.len() > MAX_TARGET_RELATIONS {
        return Err(complete_projection_overflow());
    }
    let mut target_rooms_by_offering: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for (offering, room) in target_rooms {
        target_rooms_by_offering
            .entry(offering)
            .or_default()
            .push(room);
    }
    let mut offerings = Vec::with_capacity(rows.len());
    for row in rows {
        let groups = by_offering.remove(&row.id).unwrap_or_default();
        let homeroom_ids = target_rooms_by_offering.remove(&row.id).unwrap_or_default();
        let curriculum_sources = publications
            .iter()
            .filter(|publication| sources.contains(&(row.id, publication.publication_id)))
            .cloned()
            .collect();
        offerings.push(DeliveryOffering {
            row_version: row.row_version,
            curriculum_sources,
            id: row.id,
            kind: row.kind,
            code: row.code_snapshot,
            name: row.name_snapshot,
            owning_organization_unit_id: row.owning_organization_unit_id,
            source_requirement_kind: row.source_requirement_kind,
            source_requirement_id: row.source_requirement_id,
            weekly_period_target: row.weekly_period_target,
            catalog: row.snapshot,
            targets: row.targets,
            homeroom_ids,
            groups,
        });
    }
    let subject_ids = offerings
        .iter()
        .filter_map(|offering| match &offering.catalog {
            LearningOfferingSnapshot::Course(course) => Some(course.subject_version_id),
            LearningOfferingSnapshot::Activity(_) => None,
        })
        .collect::<Vec<_>>();
    let course_metadata =
        school_academic_core::services::catalog::subject_groups_for_versions_on_connection(
            connection,
            &subject_ids,
        )
        .await?
        .into_iter()
        .map(|metadata| DeliveryCourseMetadata {
            subject_version_id: metadata.subject_version_id,
            subject_group_id: metadata.subject_group_id,
            subject_group_name: metadata.name,
        })
        .collect();
    Ok(DeliveryWorkspace {
        academic_year_id: year,
        academic_term_id: term,
        row_version,
        offerings,
        course_metadata,
    })
}

pub async fn teacher_allocations_on_connection(
    connection: &mut PgConnection,
    offering_ids: &[Uuid],
) -> Result<Vec<DeliveryTeacherAllocation>, AppError> {
    if offering_ids.is_empty() {
        return Ok(vec![]);
    }
    allocations_on_connection(connection, AllocationScope::Offerings(offering_ids), None).await
}

enum AllocationScope<'a> {
    Term(Uuid),
    Offerings(&'a [Uuid]),
}

async fn allocations_on_connection(
    connection: &mut PgConnection,
    scope: AllocationScope<'_>,
    actor: Option<Uuid>,
) -> Result<Vec<DeliveryTeacherAllocation>, AppError> {
    let (term, offerings) = match scope {
        AllocationScope::Term(term) => (Some(term), &[][..]),
        AllocationScope::Offerings(ids) => (None, ids),
    };
    Ok(DbSession::new(connection).query_as("academic_delivery.current.teacher_allocations", "SELECT offering.academic_year_id,offering.academic_term_id,offering.id AS learning_offering_id,group_row.id AS learning_group_id,teacher.id AS assignment_id,teacher.teacher_id,teacher.role FROM learning_offerings offering JOIN learning_groups group_row ON group_row.learning_offering_id=offering.id AND group_row.academic_term_id=offering.academic_term_id AND group_row.academic_year_id=offering.academic_year_id JOIN learning_group_teachers teacher ON teacher.learning_group_id=group_row.id AND teacher.academic_term_id=group_row.academic_term_id AND teacher.academic_year_id=group_row.academic_year_id JOIN users account ON account.id=teacher.teacher_id AND account.status='active' AND account.user_type='staff' WHERE (offering.academic_term_id=$1 OR offering.id=ANY($2)) AND ($3::uuid IS NULL OR teacher.teacher_id=$3) ORDER BY offering.id,group_row.id,teacher.teacher_id,teacher.id").bind(term).bind(offerings).bind(actor).fetch_all().await?)
}

pub(crate) async fn assigned_offering_ids_for_term_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    actor: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    Ok(
        allocations_on_connection(connection, AllocationScope::Term(term), Some(actor))
            .await?
            .into_iter()
            .map(|row| row.learning_offering_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    )
}

pub(crate) async fn assigned_offering_ids_on_connection(
    connection: &mut PgConnection,
    offerings: &[Uuid],
    actor: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    if offerings.is_empty() {
        return Ok(Vec::new());
    }
    Ok(allocations_on_connection(
        connection,
        AllocationScope::Offerings(offerings),
        Some(actor),
    )
    .await?
    .into_iter()
    .map(|row| row.learning_offering_id)
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect())
}

pub async fn group_teacher_assignments_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
    group_ids: &[Uuid],
) -> Result<CurrentTeacherAssignments, AppError> {
    let rows: Vec<(Uuid,Uuid,LearningTeacherRole)> = DbSession::new(connection).query_as("academic_delivery.current.group_teacher_assignments", "SELECT teacher.learning_group_id,teacher.teacher_id,teacher.role FROM learning_group_teachers teacher JOIN learning_groups group_row ON group_row.id=teacher.learning_group_id JOIN learning_offerings offering ON offering.id=group_row.learning_offering_id JOIN users account ON account.id=teacher.teacher_id AND account.status='active' AND account.user_type='staff' WHERE offering.academic_year_id=$1 AND offering.academic_term_id=$2 AND group_row.academic_year_id=$1 AND group_row.academic_term_id=$2 AND teacher.learning_group_id=ANY($3) ORDER BY teacher.learning_group_id,teacher.teacher_id,teacher.role").bind(year).bind(term).bind(group_ids).fetch_all().await?;
    let mut groups: BTreeMap<Uuid, (BTreeSet<Uuid>, BTreeSet<Uuid>)> = BTreeMap::new();
    for (group, teacher, role) in rows {
        let (teachers, primaries) = groups.entry(group).or_default();
        teachers.insert(teacher);
        if role == LearningTeacherRole::Primary {
            primaries.insert(teacher);
        }
    }
    Ok(CurrentTeacherAssignments {
        groups: groups
            .into_iter()
            .map(
                |(learning_group_id, (teachers, primaries))| GroupTeacherAssignments {
                    learning_group_id,
                    teacher_ids: teachers.into_iter().collect(),
                    primary_teacher_ids: primaries.into_iter().collect(),
                },
            )
            .collect(),
    })
}

pub async fn assigned_group_ids_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
    actor: Uuid,
) -> Result<CurrentTeacherGroupIds, AppError> {
    let learning_group_ids =
        allocations_on_connection(connection, AllocationScope::Term(term), Some(actor))
            .await?
            .into_iter()
            .filter(|row| row.academic_year_id == year)
            .map(|row| row.learning_group_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    Ok(CurrentTeacherGroupIds { learning_group_ids })
}

pub async fn course_offering_ids_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    Ok(DbSession::new(connection).scalar("academic_delivery.current.course_offerings", "SELECT id FROM learning_offerings WHERE academic_year_id=$1 AND academic_term_id=$2 AND kind='course' ORDER BY id").bind(year).bind(term).fetch_all().await?)
}

pub async fn term_revision_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
) -> Result<i64, AppError> {
    DbSession::new(connection).scalar("academic_delivery.current.term_revision", "SELECT COALESCE(state.row_version,1) FROM academic_terms term LEFT JOIN academic_delivery_state state ON state.academic_term_id=term.id WHERE term.id=$1 AND term.academic_year_id=$2").bind(term).bind(year).fetch_optional().await?.ok_or_else(|| AppError::NotFound("ไม่พบปีและภาคเรียนที่เลือก".into()))
}

/// Current registry groups for an exact offering; no publication or date selection.
pub async fn offering_group_ids_on_connection(
    conn: &mut PgConnection,
    offering_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let rows:Vec<Uuid>=DbSession::new(conn).scalar("academic_delivery.current.offering_groups","SELECT g.id FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id AND o.academic_year_id=g.academic_year_id AND o.academic_term_id=g.academic_term_id WHERE o.id=$1 ORDER BY g.code,g.id LIMIT 20001").bind(offering_id).fetch_all().await?;
    if rows.len() > 20000 {
        return Err(AppError::ValidationError("กลุ่มเรียนเกินขนาดที่รองรับ".into()));
    }
    Ok(rows)
}
