use chrono::NaiveDate;
use school_database::DbSession;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::GradeLevelLookupItem;
use school_authorization::AcademicResourceListFilter;
use school_errors::AppError;

use super::super::models::{
    ActivityVersion, CatalogActivity, CatalogActivityOverview, CatalogActivityOverviewItem,
    CatalogDisplayState, CatalogSubject, CatalogSubjectGroupOption, CatalogSubjectOverview,
    CatalogSubjectOverviewItem, CreateActivityVersionRequest, CreateCatalogActivityRequest,
    CreateCatalogSubjectRequest, CreateSubjectGroupRequest, CreateSubjectVersionRequest,
    DefaultTeacher, DeleteCatalogRequest, PublishVersionRequest, ReplaceDefaultTeachersRequest,
    SubjectGroup, SubjectVersion, UpdateActivityVersionRequest, UpdateCatalogActivityRequest,
    UpdateCatalogSubjectRequest, UpdateSubjectGroupRequest, UpdateSubjectVersionRequest,
    VersionStatus,
};
use super::{ensure_draft_version, parse_row_version, validate_canonical_decimal};

const SUBJECT_COLUMNS: &str =
    "id, code, subject_group_id, owning_organization_unit_id, archived_at, row_version, created_at, updated_at";
const ACTIVITY_COLUMNS: &str =
    "id, code, activity_type, owning_organization_unit_id, archived_at, row_version, created_at, updated_at";

// Owned links (draft grades and default teachers) are removed with the catalog entry.
// All downstream references, including publication snapshots without catalog FKs, refuse deletion.
#[derive(Clone, Copy)]
enum CatalogKind {
    Subject,
    Activity,
}

impl CatalogKind {
    fn tables(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Subject => ("subjects", "subject_versions", "subject_id"),
            Self::Activity => ("activities", "activity_versions", "activity_id"),
        }
    }

    fn deletable_predicate(self) -> &'static str {
        match self {
            Self::Subject => {
                r#"stable.archived_at IS NULL
                AND NOT EXISTS (SELECT 1 FROM subject_versions v WHERE v.subject_id = stable.id
                    AND (v.status <> 'draft' OR v.published_at IS NOT NULL))
                AND NOT EXISTS (SELECT 1 FROM course_offering_details r WHERE r.subject_id = stable.id)
                AND NOT EXISTS (SELECT 1 FROM academic_exam_schedule_items r WHERE r.subject_id = stable.id)
                AND NOT EXISTS (SELECT 1 FROM academic_question_bank_questions r WHERE r.subject_id = stable.id)
                AND NOT EXISTS (SELECT 1 FROM subject_term_evaluation_configurations r WHERE r.subject_id = stable.id)
                AND NOT EXISTS (SELECT 1 FROM subject_term_evaluation_criteria r WHERE r.subject_id = stable.id)
                AND NOT EXISTS (SELECT 1 FROM subject_term_evaluation_locks r WHERE r.subject_id = stable.id)
                AND NOT EXISTS (SELECT 1 FROM academic_course_result_locks r WHERE r.subject_id = stable.id)
                AND NOT EXISTS (
                    SELECT 1 FROM subject_versions v WHERE v.subject_id = stable.id AND (
                        EXISTS (SELECT 1 FROM curriculum_course_requirements r WHERE r.subject_version_id = v.id)
                        OR EXISTS (SELECT 1 FROM curriculum_publication_courses r WHERE r.subject_version_id = v.id)
                        OR EXISTS (SELECT 1 FROM curriculum_requirement_sources r WHERE r.resource_kind = 'course' AND r.catalog_version_id = v.id)
                        OR EXISTS (SELECT 1 FROM course_assessment_plans r WHERE r.subject_version_id = v.id)
                    ))"#
            }
            Self::Activity => {
                r#"stable.archived_at IS NULL
                AND NOT EXISTS (SELECT 1 FROM activity_versions v WHERE v.activity_id = stable.id
                    AND (v.status <> 'draft' OR v.published_at IS NOT NULL))
                AND NOT EXISTS (SELECT 1 FROM activity_offering_details r WHERE r.activity_id = stable.id)
                AND NOT EXISTS (
                    SELECT 1 FROM activity_versions v WHERE v.activity_id = stable.id AND (
                        EXISTS (SELECT 1 FROM curriculum_activity_requirements r WHERE r.activity_version_id = v.id)
                        OR EXISTS (SELECT 1 FROM curriculum_publication_activities r WHERE r.activity_version_id = v.id)
                        OR EXISTS (SELECT 1 FROM curriculum_requirement_sources r WHERE r.resource_kind = 'activity' AND r.catalog_version_id = v.id)
                    ))"#
            }
        }
    }
}

async fn deletable_catalog_ids(
    connection: &mut sqlx::PgConnection,
    kind: CatalogKind,
    ids: &[Uuid],
) -> Result<std::collections::HashSet<Uuid>, AppError> {
    if ids.is_empty() {
        return Ok(std::collections::HashSet::new());
    }
    let (table, _, _) = kind.tables();
    let sql = format!(
        "SELECT stable.id FROM {table} stable WHERE stable.id = ANY($1) AND {}",
        kind.deletable_predicate()
    );
    Ok(DbSession::new(connection)
        .scalar(
            "academic_core.services.catalog.deletable_catalog_ids",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(ids)
        .fetch_all()
        .await?
        .into_iter()
        .collect())
}

pub async fn delete_subject(
    pool: &PgPool,
    id: Uuid,
    request: DeleteCatalogRequest,
) -> Result<(), AppError> {
    delete_catalog(pool, CatalogKind::Subject, id, request).await
}

pub async fn delete_activity(
    pool: &PgPool,
    id: Uuid,
    request: DeleteCatalogRequest,
) -> Result<(), AppError> {
    delete_catalog(pool, CatalogKind::Activity, id, request).await
}

async fn delete_catalog(
    pool: &PgPool,
    kind: CatalogKind,
    id: Uuid,
    request: DeleteCatalogRequest,
) -> Result<(), AppError> {
    parse_row_version(request.row_version)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.delete_catalog",
    )
    .await?;
    let (table, versions, foreign_key) = kind.tables();
    // Create and publish commands take this same stable-identity lock before inspecting versions.
    let row_version: i64 = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.catalog.delete_catalog",
            sqlx::AssertSqlSafe(format!(
                "SELECT row_version FROM {table} WHERE id = $1 FOR UPDATE"
            )),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการในคลัง".to_string()))?;
    if row_version != request.row_version {
        return Err(AppError::Conflict(
            "รายการในคลังถูกแก้ไขแล้ว กรุณาโหลดข้อมูลใหม่".to_string(),
        ));
    }
    if !deletable_catalog_ids(&mut transaction, kind, &[id])
        .await?
        .contains(&id)
    {
        return Err(AppError::Conflict(
            "ลบได้เฉพาะรายการที่ยังไม่เคยเผยแพร่ ไม่เก็บถาวร และยังไม่มีการอ้างอิง".to_string(),
        ));
    }
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.catalog.delete_catalog",
            sqlx::AssertSqlSafe(format!("DELETE FROM {versions} WHERE {foreign_key} = $1")),
        )
        .bind(id)
        .execute()
        .await
        .map_err(map_catalog_delete_error)?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.catalog.delete_catalog",
            sqlx::AssertSqlSafe(format!("DELETE FROM {table} WHERE id = $1")),
        )
        .bind(id)
        .execute()
        .await
        .map_err(map_catalog_delete_error)?;
    transaction
        .commit()
        .await
        .map_err(map_catalog_delete_error)?;
    Ok(())
}

fn map_catalog_delete_error(error: sqlx::Error) -> AppError {
    if matches!(&error, sqlx::Error::Database(database) if database.code().as_deref() == Some("23503"))
    {
        AppError::Conflict("รายการในคลังมีการอ้างอิงแล้ว จึงไม่สามารถลบได้".to_string())
    } else {
        AppError::DbError(error)
    }
}

/// The school civil date used for subject catalog eligibility.
pub fn subject_catalog_today() -> NaiveDate {
    subject_catalog_date_at(chrono::Utc::now())
}

fn subject_catalog_date_at(now: chrono::DateTime<chrono::Utc>) -> NaiveDate {
    now.with_timezone(&chrono_tz::Asia::Bangkok).date_naive()
}

