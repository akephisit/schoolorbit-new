use school_database::DbSession;
use school_errors::AppError;
use std::collections::{HashMap, HashSet};

use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::models::GradeLevelLookupItem;

use super::super::models::{
    CatalogCurriculumMetrics, CatalogWeeklyUnit, CurriculumCourseCreditCategory,
    CurriculumDocumentSection, CurriculumStructureRequirement, CurriculumStructureRequirementInput,
    CurriculumStructureValidation, CurriculumStructureWorkspace, CurriculumTermSlot,
    CurriculumValidationNotice, ReplaceCurriculumStructureRequest,
    ReplaceCurriculumTermSlotsRequest, RequirementKind, RequirementResourceKind, VersionStatus,
};
use super::{curriculum, parse_row_version};

const MAX_STRUCTURE_REQUIREMENTS: usize = 5_000;

#[derive(sqlx::FromRow)]
struct GradeLevelRow {
    id: Uuid,
    level_type: String,
    year: i32,
}

#[derive(sqlx::FromRow)]
struct RequirementRow {
    id: Uuid,
    study_program_id: Uuid,
    grade_level_id: Uuid,
    grade_level_type: String,
    grade_level_year: i32,
    term_slot_id: Uuid,
    resource_kind: RequirementResourceKind,
    catalog_version_id: Uuid,
    catalog_id: Uuid,
    version_no: i32,
    code: String,
    name: String,
    catalog_classification: String,
    requirement_kind: RequirementKind,
    weekly_value: Option<String>,
    weekly_unit: String,
    credit: Option<String>,
    total_hours: Option<String>,
    display_order: i32,
}

#[derive(sqlx::FromRow)]
struct ProgramLockRow {
    curriculum_level_id: Uuid,
    row_version: i64,
    version_status: VersionStatus,
}

#[derive(sqlx::FromRow)]
struct CatalogGradeRow {
    id: Uuid,
    grade_level_ids: Vec<Uuid>,
}

pub async fn get_workspace(
    pool: &PgPool,
    version_id: Uuid,
) -> Result<CurriculumStructureWorkspace, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.curriculum_structure.get_workspace",
    )
    .await?;
    get_workspace_on_connection(&mut connection, version_id).await
}

pub async fn get_workspace_on_connection(
    connection: &mut sqlx::PgConnection,
    version_id: Uuid,
) -> Result<CurriculumStructureWorkspace, AppError> {
    get_workspace_from_publication(connection, version_id, None).await
}

