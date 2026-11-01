use crate::collaboration::{LifecycleOutcome, TimetableMutationContext, VersionMutationPermit};
use school_database::DbSession;
use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::timetable_block_conflicts::map_write_error;
use crate::models::timetable_version::{
    CloneTimetableVersionRequest, CreateTimetableVersionRequest, TimetableVersion,
    TimetableVersionDisplayState, TimetableVersionStatus, TimetableVersionTarget,
};
use school_academic_core::services::lifecycle_guard;
use school_errors::AppError;

pub fn bangkok_date_at(now: chrono::DateTime<Utc>) -> NaiveDate {
    (now + chrono::Duration::hours(7)).date_naive()
}
pub fn bangkok_today() -> NaiveDate {
    bangkok_date_at(Utc::now())
}

pub mod opening;
mod term_preparation;
pub use term_preparation::apply as apply_term_preparation;

pub async fn pending_term_work(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<Vec<school_academic_core::ports::PendingTermWork>, AppError> {
    Ok(DbSession::new(&mut **tx).query_as("academic_timetable.services.timetable_version_service.pending_term_work",
        "SELECT version.id,
         md5(to_jsonb(version)::text || COALESCE((SELECT jsonb_agg(to_jsonb(block) ORDER BY block.id)::text
             FROM academic_timetable_blocks block WHERE block.timetable_version_id=version.id),'[]')) AS revision,
         false AS blocks_closure FROM academic_timetable_versions version WHERE academic_year_id=$1
         AND academic_term_id=$2 AND status='draft' ORDER BY version.id",
    ).bind(year).bind(term).fetch_all().await?)
}

#[derive(Debug, Clone, FromRow)]
struct TimetableVersionRow {
    publication_no: Option<i64>,
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    effective_from: Option<NaiveDate>,
    effective_until: Option<NaiveDate>,
    status: TimetableVersionStatus,
    source_version_id: Option<Uuid>,
    delivery_version_id: Uuid,
    bell_schedule_id: Uuid,
    row_version: i64,
    created_by: Option<Uuid>,
    published_by: Option<Uuid>,
    published_at: Option<chrono::DateTime<Utc>>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct CloneSourceRow {
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    status: TimetableVersionStatus,
    row_version: i64,
    bell_schedule_id: Uuid,
}

const VERSION_SELECT: &str = r#"
    SELECT version.publication_no,version.id,
           version.academic_term_id,
           version.academic_year_id,
           version.effective_from,
           CASE
               WHEN version.status = 'published' THEN (
                   (
                       SELECT next_version.effective_from - 1
                       FROM academic_timetable_versions next_version
                       WHERE next_version.academic_term_id = version.academic_term_id
                         AND next_version.status = 'published'
                         AND next_version.publication_no > version.publication_no
                       ORDER BY next_version.effective_from, next_version.id
                       LIMIT 1
                   )
               )
               ELSE NULL
           END AS effective_until,
           version.status,
           version.source_version_id,
           version.delivery_version_id,
           version.bell_schedule_id,
           version.row_version,
           version.created_by,
           version.published_by,
           version.published_at,
           version.created_at,
           version.updated_at
    FROM academic_timetable_versions version
    JOIN academic_terms term ON term.id = version.academic_term_id
"#;

pub(crate) fn derive_display_state(
    effective_from: NaiveDate,
    effective_until: Option<NaiveDate>,
    today: NaiveDate,
) -> TimetableVersionDisplayState {
    if effective_until.is_some_and(|end| end < effective_from) {
        TimetableVersionDisplayState::Superseded
    } else if today < effective_from {
        TimetableVersionDisplayState::Upcoming
    } else if effective_until.is_some_and(|end| today > end) {
        TimetableVersionDisplayState::Historical
    } else {
        TimetableVersionDisplayState::Current
    }
}

pub async fn list_versions(
    pool: &PgPool,
    term_id: Uuid,
) -> Result<Vec<TimetableVersion>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.services.timetable_version_service.list_versions",
    )
    .await?;
    list_versions_on_connection(&mut connection, term_id).await
}

pub async fn list_versions_on_connection(
    connection: &mut sqlx::PgConnection,
    term_id: Uuid,
) -> Result<Vec<TimetableVersion>, AppError> {
    let sql = format!(
        "{VERSION_SELECT} WHERE version.academic_term_id = $1 \
         ORDER BY version.publication_no DESC NULLS FIRST, \
                  CASE version.status WHEN 'draft' THEN 0 WHEN 'published' THEN 1 ELSE 2 END, \
                  version.id"
    );
    let rows = DbSession::new(connection)
        .query_as::<TimetableVersionRow>(
            "academic_timetable.services.timetable_version_service.list_versions_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(term_id)
        .fetch_all()
        .await?;
    hydrate_versions(connection, rows, bangkok_today()).await
}

/// Public read DTOs expose only opening targets within the timetable read scope.
pub async fn restrict_targets(
    pool: &PgPool,
    versions: &mut [TimetableVersion],
    access: &crate::policy::TimetableAccessFilter,
) -> Result<(), AppError> {
    if access.includes_school_owned {
        return Ok(());
    }

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.services.timetable_version_service.restrict_targets",
    )
    .await?;
    restrict_targets_on_connection(&mut connection, versions, access).await
}