#[derive(sqlx::FromRow)]
struct SubjectRevisionSelection {
    id: Uuid,
    subject_id: Uuid,
    version_no: i32,
    effective_from: NaiveDate,
}

fn select_current_subject_revision<'a, T: 'a>(
    versions: impl Iterator<Item = &'a T>,
    today: NaiveDate,
    dates: impl Fn(&T) -> (NaiveDate, i32),
) -> Option<&'a T> {
    versions
        .filter(|version| dates(version).0 <= today)
        .max_by_key(|version| {
            let (from, revision) = dates(version);
            (from, revision)
        })
}

/// Provider-selected current published revision for one stable subject.
#[derive(Debug, Clone, Copy)]
pub struct CurrentSubjectRevision {
    pub id: Uuid,
    pub version_no: i32,
}

/// Current published metadata only: future revisions never become current early. Callers making a
/// write decision hold stable-subject locks until their transaction commits.
pub async fn current_subject_revisions(
    pool: &PgPool,
    subject_ids: &[Uuid],
) -> Result<HashMap<Uuid, CurrentSubjectRevision>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.current_subject_revisions",
    )
    .await?;
    current_subject_revisions_on_connection(&mut connection, subject_ids).await
}
pub async fn current_subject_revisions_on_connection(
    connection: &mut sqlx::PgConnection,
    subject_ids: &[Uuid],
) -> Result<HashMap<Uuid, CurrentSubjectRevision>, AppError> {
    let versions = DbSession::new(connection).query_as::<SubjectRevisionSelection>("academic_core.services.catalog.current_subject_revisions_on_connection",
        "SELECT id,subject_id,version_no,effective_from FROM subject_versions WHERE subject_id=ANY($1) AND status='published'",
    ).bind(subject_ids).fetch_all().await?;
    let mut grouped = HashMap::<Uuid, Vec<SubjectRevisionSelection>>::new();
    for version in versions {
        grouped.entry(version.subject_id).or_default().push(version);
    }
    let today = subject_catalog_today();
    Ok(grouped
        .iter()
        .filter_map(|(subject_id, versions)| {
            select_current_subject_revision(versions.iter(), today, |version| {
                (version.effective_from, version.version_no)
            })
            .map(|version| {
                (
                    *subject_id,
                    CurrentSubjectRevision {
                        id: version.id,
                        version_no: version.version_no,
                    },
                )
            })
        })
        .collect())
}

/// ID-only adapter for current revision admission decisions.
pub async fn current_subject_version_ids(
    pool: &PgPool,
    subject_ids: &[Uuid],
) -> Result<HashMap<Uuid, Uuid>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.current_subject_version_ids",
    )
    .await?;
    current_subject_version_ids_on_connection(&mut connection, subject_ids).await
}
pub async fn current_subject_version_ids_on_connection(
    connection: &mut sqlx::PgConnection,
    subject_ids: &[Uuid],
) -> Result<HashMap<Uuid, Uuid>, AppError> {
    Ok(
        current_subject_revisions_on_connection(connection, subject_ids)
            .await?
            .into_iter()
            .map(|(subject_id, revision)| (subject_id, revision.id))
            .collect(),
    )
}

/// Match the lock used by subject revision commands, in a deterministic multi-subject order.
pub async fn lock_subjects_for_revision_selection(
    transaction: &mut Transaction<'_, Postgres>,
    subject_ids: &[Uuid],
) -> Result<(), AppError> {
    DbSession::new(&mut **transaction)
        .scalar::<Uuid>(
            "academic_core.services.catalog.lock_subjects_for_revision_selection",
            "SELECT id FROM subjects WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(subject_ids)
        .fetch_all()
        .await?;
    Ok(())
}

pub fn owner_allowed(filter: &AcademicResourceListFilter, owner: Option<Uuid>) -> bool {
    filter.includes_school_owned
        || owner.is_some_and(|owner_id| filter.allowed_organization_unit_ids().contains(&owner_id))
}

pub async fn list_subjects(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<CatalogSubject>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.list_subjects",
    )
    .await?;
    list_subjects_on_connection(&mut connection, filter).await
}

pub async fn list_subjects_on_connection(
    connection: &mut sqlx::PgConnection,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<CatalogSubject>, AppError> {
    let owner_ids = filter.allowed_organization_unit_ids();
    let sql = format!(
        "SELECT {SUBJECT_COLUMNS} FROM subjects \
         WHERE $1 OR owning_organization_unit_id = ANY($2) ORDER BY code, id LIMIT 500"
    );
    Ok(DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.list_subjects_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(filter.includes_school_owned)
        .bind(owner_ids)
        .fetch_all()
        .await?)
}

pub async fn list_subject_overview(
    pool: &PgPool,
    read_filter: &AcademicResourceListFilter,
    manage_filter: &AcademicResourceListFilter,
    today: NaiveDate,
) -> Result<CatalogSubjectOverview, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.list_subject_overview",
    )
    .await?;
    list_subject_overview_on_connection(&mut connection, read_filter, manage_filter, today).await
}

pub async fn list_subject_overview_on_connection(
    connection: &mut sqlx::PgConnection,
    read_filter: &AcademicResourceListFilter,
    manage_filter: &AcademicResourceListFilter,
    today: NaiveDate,
) -> Result<CatalogSubjectOverview, AppError> {
    let subjects = list_subjects_on_connection(connection, read_filter).await?;
    let subject_ids = subjects
        .iter()
        .map(|subject| subject.id)
        .collect::<Vec<_>>();
    let versions: Vec<SubjectVersion> = if subject_ids.is_empty() {
        Vec::new()
    } else {
        DbSession::new(connection)
            .query_as(
                "academic_core.services.catalog.list_subject_overview_on_connection",
                sqlx::AssertSqlSafe(subject_version_select(
                    "version.subject_id = ANY($1)",
                    "version.subject_id, version.version_no DESC",
                )),
            )
            .bind(&subject_ids)
            .fetch_all()
            .await?
    };
    let deletable_ids =
        deletable_catalog_ids(connection, CatalogKind::Subject, &subject_ids).await?;
    let catalog_grade_levels = list_catalog_grade_levels(connection).await?;
    let grade_level_options = active_catalog_grade_levels(&catalog_grade_levels);
    let subject_group_options =
        list_catalog_subject_group_options(connection, read_filter, manage_filter).await?;
    let mut versions_by_subject = HashMap::<Uuid, Vec<SubjectVersion>>::new();
    for version in versions {
        versions_by_subject
            .entry(version.subject_id)
            .or_default()
            .push(version);
    }

    let items = subjects
        .into_iter()
        .map(|subject| {
            let versions = versions_by_subject.remove(&subject.id).unwrap_or_default();
            let (display_version, display_state, draft_count) =
                select_subject_display(&versions, today);
            let grade_levels = display_version
                .as_ref()
                .map(|version| {
                    resolve_grade_levels(&version.grade_level_ids, &catalog_grade_levels)
                })
                .unwrap_or_default();
            let can_manage =
                owner_allowed(manage_filter, Some(subject.owning_organization_unit_id));
            let can_delete = can_manage && deletable_ids.contains(&subject.id);
            CatalogSubjectOverviewItem {
                can_delete,
                subject,
                display_version,
                display_state,
                draft_count,
                grade_levels,
                can_manage,
            }
        })
        .collect();

    Ok(CatalogSubjectOverview {
        items,
        grade_level_options,
        subject_group_options,
    })
}

pub async fn get_subject(pool: &PgPool, id: Uuid) -> Result<CatalogSubject, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.get_subject",
    )
    .await?;
    get_subject_on_connection(&mut connection, id).await
}

pub async fn get_subject_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<CatalogSubject, AppError> {
    let sql = format!("SELECT {SUBJECT_COLUMNS} FROM subjects WHERE id = $1");
    DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.get_subject_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชา".to_string()))
}

pub async fn create_subject(
    pool: &PgPool,
    request: CreateCatalogSubjectRequest,
) -> Result<CatalogSubject, AppError> {
    let _preflight_code = normalize_code(&request.code)?;

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.create_subject",
    )
    .await?;
    create_subject_on_connection(&mut connection, request).await
}