pub(super) async fn get_workspace_from_publication(
    connection: &mut sqlx::PgConnection,
    version_id: Uuid,
    publication: Option<Uuid>,
) -> Result<CurriculumStructureWorkspace, AppError> {
    let level = curriculum::level_from_publication(connection, version_id, publication).await?;
    let curriculum_term_slots =
        super::curriculum_publications::source("curriculum_term_slots", publication)?;
    let curriculum_levels =
        super::curriculum_publications::source("curriculum_levels", publication)?;
    let curriculum_course_requirements =
        super::curriculum_publications::source("curriculum_course_requirements", publication)?;
    let curriculum_activity_requirements =
        super::curriculum_publications::source("curriculum_activity_requirements", publication)?;
    let study_programs = super::curriculum_publications::source("study_programs", publication)?;
    let term_slots = DbSession::new(connection)
        .query_as::<CurriculumTermSlot>(
            "academic_core.services.curriculum_structure.get_workspace_from_publication",
            sqlx::AssertSqlSafe(format!(
                r#"SELECT id, curriculum_level_id, sequence, term_type, type_occurrence,
                  name, row_version
           FROM {curriculum_term_slots}
           WHERE curriculum_level_id = $1
           ORDER BY sequence, id"#
            )),
        )
        .bind(version_id)
        .bind(publication)
        .fetch_all()
        .await?;
    let programs =
        curriculum::programs_from_publication(connection, version_id, publication).await?;
    let grade_levels = DbSession::new(connection)
        .query_as::<GradeLevelRow>(
            "academic_core.services.curriculum_structure.get_workspace_from_publication",
            sqlx::AssertSqlSafe(format!(
                r#"SELECT grade.id, grade.level_type, grade.year
           FROM {curriculum_levels} version
           JOIN curriculum_editions curriculum ON curriculum.id=version.edition_id
           JOIN grade_levels grade ON grade.id IN (
               SELECT jsonb_array_elements_text(
                   COALESCE(version.grade_level_ids, '[]'::jsonb)
               )::uuid
           )
           WHERE version.id = $1
           ORDER BY CASE grade.level_type
                        WHEN 'kindergarten' THEN 1
                        WHEN 'primary' THEN 2
                        WHEN 'secondary' THEN 3
                        ELSE 4
                    END,
                    grade.year,
                    grade.id"#
            )),
        )
        .bind(version_id)
        .bind(publication)
        .fetch_all()
        .await?
        .into_iter()
        .map(grade_level_item)
        .collect::<Vec<_>>();
    let rows = DbSession::new(connection)
        .query_as::<RequirementRow>(
            "academic_core.services.curriculum_structure.get_workspace_from_publication",
            sqlx::AssertSqlSafe(format!(
                r#"SELECT requirement.id,
                  requirement.study_program_id,
                  requirement.grade_level_id,
                  grade.level_type AS grade_level_type,
                  grade.year AS grade_level_year,
                  requirement.term_slot_id,
                  'course'::text AS resource_kind,
                  requirement.subject_version_id AS catalog_version_id,
                  subject.id AS catalog_id,
                  version.version_no,
                  subject.code,
                  version.name_th AS name,
                  version.type AS catalog_classification,
                  requirement.requirement_kind,
                  version.periods_per_week::text AS weekly_value,
                  'period'::text AS weekly_unit,
                  version.credit::text AS credit,
                  version.hours_per_semester::text AS total_hours,
                  requirement.display_order
           FROM {curriculum_course_requirements} requirement
           JOIN {study_programs} program ON program.id = requirement.study_program_id
           JOIN grade_levels grade ON grade.id = requirement.grade_level_id
           JOIN subject_versions version ON version.id = requirement.subject_version_id
           JOIN subjects subject ON subject.id = version.subject_id
           WHERE program.curriculum_level_id = $1
           UNION ALL
           SELECT requirement.id,
                  requirement.study_program_id,
                  requirement.grade_level_id,
                  grade.level_type AS grade_level_type,
                  grade.year AS grade_level_year,
                  requirement.term_slot_id,
                  'activity'::text AS resource_kind,
                  requirement.activity_version_id AS catalog_version_id,
                  activity.id AS catalog_id,
                  version.version_no,
                  activity.code,
                  version.name,
                  activity.activity_type AS catalog_classification,
                  requirement.requirement_kind,
                  version.hours_per_week::text AS weekly_value,
                  'hour'::text AS weekly_unit,
                  NULL::text AS credit,
                  version.hours_per_term::text AS total_hours,
                  requirement.display_order
           FROM {curriculum_activity_requirements} requirement
           JOIN {study_programs} program ON program.id = requirement.study_program_id
           JOIN grade_levels grade ON grade.id = requirement.grade_level_id
           JOIN activity_versions version ON version.id = requirement.activity_version_id
           JOIN activities activity ON activity.id = version.activity_id
           WHERE program.curriculum_level_id = $1
           ORDER BY study_program_id, grade_level_year, term_slot_id,
                    display_order, resource_kind, catalog_version_id"#
            )),
        )
        .bind(version_id)
        .bind(publication)
        .fetch_all()
        .await?;

    let subject_ids = rows
        .iter()
        .filter(|row| row.resource_kind == RequirementResourceKind::Course)
        .map(|row| row.catalog_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let current_subject_revisions =
        super::catalog::current_subject_revisions_on_connection(connection, &subject_ids).await?;
    let activity_ids = rows
        .iter()
        .filter(|row| row.resource_kind == RequirementResourceKind::Activity)
        .map(|row| row.catalog_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let current_activity_revisions =
        super::catalog::current_activity_revisions_on_connection(connection, &activity_ids).await?;

    let version_ids = rows
        .iter()
        .filter(|row| row.resource_kind == RequirementResourceKind::Course)
        .map(|row| row.catalog_version_id)
        .collect::<Vec<_>>();
    let groups =
        super::catalog::subject_groups_for_versions_on_connection(connection, &version_ids)
            .await?
            .into_iter()
            .map(|group| (group.subject_version_id, group))
            .collect::<HashMap<_, _>>();

    let program_order = programs
        .iter()
        .enumerate()
        .map(|(index, program)| (program.id, index))
        .collect::<HashMap<_, _>>();
    let slot_order = term_slots
        .iter()
        .map(|slot| (slot.id, slot.sequence))
        .collect::<HashMap<_, _>>();
    let mut validation = CurriculumStructureValidation::default();
    let mut requirements = rows
        .into_iter()
        .map(|row| {
            let current_version_no = match row.resource_kind {
                RequirementResourceKind::Course => current_subject_revisions
                    .get(&row.catalog_id)
                    .map(|current| current.version_no),
                RequirementResourceKind::Activity => current_activity_revisions
                    .get(&row.catalog_id)
                    .map(|current| current.version_no),
            };
            let has_newer_current_version =
                current_version_no.is_some_and(|version_no| version_no > row.version_no);
            let group = groups.get(&row.catalog_version_id);
            let mut requirement =
                requirement_from_row(row, has_newer_current_version, &mut validation);
            requirement.subject_group_id = group.map(|group| group.subject_group_id);
            requirement.subject_group_order = group.and_then(|group| group.display_order);
            requirement
        })
        .collect::<Vec<_>>();
    requirements.sort_by_key(|requirement| {
        (
            program_order
                .get(&requirement.study_program_id)
                .copied()
                .unwrap_or(usize::MAX),
            requirement.grade_level.level_order,
            slot_order
                .get(&requirement.term_slot_id)
                .copied()
                .unwrap_or(i32::MAX),
            requirement.display_order,
            match requirement.resource_kind {
                RequirementResourceKind::Course => 0,
                RequirementResourceKind::Activity => 1,
            },
            requirement.catalog_version_id,
        )
    });
    let row_version = level.row_version;
    Ok(CurriculumStructureWorkspace {
        level,
        term_slots,
        programs,
        grade_levels,
        requirements,
        validation,
        row_version,
    })
}

pub async fn replace_program_structure(
    pool: &PgPool,
    program_id: Uuid,
    request: ReplaceCurriculumStructureRequest,
) -> Result<CurriculumStructureWorkspace, AppError> {
    parse_row_version(request.row_version)?;
    validate_requirement_inputs(&request.requirements)?;

    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.curriculum_structure.replace_program_structure",
    )
    .await?;
    let level_id: Uuid = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.curriculum_structure.replace_program_structure",
            "SELECT curriculum_level_id FROM study_programs WHERE id=$1",
        )
        .bind(program_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียน".into()))?;
    super::curriculum::require_draft_level(&mut transaction, level_id, request.draft_id).await?;
    let program = DbSession::new(&mut *transaction).query_as::<ProgramLockRow>("academic_core.services.curriculum_structure.replace_program_structure",
        r#"SELECT program.curriculum_level_id,
                  program.row_version,
                  CASE WHEN curriculum.draft_id IS NOT NULL THEN 'draft' ELSE curriculum.status END AS version_status
           FROM study_programs program
           JOIN curriculum_levels version ON version.id = program.curriculum_level_id
           JOIN curriculum_editions curriculum ON curriculum.id=version.edition_id
           WHERE program.id = $1
           FOR UPDATE OF program"#,
    )
    .bind(program_id)
    .fetch_optional()
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียน".to_string()))?;
    if program.version_status != VersionStatus::Draft {
        return Err(AppError::Conflict(
            "เวอร์ชันหลักสูตรที่เผยแพร่แล้วแก้ไขไม่ได้".to_string(),
        ));
    }
    if program.row_version != request.row_version {
        return Err(AppError::Conflict(
            "แผนการเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }

    validate_new_catalog_requirements(&mut transaction, program_id, &request.requirements).await?;

    validate_requirement_ownership(
        &mut transaction,
        program_id,
        program.curriculum_level_id,
        &request.requirements,
    )
    .await?;

    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.curriculum_structure.replace_program_structure",
            "DELETE FROM curriculum_course_requirements WHERE study_program_id = $1",
        )
        .bind(program_id)
        .execute()
        .await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.curriculum_structure.replace_program_structure",
            "DELETE FROM curriculum_activity_requirements WHERE study_program_id = $1",
        )
        .bind(program_id)
        .execute()
        .await?;

    let course_requirements = request
        .requirements
        .iter()
        .filter(|item| item.resource_kind == RequirementResourceKind::Course)
        .collect::<Vec<_>>();
    if !course_requirements.is_empty() {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO curriculum_course_requirements (id, curriculum_level_id, \
             study_program_id, subject_version_id, grade_level_id, term_slot_id, \
             requirement_kind, display_order) ",
        );
        builder.push_values(course_requirements, |mut values, requirement| {
            values
                .push_bind(Uuid::now_v7())
                .push_bind(program.curriculum_level_id)
                .push_bind(program_id)
                .push_bind(requirement.catalog_version_id)
                .push_bind(requirement.grade_level_id)
                .push_bind(requirement.term_slot_id)
                .push_bind(requirement.requirement_kind)
                .push_bind(requirement.display_order);
        });
        DbSession::new(&mut *transaction)
            .prepared_query(
                "academic_core.services.curriculum_structure.replace_program_structure",
                builder.build(),
            )
            .execute()
            .await?;
    }

    let activity_requirements = request
        .requirements
        .iter()
        .filter(|item| item.resource_kind == RequirementResourceKind::Activity)
        .collect::<Vec<_>>();
    if !activity_requirements.is_empty() {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO curriculum_activity_requirements (id, curriculum_level_id, \
             study_program_id, activity_version_id, grade_level_id, term_slot_id, \
             requirement_kind, display_order) ",
        );
        builder.push_values(activity_requirements, |mut values, requirement| {
            values
                .push_bind(Uuid::now_v7())
                .push_bind(program.curriculum_level_id)
                .push_bind(program_id)
                .push_bind(requirement.catalog_version_id)
                .push_bind(requirement.grade_level_id)
                .push_bind(requirement.term_slot_id)
                .push_bind(requirement.requirement_kind)
                .push_bind(requirement.display_order);
        });
        DbSession::new(&mut *transaction)
            .prepared_query(
                "academic_core.services.curriculum_structure.replace_program_structure",
                builder.build(),
            )
            .execute()
            .await?;
    }

    DbSession::new(&mut *transaction).query("academic_core.services.curriculum_structure.replace_program_structure",
        "UPDATE study_programs SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(program_id)
    .execute()
    .await?;
    transaction.commit().await?;

    get_workspace(pool, program.curriculum_level_id).await
}

pub async fn replace_term_slots(
    pool: &PgPool,
    version_id: Uuid,
    request: ReplaceCurriculumTermSlotsRequest,
) -> Result<CurriculumStructureWorkspace, AppError> {
    parse_row_version(request.row_version)?;
    if request.slots.len() > 20 {
        return Err(AppError::ValidationError(
            "จำนวนภาคเรียนในหลักสูตรมากเกินขีดจำกัด".to_string(),
        ));
    }
    let mut sequences = HashSet::new();
    let mut type_occurrences = HashSet::new();
    let mut supplied_ids = HashSet::new();
    for slot in &request.slots {
        if slot.sequence <= 0 || slot.type_occurrence <= 0 || slot.name.trim().is_empty() {
            return Err(AppError::ValidationError(
                "ข้อมูลภาคเรียนในหลักสูตรไม่ครบถ้วน".to_string(),
            ));
        }
        if !sequences.insert(slot.sequence)
            || !type_occurrences.insert((slot.term_type, slot.type_occurrence))
        {
            return Err(AppError::ValidationError(
                "ลำดับหรือประเภทภาคเรียนในหลักสูตรซ้ำกัน".to_string(),
            ));
        }
        if slot.id.is_some_and(|id| !supplied_ids.insert(id)) {
            return Err(AppError::ValidationError(
                "รหัสภาคเรียนในหลักสูตรซ้ำกัน".to_string(),
            ));
        }
    }

    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.curriculum_structure.replace_term_slots",
    )
    .await?;
    super::curriculum::require_draft_level(&mut transaction, version_id, request.draft_id).await?;
    let (status, current_row_version): (VersionStatus, i64) = DbSession::new(&mut *transaction).query_as("academic_core.services.curriculum_structure.replace_term_slots",
        "SELECT CASE WHEN e.draft_id IS NOT NULL THEN 'draft' ELSE e.status END,l.row_version FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.id=$1 FOR UPDATE OF l",
    )
    .bind(version_id)
    .fetch_optional()
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันหลักสูตร".to_string()))?;
    if status != VersionStatus::Draft {
        return Err(AppError::Conflict(
            "เวอร์ชันหลักสูตรที่เผยแพร่แล้วแก้ไขไม่ได้".to_string(),
        ));
    }
    if current_row_version != request.row_version {
        return Err(AppError::Conflict(
            "เวอร์ชันหลักสูตรถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }

    let existing_ids = DbSession::new(&mut *transaction).scalar::<Uuid>("academic_core.services.curriculum_structure.replace_term_slots",
        "SELECT id FROM curriculum_term_slots WHERE curriculum_level_id = $1 ORDER BY id FOR UPDATE",
    )
    .bind(version_id)
    .fetch_all()
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    if supplied_ids.iter().any(|id| !existing_ids.contains(id)) {
        return Err(AppError::ValidationError(
            "ภาคเรียนที่แก้ไขไม่อยู่ในเวอร์ชันหลักสูตรนี้".to_string(),
        ));
    }
    let removed_ids = existing_ids
        .difference(&supplied_ids)
        .copied()
        .collect::<Vec<_>>();
    if !removed_ids.is_empty() {
        let referenced: bool = DbSession::new(&mut *transaction)
            .scalar(
                "academic_core.services.curriculum_structure.replace_term_slots",
                r#"SELECT EXISTS (
                   SELECT 1 FROM curriculum_course_requirements
                   WHERE term_slot_id = ANY($1)
               ) OR EXISTS (
                   SELECT 1 FROM curriculum_activity_requirements
                   WHERE term_slot_id = ANY($1)
               )"#,
            )
            .bind(&removed_ids)
            .fetch_one()
            .await?;
        if referenced {
            return Err(AppError::Conflict(
                "ย้ายหรือลบรายการในภาคเรียนนี้ก่อนลบภาคเรียนออกจากหลักสูตร".to_string(),
            ));
        }
    }

    let normalized_slots = request
        .slots
        .into_iter()
        .map(|slot| {
            (
                slot.id.unwrap_or_else(Uuid::new_v4),
                slot.sequence,
                slot.term_type,
                slot.type_occurrence,
                slot.name.trim().to_string(),
            )
        })
        .collect::<Vec<_>>();
    let retained_ids = normalized_slots
        .iter()
        .map(|slot| slot.0)
        .collect::<Vec<_>>();
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.curriculum_structure.replace_term_slots",
            r#"UPDATE curriculum_term_slots
           SET sequence = sequence + 100000,
               type_occurrence = type_occurrence + 100000
           WHERE curriculum_level_id = $1"#,
        )
        .bind(version_id)
        .execute()
        .await?;
    DbSession::new(&mut *transaction).query("academic_core.services.curriculum_structure.replace_term_slots",
        "DELETE FROM curriculum_term_slots WHERE curriculum_level_id = $1 AND NOT (id = ANY($2))",
    )
    .bind(version_id)
    .bind(&retained_ids)
    .execute()
    .await?;

    if !normalized_slots.is_empty() {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO curriculum_term_slots (id, curriculum_level_id, sequence, \
             term_type, type_occurrence, name) ",
        );
        builder.push_values(
            normalized_slots,
            |mut values, (id, sequence, term_type, type_occurrence, name)| {
                values
                    .push_bind(id)
                    .push_bind(version_id)
                    .push_bind(sequence)
                    .push_bind(term_type)
                    .push_bind(type_occurrence)
                    .push_bind(name);
            },
        );
        builder.push(
            " ON CONFLICT (id) DO UPDATE SET sequence = EXCLUDED.sequence, \
             term_type = EXCLUDED.term_type, type_occurrence = EXCLUDED.type_occurrence, \
             name = EXCLUDED.name, row_version = curriculum_term_slots.row_version + 1, \
             updated_at = now()",
        );
        DbSession::new(&mut *transaction)
            .prepared_query(
                "academic_core.services.curriculum_structure.replace_term_slots",
                builder.build(),
            )
            .execute()
            .await?;
    }

    DbSession::new(&mut *transaction).query("academic_core.services.curriculum_structure.replace_term_slots",
        "UPDATE curriculum_levels SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(version_id)
    .execute()
    .await?;
    transaction.commit().await?;
    get_workspace(pool, version_id).await
}

fn validate_requirement_inputs(
    requirements: &[CurriculumStructureRequirementInput],
) -> Result<(), AppError> {
    if requirements.len() > MAX_STRUCTURE_REQUIREMENTS {
        return Err(AppError::ValidationError(
            "จำนวนรายการในโครงสร้างหลักสูตรมากเกินขีดจำกัด".to_string(),
        ));
    }
    let mut keys = HashSet::new();
    for requirement in requirements {
        if requirement.display_order < 0 {
            return Err(AppError::ValidationError("ลำดับรายการต้องไม่ติดลบ".to_string()));
        }
        let key = (
            requirement.resource_kind,
            requirement.catalog_version_id,
            requirement.grade_level_id,
            requirement.term_slot_id,
        );
        if !keys.insert(key) {
            return Err(AppError::ValidationError(
                "มีรายวิชาหรือกิจกรรมซ้ำในระดับชั้นและภาคเรียนเดียวกัน".to_string(),
            ));
        }
    }
    Ok(())
}

async fn validate_new_catalog_requirements(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    program_id: Uuid,
    requirements: &[CurriculumStructureRequirementInput],
) -> Result<(), AppError> {
    // Keep a consistent cross-kind lock order; each provider lock sorts stable UUIDs.
    for kind in [
        RequirementResourceKind::Course,
        RequirementResourceKind::Activity,
    ] {
        let (existing_sql, versions_sql) = match kind {
            RequirementResourceKind::Course => (
                "SELECT subject_version_id,grade_level_id,term_slot_id FROM curriculum_course_requirements WHERE study_program_id=$1",
                "SELECT id,subject_id FROM subject_versions WHERE id=ANY($1)",
            ),
            RequirementResourceKind::Activity => (
                "SELECT activity_version_id,grade_level_id,term_slot_id FROM curriculum_activity_requirements WHERE study_program_id=$1",
                "SELECT id,activity_id FROM activity_versions WHERE id=ANY($1)",
            ),
        };
        let existing = DbSession::new(&mut **transaction)
            .query_as::<(Uuid, Uuid, Uuid)>(
                "academic_core.services.curriculum_structure.validate_new_catalog_requirements",
                existing_sql,
            )
            .bind(program_id)
            .fetch_all()
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
        let selected = requirements
            .iter()
            .filter(|item| item.resource_kind == kind)
            .collect::<Vec<_>>();
        let mut version_ids = existing
            .iter()
            .map(|key| key.0)
            .chain(selected.iter().map(|item| item.catalog_version_id))
            .collect::<Vec<_>>();
        version_ids.sort_unstable();
        version_ids.dedup();
        if version_ids.is_empty() {
            continue;
        }
        let catalogs = DbSession::new(&mut **transaction)
            .query_as::<(Uuid, Uuid)>(
                "academic_core.services.curriculum_structure.validate_new_catalog_requirements",
                versions_sql,
            )
            .bind(&version_ids)
            .fetch_all()
            .await?
            .into_iter()
            .collect::<HashMap<_, _>>();
        let catalog_ids = catalogs.values().copied().collect::<Vec<_>>();
        let current = match kind {
            RequirementResourceKind::Course => {
                super::catalog::lock_subjects_for_revision_selection(transaction, &catalog_ids)
                    .await?;
                super::catalog::current_subject_version_ids_on_connection(
                    &mut **transaction,
                    &catalog_ids,
                )
                .await?
            }
            RequirementResourceKind::Activity => {
                super::catalog::lock_activities_for_revision_selection(transaction, &catalog_ids)
                    .await?;
                super::catalog::current_activity_revisions_on_connection(
                    &mut **transaction,
                    &catalog_ids,
                )
                .await?
                .into_iter()
                .map(|(id, revision)| (id, revision.id))
                .collect()
            }
        };
        // Read current selection only after taking the same stable locks used by publication.
        let mut buckets = HashMap::<(Uuid, Uuid, Uuid), (usize, bool)>::new();
        for requirement in selected {
            let catalog_id = catalogs
                .get(&requirement.catalog_version_id)
                .ok_or_else(|| catalog_selection_conflict(kind))?;
            let preserved = existing.contains(&(
                requirement.catalog_version_id,
                requirement.grade_level_id,
                requirement.term_slot_id,
            ));
            if !preserved && current.get(catalog_id) != Some(&requirement.catalog_version_id) {
                return Err(catalog_selection_conflict(kind));
            }
            let bucket = buckets
                .entry((
                    *catalog_id,
                    requirement.grade_level_id,
                    requirement.term_slot_id,
                ))
                .or_default();
            bucket.0 += 1;
            bucket.1 |= !preserved;
        }
        if buckets
            .values()
            .any(|(count, has_new)| *count > 1 && *has_new)
        {
            let resource = catalog_requirement_name(kind);
            return Err(AppError::Conflict(format!(
                "มี{resource}เดียวกันซ้ำในชั้นและภาคเรียนนี้ กรุณาลบรายการเดิมก่อนเพิ่มเวอร์ชันใหม่"
            )));
        }
    }
    Ok(())
}

fn catalog_requirement_name(kind: RequirementResourceKind) -> &'static str {
    match kind {
        RequirementResourceKind::Course => "รายวิชา",
        RequirementResourceKind::Activity => "กิจกรรม",
    }
}

