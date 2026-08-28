use chrono::{NaiveDate, Utc};
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

use super::super::models::{
    AcademicTerm, AcademicTermStatus, AcademicTermType, AcademicYear, AcademicYearStatus,
    CreateAcademicTermRequest, CreateAcademicYearRequest, UpdateAcademicTermRequest,
    UpdateAcademicYearRequest,
};
use super::{parse_row_version, validate_date_containment};

const SCHOOL_DAYS: &[&str] = &["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DerivedTermIdentity {
    pub code: String,
    pub name: String,
}

pub(crate) fn derive_academic_year_name(
    year: i32,
    custom_name: Option<&str>,
) -> Result<String, AppError> {
    if year <= 0 {
        return Err(AppError::ValidationError(
            "ปีการศึกษาต้องมากกว่าศูนย์".to_string(),
        ));
    }
    match custom_name {
        Some(name) if name.trim().is_empty() => Err(AppError::ValidationError(
            "ชื่อแสดงผลที่กำหนดเองห้ามว่าง".to_string(),
        )),
        Some(name) => Ok(name.trim().to_string()),
        None => Ok(format!("ปีการศึกษา {year}")),
    }
}

pub(crate) fn derive_term_identity(
    term_type: AcademicTermType,
    sequence: i32,
    custom_name: Option<&str>,
) -> Result<DerivedTermIdentity, AppError> {
    if sequence <= 0 {
        return Err(AppError::ValidationError(
            "ลำดับภาคเรียนต้องมากกว่าศูนย์".to_string(),
        ));
    }
    let (code, standard_name) = match term_type {
        AcademicTermType::Regular => (sequence.to_string(), format!("ภาคเรียนที่ {sequence}")),
        AcademicTermType::Summer => ("SUMMER".to_string(), "ภาคฤดูร้อน".to_string()),
        AcademicTermType::Remedial => ("REMEDIAL".to_string(), "ภาคซ่อมเสริม".to_string()),
        AcademicTermType::Custom => (
            format!("CUSTOM-{sequence}"),
            format!("ภาคเรียนกำหนดเอง {sequence}"),
        ),
    };
    let name = match custom_name {
        Some(name) if name.trim().is_empty() => {
            return Err(AppError::ValidationError(
                "ชื่อแสดงผลที่กำหนดเองห้ามว่าง".to_string(),
            ));
        }
        Some(name) => name.trim().to_string(),
        None => standard_name,
    };
    Ok(DerivedTermIdentity { code, name })
}