pub async fn restrict_targets_on_connection(
    connection: &mut sqlx::PgConnection,
    versions: &mut [TimetableVersion],
    access: &crate::policy::TimetableAccessFilter,
) -> Result<(), AppError> {
    if access.includes_school_owned {
        return Ok(());
    }
    let ids = versions
        .iter()
        .map(|version| version.delivery_version_id)
        .collect::<Vec<_>>();
    let snapshots =
        school_academic_delivery::services::versions::snapshots_on_connection(connection, &ids)
            .await?;
    for version in versions {
        let snapshot = snapshots
            .get(&version.delivery_version_id)
            .ok_or_else(|| AppError::Conflict("ไม่พบรุ่นเปิดสอนของตาราง".into()))?;
        version.targets.retain(|target| {
            snapshot.offerings.iter().any(|offering| {
                offering.id == target.learning_offering_id
                    && (access
                        .organization_unit_ids
                        .contains(&offering.owning_organization_unit_id)
                        || access
                            .organization_tree_unit_ids
                            .contains(&offering.owning_organization_unit_id)
                        || access.assigned_actor_id.is_some_and(|actor| {
                            offering.groups.iter().any(|group| {
                                group
                                    .teachers
                                    .iter()
                                    .any(|teacher| teacher.teacher_id == actor)
                            })
                        }))
            })
        });
    }
    Ok(())
}

pub async fn resolve_for_date(
    pool: &PgPool,
    term_id: Uuid,
    on_date: NaiveDate,
) -> Result<TimetableVersion, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.services.timetable_version_service.resolve_for_date",
    )
    .await?;
    resolve_for_date_on_connection(&mut connection, term_id, on_date).await
}

pub async fn resolve_for_date_on_connection(
    connection: &mut sqlx::PgConnection,
    term_id: Uuid,
    on_date: NaiveDate,
) -> Result<TimetableVersion, AppError> {
    let version_id =
        resolve_version_id_for_date_on_connection(connection, term_id, on_date).await?;
    let sql = format!("{VERSION_SELECT} WHERE version.id = $1");
    let row = DbSession::new(connection)
        .query_as::<TimetableVersionRow>(
            "academic_timetable.services.timetable_version_service.resolve_for_date_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(version_id)
        .fetch_one()
        .await?;
    let mut versions = hydrate_versions(connection, vec![row], on_date).await?;
    versions.pop().ok_or_else(|| {
        AppError::InternalServerError("ไม่สามารถโหลดตารางเรียนตามวันที่เลือกได้".to_string())
    })
}

pub async fn resolve_version_id_for_date(
    pool: &PgPool,
    term_id: Uuid,
    on_date: NaiveDate,
) -> Result<Uuid, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.services.timetable_version_service.resolve_version_id_for_date",
    )
    .await?;
    resolve_version_id_for_date_on_connection(&mut connection, term_id, on_date).await
}
pub async fn resolve_version_id_for_date_on_connection(
    connection: &mut sqlx::PgConnection,
    term_id: Uuid,
    on_date: NaiveDate,
) -> Result<Uuid, AppError> {
    DbSession::new(connection).scalar::<Option<Uuid>>("academic_timetable.services.timetable_version_service.resolve_version_id_for_date_on_connection",
        "SELECT academic_timetable_applicable_publication($1,$2)",
    )
    .bind(term_id)
    .bind(on_date)
    .fetch_one()
    .await?
    .ok_or_else(|| AppError::NotFound(format!("ไม่พบตารางเรียนที่เผยแพร่และมีผลในวันที่ {on_date}")))
}

/// Start the first timetable after independently publishing its opening graph.
/// Term serialization also makes a repeated request resume the same blank draft.
pub async fn create_initial_draft(
    pool: &PgPool,
    context: &TimetableMutationContext,
    request: CreateTimetableVersionRequest,
) -> Result<TimetableVersion, AppError> {
    let drafts: Vec<Uuid> = {
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            pool, "academic_timetable.services.timetable_version_service.create_initial_draft.resolve_destination",
        ).await?;
        DbSession::new(&mut connection).scalar("academic_timetable.services.timetable_version_service.create_initial_draft.resolve_destination",
            "SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='draft' ORDER BY created_at,id")
            .bind(request.academic_term_id).fetch_all().await?
    };
    if let [id] = drafts.as_slice() {
        context
            .execute_lifecycle_outcome(pool, *id, move |pool, permit| async move {
                create_initial_draft_in_permit(
                    &pool,
                    VersionMutationPermit::Existing(permit),
                    request,
                )
                .await
            })
            .await
    } else {
        context
            .execute_creation(pool, move |pool, permit| async move {
                let outcome = create_initial_draft_in_permit(
                    &pool,
                    VersionMutationPermit::Creation(permit),
                    request,
                )
                .await?;
                Ok(match outcome {
                    LifecycleOutcome::Changed(value) | LifecycleOutcome::Unchanged(value) => value,
                })
            })
            .await
    }
}

