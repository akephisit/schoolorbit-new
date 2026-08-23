use chrono::Duration;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

use super::super::models::{
    CreateHomeroomPlacementRequest, CreateHomeroomRequest, CreateStudentAcademicYearRequest,
    Homeroom, HomeroomAdvisor, HomeroomPlacement, HomeroomPlacementStatus,
    HomeroomPlacementTransfer, ReplaceHomeroomAdvisorsRequest, StudentAcademicYear,
    StudentAcademicYearFilter, StudentAcademicYearStatus, TransferHomeroomPlacementRequest,
    UpdateHomeroomRequest, UpdateStudentAcademicYearRequest,
};
use super::parse_row_version;
use super::years_terms::append_audit;

const HOMEROOM_COLUMNS: &str = r#"
    id, code, name, academic_year_id, grade_level_id, room_number,
    study_program_id, capacity, is_active, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;
const STUDENT_YEAR_COLUMNS: &str = r#"
    id, student_id, academic_year_id, grade_level_id, study_program_id,
    status, row_version, migration_provenance <> '{}'::jsonb AS migrated,
    created_at, updated_at
"#;
const PLACEMENT_COLUMNS: &str = r#"
    id, student_academic_year_id, academic_year_id, homeroom_id, start_date,
    end_date, status, enrollment_type, class_number, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;