#[derive(FromRow)]
struct AcademicYearRow {
    id: Uuid,
    year: i32,
    name: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
    school_days: String,
    status: AcademicYearStatus,
    row_version: i64,
    migrated: bool,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

impl From<AcademicYearRow> for AcademicYear {
    fn from(row: AcademicYearRow) -> Self {
        Self {
            id: row.id,
            year: row.year,
            name: row.name,
            start_date: row.start_date,
            end_date: row.end_date,
            school_days: row
                .school_days
                .split(',')
                .filter(|day| !day.is_empty())
                .map(str::to_string)
                .collect(),
            status: row.status,
            row_version: row.row_version,
            migrated: row.migrated,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(FromRow)]
struct AcademicTermRow {
    id: Uuid,
    academic_year_id: Uuid,
    sequence: i32,
    code: String,
    name: String,
    term_type: AcademicTermType,
    start_date: NaiveDate,
    end_date: NaiveDate,
    included_in_year_result: bool,
    blocks_year_closure: bool,
    bell_schedule_id: Uuid,
    status: AcademicTermStatus,
    row_version: i64,
    migrated: bool,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

impl From<AcademicTermRow> for AcademicTerm {
    fn from(row: AcademicTermRow) -> Self {
        Self {
            id: row.id,
            academic_year_id: row.academic_year_id,
            sequence: row.sequence,
            code: row.code,
            name: row.name,
            term_type: row.term_type,
            start_date: row.start_date,
            end_date: row.end_date,
            included_in_year_result: row.included_in_year_result,
            blocks_year_closure: row.blocks_year_closure,
            bell_schedule_id: row.bell_schedule_id,
            status: row.status,
            row_version: row.row_version,
            migrated: row.migrated,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

const YEAR_COLUMNS: &str = r#"
    id, year, name, start_date, end_date, school_days, status, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;

const TERM_COLUMNS: &str = r#"
    id, academic_year_id, sequence_no AS sequence, code, name, term_type,
    start_date, end_date, included_in_year_result, blocks_year_closure,
    bell_schedule_id, status, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;

pub async fn list_years(pool: &PgPool) -> Result<Vec<AcademicYear>, AppError> {
    let sql = format!("SELECT {YEAR_COLUMNS} FROM academic_years ORDER BY year DESC, id");
    let rows = sqlx::query_as::<_, AcademicYearRow>(&sql)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_year(pool: &PgPool, id: Uuid) -> Result<AcademicYear, AppError> {
    let sql = format!("SELECT {YEAR_COLUMNS} FROM academic_years WHERE id = $1");
    sqlx::query_as::<_, AcademicYearRow>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(Into::into)
        .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษา".to_string()))
}

pub async fn create_year(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateAcademicYearRequest,
) -> Result<AcademicYear, AppError> {
    let name = derive_academic_year_name(request.year, request.custom_name.as_deref())?;
    let school_days = validate_year_fields(
        request.year,
        request.start_date,
        request.end_date,
        &request.school_days,
    )?;
    let id = Uuid::new_v4();
    let mut transaction = pool.begin().await?;
    let sql = format!(
        "INSERT INTO academic_years (id, year, name, start_date, end_date, school_days, status) \
         VALUES ($1, $2, $3, $4, $5, $6, 'planning') RETURNING {YEAR_COLUMNS}"
    );
    let row = sqlx::query_as::<_, AcademicYearRow>(&sql)
        .bind(id)
        .bind(request.year)
        .bind(name)
        .bind(request.start_date)
        .bind(request.end_date)
        .bind(school_days.join(","))
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_year_write_error)?;
    append_audit(
        &mut transaction,
        "academic_year.created",
        "academic_year",
        id,
        Some(id),
        None,
        actor_user_id,
        json!({"year": request.year}),
    )
    .await?;
    transaction.commit().await?;
    Ok(row.into())
}

pub async fn update_year(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: UpdateAcademicYearRequest,
) -> Result<AcademicYear, AppError> {
    let name = derive_academic_year_name(request.year, request.custom_name.as_deref())?;
    let school_days = validate_year_fields(
        request.year,
        request.start_date,
        request.end_date,
        &request.school_days,
    )?;
    parse_row_version(request.row_version)?;
    let mut transaction = pool.begin().await?;
    ensure_terms_fit_year(&mut transaction, id, request.start_date, request.end_date).await?;
    let sql = format!(
        "UPDATE academic_years SET year = $1, name = $2, start_date = $3, end_date = $4, \
         school_days = $5, row_version = row_version + 1, updated_at = now() \
         WHERE id = $6 AND row_version = $7 AND status = 'planning' RETURNING {YEAR_COLUMNS}"
    );
    let row = sqlx::query_as::<_, AcademicYearRow>(&sql)
        .bind(request.year)
        .bind(name)
        .bind(request.start_date)
        .bind(request.end_date)
        .bind(school_days.join(","))
        .bind(id)
        .bind(request.row_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_year_write_error)?;
    let row = match row {
        Some(row) => row,
        None => {
            return Err(
                classify_year_update_failure(&mut transaction, id, request.row_version).await?,
            )
        }
    };
    append_audit(
        &mut transaction,
        "academic_year.updated",
        "academic_year",
        id,
        Some(id),
        None,
        actor_user_id,
        json!({"rowVersion": row.row_version}),
    )
    .await?;
    transaction.commit().await?;
    Ok(row.into())
}

pub async fn list_terms(
    pool: &PgPool,
    academic_year_id: Uuid,
) -> Result<Vec<AcademicTerm>, AppError> {
    let sql = format!(
        "SELECT {TERM_COLUMNS} FROM academic_terms WHERE academic_year_id = $1 \
         ORDER BY sequence_no, start_date, id"
    );
    let rows = sqlx::query_as::<_, AcademicTermRow>(&sql)
        .bind(academic_year_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub(super) async fn list_all_terms(pool: &PgPool) -> Result<Vec<AcademicTerm>, AppError> {
    let sql = format!(
        "SELECT term.* FROM (SELECT {TERM_COLUMNS} FROM academic_terms) term \
         JOIN academic_years year ON year.id = term.academic_year_id \
         ORDER BY year.year DESC, year.id, term.sequence, term.start_date, term.id"
    );
    let rows = sqlx::query_as::<_, AcademicTermRow>(&sql)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_term(pool: &PgPool, id: Uuid) -> Result<AcademicTerm, AppError> {
    let sql = format!("SELECT {TERM_COLUMNS} FROM academic_terms WHERE id = $1");
    sqlx::query_as::<_, AcademicTermRow>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(Into::into)
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".to_string()))
}

pub async fn create_term(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateAcademicTermRequest,
) -> Result<AcademicTerm, AppError> {
    let mut transaction = pool.begin().await?;
    validate_term_context(
        &mut transaction,
        request.academic_year_id,
        request.bell_schedule_id,
        request.start_date,
        request.end_date,
    )
    .await?;
    let sequence: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence_no), 0) + 1 FROM academic_terms WHERE academic_year_id = $1",
    )
    .bind(request.academic_year_id)
    .fetch_one(&mut *transaction)
    .await?;
    let identity =
        derive_term_identity(request.term_type, sequence, request.custom_name.as_deref())?;
    validate_term_fields(&identity.name, request.start_date, request.end_date)?;
    let code = unique_term_code(
        &mut transaction,
        request.academic_year_id,
        &identity.code,
        sequence,
    )
    .await?;
    let id = Uuid::new_v4();
    let sql = format!(
        "INSERT INTO academic_terms (id, academic_year_id, sequence_no, code, name, term_type, \
         start_date, end_date, included_in_year_result, blocks_year_closure, bell_schedule_id, status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'planning') \
         RETURNING {TERM_COLUMNS}"
    );
    let row = sqlx::query_as::<_, AcademicTermRow>(&sql)
        .bind(id)
        .bind(request.academic_year_id)
        .bind(sequence)
        .bind(&code)
        .bind(&identity.name)
        .bind(request.term_type)
        .bind(request.start_date)
        .bind(request.end_date)
        .bind(request.included_in_year_result)
        .bind(request.blocks_year_closure)
        .bind(request.bell_schedule_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_term_write_error)?;
    append_audit(
        &mut transaction,
        "academic_term.created",
        "academic_term",
        id,
        Some(request.academic_year_id),
        Some(id),
        actor_user_id,
        json!({"sequence": sequence, "code": code}),
    )
    .await?;
    transaction.commit().await?;
    Ok(row.into())
}

pub async fn update_term(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: UpdateAcademicTermRequest,
) -> Result<AcademicTerm, AppError> {
    parse_row_version(request.row_version)?;
    let mut transaction = pool.begin().await?;
    let (academic_year_id, sequence): (Uuid, i32) = sqlx::query_as(
        "SELECT academic_year_id, sequence_no FROM academic_terms WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".to_string()))?;
    validate_term_context(
        &mut transaction,
        academic_year_id,
        request.bell_schedule_id,
        request.start_date,
        request.end_date,
    )
    .await?;
    let identity =
        derive_term_identity(request.term_type, sequence, request.custom_name.as_deref())?;
    validate_term_fields(&identity.name, request.start_date, request.end_date)?;
    let sql = format!(
        "UPDATE academic_terms SET name = $1, term_type = $2, start_date = $3, end_date = $4, \
         included_in_year_result = $5, blocks_year_closure = $6, bell_schedule_id = $7, \
         row_version = row_version + 1, updated_at = now() \
         WHERE id = $8 AND row_version = $9 AND status = 'planning' RETURNING {TERM_COLUMNS}"
    );
    let row = sqlx::query_as::<_, AcademicTermRow>(&sql)
        .bind(identity.name)
        .bind(request.term_type)
        .bind(request.start_date)
        .bind(request.end_date)
        .bind(request.included_in_year_result)
        .bind(request.blocks_year_closure)
        .bind(request.bell_schedule_id)
        .bind(id)
        .bind(request.row_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_term_write_error)?;
    let row = match row {
        Some(row) => row,
        None => {
            return Err(
                classify_term_update_failure(&mut transaction, id, request.row_version).await?,
            )
        }
    };
    append_audit(
        &mut transaction,
        "academic_term.updated",
        "academic_term",
        id,
        Some(academic_year_id),
        Some(id),
        actor_user_id,
        json!({"rowVersion": row.row_version}),
    )
    .await?;
    transaction.commit().await?;
    Ok(row.into())
}

pub async fn delete_term(pool: &PgPool, actor_user_id: Uuid, id: Uuid) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    let (academic_year_id, status): (Uuid, AcademicTermStatus) = sqlx::query_as(
        "SELECT academic_year_id, status FROM academic_terms WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".to_string()))?;
    let dependency_count: i64 = sqlx::query_scalar(
        r#"
        SELECT
            (SELECT count(*) FROM learning_offerings WHERE academic_term_id = $1)
          + (SELECT count(*) FROM academic_timetable_entries WHERE academic_term_id = $1)
          + (SELECT count(*) FROM course_assessment_plans WHERE academic_term_id = $1)
          + (SELECT count(*) FROM academic_exam_rounds WHERE academic_term_id = $1)
          + (SELECT count(*) FROM supervision_cycles WHERE academic_term_id = $1)
          + (SELECT count(*) FROM calendar_events WHERE academic_term_id = $1)
        "#,
    )
    .bind(id)
    .fetch_one(&mut *transaction)
    .await?;
    super::ensure_planning_delete(status, dependency_count)?;
    sqlx::query("DELETE FROM academic_terms WHERE id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    append_audit(
        &mut transaction,
        "academic_term.deleted",
        "academic_term",
        id,
        Some(academic_year_id),
        None,
        actor_user_id,
        json!({}),
    )
    .await?;
    transaction.commit().await?;
    Ok(())
}

fn validate_year_fields(
    year: i32,
    start_date: NaiveDate,
    end_date: NaiveDate,
    school_days: &[String],
) -> Result<Vec<String>, AppError> {
    if year <= 0 || start_date > end_date {
        return Err(AppError::ValidationError("ข้อมูลปีการศึกษาไม่ถูกต้อง".to_string()));
    }
    if school_days.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องกำหนดวันเรียนอย่างน้อยหนึ่งวัน".to_string(),
        ));
    }
    let mut normalized = Vec::new();
    for supported in SCHOOL_DAYS {
        let count = school_days
            .iter()
            .filter(|day| day.trim().eq_ignore_ascii_case(supported))
            .count();
        if count > 1 {
            return Err(AppError::ValidationError(
                "วันเรียนไม่ถูกต้องหรือซ้ำกัน".to_string(),
            ));
        }
        if count == 1 {
            normalized.push((*supported).to_string());
        }
    }
    if normalized.len() != school_days.len() {
        return Err(AppError::ValidationError(
            "วันเรียนไม่ถูกต้องหรือซ้ำกัน".to_string(),
        ));
    }
    Ok(normalized)
}

fn validate_term_fields(
    name: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<(), AppError> {
    if name.trim().is_empty() || start_date > end_date {
        return Err(AppError::ValidationError("ข้อมูลภาคเรียนไม่ถูกต้อง".to_string()));
    }
    Ok(())
}

async fn validate_term_context(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    bell_schedule_id: Uuid,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<(), AppError> {
    let (year_start, year_end, status): (NaiveDate, NaiveDate, AcademicYearStatus) =
        sqlx::query_as(
            "SELECT start_date, end_date, status FROM academic_years WHERE id = $1 FOR UPDATE",
        )
        .bind(academic_year_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษา".to_string()))?;
    if status != AcademicYearStatus::Planning {
        return Err(AppError::Conflict(
            "แก้ไขภาคเรียนได้เฉพาะปีการศึกษาสถานะ planning".to_string(),
        ));
    }
    validate_date_containment(year_start, year_end, start_date, end_date)?;
    let schedule_matches: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM bell_schedules WHERE id = $1 AND academic_year_id = $2)",
    )
    .bind(bell_schedule_id)
    .bind(academic_year_id)
    .fetch_one(&mut **transaction)
    .await?;
    if !schedule_matches {
        return Err(AppError::ValidationError(
            "ตารางคาบต้องอยู่ในปีการศึกษาเดียวกับภาคเรียน".to_string(),
        ));
    }
    Ok(())
}

async fn unique_term_code(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    base_code: &str,
    sequence: i32,
) -> Result<String, AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM academic_terms WHERE academic_year_id = $1 AND code = $2)",
    )
    .bind(academic_year_id)
    .bind(base_code)
    .fetch_one(&mut **transaction)
    .await?;
    if exists {
        Ok(format!("{base_code}-{sequence}"))
    } else {
        Ok(base_code.to_string())
    }
}

