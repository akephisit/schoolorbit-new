use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

use super::super::models::{
    CloneCurriculumVersionRequest, CreateCurriculumRequest, CreateCurriculumVersionRequest,
    CreateStudyProgramRequest, Curriculum, CurriculumVersion, PublishVersionRequest, StudyProgram,
    StudyProgramOption, UpdateCurriculumRequest, UpdateCurriculumVersionRequest,
    UpdateStudyProgramRequest, VersionStatus,
};
use super::{ensure_draft_version, parse_row_version};

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
const MAX_CLONED_CURRICULUM_CHILDREN: i64 = 50_000;

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
    let mut transaction = pool.begin().await?;
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
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        WITH ordered_terms AS (
            SELECT sequence_no,
                   term_type,
                   name,
                   row_number() OVER (
                       PARTITION BY term_type
                       ORDER BY sequence_no, id
                   )::integer AS type_occurrence
            FROM academic_terms
            WHERE academic_year_id = $2
        )
        INSERT INTO curriculum_term_slots (
            id, curriculum_version_id, sequence, term_type, type_occurrence, name
        )
        SELECT gen_random_uuid(), $1, sequence_no, term_type, type_occurrence, name
        FROM ordered_terms
        ORDER BY sequence_no
        "#,
    )
    .bind(id)
    .bind(request.start_academic_year_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    get_version(pool, id).await
}

