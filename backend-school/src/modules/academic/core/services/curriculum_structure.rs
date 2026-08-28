use std::collections::{HashMap, HashSet};

use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{error::AppError, modules::lookup::models::GradeLevelLookupItem};

use super::super::models::{
    CatalogCurriculumMetrics, CatalogWeeklyUnit, CurriculumDocumentSection,
    CurriculumStructureRequirement, CurriculumStructureRequirementInput,
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
    curriculum_version_id: Uuid,
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
    let curriculum_version = curriculum::get_version(pool, version_id).await?;
    let term_slots = sqlx::query_as::<_, CurriculumTermSlot>(
        r#"SELECT id, curriculum_version_id, sequence, term_type, type_occurrence,
                  name, row_version
           FROM curriculum_term_slots
           WHERE curriculum_version_id = $1
           ORDER BY sequence, id"#,
    )
    .bind(version_id)
    .fetch_all(pool)
    .await?;
    let programs = curriculum::list_programs_for_version(pool, version_id).await?;
    let grade_levels = sqlx::query_as::<_, GradeLevelRow>(
        r#"SELECT grade.id, grade.level_type, grade.year
           FROM curriculum_versions version
           JOIN curricula curriculum ON curriculum.id = version.curriculum_id
           JOIN grade_levels grade ON grade.id IN (
               SELECT jsonb_array_elements_text(
                   COALESCE(curriculum.grade_level_ids, '[]'::jsonb)
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
                    grade.id"#,
    )
    .bind(version_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(grade_level_item)
    .collect::<Vec<_>>();
    let rows = sqlx::query_as::<_, RequirementRow>(
        r#"SELECT requirement.id,
                  requirement.study_program_id,
                  requirement.grade_level_id,
                  grade.level_type AS grade_level_type,
                  grade.year AS grade_level_year,
                  requirement.term_slot_id,
                  'course'::text AS resource_kind,
                  requirement.subject_version_id AS catalog_version_id,
                  subject.code,
                  version.name_th AS name,
                  version.type AS catalog_classification,
                  requirement.requirement_kind,
                  version.periods_per_week::text AS weekly_value,
                  'period'::text AS weekly_unit,
                  version.credit::text AS credit,
                  version.hours_per_semester::text AS total_hours,
                  requirement.display_order
           FROM curriculum_course_requirements requirement
           JOIN study_programs program ON program.id = requirement.study_program_id
           JOIN grade_levels grade ON grade.id = requirement.grade_level_id
           JOIN subject_versions version ON version.id = requirement.subject_version_id
           JOIN subjects subject ON subject.id = version.subject_id
           WHERE program.curriculum_version_id = $1
           UNION ALL
           SELECT requirement.id,
                  requirement.study_program_id,
                  requirement.grade_level_id,
                  grade.level_type AS grade_level_type,
                  grade.year AS grade_level_year,
                  requirement.term_slot_id,
                  'activity'::text AS resource_kind,
                  requirement.activity_version_id AS catalog_version_id,
                  activity.code,
                  version.name,
                  activity.activity_type AS catalog_classification,
                  requirement.requirement_kind,
                  version.hours_per_week::text AS weekly_value,
                  'hour'::text AS weekly_unit,
                  NULL::text AS credit,
                  version.hours_per_term::text AS total_hours,
                  requirement.display_order
           FROM curriculum_activity_requirements requirement
           JOIN study_programs program ON program.id = requirement.study_program_id
           JOIN grade_levels grade ON grade.id = requirement.grade_level_id
           JOIN activity_versions version ON version.id = requirement.activity_version_id
           JOIN activities activity ON activity.id = version.activity_id
           WHERE program.curriculum_version_id = $1
           ORDER BY study_program_id, grade_level_year, term_slot_id,
                    display_order, resource_kind, catalog_version_id"#,
    )
    .bind(version_id)
    .fetch_all(pool)
    .await?;

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
        .map(|row| requirement_from_row(row, &mut validation))
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
    let row_version = curriculum_version.row_version;
    Ok(CurriculumStructureWorkspace {
        curriculum_version,
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

    let mut transaction = pool.begin().await?;
    let program = sqlx::query_as::<_, ProgramLockRow>(
        r#"SELECT program.curriculum_version_id,
                  program.row_version,
                  version.status AS version_status
           FROM study_programs program
           JOIN curriculum_versions version ON version.id = program.curriculum_version_id
           WHERE program.id = $1
           FOR UPDATE OF program, version"#,
    )
    .bind(program_id)
    .fetch_optional(&mut *transaction)
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

    validate_requirement_ownership(
        &mut transaction,
        program.curriculum_version_id,
        &request.requirements,
    )
    .await?;

    sqlx::query("DELETE FROM curriculum_course_requirements WHERE study_program_id = $1")
        .bind(program_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM curriculum_activity_requirements WHERE study_program_id = $1")
        .bind(program_id)
        .execute(&mut *transaction)
        .await?;

    let course_requirements = request
        .requirements
        .iter()
        .filter(|item| item.resource_kind == RequirementResourceKind::Course)
        .collect::<Vec<_>>();
    if !course_requirements.is_empty() {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO curriculum_course_requirements (id, curriculum_version_id, \
             study_program_id, subject_version_id, grade_level_id, term_slot_id, \
             requirement_kind, display_order) ",
        );
        builder.push_values(course_requirements, |mut values, requirement| {
            values
                .push_bind(Uuid::new_v4())
                .push_bind(program.curriculum_version_id)
                .push_bind(program_id)
                .push_bind(requirement.catalog_version_id)
                .push_bind(requirement.grade_level_id)
                .push_bind(requirement.term_slot_id)
                .push_bind(requirement.requirement_kind)
                .push_bind(requirement.display_order);
        });
        builder.build().execute(&mut *transaction).await?;
    }

    let activity_requirements = request
        .requirements
        .iter()
        .filter(|item| item.resource_kind == RequirementResourceKind::Activity)
        .collect::<Vec<_>>();
    if !activity_requirements.is_empty() {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO curriculum_activity_requirements (id, curriculum_version_id, \
             study_program_id, activity_version_id, grade_level_id, term_slot_id, \
             requirement_kind, display_order) ",
        );
        builder.push_values(activity_requirements, |mut values, requirement| {
            values
                .push_bind(Uuid::new_v4())
                .push_bind(program.curriculum_version_id)
                .push_bind(program_id)
                .push_bind(requirement.catalog_version_id)
                .push_bind(requirement.grade_level_id)
                .push_bind(requirement.term_slot_id)
                .push_bind(requirement.requirement_kind)
                .push_bind(requirement.display_order);
        });
        builder.build().execute(&mut *transaction).await?;
    }

    sqlx::query(
        "UPDATE study_programs SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(program_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    get_workspace(pool, program.curriculum_version_id).await
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

    let mut transaction = pool.begin().await?;
    let (status, current_row_version): (VersionStatus, i64) = sqlx::query_as(
        "SELECT status, row_version FROM curriculum_versions WHERE id = $1 FOR UPDATE",
    )
    .bind(version_id)
    .fetch_optional(&mut *transaction)
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

    let existing_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM curriculum_term_slots WHERE curriculum_version_id = $1 ORDER BY id FOR UPDATE",
    )
    .bind(version_id)
    .fetch_all(&mut *transaction)
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
        let referenced: bool = sqlx::query_scalar(
            r#"SELECT EXISTS (
                   SELECT 1 FROM curriculum_course_requirements
                   WHERE term_slot_id = ANY($1)
               ) OR EXISTS (
                   SELECT 1 FROM curriculum_activity_requirements
                   WHERE term_slot_id = ANY($1)
               )"#,
        )
        .bind(&removed_ids)
        .fetch_one(&mut *transaction)
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
    sqlx::query(
        r#"UPDATE curriculum_term_slots
           SET sequence = sequence + 100000,
               type_occurrence = type_occurrence + 100000
           WHERE curriculum_version_id = $1"#,
    )
    .bind(version_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "DELETE FROM curriculum_term_slots WHERE curriculum_version_id = $1 AND NOT (id = ANY($2))",
    )
    .bind(version_id)
    .bind(&retained_ids)
    .execute(&mut *transaction)
    .await?;

    if !normalized_slots.is_empty() {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO curriculum_term_slots (id, curriculum_version_id, sequence, \
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
        builder.build().execute(&mut *transaction).await?;
    }

    sqlx::query(
        "UPDATE curriculum_versions SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(version_id)
    .execute(&mut *transaction)
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

async fn validate_requirement_ownership(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    version_id: Uuid,
    requirements: &[CurriculumStructureRequirementInput],
) -> Result<(), AppError> {
    let slot_ids = requirements
        .iter()
        .map(|item| item.term_slot_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let slot_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM curriculum_term_slots WHERE curriculum_version_id = $1 AND id = ANY($2)",
    )
    .bind(version_id)
    .bind(&slot_ids)
    .fetch_one(&mut **transaction)
    .await?;
    if slot_count != slot_ids.len() as i64 {
        return Err(AppError::ValidationError(
            "ภาคเรียนของหลักสูตรไม่อยู่ในเวอร์ชันนี้".to_string(),
        ));
    }

    let supported_grade_ids: sqlx::types::Json<Vec<Uuid>> = sqlx::query_scalar(
        r#"SELECT COALESCE(curriculum.grade_level_ids, '[]'::jsonb)
           FROM curriculum_versions version
           JOIN curricula curriculum ON curriculum.id = version.curriculum_id
           WHERE version.id = $1"#,
    )
    .bind(version_id)
    .fetch_one(&mut **transaction)
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
        if !grades.contains(&requirement.grade_level_id) {
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
    let rows = sqlx::query_as::<_, CatalogGradeRow>(sql)
        .bind(ids)
        .fetch_all(&mut **transaction)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.id, row.grade_level_ids.into_iter().collect()))
        .collect())
}

fn requirement_from_row(
    row: RequirementRow,
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
        code: row.code,
        name: row.name,
        section,
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