fn catalog_selection_conflict(kind: RequirementResourceKind) -> AppError {
    let resource = catalog_requirement_name(kind);
    AppError::Conflict(format!(
        "เวอร์ชัน{resource}เปลี่ยนหรือยังไม่มีเวอร์ชันที่ใช้ได้ในวันนี้ กรุณาโหลดตัวเลือกใหม่และเลือกเวอร์ชันปัจจุบัน"
    ))
}

async fn validate_requirement_ownership(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    program_id: Uuid,
    version_id: Uuid,
    requirements: &[CurriculumStructureRequirementInput],
) -> Result<(), AppError> {
    let slot_ids = requirements
        .iter()
        .map(|item| item.term_slot_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let slot_count: i64 = DbSession::new(&mut **transaction).scalar("academic_core.services.curriculum_structure.validate_requirement_ownership",
        "SELECT count(*) FROM curriculum_term_slots WHERE curriculum_level_id = $1 AND id = ANY($2)",
    )
    .bind(version_id)
    .bind(&slot_ids)
    .fetch_one()
    .await?;
    if slot_count != slot_ids.len() as i64 {
        return Err(AppError::ValidationError(
            "ภาคเรียนของหลักสูตรไม่อยู่ในเวอร์ชันนี้".to_string(),
        ));
    }

    let supported_grade_ids: sqlx::types::Json<Vec<Uuid>> = DbSession::new(&mut **transaction)
        .scalar(
            "academic_core.services.curriculum_structure.validate_requirement_ownership",
            r#"SELECT COALESCE(version.grade_level_ids, '[]'::jsonb)
           FROM curriculum_levels version
           JOIN curriculum_editions curriculum ON curriculum.id=version.edition_id
           WHERE version.id = $1"#,
        )
        .bind(version_id)
        .fetch_one()
        .await?;
    let supported_grade_ids = supported_grade_ids.0.into_iter().collect::<HashSet<_>>();
    if requirements
        .iter()
        .any(|item| !supported_grade_ids.contains(&item.grade_level_id))
    {
        return Err(AppError::ValidationError("ระดับชั้นไม่อยู่ในหลักสูตรนี้".to_string()));
    }

    let course_ids = requirements
        .iter()
        .filter(|item| item.resource_kind == RequirementResourceKind::Course)
        .map(|item| item.catalog_version_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let activity_ids = requirements
        .iter()
        .filter(|item| item.resource_kind == RequirementResourceKind::Activity)
        .map(|item| item.catalog_version_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let course_grades = load_catalog_grades(transaction, &course_ids, true).await?;
    let activity_grades = load_catalog_grades(transaction, &activity_ids, false).await?;

    // Retaining an exact historical pin does not admit a new grade or slot. Empty
    // immutable history remains empty; only its already persisted tuples survive.
    let existing = DbSession::new(&mut **transaction)
        .query_as::<(bool, Uuid, Uuid, Uuid)>(
            "academic_core.services.curriculum_structure.validate_requirement_ownership",
            "SELECT true,subject_version_id,grade_level_id,term_slot_id FROM curriculum_course_requirements WHERE study_program_id=$1 UNION ALL SELECT false,activity_version_id,grade_level_id,term_slot_id FROM curriculum_activity_requirements WHERE study_program_id=$1",
        )
        .bind(program_id)
        .fetch_all()
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    for requirement in requirements {
        let grades = match requirement.resource_kind {
            RequirementResourceKind::Course => course_grades.get(&requirement.catalog_version_id),
            RequirementResourceKind::Activity => {
                activity_grades.get(&requirement.catalog_version_id)
            }
        }
        .ok_or_else(|| {
            AppError::ValidationError("เลือกได้เฉพาะเวอร์ชันรายวิชาหรือกิจกรรมที่เผยแพร่แล้ว".to_string())
        })?;
        let preserved_empty_pin = grades.is_empty()
            && existing.contains(&(
                requirement.resource_kind == RequirementResourceKind::Course,
                requirement.catalog_version_id,
                requirement.grade_level_id,
                requirement.term_slot_id,
            ));
        if !grades.contains(&requirement.grade_level_id) && !preserved_empty_pin {
            return Err(AppError::ValidationError(
                "เวอร์ชันรายการที่เลือกไม่รองรับระดับชั้นนี้".to_string(),
            ));
        }
    }
    Ok(())
}

async fn load_catalog_grades(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    ids: &[Uuid],
    subject: bool,
) -> Result<HashMap<Uuid, HashSet<Uuid>>, AppError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = if subject {
        r#"SELECT version.id,
                  COALESCE(ARRAY(
                      SELECT grade.grade_level_id
                      FROM subject_version_grade_levels grade
                      WHERE grade.subject_id = version.id
                      ORDER BY grade.grade_level_id
                  ), ARRAY[]::uuid[]) AS grade_level_ids
           FROM subject_versions version
           WHERE version.id = ANY($1) AND version.status = 'published'"#
    } else {
        r#"SELECT id,
                  COALESCE(ARRAY(
                      SELECT jsonb_array_elements_text(
                          COALESCE(grade_level_ids, '[]'::jsonb)
                      )::uuid
                  ), ARRAY[]::uuid[]) AS grade_level_ids
           FROM activity_versions
           WHERE id = ANY($1) AND status = 'published'"#
    };
    let rows = DbSession::new(&mut **transaction)
        .query_as::<CatalogGradeRow>(
            "academic_core.services.curriculum_structure.load_catalog_grades",
            sql,
        )
        .bind(ids)
        .fetch_all()
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.id, row.grade_level_ids.into_iter().collect()))
        .collect())
}