pub async fn create_subject_on_connection(
    connection: &mut sqlx::PgConnection,
    request: CreateCatalogSubjectRequest,
) -> Result<CatalogSubject, AppError> {
    let code = normalize_code(&request.code)?;
    let owner_id =
        resolve_subject_group_owner_on_connection(connection, request.subject_group_id).await?;
    let id = Uuid::now_v7();
    let sql = format!(
        "INSERT INTO subjects (id, code, identity_key, subject_group_id, owning_organization_unit_id) \
         VALUES ($1, $2, $3, $4, $5) RETURNING {SUBJECT_COLUMNS}"
    );
    DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.create_subject_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .bind(&code)
        .bind(code.to_lowercase())
        .bind(request.subject_group_id)
        .bind(owner_id)
        .fetch_one()
        .await
        .map_err(map_catalog_write_error)
}

pub async fn update_subject(
    pool: &PgPool,
    id: Uuid,
    request: UpdateCatalogSubjectRequest,
) -> Result<CatalogSubject, AppError> {
    parse_row_version(request.row_version)?;
    let _preflight_code = normalize_code(&request.code)?;

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.update_subject",
    )
    .await?;
    update_subject_on_connection(&mut connection, id, request).await
}

pub async fn update_subject_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
    request: UpdateCatalogSubjectRequest,
) -> Result<CatalogSubject, AppError> {
    parse_row_version(request.row_version)?;
    let code = normalize_code(&request.code)?;
    let owner_id =
        resolve_subject_group_owner_on_connection(connection, request.subject_group_id).await?;
    let sql = format!(
        "UPDATE subjects SET code = $1, identity_key = $2, subject_group_id = $3, \
         owning_organization_unit_id = $4, \
         archived_at = CASE WHEN $5 THEN COALESCE(archived_at, now()) ELSE NULL END, \
         row_version = row_version + 1, updated_at = now() WHERE id = $6 AND row_version = $7 \
         RETURNING {SUBJECT_COLUMNS}"
    );
    DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.update_subject_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(&code)
        .bind(code.to_lowercase())
        .bind(request.subject_group_id)
        .bind(owner_id)
        .bind(request.archived)
        .bind(id)
        .bind(request.row_version)
        .fetch_optional()
        .await
        .map_err(map_catalog_write_error)?
        .ok_or_else(|| AppError::Conflict("รายวิชาถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()))
}

pub async fn list_subject_versions(
    pool: &PgPool,
    subject_id: Uuid,
) -> Result<Vec<SubjectVersion>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.list_subject_versions",
    )
    .await?;
    list_subject_versions_on_connection(&mut connection, subject_id).await
}

pub async fn list_subject_versions_on_connection(
    connection: &mut sqlx::PgConnection,
    subject_id: Uuid,
) -> Result<Vec<SubjectVersion>, AppError> {
    ensure_subject_exists(connection, subject_id).await?;
    Ok(DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.list_subject_versions_on_connection",
            sqlx::AssertSqlSafe(subject_version_select(
                "version.subject_id = $1",
                "version.version_no DESC",
            )),
        )
        .bind(subject_id)
        .fetch_all()
        .await?)
}

pub async fn get_subject_version(pool: &PgPool, id: Uuid) -> Result<SubjectVersion, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.get_subject_version",
    )
    .await?;
    get_subject_version_on_connection(&mut connection, id).await
}

pub async fn get_subject_version_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<SubjectVersion, AppError> {
    DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.get_subject_version_on_connection",
            sqlx::AssertSqlSafe(subject_version_select("version.id = $1", "version.id")),
        )
        .bind(id)
        .fetch_optional()
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
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.create_subject_version",
    )
    .await?;
    let (subject_code,): (String,) = DbSession::new(&mut *transaction)
        .query_as(
            "academic_core.services.catalog.create_subject_version",
            "SELECT code FROM subjects WHERE id = $1 FOR UPDATE",
        )
        .bind(subject_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชา".to_string()))?;
    ensure_subject_revision_differs(&mut transaction, subject_id, None, &request).await?;
    let version_no: i32 = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.catalog.create_subject_version",
            "SELECT COALESCE(max(version_no), 0) + 1 FROM subject_versions WHERE subject_id = $1",
        )
        .bind(subject_id)
        .fetch_one()
        .await?;
    let id = Uuid::now_v7();
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.catalog.create_subject_version",
            r#"
        INSERT INTO subject_versions (
            id, subject_id, version_no, code, name_th, name_en, credit,
            hours_per_semester, type, description, effective_from,
            term, is_active,
            periods_per_week, status
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
            $12, true, $13, 'draft'
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
        .bind(request.description)
        .bind(request.effective_from)
        .bind(request.term_code)
        .bind(request.periods_per_week)
        .execute()
        .await?;
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
        description: request.description,
        effective_from: request.effective_from,
        term_code: request.term_code,
        periods_per_week: request.periods_per_week,
        grade_level_ids: request.grade_level_ids,
    };
    parse_row_version(row_version)?;
    validate_subject_version(&values)?;
    let credit = validate_canonical_decimal(&values.credit, 2)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.update_subject_version",
    )
    .await?;
    let subject_id = lock_subject_for_version(&mut transaction, id).await?;
    let status: super::super::models::VersionStatus = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.catalog.update_subject_version",
            "SELECT status FROM subject_versions WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันรายวิชา".to_string()))?;
    ensure_draft_version(status)?;
    ensure_subject_revision_differs(&mut transaction, subject_id, Some(id), &values).await?;
    let result = DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.catalog.update_subject_version",
            r#"
        UPDATE subject_versions SET name_th = $1, name_en = $2, credit = $3,
            hours_per_semester = $4, type = $5, description = $6,
            effective_from = $7, term = $8,
            periods_per_week = $9,
            row_version = row_version + 1, updated_at = now()
        WHERE id = $10 AND row_version = $11 AND status = 'draft'
        "#,
        )
        .bind(values.name_th.trim())
        .bind(values.name_en)
        .bind(credit)
        .bind(values.hours_per_semester)
        .bind(values.subject_type.trim())
        .bind(values.description)
        .bind(values.effective_from)
        .bind(values.term_code)
        .bind(values.periods_per_week)
        .bind(id)
        .bind(row_version)
        .execute()
        .await?;
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
    parse_row_version(request.row_version)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.publish_subject_version",
    )
    .await?;
    let subject_id = lock_subject_for_version(&mut transaction, id).await?;
    let version: SubjectVersion = DbSession::new(&mut *transaction)
        .query_as(
            "academic_core.services.catalog.publish_subject_version",
            sqlx::AssertSqlSafe(subject_version_select("version.id = $1", "version.id")),
        )
        .bind(id)
        .fetch_one()
        .await?;
    ensure_draft_version(version.status)?;
    if version.row_version != request.row_version {
        return Err(AppError::Conflict(
            "เวอร์ชันรายวิชาถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    let values = subject_version_content(&version);
    if validate_canonical_decimal(&version.credit, 2)? <= sqlx::types::BigDecimal::from(0) {
        return Err(AppError::ValidationError(
            "หน่วยกิตต้องมากกว่า 0 ก่อนเผยแพร่เวอร์ชันรายวิชา".to_string(),
        ));
    }
    if !version.periods_per_week.is_some_and(|value| value >= 0) {
        return Err(AppError::ValidationError(
            "ระบุคาบมาตรฐานต่อสัปดาห์เป็น 0 หรือมากกว่าก่อนเผยแพร่เวอร์ชันรายวิชา".to_string(),
        ));
    }
    if !version.hours_per_semester.is_some_and(|value| value >= 0) {
        return Err(AppError::ValidationError(
            "ระบุชั่วโมงรวมต่อภาคเรียนเป็น 0 หรือมากกว่าก่อนเผยแพร่เวอร์ชันรายวิชา".to_string(),
        ));
    }
    ensure_subject_revision_differs(&mut transaction, subject_id, Some(id), &values).await?;
    DbSession::new(&mut *transaction).query("academic_core.services.catalog.publish_subject_version", "UPDATE subject_versions SET status = 'published', published_at = now(), row_version = row_version + 1, updated_at = now() WHERE id = $1")
        .bind(id).execute().await?;
    transaction.commit().await?;
    get_subject_version(pool, id).await
}

pub async fn list_activities(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<CatalogActivity>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.list_activities",
    )
    .await?;
    list_activities_on_connection(&mut connection, filter).await
}

pub async fn list_activities_on_connection(
    connection: &mut sqlx::PgConnection,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<CatalogActivity>, AppError> {
    let owner_ids = filter.allowed_organization_unit_ids();
    let sql = format!(
        "SELECT {ACTIVITY_COLUMNS} FROM activities \
         WHERE $1 OR owning_organization_unit_id = ANY($2) ORDER BY code, id LIMIT 500"
    );
    Ok(DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.list_activities_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(filter.includes_school_owned)
        .bind(owner_ids)
        .fetch_all()
        .await?)
}

pub async fn list_activity_overview(
    pool: &PgPool,
    read_filter: &AcademicResourceListFilter,
    manage_filter: &AcademicResourceListFilter,
    today: NaiveDate,
) -> Result<CatalogActivityOverview, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.list_activity_overview",
    )
    .await?;
    list_activity_overview_on_connection(&mut connection, read_filter, manage_filter, today).await
}