async fn create_initial_draft_in_permit(
    pool: &PgPool,
    permit: VersionMutationPermit,
    request: CreateTimetableVersionRequest,
) -> Result<LifecycleOutcome<TimetableVersion>, AppError> {
    let actor = permit.actor_id();
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_timetable.services.timetable_version_service.create_initial_draft",
    )
    .await?;
    let (year, bell): (Uuid, Option<Uuid>) = DbSession::new(&mut *tx)
        .query_as(
            "academic_timetable.services.timetable_version_service.create_initial_draft",
            "SELECT academic_year_id,bell_schedule_id FROM academic_terms WHERE id=$1",
        )
        .bind(request.academic_term_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    lifecycle_guard::require_term_write_exclusive(&mut tx, year, request.academic_term_id).await?;
    let bell = bell.ok_or_else(|| {
        AppError::ValidationError("ตั้งค่าตารางเวลาของภาคเรียนก่อนสร้างตารางสอน".into())
    })?;
    let published: bool = DbSession::new(&mut *tx).scalar("academic_timetable.services.timetable_version_service.create_initial_draft", "SELECT EXISTS(SELECT 1 FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published')")
        .bind(request.academic_term_id).fetch_one().await?;
    if published {
        return Err(AppError::Conflict(
            "มีตารางที่เผยแพร่แล้ว กรุณาเปิดรุ่นต้นทางแล้วกดแก้ไข".into(),
        ));
    }
    let drafts: Vec<(Uuid,i64)> = DbSession::new(&mut *tx).query_as("academic_timetable.services.timetable_version_service.create_initial_draft", "SELECT id,row_version FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='draft' ORDER BY created_at,id FOR UPDATE")
        .bind(request.academic_term_id).fetch_all().await?;
    if drafts.len() > 1 {
        return Err(AppError::Conflict(
            "มีหลายแบบร่าง กรุณาเลือกแบบร่างที่จะจัดต่อ".into(),
        ));
    }
    let (id, changed) = if let Some((id, revision)) = drafts.first() {
        let changed =
            super::timetable_lifecycle::align_resumed_source(&mut tx, &permit, *id, *revision)
                .await?;
        (*id, changed)
    } else {
        if matches!(&permit, VersionMutationPermit::Existing(_)) {
            return Err(AppError::Conflict(
                "แบบร่างที่เลือกถูกลบแล้ว กรุณาโหลดข้อมูลใหม่".into(),
            ));
        }
        let delivery = school_academic_delivery::services::versions::latest_published_id(
            &mut tx,
            request.academic_term_id,
        )
        .await?;
        let id = Uuid::now_v7();
        DbSession::new(&mut *tx).query("academic_timetable.services.timetable_version_service.create_initial_draft", "INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,effective_from,status,delivery_version_id,bell_schedule_id,created_by) VALUES($1,$2,$3,NULL,'draft',$4,$5,$6)")
            .bind(id).bind(request.academic_term_id).bind(year).bind(delivery).bind(bell).bind(actor).execute().await?;
        DbSession::new(&mut *tx).query("academic_timetable.services.timetable_version_service.create_initial_draft", "INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload) VALUES('academic_timetable_version.draft_created','academic_timetable_version',$1,$2,$3,$4,$5)")
            .bind(id).bind(year).bind(request.academic_term_id).bind(actor).bind(serde_json::json!({"deliveryVersionId":delivery})).execute().await?;
        (id, true)
    };
    let version = get_version_on_connection(&mut tx, id, bangkok_today()).await?;
    permit.commit(tx).await?;
    Ok(if changed {
        LifecycleOutcome::Changed(version)
    } else {
        LifecycleOutcome::Unchanged(version)
    })
}

pub async fn clone_draft(
    pool: &PgPool,
    context: &TimetableMutationContext,
    source_id: Uuid,
    request: CloneTimetableVersionRequest,
) -> Result<TimetableVersion, AppError> {
    let existing: Vec<Uuid> = {
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            pool,
            "academic_timetable.services.timetable_version_service.clone_draft.resolve_destination",
        )
        .await?;
        DbSession::new(&mut connection).scalar("academic_timetable.services.timetable_version_service.clone_draft.resolve_destination", "SELECT id FROM academic_timetable_versions WHERE source_version_id=$1 AND status='draft' ORDER BY id").bind(source_id).fetch_all().await?
    };
    let target = request
        .resume_draft_id
        .or_else(|| (existing.len() == 1).then(|| existing[0]));
    if let Some(id) = target {
        context
            .execute_lifecycle_outcome(pool, id, move |pool, permit| async move {
                clone_draft_in_permit(
                    &pool,
                    VersionMutationPermit::Existing(permit),
                    source_id,
                    request,
                )
                .await
            })
            .await
    } else {
        context
            .execute_creation(pool, move |pool, permit| async move {
                let outcome = clone_draft_in_permit(
                    &pool,
                    VersionMutationPermit::Creation(permit),
                    source_id,
                    request,
                )
                .await?;
                Ok(match outcome {
                    LifecycleOutcome::Changed(value) | LifecycleOutcome::Unchanged(value) => value,
                })
            })
            .await
    }
}