fn requirement_from_row(
    row: RequirementRow,
    has_newer_current_version: bool,
    validation: &mut CurriculumStructureValidation,
) -> CurriculumStructureRequirement {
    if row.weekly_value.is_none() {
        validation.blockers.push(CurriculumValidationNotice {
            code: "catalog_weekly_value_missing".to_string(),
            message: format!("{} {} ยังไม่มีจำนวนคาบหรือชั่วโมงต่อสัปดาห์", row.code, row.name),
            catalog_version_id: Some(row.catalog_version_id),
        });
    }
    if row.total_hours.is_none() {
        validation.blockers.push(CurriculumValidationNotice {
            code: "catalog_total_hours_missing".to_string(),
            message: format!("{} {} ยังไม่มีชั่วโมงรวมต่อภาคเรียน", row.code, row.name),
            catalog_version_id: Some(row.catalog_version_id),
        });
    }
    let section = match row.resource_kind {
        RequirementResourceKind::Activity => CurriculumDocumentSection::StudentDevelopment,
        RequirementResourceKind::Course
            if row
                .catalog_classification
                .eq_ignore_ascii_case("ADDITIONAL") =>
        {
            CurriculumDocumentSection::AdditionalCourse
        }
        RequirementResourceKind::Course => CurriculumDocumentSection::BasicCourse,
    };
    let weekly_unit = if row.weekly_unit == "period" {
        CatalogWeeklyUnit::Period
    } else {
        CatalogWeeklyUnit::Hour
    };
    CurriculumStructureRequirement {
        id: row.id,
        study_program_id: row.study_program_id,
        grade_level: grade_level_item(GradeLevelRow {
            id: row.grade_level_id,
            level_type: row.grade_level_type,
            year: row.grade_level_year,
        }),
        term_slot_id: row.term_slot_id,
        resource_kind: row.resource_kind,
        catalog_version_id: row.catalog_version_id,
        catalog_id: row.catalog_id,
        version_no: row.version_no,
        has_newer_current_version,
        code: row.code,
        name: row.name,
        section,
        credit_category: match row.resource_kind {
            RequirementResourceKind::Course
                if row.catalog_classification.eq_ignore_ascii_case("BASIC") =>
            {
                Some(CurriculumCourseCreditCategory::Basic)
            }
            RequirementResourceKind::Course
                if row
                    .catalog_classification
                    .eq_ignore_ascii_case("ADDITIONAL") =>
            {
                Some(CurriculumCourseCreditCategory::Additional)
            }
            _ => None,
        },
        subject_group_id: None,
        subject_group_order: None,
        requirement_kind: row.requirement_kind,
        metrics: CatalogCurriculumMetrics {
            weekly_value: row.weekly_value,
            weekly_unit,
            credit: row.credit,
            total_hours: row.total_hours,
        },
        display_order: row.display_order,
    }
}