pub async fn list_activity_overview_on_connection(
    connection: &mut sqlx::PgConnection,
    read_filter: &AcademicResourceListFilter,
    manage_filter: &AcademicResourceListFilter,
    today: NaiveDate,
) -> Result<CatalogActivityOverview, AppError> {
    let activities = list_activities_on_connection(connection, read_filter).await?;
    let activity_ids = activities
        .iter()
        .map(|activity| activity.id)
        .collect::<Vec<_>>();
    let versions: Vec<ActivityVersion> = if activity_ids.is_empty() {
        Vec::new()
    } else {
        DbSession::new(connection)
            .query_as(
                "academic_core.services.catalog.list_activity_overview_on_connection",
                sqlx::AssertSqlSafe(activity_version_select(
                    "version.activity_id = ANY($1)",
                    "version.activity_id, version.version_no DESC",
                )),
            )
            .bind(&activity_ids)
            .fetch_all()
            .await?
    };
    let deletable_ids =
        deletable_catalog_ids(connection, CatalogKind::Activity, &activity_ids).await?;
    let catalog_grade_levels = list_catalog_grade_levels(connection).await?;
    let grade_level_options = active_catalog_grade_levels(&catalog_grade_levels);
    let can_create = owner_allowed(
        manage_filter,
        Some(resolve_activity_owner_on_connection(connection).await?),
    );
    let mut versions_by_activity = HashMap::<Uuid, Vec<ActivityVersion>>::new();
    for version in versions {
        versions_by_activity
            .entry(version.activity_id)
            .or_default()
            .push(version);
    }

    let items = activities
        .into_iter()
        .map(|activity| {
            let versions = versions_by_activity
                .remove(&activity.id)
                .unwrap_or_default();
            let (display_version, display_state, draft_count) =
                select_activity_display(&versions, today);
            let grade_levels = display_version
                .as_ref()
                .map(|version| {
                    resolve_grade_levels(&version.grade_level_ids, &catalog_grade_levels)
                })
                .unwrap_or_default();
            let can_manage =
                owner_allowed(manage_filter, Some(activity.owning_organization_unit_id));
            let can_delete = can_manage && deletable_ids.contains(&activity.id);
            CatalogActivityOverviewItem {
                can_delete,
                activity,
                display_version,
                display_state,
                draft_count,
                grade_levels,
                can_manage,
            }
        })
        .collect();

    Ok(CatalogActivityOverview {
        items,
        grade_level_options,
        can_create,
    })
}

pub async fn get_activity(pool: &PgPool, id: Uuid) -> Result<CatalogActivity, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.get_activity",
    )
    .await?;
    get_activity_on_connection(&mut connection, id).await
}

pub async fn get_activity_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<CatalogActivity, AppError> {
    let sql = format!("SELECT {ACTIVITY_COLUMNS} FROM activities WHERE id = $1");
    DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.get_activity_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรม".to_string()))
}

pub async fn create_activity(
    pool: &PgPool,
    request: CreateCatalogActivityRequest,
) -> Result<CatalogActivity, AppError> {
    let _preflight_activity_type = normalize_activity_type(&request.activity_type)?;

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.create_activity",
    )
    .await?;
    create_activity_on_connection(&mut connection, request).await
}

pub async fn create_activity_on_connection(
    connection: &mut sqlx::PgConnection,
    request: CreateCatalogActivityRequest,
) -> Result<CatalogActivity, AppError> {
    let activity_type = normalize_activity_type(&request.activity_type)?;
    let owner_id = resolve_activity_owner_on_connection(connection).await?;
    let id = Uuid::now_v7();
    let code = activity_code(id);
    let identity_key = format!("activity:{id}");
    let sql = format!(
        "INSERT INTO activities (id, code, identity_key, activity_type, owning_organization_unit_id) \
         VALUES ($1, $2, $3, $4, $5) RETURNING {ACTIVITY_COLUMNS}"
    );
    DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.create_activity_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .bind(code)
        .bind(identity_key)
        .bind(activity_type)
        .bind(owner_id)
        .fetch_one()
        .await
        .map_err(map_catalog_write_error)
}

pub async fn update_activity(
    pool: &PgPool,
    id: Uuid,
    request: UpdateCatalogActivityRequest,
) -> Result<CatalogActivity, AppError> {
    parse_row_version(request.row_version)?;
    let _preflight_activity_type = normalize_activity_type(&request.activity_type)?;

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.update_activity",
    )
    .await?;
    update_activity_on_connection(&mut connection, id, request).await
}

pub async fn update_activity_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
    request: UpdateCatalogActivityRequest,
) -> Result<CatalogActivity, AppError> {
    parse_row_version(request.row_version)?;
    let activity_type = normalize_activity_type(&request.activity_type)?;
    let sql = format!(
        "UPDATE activities SET activity_type = $1, \
         archived_at = CASE WHEN $2 THEN COALESCE(archived_at, now()) ELSE NULL END, \
         row_version = row_version + 1, updated_at = now() \
         WHERE id = $3 AND row_version = $4 RETURNING {ACTIVITY_COLUMNS}"
    );
    DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.update_activity_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(activity_type)
        .bind(request.archived)
        .bind(id)
        .bind(request.row_version)
        .fetch_optional()
        .await
        .map_err(map_catalog_write_error)?
        .ok_or_else(|| AppError::Conflict("กิจกรรมถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()))
}

fn activity_code(id: Uuid) -> String {
    format!("ACT-{}", id.simple()).to_uppercase()
}

fn normalize_activity_type(value: &str) -> Result<String, AppError> {
    let value = value.trim().to_lowercase();
    if value.is_empty() {
        return Err(AppError::ValidationError("ประเภทกิจกรรมห้ามว่าง".to_string()));
    }
    Ok(value)
}

#[cfg(test)]
mod activity_identity_tests {
    use super::*;

    #[test]
    fn generated_code_uses_the_full_stable_identity() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        assert_eq!(activity_code(first), "ACT-00000000000000000000000000000001");
        assert_ne!(activity_code(first), activity_code(second));
    }

    #[test]
    fn activity_type_is_normalized_and_cannot_be_blank() {
        assert_eq!(normalize_activity_type("  GUIDANCE  ").unwrap(), "guidance");
        assert!(normalize_activity_type("  ").is_err());
    }

    #[test]
    fn activity_write_contracts_refuse_client_codes() {
        assert!(
            serde_json::from_value::<CreateCatalogActivityRequest>(serde_json::json!({
                "activityType": "club"
            }))
            .is_ok()
        );
        assert!(
            serde_json::from_value::<CreateCatalogActivityRequest>(serde_json::json!({
                "activityType": "club", "code": "MANUAL"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<UpdateCatalogActivityRequest>(serde_json::json!({
                "activityType": "club", "archived": false, "rowVersion": 1, "code": "MANUAL"
            }))
            .is_err()
        );
    }
}

pub async fn list_activity_versions(
    pool: &PgPool,
    activity_id: Uuid,
) -> Result<Vec<ActivityVersion>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.list_activity_versions",
    )
    .await?;
    list_activity_versions_on_connection(&mut connection, activity_id).await
}

pub async fn list_activity_versions_on_connection(
    connection: &mut sqlx::PgConnection,
    activity_id: Uuid,
) -> Result<Vec<ActivityVersion>, AppError> {
    get_activity_on_connection(connection, activity_id).await?;
    Ok(DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.list_activity_versions_on_connection",
            sqlx::AssertSqlSafe(activity_version_select(
                "version.activity_id = $1",
                "version.version_no DESC",
            )),
        )
        .bind(activity_id)
        .fetch_all()
        .await?)
}

pub async fn get_activity_version(pool: &PgPool, id: Uuid) -> Result<ActivityVersion, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.get_activity_version",
    )
    .await?;
    get_activity_version_on_connection(&mut connection, id).await
}