async fn clone_draft_in_permit(
    pool: &PgPool,
    permit: VersionMutationPermit,
    source_id: Uuid,
    request: CloneTimetableVersionRequest,
) -> Result<LifecycleOutcome<TimetableVersion>, AppError> {
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_timetable.services.timetable_version_service.clone_draft",
    )
    .await?;
    let (id, changed) =
        clone_draft_in_transaction(&mut transaction, &permit, source_id, request).await?;
    let result = get_version_on_connection(&mut transaction, id, bangkok_today()).await?;
    permit.commit(transaction).await?;
    Ok(if changed {
        LifecycleOutcome::Changed(result)
    } else {
        LifecycleOutcome::Unchanged(result)
    })
}

/// Coordinate before version/block locks. Resolve immutable IDs without row
/// locks and acquire the term write mode initially, including nested Delivery
/// callers which already hold that same term lock.
pub(crate) async fn lock_version_term_context(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
) -> Result<lifecycle_guard::AcademicWriteState, AppError> {
    let (year_id, term_id): (Uuid, Uuid) = DbSession::new(&mut **transaction).query_as("academic_timetable.services.timetable_version_service.require_version_term_write",
        "SELECT academic_year_id, academic_term_id FROM academic_timetable_versions WHERE id=$1",
    )
    .bind(version_id)
    .fetch_optional()
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางสอน".into()))?;
    lifecycle_guard::lock_context_exclusive(transaction, year_id, term_id).await
}

pub(crate) async fn require_version_term_write(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
) -> Result<(), AppError> {
    lock_version_term_context(transaction, version_id)
        .await?
        .require_writable()
}

/// Content decisions hold the term before reading the latest publication and the draft.
/// Alignment and deletion intentionally use the independent lifecycle write guard.
pub(crate) async fn require_current_draft_write(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
) -> Result<(), AppError> {
    require_version_term_write(transaction, version_id).await?;
    let (term, source, status): (Uuid, Uuid, TimetableVersionStatus) = DbSession::new(&mut **transaction)
        .query_as("academic_timetable.services.timetable_version_service.require_current_draft_write",
            "SELECT academic_term_id,delivery_version_id,status FROM academic_timetable_versions WHERE id=$1 FOR UPDATE")
        .bind(version_id).fetch_optional().await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางสอน".into()))?;
    if status != TimetableVersionStatus::Draft {
        return Err(AppError::Conflict("แก้ไขได้เฉพาะรุ่นตารางสอนแบบร่าง".into()));
    }
    let latest =
        school_academic_delivery::services::versions::latest_published_id(transaction, term)
            .await?;
    if source != latest {
        return Err(super::timetable_source::outdated_draft_source());
    }
    Ok(())
}