pub async fn list_homerooms(
    pool: &PgPool,
    academic_year_id: Uuid,
) -> Result<Vec<Homeroom>, AppError> {
    let sql = format!(
        "SELECT {HOMEROOM_COLUMNS} FROM homerooms WHERE academic_year_id = $1 \
         ORDER BY grade_level_id, room_number NULLS LAST, code, id LIMIT 500"
    );
    Ok(sqlx::query_as(&sql)
        .bind(academic_year_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get_homeroom(pool: &PgPool, id: Uuid) -> Result<Homeroom, AppError> {
    let sql = format!("SELECT {HOMEROOM_COLUMNS} FROM homerooms WHERE id = $1");
    sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบห้องเรียนประจำชั้น".to_string()))
}

pub async fn create_homeroom(
    pool: &PgPool,
    request: CreateHomeroomRequest,
) -> Result<Homeroom, AppError> {
    validate_homeroom_fields(&request.code, &request.name, request.capacity)?;
    let mut transaction = pool.begin().await?;
    let curriculum_version_id = validate_homeroom_context(
        &mut transaction,
        request.academic_year_id,
        request.grade_level_id,
        request.study_program_id,
    )
    .await?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO homerooms (
            id, code, name, academic_year_id, grade_level_id, room_number,
            is_active, legacy_curriculum_version_id, study_program_id, capacity
        ) VALUES ($1, $2, $3, $4, $5, $6, true, $7, $8, $9)
        "#,
    )
    .bind(id)
    .bind(request.code.trim().to_uppercase())
    .bind(request.name.trim())
    .bind(request.academic_year_id)
    .bind(request.grade_level_id)
    .bind(request.room_number)
    .bind(curriculum_version_id)
    .bind(request.study_program_id)
    .bind(request.capacity)
    .execute(&mut *transaction)
    .await
    .map_err(map_homeroom_write_error)?;
    transaction.commit().await?;
    get_homeroom(pool, id).await
}

pub async fn update_homeroom(
    pool: &PgPool,
    id: Uuid,
    request: UpdateHomeroomRequest,
) -> Result<Homeroom, AppError> {
    parse_row_version(request.row_version)?;
    validate_homeroom_fields(&request.code, &request.name, request.capacity)?;
    let mut transaction = pool.begin().await?;
    let academic_year_id: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM homerooms WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบห้องเรียนประจำชั้น".to_string()))?;
    let curriculum_version_id = validate_homeroom_context(
        &mut transaction,
        academic_year_id,
        request.grade_level_id,
        request.study_program_id,
    )
    .await?;
    let result = sqlx::query(
        r#"
        UPDATE homerooms SET code = $1, name = $2, grade_level_id = $3,
            room_number = $4, legacy_curriculum_version_id = $5, study_program_id = $6,
            capacity = $7, row_version = row_version + 1, updated_at = now()
        WHERE id = $8 AND row_version = $9
        "#,
    )
    .bind(request.code.trim().to_uppercase())
    .bind(request.name.trim())
    .bind(request.grade_level_id)
    .bind(request.room_number)
    .bind(curriculum_version_id)
    .bind(request.study_program_id)
    .bind(request.capacity)
    .bind(id)
    .bind(request.row_version)
    .execute(&mut *transaction)
    .await
    .map_err(map_homeroom_write_error)?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict("ห้องเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    transaction.commit().await?;
    get_homeroom(pool, id).await
}

pub async fn list_advisors(
    pool: &PgPool,
    homeroom_id: Uuid,
) -> Result<Vec<HomeroomAdvisor>, AppError> {
    get_homeroom(pool, homeroom_id).await?;
    Ok(sqlx::query_as(
        "SELECT id, user_id, role FROM homeroom_advisors WHERE homeroom_id = $1 \
         ORDER BY role, user_id",
    )
    .bind(homeroom_id)
    .fetch_all(pool)
    .await?)
}

pub async fn replace_advisors(
    pool: &PgPool,
    homeroom_id: Uuid,
    request: ReplaceHomeroomAdvisorsRequest,
) -> Result<Vec<HomeroomAdvisor>, AppError> {
    parse_row_version(request.row_version)?;
    let mut users = std::collections::HashSet::new();
    let primary_count = request
        .advisors
        .iter()
        .filter(|advisor| advisor.role == "primary")
        .count();
    if primary_count > 1
        || request.advisors.iter().any(|advisor| {
            !matches!(advisor.role.as_str(), "primary" | "secondary")
                || !users.insert(advisor.user_id)
        })
    {
        return Err(AppError::ValidationError(
            "รายชื่อหรือบทบาทครูที่ปรึกษาไม่ถูกต้อง".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    let actual: i64 =
        sqlx::query_scalar("SELECT row_version FROM homerooms WHERE id = $1 FOR UPDATE")
            .bind(homeroom_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบห้องเรียนประจำชั้น".to_string()))?;
    if actual != request.row_version {
        return Err(AppError::Conflict("ห้องเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    sqlx::query("DELETE FROM homeroom_advisors WHERE homeroom_id = $1")
        .bind(homeroom_id)
        .execute(&mut *transaction)
        .await?;
    for advisor in request.advisors {
        sqlx::query(
            "INSERT INTO homeroom_advisors (id, homeroom_id, user_id, role) VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(homeroom_id)
        .bind(advisor.user_id)
        .bind(advisor.role)
        .execute(&mut *transaction)
        .await?;
    }
    sqlx::query(
        "UPDATE homerooms SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(homeroom_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    list_advisors(pool, homeroom_id).await
}

pub async fn list_student_years(
    pool: &PgPool,
    filter: StudentAcademicYearFilter,
) -> Result<Vec<StudentAcademicYear>, AppError> {
    let sql = format!(
        r#"
        SELECT {STUDENT_YEAR_COLUMNS}
        FROM student_academic_years student_year
        WHERE academic_year_id = $1
          AND ($2::uuid IS NULL OR student_id = $2)
          AND ($3::uuid IS NULL OR grade_level_id = $3)
          AND ($4::uuid IS NULL OR study_program_id = $4)
          AND ($5::text IS NULL OR status = $5)
          AND ($6::uuid IS NULL OR EXISTS (
              SELECT 1 FROM homeroom_placements placement
              WHERE placement.student_academic_year_id = student_year.id
                AND placement.homeroom_id = $6
                AND placement.status IN ('planned', 'current')
          ))
        ORDER BY student_id, id
        LIMIT 1000
        "#
    );
    Ok(sqlx::query_as(&sql)
        .bind(filter.academic_year_id)
        .bind(filter.student_id)
        .bind(filter.grade_level_id)
        .bind(filter.study_program_id)
        .bind(filter.status)
        .bind(filter.homeroom_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get_student_year(pool: &PgPool, id: Uuid) -> Result<StudentAcademicYear, AppError> {
    let sql = format!("SELECT {STUDENT_YEAR_COLUMNS} FROM student_academic_years WHERE id = $1");
    sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบข้อมูลนักเรียนประจำปี".to_string()))
}

pub async fn create_student_year(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateStudentAcademicYearRequest,
) -> Result<StudentAcademicYear, AppError> {
    let mut transaction = pool.begin().await?;
    validate_student_year_context(
        &mut transaction,
        request.academic_year_id,
        request.grade_level_id,
        request.study_program_id,
    )
    .await?;
    let student_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM users WHERE id = $1 AND user_type = 'student')",
    )
    .bind(request.student_id)
    .fetch_one(&mut *transaction)
    .await?;
    if !student_exists {
        return Err(AppError::ValidationError("ไม่พบนักเรียน".to_string()));
    }
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO student_academic_years (
            id, student_id, academic_year_id, grade_level_id, study_program_id, status
        ) VALUES ($1, $2, $3, $4, $5, 'planned')
        "#,
    )
    .bind(id)
    .bind(request.student_id)
    .bind(request.academic_year_id)
    .bind(request.grade_level_id)
    .bind(request.study_program_id)
    .execute(&mut *transaction)
    .await
    .map_err(map_student_year_write_error)?;
    append_audit(
        &mut transaction,
        "student_academic_year.created",
        "student_academic_year",
        id,
        Some(request.academic_year_id),
        None,
        actor_user_id,
        serde_json::json!({"status": "planned"}),
    )
    .await?;
    transaction.commit().await?;
    get_student_year(pool, id).await
}

pub async fn update_student_year(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: UpdateStudentAcademicYearRequest,
) -> Result<StudentAcademicYear, AppError> {
    parse_row_version(request.row_version)?;
    let mut transaction = pool.begin().await?;
    let (academic_year_id, status): (Uuid, StudentAcademicYearStatus) = sqlx::query_as(
        "SELECT academic_year_id, status FROM student_academic_years WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบข้อมูลนักเรียนประจำปี".to_string()))?;
    if status != StudentAcademicYearStatus::Planned {
        return Err(AppError::Conflict(
            "แก้ไขได้เฉพาะข้อมูลนักเรียนสถานะ planned".to_string(),
        ));
    }
    validate_student_year_context(
        &mut transaction,
        academic_year_id,
        request.grade_level_id,
        request.study_program_id,
    )
    .await?;
    let result = sqlx::query(
        "UPDATE student_academic_years SET grade_level_id = $1, study_program_id = $2, \
         row_version = row_version + 1, updated_at = now() \
         WHERE id = $3 AND row_version = $4 AND status = 'planned'",
    )
    .bind(request.grade_level_id)
    .bind(request.study_program_id)
    .bind(id)
    .bind(request.row_version)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "ข้อมูลนักเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    append_audit(
        &mut transaction,
        "student_academic_year.updated",
        "student_academic_year",
        id,
        Some(academic_year_id),
        None,
        actor_user_id,
        serde_json::json!({"rowVersion": request.row_version + 1}),
    )
    .await?;
    transaction.commit().await?;
    get_student_year(pool, id).await
}

pub async fn create_placement(
    pool: &PgPool,
    actor_user_id: Uuid,
    student_year_id: Uuid,
    request: CreateHomeroomPlacementRequest,
) -> Result<HomeroomPlacement, AppError> {
    parse_row_version(request.row_version)?;
    if request.enrollment_type.trim().is_empty() {
        return Err(AppError::ValidationError(
            "ประเภทการเข้าเรียนห้ามว่าง".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    let (academic_year_id, grade_level_id, study_program_id, actual): (Uuid, Uuid, Uuid, i64) =
        sqlx::query_as(
            "SELECT academic_year_id, grade_level_id, study_program_id, row_version \
         FROM student_academic_years WHERE id = $1 FOR UPDATE",
        )
        .bind(student_year_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบข้อมูลนักเรียนประจำปี".to_string()))?;
    if actual != request.row_version {
        return Err(AppError::Conflict(
            "ข้อมูลนักเรียนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    validate_target_homeroom(
        &mut transaction,
        request.homeroom_id,
        academic_year_id,
        grade_level_id,
        study_program_id,
    )
    .await?;
    validate_placement_date(&mut transaction, academic_year_id, request.start_date).await?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO homeroom_placements (
            id, student_academic_year_id, academic_year_id, homeroom_id,
            start_date, status, enrollment_type, class_number
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(id)
    .bind(student_year_id)
    .bind(academic_year_id)
    .bind(request.homeroom_id)
    .bind(request.start_date)
    .bind(request.status)
    .bind(request.enrollment_type.trim())
    .bind(request.class_number)
    .execute(&mut *transaction)
    .await
    .map_err(map_placement_write_error)?;
    sqlx::query("UPDATE student_academic_years SET row_version = row_version + 1, updated_at = now() WHERE id = $1")
        .bind(student_year_id)
        .execute(&mut *transaction)
        .await?;
    append_audit(
        &mut transaction,
        "homeroom_placement.created",
        "homeroom_placement",
        id,
        Some(academic_year_id),
        None,
        actor_user_id,
        serde_json::json!({"status": request.status}),
    )
    .await?;
    transaction.commit().await?;
    get_placement(pool, id).await
}

pub async fn transfer_placement(
    pool: &PgPool,
    actor_user_id: Uuid,
    placement_id: Uuid,
    request: TransferHomeroomPlacementRequest,
) -> Result<HomeroomPlacementTransfer, AppError> {
    parse_row_version(request.row_version)?;
    if request.enrollment_type.trim().is_empty() {
        return Err(AppError::ValidationError(
            "ประเภทการเข้าเรียนห้ามว่าง".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    let digest: String = sqlx::query_scalar("SELECT encode(sha256(convert_to($1, 'UTF8')), 'hex')")
        .bind(request.idempotency_key.to_string())
        .fetch_one(&mut *transaction)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(&digest)
        .execute(&mut *transaction)
        .await?;
    if let Some(replayed) = replay_transfer(&mut transaction, &digest).await? {
        transaction.commit().await?;
        return Ok(replayed);
    }
    let old = get_placement_for_update(&mut transaction, placement_id).await?;
    if old.row_version != request.row_version || old.status != HomeroomPlacementStatus::Current {
        return Err(AppError::Conflict(
            "รายการจัดห้องไม่ใช่รายการปัจจุบันหรือถูกแก้ไขแล้ว".to_string(),
        ));
    }
    if request.transfer_date <= old.start_date {
        return Err(AppError::ValidationError(
            "วันย้ายห้องต้องอยู่หลังวันเริ่มรายการเดิม".to_string(),
        ));
    }
    let (grade_level_id, study_program_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT grade_level_id, study_program_id FROM student_academic_years \
         WHERE id = $1 FOR UPDATE",
    )
    .bind(old.student_academic_year_id)
    .fetch_one(&mut *transaction)
    .await?;
    validate_target_homeroom(
        &mut transaction,
        request.target_homeroom_id,
        old.academic_year_id,
        grade_level_id,
        study_program_id,
    )
    .await?;
    validate_placement_date(
        &mut transaction,
        old.academic_year_id,
        request.transfer_date,
    )
    .await?;
    sqlx::query(
        "UPDATE homeroom_placements SET end_date = $1, status = 'ended', \
         row_version = row_version + 1, updated_at = now() WHERE id = $2",
    )
    .bind(request.transfer_date - Duration::days(1))
    .bind(old.id)
    .execute(&mut *transaction)
    .await?;
    let new_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO homeroom_placements (
            id, student_academic_year_id, academic_year_id, homeroom_id,
            start_date, status, enrollment_type, class_number
        ) VALUES ($1, $2, $3, $4, $5, 'current', $6, $7)
        "#,
    )
    .bind(new_id)
    .bind(old.student_academic_year_id)
    .bind(old.academic_year_id)
    .bind(request.target_homeroom_id)
    .bind(request.transfer_date)
    .bind(request.enrollment_type.trim())
    .bind(request.class_number)
    .execute(&mut *transaction)
    .await?;
    append_audit(
        &mut transaction,
        "homeroom_placement.transferred",
        "homeroom_placement",
        old.id,
        Some(old.academic_year_id),
        None,
        actor_user_id,
        serde_json::json!({
            "newPlacementId": new_id,
            "idempotencyDigest": digest,
        }),
    )
    .await?;
    transaction.commit().await?;
    Ok(HomeroomPlacementTransfer {
        ended_placement: get_placement(pool, old.id).await?,
        new_placement: get_placement(pool, new_id).await?,
        replayed: false,
    })
}

async fn get_placement(pool: &PgPool, id: Uuid) -> Result<HomeroomPlacement, AppError> {
    let sql = format!("SELECT {PLACEMENT_COLUMNS} FROM homeroom_placements WHERE id = $1");
    sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการจัดห้อง".to_string()))
}

async fn get_placement_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<HomeroomPlacement, AppError> {
    let sql =
        format!("SELECT {PLACEMENT_COLUMNS} FROM homeroom_placements WHERE id = $1 FOR UPDATE");
    sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการจัดห้อง".to_string()))
}

async fn replay_transfer(
    transaction: &mut Transaction<'_, Postgres>,
    digest: &str,
) -> Result<Option<HomeroomPlacementTransfer>, AppError> {
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"
        SELECT entity_id, (payload->>'newPlacementId')::uuid
        FROM academic_audit_events
        WHERE event_code = 'homeroom_placement.transferred'
          AND payload->>'idempotencyDigest' = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(digest)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((old_id, new_id)) = row else {
        return Ok(None);
    };
    let sql = format!("SELECT {PLACEMENT_COLUMNS} FROM homeroom_placements WHERE id = $1");
    let ended_placement = sqlx::query_as(&sql)
        .bind(old_id)
        .fetch_one(&mut **transaction)
        .await?;
    let new_placement = sqlx::query_as(&sql)
        .bind(new_id)
        .fetch_one(&mut **transaction)
        .await?;
    Ok(Some(HomeroomPlacementTransfer {
        ended_placement,
        new_placement,
        replayed: true,
    }))
}

fn validate_homeroom_fields(code: &str, name: &str, capacity: i32) -> Result<(), AppError> {
    if code.trim().is_empty() || name.trim().is_empty() || capacity <= 0 {
        return Err(AppError::ValidationError(
            "ข้อมูลห้องเรียนประจำชั้นไม่ถูกต้อง".to_string(),
        ));
    }
    Ok(())
}

async fn validate_homeroom_context(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
) -> Result<Uuid, AppError> {
    let year_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM academic_years WHERE id = $1 FOR SHARE")
            .bind(academic_year_id)
            .fetch_optional(&mut **transaction)
            .await?;
    if year_status.as_deref() != Some("planning") {
        return Err(AppError::Conflict(
            "จัดห้องได้เฉพาะปีการศึกษาสถานะ planning".to_string(),
        ));
    }
    let grade_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM grade_levels WHERE id = $1)")
            .bind(grade_level_id)
            .fetch_one(&mut **transaction)
            .await?;
    if !grade_exists {
        return Err(AppError::ValidationError("ระดับชั้นไม่ถูกต้อง".to_string()));
    }
    sqlx::query_scalar(
        r#"
        SELECT program.curriculum_version_id
        FROM study_programs program
        JOIN curriculum_versions version ON version.id = program.curriculum_version_id
        JOIN academic_years starts ON starts.id = version.start_academic_year_id
        JOIN academic_years target ON target.id = $1
        LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
        WHERE program.id = $2
          AND starts.start_date <= target.start_date
          AND (ends.end_date IS NULL OR ends.end_date >= target.end_date)
        "#,
    )
    .bind(academic_year_id)
    .bind(study_program_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::ValidationError("แผนการเรียนใช้ไม่ได้กับปีที่เลือก".to_string()))
}

async fn validate_student_year_context(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
) -> Result<(), AppError> {
    validate_homeroom_context(
        transaction,
        academic_year_id,
        grade_level_id,
        study_program_id,
    )
    .await
    .map(|_| ())
}

async fn validate_target_homeroom(
    transaction: &mut Transaction<'_, Postgres>,
    homeroom_id: Uuid,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
) -> Result<(), AppError> {
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM homerooms
            WHERE id = $1 AND academic_year_id = $2 AND grade_level_id = $3
              AND study_program_id = $4 AND is_active IS TRUE
        )
        "#,
    )
    .bind(homeroom_id)
    .bind(academic_year_id)
    .bind(grade_level_id)
    .bind(study_program_id)
    .fetch_one(&mut **transaction)
    .await?;
    if matches {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "ห้องเป้าหมายต้องอยู่ในปี ระดับชั้น และแผนการเรียนเดียวกับนักเรียน".to_string(),
        ))
    }
}

async fn validate_placement_date(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    date: chrono::NaiveDate,
) -> Result<(), AppError> {
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM academic_years WHERE id = $1 AND $2 BETWEEN start_date AND end_date)",
    )
    .bind(academic_year_id)
    .bind(date)
    .fetch_one(&mut **transaction)
    .await?;
    if valid {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "วันที่จัดห้องอยู่นอกปีการศึกษา".to_string(),
        ))
    }
}

fn map_homeroom_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.code().as_deref() == Some("23505") {
            return AppError::Conflict("ห้องเรียนซ้ำในปีและระดับชั้นเดียวกัน".to_string());
        }
    }
    AppError::DbError(error)
}

fn map_student_year_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("student_academic_years_student_year_key") {
            return AppError::Conflict("นักเรียนมีข้อมูลในปีการศึกษานี้แล้ว".to_string());
        }
    }
    AppError::DbError(error)
}

fn map_placement_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("homeroom_placements_one_current_key") {
            return AppError::Conflict("นักเรียนมีห้องปัจจุบันอยู่แล้ว".to_string());
        }
    }
    AppError::DbError(error)
}