pub async fn get_activity_version_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<ActivityVersion, AppError> {
    DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.get_activity_version_on_connection",
            sqlx::AssertSqlSafe(activity_version_select("version.id = $1", "version.id")),
        )
        .bind(id)
        .fetch_optional()
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
    let hours_per_term = request
        .hours_per_term
        .as_deref()
        .map(|value| validate_canonical_decimal(value, 2))
        .transpose()?;
    let grade_level_ids = sqlx::types::Json(request.grade_level_ids.clone());
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.create_activity_version",
    )
    .await?;
    let activity_type: String = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.catalog.create_activity_version",
            "SELECT activity_type FROM activities WHERE id = $1 FOR UPDATE",
        )
        .bind(activity_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรม".to_string()))?;
    let version_no: i32 = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.catalog.create_activity_version",
            "SELECT COALESCE(max(version_no), 0) + 1 FROM activity_versions WHERE activity_id = $1",
        )
        .bind(activity_id)
        .fetch_one()
        .await?;
    let legacy_periods = request
        .hours_per_week
        .split('.')
        .next()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(1)
        .max(1);
    let id = Uuid::now_v7();
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.catalog.create_activity_version",
            r#"
        INSERT INTO activity_versions (
            id, activity_id, version_no, name, activity_type, description,
            periods_per_week, hours_per_week, hours_per_term, scheduling_mode, is_active,
            term, grade_level_ids, effective_from, status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, true, $11, $12, $13, 'draft')
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
        .bind(hours_per_term)
        .bind(request.scheduling_mode.trim())
        .bind(request.term_code)
        .bind(grade_level_ids)
        .bind(request.effective_from)
        .execute()
        .await?;
    transaction.commit().await?;
    get_activity_version(pool, id).await
}

pub async fn update_activity_version(
    pool: &PgPool,
    id: Uuid,
    request: UpdateActivityVersionRequest,
) -> Result<ActivityVersion, AppError> {
    parse_row_version(request.row_version)?;
    validate_activity_version_fields(
        &request.name,
        &request.scheduling_mode,
        &request.hours_per_week,
        request.hours_per_term.as_deref(),
    )?;

    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.update_activity_version",
    )
    .await?;
    lock_activity_for_version(&mut transaction, id).await?;
    let version = update_activity_version_on_connection(&mut transaction, id, request).await?;
    transaction.commit().await?;
    Ok(version)
}

pub async fn update_activity_version_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
    request: UpdateActivityVersionRequest,
) -> Result<ActivityVersion, AppError> {
    let row_version = request.row_version;
    let values = CreateActivityVersionRequest {
        name: request.name,
        description: request.description,
        hours_per_week: request.hours_per_week,
        hours_per_term: request.hours_per_term,
        scheduling_mode: request.scheduling_mode,
        effective_from: request.effective_from,
        term_code: request.term_code,
        grade_level_ids: request.grade_level_ids,
    };
    parse_row_version(row_version)?;
    validate_activity_version(&values)?;
    let hours = validate_canonical_decimal(&values.hours_per_week, 2)?;
    let hours_per_term = values
        .hours_per_term
        .as_deref()
        .map(|value| validate_canonical_decimal(value, 2))
        .transpose()?;
    let grade_level_ids = sqlx::types::Json(values.grade_level_ids.clone());
    let result = DbSession::new(connection)
        .query(
            "academic_core.services.catalog.update_activity_version_on_connection",
            r#"
        UPDATE activity_versions SET name = $1, description = $2, hours_per_week = $3,
            hours_per_term = $4, scheduling_mode = $5, effective_from = $6,
            term = $7, grade_level_ids = $8, row_version = row_version + 1,
            updated_at = now()
        WHERE id = $9 AND row_version = $10 AND status = 'draft'
        "#,
        )
        .bind(values.name.trim())
        .bind(values.description)
        .bind(hours)
        .bind(hours_per_term)
        .bind(values.scheduling_mode.trim())
        .bind(values.effective_from)
        .bind(values.term_code)
        .bind(grade_level_ids)
        .bind(id)
        .bind(row_version)
        .execute()
        .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "เวอร์ชันกิจกรรมเผยแพร่แล้วหรือถูกแก้ไขโดยผู้ใช้อื่น".to_string(),
        ));
    }
    get_activity_version_on_connection(connection, id).await
}

pub async fn publish_activity_version(
    pool: &PgPool,
    id: Uuid,
    request: PublishVersionRequest,
) -> Result<ActivityVersion, AppError> {
    parse_row_version(request.row_version)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.publish_activity_version",
    )
    .await?;
    lock_activity_for_version(&mut transaction, id).await?;
    let (row_version, status, weekly_valid, total_valid): (i64, VersionStatus, bool, Option<bool>) =
        DbSession::new(&mut *transaction).query_as(
            "academic_core.services.catalog.publish_activity_version",
            "SELECT row_version, status, hours_per_week >= 0, hours_per_term >= 0 FROM activity_versions WHERE id = $1 FOR UPDATE",
        ).bind(id).fetch_one().await?;
    ensure_draft_version(status)?;
    if row_version != request.row_version {
        return Err(AppError::Conflict(
            "เวอร์ชันกิจกรรมถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    if !weekly_valid || total_valid != Some(true) {
        return Err(AppError::ValidationError(
            "ระบุชั่วโมงต่อสัปดาห์และชั่วโมงรวมต่อภาคเรียนเป็น 0 หรือมากกว่าก่อนเผยแพร่เวอร์ชันกิจกรรม".to_string(),
        ));
    }
    DbSession::new(&mut *transaction).query(
        "academic_core.services.catalog.publish_activity_version",
        "UPDATE activity_versions SET status = 'published', published_at = now(), row_version = row_version + 1, updated_at = now() WHERE id = $1",
    ).bind(id).execute().await?;
    let version = get_activity_version_on_connection(&mut transaction, id).await?;
    transaction.commit().await?;
    Ok(version)
}

pub async fn list_subject_default_teachers(
    pool: &PgPool,
    subject_id: Uuid,
) -> Result<Vec<DefaultTeacher>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.list_subject_default_teachers",
    )
    .await?;
    list_subject_default_teachers_on_connection(&mut connection, subject_id).await
}

pub async fn list_subject_default_teachers_on_connection(
    connection: &mut sqlx::PgConnection,
    subject_id: Uuid,
) -> Result<Vec<DefaultTeacher>, AppError> {
    Ok(DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.list_subject_default_teachers_on_connection",
            "SELECT instructor_id AS user_id, role FROM subject_default_instructors \
         WHERE subject_id = $1 ORDER BY role, instructor_id",
        )
        .bind(subject_id)
        .fetch_all()
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
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.list_activity_default_teachers",
    )
    .await?;
    list_activity_default_teachers_on_connection(&mut connection, activity_id).await
}

pub async fn list_activity_default_teachers_on_connection(
    connection: &mut sqlx::PgConnection,
    activity_id: Uuid,
) -> Result<Vec<DefaultTeacher>, AppError> {
    Ok(DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.list_activity_default_teachers_on_connection",
            "SELECT instructor_id AS user_id, role FROM activity_default_instructors \
         WHERE activity_id = $1 ORDER BY role, instructor_id",
        )
        .bind(activity_id)
        .fetch_all()
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

const SUBJECT_GROUP_SELECT: &str =
    "SELECT g.id, u.id AS organization_unit_id, g.code, g.name_th, g.name_en,
            g.display_order, g.is_active, g.row_version, g.created_at, g.updated_at
     FROM subject_groups g JOIN organization_units u ON u.subject_group_id = g.id";

pub async fn list_subject_groups(pool: &PgPool) -> Result<Vec<SubjectGroup>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.list_subject_groups",
    )
    .await?;
    list_subject_groups_on_connection(&mut connection).await
}

pub async fn list_subject_groups_on_connection(
    connection: &mut sqlx::PgConnection,
) -> Result<Vec<SubjectGroup>, AppError> {
    Ok(DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.list_subject_groups_on_connection",
            sqlx::AssertSqlSafe(format!(
                "{SUBJECT_GROUP_SELECT} ORDER BY g.display_order, g.code, g.id"
            )),
        )
        .fetch_all()
        .await?)
}