async fn clone_draft_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    permit: &VersionMutationPermit,
    source_id: Uuid,
    request: CloneTimetableVersionRequest,
) -> Result<(Uuid, bool), AppError> {
    let actor_id = permit.actor_id();
    let source_row_version = request.source_row_version;
    if source_row_version <= 0 {
        return Err(AppError::ValidationError(
            "sourceRowVersion ต้องมากกว่าศูนย์".to_string(),
        ));
    }

    require_version_term_write(transaction, source_id).await?;
    let source: CloneSourceRow = DbSession::new(&mut **transaction)
        .query_as(
            "academic_timetable.services.timetable_version_service.clone_draft_in_transaction",
            r#"SELECT source.id,
                  source.academic_term_id,
                  source.academic_year_id,
                  source.status,
                  source.row_version,
                  term.bell_schedule_id
           FROM academic_timetable_versions source
           JOIN academic_terms term ON term.id = source.academic_term_id
           JOIN academic_years year ON year.id = source.academic_year_id
           WHERE source.id = $1
           FOR UPDATE OF source"#,
        )
        .bind(source_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางเรียนต้นทาง".to_string()))?;

    if source.status != TimetableVersionStatus::Published {
        return Err(AppError::Conflict(
            "สร้างแบบร่างใหม่ได้จากรุ่นตารางเรียนที่เผยแพร่แล้วเท่านั้น".to_string(),
        ));
    }
    if source.row_version != source_row_version {
        return Err(AppError::Conflict(format!(
            "รุ่นตารางเรียนต้นทางถูกแก้ไขแล้ว (expected {}, actual {})",
            source_row_version, source.row_version
        )));
    }
    let delivery_version_id = school_academic_delivery::services::versions::latest_published_id(
        transaction,
        source.academic_term_id,
    )
    .await?;
    let drafts: Vec<(Uuid, i64)> = DbSession::new(&mut **transaction).query_as("academic_timetable.services.timetable_version_service.clone_draft_in_transaction", "SELECT id,row_version FROM academic_timetable_versions WHERE source_version_id=$1 AND status='draft' ORDER BY created_at,id FOR UPDATE")
        .bind(source.id).fetch_all().await?;
    let resume = match request.resume_draft_id {
        Some(id) => Some(
            *drafts
                .iter()
                .find(|(draft_id, _)| *draft_id == id)
                .ok_or_else(|| AppError::Conflict("ไม่พบแบบร่างจากรุ่นต้นทางที่เลือก".into()))?,
        ),
        None if drafts.len() == 1 => drafts.first().copied(),
        None if drafts.len() > 1 => {
            return Err(AppError::Conflict(
                "มีหลายแบบร่างจากรุ่นนี้ กรุณาเลือกแบบร่างที่ต้องการแก้ต่อ".into(),
            ));
        }
        None => None,
    };
    if let Some((id, revision)) = resume {
        permit.require_existing(id)?;
        if request
            .draft_row_version
            .is_some_and(|expected| expected != revision)
        {
            return Err(AppError::Conflict("แบบร่างถูกแก้ไข กรุณาโหลดข้อมูลใหม่".into()));
        }
        let changed =
            super::timetable_lifecycle::align_resumed_source(transaction, permit, id, revision)
                .await?;
        return Ok((id, changed));
    }

    if matches!(permit, VersionMutationPermit::Existing(_)) {
        return Err(AppError::Conflict(
            "Selected draft disappeared; reload before creating another".into(),
        ));
    }
    let new_version_id = Uuid::now_v7();
    DbSession::new(&mut **transaction)
        .query(
            "academic_timetable.services.timetable_version_service.clone_draft_in_transaction",
            r#"INSERT INTO academic_timetable_versions (
               id, academic_term_id, academic_year_id, effective_from, status,
               source_version_id, delivery_version_id, bell_schedule_id, created_by
           ) VALUES ($1, $2, $3, NULL, 'draft', $4, $5, $6, $7)"#,
        )
        .bind(new_version_id)
        .bind(source.academic_term_id)
        .bind(source.academic_year_id)
        .bind(source.id)
        .bind(delivery_version_id)
        .bind(source.bell_schedule_id)
        .bind(actor_id)
        .execute()
        .await?;

    // UUIDv5 maps each source identity consistently across the bulk child copies.
    // Read the source graph directly: runtime roles intentionally cannot create TEMP tables.
    DbSession::new(&mut **transaction)
        .query(
            "academic_timetable.services.timetable_version_service.clone_draft_in_transaction",
            r#"INSERT INTO academic_timetable_blocks (
               id, timetable_version_id, academic_term_id, academic_year_id,
               bell_schedule_id, bell_schedule_period_id, day_of_week,
               block_kind, scheduling_mode, learning_offering_id, structural_kind,
               title, note, series_id, row_version, is_active, migration_provenance,
               created_by, updated_by, created_at, updated_at
           )
           SELECT uuid_generate_v5($1, 'block:' || source.id::text),
                  $1, source.academic_term_id, source.academic_year_id,
                  source.bell_schedule_id, source.bell_schedule_period_id, source.day_of_week,
                  source.block_kind, source.scheduling_mode, source.learning_offering_id,
                  source.structural_kind, source.title, source.note,
                  CASE WHEN source.series_id IS NULL THEN NULL
                       ELSE uuid_generate_v5($1, 'series:' || source.series_id::text) END,
                  1, true,
                  source.migration_provenance || jsonb_build_object(
                      'clonedFromBlockId', source.id::text,
                      'sourceVersionId', $2::text
                  ),
                  $3, $3, now(), now()
           FROM academic_timetable_blocks source
           WHERE source.timetable_version_id = $2 AND source.is_active"#,
        )
        .bind(new_version_id)
        .bind(source.id)
        .bind(actor_id)
        .execute()
        .await
        .map_err(map_write_error)?;
    DbSession::new(&mut **transaction)
        .query(
            "academic_timetable.services.timetable_version_service.clone_draft_in_transaction",
            r#"INSERT INTO academic_timetable_block_groups (
               id, block_id, learning_group_id, learning_offering_id,
               academic_term_id, academic_year_id, room_id, row_version, homeroom_ids,
               is_active, migration_provenance, created_by, updated_by,
               created_at, updated_at
           )
           SELECT uuid_generate_v5($1, 'block-group:' || source.id::text),
                  uuid_generate_v5($1, 'block:' || source.block_id::text),
                  source.learning_group_id,
                  source.learning_offering_id, source.academic_term_id,
                  source.academic_year_id, source.room_id, 1, source.homeroom_ids, true,
                  source.migration_provenance || jsonb_build_object(
                      'clonedFromBlockGroupId', source.id::text,
                      'sourceVersionId', $2::text
                  ),
                  $3, $3, now(), now()
           FROM academic_timetable_block_groups source
           JOIN academic_timetable_blocks block ON block.id = source.block_id
           WHERE block.timetable_version_id = $2 AND block.is_active AND source.is_active"#,
        )
        .bind(new_version_id)
        .bind(source.id)
        .bind(actor_id)
        .execute()
        .await
        .map_err(map_write_error)?;
    DbSession::new(&mut **transaction)
        .query(
            "academic_timetable.services.timetable_version_service.clone_draft_in_transaction",
            r#"INSERT INTO academic_timetable_block_group_instructors (
               id, block_group_id, instructor_id, role, display_order,
               row_version, created_at, updated_at
           )
           SELECT gen_random_uuid(), uuid_generate_v5($1, 'block-group:' || source.block_group_id::text),
                  source.instructor_id,
                  source.role, source.display_order, 1, now(), now()
           FROM academic_timetable_block_group_instructors source
           JOIN academic_timetable_block_groups block_group ON block_group.id = source.block_group_id
           JOIN academic_timetable_blocks block ON block.id = block_group.block_id
           WHERE block.timetable_version_id = $2 AND block.is_active AND block_group.is_active"#,
        )
        .bind(new_version_id)
        .bind(source.id)
        .execute()
        .await
        .map_err(map_write_error)?;
    DbSession::new(&mut **transaction)
        .query(
            "academic_timetable.services.timetable_version_service.clone_draft_in_transaction",
            r#"INSERT INTO academic_timetable_block_homerooms (
               id, block_id, homeroom_id, academic_term_id, academic_year_id,
               target_kind, room_id, row_version, is_active, migration_provenance,
               created_by, updated_by, created_at, updated_at
           )
           SELECT uuid_generate_v5($1, 'homeroom-target:' || source.id::text),
                  uuid_generate_v5($1, 'block:' || source.block_id::text),
                  source.homeroom_id, source.academic_term_id,
                  source.academic_year_id, source.target_kind, source.room_id,
                  1, true,
                  source.migration_provenance || jsonb_build_object(
                      'clonedFromTargetId', source.id::text,
                      'sourceVersionId', $2::text
                  ),
                  $3, $3, now(), now()
           FROM academic_timetable_block_homerooms source
           JOIN academic_timetable_blocks block ON block.id = source.block_id
           WHERE block.timetable_version_id = $2 AND block.is_active AND source.is_active"#,
        )
        .bind(new_version_id)
        .bind(source.id)
        .bind(actor_id)
        .execute()
        .await
        .map_err(map_write_error)?;
    DbSession::new(&mut **transaction)
        .query(
            "academic_timetable.services.timetable_version_service.clone_draft_in_transaction",
            r#"INSERT INTO academic_timetable_block_teachers (
               id, block_id, teacher_id, academic_term_id, academic_year_id,
               row_version, is_active, migration_provenance,
               created_by, updated_by, created_at, updated_at
           )
           SELECT uuid_generate_v5($1, 'teacher-target:' || source.id::text),
                  uuid_generate_v5($1, 'block:' || source.block_id::text),
                  source.teacher_id, source.academic_term_id,
                  source.academic_year_id, 1, true,
                  source.migration_provenance || jsonb_build_object(
                      'clonedFromTargetId', source.id::text,
                      'sourceVersionId', $2::text
                  ),
                  $3, $3, now(), now()
           FROM academic_timetable_block_teachers source
           JOIN academic_timetable_blocks block ON block.id = source.block_id
           WHERE block.timetable_version_id = $2 AND block.is_active AND source.is_active"#,
        )
        .bind(new_version_id)
        .bind(source.id)
        .bind(actor_id)
        .execute()
        .await
        .map_err(map_write_error)?;
    DbSession::new(&mut **transaction)
        .query(
            "academic_timetable.services.timetable_version_service.clone_draft_in_transaction",
            r#"INSERT INTO academic_timetable_block_group_sync (
               id, block_id, learning_group_id, learning_offering_id,
               academic_term_id, academic_year_id, status, linked_block_group_id,
               conflict_code, conflict_message, attempted_group_row_version,
               row_version, created_by, updated_by, created_at, updated_at
           )
           SELECT uuid_generate_v5($1, 'sync:' || source.id::text),
                  uuid_generate_v5($1, 'block:' || source.block_id::text),
                  source.learning_group_id,
                  source.learning_offering_id, source.academic_term_id,
                  source.academic_year_id, source.status,
                  uuid_generate_v5($1, 'block-group:' || linked_group.id::text),
                  source.conflict_code, source.conflict_message,
                  source.attempted_group_row_version, 1, $3, $3, now(), now()
           FROM academic_timetable_block_group_sync source
           JOIN academic_timetable_blocks block ON block.id = source.block_id
           LEFT JOIN (
               SELECT block_group.id
               FROM academic_timetable_block_groups block_group
               JOIN academic_timetable_blocks source_block ON source_block.id = block_group.block_id
               WHERE source_block.timetable_version_id = $2
                 AND source_block.is_active AND block_group.is_active
           ) linked_group ON linked_group.id = source.linked_block_group_id
           WHERE block.timetable_version_id = $2 AND block.is_active"#,
        )
        .bind(new_version_id)
        .bind(source.id)
        .bind(actor_id)
        .execute()
        .await
        .map_err(map_write_error)?;

    let (source_block_count, cloned_block_count, source_child_count, cloned_child_count): (
        i64,
        i64,
        i64,
        i64,
    ) = DbSession::new(&mut **transaction).query_as("academic_timetable.services.timetable_version_service.clone_draft_in_transaction",
        r#"SELECT
               (SELECT count(*) FROM academic_timetable_blocks
                WHERE timetable_version_id = $1 AND is_active),
               (SELECT count(*) FROM academic_timetable_blocks
                WHERE timetable_version_id = $2 AND is_active),
               (SELECT count(*) FROM academic_timetable_block_groups child
                JOIN academic_timetable_blocks block ON block.id = child.block_id
                WHERE block.timetable_version_id = $1 AND block.is_active AND child.is_active)
                 + (SELECT count(*) FROM academic_timetable_block_homerooms child
                    JOIN academic_timetable_blocks block ON block.id = child.block_id
                    WHERE block.timetable_version_id = $1 AND block.is_active AND child.is_active)
                 + (SELECT count(*) FROM academic_timetable_block_teachers child
                    JOIN academic_timetable_blocks block ON block.id = child.block_id
                    WHERE block.timetable_version_id = $1 AND block.is_active AND child.is_active),
               (SELECT count(*) FROM academic_timetable_block_groups child
                JOIN academic_timetable_blocks block ON block.id = child.block_id
                WHERE block.timetable_version_id = $2 AND block.is_active AND child.is_active)
                 + (SELECT count(*) FROM academic_timetable_block_homerooms child
                    JOIN academic_timetable_blocks block ON block.id = child.block_id
                    WHERE block.timetable_version_id = $2 AND block.is_active AND child.is_active)
                 + (SELECT count(*) FROM academic_timetable_block_teachers child
                    JOIN academic_timetable_blocks block ON block.id = child.block_id
                    WHERE block.timetable_version_id = $2 AND block.is_active AND child.is_active)"#,
    )
    .bind(source.id)
    .bind(new_version_id)
    .fetch_one()
    .await?;
    if source_block_count != cloned_block_count || source_child_count != cloned_child_count {
        return Err(AppError::InternalServerError(
            "คัดลอกรุ่นตารางสอนไม่ครบถ้วน กรุณาลองใหม่".to_string(),
        ));
    }
    DbSession::new(&mut **transaction).query("academic_timetable.services.timetable_version_service.clone_draft_in_transaction", "INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload) VALUES('academic_timetable_version.draft_created','academic_timetable_version',$1,$2,$3,$4,$5)")
        .bind(new_version_id).bind(source.academic_year_id).bind(source.academic_term_id).bind(actor_id)
        .bind(sqlx::types::Json(serde_json::json!({"sourceVersionId":source.id,"deliveryVersionId":delivery_version_id,"copiedBlockCount":cloned_block_count}))).execute().await?;
    Ok((new_version_id, true))
}

