use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

use super::super::models::{
    ActivityVersion, CatalogActivity, CatalogSubject, CreateActivityVersionRequest,
    CreateCatalogActivityRequest, CreateCatalogSubjectRequest, CreateSubjectGroupRequest,
    CreateSubjectVersionRequest, DefaultTeacher, PublishVersionRequest,
    ReplaceDefaultTeachersRequest, SubjectGroup, SubjectVersion, UpdateActivityVersionRequest,
    UpdateCatalogActivityRequest, UpdateCatalogSubjectRequest, UpdateSubjectGroupRequest,
    UpdateSubjectVersionRequest,
};
use super::{ensure_draft_version, parse_row_version, validate_canonical_decimal};

const SUBJECT_COLUMNS: &str =
    "id, code, owning_organization_unit_id, archived_at, row_version, created_at, updated_at";
const ACTIVITY_COLUMNS: &str =
    "id, code, activity_type, owning_organization_unit_id, archived_at, row_version, created_at, updated_at";

pub fn owner_allowed(filter: &AcademicResourceListFilter, owner: Option<Uuid>) -> bool {
    filter.includes_school_owned
        || owner.is_some_and(|owner_id| filter.allowed_organization_unit_ids().contains(&owner_id))
}

pub async fn list_subjects(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<CatalogSubject>, AppError> {
    let owner_ids = filter.allowed_organization_unit_ids();
    let sql = format!(
        "SELECT {SUBJECT_COLUMNS} FROM subjects \
         WHERE $1 OR owning_organization_unit_id = ANY($2) ORDER BY code, id LIMIT 500"
    );
    Ok(sqlx::query_as(&sql)
        .bind(filter.includes_school_owned)
        .bind(owner_ids)
        .fetch_all(pool)
        .await?)
}

pub async fn get_subject(pool: &PgPool, id: Uuid) -> Result<CatalogSubject, AppError> {
    let sql = format!("SELECT {SUBJECT_COLUMNS} FROM subjects WHERE id = $1");
    sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชา".to_string()))
}

pub async fn create_subject(
    pool: &PgPool,
    request: CreateCatalogSubjectRequest,
) -> Result<CatalogSubject, AppError> {
    let code = normalize_code(&request.code)?;
    let id = Uuid::new_v4();
    let sql = format!(
        "INSERT INTO subjects (id, code, identity_key, owning_organization_unit_id) \
         VALUES ($1, $2, $3, $4) RETURNING {SUBJECT_COLUMNS}"
    );
    sqlx::query_as(&sql)
        .bind(id)
        .bind(&code)
        .bind(code.to_lowercase())
        .bind(request.owning_organization_unit_id)
        .fetch_one(pool)
        .await
        .map_err(map_catalog_write_error)
}

pub async fn update_subject(
    pool: &PgPool,
    id: Uuid,
    request: UpdateCatalogSubjectRequest,
) -> Result<CatalogSubject, AppError> {
    parse_row_version(request.row_version)?;
    let code = normalize_code(&request.code)?;
    let sql = format!(
        "UPDATE subjects SET code = $1, identity_key = $2, owning_organization_unit_id = $3, \
         archived_at = CASE WHEN $4 THEN COALESCE(archived_at, now()) ELSE NULL END, \
         row_version = row_version + 1, updated_at = now() WHERE id = $5 AND row_version = $6 \
         RETURNING {SUBJECT_COLUMNS}"
    );
    sqlx::query_as(&sql)
        .bind(&code)
        .bind(code.to_lowercase())
        .bind(request.owning_organization_unit_id)
        .bind(request.archived)
        .bind(id)
        .bind(request.row_version)
        .fetch_optional(pool)
        .await
        .map_err(map_catalog_write_error)?
        .ok_or_else(|| AppError::Conflict("รายวิชาถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()))
}

pub async fn list_subject_versions(
    pool: &PgPool,
    subject_id: Uuid,
) -> Result<Vec<SubjectVersion>, AppError> {
    ensure_subject_exists(pool, subject_id).await?;
    Ok(sqlx::query_as(&subject_version_select(
        "version.subject_id = $1",
        "version.version_no DESC",
    ))
    .bind(subject_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_subject_version(pool: &PgPool, id: Uuid) -> Result<SubjectVersion, AppError> {
    sqlx::query_as(&subject_version_select("version.id = $1", "version.id"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันรายวิชา".to_string()))
}

pub async fn create_subject_version(
    pool: &PgPool,
    subject_id: Uuid,
    request: CreateSubjectVersionRequest,
) -> Result<SubjectVersion, AppError> {
    validate_subject_version(&request)?;
    let credit = validate_canonical_decimal(&request.credit, 2)?;
    let mut transaction = pool.begin().await?;
    let (subject_code,): (String,) =
        sqlx::query_as("SELECT code FROM subjects WHERE id = $1 FOR UPDATE")
            .bind(subject_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชา".to_string()))?;
    let version_no: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(version_no), 0) + 1 FROM subject_versions WHERE subject_id = $1",
    )
    .bind(subject_id)
    .fetch_one(&mut *transaction)
    .await?;
    let start_year_id = resolve_start_year(&mut transaction, request.effective_from).await?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO subject_versions (
            id, subject_id, version_no, code, name_th, name_en, credit,
            hours_per_semester, type, group_id, description, effective_from,
            effective_until, start_academic_year_id, term, is_active,
            periods_per_week, status
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
            $14, $15, true, $16, 'draft'
        )
        "#,
    )
    .bind(id)
    .bind(subject_id)
    .bind(version_no)
    .bind(subject_code)
    .bind(request.name_th.trim())
    .bind(request.name_en)
    .bind(credit)
    .bind(request.hours_per_semester)
    .bind(request.subject_type.trim())
    .bind(request.group_id)
    .bind(request.description)
    .bind(request.effective_from)
    .bind(request.effective_until)
    .bind(start_year_id)
    .bind(request.term_code)
    .bind(request.periods_per_week)
    .execute(&mut *transaction)
    .await
    .map_err(map_version_write_error)?;
    replace_subject_grade_levels(&mut transaction, id, request.grade_level_ids).await?;
    transaction.commit().await?;
    get_subject_version(pool, id).await
}

pub async fn update_subject_version(
    pool: &PgPool,
    id: Uuid,
    request: UpdateSubjectVersionRequest,
) -> Result<SubjectVersion, AppError> {
    let row_version = request.row_version;
    let values = CreateSubjectVersionRequest {
        name_th: request.name_th,
        name_en: request.name_en,
        credit: request.credit,
        hours_per_semester: request.hours_per_semester,
        subject_type: request.subject_type,
        group_id: request.group_id,
        description: request.description,
        effective_from: request.effective_from,
        effective_until: request.effective_until,
        term_code: request.term_code,
        periods_per_week: request.periods_per_week,
        grade_level_ids: request.grade_level_ids,
    };
    parse_row_version(row_version)?;
    validate_subject_version(&values)?;
    let credit = validate_canonical_decimal(&values.credit, 2)?;
    let mut transaction = pool.begin().await?;
    let status: super::super::models::VersionStatus =
        sqlx::query_scalar("SELECT status FROM subject_versions WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันรายวิชา".to_string()))?;
    ensure_draft_version(status)?;
    let result = sqlx::query(
        r#"
        UPDATE subject_versions SET name_th = $1, name_en = $2, credit = $3,
            hours_per_semester = $4, type = $5, group_id = $6, description = $7,
            effective_from = $8, effective_until = $9, term = $10,
            periods_per_week = $11,
            row_version = row_version + 1, updated_at = now()
        WHERE id = $12 AND row_version = $13 AND status = 'draft'
        "#,
    )
    .bind(values.name_th.trim())
    .bind(values.name_en)
    .bind(credit)
    .bind(values.hours_per_semester)
    .bind(values.subject_type.trim())
    .bind(values.group_id)
    .bind(values.description)
    .bind(values.effective_from)
    .bind(values.effective_until)
    .bind(values.term_code)
    .bind(values.periods_per_week)
    .bind(id)
    .bind(row_version)
    .execute(&mut *transaction)
    .await
    .map_err(map_version_write_error)?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "เวอร์ชันรายวิชาถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    replace_subject_grade_levels(&mut transaction, id, values.grade_level_ids).await?;
    transaction.commit().await?;
    get_subject_version(pool, id).await
}

pub async fn publish_subject_version(
    pool: &PgPool,
    id: Uuid,
    request: PublishVersionRequest,
) -> Result<SubjectVersion, AppError> {
    publish_version(pool, "subject_versions", id, request.row_version).await?;
    get_subject_version(pool, id).await
}

pub async fn list_activities(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<CatalogActivity>, AppError> {
    let owner_ids = filter.allowed_organization_unit_ids();
    let sql = format!(
        "SELECT {ACTIVITY_COLUMNS} FROM activities \
         WHERE $1 OR owning_organization_unit_id = ANY($2) ORDER BY code, id LIMIT 500"
    );
    Ok(sqlx::query_as(&sql)
        .bind(filter.includes_school_owned)
        .bind(owner_ids)
        .fetch_all(pool)
        .await?)
}

pub async fn get_activity(pool: &PgPool, id: Uuid) -> Result<CatalogActivity, AppError> {
    let sql = format!("SELECT {ACTIVITY_COLUMNS} FROM activities WHERE id = $1");
    sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรม".to_string()))
}

pub async fn create_activity(
    pool: &PgPool,
    request: CreateCatalogActivityRequest,
) -> Result<CatalogActivity, AppError> {
    let code = normalize_code(&request.code)?;
    if request.activity_type.trim().is_empty() {
        return Err(AppError::ValidationError("ประเภทกิจกรรมห้ามว่าง".to_string()));
    }
    let id = Uuid::new_v4();
    let identity_key = format!(
        "{}:{}",
        request.activity_type.trim().to_lowercase(),
        code.to_lowercase()
    );
    let sql = format!(
        "INSERT INTO activities (id, code, identity_key, activity_type, owning_organization_unit_id) \
         VALUES ($1, $2, $3, $4, $5) RETURNING {ACTIVITY_COLUMNS}"
    );
    sqlx::query_as(&sql)
        .bind(id)
        .bind(code)
        .bind(identity_key)
        .bind(request.activity_type.trim().to_lowercase())
        .bind(request.owning_organization_unit_id)
        .fetch_one(pool)
        .await
        .map_err(map_catalog_write_error)
}

pub async fn update_activity(
    pool: &PgPool,
    id: Uuid,
    request: UpdateCatalogActivityRequest,
) -> Result<CatalogActivity, AppError> {
    parse_row_version(request.row_version)?;
    let code = normalize_code(&request.code)?;
    let identity_key = format!(
        "{}:{}",
        request.activity_type.trim().to_lowercase(),
        code.to_lowercase()
    );
    let sql = format!(
        "UPDATE activities SET code = $1, identity_key = $2, activity_type = $3, \
         owning_organization_unit_id = $4, \
         archived_at = CASE WHEN $5 THEN COALESCE(archived_at, now()) ELSE NULL END, \
         row_version = row_version + 1, updated_at = now() \
         WHERE id = $6 AND row_version = $7 RETURNING {ACTIVITY_COLUMNS}"
    );
    sqlx::query_as(&sql)
        .bind(code)
        .bind(identity_key)
        .bind(request.activity_type.trim().to_lowercase())
        .bind(request.owning_organization_unit_id)
        .bind(request.archived)
        .bind(id)
        .bind(request.row_version)
        .fetch_optional(pool)
        .await
        .map_err(map_catalog_write_error)?
        .ok_or_else(|| AppError::Conflict("กิจกรรมถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()))
}

pub async fn list_activity_versions(
    pool: &PgPool,
    activity_id: Uuid,
) -> Result<Vec<ActivityVersion>, AppError> {
    get_activity(pool, activity_id).await?;
    Ok(sqlx::query_as(&activity_version_select(
        "version.activity_id = $1",
        "version.version_no DESC",
    ))
    .bind(activity_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_activity_version(pool: &PgPool, id: Uuid) -> Result<ActivityVersion, AppError> {
    sqlx::query_as(&activity_version_select("version.id = $1", "version.id"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันกิจกรรม".to_string()))
}

pub async fn create_activity_version(
    pool: &PgPool,
    activity_id: Uuid,
    request: CreateActivityVersionRequest,
) -> Result<ActivityVersion, AppError> {
    validate_activity_version(&request)?;
    let hours = validate_canonical_decimal(&request.hours_per_week, 2)?;
    let grade_level_ids = sqlx::types::Json(request.grade_level_ids.clone());
    let mut transaction = pool.begin().await?;
    let version_no: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(version_no), 0) + 1 FROM activity_versions WHERE activity_id = $1",
    )
    .bind(activity_id)
    .fetch_one(&mut *transaction)
    .await?;
    let activity_type: String =
        sqlx::query_scalar("SELECT activity_type FROM activities WHERE id = $1 FOR UPDATE")
            .bind(activity_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรม".to_string()))?;
    let start_year_id = resolve_start_year(&mut transaction, request.effective_from).await?;
    let legacy_periods = request
        .hours_per_week
        .split('.')
        .next()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(1)
        .max(1);
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO activity_versions (
            id, activity_id, version_no, name, activity_type, description,
            periods_per_week, hours_per_week, scheduling_mode, is_active,
            term, grade_level_ids, start_academic_year_id, effective_from,
            effective_until, status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10, $11, $12, $13, $14, 'draft')
        "#,
    )
    .bind(id)
    .bind(activity_id)
    .bind(version_no)
    .bind(request.name.trim())
    .bind(activity_type)
    .bind(request.description)
    .bind(legacy_periods)
    .bind(hours)
    .bind(request.scheduling_mode.trim())
    .bind(request.term_code)
    .bind(grade_level_ids)
    .bind(start_year_id)
    .bind(request.effective_from)
    .bind(request.effective_until)
    .execute(&mut *transaction)
    .await
    .map_err(map_version_write_error)?;
    transaction.commit().await?;
    get_activity_version(pool, id).await
}

pub async fn update_activity_version(
    pool: &PgPool,
    id: Uuid,
    request: UpdateActivityVersionRequest,
) -> Result<ActivityVersion, AppError> {
    let row_version = request.row_version;
    let values = CreateActivityVersionRequest {
        name: request.name,
        description: request.description,
        hours_per_week: request.hours_per_week,
        scheduling_mode: request.scheduling_mode,
        effective_from: request.effective_from,
        effective_until: request.effective_until,
        term_code: request.term_code,
        grade_level_ids: request.grade_level_ids,
    };
    parse_row_version(row_version)?;
    validate_activity_version(&values)?;
    let hours = validate_canonical_decimal(&values.hours_per_week, 2)?;
    let grade_level_ids = sqlx::types::Json(values.grade_level_ids.clone());
    let result = sqlx::query(
        r#"
        UPDATE activity_versions SET name = $1, description = $2, hours_per_week = $3,
            scheduling_mode = $4, effective_from = $5, effective_until = $6,
            term = $7, grade_level_ids = $8, row_version = row_version + 1,
            updated_at = now()
        WHERE id = $9 AND row_version = $10 AND status = 'draft'
        "#,
    )
    .bind(values.name.trim())
    .bind(values.description)
    .bind(hours)
    .bind(values.scheduling_mode.trim())
    .bind(values.effective_from)
    .bind(values.effective_until)
    .bind(values.term_code)
    .bind(grade_level_ids)
    .bind(id)
    .bind(row_version)
    .execute(pool)
    .await
    .map_err(map_version_write_error)?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "เวอร์ชันกิจกรรมเผยแพร่แล้วหรือถูกแก้ไขโดยผู้ใช้อื่น".to_string(),
        ));
    }
    get_activity_version(pool, id).await
}

pub async fn publish_activity_version(
    pool: &PgPool,
    id: Uuid,
    request: PublishVersionRequest,
) -> Result<ActivityVersion, AppError> {
    publish_version(pool, "activity_versions", id, request.row_version).await?;
    get_activity_version(pool, id).await
}

pub async fn list_subject_default_teachers(
    pool: &PgPool,
    subject_id: Uuid,
) -> Result<Vec<DefaultTeacher>, AppError> {
    Ok(sqlx::query_as(
        "SELECT instructor_id AS user_id, role FROM subject_default_instructors \
         WHERE subject_id = $1 ORDER BY role, instructor_id",
    )
    .bind(subject_id)
    .fetch_all(pool)
    .await?)
}

pub async fn replace_subject_default_teachers(
    pool: &PgPool,
    subject_id: Uuid,
    request: ReplaceDefaultTeachersRequest,
) -> Result<Vec<DefaultTeacher>, AppError> {
    replace_default_teachers(
        pool,
        "subjects",
        "subject_default_instructors",
        "subject_id",
        subject_id,
        request,
    )
    .await?;
    list_subject_default_teachers(pool, subject_id).await
}

pub async fn list_activity_default_teachers(
    pool: &PgPool,
    activity_id: Uuid,
) -> Result<Vec<DefaultTeacher>, AppError> {
    Ok(sqlx::query_as(
        "SELECT instructor_id AS user_id, role FROM activity_default_instructors \
         WHERE activity_id = $1 ORDER BY role, instructor_id",
    )
    .bind(activity_id)
    .fetch_all(pool)
    .await?)
}

pub async fn replace_activity_default_teachers(
    pool: &PgPool,
    activity_id: Uuid,
    request: ReplaceDefaultTeachersRequest,
) -> Result<Vec<DefaultTeacher>, AppError> {
    replace_default_teachers(
        pool,
        "activities",
        "activity_default_instructors",
        "activity_id",
        activity_id,
        request,
    )
    .await?;
    list_activity_default_teachers(pool, activity_id).await
}

pub async fn list_subject_groups(pool: &PgPool) -> Result<Vec<SubjectGroup>, AppError> {
    Ok(sqlx::query_as(
        "SELECT id, code, name_th, name_en, display_order, is_active, row_version, created_at, updated_at \
         FROM subject_groups ORDER BY display_order, code, id",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn create_subject_group(
    pool: &PgPool,
    request: CreateSubjectGroupRequest,
) -> Result<SubjectGroup, AppError> {
    validate_subject_group(&request.code, &request.name_th, &request.name_en)?;
    sqlx::query_as(
        r#"
        INSERT INTO subject_groups (id, code, name_th, name_en, display_order, is_active)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, code, name_th, name_en, display_order, is_active, row_version, created_at, updated_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(request.code.trim().to_uppercase())
    .bind(request.name_th.trim())
    .bind(request.name_en.trim())
    .bind(request.display_order)
    .bind(request.is_active)
    .fetch_one(pool)
    .await
    .map_err(map_catalog_write_error)
}

pub async fn update_subject_group(
    pool: &PgPool,
    id: Uuid,
    request: UpdateSubjectGroupRequest,
) -> Result<SubjectGroup, AppError> {
    parse_row_version(request.row_version)?;
    validate_subject_group(&request.code, &request.name_th, &request.name_en)?;
    sqlx::query_as(
        r#"
        UPDATE subject_groups SET code = $1, name_th = $2, name_en = $3,
            display_order = $4, is_active = $5, row_version = row_version + 1, updated_at = now()
        WHERE id = $6 AND row_version = $7
        RETURNING id, code, name_th, name_en, display_order, is_active, row_version, created_at, updated_at
        "#,
    )
    .bind(request.code.trim().to_uppercase())
    .bind(request.name_th.trim())
    .bind(request.name_en.trim())
    .bind(request.display_order)
    .bind(request.is_active)
    .bind(id)
    .bind(request.row_version)
    .fetch_optional(pool)
    .await
    .map_err(map_catalog_write_error)?
    .ok_or_else(|| AppError::Conflict("กลุ่มสาระถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()))
}

pub async fn delete_subject_group(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let used: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM subject_versions WHERE group_id = $1)")
            .bind(id)
            .fetch_one(pool)
            .await?;
    if used {
        return Err(AppError::Conflict("กลุ่มสาระถูกใช้งานอยู่".to_string()));
    }
    let result = sqlx::query("DELETE FROM subject_groups WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบกลุ่มสาระ".to_string()));
    }
    Ok(())
}

fn subject_version_select(predicate: &str, order: &str) -> String {
    format!(
        r#"
        SELECT version.id, version.subject_id, version.version_no, version.name_th,
               version.name_en, version.credit::text AS credit,
               version.hours_per_semester, version.type AS subject_type, version.group_id,
               version.description, version.effective_from, version.effective_until,
               version.term AS term_code, version.periods_per_week,
               ARRAY(SELECT link.grade_level_id FROM subject_version_grade_levels link
                     WHERE link.subject_id = version.id ORDER BY link.grade_level_id) AS grade_level_ids,
               version.status, version.published_at, version.row_version,
               version.migration_provenance <> '{{}}'::jsonb AS migrated,
               version.created_at, version.updated_at
        FROM subject_versions version
        WHERE {predicate}
        ORDER BY {order}
        "#
    )
}

fn activity_version_select(predicate: &str, order: &str) -> String {
    format!(
        r#"
        SELECT version.id, version.activity_id, version.version_no, version.name,
               version.description, version.hours_per_week::text AS hours_per_week,
               version.scheduling_mode, version.effective_from, version.effective_until,
               version.term AS term_code,
               COALESCE(ARRAY(SELECT jsonb_array_elements_text(COALESCE(version.grade_level_ids, '[]'::jsonb))::uuid), ARRAY[]::uuid[]) AS grade_level_ids,
               version.status, version.published_at, version.row_version,
               version.migration_provenance <> '{{}}'::jsonb AS migrated,
               version.created_at, version.updated_at
        FROM activity_versions version
        WHERE {predicate}
        ORDER BY {order}
        "#
    )
}

fn validate_subject_version(request: &CreateSubjectVersionRequest) -> Result<(), AppError> {
    validate_effective_range(request.effective_from, request.effective_until)?;
    validate_canonical_decimal(&request.credit, 2)?;
    if request.name_th.trim().is_empty() || request.subject_type.trim().is_empty() {
        return Err(AppError::ValidationError(
            "ชื่อและประเภทรายวิชาห้ามว่าง".to_string(),
        ));
    }
    Ok(())
}

fn validate_activity_version(request: &CreateActivityVersionRequest) -> Result<(), AppError> {
    validate_effective_range(request.effective_from, request.effective_until)?;
    validate_canonical_decimal(&request.hours_per_week, 2)?;
    if request.name.trim().is_empty()
        || !matches!(
            request.scheduling_mode.as_str(),
            "synchronized" | "independent"
        )
    {
        return Err(AppError::ValidationError(
            "ข้อมูลเวอร์ชันกิจกรรมไม่ถูกต้อง".to_string(),
        ));
    }
    Ok(())
}

fn validate_effective_range(
    from: chrono::NaiveDate,
    until: Option<chrono::NaiveDate>,
) -> Result<(), AppError> {
    if until.is_some_and(|until| until <= from) {
        return Err(AppError::ValidationError("ช่วงวันที่มีผลไม่ถูกต้อง".to_string()));
    }
    Ok(())
}

fn normalize_code(code: &str) -> Result<String, AppError> {
    let code = code.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::ValidationError("รหัสห้ามว่าง".to_string()));
    }
    Ok(code)
}

fn validate_subject_group(code: &str, name_th: &str, name_en: &str) -> Result<(), AppError> {
    if code.trim().is_empty() || name_th.trim().is_empty() || name_en.trim().is_empty() {
        return Err(AppError::ValidationError("ข้อมูลกลุ่มสาระไม่ครบถ้วน".to_string()));
    }
    Ok(())
}

async fn ensure_subject_exists(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM subjects WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("ไม่พบรายวิชา".to_string()))
    }
}

async fn resolve_start_year(
    transaction: &mut Transaction<'_, Postgres>,
    effective_from: chrono::NaiveDate,
) -> Result<Uuid, AppError> {
    sqlx::query_scalar(
        "SELECT id FROM academic_years WHERE start_date <= $1 AND end_date >= $1 \
         ORDER BY start_date DESC LIMIT 1",
    )
    .bind(effective_from)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::ValidationError("วันที่เริ่มใช้ไม่อยู่ในปีการศึกษาใด".to_string()))
}

async fn replace_subject_grade_levels(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
    grade_level_ids: Vec<Uuid>,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM subject_version_grade_levels WHERE subject_id = $1")
        .bind(version_id)
        .execute(&mut **transaction)
        .await?;
    for grade_level_id in grade_level_ids {
        sqlx::query(
            "INSERT INTO subject_version_grade_levels (subject_id, grade_level_id) VALUES ($1, $2)",
        )
        .bind(version_id)
        .bind(grade_level_id)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn publish_version(
    pool: &PgPool,
    table: &str,
    id: Uuid,
    row_version: i64,
) -> Result<(), AppError> {
    parse_row_version(row_version)?;
    let sql = format!(
        "UPDATE {table} SET status = 'published', published_at = now(), \
         row_version = row_version + 1, updated_at = now() \
         WHERE id = $1 AND row_version = $2 AND status = 'draft'"
    );
    let result = sqlx::query(&sql)
        .bind(id)
        .bind(row_version)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "เวอร์ชันเผยแพร่แล้วหรือถูกแก้ไขโดยผู้ใช้อื่น".to_string(),
        ));
    }
    Ok(())
}

async fn replace_default_teachers(
    pool: &PgPool,
    stable_table: &str,
    link_table: &str,
    foreign_key: &str,
    resource_id: Uuid,
    request: ReplaceDefaultTeachersRequest,
) -> Result<(), AppError> {
    parse_row_version(request.row_version)?;
    let mut unique = std::collections::HashSet::new();
    if request
        .teachers
        .iter()
        .any(|teacher| !unique.insert(teacher.user_id))
    {
        return Err(AppError::ValidationError("รายชื่อครูเริ่มต้นซ้ำกัน".to_string()));
    }
    let mut transaction = pool.begin().await?;
    let lock_sql = format!("SELECT row_version FROM {stable_table} WHERE id = $1 FOR UPDATE");
    let actual: i64 = sqlx::query_scalar(&lock_sql)
        .bind(resource_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบทรัพยากรคลัง".to_string()))?;
    if actual != request.row_version {
        return Err(AppError::Conflict("ข้อมูลถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    let delete_sql = format!("DELETE FROM {link_table} WHERE {foreign_key} = $1");
    sqlx::query(&delete_sql)
        .bind(resource_id)
        .execute(&mut *transaction)
        .await?;
    let insert_sql = format!(
        "INSERT INTO {link_table} (id, {foreign_key}, instructor_id, role) VALUES ($1, $2, $3, $4)"
    );
    for teacher in request.teachers {
        sqlx::query(&insert_sql)
            .bind(Uuid::new_v4())
            .bind(resource_id)
            .bind(teacher.user_id)
            .bind(teacher.role)
            .execute(&mut *transaction)
            .await?;
    }
    let update_sql = format!(
        "UPDATE {stable_table} SET row_version = row_version + 1, updated_at = now() WHERE id = $1"
    );
    sqlx::query(&update_sql)
        .bind(resource_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

fn map_catalog_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.code().as_deref() == Some("23505") {
            return AppError::Conflict("รหัสหรือข้อมูลประจำรายการซ้ำ".to_string());
        }
    }
    AppError::DbError(error)
}

fn map_version_write_error(error: sqlx::Error) -> AppError {
    if error.to_string().contains("VERSION_RANGE_OVERLAP") {
        AppError::Conflict("ช่วงวันที่ของเวอร์ชันทับซ้อนกับเวอร์ชันเดิม".to_string())
    } else {
        AppError::DbError(error)
    }
}