pub async fn create_subject_group(
    pool: &PgPool,
    request: CreateSubjectGroupRequest,
) -> Result<SubjectGroup, AppError> {
    validate_subject_group(&request.name_th, &request.name_en)?;
    if request.display_order.is_some_and(|order| order < 0) {
        return Err(AppError::ValidationError("ลำดับต้องไม่ติดลบ".to_string()));
    }
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.create_subject_group",
    )
    .await?;
    let code: String = DbSession::new(&mut *tx)
        .scalar(
            "academic_core.services.catalog.create_subject_group",
            "SELECT next_organization_code('SG')",
        )
        .fetch_one()
        .await?;
    let id: Uuid = DbSession::new(&mut *tx).scalar("academic_core.services.catalog.create_subject_group",
        "INSERT INTO subject_groups (code, name_th, name_en, display_order, is_active)
         VALUES ($1, $2, $3, COALESCE($4, (SELECT COALESCE(MAX(display_order), 0) + 1 FROM subject_groups)), true)
         RETURNING id",
    )
    .bind(code).bind(request.name_th.trim()).bind(request.name_en.trim())
    .bind(request.display_order).fetch_one().await.map_err(map_catalog_write_error)?;
    // The database trigger creates the membership/permission owner in this same transaction.
    let group = DbSession::new(&mut *tx)
        .query_as(
            "academic_core.services.catalog.create_subject_group",
            sqlx::AssertSqlSafe(format!("{SUBJECT_GROUP_SELECT} WHERE g.id = $1")),
        )
        .bind(id)
        .fetch_one()
        .await?;
    tx.commit().await?;
    Ok(group)
}

pub async fn update_subject_group(
    pool: &PgPool,
    id: Uuid,
    request: UpdateSubjectGroupRequest,
) -> Result<SubjectGroup, AppError> {
    parse_row_version(request.row_version)?;
    validate_subject_group(&request.name_th, &request.name_en)?;
    if request.display_order < 0 {
        return Err(AppError::ValidationError("ลำดับต้องไม่ติดลบ".to_string()));
    }
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.update_subject_group",
    )
    .await?;
    let updated = DbSession::new(&mut *tx)
        .query(
            "academic_core.services.catalog.update_subject_group",
            "UPDATE subject_groups SET name_th = $1, name_en = $2, display_order = $3,
            is_active = COALESCE($4, is_active), row_version = row_version + 1, updated_at = now()
         WHERE id = $5 AND row_version = $6",
        )
        .bind(request.name_th.trim())
        .bind(request.name_en.trim())
        .bind(request.display_order)
        .bind(request.is_active)
        .bind(id)
        .bind(request.row_version)
        .execute()
        .await
        .map_err(map_catalog_write_error)?;
    if updated.rows_affected() == 0 {
        return Err(AppError::Conflict("กลุ่มสาระถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    let group = DbSession::new(&mut *tx)
        .query_as(
            "academic_core.services.catalog.update_subject_group",
            sqlx::AssertSqlSafe(format!("{SUBJECT_GROUP_SELECT} WHERE g.id = $1")),
        )
        .bind(id)
        .fetch_one()
        .await?;
    tx.commit().await?;
    Ok(group)
}

#[derive(Debug, sqlx::FromRow)]
struct CatalogGradeLevelRow {
    id: Uuid,
    level_type: String,
    year: i32,
    is_active: bool,
}

struct CatalogGradeLevel {
    item: GradeLevelLookupItem,
    is_active: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct CatalogSubjectGroupRow {
    subject_group_id: Uuid,
    organization_unit_id: Uuid,
    code: String,
    name: String,
}

async fn list_catalog_grade_levels(
    connection: &mut sqlx::PgConnection,
) -> Result<Vec<CatalogGradeLevel>, AppError> {
    let rows = DbSession::new(connection)
        .query_as::<CatalogGradeLevelRow>(
            "academic_core.services.catalog.list_catalog_grade_levels",
            r#"
        SELECT id, level_type, year, is_active
        FROM grade_levels
        ORDER BY CASE level_type
                    WHEN 'kindergarten' THEN 1
                    WHEN 'primary' THEN 2
                    WHEN 'secondary' THEN 3
                    ELSE 4
                 END,
                 year,
                 id
        "#,
        )
        .fetch_all()
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| CatalogGradeLevel {
            is_active: row.is_active,
            item: grade_level_lookup_item(row),
        })
        .collect())
}

pub(super) async fn list_catalog_subject_group_options(
    connection: &mut sqlx::PgConnection,
    read_filter: &AcademicResourceListFilter,
    manage_filter: &AcademicResourceListFilter,
) -> Result<Vec<CatalogSubjectGroupOption>, AppError> {
    let read_owner_ids = read_filter.allowed_organization_unit_ids();
    if !read_filter.includes_school_owned && read_owner_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = DbSession::new(connection)
        .query_as::<CatalogSubjectGroupRow>(
            "academic_core.services.catalog.list_catalog_subject_group_options",
            r#"
        SELECT subject_group.id AS subject_group_id,
               owner.id AS organization_unit_id,
               subject_group.code,
               subject_group.name_th AS name
        FROM subject_groups subject_group
        JOIN organization_units owner
          ON owner.subject_group_id = subject_group.id
         AND owner.is_active = true
        WHERE subject_group.is_active = true
          AND ($1 OR owner.id = ANY($2))
        ORDER BY subject_group.display_order, subject_group.name_th, subject_group.id
        "#,
        )
        .bind(read_filter.includes_school_owned)
        .bind(read_owner_ids)
        .fetch_all()
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| CatalogSubjectGroupOption {
            subject_group_id: row.subject_group_id,
            organization_unit_id: row.organization_unit_id,
            code: row.code,
            name: row.name,
            can_manage: owner_allowed(manage_filter, Some(row.organization_unit_id)),
        })
        .collect())
}

pub async fn resolve_subject_group_owner(
    pool: &PgPool,
    subject_group_id: Uuid,
) -> Result<Uuid, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.resolve_subject_group_owner",
    )
    .await?;
    resolve_subject_group_owner_on_connection(&mut connection, subject_group_id).await
}

pub async fn resolve_subject_group_owner_on_connection(
    connection: &mut sqlx::PgConnection,
    subject_group_id: Uuid,
) -> Result<Uuid, AppError> {
    DbSession::new(connection)
        .scalar(
            "academic_core.services.catalog.resolve_subject_group_owner_on_connection",
            r#"
        SELECT owner.id
        FROM subject_groups subject_group
        JOIN organization_units owner
          ON owner.subject_group_id = subject_group.id
         AND owner.is_active = true
        WHERE subject_group.id = $1
          AND subject_group.is_active = true
        "#,
        )
        .bind(subject_group_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| {
            AppError::ValidationError("กลุ่มสาระนี้ยังไม่มีหน่วยงานผู้รับผิดชอบที่ใช้งานอยู่".to_string())
        })
}

pub async fn resolve_activity_owner(pool: &PgPool) -> Result<Uuid, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.resolve_activity_owner",
    )
    .await?;
    resolve_activity_owner_on_connection(&mut connection).await
}

pub async fn resolve_activity_owner_on_connection(
    connection: &mut sqlx::PgConnection,
) -> Result<Uuid, AppError> {
    DbSession::new(connection)
        .scalar(
            "academic_core.services.catalog.resolve_activity_owner_on_connection",
            r#"
        SELECT id
        FROM organization_units
        WHERE code = 'ACAD-ACT'
          AND is_active = true
        "#,
        )
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::ValidationError("ยังไม่ได้ตั้งค่าหน่วยงานกิจกรรมพัฒนาผู้เรียน".to_string()))
}

fn active_catalog_grade_levels(options: &[CatalogGradeLevel]) -> Vec<GradeLevelLookupItem> {
    options
        .iter()
        .filter(|option| option.is_active)
        .map(|option| option.item.clone())
        .collect()
}