fn grade_level_item(row: GradeLevelRow) -> GradeLevelLookupItem {
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
            format!("ระดับอื่นปีที่ {}", row.year),
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
mod credit_category_tests {
    use super::*;

    #[test]
    fn credit_category_uses_pinned_classification_and_keeps_unknown_partial() {
        for (kind, classification, expected) in [
            (
                RequirementResourceKind::Course,
                "BASIC",
                Some(CurriculumCourseCreditCategory::Basic),
            ),
            (
                RequirementResourceKind::Course,
                "ADDITIONAL",
                Some(CurriculumCourseCreditCategory::Additional),
            ),
            (RequirementResourceKind::Course, "LEGACY-UNKNOWN", None),
            (RequirementResourceKind::Activity, "BASIC", None),
        ] {
            let row = RequirementRow {
                id: Uuid::new_v4(),
                study_program_id: Uuid::new_v4(),
                grade_level_id: Uuid::new_v4(),
                grade_level_type: "secondary".into(),
                grade_level_year: 1,
                term_slot_id: Uuid::new_v4(),
                resource_kind: kind,
                catalog_version_id: Uuid::new_v4(),
                catalog_id: Uuid::new_v4(),
                version_no: 1,
                code: "REAL-CODE".into(),
                name: "วิชาพื้นฐานชื่อไม่ใช่ประเภท".into(),
                catalog_classification: classification.into(),
                requirement_kind: RequirementKind::Required,
                weekly_value: Some("1".into()),
                weekly_unit: "period".into(),
                credit: Some("1".into()),
                total_hours: Some("20".into()),
                display_order: 1,
            };
            let result =
                requirement_from_row(row, false, &mut CurriculumStructureValidation::default());
            assert_eq!(result.credit_category, expected);
            if classification == "LEGACY-UNKNOWN" {
                assert_eq!(result.section, CurriculumDocumentSection::BasicCourse);
            }
        }
    }
}
