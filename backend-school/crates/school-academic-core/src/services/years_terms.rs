use chrono::Utc;
use school_database::DbSession;
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use school_errors::AppError;

use super::super::models::{
    AcademicTerm, AcademicTermStatus, AcademicTermType, AcademicYear, AcademicYearStatus,
    CreateAcademicTermRequest, CreateAcademicYearRequest, UpdateAcademicTermRequest,
    UpdateAcademicYearRequest,
};
use super::{lifecycle_guard, parse_row_version};

const SCHOOL_DAYS: &[&str] = &["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedTermIdentity {
    pub code: String,
    pub name: String,
}

pub fn derive_academic_year_name(year: i32, custom_name: Option<&str>) -> Result<String, AppError> {
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

pub fn derive_term_identity(
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
    is_visible: bool,
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
            school_days: row
                .school_days
                .split(',')
                .filter(|day| !day.is_empty())
                .map(str::to_string)
                .collect(),
            is_visible: row.is_visible,
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
    is_visible: bool,
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
            included_in_year_result: row.included_in_year_result,
            blocks_year_closure: row.blocks_year_closure,
            bell_schedule_id: row.bell_schedule_id,
            is_visible: row.is_visible,
            status: row.status,
            row_version: row.row_version,
            migrated: row.migrated,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(FromRow)]
struct AcademicYearUpdateRow {
    #[sqlx(flatten)]
    row: AcademicYearRow,
    before: serde_json::Value,
    after: serde_json::Value,
}

#[derive(FromRow)]
struct AcademicTermUpdateRow {
    #[sqlx(flatten)]
    row: AcademicTermRow,
    before: serde_json::Value,
    after: serde_json::Value,
}

const YEAR_COLUMNS: &str = r#"
    id, year, name, is_visible, school_days, status, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;

const TERM_COLUMNS: &str = r#"
    id, academic_year_id, sequence_no AS sequence, code, name, term_type,
    is_visible, included_in_year_result, blocks_year_closure,
    bell_schedule_id, status, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;

pub async fn list_years(pool: &PgPool) -> Result<Vec<AcademicYear>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.years_terms.list_years",
    )
    .await?;
    list_years_on_connection(&mut connection).await
}