fn grade_level_lookup_item(row: CatalogGradeLevelRow) -> GradeLevelLookupItem {
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
            format!("Other {}", row.year),
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

fn resolve_grade_levels(
    grade_level_ids: &[Uuid],
    options: &[CatalogGradeLevel],
) -> Vec<GradeLevelLookupItem> {
    options
        .iter()
        .filter(|option| grade_level_ids.contains(&option.item.id))
        .map(|option| option.item.clone())
        .collect()
}

fn select_subject_display(
    versions: &[SubjectVersion],
    today: NaiveDate,
) -> (Option<SubjectVersion>, CatalogDisplayState, i64) {
    let draft_count = versions
        .iter()
        .filter(|version| version.status == VersionStatus::Draft)
        .count() as i64;
    let display = select_display_version(
        versions
            .iter()
            .filter(|version| version.status == VersionStatus::Published),
        today,
        |version| version.effective_from,
        |version| version.version_no,
    );
    match display {
        Some((version, state)) => (Some(version.clone()), state, draft_count),
        None => (None, CatalogDisplayState::Unpublished, draft_count),
    }
}

fn select_activity_display(
    versions: &[ActivityVersion],
    today: NaiveDate,
) -> (Option<ActivityVersion>, CatalogDisplayState, i64) {
    let draft_count = versions
        .iter()
        .filter(|version| version.status == VersionStatus::Draft)
        .count() as i64;
    let display = select_display_version(
        versions
            .iter()
            .filter(|version| version.status == VersionStatus::Published),
        today,
        |version| version.effective_from,
        |version| version.version_no,
    );
    match display {
        Some((version, state)) => (Some(version.clone()), state, draft_count),
        None => (None, CatalogDisplayState::Unpublished, draft_count),
    }
}

fn select_display_version<'a, T: 'a>(
    versions: impl Iterator<Item = &'a T>,
    today: NaiveDate,
    effective_from: impl Fn(&T) -> NaiveDate,
    version_no: impl Fn(&T) -> i32,
) -> Option<(&'a T, CatalogDisplayState)> {
    let versions = versions.collect::<Vec<_>>();
    select_current_subject_revision(versions.iter().copied(), today, |version| {
        (effective_from(version), version_no(version))
    })
    .map(|version| (version, CatalogDisplayState::Current))
    .or_else(|| {
        versions
            .into_iter()
            .filter(|version| effective_from(version) > today)
            .min_by_key(|version| {
                (
                    effective_from(version),
                    std::cmp::Reverse(version_no(version)),
                )
            })
            .map(|version| (version, CatalogDisplayState::Upcoming))
    })
}

fn subject_version_select(predicate: &str, order: &str) -> String {
    format!(
        r#"
        SELECT version.id, version.subject_id, version.version_no, version.name_th,
               version.name_en, version.credit::text AS credit,
               version.hours_per_semester, version.type AS subject_type,
               version.description, version.effective_from,
               version.term AS term_code, version.periods_per_week,
               ARRAY(SELECT link.grade_level_id FROM subject_version_grade_levels link
                     WHERE link.subject_id = version.id ORDER BY link.grade_level_id) AS grade_level_ids,
               version.status, version.published_at, version.row_version,
               (version.migration_provenance - 'historicalCatalogContext') <> '{{}}'::jsonb AS migrated,
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
               version.hours_per_term::text AS hours_per_term,
               version.scheduling_mode, version.effective_from,
               version.term AS term_code,
               COALESCE(ARRAY(SELECT jsonb_array_elements_text(COALESCE(version.grade_level_ids, '[]'::jsonb))::uuid), ARRAY[]::uuid[]) AS grade_level_ids,
               version.status, version.published_at, version.row_version,
               (version.migration_provenance - 'historicalCatalogContext') <> '{{}}'::jsonb AS migrated,
               version.created_at, version.updated_at
        FROM activity_versions version
        WHERE {predicate}
        ORDER BY {order}
        "#
    )
}

fn validate_subject_version(request: &CreateSubjectVersionRequest) -> Result<(), AppError> {
    validate_canonical_decimal(&request.credit, 2)?;
    if request.periods_per_week.is_some_and(|value| value < 0)
        || request.hours_per_semester.is_some_and(|value| value < 0)
    {
        return Err(AppError::ValidationError(
            "คาบมาตรฐานต่อสัปดาห์และชั่วโมงรวมต่อภาคเรียนต้องเป็น 0 หรือมากกว่า".to_string(),
        ));
    }
    if request.name_th.trim().is_empty() || request.subject_type.trim().is_empty() {
        return Err(AppError::ValidationError(
            "ชื่อและประเภทรายวิชาห้ามว่าง".to_string(),
        ));
    }
    Ok(())
}

fn validate_activity_version(request: &CreateActivityVersionRequest) -> Result<(), AppError> {
    validate_activity_version_fields(
        &request.name,
        &request.scheduling_mode,
        &request.hours_per_week,
        request.hours_per_term.as_deref(),
    )
}

fn validate_activity_version_fields(
    name: &str,
    scheduling_mode: &str,
    hours_per_week: &str,
    hours_per_term: Option<&str>,
) -> Result<(), AppError> {
    validate_canonical_decimal(hours_per_week, 2)?;
    hours_per_term
        .map(|value| validate_canonical_decimal(value, 2))
        .transpose()?;
    if name.trim().is_empty() || !matches!(scheduling_mode, "synchronized" | "independent") {
        return Err(AppError::ValidationError(
            "ข้อมูลเวอร์ชันกิจกรรมไม่ถูกต้อง".to_string(),
        ));
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

fn validate_subject_group(name_th: &str, name_en: &str) -> Result<(), AppError> {
    if name_th.trim().is_empty()
        || name_th.trim().chars().count() > 200
        || name_en.trim().chars().count() > 200
    {
        return Err(AppError::ValidationError("ข้อมูลกลุ่มสาระไม่ครบถ้วน".to_string()));
    }
    Ok(())
}

async fn ensure_subject_exists(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = DbSession::new(connection)
        .scalar(
            "academic_core.services.catalog.ensure_subject_exists",
            "SELECT EXISTS (SELECT 1 FROM subjects WHERE id = $1)",
        )
        .bind(id)
        .fetch_one()
        .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("ไม่พบรายวิชา".to_string()))
    }
}

// Every subject-version command locks the stable identity before reading revision content.
// This orders create, draft updates and publication without mutating published history.
async fn lock_subject_for_version(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Uuid, AppError> {
    DbSession::new(&mut **transaction).scalar("academic_core.services.catalog.lock_subject_for_version", "SELECT subject.id FROM subjects subject JOIN subject_versions version ON version.subject_id = subject.id WHERE version.id = $1 FOR UPDATE OF subject")
        .bind(id).fetch_optional().await?
        .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันรายวิชา".to_string()))
}

async fn lock_activity_for_version(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Uuid, AppError> {
    DbSession::new(&mut **transaction)
        .scalar("academic_core.services.catalog.lock_activity_for_version",
            "SELECT activity.id FROM activities activity JOIN activity_versions version ON version.activity_id = activity.id WHERE version.id = $1 FOR UPDATE OF activity")
        .bind(id).fetch_optional().await?
        .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันกิจกรรม".to_string()))
}

fn subject_version_content(version: &SubjectVersion) -> CreateSubjectVersionRequest {
    CreateSubjectVersionRequest {
        name_th: version.name_th.clone(),
        name_en: version.name_en.clone(),
        credit: version.credit.clone(),
        hours_per_semester: version.hours_per_semester,
        subject_type: version.subject_type.clone(),
        description: version.description.clone(),
        effective_from: version.effective_from,
        term_code: version.term_code.clone(),
        periods_per_week: version.periods_per_week,
        grade_level_ids: version.grade_level_ids.clone(),
    }
}

async fn ensure_subject_revision_differs(
    transaction: &mut Transaction<'_, Postgres>,
    subject_id: Uuid,
    excluded_id: Option<Uuid>,
    request: &CreateSubjectVersionRequest,
) -> Result<(), AppError> {
    let mut grades = request.grade_level_ids.clone();
    grades.sort_unstable();
    grades.dedup();
    // Compare the revision applicable at the requested date, rather than every historical
    // revision: restoring A after B remains valid. Other drafts participate regardless of date.
    let duplicate: bool = DbSession::new(&mut **transaction)
        .scalar(
            "academic_core.services.catalog.ensure_subject_revision_differs",
            r#"WITH applicable AS (
            SELECT id FROM subject_versions
            WHERE subject_id = $1 AND status = 'published'
              AND ($2::uuid IS NULL OR id <> $2)
              AND effective_from <= $3
            ORDER BY effective_from DESC, version_no DESC LIMIT 1
        )
        SELECT EXISTS (
            SELECT 1 FROM subject_versions version
            WHERE subject_id = $1 AND ($2::uuid IS NULL OR id <> $2)
              AND (status = 'draft' OR id IN (SELECT id FROM applicable))
              AND name_th = $4 AND name_en IS NOT DISTINCT FROM $5
              AND credit = $6 AND hours_per_semester IS NOT DISTINCT FROM $7
              AND type = $8 AND description IS NOT DISTINCT FROM $9
              AND effective_from = $3
              AND term IS NOT DISTINCT FROM $10 AND periods_per_week IS NOT DISTINCT FROM $11
              AND ARRAY(SELECT grade_level_id FROM subject_version_grade_levels
                        WHERE subject_id = version.id ORDER BY grade_level_id) = $12::uuid[]
        )"#,
        )
        .bind(subject_id)
        .bind(excluded_id)
        .bind(request.effective_from)
        .bind(request.name_th.trim())
        .bind(&request.name_en)
        .bind(validate_canonical_decimal(&request.credit, 2)?)
        .bind(request.hours_per_semester)
        .bind(request.subject_type.trim())
        .bind(&request.description)
        .bind(&request.term_code)
        .bind(request.periods_per_week)
        .bind(grades)
        .fetch_one()
        .await?;
    if duplicate {
        return Err(AppError::Conflict(
            "ข้อมูลเวอร์ชันรายวิชาซ้ำกับเวอร์ชันที่ใช้อยู่หรือฉบับร่าง".to_string(),
        ));
    }
    Ok(())
}

async fn replace_subject_grade_levels(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
    grade_level_ids: Vec<Uuid>,
) -> Result<(), AppError> {
    DbSession::new(&mut **transaction)
        .query(
            "academic_core.services.catalog.replace_subject_grade_levels",
            "DELETE FROM subject_version_grade_levels WHERE subject_id = $1",
        )
        .bind(version_id)
        .execute()
        .await?;
    let mut grade_level_ids = grade_level_ids;
    grade_level_ids.sort_unstable();
    grade_level_ids.dedup();
    for grade_level_id in grade_level_ids {
        DbSession::new(&mut **transaction).query("academic_core.services.catalog.replace_subject_grade_levels",
            "INSERT INTO subject_version_grade_levels (subject_id, grade_level_id) VALUES ($1, $2)",
        )
        .bind(version_id)
        .bind(grade_level_id)
        .execute()
        .await?;
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
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.catalog.replace_default_teachers",
    )
    .await?;
    let lock_sql = format!("SELECT row_version FROM {stable_table} WHERE id = $1 FOR UPDATE");
    let actual: i64 = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.catalog.replace_default_teachers",
            sqlx::AssertSqlSafe(lock_sql),
        )
        .bind(resource_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบทรัพยากรคลัง".to_string()))?;
    if actual != request.row_version {
        return Err(AppError::Conflict("ข้อมูลถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    let delete_sql = format!("DELETE FROM {link_table} WHERE {foreign_key} = $1");
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.catalog.replace_default_teachers",
            sqlx::AssertSqlSafe(delete_sql),
        )
        .bind(resource_id)
        .execute()
        .await?;
    let insert_sql = format!(
        "INSERT INTO {link_table} (id, {foreign_key}, instructor_id, role) VALUES ($1, $2, $3, $4)"
    );
    for teacher in request.teachers {
        DbSession::new(&mut *transaction)
            .query(
                "academic_core.services.catalog.replace_default_teachers",
                sqlx::AssertSqlSafe(insert_sql.as_str()),
            )
            .bind(Uuid::new_v4())
            .bind(resource_id)
            .bind(teacher.user_id)
            .bind(teacher.role)
            .execute()
            .await?;
    }
    let update_sql = format!(
        "UPDATE {stable_table} SET row_version = row_version + 1, updated_at = now() WHERE id = $1"
    );
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.catalog.replace_default_teachers",
            sqlx::AssertSqlSafe(update_sql),
        )
        .bind(resource_id)
        .execute()
        .await?;
    transaction.commit().await?;
    Ok(())
}

fn map_catalog_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database
            .message()
            .contains("LEARNING_AREA_AFFILIATION_INVALID")
        {
            return AppError::Conflict(
                "กลุ่มสาระปิดใช้งานหรือข้อมูลสังกัดเปลี่ยน กรุณาโหลดทะเบียนใหม่".to_string(),
            );
        }
        if database.message().contains("LEARNING_AREA_IN_USE") {
            return AppError::Conflict(
                "กลุ่มสาระยังมีรายวิชาที่ใช้งานอยู่ หรือเป็นทะเบียนกิจกรรมพัฒนาผู้เรียน".to_string(),
            );
        }
        if database.code().as_deref() == Some("23505") {
            return AppError::Conflict("รหัสหรือข้อมูลประจำรายการซ้ำ".to_string());
        }
    }
    AppError::DbError(error)
}

/// Minimal catalogue metadata for already-authorized subject versions.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SubjectVersionSubjectGroup {
    pub subject_version_id: Uuid,
    pub subject_group_id: Uuid,
    pub name: String,
    pub display_order: Option<i32>,
}

