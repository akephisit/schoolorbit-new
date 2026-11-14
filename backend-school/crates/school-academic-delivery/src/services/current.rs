//! Live Delivery projections. Normalized rows are the sole authority.
use crate::models::{
    LearningOfferingQuery, LearningOfferingSnapshot, LearningTeacherRole, current::*,
};
use school_authorization::{
    AcademicResourceAccess, AcademicResourceListFilter, academic_resource_access_for,
};
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

pub async fn visible_workspace_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
    access: &AcademicResourceListFilter,
) -> Result<DeliveryWorkspace, AppError> {
    let mut result = workspace_on_connection(connection, year, term).await?;
    result.offerings.retain(|offering| {
        let assigned = access.assigned_actor_id.is_some_and(|actor| {
            offering.groups.iter().any(|group| {
                group
                    .teachers
                    .iter()
                    .any(|teacher| teacher.teacher_id == actor)
            })
        });
        academic_resource_access_for(access, Some(offering.owning_organization_unit_id), assigned)
            != AcademicResourceAccess::None
    });
    let visible_subjects: BTreeSet<_> = result
        .offerings
        .iter()
        .filter_map(|offering| match &offering.catalog {
            LearningOfferingSnapshot::Course(course) => Some(course.subject_version_id),
            LearningOfferingSnapshot::Activity(_) => None,
        })
        .collect();
    result
        .course_metadata
        .retain(|metadata| visible_subjects.contains(&metadata.subject_version_id));
    Ok(result)
}

pub async fn workspace_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
) -> Result<DeliveryWorkspace, AppError> {
    let row_version: i64 = DbSession::new(connection).scalar("academic_delivery.current.workspace_context", "SELECT COALESCE(state.row_version,1) FROM academic_terms term LEFT JOIN academic_delivery_state state ON state.academic_term_id=term.id AND state.academic_year_id=term.academic_year_id WHERE term.id=$1 AND term.academic_year_id=$2").bind(term).bind(year).fetch_optional().await?.ok_or_else(|| AppError::NotFound("ไม่พบปีและภาคเรียนที่เลือก".into()))?;
    let all = AcademicResourceListFilter {
        includes_school_owned: true,
        organization_unit_ids: vec![],
        organization_tree_unit_ids: vec![],
        assigned_actor_id: None,
    };
    let rows = super::offerings::list_on_connection(
        connection,
        LearningOfferingQuery {
            academic_term_id: term,
        },
        &all,
    )
    .await?;
    let groups = super::groups::list_for_term_on_connection(connection, term, &all).await?;
    let targets: HashMap<Uuid,i32> = DbSession::new(connection).query_as("academic_delivery.current.weekly_targets", "SELECT id,weekly_period_target FROM learning_offerings WHERE academic_term_id=$1 AND academic_year_id=$2 ORDER BY id").bind(term).bind(year).fetch_all().await?.into_iter().collect();
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
    let sources: Vec<(Uuid,Uuid)> = DbSession::new(connection).query_as("academic_delivery.current.curriculum_source_ids", "SELECT source.learning_offering_id,source.publication_id FROM learning_offering_curriculum_sources source JOIN learning_offerings offering ON offering.id=source.learning_offering_id WHERE offering.academic_term_id=$1 AND offering.academic_year_id=$2 ORDER BY source.learning_offering_id,source.publication_id").bind(term).bind(year).fetch_all().await?;
    let source_ids = sources.iter().map(|(_, id)| *id).collect::<Vec<_>>();
    let publications: Vec<crate::models::CurriculumPublicationSource> = DbSession::new(connection).query_as("academic_delivery.current.curriculum_sources", "SELECT publication.edition_id,publication.id AS publication_id,publication.publication_no,publication.revision_year,publication.name AS edition_name FROM curriculum_publications publication WHERE publication.id=ANY($1) ORDER BY publication.edition_id,publication.id").bind(source_ids).fetch_all().await?;
    let target_rooms:Vec<(Uuid,Uuid)>=DbSession::new(connection).query_as("academic_delivery.current.target_homerooms", "SELECT DISTINCT target.learning_offering_id,room.id FROM learning_offering_targets target JOIN homerooms room ON room.academic_year_id=target.academic_year_id AND room.is_active AND ((target.target_kind='homeroom' AND target.homeroom_id=room.id) OR (target.target_kind='grade_program' AND target.grade_level_id=room.grade_level_id AND target.study_program_id=room.study_program_id)) WHERE target.academic_year_id=$1 AND target.academic_term_id=$2 ORDER BY target.learning_offering_id,room.id LIMIT 250001").bind(year).bind(term).fetch_all().await?;
    if target_rooms.len() > 250000 {
        return Err(AppError::ValidationError(
            "Current Delivery target coverage exceeds supported size".into(),
        ));
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
            weekly_period_target: *targets.get(&row.id).ok_or_else(|| {
                AppError::InternalServerError("Current weekly target is missing".into())
            })?,
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
    Ok(DbSession::new(connection).query_as("academic_delivery.current.teacher_allocations", "SELECT offering.academic_year_id,offering.academic_term_id,offering.id AS learning_offering_id,group_row.id AS learning_group_id,teacher.id AS assignment_id,teacher.teacher_id,teacher.role FROM learning_offerings offering JOIN learning_groups group_row ON group_row.learning_offering_id=offering.id JOIN learning_group_teachers teacher ON teacher.learning_group_id=group_row.id JOIN users account ON account.id=teacher.teacher_id AND account.status='active' AND account.user_type='staff' WHERE offering.id=ANY($1) ORDER BY offering.id,group_row.id,teacher.teacher_id,teacher.id").bind(offering_ids).fetch_all().await?)
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
    let learning_group_ids = DbSession::new(connection).scalar("academic_delivery.current.assigned_groups", "SELECT DISTINCT group_row.id FROM learning_groups group_row JOIN learning_offerings offering ON offering.id=group_row.learning_offering_id JOIN learning_group_teachers teacher ON teacher.learning_group_id=group_row.id JOIN users account ON account.id=teacher.teacher_id AND account.status='active' AND account.user_type='staff' WHERE offering.academic_year_id=$1 AND offering.academic_term_id=$2 AND group_row.academic_year_id=$1 AND group_row.academic_term_id=$2 AND teacher.teacher_id=$3 ORDER BY group_row.id").bind(year).bind(term).bind(actor).fetch_all().await?;
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