pub async fn list_years_on_connection(
    connection: &mut sqlx::PgConnection,
) -> Result<Vec<AcademicYear>, AppError> {
    let sql = format!("SELECT {YEAR_COLUMNS} FROM academic_years ORDER BY year DESC, id");
    let rows = DbSession::new(connection)
        .query_as::<AcademicYearRow>(
            "academic_core.services.years_terms.list_years_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .fetch_all()
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub(super) async fn get_years_by_ids(
    connection: &mut sqlx::PgConnection,
    ids: &[Uuid],
) -> Result<Vec<AcademicYear>, AppError> {
    let sql =
        format!("SELECT {YEAR_COLUMNS} FROM academic_years WHERE id=ANY($1) ORDER BY year DESC,id");
    let rows = DbSession::new(connection)
        .query_as::<AcademicYearRow>(
            "academic_core.services.years_terms.get_years_by_ids",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(ids)
        .fetch_all()
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub(super) async fn get_terms_by_ids(
    connection: &mut sqlx::PgConnection,
    ids: &[Uuid],
) -> Result<Vec<AcademicTerm>, AppError> {
    let sql = format!(
        "SELECT {TERM_COLUMNS} FROM academic_terms WHERE id=ANY($1) ORDER BY academic_year_id,sequence_no,id"
    );
    let rows = DbSession::new(connection)
        .query_as::<AcademicTermRow>(
            "academic_core.services.years_terms.get_terms_by_ids",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(ids)
        .fetch_all()
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_year(pool: &PgPool, id: Uuid) -> Result<AcademicYear, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.years_terms.get_year",
    )
    .await?;
    get_year_on_connection(&mut connection, id).await
}

pub async fn get_year_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<AcademicYear, AppError> {
    let sql = format!("SELECT {YEAR_COLUMNS} FROM academic_years WHERE id = $1");
    DbSession::new(connection)
        .query_as::<AcademicYearRow>(
            "academic_core.services.years_terms.get_year_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .fetch_optional()
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
    let school_days = validate_year_fields(request.year, &request.school_days)?;
    let id = Uuid::now_v7();
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.years_terms.create_year",
    )
    .await?;
    lifecycle_guard::lock_transition_shared(&mut transaction).await?;
    let sql = format!(
        "INSERT INTO academic_years (id, year, name, school_days, status) \
         VALUES ($1, $2, $3, $4, 'planning') RETURNING {YEAR_COLUMNS}"
    );
    let row = DbSession::new(&mut *transaction)
        .query_as::<AcademicYearRow>(
            "academic_core.services.years_terms.create_year",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .bind(request.year)
        .bind(name)
        .bind(school_days.join(","))
        .fetch_one()
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
    let school_days = validate_year_fields(request.year, &request.school_days)?;
    parse_row_version(request.row_version)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.years_terms.update_year",
    )
    .await?;
    lifecycle_guard::require_year_write_exclusive(&mut transaction, id).await?;
    let returning = YEAR_COLUMNS
        .split(',')
        .map(|column| format!("NEW.{}", column.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "UPDATE academic_years SET year = $1, name = $2, school_days = $3, row_version = row_version + 1, updated_at = now() \
         WHERE id = $4 AND row_version = $5 AND status = 'planning' RETURNING {returning}, jsonb_build_object('year', OLD.year, 'name', OLD.name, 'schoolDays', OLD.school_days, 'rowVersion', OLD.row_version) AS before, jsonb_build_object('year', NEW.year, 'name', NEW.name, 'schoolDays', NEW.school_days, 'rowVersion', NEW.row_version) AS after"
    );
    let row = DbSession::new(&mut *transaction)
        .query_as::<AcademicYearUpdateRow>(
            "academic_core.services.years_terms.update_year",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(request.year)
        .bind(name)
        .bind(school_days.join(","))
        .bind(id)
        .bind(request.row_version)
        .fetch_optional()
        .await
        .map_err(map_year_write_error)?;
    let row = match row {
        Some(row) => row,
        None => {
            return Err(
                classify_year_update_failure(&mut transaction, id, request.row_version).await?,
            );
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
        json!({"rowVersion": row.row.row_version, "before": row.before, "after": row.after}),
    )
    .await?;
    transaction.commit().await?;
    Ok(row.row.into())
}

pub async fn list_terms(
    pool: &PgPool,
    academic_year_id: Uuid,
) -> Result<Vec<AcademicTerm>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.years_terms.list_terms",
    )
    .await?;
    list_terms_on_connection(&mut connection, academic_year_id).await
}

pub async fn list_terms_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
) -> Result<Vec<AcademicTerm>, AppError> {
    let sql = format!(
        "SELECT {TERM_COLUMNS} FROM academic_terms WHERE academic_year_id = $1 \
         ORDER BY sequence_no, id"
    );
    let rows = DbSession::new(connection)
        .query_as::<AcademicTermRow>(
            "academic_core.services.years_terms.list_terms_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(academic_year_id)
        .fetch_all()
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub(super) async fn list_all_terms(
    connection: &mut sqlx::PgConnection,
) -> Result<Vec<AcademicTerm>, AppError> {
    let sql = format!(
        "SELECT term.* FROM (SELECT {TERM_COLUMNS} FROM academic_terms) term \
         JOIN academic_years year ON year.id = term.academic_year_id \
         ORDER BY year.year DESC, year.id, term.sequence, term.id"
    );
    let rows = DbSession::new(connection)
        .query_as::<AcademicTermRow>(
            "academic_core.services.years_terms.list_all_terms",
            sqlx::AssertSqlSafe(sql),
        )
        .fetch_all()
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_term(pool: &PgPool, id: Uuid) -> Result<AcademicTerm, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.years_terms.get_term",
    )
    .await?;
    get_term_on_connection(&mut connection, id).await
}

pub async fn get_term_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<AcademicTerm, AppError> {
    let sql = format!("SELECT {TERM_COLUMNS} FROM academic_terms WHERE id = $1");
    DbSession::new(connection)
        .query_as::<AcademicTermRow>(
            "academic_core.services.years_terms.get_term_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .map(Into::into)
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".to_string()))
}

pub async fn create_term(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateAcademicTermRequest,
) -> Result<AcademicTerm, AppError> {
    validate_annual_inclusion(request.included_in_year_result, request.blocks_year_closure)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.years_terms.create_term",
    )
    .await?;
    validate_term_context(
        &mut transaction,
        request.academic_year_id,
        request.bell_schedule_id,
    )
    .await?;
    let sequence: i32 = DbSession::new(&mut *transaction).scalar("academic_core.services.years_terms.create_term",
        "SELECT COALESCE(MAX(sequence_no), 0) + 1 FROM academic_terms WHERE academic_year_id = $1",
    )
    .bind(request.academic_year_id)
    .fetch_one()
    .await?;
    let identity =
        derive_term_identity(request.term_type, sequence, request.custom_name.as_deref())?;
    validate_term_fields(&identity.name)?;
    let code = unique_term_code(
        &mut transaction,
        request.academic_year_id,
        &identity.code,
        sequence,
    )
    .await?;
    let id = Uuid::now_v7();
    let sql = format!(
        "INSERT INTO academic_terms (id, academic_year_id, sequence_no, code, name, term_type, \
         included_in_year_result, blocks_year_closure, bell_schedule_id, status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'planning') \
         RETURNING {TERM_COLUMNS}"
    );
    let row = DbSession::new(&mut *transaction)
        .query_as::<AcademicTermRow>(
            "academic_core.services.years_terms.create_term",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .bind(request.academic_year_id)
        .bind(sequence)
        .bind(&code)
        .bind(&identity.name)
        .bind(request.term_type)
        .bind(request.included_in_year_result)
        .bind(request.blocks_year_closure)
        .bind(request.bell_schedule_id)
        .fetch_one()
        .await
        .map_err(map_term_write_error)?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.years_terms.create_term",
            r#"INSERT INTO academic_assessment_phase_controls (
               academic_term_id, academic_year_id, phase_code, updated_by
           )
           SELECT $1, $2, phase.phase_code, $3
           FROM (
               VALUES
                   ('before_midterm'::text),
                   ('midterm'::text),
                   ('after_midterm'::text),
                   ('final'::text)
           ) AS phase(phase_code)"#,
        )
        .bind(id)
        .bind(request.academic_year_id)
        .bind(actor_user_id)
        .execute()
        .await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.years_terms.create_term",
            r#"INSERT INTO academic_gradebook_phase_controls (
               academic_term_id, academic_year_id, phase_code, updated_by
           )
           SELECT $1, $2, phase.phase_code, $3
           FROM (VALUES ('before_midterm'::text), ('midterm'::text),
                        ('after_midterm'::text), ('final'::text)) AS phase(phase_code)"#,
        )
        .bind(id)
        .bind(request.academic_year_id)
        .bind(actor_user_id)
        .execute()
        .await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.years_terms.create_term",
            r#"INSERT INTO academic_learner_evaluation_controls (
               academic_term_id, academic_year_id, domain, updated_by
           )
           SELECT $1, $2, evaluation.domain, $3
           FROM (VALUES ('desirable_characteristic'::text),
                        ('reading_thinking_writing'::text)) AS evaluation(domain)"#,
        )
        .bind(id)
        .bind(request.academic_year_id)
        .bind(actor_user_id)
        .execute()
        .await?;
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
    validate_annual_inclusion(request.included_in_year_result, request.blocks_year_closure)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.years_terms.update_term",
    )
    .await?;
    // Resolve identity without taking the child lock. Configuration and lifecycle
    // writers lock the year before the term to avoid inverse-order deadlocks.
    let (academic_year_id, sequence): (Uuid, i32) = DbSession::new(&mut *transaction)
        .query_as(
            "academic_core.services.years_terms.update_term",
            "SELECT academic_year_id, sequence_no FROM academic_terms WHERE id = $1",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".to_string()))?;
    validate_term_context(&mut transaction, academic_year_id, request.bell_schedule_id).await?;
    let identity =
        derive_term_identity(request.term_type, sequence, request.custom_name.as_deref())?;
    validate_term_fields(&identity.name)?;
    let returning = TERM_COLUMNS
        .split(',')
        .map(|column| format!("NEW.{}", column.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "UPDATE academic_terms SET name = $1, term_type = $2, included_in_year_result = $3, blocks_year_closure = $4, bell_schedule_id = $5, \
         row_version = row_version + 1, updated_at = now() \
         WHERE id = $6 AND row_version = $7 AND status = 'planning' RETURNING {returning}, jsonb_build_object('name', OLD.name, 'termType', OLD.term_type, 'includedInYearResult', OLD.included_in_year_result, 'blocksYearClosure', OLD.blocks_year_closure, 'bellScheduleId', OLD.bell_schedule_id, 'rowVersion', OLD.row_version) AS before, jsonb_build_object('name', NEW.name, 'termType', NEW.term_type, 'includedInYearResult', NEW.included_in_year_result, 'blocksYearClosure', NEW.blocks_year_closure, 'bellScheduleId', NEW.bell_schedule_id, 'rowVersion', NEW.row_version) AS after"
    );
    let row = DbSession::new(&mut *transaction)
        .query_as::<AcademicTermUpdateRow>(
            "academic_core.services.years_terms.update_term",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(identity.name)
        .bind(request.term_type)
        .bind(request.included_in_year_result)
        .bind(request.blocks_year_closure)
        .bind(request.bell_schedule_id)
        .bind(id)
        .bind(request.row_version)
        .fetch_optional()
        .await
        .map_err(map_term_write_error)?;
    let row = match row {
        Some(row) => row,
        None => {
            return Err(
                classify_term_update_failure(&mut transaction, id, request.row_version).await?,
            );
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
        json!({"rowVersion": row.row.row_version, "before": row.before, "after": row.after}),
    )
    .await?;
    transaction.commit().await?;
    Ok(row.row.into())
}

pub async fn delete_term(pool: &PgPool, actor_user_id: Uuid, id: Uuid) -> Result<(), AppError> {
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.years_terms.delete_term",
    )
    .await?;
    let academic_year_id: Uuid = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.years_terms.delete_term",
            "SELECT academic_year_id FROM academic_terms WHERE id=$1",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".to_string()))?;
    lock_term_planning_year(&mut transaction, academic_year_id).await?;
    let status: AcademicTermStatus = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.years_terms.delete_term",
            "SELECT status FROM academic_terms WHERE id=$1 AND academic_year_id=$2 FOR UPDATE",
        )
        .bind(id)
        .bind(academic_year_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนในปีที่เลือก".into()))?;
    let dependency_count: i64 = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.years_terms.delete_term",
            r#"
        SELECT
            (SELECT count(*) FROM learning_offerings WHERE academic_term_id = $1)
          + (SELECT count(*) FROM academic_timetable_blocks WHERE academic_term_id = $1)
          + (SELECT count(*) FROM course_assessment_plans WHERE academic_term_id = $1)
          + (SELECT count(*) FROM academic_exam_rounds WHERE academic_term_id = $1)
          + (SELECT count(*) FROM supervision_cycles WHERE academic_term_id = $1)
        "#,
        )
        .bind(id)
        .fetch_one()
        .await?;
    super::ensure_planning_delete(status, dependency_count)?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.years_terms.delete_term",
            "DELETE FROM academic_gradebook_phase_controls WHERE academic_term_id = $1",
        )
        .bind(id)
        .execute()
        .await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.years_terms.delete_term",
            "DELETE FROM academic_learner_evaluation_controls WHERE academic_term_id = $1",
        )
        .bind(id)
        .execute()
        .await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.years_terms.delete_term",
            "DELETE FROM academic_terms WHERE id = $1",
        )
        .bind(id)
        .execute()
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