pub async fn get_version(
    pool: &PgPool,
    version_id: Uuid,
    display_date: NaiveDate,
) -> Result<TimetableVersion, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.services.timetable_version_service.get_version",
    )
    .await?;
    get_version_on_connection(&mut connection, version_id, display_date).await
}

/// Content triggers change only these fields. Preserve the same pinned source metadata without
/// re-reading its full immutable delivery graph for every lesson edit.
pub(crate) async fn refresh_content_revision_on_connection(
    connection: &mut sqlx::PgConnection,
    mut version: TimetableVersion,
) -> Result<TimetableVersion, AppError> {
    let (revision, updated_at): (i64, chrono::DateTime<Utc>) = DbSession::new(connection)
        .query_as(
            "academic_timetable.services.timetable_version_service.refresh_content_revision",
            "SELECT row_version,updated_at FROM academic_timetable_versions WHERE id=$1",
        )
        .bind(version.id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("Timetable version not found".into()))?;
    version.row_version = revision;
    version.updated_at = updated_at;
    Ok(version)
}

pub async fn get_version_on_connection(
    connection: &mut sqlx::PgConnection,
    version_id: Uuid,
    display_date: NaiveDate,
) -> Result<TimetableVersion, AppError> {
    let sql = format!("{VERSION_SELECT} WHERE version.id = $1");
    let row = DbSession::new(connection)
        .query_as::<TimetableVersionRow>(
            "academic_timetable.services.timetable_version_service.get_version_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(version_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางเรียน".to_string()))?;
    let mut versions = hydrate_versions(connection, vec![row], display_date).await?;
    versions
        .pop()
        .ok_or_else(|| AppError::InternalServerError("ไม่สามารถโหลดรุ่นตารางเรียนได้".to_string()))
}