async fn ensure_terms_fit_year(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<(), AppError> {
    let invalid: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM academic_terms WHERE academic_year_id = $1 \
         AND (start_date < $2 OR end_date > $3))",
    )
    .bind(academic_year_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_one(&mut **transaction)
    .await?;
    if invalid {
        return Err(AppError::ValidationError(
            "ช่วงปีการศึกษาใหม่ไม่ครอบคลุมภาคเรียนเดิม".to_string(),
        ));
    }
    Ok(())
}

async fn classify_year_update_failure(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    expected: i64,
) -> Result<AppError, AppError> {
    let state: Option<(AcademicYearStatus, i64)> =
        sqlx::query_as("SELECT status, row_version FROM academic_years WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut **transaction)
            .await?;
    Ok(match state {
        None => AppError::NotFound("ไม่พบปีการศึกษา".to_string()),
        Some((status, _)) if status != AcademicYearStatus::Planning => {
            AppError::Conflict("แก้ไขได้เฉพาะปีการศึกษาสถานะ planning".to_string())
        }
        Some((_, actual)) => AppError::Conflict(format!(
            "ข้อมูลปีการศึกษาถูกแก้ไขแล้ว (expected {expected}, actual {actual})"
        )),
    })
}

async fn classify_term_update_failure(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    expected: i64,
) -> Result<AppError, AppError> {
    let state: Option<(AcademicTermStatus, i64)> =
        sqlx::query_as("SELECT status, row_version FROM academic_terms WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut **transaction)
            .await?;
    Ok(match state {
        None => AppError::NotFound("ไม่พบภาคเรียน".to_string()),
        Some((status, _)) if status != AcademicTermStatus::Planning => {
            AppError::Conflict("แก้ไขได้เฉพาะภาคเรียนสถานะ planning".to_string())
        }
        Some((_, actual)) => AppError::Conflict(format!(
            "ข้อมูลภาคเรียนถูกแก้ไขแล้ว (expected {expected}, actual {actual})"
        )),
    })
}

fn map_year_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("academic_years_year_key") {
            return AppError::Conflict("ปีการศึกษาซ้ำ".to_string());
        }
    }
    AppError::DbError(error)
}

fn map_term_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        return match database.constraint() {
            Some("academic_terms_year_sequence_key") => {
                AppError::Conflict("ลำดับภาคเรียนซ้ำภายในปีการศึกษา".to_string())
            }
            Some("academic_terms_year_code_key") => {
                AppError::Conflict("รหัสภาคเรียนซ้ำภายในปีการศึกษา".to_string())
            }
            _ => AppError::DbError(error),
        };
    }
    AppError::DbError(error)
}

pub(super) async fn append_audit<T: serde::Serialize>(
    transaction: &mut Transaction<'_, Postgres>,
    event_code: &str,
    entity_type: &str,
    entity_id: Uuid,
    academic_year_id: Option<Uuid>,
    academic_term_id: Option<Uuid>,
    actor_user_id: Uuid,
    payload: T,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO academic_audit_events (
            event_code, entity_type, entity_id, academic_year_id, academic_term_id,
            actor_user_id, payload
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(event_code)
    .bind(entity_type)
    .bind(entity_id)
    .bind(academic_year_id)
    .bind(academic_term_id)
    .bind(actor_user_id)
    .bind(sqlx::types::Json(payload))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