pub async fn clone_version_draft(
    pool: &PgPool,
    source_version_id: Uuid,
    request: CloneCurriculumVersionRequest,
) -> Result<CurriculumVersion, AppError> {
    parse_row_version(request.source_row_version)?;
    let version_fields = CreateCurriculumVersionRequest {
        version_name: request.version_name,
        start_academic_year_id: request.start_academic_year_id,
        end_academic_year_id: request.end_academic_year_id,
        description: request.description,
    };
    validate_version_fields(pool, &version_fields).await?;

    let mut transaction = pool.begin().await?;
    let source: CurriculumVersion = {
        let sql =
            format!("SELECT {VERSION_COLUMNS} FROM curriculum_versions WHERE id = $1 FOR UPDATE");
        sqlx::query_as(&sql)
            .bind(source_version_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นหลักสูตรต้นทาง".to_string()))?
    };
    if source.status != VersionStatus::Published {
        return Err(AppError::Conflict(
            "สร้างหลักสูตรรุ่นใหม่ได้จากรุ่นที่เผยแพร่แล้วเท่านั้น".to_string(),
        ));
    }
    if source.row_version != request.source_row_version {
        return Err(AppError::Conflict(format!(
            "รุ่นหลักสูตรต้นทางถูกแก้ไขแล้ว (expected {}, actual {})",
            request.source_row_version, source.row_version
        )));
    }
    let (source_start, requested_start): (chrono::NaiveDate, chrono::NaiveDate) = sqlx::query_as(
        r#"SELECT source_year.start_date, requested_year.start_date
               FROM academic_years source_year
               JOIN academic_years requested_year ON requested_year.id = $2
               WHERE source_year.id = $1"#,
    )
    .bind(source.start_academic_year_id)
    .bind(version_fields.start_academic_year_id)
    .fetch_one(&mut *transaction)
    .await?;
    if requested_start <= source_start {
        return Err(AppError::ValidationError(
            "ปีเริ่มใช้ของรุ่นใหม่ต้องอยู่หลังปีเริ่มใช้ของหลักสูตรต้นทาง".to_string(),
        ));
    }

    let child_count: i64 = sqlx::query_scalar(
        r#"SELECT
               (SELECT count(*) FROM curriculum_term_slots WHERE curriculum_version_id = $1)
             + (SELECT count(*) FROM study_programs
                WHERE curriculum_version_id = $1 AND status <> 'archived')
             + (SELECT count(*) FROM curriculum_course_requirements
                WHERE curriculum_version_id = $1)
             + (SELECT count(*) FROM curriculum_activity_requirements
                WHERE curriculum_version_id = $1)"#,
    )
    .bind(source.id)
    .fetch_one(&mut *transaction)
    .await?;
    if child_count > MAX_CLONED_CURRICULUM_CHILDREN {
        return Err(AppError::ValidationError(format!(
            "โครงสร้างหลักสูตรมีมากกว่า {MAX_CLONED_CURRICULUM_CHILDREN} รายการ กรุณาติดต่อผู้ดูแลระบบ"
        )));
    }

    let cloned_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO curriculum_versions (
               id, curriculum_id, version_name, start_academic_year_id,
               end_academic_year_id, description, is_active, status
           ) VALUES ($1, $2, $3, $4, $5, $6, true, 'draft')"#,
    )
    .bind(cloned_id)
    .bind(source.curriculum_id)
    .bind(version_fields.version_name.trim())
    .bind(version_fields.start_academic_year_id)
    .bind(version_fields.end_academic_year_id)
    .bind(version_fields.description)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        r#"INSERT INTO curriculum_term_slots (
               id, curriculum_version_id, sequence, term_type, type_occurrence, name
           )
           SELECT uuid_generate_v5($1, 'term-slot:' || source.id::text),
                  $1, source.sequence, source.term_type, source.type_occurrence, source.name
           FROM curriculum_term_slots source
           WHERE source.curriculum_version_id = $2
           ORDER BY source.sequence, source.id"#,
    )
    .bind(cloned_id)
    .bind(source.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"INSERT INTO study_programs (
               id, curriculum_version_id, code, name_th, name_en, is_default,
               status, owning_organization_unit_id
           )
           SELECT uuid_generate_v5($1, 'study-program:' || source.id::text),
                  $1, source.code, source.name_th, source.name_en, source.is_default,
                  'draft', source.owning_organization_unit_id
           FROM study_programs source
           WHERE source.curriculum_version_id = $2
             AND source.status <> 'archived'
           ORDER BY source.is_default DESC, source.code, source.id"#,
    )
    .bind(cloned_id)
    .bind(source.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"INSERT INTO curriculum_course_requirements (
               id, curriculum_version_id, study_program_id, subject_version_id,
               grade_level_id, term_slot_id, requirement_kind, display_order
           )
           SELECT uuid_generate_v5($1, 'course-requirement:' || requirement.id::text),
                  $1,
                  uuid_generate_v5($1, 'study-program:' || requirement.study_program_id::text),
                  requirement.subject_version_id,
                  requirement.grade_level_id,
                  uuid_generate_v5($1, 'term-slot:' || requirement.term_slot_id::text),
                  requirement.requirement_kind,
                  requirement.display_order
           FROM curriculum_course_requirements requirement
           JOIN study_programs source_program ON source_program.id = requirement.study_program_id
           WHERE requirement.curriculum_version_id = $2
             AND source_program.status <> 'archived'
           ORDER BY requirement.study_program_id, requirement.display_order, requirement.id"#,
    )
    .bind(cloned_id)
    .bind(source.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"INSERT INTO curriculum_activity_requirements (
               id, curriculum_version_id, study_program_id, activity_version_id,
               grade_level_id, term_slot_id, requirement_kind, display_order
           )
           SELECT uuid_generate_v5($1, 'activity-requirement:' || requirement.id::text),
                  $1,
                  uuid_generate_v5($1, 'study-program:' || requirement.study_program_id::text),
                  requirement.activity_version_id,
                  requirement.grade_level_id,
                  uuid_generate_v5($1, 'term-slot:' || requirement.term_slot_id::text),
                  requirement.requirement_kind,
                  requirement.display_order
           FROM curriculum_activity_requirements requirement
           JOIN study_programs source_program ON source_program.id = requirement.study_program_id
           WHERE requirement.curriculum_version_id = $2
             AND source_program.status <> 'archived'
           ORDER BY requirement.study_program_id, requirement.display_order, requirement.id"#,
    )
    .bind(cloned_id)
    .bind(source.id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    get_version(pool, cloned_id).await
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
    let term_slot_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM curriculum_term_slots WHERE curriculum_version_id = $1",
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if term_slot_count == 0 {
        return Err(AppError::ValidationError(
            "ต้องกำหนดภาคเรียนในโครงสร้างหลักสูตรก่อนเผยแพร่".to_string(),
        ));
    }
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
    let incomplete_course_metrics: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM curriculum_course_requirements requirement
           JOIN subject_versions version ON version.id = requirement.subject_version_id
           WHERE requirement.curriculum_version_id = $1
             AND (version.periods_per_week IS NULL
                  OR version.periods_per_week <= 0
                  OR version.credit IS NULL
                  OR version.credit <= 0
                  OR version.hours_per_semester IS NULL
                  OR version.hours_per_semester <= 0)"#,
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if incomplete_course_metrics != 0 {
        return Err(AppError::ValidationError(
            "รายวิชาในโครงสร้างต้องมีหน่วยกิต จำนวนคาบ และชั่วโมงรวมจากบัญชีรายวิชาให้ครบและมากกว่า 0"
                .to_string(),
        ));
    }
    let incomplete_activity_metrics: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM curriculum_activity_requirements requirement
           JOIN activity_versions version ON version.id = requirement.activity_version_id
           WHERE requirement.curriculum_version_id = $1
             AND (version.hours_per_week IS NULL
                  OR version.hours_per_week <= 0
                  OR version.hours_per_term IS NULL
                  OR version.hours_per_term <= 0)"#,
    )
    .bind(version.id)
    .fetch_one(&mut **transaction)
    .await?;
    if incomplete_activity_metrics != 0 {
        return Err(AppError::ValidationError(
            "กิจกรรมในโครงสร้างต้องมีชั่วโมงต่อสัปดาห์และชั่วโมงรวมต่อภาคเรียนให้ครบและมากกว่า 0".to_string(),
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