fn validate_year_fields(year: i32, school_days: &[String]) -> Result<Vec<String>, AppError> {
    if year <= 0 {
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

fn validate_term_fields(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::ValidationError("ข้อมูลภาคเรียนไม่ถูกต้อง".to_string()));
    }
    Ok(())
}

async fn validate_term_context(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    bell_schedule_id: Uuid,
) -> Result<(), AppError> {
    lock_term_planning_year(transaction, academic_year_id).await?;
    let schedule_matches: bool = DbSession::new(&mut **transaction)
        .scalar(
            "academic_core.services.years_terms.validate_term_context",
            "SELECT EXISTS (SELECT 1 FROM bell_schedules WHERE id = $1 AND academic_year_id = $2)",
        )
        .bind(bell_schedule_id)
        .bind(academic_year_id)
        .fetch_one()
        .await?;
    if !schedule_matches {
        return Err(AppError::ValidationError(
            "ตารางคาบต้องอยู่ในปีการศึกษาเดียวกับภาคเรียน".to_string(),
        ));
    }
    Ok(())
}

fn ensure_term_planning_year(status: AcademicYearStatus) -> Result<(), AppError> {
    if !matches!(
        status,
        AcademicYearStatus::Planning | AcademicYearStatus::Active
    ) {
        return Err(AppError::Conflict(
            "จัดการภาคเรียนร่างได้เฉพาะปีที่กำลังวางแผนหรือกำลังใช้งาน".into(),
        ));
    }
    Ok(())
}

