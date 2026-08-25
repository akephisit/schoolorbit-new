use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

use super::super::models::{
    CreateCurriculumRequest, CreateCurriculumVersionRequest, CreateStudyProgramRequest, Curriculum,
    CurriculumVersion, ProgramRequirement, ProgramRequirementInput, PublishVersionRequest,
    ReplaceProgramRequirementsRequest, RequirementResourceKind, StudyProgram, StudyProgramOption,
    StudyProgramRequirement, UpdateCurriculumRequest, UpdateCurriculumVersionRequest,
    UpdateStudyProgramRequest, VersionStatus,
};
use super::{ensure_draft_version, parse_row_version, validate_canonical_decimal};

const VERSION_COLUMNS: &str = r#"
    id, curriculum_id, version_name, start_academic_year_id, end_academic_year_id,
    description, status, published_at, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;
const PROGRAM_COLUMNS: &str = r#"
    id, curriculum_version_id, code, name_th, name_en, is_default, status,
    owning_organization_unit_id, row_version, created_at, updated_at
"#;
const MAX_STUDY_PROGRAM_OPTIONS: usize = 2_000;

pub async fn list(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<Curriculum>, AppError> {
    let owner_ids = filter.allowed_organization_unit_ids();
    Ok(sqlx::query_as(
        r#"
        SELECT id, code, name_th, name_en, description, is_active,
               COALESCE(ARRAY(
                   SELECT jsonb_array_elements_text(COALESCE(grade_level_ids, '[]'::jsonb))::uuid
               ), ARRAY[]::uuid[]) AS grade_level_ids,
               owning_organization_unit_id, row_version, created_at, updated_at
        FROM curricula
        WHERE $1 OR owning_organization_unit_id = ANY($2)
        ORDER BY code, id
        LIMIT 500
        "#,
    )
    .bind(filter.includes_school_owned)
    .bind(owner_ids)
    .fetch_all(pool)
    .await?)
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Curriculum, AppError> {
    sqlx::query_as(
        r#"
        SELECT id, code, name_th, name_en, description, is_active,
               COALESCE(ARRAY(
                   SELECT jsonb_array_elements_text(COALESCE(grade_level_ids, '[]'::jsonb))::uuid
               ), ARRAY[]::uuid[]) AS grade_level_ids,
               owning_organization_unit_id, row_version, created_at, updated_at
        FROM curricula WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบหลักสูตร".to_string()))
}

pub async fn create(
    pool: &PgPool,
    request: CreateCurriculumRequest,
) -> Result<Curriculum, AppError> {
    validate_curriculum_fields(&request.code, &request.name_th)?;
    validate_grade_levels(pool, &request.grade_level_ids).await?;
    let grade_level_ids = sqlx::types::Json(request.grade_level_ids.clone());
    let code = request.code.trim().to_uppercase();
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO curricula (
            id, code, identity_key, name_th, name_en, description, is_active,
            grade_level_ids, owning_organization_unit_id
        ) VALUES ($1, $2, $3, $4, $5, $6, true, $7, $8)
        "#,
    )
    .bind(id)
    .bind(&code)
    .bind(code.to_lowercase())
    .bind(request.name_th.trim())
    .bind(request.name_en)
    .bind(request.description)
    .bind(grade_level_ids)
    .bind(request.owning_organization_unit_id)
    .execute(pool)
    .await
    .map_err(map_curriculum_write_error)?;
    get(pool, id).await
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    request: UpdateCurriculumRequest,
) -> Result<Curriculum, AppError> {
    parse_row_version(request.row_version)?;
    validate_curriculum_fields(&request.code, &request.name_th)?;
    validate_grade_levels(pool, &request.grade_level_ids).await?;
    let grade_level_ids = sqlx::types::Json(request.grade_level_ids.clone());
    let code = request.code.trim().to_uppercase();
    let result = sqlx::query(
        r#"
        UPDATE curricula SET code = $1, identity_key = $2, name_th = $3, name_en = $4,
            description = $5, grade_level_ids = $6, owning_organization_unit_id = $7,
            row_version = row_version + 1, updated_at = now()
        WHERE id = $8 AND row_version = $9
        "#,
    )
    .bind(&code)
    .bind(code.to_lowercase())
    .bind(request.name_th.trim())
    .bind(request.name_en)
    .bind(request.description)
    .bind(grade_level_ids)
    .bind(request.owning_organization_unit_id)
    .bind(id)
    .bind(request.row_version)
    .execute(pool)
    .await
    .map_err(map_curriculum_write_error)?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict("หลักสูตรถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    get(pool, id).await
}

pub async fn list_versions(
    pool: &PgPool,
    curriculum_id: Uuid,
) -> Result<Vec<CurriculumVersion>, AppError> {
    get(pool, curriculum_id).await?;
    let sql = format!(
        "SELECT {VERSION_COLUMNS} FROM curriculum_versions WHERE curriculum_id = $1 \
         ORDER BY created_at DESC, id"
    );
    Ok(sqlx::query_as(&sql)
        .bind(curriculum_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get_version(pool: &PgPool, id: Uuid) -> Result<CurriculumVersion, AppError> {
    let sql = format!("SELECT {VERSION_COLUMNS} FROM curriculum_versions WHERE id = $1");
    sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันหลักสูตร".to_string()))
}

pub async fn create_version(
    pool: &PgPool,
    curriculum_id: Uuid,
    request: CreateCurriculumVersionRequest,
) -> Result<CurriculumVersion, AppError> {
    validate_version_fields(pool, &request).await?;
    get(pool, curriculum_id).await?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO curriculum_versions (
            id, curriculum_id, version_name, start_academic_year_id,
            end_academic_year_id, description, is_active, status
        ) VALUES ($1, $2, $3, $4, $5, $6, true, 'draft')
        "#,
    )
    .bind(id)
    .bind(curriculum_id)
    .bind(request.version_name.trim())
    .bind(request.start_academic_year_id)
    .bind(request.end_academic_year_id)
    .bind(request.description)
    .execute(pool)
    .await?;
    get_version(pool, id).await
}

pub async fn update_version(
    pool: &PgPool,
    id: Uuid,
    request: UpdateCurriculumVersionRequest,
) -> Result<CurriculumVersion, AppError> {
    let row_version = request.row_version;
    let values = CreateCurriculumVersionRequest {
        version_name: request.version_name,
        start_academic_year_id: request.start_academic_year_id,
        end_academic_year_id: request.end_academic_year_id,
        description: request.description,
    };
    parse_row_version(row_version)?;
    validate_version_fields(pool, &values).await?;
    let status: VersionStatus =
        sqlx::query_scalar("SELECT status FROM curriculum_versions WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันหลักสูตร".to_string()))?;
    ensure_draft_version(status)?;
    let result = sqlx::query(
        r#"
        UPDATE curriculum_versions SET version_name = $1, start_academic_year_id = $2,
            end_academic_year_id = $3, description = $4,
            row_version = row_version + 1, updated_at = now()
        WHERE id = $5 AND row_version = $6 AND status = 'draft'
        "#,
    )
    .bind(values.version_name.trim())
    .bind(values.start_academic_year_id)
    .bind(values.end_academic_year_id)
    .bind(values.description)
    .bind(id)
    .bind(row_version)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "เวอร์ชันหลักสูตรถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    get_version(pool, id).await
}

pub async fn publish_version(
    pool: &PgPool,
    id: Uuid,
    request: PublishVersionRequest,
) -> Result<CurriculumVersion, AppError> {
    parse_row_version(request.row_version)?;
    let mut transaction = pool.begin().await?;
    let version: CurriculumVersion = {
        let sql =
            format!("SELECT {VERSION_COLUMNS} FROM curriculum_versions WHERE id = $1 FOR UPDATE");
        sqlx::query_as(&sql)
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันหลักสูตร".to_string()))?
    };
    ensure_draft_version(version.status)?;
    if version.row_version != request.row_version {
        return Err(AppError::Conflict(
            "เวอร์ชันหลักสูตรถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    validate_publishable(&mut transaction, &version).await?;
    sqlx::query(
        "UPDATE study_programs SET status = 'published', row_version = row_version + 1, \
         updated_at = now() WHERE curriculum_version_id = $1 AND status = 'draft'",
    )
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE curriculum_versions SET status = 'published', published_at = now(), \
         row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    get_version(pool, id).await
}

pub async fn list_programs(pool: &PgPool, version_id: Uuid) -> Result<Vec<StudyProgram>, AppError> {
    get_version(pool, version_id).await?;
    list_programs_for_version(pool, version_id).await
}

pub(super) async fn list_programs_for_version(
    pool: &PgPool,
    version_id: Uuid,
) -> Result<Vec<StudyProgram>, AppError> {
    let sql = format!(
        "SELECT {PROGRAM_COLUMNS} FROM study_programs WHERE curriculum_version_id = $1 \
         ORDER BY is_default DESC, code, id"
    );
    Ok(sqlx::query_as(&sql)
        .bind(version_id)
        .fetch_all(pool)
        .await?)
}

#[derive(sqlx::FromRow)]
struct StudyProgramRequirementRow {
    study_program_id: Uuid,
    id: Uuid,
    resource_kind: RequirementResourceKind,
    catalog_version_id: Uuid,
    grade_level_id: Uuid,
    recommended_term_code: Option<String>,
    requirement_kind: super::super::models::RequirementKind,
    credit: Option<String>,
    hours: Option<String>,
    display_order: i32,
    row_version: i64,
}

impl From<StudyProgramRequirementRow> for StudyProgramRequirement {
    fn from(row: StudyProgramRequirementRow) -> Self {
        Self {
            study_program_id: row.study_program_id,
            requirement: ProgramRequirement {
                id: row.id,
                resource_kind: row.resource_kind,
                catalog_version_id: row.catalog_version_id,
                grade_level_id: row.grade_level_id,
                recommended_term_code: row.recommended_term_code,
                requirement_kind: row.requirement_kind,
                credit: row.credit,
                hours: row.hours,
                display_order: row.display_order,
                row_version: row.row_version,
            },
        }
    }
}

pub(super) async fn list_requirements_for_programs(
    pool: &PgPool,
    program_ids: &[Uuid],
) -> Result<Vec<StudyProgramRequirement>, AppError> {
    if program_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<StudyProgramRequirementRow> = sqlx::query_as(
        r#"
        WITH requirement_rows AS (
            SELECT requirement.study_program_id, requirement.id,
                   'course'::text AS resource_kind,
                   requirement.subject_version_id AS catalog_version_id,
                   requirement.grade_level_id, requirement.recommended_term_code,
                   requirement.requirement_kind, requirement.credit::text AS credit,
                   requirement.hours::text AS hours,
                   COALESCE(requirement.display_order, 0) AS display_order,
                   requirement.row_version
            FROM curriculum_course_requirements requirement
            WHERE requirement.study_program_id = ANY($1)
            UNION ALL
            SELECT requirement.study_program_id, requirement.id,
                   'activity'::text AS resource_kind,
                   requirement.activity_version_id AS catalog_version_id,
                   requirement.grade_level_id, requirement.recommended_term_code,
                   requirement.requirement_kind, NULL::text AS credit,
                   requirement.hours::text AS hours,
                   requirement.display_order, requirement.row_version
            FROM curriculum_activity_requirements requirement
            WHERE requirement.study_program_id = ANY($1)
        )
        SELECT requirement.study_program_id, requirement.id, requirement.resource_kind,
               requirement.catalog_version_id, requirement.grade_level_id,
               requirement.recommended_term_code, requirement.requirement_kind,
               requirement.credit, requirement.hours, requirement.display_order,
               requirement.row_version
        FROM requirement_rows requirement
        JOIN study_programs program ON program.id = requirement.study_program_id
        ORDER BY program.is_default DESC, program.code, program.id,
                 requirement.display_order, requirement.resource_kind,
                 requirement.catalog_version_id, requirement.id
        "#,
    )
    .bind(program_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn list_study_program_options_for_year(
    pool: &PgPool,
    academic_year_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<StudyProgramOption>, AppError> {
    let target_year: (chrono::NaiveDate, chrono::NaiveDate) =
        sqlx::query_as("SELECT start_date, end_date FROM academic_years WHERE id = $1")
            .bind(academic_year_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษา".to_string()))?;
    let owner_ids = filter.allowed_organization_unit_ids();
    let options: Vec<StudyProgramOption> = sqlx::query_as(
        r#"
        SELECT program.id, program.code, program.name_th AS name,
               curriculum.id AS curriculum_id, curriculum.name_th AS curriculum_name
        FROM study_programs program
        JOIN curriculum_versions version ON version.id = program.curriculum_version_id
        JOIN curricula curriculum ON curriculum.id = version.curriculum_id
        JOIN academic_years starts ON starts.id = version.start_academic_year_id
        LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
        WHERE program.status = 'published'
          AND version.status = 'published'
          AND curriculum.is_active IS TRUE
          AND starts.start_date <= $1
          AND (ends.end_date IS NULL OR ends.end_date >= $2)
          AND ($3 OR curriculum.owning_organization_unit_id = ANY($4))
        ORDER BY curriculum.code, curriculum.id, program.is_default DESC, program.code, program.id
        LIMIT $5
        "#,
    )
    .bind(target_year.0)
    .bind(target_year.1)
    .bind(filter.includes_school_owned)
    .bind(owner_ids)
    .bind((MAX_STUDY_PROGRAM_OPTIONS + 1) as i64)
    .fetch_all(pool)
    .await?;
    if options.len() > MAX_STUDY_PROGRAM_OPTIONS {
        return Err(AppError::ValidationError(
            "จำนวนตัวเลือกแผนการเรียนในปีการศึกษามากเกิน 2,000 รายการ".to_string(),
        ));
    }
    Ok(options)
}

pub async fn get_program(pool: &PgPool, id: Uuid) -> Result<StudyProgram, AppError> {
    let sql = format!("SELECT {PROGRAM_COLUMNS} FROM study_programs WHERE id = $1");
    sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียน".to_string()))
}

pub async fn create_program(
    pool: &PgPool,
    version_id: Uuid,
    request: CreateStudyProgramRequest,
) -> Result<StudyProgram, AppError> {
    validate_program_fields(&request.code, &request.name_th)?;
    let mut transaction = pool.begin().await?;
    require_draft_curriculum_version(&mut transaction, version_id).await?;
    if request.is_default {
        clear_default_program(&mut transaction, version_id, None).await?;
    }
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO study_programs (
            id, curriculum_version_id, code, name_th, name_en, is_default,
            status, owning_organization_unit_id
        ) VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7)
        "#,
    )
    .bind(id)
    .bind(version_id)
    .bind(request.code.trim().to_uppercase())
    .bind(request.name_th.trim())
    .bind(request.name_en)
    .bind(request.is_default)
    .bind(request.owning_organization_unit_id)
    .execute(&mut *transaction)
    .await
    .map_err(map_program_write_error)?;
    transaction.commit().await?;
    get_program(pool, id).await
}

pub async fn update_program(
    pool: &PgPool,
    id: Uuid,
    request: UpdateStudyProgramRequest,
) -> Result<StudyProgram, AppError> {
    parse_row_version(request.row_version)?;
    validate_program_fields(&request.code, &request.name_th)?;
    let mut transaction = pool.begin().await?;
    let (version_id, status): (Uuid, VersionStatus) = sqlx::query_as(
        "SELECT curriculum_version_id, status FROM study_programs WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียน".to_string()))?;
    ensure_draft_version(status)?;
    require_draft_curriculum_version(&mut transaction, version_id).await?;
    if request.is_default {
        clear_default_program(&mut transaction, version_id, Some(id)).await?;
    }
    let result = sqlx::query(
        r#"
        UPDATE study_programs SET code = $1, name_th = $2, name_en = $3,
            is_default = $4, owning_organization_unit_id = $5,
            row_version = row_version + 1, updated_at = now()
        WHERE id = $6 AND row_version = $7 AND status = 'draft'
        "#,
    )
    .bind(request.code.trim().to_uppercase())
    .bind(request.name_th.trim())
    .bind(request.name_en)
    .bind(request.is_default)
    .bind(request.owning_organization_unit_id)
    .bind(id)
    .bind(request.row_version)
    .execute(&mut *transaction)
    .await
    .map_err(map_program_write_error)?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "แผนการเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    transaction.commit().await?;
    get_program(pool, id).await
}

pub async fn list_requirements(
    pool: &PgPool,
    program_id: Uuid,
) -> Result<Vec<ProgramRequirement>, AppError> {
    get_program(pool, program_id).await?;
    Ok(sqlx::query_as(
        r#"
        SELECT requirement.id, 'course'::text AS resource_kind,
               requirement.subject_version_id AS catalog_version_id,
               requirement.grade_level_id, requirement.recommended_term_code,
               requirement.requirement_kind, requirement.credit::text AS credit,
               requirement.hours::text AS hours,
               COALESCE(requirement.display_order, 0) AS display_order,
               requirement.row_version
        FROM curriculum_course_requirements requirement
        WHERE requirement.study_program_id = $1
        UNION ALL
        SELECT requirement.id, 'activity'::text AS resource_kind,
               requirement.activity_version_id AS catalog_version_id,
               requirement.grade_level_id, requirement.recommended_term_code,
               requirement.requirement_kind, NULL::text AS credit,
               requirement.hours::text AS hours,
               requirement.display_order, requirement.row_version
        FROM curriculum_activity_requirements requirement
        WHERE requirement.study_program_id = $1
        ORDER BY display_order, resource_kind, catalog_version_id
        "#,
    )
    .bind(program_id)
    .fetch_all(pool)
    .await?)
}

pub async fn replace_requirements(
    pool: &PgPool,
    program_id: Uuid,
    request: ReplaceProgramRequirementsRequest,
) -> Result<Vec<ProgramRequirement>, AppError> {
    parse_row_version(request.row_version)?;
    validate_requirements(&request.requirements)?;
    let mut transaction = pool.begin().await?;
    let (version_id, status, actual): (Uuid, VersionStatus, i64) = sqlx::query_as(
        "SELECT curriculum_version_id, status, row_version FROM study_programs WHERE id = $1 FOR UPDATE",
    )
    .bind(program_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแผนการเรียน".to_string()))?;
    ensure_draft_version(status)?;
    require_draft_curriculum_version(&mut transaction, version_id).await?;
    if actual != request.row_version {
        return Err(AppError::Conflict(
            "แผนการเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    validate_requirement_resources(&mut transaction, &request.requirements).await?;
    sqlx::query("DELETE FROM curriculum_course_requirements WHERE study_program_id = $1")
        .bind(program_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM curriculum_activity_requirements WHERE study_program_id = $1")
        .bind(program_id)
        .execute(&mut *transaction)
        .await?;
    for requirement in request.requirements {
        insert_requirement(&mut transaction, version_id, program_id, requirement).await?;
    }
    sqlx::query(
        "UPDATE study_programs SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(program_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    list_requirements(pool, program_id).await
}

fn validate_curriculum_fields(code: &str, name_th: &str) -> Result<(), AppError> {
    if code.trim().is_empty() || name_th.trim().is_empty() {
        return Err(AppError::ValidationError(
            "รหัสและชื่อหลักสูตรห้ามว่าง".to_string(),
        ));
    }
    Ok(())
}

fn validate_program_fields(code: &str, name_th: &str) -> Result<(), AppError> {
    if code.trim().is_empty() || name_th.trim().is_empty() {
        return Err(AppError::ValidationError(
            "รหัสและชื่อแผนการเรียนห้ามว่าง".to_string(),
        ));
    }
    Ok(())
}

async fn validate_version_fields(
    pool: &PgPool,
    request: &CreateCurriculumVersionRequest,
) -> Result<(), AppError> {
    if request.version_name.trim().is_empty() {
        return Err(AppError::ValidationError(
            "ชื่อเวอร์ชันหลักสูตรห้ามว่าง".to_string(),
        ));
    }
    let start: chrono::NaiveDate =
        sqlx::query_scalar("SELECT start_date FROM academic_years WHERE id = $1")
            .bind(request.start_academic_year_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::ValidationError("ไม่พบปีเริ่มใช้หลักสูตร".to_string()))?;
    if let Some(end_id) = request.end_academic_year_id {
        let end: chrono::NaiveDate =
            sqlx::query_scalar("SELECT end_date FROM academic_years WHERE id = $1")
                .bind(end_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::ValidationError("ไม่พบปีสิ้นสุดหลักสูตร".to_string()))?;
        if end < start {
            return Err(AppError::ValidationError(
                "ช่วงปีของหลักสูตรไม่ถูกต้อง".to_string(),
            ));
        }
    }
    Ok(())
}

async fn validate_grade_levels(pool: &PgPool, ids: &[Uuid]) -> Result<(), AppError> {
    let unique = ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if unique.len() != ids.len() {
        return Err(AppError::ValidationError("ระดับชั้นในหลักสูตรซ้ำกัน".to_string()));
    }
    if ids.is_empty() {
        return Ok(());
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM grade_levels WHERE id = ANY($1)")
        .bind(ids)
        .fetch_one(pool)
        .await?;
    if count != ids.len() as i64 {
        return Err(AppError::ValidationError(
            "ระดับชั้นในหลักสูตรไม่ถูกต้อง".to_string(),
        ));
    }
    Ok(())
}

fn validate_requirements(requirements: &[ProgramRequirementInput]) -> Result<(), AppError> {
    let mut seen = std::collections::HashSet::new();
    for requirement in requirements {
        let key = (
            requirement.resource_kind,
            requirement.catalog_version_id,
            requirement.grade_level_id,
            requirement.recommended_term_code.clone(),
        );
        if !seen.insert(key) {
            return Err(AppError::ValidationError("ข้อกำหนดหลักสูตรซ้ำกัน".to_string()));
        }
        match requirement.resource_kind {
            RequirementResourceKind::Course => {
                let credit = requirement
                    .credit
                    .as_deref()
                    .ok_or_else(|| AppError::ValidationError("รายวิชาต้องระบุหน่วยกิต".to_string()))?;
                validate_canonical_decimal(credit, 2)?;
                if let Some(hours) = &requirement.hours {
                    validate_canonical_decimal(hours, 2)?;
                }
            }
            RequirementResourceKind::Activity => {
                if requirement.credit.is_some() {
                    return Err(AppError::ValidationError("กิจกรรมไม่มีหน่วยกิต".to_string()));
                }
                validate_canonical_decimal(
                    requirement.hours.as_deref().ok_or_else(|| {
                        AppError::ValidationError("กิจกรรมต้องระบุชั่วโมง".to_string())
                    })?,
                    2,
                )?;
            }
        }
    }
    Ok(())
}

async fn validate_requirement_resources(
    transaction: &mut Transaction<'_, Postgres>,
    requirements: &[ProgramRequirementInput],
) -> Result<(), AppError> {
    for requirement in requirements {
        let exists: bool = match requirement.resource_kind {
            RequirementResourceKind::Course => {
                sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM subject_versions WHERE id = $1)")
                    .bind(requirement.catalog_version_id)
                    .fetch_one(&mut **transaction)
                    .await?
            }
            RequirementResourceKind::Activity => {
                sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM activity_versions WHERE id = $1)")
                    .bind(requirement.catalog_version_id)
                    .fetch_one(&mut **transaction)
                    .await?
            }
        };
        if !exists {
            return Err(AppError::ValidationError(
                "เวอร์ชันวิชาหรือกิจกรรมไม่ถูกต้อง".to_string(),
            ));
        }
        let grade_exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM grade_levels WHERE id = $1)")
                .bind(requirement.grade_level_id)
                .fetch_one(&mut **transaction)
                .await?;
        if !grade_exists {
            return Err(AppError::ValidationError(
                "ระดับชั้นของข้อกำหนดไม่ถูกต้อง".to_string(),
            ));
        }
    }
    Ok(())
}

async fn insert_requirement(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
    program_id: Uuid,
    requirement: ProgramRequirementInput,
) -> Result<(), AppError> {
    match requirement.resource_kind {
        RequirementResourceKind::Course => {
            let credit =
                validate_canonical_decimal(requirement.credit.as_deref().unwrap_or(""), 2)?;
            let hours = requirement
                .hours
                .as_deref()
                .map(|value| validate_canonical_decimal(value, 2))
                .transpose()?;
            sqlx::query(
                r#"
                INSERT INTO curriculum_course_requirements (
                    id, curriculum_version_id, study_program_id, subject_version_id,
                    grade_level_id, recommended_term_code, requirement_kind,
                    credit, hours, display_order
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(version_id)
            .bind(program_id)
            .bind(requirement.catalog_version_id)
            .bind(requirement.grade_level_id)
            .bind(requirement.recommended_term_code)
            .bind(requirement.requirement_kind)
            .bind(credit)
            .bind(hours)
            .bind(requirement.display_order)
            .execute(&mut **transaction)
            .await?;
        }
        RequirementResourceKind::Activity => {
            let hours = validate_canonical_decimal(requirement.hours.as_deref().unwrap_or(""), 2)?;
            sqlx::query(
                r#"
                INSERT INTO curriculum_activity_requirements (
                    id, curriculum_version_id, study_program_id, activity_version_id,
                    grade_level_id, recommended_term_code, requirement_kind,
                    hours, display_order
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(version_id)
            .bind(program_id)
            .bind(requirement.catalog_version_id)
            .bind(requirement.grade_level_id)
            .bind(requirement.recommended_term_code)
            .bind(requirement.requirement_kind)
            .bind(hours)
            .bind(requirement.display_order)
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(())
}

async fn require_draft_curriculum_version(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
) -> Result<(), AppError> {
    let status: VersionStatus =
        sqlx::query_scalar("SELECT status FROM curriculum_versions WHERE id = $1 FOR SHARE")
            .bind(version_id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันหลักสูตร".to_string()))?;
    ensure_draft_version(status)
}

async fn clear_default_program(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
    except: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE study_programs SET is_default = false WHERE curriculum_version_id = $1 \
         AND ($2::uuid IS NULL OR id <> $2)",
    )
    .bind(version_id)
    .bind(except)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn validate_publishable(
    transaction: &mut Transaction<'_, Postgres>,
    version: &CurriculumVersion,
) -> Result<(), AppError> {
    let program_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM study_programs WHERE curriculum_version_id = $1 AND status <> 'archived'",
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if program_count == 0 {
        return Err(AppError::ValidationError(
            "ต้องมีแผนการเรียนอย่างน้อยหนึ่งรายการก่อนเผยแพร่".to_string(),
        ));
    }
    let default_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM study_programs WHERE curriculum_version_id = $1 AND is_default",
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if default_count != 1 {
        return Err(AppError::ValidationError(
            "ต้องมีแผนการเรียนเริ่มต้นหนึ่งรายการพอดี".to_string(),
        ));
    }
    let empty_programs: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*) FROM study_programs program
        WHERE program.curriculum_version_id = $1
          AND NOT EXISTS (SELECT 1 FROM curriculum_course_requirements course WHERE course.study_program_id = program.id)
          AND NOT EXISTS (SELECT 1 FROM curriculum_activity_requirements activity WHERE activity.study_program_id = program.id)
        "#,
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if empty_programs != 0 {
        return Err(AppError::ValidationError(
            "ทุกแผนการเรียนต้องมีข้อกำหนดอย่างน้อยหนึ่งรายการ".to_string(),
        ));
    }
    let unpublished_resources: i64 = sqlx::query_scalar(
        r#"
        SELECT
          (SELECT count(*) FROM curriculum_course_requirements requirement
           JOIN subject_versions subject ON subject.id = requirement.subject_version_id
           WHERE requirement.curriculum_version_id = $1 AND subject.status <> 'published')
        + (SELECT count(*) FROM curriculum_activity_requirements requirement
           JOIN activity_versions activity ON activity.id = requirement.activity_version_id
           WHERE requirement.curriculum_version_id = $1 AND activity.status <> 'published')
        "#,
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if unpublished_resources != 0 {
        return Err(AppError::ValidationError(
            "ข้อกำหนดอ้างอิงเวอร์ชันที่ยังไม่เผยแพร่".to_string(),
        ));
    }
    Ok(())
}

fn map_curriculum_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.code().as_deref() == Some("23505") {
            return AppError::Conflict("รหัสหรือข้อมูลประจำหลักสูตรซ้ำ".to_string());
        }
    }
    AppError::DbError(error)
}

fn map_program_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.code().as_deref() == Some("23505") {
            return AppError::Conflict("รหัสแผนการเรียนซ้ำในเวอร์ชันหลักสูตร".to_string());
        }
    }
    AppError::DbError(error)
}