async fn hydrate_versions(
    connection: &mut sqlx::PgConnection,
    rows: Vec<TimetableVersionRow>,
    display_date: NaiveDate,
) -> Result<Vec<TimetableVersion>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let source_ids: Vec<Uuid> = rows.iter().map(|row| row.delivery_version_id).collect();
    let snapshots = school_academic_delivery::services::versions::snapshots_on_connection(
        connection,
        &source_ids,
    )
    .await?;
    let mut targets_by_version: HashMap<Uuid, Vec<TimetableVersionTarget>> = HashMap::new();
    for row in &rows {
        let snapshot = snapshots
            .get(&row.delivery_version_id)
            .ok_or_else(|| AppError::Conflict("รุ่นเปิดสอนของตารางไม่พร้อม กรุณาติดต่อผู้ดูแลระบบ".into()))?;
        targets_by_version.insert(
            row.id,
            snapshot
                .offerings
                .iter()
                .map(|offering| TimetableVersionTarget {
                    timetable_version_id: row.id,
                    learning_offering_id: offering.id,
                    weekly_period_target: offering.weekly_period_target,
                    standard_periods_per_week: match &offering.catalog {
                        school_academic_delivery::models::LearningOfferingSnapshot::Course(
                            course,
                        ) => Some(course.standard_periods_per_week),
                        school_academic_delivery::models::LearningOfferingSnapshot::Activity(_) => {
                            None
                        }
                    },
                })
                .collect(),
        );
    }

    Ok(rows
        .into_iter()
        .map(|row| {
            let display_state = row
                .effective_from
                .filter(|_| row.status == TimetableVersionStatus::Published)
                .map(|date| derive_display_state(date, row.effective_until, display_date));
            TimetableVersion {
                publication_no: row.publication_no,
                id: row.id,
                academic_term_id: row.academic_term_id,
                academic_year_id: row.academic_year_id,
                effective_from: row.effective_from,
                effective_until: row.effective_until,
                status: row.status,
                display_state,
                source_version_id: row.source_version_id,
                delivery_version_id: row.delivery_version_id,
                bell_schedule_id: row.bell_schedule_id,
                row_version: row.row_version,
                created_by: row.created_by,
                published_by: row.published_by,
                published_at: row.published_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
                targets: targets_by_version.remove(&row.id).unwrap_or_default(),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::derive_display_state;
    use crate::models::timetable_version::TimetableVersionDisplayState;
    use chrono::NaiveDate;

    #[test]
    fn bangkok_display_date_and_superseded_future_plan_are_truthful() {
        let instant = chrono::DateTime::parse_from_rfc3339("2026-10-31T18:30:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let today = super::bangkok_date_at(instant);
        assert_eq!(today, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(
            derive_display_state(today, None, today),
            TimetableVersionDisplayState::Current
        );
        assert_eq!(
            derive_display_state(
                today + chrono::Duration::days(4),
                Some(today - chrono::Duration::days(1)),
                today
            ),
            TimetableVersionDisplayState::Superseded
        );
    }

    #[test]
    fn display_state_uses_the_effective_interval() {
        let today = NaiveDate::from_ymd_opt(2027, 6, 15).unwrap();

        assert_eq!(
            derive_display_state(NaiveDate::from_ymd_opt(2027, 7, 1).unwrap(), None, today,),
            TimetableVersionDisplayState::Upcoming
        );
        assert_eq!(
            derive_display_state(
                NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
                Some(NaiveDate::from_ymd_opt(2027, 6, 30).unwrap()),
                today,
            ),
            TimetableVersionDisplayState::Current
        );
        assert_eq!(
            derive_display_state(
                NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
                Some(NaiveDate::from_ymd_opt(2027, 6, 14).unwrap()),
                today,
            ),
            TimetableVersionDisplayState::Historical
        );
    }
}

#[cfg(test)]
mod pool_facade_preflight_tests {
    use super::*;
    async fn closed_pool() -> sqlx::PgPool {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        pool
    }

    #[tokio::test]
    async fn pool_facade_preflight_school_target_grant_needs_no_database() {
        let pool = closed_pool().await;
        let access = crate::policy::TimetableAccessFilter {
            includes_school_owned: true,
            ..Default::default()
        };
        restrict_targets(&pool, &mut [], &access).await.unwrap();
    }
}