fn validate_annual_inclusion(included: bool, blocks_closure: bool) -> Result<(), AppError> {
    if included && !blocks_closure {
        return Err(AppError::ValidationError(
            "ภาคเรียนที่รวมผลรายปีต้องปิดให้ครบก่อนปิดปีการศึกษา".into(),
        ));
    }
    Ok(())
}

async fn lock_term_planning_year(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
) -> Result<(), AppError> {
    let status =
        lifecycle_guard::require_year_write_exclusive(transaction, academic_year_id).await?;
    ensure_term_planning_year(status)
}

async fn unique_term_code(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    base_code: &str,
    sequence: i32,
) -> Result<String, AppError> {
    let exists: bool = DbSession::new(&mut **transaction).scalar("academic_core.services.years_terms.unique_term_code",
        "SELECT EXISTS (SELECT 1 FROM academic_terms WHERE academic_year_id = $1 AND code = $2)",
    )
    .bind(academic_year_id)
    .bind(base_code)
    .fetch_one()
    .await?;
    if exists {
        Ok(format!("{base_code}-{sequence}"))
    } else {
        Ok(base_code.to_string())
    }
}

async fn classify_year_update_failure(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    expected: i64,
) -> Result<AppError, AppError> {
    let state: Option<(AcademicYearStatus, i64)> = DbSession::new(&mut **transaction)
        .query_as(
            "academic_core.services.years_terms.classify_year_update_failure",
            "SELECT status, row_version FROM academic_years WHERE id = $1",
        )
        .bind(id)
        .fetch_optional()
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
    let state: Option<(AcademicTermStatus, i64)> = DbSession::new(&mut **transaction)
        .query_as(
            "academic_core.services.years_terms.classify_term_update_failure",
            "SELECT status, row_version FROM academic_terms WHERE id = $1",
        )
        .bind(id)
        .fetch_optional()
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
            Some("academic_terms_included_blocks_closure_check") => {
                AppError::ValidationError("ภาคเรียนที่รวมผลรายปีต้องปิดให้ครบก่อนปิดปีการศึกษา".into())
            }
            _ => AppError::DbError(error),
        };
    }
    AppError::DbError(error)
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_term_planning_year, validate_annual_inclusion, validate_term_fields,
        AcademicYearStatus,
    };

    #[test]
    fn future_term_configuration_requires_an_open_year_and_consistent_annual_flags() {
        for status in [AcademicYearStatus::Planning, AcademicYearStatus::Active] {
            assert!(ensure_term_planning_year(status).is_ok());
        }
        for status in [
            AcademicYearStatus::Ready,
            AcademicYearStatus::Closing,
            AcademicYearStatus::Closed,
            AcademicYearStatus::Archived,
        ] {
            assert!(ensure_term_planning_year(status).is_err());
        }
        assert!(validate_annual_inclusion(false, false).is_ok());
        assert!(validate_annual_inclusion(false, true).is_ok());
        assert!(validate_annual_inclusion(true, true).is_ok());
        assert!(validate_annual_inclusion(true, false).is_err());
    }

    #[test]
    fn term_configuration_validates_its_identity_without_boundary_dates() {
        assert!(validate_term_fields("ภาคเรียนที่ 1").is_ok());
        assert!(validate_term_fields("  ").is_err());
    }
}

pub async fn append_audit<T: serde::Serialize>(
    transaction: &mut Transaction<'_, Postgres>,
    event_code: &str,
    entity_type: &str,
    entity_id: Uuid,
    academic_year_id: Option<Uuid>,
    academic_term_id: Option<Uuid>,
    actor_user_id: Uuid,
    payload: T,
) -> Result<(), AppError> {
    DbSession::new(&mut **transaction)
        .query(
            "academic_core.services.years_terms.append_audit",
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
        .execute()
        .await?;
    Ok(())
}