pub async fn subject_groups_for_versions(
    pool: &PgPool,
    version_ids: &[Uuid],
) -> Result<Vec<SubjectVersionSubjectGroup>, AppError> {
    if version_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.catalog.subject_groups_for_versions",
    )
    .await?;
    subject_groups_for_versions_on_connection(&mut connection, version_ids).await
}

pub async fn subject_groups_for_versions_on_connection(
    connection: &mut sqlx::PgConnection,
    version_ids: &[Uuid],
) -> Result<Vec<SubjectVersionSubjectGroup>, AppError> {
    if version_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(DbSession::new(connection)
        .query_as(
            "academic_core.services.catalog.subject_groups_for_versions_on_connection",
            "SELECT version.id AS subject_version_id, sg.id AS subject_group_id,
                sg.name_th AS name, sg.display_order
         FROM subject_versions version
         JOIN subjects subject ON subject.id = version.subject_id
         JOIN subject_groups sg ON sg.id = subject.subject_group_id
         WHERE version.id = ANY($1) ORDER BY version.id",
        )
        .bind(version_ids)
        .fetch_all()
        .await?)
}

#[cfg(test)]
#[path = "catalog_revision_selection_tests.rs"]
mod revision_selection_tests;

#[cfg(test)]
mod pool_facade_tests {
    use super::*;
    async fn closed_pool() -> sqlx::PgPool {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        pool
    }

    #[tokio::test]
    async fn pool_facade_preflight_empty_catalog_metadata_needs_no_database() {
        let pool = closed_pool().await;
        assert!(subject_groups_for_versions(&pool, &[])
            .await
            .unwrap()
            .is_empty());
    }
    #[tokio::test]
    async fn pool_facade_preflight_scope_denials_precede_checkout() {
        let pool = closed_pool().await;
        let filter = school_authorization::AcademicResourceListFilter::default();
        assert!(matches!(
            crate::services::curriculum::list(&pool, &filter).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            crate::services::workspaces::curriculum_create_options(&pool, &filter).await,
            Err(AppError::Forbidden(_))
        ));
        let actor = school_authorization::ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![],
        };
        assert!(matches!(
            crate::services::workspaces::setup_workspace(&pool, &actor).await,
            Err(AppError::Forbidden(_))
        ));
    }
    #[tokio::test]
    async fn pool_facade_preflight_conflicting_view_is_a_validation_error() {
        let pool = closed_pool().await;
        let query = crate::models::CurriculumViewQuery {
            draft_id: Some(Uuid::new_v4()),
            publication_id: Some(Uuid::new_v4()),
        };
        assert!(matches!(
            crate::services::curriculum_publications::resolve_view(&pool, Uuid::new_v4(), &query)
                .await,
            Err(AppError::ValidationError(_))
        ));
    }
    #[tokio::test]
    async fn pool_facade_exhausted_catalog_and_scope_paths_need_no_available_connection() {
        let pool = school_test_db::create_named_test_pool_with_max_connections(
            "catalog_facade_exhausted",
            1,
        )
        .await;
        let held = pool.acquire().await.unwrap();
        let filter = school_authorization::AcademicResourceListFilter::default();
        tokio::time::timeout(std::time::Duration::from_millis(250), async {
            assert!(subject_groups_for_versions(&pool, &[])
                .await
                .unwrap()
                .is_empty());
            assert!(matches!(
                crate::services::curriculum::list(&pool, &filter).await,
                Err(AppError::Forbidden(_))
            ));
            assert!(matches!(
                crate::services::workspaces::curriculum_create_options(&pool, &filter).await,
                Err(AppError::Forbidden(_))
            ));
        })
        .await
        .expect("pure facade paths must finish while the only connection is held");
        drop(held);
        pool.close().await;
    }
}
